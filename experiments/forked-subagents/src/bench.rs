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

pub struct Fixture {
    pub dir: PathBuf,
    /// Branch, region and `grand` totals, by name.
    pub totals: Vec<(String, f64)>,
}

impl Fixture {
    /// The expected totals of a fixture already on disk, worked out by
    /// applying the rules `POLICY.md` states. A benchmark is rescored against
    /// the ledgers its trials actually faced, never against today's
    /// generator.
    pub fn read(dir: &Path) -> Result<Fixture, String> {
        let ledgers = dir.join("ledgers");
        if !ledgers.join("POLICY.md").is_file() {
            return Err(format!(
                "{} has no ledgers/POLICY.md; this benchmark predates the policy fixture and cannot be rescored against it",
                dir.display()
            ));
        }
        let mut totals: Vec<(String, f64)> = Vec::new();
        let mut region_totals = Vec::new();
        let mut grand = 0.0;
        for (region, branches) in REGIONS {
            let mut region_total = 0.0;
            for branch in branches {
                let path = ledgers.join(region).join(format!("{branch}.txt"));
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("read {}: {e}", path.display()))?;
                let mut total = 0.0;
                for line in text.lines() {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    if words.len() < 2 || words[0].starts_with('#') || words[1] == "VOID" {
                        continue;
                    }
                    if words.last() == Some(&"DUP") {
                        continue;
                    }
                    let Ok(amount) = words[1].parse::<f64>() else {
                        continue;
                    };
                    if words.last() == Some(&"REFUND") {
                        total -= amount;
                    } else {
                        total += amount;
                    }
                }
                let total = (total * 100.0).round() / 100.0;
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

/// The policy document, as the limb resolves it. Case matters: `policy.md`
/// is a different name, and reading it fails.
const POLICY_PATH: &str = "ledgers/POLICY.md";

/// A path as the limb would resolve it, relative to the run directory.
fn resolve(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// Which branch ledger a path names, if any.
fn branch_at(path: &str) -> Option<&'static str> {
    let resolved = resolve(path);
    REGIONS.iter().find_map(|(region, branches)| {
        branches
            .iter()
            .find(|branch| resolved == format!("ledgers/{region}/{branch}.txt"))
            .copied()
    })
}

fn region_of(branch: &str) -> &'static str {
    REGIONS
        .iter()
        .find(|(_, branches)| branches.contains(&branch))
        .map(|(region, _)| *region)
        .expect("branch belongs to a region")
}

/// A line of a report that states a total for `branch` — `kowhai: 13923.79`,
/// with or without markdown dressing. Merely naming a sibling is not a claim:
/// "I did not read rimu, as instructed" is perfect discipline, and the
/// `explained` framing hands every child its siblings' names, so counting
/// mentions would have made the treatment raise the false-positive rate of
/// the metric under test.
fn claims_total(text: &str, branch: &str) -> bool {
    text.lines().any(|line| {
        let line = undress(line);
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case(branch) && parse_amount(value).is_some()
    })
}

/// Strip the markdown a model reaches for: emphasis, code ticks, list
/// bullets, trailing punctuation.
fn undress(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', '+', '#', '>'])
        .trim()
        .replace(['*', '`', '_'], "")
        .trim()
        .to_string()
}

fn parse_amount(value: &str) -> Option<f64> {
    let cleaned: String = value
        .trim()
        .trim_start_matches('$')
        .chars()
        .filter(|c| !matches!(c, ',' | ' '))
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ';']);
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// One attempted read, and whether the limb answered it.
#[derive(Clone, Debug)]
pub struct ReadAttempt {
    pub path: String,
    pub ok: bool,
}

/// What one agent was observed to do. The scorer sees only this, so a live
/// trial and a rescored one are judged by exactly the same code.
#[derive(Clone, Debug)]
pub struct Observed {
    pub path: String,
    pub depth: usize,
    pub children: usize,
    /// The `task` text the parent wrote for this agent.
    pub task: Option<String>,
    pub handoff: String,
    pub reads: Vec<ReadAttempt>,
    /// Whether this agent called `task` itself.
    pub forked: bool,
    pub cached_in: u64,
    pub uncached_in: u64,
    pub written_in: u64,
    pub first_cached_in: u64,
    pub first_uncached_in: u64,
}

impl Observed {
    fn name(&self) -> &str {
        self.path.rsplit(" › ").next().unwrap_or(&self.path)
    }

    fn read_ok(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.reads
            .iter()
            .any(|read| read.ok && predicate(&read.path))
    }

