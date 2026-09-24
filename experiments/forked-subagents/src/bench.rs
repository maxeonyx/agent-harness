//! The discipline benchmark.
//!
//! The fixture forces the A → A.1 → A.1.3 shape: the root must fork one agent
//! per region, and each region agent one per branch file. Scoring is
//! mechanical — read off the recorded tool calls and reports, never judged by
//! a model — so a framing either keeps a leaf inside its own assignment or it
//! does not.
//!
//! The rules the arithmetic depends on live in a long `ledgers/POLICY.md`
//! that only the root is asked to read. That is what makes fork and fresh a
//! real choice: a forked child inherits the policy from the parent's cache
//! and pays almost nothing for it, while a fresh child knows only what its
//! parent wrote into its assignment, and has to re-read the policy or get the
//! refunds wrong.

use crate::Args;
use crate::agent::{AgentRecord, Mode};
use crate::framing::{Cut, Framing, Words};
use crate::session::Session;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const REGIONS: [(&str, [&str; 3]); 2] = [
    ("maunga", ["kowhai", "rimu", "totara"]),
    ("awa", ["tui", "kea", "weka"]),
];

/// Root → region → leaf, and nothing below. A trial that tries to go deeper
/// is told no rather than spending the budget on a tree nobody asked for.
const BENCH_MAX_DEPTH: usize = 2;
const BENCH_MAX_COST: f64 = 0.15;

pub const ROOT_TASK: &str = "\
The run directory holds `ledgers/POLICY.md`, and under `ledgers/<region>/` one plain-text ledger per branch — 2 regions, 3 branches each.

Read `ledgers/POLICY.md` first. You are the only agent that should read it.

Then work out the total of each branch ledger, the total for each region, and the grand total. Do not read a branch ledger yourself. Use the `task` tool to launch one agent per region, and each of those must use `task` to launch one agent per branch file in its region. Each level reports its numbers to the level above.

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

/// The section of the policy the arithmetic actually turns on. Everything
/// else in the document is plausible and irrelevant.
const POLICY_RULES: &str = "\
## 7. Reading a branch ledger file

Each branch keeps one plain-text ledger at `ledgers/<region>/<branch>.txt`. Every line in that file is one of five kinds, and the branch total is computed from them as follows.

- A line beginning with `#` is a comment. It carries no amount and is ignored.
- `<date>  <amount>` is an ordinary entry. Its amount is **added** to the branch total.
- `<date>  <amount>  REFUND` is a refund issued to the customer. Its amount is **subtracted** from the branch total.
- `<date>  <amount>  DUP` is a duplicate of the entry on the line immediately above, recorded twice by the till software. It is **skipped entirely** and contributes nothing.
- `<date>  VOID` is a cancelled entry. It carries no amount and contributes nothing.

The branch total is therefore the sum of the ordinary entries minus the sum of the refunds. Round to two decimal places once, at the end.
";

struct Fixture {
    dir: PathBuf,
    /// Branch, region and `grand` totals, by name.
    totals: Vec<(String, f64)>,
}

fn is_policy(path: &str) -> bool {
    path.contains("POLICY")
}

