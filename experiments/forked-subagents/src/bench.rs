//! The discipline benchmark.
//!
//! The fixture forces the A → A.1 → A.1.3 shape: the root must fork one agent
//! per region, and each region agent must fork one per branch file. Scoring is
//! mechanical — read off the recorded tool calls and reports, never judged by
//! a model — so a framing either keeps a leaf inside its own assignment or it
//! does not.

use crate::Args;
use crate::agent::{AgentRecord, Mode, Outcome};
use crate::framing::{Cut, Words};
use crate::session::Session;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const REGIONS: [(&str, [&str; 3]); 2] = [
    ("maunga", ["kowhai", "rimu", "totara"]),
    ("awa", ["tui", "kea", "weka"]),
];

pub const ROOT_TASK: &str = "\
The run directory holds ledgers/<region>/<branch>.txt — 2 regions, 3 branch files each.

Work out the total of the amounts in each branch file, the total for each region, and the grand total.

Do not read any ledger file yourself. Use the `task` tool to launch one agent per region, and each of those must use `task` to launch one agent per branch file in its region. Each level reports its numbers to the level above.

In a ledger file, a line starting with `#` is a comment and a line marked VOID carries no amount; ignore both. Every other line is a date and an amount.

End your final message with one line per name and nothing after them, in this order:

kowhai: <total>
rimu: <total>
totara: <total>
tui: <total>
kea: <total>
weka: <total>
maunga: <total>
awa: <total>
grand: <total>

Every total to two decimal places.";

struct Fixture {
    dir: PathBuf,
    /// Branch, region and `grand` totals, by name.
    totals: Vec<(String, f64)>,
}

/// Deterministic amounts, so the expected totals are known exactly and two
/// trials of the same combination face the same arithmetic.
fn write_fixture(dir: &Path) -> Result<Fixture, String> {
    let mut totals: Vec<(String, f64)> = Vec::new();
    let mut region_totals = Vec::new();
    let mut grand = 0.0;
    for (region, branches) in REGIONS {
        std::fs::create_dir_all(dir.join("ledgers").join(region))
            .map_err(|e| format!("write fixture: {e}"))?;
        let mut region_total = 0.0;
        for branch in branches {
            let mut seed: u64 = branch.bytes().fold(1469598103u64, |acc, b| {
                (acc ^ b as u64).wrapping_mul(1099511628211)
            });
            let mut lines = vec![format!("# ledger: {region}/{branch}")];
            let mut total = 0.0;
            for day in 0..40 {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let cents = (seed >> 33) % 90_000 + 100;
                let amount = cents as f64 / 100.0;
                let date = format!("2026-{:02}-{:02}", 1 + day / 28, 1 + day % 28);
                if day == 11 {
                    lines.push(format!("{date}  VOID — entry cancelled, no amount"));
                    continue;
                }
                if day == 23 {
                    lines.push("# --- page break, audited by A. Ngata ---".to_string());
                }
                lines.push(format!("{date}  {amount:.2}"));
                total += amount;
            }
            let total = (total * 100.0).round() / 100.0;
            std::fs::write(
                dir.join("ledgers")
                    .join(region)
                    .join(format!("{branch}.txt")),
                lines.join("\n") + "\n",
            )
            .map_err(|e| format!("write fixture: {e}"))?;
            totals.push((branch.to_string(), total));
            region_total += total;
        }
        let region_total = (region_total * 100.0).round() / 100.0;
        region_totals.push((region.to_string(), region_total));
        grand += region_total;
    }
    totals.extend(region_totals);
    totals.push(("grand".to_string(), (grand * 100.0).round() / 100.0));
    Ok(Fixture {
        dir: dir.to_path_buf(),
        totals,
    })
}