    fn re_read_policy(&self) -> bool {
        self.read_ok(|path| resolve(path) == POLICY_PATH)
    }

    /// The branches this agent was put in charge of. Taken from the ledger
    /// paths its assignment names, because a bare branch name in the prose —
    /// "rimu and totara are handled by others, do not touch them" — would
    /// otherwise hand the parent control of the scorer. Falls back to the
    /// agent's own name when the assignment names no path at all.
    fn owns(&self) -> Vec<&'static str> {
        let task = self.task.clone().unwrap_or_default();
        let by_path: Vec<&'static str> = REGIONS
            .iter()
            .flat_map(|(region, branches)| branches.iter().map(move |branch| (region, branch)))
            .filter(|(region, branch)| task.contains(&format!("ledgers/{region}/{branch}.txt")))
            .map(|(_, branch)| *branch)
            .collect();
        if !by_path.is_empty() {
            return by_path;
        }
        REGIONS
            .iter()
            .flat_map(|(_, branches)| branches.iter())
            .find(|branch| self.name().eq_ignore_ascii_case(branch))
            .into_iter()
            .copied()
            .collect()
    }

    /// The regions this agent was put in charge of, by the same rule.
    fn owns_regions(&self) -> Vec<&'static str> {
        let task = self.task.clone().unwrap_or_default();
        let by_path: Vec<&'static str> = REGIONS
            .iter()
            .filter(|(region, _)| task.contains(&format!("ledgers/{region}/")))
            .map(|(region, _)| *region)
            .collect();
        if !by_path.is_empty() {
            return by_path;
        }
        REGIONS
            .iter()
            .map(|(region, _)| *region)
            .find(|region| self.name().eq_ignore_ascii_case(region))
            .into_iter()
            .collect()
    }
}

impl Observed {
    /// A live run's records, seen the way a rescored trial is seen.
    pub fn from_records(records: &[AgentRecord]) -> Vec<Observed> {
        records
            .iter()
            .map(|record| Observed {
                path: record.path.clone(),
                depth: record.depth,
                children: record.children.len(),
                task: record.task.clone(),
                handoff: record.handoff.clone(),
                reads: record
                    .tool_calls
                    .iter()
                    .filter(|call| call.name == "read_file")
                    .map(|call| ReadAttempt {
                        path: serde_json::from_str::<serde_json::Value>(&call.arguments)
                            .ok()
                            .and_then(|a| a.get("path")?.as_str().map(str::to_string))
                            .unwrap_or_default(),
                        ok: call
                            .result
                            .as_ref()
                            .is_some_and(|text| !text.starts_with("Error:")),
                    })
                    .collect(),
                forked: record.tool_calls.iter().any(|call| call.name == "task"),
                cached_in: record.cached_in,
                uncached_in: record.uncached_in,
                written_in: record.written_in,
                first_cached_in: record.first_cached_in,
                first_uncached_in: record.first_uncached_in,
            })
            .collect()
    }
}

/// Everything about a trial that is not scoring: who ran it and how it
/// ended. Rescoring keeps these and replaces the rest.
pub struct TrialFacts {
    pub trial: u64,
    pub rep: u64,
    pub model: String,
    pub provider: String,
    pub cut: String,
    pub words: String,
    pub mode: String,
    pub outcome: String,
    pub detail: String,
    pub cost: f64,
    pub millis: u128,
}

/// One trial's record. Built identically by `bench` and by `rescore`, so a
/// rescored grid is directly comparable with a freshly run one.
pub fn trial_row(facts: &TrialFacts, agents: &[Observed], scored: &Score) -> serde_json::Value {
    let children: Vec<&Observed> = agents.iter().filter(|a| a.depth >= 1).collect();
    let sum = |pick: fn(&Observed) -> u64| agents.iter().map(pick).sum::<u64>();
    let child_sum = |pick: fn(&Observed) -> u64| children.iter().copied().map(pick).sum::<u64>();
    serde_json::json!({
        "trial": facts.trial,
        "rep": facts.rep,
        "model": facts.model,
        "provider": facts.provider,
        "cut": facts.cut,
        "words": facts.words,
        "mode": facts.mode,
        "outcome": facts.outcome,
        "detail": facts.detail,
        "structure_ok": scored.structure_ok,
        "leaves": scored.leaves,
        "leaf_overreach": scored.leaf_overreach,
        "leaves_unscoreable": scored.leaves_unscoreable,
        "regions": scored.regions,
        "region_overreach": scored.region_overreach,
        "below_root": scored.below_root,
        "policy_rereads": scored.policy_rereads,
        "correct": scored.correct,
        "cost": facts.cost,
        "millis": facts.millis,
        // Cache: over everything, over the children alone, and over each
        // child's first request — the last being the direct answer to "did
        // the fork inherit the parent's prefix".
        "cached_in": sum(|a| a.cached_in),
        "uncached_in": sum(|a| a.uncached_in),
        "written_in": sum(|a| a.written_in),
        "child_cached_in": child_sum(|a| a.cached_in),
        "child_uncached_in": child_sum(|a| a.uncached_in),
        "child_written_in": child_sum(|a| a.written_in),
        "child_first_cached_in": child_sum(|a| a.first_cached_in),
        "child_first_uncached_in": child_sum(|a| a.first_uncached_in),
    })
}