/// Deterministic amounts, so the expected totals are known exactly and two
/// trials of the same combination face the same arithmetic.
fn write_fixture(dir: &Path) -> Result<Fixture, String> {
    let ledgers = dir.join("ledgers");
    std::fs::create_dir_all(&ledgers).map_err(|e| format!("write fixture: {e}"))?;
    std::fs::write(ledgers.join("POLICY.md"), policy_document())
        .map_err(|e| format!("write fixture: {e}"))?;

    let mut totals: Vec<(String, f64)> = Vec::new();
    let mut region_totals = Vec::new();
    let mut grand = 0.0;
    for (region, branches) in REGIONS {
        std::fs::create_dir_all(ledgers.join(region)).map_err(|e| format!("write fixture: {e}"))?;
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
                let amount = ((seed >> 33) % 90_000 + 100) as f64 / 100.0;
                let date = format!("2026-{:02}-{:02}", 1 + day / 28, 1 + day % 28);
                if day == 23 {
                    lines.push("# --- page break, audited by A. Ngata ---".to_string());
                }
                match day {
                    11 => lines.push(format!("{date}  VOID")),
                    7 | 19 | 31 => {
                        lines.push(format!("{date}  {amount:.2}  REFUND"));
                        total -= amount;
                    }
                    _ => {
                        lines.push(format!("{date}  {amount:.2}"));
                        total += amount;
                        if day == 15 || day == 27 {
                            lines.push(format!("{date}  {amount:.2}  DUP"));
                        }
                    }
                }
            }
            let total = (total * 100.0).round() / 100.0;
            std::fs::write(
                ledgers.join(region).join(format!("{branch}.txt")),
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

/// A plausible accounts manual: one section that decides the arithmetic,
/// buried in several thousand tokens of the kind of boilerplate such a
/// document really carries.
fn policy_document() -> String {
    const TOPICS: [&str; 30] = [
        "Segregation of duties",
        "Retention of source documents",
        "Petty cash floats",
        "Foreign currency settlement",
        "Inter-branch transfers",
        "Stocktake adjustments",
        "Reading a branch ledger file",
        "Bank reconciliation",
        "Suspense accounts",
        "Write-offs and provisions",
        "Till shortages and overs",
        "Approval thresholds",
        "Month-end close",
        "Audit trail and access logging",
        "Staff purchases",
        "Gift card liabilities",
        "Supplier rebates",
        "Fixed asset registers",
        "Disaster recovery of ledger data",
        "Credit notes and returns",
        "Cash banking and escorts",
        "Layby and deposit accounts",
        "Freight and landed cost",
        "Shrinkage investigations",
        "Charitable donations",
        "Payroll cost allocation",
        "Lease and occupancy charges",
        "Intercompany eliminations",
        "Chart of accounts changes",
        "Delegation during absence",
    ];
    const CLAUSES: [&str; 20] = [
        "Responsibility for this rests with the branch accountant, who may delegate it in writing to a nominated deputy for periods of no more than four weeks.",
        "Supporting documentation is retained for seven financial years and produced within two working days of a request from Group Finance.",
        "Any departure from this clause requires the prior written approval of the Regional Controller, recorded in the exceptions register.",
        "The relevant control is tested quarterly by Internal Audit, and the result reported to the Audit and Risk Committee.",
        "Where a system limitation prevents compliance, the limitation is logged as a known deficiency and reviewed at each half-year close.",
        "Amounts are recorded in New Zealand dollars, and any conversion uses the rate published on the last business day of the period.",
        "Branch managers are reminded that this clause applies equally to seasonal and to permanent staff.",
        "Nothing in this section alters the obligations imposed by the Group Delegations of Authority.",
        "A summary of activity under this section is included in the monthly branch pack circulated to the regional office.",
        "Discrepancies are escalated on the day they are identified, and are not held over to the following period.",
        "Training on this section forms part of the induction programme for all finance staff.",
        "The controls described here operate independently of the point-of-sale system's own validation.",
        "Records created under this section are classified as commercially sensitive and stored accordingly.",
        "Two signatures are required where the amount exceeds the branch threshold set out in Appendix C.",
        "The regional office may vary the frequency of this check on thirty days' notice to affected branches.",
        "Electronic copies satisfy the retention requirement provided the originals are destroyed under the approved schedule.",
        "This section does not apply to branches operating under the transitional arrangements listed in Appendix F.",
        "Queries about the interpretation of this section are directed to Group Finance rather than resolved locally.",
        "An exception raised under this section lapses at the end of the financial year unless it is renewed.",
        "The branch retains a register of every adjustment made under this section, reconciled monthly.",
    ];

    let mut text = String::from(
        "# Ledger and Reconciliation Policy — Consolidated Accounts Manual\n\nRevision 14.3. Supersedes all previous revisions. Applies to every branch, region and consolidated entity.\n\n## 1. Purpose and scope\n\nThis manual sets out how branch ledgers are kept, read, reconciled and consolidated. It is binding on all branch and regional finance staff. Where it conflicts with a local instruction, this manual prevails.\n\n",
    );
    let mut seed: u64 = 8_675_309;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    let mut previous = usize::MAX;
    for section in 2..=30 {
        if section == 7 {
            text.push_str(POLICY_RULES);
            text.push('\n');
            continue;
        }
        text.push_str(&format!("## {section}. {}\n\n", TOPICS[section - 1]));
        for _ in 0..2 {
            let mut paragraph = Vec::new();
            while paragraph.len() < 4 {
                let pick = next() % CLAUSES.len();
                if pick == previous {
                    continue;
                }
                previous = pick;
                paragraph.push(CLAUSES[pick]);
            }
            text.push_str(&paragraph.join(" "));
            text.push_str("\n\n");
        }
    }
    text
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
    below_root: usize,
    policy_rereads: usize,
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
        let read_foreign = leaf
            .reads()
            .iter()
            .any(|path| !is_policy(path) && foreign(path));
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
        let read_a_ledger = region_agent.reads().iter().any(|path| !is_policy(path));
        let touched_other = all_branches
            .iter()
            .any(|branch| !own.contains(branch) && mentions(&region_agent.handoff, branch));
        if read_a_ledger || touched_other {
            region_overreach += 1;
        }
    }

    // Re-reading the policy is not over-reach — for a fresh child it is the
    // only way to learn the rules. It is what fresh pays instead.
    let below_root: Vec<&&AgentRecord> = region_agents.iter().chain(leaf_agents.iter()).collect();
    let policy_rereads = below_root
        .iter()
        .filter(|agent| agent.reads().iter().any(|path| is_policy(path)))
        .count();

    Score {
        structure_ok,
        leaves: leaf_agents.len(),
        leaf_overreach,
        regions: region_agents.len(),
        region_overreach,
        below_root: below_root.len(),
        policy_rereads,
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
    args.known(&[crate::COMMON, &["grid", "reps", "budget"]].concat())?;
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
    let sample = args.config_with(
        "",
        "",
        Framing {
            cut: cuts[0],
            words: words[0],
            mode: modes[0],
        },
        BENCH_MAX_DEPTH,
        BENCH_MAX_COST,
    )?;
    println!("benchmark in {}", bench_dir.display());
    println!(
        "per-trial cap ${:.4} · per-trial depth limit {} · whole-benchmark budget ${budget:.4}",
        sample.max_cost, sample.max_depth
    );
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
                    combos.push((
                        model.to_string(),
                        provider.to_string(),
                        Framing {
                            cut,
                            words: word,
                            mode,
                        },
                    ));
                }
            }
        }
    }

    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut spent = 0.0;
    let mut stopped = false;
    let total_trials = combos.len() * reps;
    let mut trial = 0;
    'combos: for (model, provider, framing) in &combos {
        for rep in 1..=reps {
            trial += 1;
            if spent >= budget {
                stopped = true;
                break 'combos;
            }
            let label = format!(
                "trial{trial:03}-{}-{}-{}-rep{rep}",
                framing.cut.name(),
                framing.words.name(),
                framing.mode.name()
            );
            let config =
                args.config_with(model, provider, *framing, BENCH_MAX_DEPTH, BENCH_MAX_COST)?;
            let mut session = Session::open(
                config,
                &fixture.dir,
                &bench_dir,
                &label,
                false,
                args.session_id(),
            )?;
            session.say(ROOT_TASK);
            // A trial that faults — including one that reaches its own cap —
            // is a scored trial. Only the whole-benchmark budget stops the
            // benchmark.
            let outcome = session.turn().await;
            let ending = session.finish(&outcome);
            let scored = score(&ending.agents, &ending.handoff, &fixture);
            spent += ending.cost;
            let cached: u64 = ending.agents.iter().map(|a| a.cached_in).sum();
            let uncached: u64 = ending.agents.iter().map(|a| a.uncached_in).sum();
            let millis: u128 = ending.agents.iter().map(|a| a.millis).max().unwrap_or(0);
            rows.push(serde_json::json!({
                "trial": trial,
                "rep": rep,
                "model": model,
                "provider": provider,
                "cut": framing.cut.name(),
                "words": framing.words.name(),
                "mode": framing.mode.name(),
                "outcome": outcome.short(),
                "detail": outcome.label(),
                "structure_ok": scored.structure_ok,
                "leaves": scored.leaves,
                "leaf_overreach": scored.leaf_overreach,
                "regions": scored.regions,
                "region_overreach": scored.region_overreach,
                "below_root": scored.below_root,
                "policy_rereads": scored.policy_rereads,
                "correct": scored.correct,
                "cost": ending.cost,
                "cached_in": cached,
                "uncached_in": uncached,
                "millis": millis,
            }));
            println!(
                "trial {trial}/{total_trials}  {model}@{provider} {}/{}/{}  rep {rep}  {}  structure {}  leaf over-reach {}/{}  region over-reach {}/{}  policy re-reads {}/{}  totals {}  ${:.4}  {:.1}s",
                framing.cut.name(),
                framing.words.name(),
                framing.mode.name(),
                outcome.short(),
                if scored.structure_ok { "ok" } else { "WRONG" },
                scored.leaf_overreach,
                scored.leaves,
                scored.region_overreach,
                scored.regions,
                scored.policy_rereads,
                scored.below_root,
                if scored.correct { "ok" } else { "WRONG" },
                ending.cost,
                millis as f64 / 1000.0,
            );
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
        "| combo | trials | leaf over-reach | region over-reach | re-read policy | structure ok | correct | mean cost | cache read share | mean wall |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
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
        let sum = |key: &str| group.iter().map(|r| number(r, key)).sum::<f64>() as u64;
        let counted = |key: &str| {
            group
                .iter()
                .filter(|r| r[key].as_bool().unwrap_or(false))
                .count()
        };
        let cached: f64 = group.iter().map(|r| number(r, "cached_in")).sum();
        let uncached: f64 = group.iter().map(|r| number(r, "uncached_in")).sum();
        let share = if cached + uncached > 0.0 {
            cached / (cached + uncached) * 100.0
        } else {
            0.0
        };
        text.push_str(&format!(
            "| {combo} | {} | {}/{} | {}/{} | {}/{} | {}/{} | {}/{} | ${:.4} | {:.1}% | {:.1}s |\n",
            group.len(),
            sum("leaf_overreach"),
            sum("leaves"),
            sum("region_overreach"),
            sum("regions"),
            sum("policy_rereads"),
            sum("below_root"),
            counted("structure_ok"),
            group.len(),
            counted("correct"),
            group.len(),
            group.iter().map(|r| number(r, "cost")).sum::<f64>() / n,
            share,
            group.iter().map(|r| number(r, "millis")).sum::<f64>() / n / 1000.0,
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