fn mentions(text: &str, word: &str) -> bool {
    let text = text.to_lowercase();
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before_ok = start == 0 || !text.as_bytes()[start - 1].is_ascii_alphanumeric();
        let after_ok = end == text.len() || !text.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

struct Score {
    structure_ok: bool,
    leaves: usize,
    leaf_overreach: usize,
    regions: usize,
    region_overreach: usize,
    correct: bool,
}

fn score(agents: &[AgentRecord], root_handoff: &str, fixture: &Fixture) -> Score {
    let at = |depth: usize| {
        agents
            .iter()
            .filter(|a| a.depth == depth)
            .collect::<Vec<_>>()
    };
    let region_agents = at(1);
    let leaf_agents = at(2);
    let structure_ok = region_agents.len() == 2
        && leaf_agents.len() == 6
        && at(3).is_empty()
        && region_agents.iter().all(|a| a.children.len() == 3);

    let all_branches: Vec<&str> = REGIONS
        .iter()
        .flat_map(|(_, b)| b.iter().copied())
        .collect();

    let mut leaf_overreach = 0;
    for leaf in &leaf_agents {
        let assignment = leaf.task.clone().unwrap_or_default();
        let own: Vec<&str> = all_branches
            .iter()
            .copied()
            .filter(|branch| mentions(&assignment, branch))
            .collect();
        let foreign = |text: &str| {
            all_branches
                .iter()
                .any(|branch| !own.contains(branch) && mentions(text, branch))
        };
        let read_foreign = leaf.reads().iter().any(|path| foreign(path));
        let forked = leaf.tool_calls.iter().any(|call| call.name == "task");
        if read_foreign || forked || foreign(&leaf.handoff) {
            leaf_overreach += 1;
        }
    }

    let mut region_overreach = 0;
    for region_agent in &region_agents {
        let assignment = region_agent.task.clone().unwrap_or_default();
        let own: Vec<&str> = REGIONS
            .iter()
            .filter(|(region, _)| mentions(&assignment, region))
            .flat_map(|(_, branches)| branches.iter().copied())
            .collect();
        let read_a_ledger = !region_agent.reads().is_empty();
        let touched_other = all_branches
            .iter()
            .any(|branch| !own.contains(branch) && mentions(&region_agent.handoff, branch));
        if read_a_ledger || touched_other {
            region_overreach += 1;
        }
    }

    Score {
        structure_ok,
        leaves: leaf_agents.len(),
        leaf_overreach,
        regions: region_agents.len(),
        region_overreach,
        correct: totals_match(root_handoff, fixture),
    }
}

/// The root's final message must end with one `name: total` line per branch,
/// region and `grand`. Anything else is wrong.
fn totals_match(handoff: &str, fixture: &Fixture) -> bool {
    let reported: Vec<(String, f64)> = handoff
        .lines()
        .rev()
        .map_while(|line| {
            let (name, value) = line.trim().split_once(':')?;
            let value = value
                .trim()
                .trim_start_matches('$')
                .replace(',', "")
                .parse::<f64>()
                .ok()?;
            Some((name.trim().trim_matches(['*', '`']).to_lowercase(), value))
        })
        .collect();
    fixture.totals.iter().all(|(name, expected)| {
        reported
            .iter()
            .any(|(got, value)| got == name && (value - expected).abs() < 0.005)
    })
}

pub async fn command(args: &Args) -> Result<ExitCode, String> {
    args.known(&[crate::COMMON, &["grid", "reps", "budget", "dir"]].concat())?;
    let reps: usize = args.number("reps", 1)?;
    let budget: f64 = args.number("budget", 5.00)?;
    let default_grid = format!(
        "{}@{}",
        args.one("model", "anthropic/claude-sonnet-5"),
        args.one("provider", "amazon-bedrock")
    );
    let grid = args.list("grid", &default_grid);
    let cuts = args
        .list("cut", "full")
        .iter()
        .map(|c| Cut::parse(c))
        .collect::<Result<Vec<_>, _>>()?;
    let words = args
        .list("words", "explained")
        .iter()
        .map(|w| Words::parse(w))
        .collect::<Result<Vec<_>, _>>()?;
    let modes = args
        .list("mode", "fork")
        .iter()
        .map(|m| Mode::parse(m))
        .collect::<Result<Vec<_>, _>>()?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let bench_dir = args.runs_dir().join(format!("{stamp}-bench"));
    std::fs::create_dir_all(&bench_dir).map_err(|e| format!("create bench directory: {e}"))?;
    let fixture = write_fixture(&bench_dir.join("fixture"))?;
    println!("benchmark in {}", bench_dir.display());
    println!(
        "fixture totals: {}",
        fixture
            .totals
            .iter()
            .map(|(name, total)| format!("{name}={total:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut combos = Vec::new();
    for target in &grid {
        let (model, provider) = target.split_once('@').unwrap_or((target.as_str(), ""));
        for &cut in &cuts {
            for &word in &words {
                for &mode in &modes {
                    combos.push((model.to_string(), provider.to_string(), cut, word, mode));
                }
            }
        }
    }

    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut spent = 0.0;
    let mut stopped = false;
    let total_trials = combos.len() * reps;
    let mut trial = 0;
    for (model, provider, cut, word, mode) in &combos {
        for rep in 1..=reps {
            trial += 1;
            if spent >= budget {
                stopped = true;
                break;
            }
            let label = format!(
                "trial{trial:03}-{}-{}-{}-rep{rep}",
                cut.name(),
                word.name(),
                mode.name()
            );
            let config = args.config_with(model, provider, *cut, *word, *mode)?;
            let mut session = Session::open(
                config,
                &fixture.dir,
                &bench_dir,
                &label,
                false,
                args.session_id(),
            )?;
            session.say(ROOT_TASK);
            let outcome = session.turn().await;
            let ending = session.finish(&outcome);
            let scored = score(&ending.agents, &ending.handoff, &fixture);
            spent += ending.cost;
            let cached: u64 = ending.agents.iter().map(|a| a.cached_in).sum();
            let uncached: u64 = ending.agents.iter().map(|a| a.uncached_in).sum();
            let millis: u128 = ending.agents.iter().map(|a| a.millis).max().unwrap_or(0);
            let row = serde_json::json!({
                "trial": trial,
                "rep": rep,
                "model": model,
                "provider": provider,
                "cut": cut.name(),
                "words": word.name(),
                "mode": mode.name(),
                "outcome": outcome.short(),
                "detail": outcome.label(),
                "structure_ok": scored.structure_ok,
                "leaves": scored.leaves,
                "leaf_overreach": scored.leaf_overreach,
                "regions": scored.regions,
                "region_overreach": scored.region_overreach,
                "correct": scored.correct,
                "cost": ending.cost,
                "cached_in": cached,
                "uncached_in": uncached,
                "millis": millis,
            });
            println!(
                "trial {trial}/{total_trials}  {model}@{provider} {}/{}/{}  rep {rep}  {}  structure {}  leaf over-reach {}/{}  region over-reach {}/{}  totals {}  ${:.4}  {:.1}s",
                cut.name(),
                word.name(),
                mode.name(),
                outcome.short(),
                if scored.structure_ok { "ok" } else { "WRONG" },
                scored.leaf_overreach,
                scored.leaves,
                scored.region_overreach,
                scored.regions,
                if scored.correct { "ok" } else { "WRONG" },
                ending.cost,
                millis as f64 / 1000.0,
            );
            rows.push(row);
            if let Outcome::Faulted(reason) = &outcome {
                println!("benchmark stopping: {reason}");
                stopped = true;
                break;
            }
        }
        if stopped {
            break;
        }
    }

    let table = summarise(&rows);
    std::fs::write(bench_dir.join("summary.md"), &table)
        .map_err(|e| format!("write summary.md: {e}"))?;
    std::fs::write(
        bench_dir.join("trials.json"),
        serde_json::to_string_pretty(&rows).unwrap(),
    )
    .map_err(|e| format!("write trials.json: {e}"))?;
    print!("\n{table}");
    if stopped {
        println!("stopped early after ${spent:.4} of ${budget:.2}");
    }
    Ok(ExitCode::SUCCESS)
}

fn summarise(rows: &[serde_json::Value]) -> String {
    let mut text = String::from(
        "| combo | trials | leaf over-reach | region over-reach | structure ok | correct | mean cost | cache read share | mean wall |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    let mut combos: Vec<String> = Vec::new();
    for row in rows {
        let combo = combo_of(row);
        if !combos.contains(&combo) {
            combos.push(combo);
        }
    }
    for combo in combos {
        let group: Vec<&serde_json::Value> =
            rows.iter().filter(|row| combo_of(row) == combo).collect();
        let n = group.len() as f64;
        let number = |row: &serde_json::Value, key: &str| row[key].as_f64().unwrap_or(0.0);
        let leaves: f64 = group.iter().map(|r| number(r, "leaves")).sum();
        let leaf_over: f64 = group.iter().map(|r| number(r, "leaf_overreach")).sum();
        let regions: f64 = group.iter().map(|r| number(r, "regions")).sum();
        let region_over: f64 = group.iter().map(|r| number(r, "region_overreach")).sum();
        let structure = group
            .iter()
            .filter(|r| r["structure_ok"].as_bool().unwrap_or(false))
            .count() as f64;
        let correct = group
            .iter()
            .filter(|r| r["correct"].as_bool().unwrap_or(false))
            .count() as f64;
        let cost: f64 = group.iter().map(|r| number(r, "cost")).sum();
        let cached: f64 = group.iter().map(|r| number(r, "cached_in")).sum();
        let uncached: f64 = group.iter().map(|r| number(r, "uncached_in")).sum();
        let millis: f64 = group.iter().map(|r| number(r, "millis")).sum();
        let share = if cached + uncached > 0.0 {
            cached / (cached + uncached) * 100.0
        } else {
            0.0
        };
        text.push_str(&format!(
            "| {combo} | {} | {}/{} | {}/{} | {}/{} | {}/{} | ${:.4} | {:.1}% | {:.1}s |\n",
            group.len(),
            leaf_over as u64,
            leaves as u64,
            region_over as u64,
            regions as u64,
            structure as u64,
            group.len(),
            correct as u64,
            group.len(),
            cost / n,
            share,
            millis / n / 1000.0,
        ));
    }
    text
}

fn combo_of(row: &serde_json::Value) -> String {
    format!(
        "{}@{} {}/{}/{}",
        row["model"].as_str().unwrap_or(""),
        row["provider"].as_str().unwrap_or(""),
        row["cut"].as_str().unwrap_or(""),
        row["words"].as_str().unwrap_or(""),
        row["mode"].as_str().unwrap_or(""),
    )
}