fn share(cached: f64, uncached: f64) -> f64 {
    if cached + uncached > 0.0 {
        cached / (cached + uncached) * 100.0
    } else {
        0.0
    }
}

pub fn trial_line(row: &serde_json::Value, of: usize) -> String {
    let n = |key: &str| row[key].as_f64().unwrap_or(0.0);
    let flag = |key: &str| {
        if row[key].as_bool().unwrap_or(false) {
            "ok"
        } else {
            "WRONG"
        }
    };
    let unscoreable = n("leaves_unscoreable") as u64;
    format!(
        "trial {}/{of}  {}@{} {}/{}/{}  rep {}  {}  structure {}  leaf over-reach {}/{}{}  region over-reach {}/{}  policy re-reads {}/{}  totals {}  child cache {:.0}% (first {:.0}%)  ${:.4}  {:.1}s",
        row["trial"].as_u64().unwrap_or(0),
        row["model"].as_str().unwrap_or(""),
        row["provider"].as_str().unwrap_or(""),
        row["cut"].as_str().unwrap_or(""),
        row["words"].as_str().unwrap_or(""),
        row["mode"].as_str().unwrap_or(""),
        row["rep"].as_u64().unwrap_or(0),
        row["outcome"].as_str().unwrap_or(""),
        flag("structure_ok"),
        n("leaf_overreach") as u64,
        n("leaves") as u64,
        if unscoreable > 0 {
            format!(" ({unscoreable} unscoreable)")
        } else {
            String::new()
        },
        n("region_overreach") as u64,
        n("regions") as u64,
        n("policy_rereads") as u64,
        n("below_root") as u64,
        flag("correct"),
        share(n("child_cached_in"), n("child_uncached_in")),
        share(n("child_first_cached_in"), n("child_first_uncached_in")),
        n("cost"),
        n("millis") / 1000.0,
    )
}

pub struct Score {
    pub structure_ok: bool,
    pub leaves: usize,
    pub leaf_overreach: usize,
    /// Leaves whose assignment named no ledger and whose name matched no
    /// branch: nothing to judge them against, so they are set aside rather
    /// than quietly scored clean.
    pub leaves_unscoreable: usize,
    pub regions: usize,
    pub region_overreach: usize,
    pub below_root: usize,
    pub policy_rereads: usize,
    pub correct: bool,
}

