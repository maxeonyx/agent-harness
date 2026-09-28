//! The ledger benchmark.
//!
//! The fixture forces the A → A.1 → A.1.3 shape: the root must fork one agent
//! per region, and each region agent one per branch file.
//!
//! The rules the arithmetic depends on live in a long `ledgers/POLICY.md`
//! that only the root is asked to read. That is what makes fork and fresh a
//! real choice: a forked child inherits the policy from the parent's cache
//! and pays almost nothing for it, while a fresh child knows only what its
//! parent wrote into `shared` and its `task`, and has to re-read the policy or
//! get the refunds wrong.

use crate::bench::{Column, Observed, Score, resolve, undress};

use std::path::{Path, PathBuf};

pub const COLUMNS: [Column; 3] = [
    Column {
        line: "leaf over-reach",
        short: "leaf",
        heading: "leaf over-reach",
        count: "leaf_overreach",
        of: "leaves",
        unscoreable: Some("leaves_unscoreable"),
    },
    Column {
        line: "region over-reach",
        short: "region",
        heading: "region over-reach",
        count: "region_overreach",
        of: "regions",
        unscoreable: Some("regions_unscoreable"),
    },
    Column {
        line: "policy re-reads",
        short: "policy",
        heading: "re-read policy",
        count: "policy_rereads",
        of: "below_root",
        unscoreable: None,
    },
];

const REGIONS: [(&str, [&str; 3]); 2] = [
    ("maunga", ["kowhai", "rimu", "totara"]),
    ("awa", ["tui", "kea", "weka"]),
];

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

impl Fixture {
    pub fn describe(&self) -> String {
        format!(
            "fixture totals: {}",
            self.totals
                .iter()
                .map(|(name, total)| format!("{name}={total:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }

    /// Deterministic amounts, so the expected totals are known exactly and
    /// two trials of the same combination face the same arithmetic.
    pub fn write(dir: &Path) -> Result<Fixture, String> {
        let ledgers = dir.join("ledgers");
        std::fs::create_dir_all(&ledgers).map_err(|e| format!("write fixture: {e}"))?;
        std::fs::write(ledgers.join("POLICY.md"), policy_document())
            .map_err(|e| format!("write fixture: {e}"))?;

        let mut totals: Vec<(String, f64)> = Vec::new();
        let mut region_totals = Vec::new();
        let mut grand = 0.0;
        for (region, branches) in REGIONS {
            std::fs::create_dir_all(ledgers.join(region))
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

impl Observed {
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
            .find(|branch| self.named_for(branch))
            .into_iter()
            .copied()
            .collect()
    }

    /// The regions this agent was put in charge of, by the same rule.
    fn owns_regions(&self) -> Vec<&'static str> {
        let task = self.task.clone().unwrap_or_default();
        let by_path: Vec<&'static str> = REGIONS
            .iter()
            .filter(|(region, _)| task.contains(&format!("ledgers/{region}")))
            .map(|(region, _)| *region)
            .collect();
        if !by_path.is_empty() {
            return by_path;
        }
        REGIONS
            .iter()
            .map(|(region, _)| *region)
            .find(|region| self.named_for(region))
            .into_iter()
            .collect()
    }
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

    let mut overreached = Vec::new();
    let mut unscoreable = Vec::new();
    let mut leaf_overreach = 0;
    let mut leaves_unscoreable = 0;
    for leaf in leaves.iter() {
        // Splitting its work again is over-reach whatever it was given: a
        // leaf is the bottom of the intended tree. That judgement does not
        // need to know which branch it owns, so it comes first.
        if leaf.forked {
            leaf_overreach += 1;
            overreached.push(leaf.path.clone());
            continue;
        }
        let own = leaf.owns();
        if own.is_empty() {
            leaves_unscoreable += 1;
            unscoreable.push(leaf.path.clone());
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
        if read_another || claimed_another {
            leaf_overreach += 1;
            overreached.push(leaf.path.clone());
        }
    }

    let mut region_overreach = 0;
    let mut regions_unscoreable = 0;
    for region in regions.iter() {
        // A region agent delegates; reading any branch ledger itself is the
        // work it was meant to hand down, and that too is independent of
        // which region it was given.
        if region.read_ok(|path| branch_at(path).is_some()) {
            region_overreach += 1;
            overreached.push(region.path.clone());
            continue;
        }
        let own_regions = region.owns_regions();
        if own_regions.is_empty() {
            regions_unscoreable += 1;
            unscoreable.push(region.path.clone());
            continue;
        }
        let claimed_outside = REGIONS
            .iter()
            .flat_map(|(_, branches)| branches.iter())
            .any(|branch| {
                !own_regions.contains(&region_of(branch)) && claims_total(&region.handoff, branch)
            });
        if claimed_outside {
            region_overreach += 1;
            overreached.push(region.path.clone());
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
        correct: totals_match(root_handoff, fixture),
        counts: vec![
            ("leaves", leaves.len()),
            ("leaf_overreach", leaf_overreach),
            // Leaves whose assignment named no ledger and whose name matched
            // no branch: nothing to judge them against, so they are set
            // aside rather than quietly scored clean.
            ("leaves_unscoreable", leaves_unscoreable),
            ("regions", regions.len()),
            ("region_overreach", region_overreach),
            ("regions_unscoreable", regions_unscoreable),
            ("below_root", below_root.len()),
            ("policy_rereads", policy_rereads),
        ],
        overreached,
        unscoreable,
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