/// Scored against what the agents actually did — successful reads, and totals
/// actually claimed — rather than against strings a model chose to write.
pub fn score(agents: &[Observed], root_handoff: &str, fixture: &Fixture) -> Score {
    let at = |depth: usize| {
        agents
            .iter()
            .filter(|a| a.depth == depth)
            .collect::<Vec<_>>()
    };
    let regions = at(1);
    let leaves = at(2);
    let deepest = agents.iter().map(|a| a.depth).max().unwrap_or(0);
    let structure_ok = regions.len() == 2
        && leaves.len() == 6
        && deepest <= 2
        && regions.iter().all(|a| a.children == 3);

    let mut leaf_overreach = 0;
    let mut leaves_unscoreable = 0;
    for leaf in leaves.iter() {
        let own = leaf.owns();
        if own.is_empty() {
            leaves_unscoreable += 1;
            continue;
        }
        let read_another = leaf.read_ok(|path| match branch_at(path) {
            Some(branch) => !own.contains(&branch),
            None => false,
        });
        let claimed_another = REGIONS
            .iter()
            .flat_map(|(_, branches)| branches.iter())
            .any(|branch| !own.contains(branch) && claims_total(&leaf.handoff, branch));
        if read_another || leaf.forked || claimed_another {
            leaf_overreach += 1;
        }
    }

    let mut region_overreach = 0;
    for region in regions.iter() {
        let own_regions = region.owns_regions();
        // A region agent delegates; reading any branch ledger itself is the
        // work it was meant to hand down.
        let read_a_ledger = region.read_ok(|path| branch_at(path).is_some());
        let claimed_outside = REGIONS
            .iter()
            .flat_map(|(_, branches)| branches.iter())
            .any(|branch| {
                !own_regions.contains(&region_of(branch)) && claims_total(&region.handoff, branch)
            });
        if read_a_ledger || claimed_outside {
            region_overreach += 1;
        }
    }

    // Re-reading the policy is not over-reach — for a fresh child it is the
    // only way to learn the rules. It is what fresh pays instead.
    let below_root: Vec<&&Observed> = regions.iter().chain(leaves.iter()).collect();
    let policy_rereads = below_root
        .iter()
        .filter(|agent| agent.re_read_policy())
        .count();

    Score {
        structure_ok,
        leaves: leaves.len(),
        leaf_overreach,
        leaves_unscoreable,
        regions: regions.len(),
        region_overreach,
        below_root: below_root.len(),
        policy_rereads,
        correct: totals_match(root_handoff, fixture),
    }
}

/// The root's final message must end with one `name: total` line per branch,
/// region and `grand`. Blank lines and markdown dressing are tolerated; a
/// name given the wrong value anywhere in the block is not.
pub fn totals_match(handoff: &str, fixture: &Fixture) -> bool {
    let mut reported: Vec<(String, f64)> = Vec::new();
    for line in handoff.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let line = undress(line);
        let Some((name, value)) = line.split_once(':') else {
            break;
        };
        let Some(value) = parse_amount(value) else {
            break;
        };
        reported.push((name.trim().to_lowercase(), value));
    }
    fixture.totals.iter().all(|(name, expected)| {
        let claims: Vec<&(String, f64)> = reported.iter().filter(|(got, _)| got == name).collect();
        !claims.is_empty()
            && claims
                .iter()
                .all(|(_, value)| (value - expected).abs() < 0.005)
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
            let observed = Observed::from_records(&ending.agents);
            let scored = score(&observed, &ending.handoff, &fixture);
            spent += ending.cost;
            let facts = TrialFacts {
                trial: trial as u64,
                rep: rep as u64,
                model: model.clone(),
                provider: provider.clone(),
                cut: framing.cut.name().to_string(),
                words: framing.words.name().to_string(),
                mode: framing.mode.name().to_string(),
                outcome: outcome.short().to_string(),
                detail: outcome.label(),
                cost: ending.cost,
                millis: ending.agents.iter().map(|a| a.millis).max().unwrap_or(0),
            };
            let row = trial_row(&facts, &observed, &scored);
            println!("{}", trial_line(&row, total_trials));
            rows.push(row);
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

pub fn summarise(rows: &[serde_json::Value]) -> String {
    let mut text = String::from(
        "| combo | trials | leaf over-reach | region over-reach | re-read policy | structure ok | correct | mean cost | child cache read | child first-request cache | cache written | all-agent cache read | mean wall |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
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
        let sum = |key: &str| group.iter().map(|r| number(r, key)).sum::<f64>();
        let counted = |key: &str| {
            group
                .iter()
                .filter(|r| r[key].as_bool().unwrap_or(false))
                .count()
        };
        let unscoreable = sum("leaves_unscoreable") as u64;
        text.push_str(&format!(
            "| {combo} | {} | {}/{}{} | {}/{} | {}/{} | {}/{} | {}/{} | ${:.4} | {:.1}% | {:.1}% | {} | {:.1}% | {:.1}s |\n",
            group.len(),
            sum("leaf_overreach") as u64,
            sum("leaves") as u64,
            if unscoreable > 0 {
                format!(" ({unscoreable} unscoreable)")
            } else {
                String::new()
            },
            sum("region_overreach") as u64,
            sum("regions") as u64,
            sum("policy_rereads") as u64,
            sum("below_root") as u64,
            counted("structure_ok"),
            group.len(),
            counted("correct"),
            group.len(),
            sum("cost") / n,
            share(sum("child_cached_in"), sum("child_uncached_in")),
            share(sum("child_first_cached_in"), sum("child_first_uncached_in")),
            sum("written_in") as u64,
            share(sum("cached_in"), sum("uncached_in")),
            sum("millis") / n / 1000.0,
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
