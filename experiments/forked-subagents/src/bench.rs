//! The discipline benchmarks.
//!
//! Each gives the root a fixed task over a generated fixture, and scores the
//! tree mechanically — read off the recorded tool calls and reports, never
//! judged by a model — so a framing either keeps an agent inside its own
//! assignment or it does not. `ledgers` hands the root the tree it must build;
//! `projects` asks for one agent per project and says nothing about depth.

use crate::Args;
use crate::agent::{AgentRecord, FaultKind, Mode};
use crate::framing::{Cut, Framing, Identity};
use crate::session::Session;
use crate::{ledgers, projects};

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

/// Two levels below the root: exactly the ledger tree, and one more than the
/// projects task asks for, so a project agent that splits again is seen
/// doing it. A trial that tries to go deeper is told no rather than spending
/// the budget on a tree nobody asked for.
const BENCH_MAX_DEPTH: usize = 2;
const BENCH_MAX_COST: f64 = 0.15;

/// A path as the limb would resolve it, relative to the run directory.
pub fn resolve(path: &str) -> String {
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

/// Strip the markdown a model reaches for: emphasis, code ticks, list
/// bullets, trailing punctuation.
pub fn undress(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', '+', '#', '>'])
        .trim()
        .replace(['*', '`', '_'], "")
        .trim()
        .to_string()
}

/// One attempted `read_file` or `list_dir`, and whether the limb answered it.
#[derive(Clone, Debug)]
pub struct ReadAttempt {
    pub path: String,
    pub ok: bool,
    pub list: bool,
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
    pub requests: usize,
    pub cached_in: u64,
    pub uncached_in: u64,
    pub written_in: u64,
    pub first_cached_in: u64,
    pub first_uncached_in: u64,
}

impl Observed {
    pub fn name(&self) -> &str {
        self.path.rsplit(" › ").next().unwrap_or(&self.path)
    }

    /// Whether this agent's name is built around `word`. Models name their
    /// children `maunga`, but also `maunga-region`, `maunga_region2` and
    /// `kowhai_branch`, and all of those mean the same thing.
    pub fn named_for(&self, word: &str) -> bool {
        self.name()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|part| part.eq_ignore_ascii_case(word))
    }

    /// A `read_file` the limb answered, of a path `predicate` accepts.
    pub fn read_ok(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.reads
            .iter()
            .any(|read| !read.list && read.ok && predicate(&read.path))
    }

    /// A `read_file` or `list_dir` the limb answered, of a path `predicate`
    /// accepts.
    pub fn accessed_ok(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.reads
            .iter()
            .any(|read| read.ok && predicate(&read.path))
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
                    .filter(|call| call.name == "read_file" || call.name == "list_dir")
                    .map(|call| ReadAttempt {
                        path: serde_json::from_str::<serde_json::Value>(&call.arguments)
                            .ok()
                            .and_then(|a| a.get("path")?.as_str().map(str::to_string))
                            .unwrap_or_default(),
                        ok: call
                            .result
                            .as_ref()
                            .is_some_and(|text| !text.starts_with("Error:")),
                        list: call.name == "list_dir",
                    })
                    .collect(),
                forked: record.tool_calls.iter().any(|call| call.name == "task"),
                requests: record.requests,
                cached_in: record.cached_in,
                uncached_in: record.uncached_in,
                written_in: record.written_in,
                first_cached_in: record.first_cached_in,
                first_uncached_in: record.first_uncached_in,
            })
            .collect()
    }
}

/// Whether a trial is evidence about the model at all. A run the provider
/// broke tells you nothing about how the model behaves, and folding it into
/// the rates as a row of zeroes quietly drags every number towards zero.
pub fn trial_is_valid(outcome: &str, fault_kind: Option<FaultKind>) -> bool {
    if outcome == "cancelled" {
        return false;
    }
    match fault_kind {
        None => true,
        Some(kind) => kind.is_the_models_doing(),
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
    pub identity: String,
    pub mode: String,
    pub outcome: String,
    pub detail: String,
    pub cost: f64,
    pub millis: u128,
    pub fault_kind: Option<FaultKind>,
}

/// How often the shared part was read from the cache: of the siblings after
/// the first in each scope, how many read it, and how many tokens they read
/// beyond what the first did. The first sibling writes the shared part. A
/// later one read it when its first request read more from the cache than
/// the sibling that read least.
struct SharedHits {
    later: usize,
    hits: usize,
    tokens: u64,
}

fn shared_hits(agents: &[Observed]) -> SharedHits {
    let mut scopes: BTreeMap<&str, Vec<&Observed>> = BTreeMap::new();
    for agent in agents.iter().filter(|a| a.depth >= 1 && a.requests > 0) {
        let (parent, _) = agent
            .path
            .rsplit_once(" › ")
            .expect("an agent below the root has a parent");
        scopes.entry(parent).or_default().push(agent);
    }
    let mut counted = SharedHits {
        later: 0,
        hits: 0,
        tokens: 0,
    };
    for siblings in scopes.values().filter(|siblings| siblings.len() >= 2) {
        let least = siblings.iter().map(|a| a.first_cached_in).min().unwrap();
        counted.later += siblings.len() - 1;
        for sibling in siblings.iter().filter(|a| a.first_cached_in > least) {
            counted.hits += 1;
            counted.tokens += sibling.first_cached_in - least;
        }
    }
    counted
}

/// One trial's record. Built identically by `bench` and by `rescore`, so a
/// rescored grid is directly comparable with a freshly run one.
pub fn trial_row(
    task: Task,
    facts: &TrialFacts,
    agents: &[Observed],
    scored: &Score,
) -> serde_json::Value {
    let children: Vec<&Observed> = agents.iter().filter(|a| a.depth >= 1).collect();
    let shared = shared_hits(agents);
    let sum = |pick: fn(&Observed) -> u64| agents.iter().map(pick).sum::<u64>();
    let child_sum = |pick: fn(&Observed) -> u64| children.iter().copied().map(pick).sum::<u64>();
    let mut row = serde_json::json!({
        "bench": task.name(),
        "trial": facts.trial,
        "rep": facts.rep,
        "model": facts.model,
        "provider": facts.provider,
        "cut": facts.cut,
        "identity": facts.identity,
        "mode": facts.mode,
        "outcome": facts.outcome,
        "detail": facts.detail,
        "fault_kind": facts.fault_kind.map(|kind| kind.name()),
        "valid": trial_is_valid(&facts.outcome, facts.fault_kind),
        "structure_ok": scored.structure_ok,
        "correct": scored.correct,
        "overreached": scored.overreached,
        "unscoreable": scored.unscoreable,
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
        "later_siblings": shared.later,
        "shared_hits": shared.hits,
        "shared_read": shared.tokens,
    });
    for (name, count) in &scored.counts {
        row[*name] = serde_json::json!(count);
    }
    row
}

fn share(cached: f64, uncached: f64) -> f64 {
    if cached + uncached > 0.0 {
        cached / (cached + uncached) * 100.0
    } else {
        0.0
    }
}

/// The benchmark a row was scored by. Rows from before there were two are
/// all ledger rows.
fn task_of(row: &serde_json::Value) -> Task {
    match row["bench"].as_str() {
        Some("projects") => Task::Projects,
        _ => Task::Ledgers,
    }
}

fn number(row: &serde_json::Value, key: &str) -> u64 {
    row[key].as_f64().unwrap_or(0.0) as u64
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
    if !row["valid"].as_bool().unwrap_or(true) {
        return format!(
            "trial {}/{of}  {}@{} {}/{}/{}  rep {}  INVALID ({} fault: {})  ${:.4}  {:.1}s",
            row["trial"].as_u64().unwrap_or(0),
            row["model"].as_str().unwrap_or(""),
            row["provider"].as_str().unwrap_or(""),
            row["cut"].as_str().unwrap_or(""),
            row["identity"].as_str().unwrap_or(""),
            row["mode"].as_str().unwrap_or(""),
            row["rep"].as_u64().unwrap_or(0),
            row["fault_kind"].as_str().unwrap_or("cancelled"),
            row["detail"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(70)
                .collect::<String>(),
            n("cost"),
            n("millis") / 1000.0,
        );
    }
    let task = task_of(row);
    let scores = task
        .columns()
        .iter()
        .map(|column| {
            let aside = match column.unscoreable.map(|key| number(row, key)) {
                Some(count) if count > 0 => format!(" ({count} unscoreable)"),
                _ => String::new(),
            };
            format!(
                "{} {}/{}{aside}",
                column.line,
                number(row, column.count),
                number(row, column.of)
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    format!(
        "trial {}/{of}  {}@{} {}/{}/{}  rep {}  {}  structure {}  {scores}  {} {}  child cache {:.0}% (first {:.0}%)  shared hits {}/{} ({} tokens)  ${:.4}  {:.1}s",
        row["trial"].as_u64().unwrap_or(0),
        row["model"].as_str().unwrap_or(""),
        row["provider"].as_str().unwrap_or(""),
        row["cut"].as_str().unwrap_or(""),
        row["identity"].as_str().unwrap_or(""),
        row["mode"].as_str().unwrap_or(""),
        row["rep"].as_u64().unwrap_or(0),
        row["outcome"].as_str().unwrap_or(""),
        flag("structure_ok"),
        task.answer(),
        flag("correct"),
        share(n("child_cached_in"), n("child_uncached_in")),
        share(n("child_first_cached_in"), n("child_first_uncached_in")),
        number(row, "shared_hits"),
        number(row, "later_siblings"),
        number(row, "shared_read"),
        n("cost"),
        n("millis") / 1000.0,
    )
}

/// The one-line verdict `rescore` compares, old against new.
pub fn verdict(row: &serde_json::Value) -> String {
    if !row["valid"].as_bool().unwrap_or(true) {
        return format!(
            "INVALID ({} fault) — excluded from every rate",
            row["fault_kind"].as_str().unwrap_or("cancelled")
        );
    }
    let flag = |key: &str| {
        if row[key].as_bool().unwrap_or(false) {
            "ok"
        } else {
            "WRONG"
        }
    };
    let task = task_of(row);
    let scores = task
        .columns()
        .iter()
        .map(|column| {
            let aside = match column.unscoreable.map(|key| number(row, key)) {
                Some(count) if count > 0 => format!("[{count}?]"),
                _ => String::new(),
            };
            format!(
                "{} {}/{}{aside}",
                column.short,
                number(row, column.count),
                number(row, column.of)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{scores} structure {} {} {}",
        flag("structure_ok"),
        task.answer(),
        flag("correct")
    )
}

/// A trial's scores. `counts` go into the trial row under their names, and
/// the benchmark's `columns` say which of them are shown, and how.
pub struct Score {
    pub structure_ok: bool,
    pub correct: bool,
    pub counts: Vec<(&'static str, usize)>,
    /// The paths behind the over-reach and unscoreable counts.
    pub overreached: Vec<String>,
    pub unscoreable: Vec<String>,
}

/// One score as a fraction: `count` of `of`, and how many were set aside as
/// unscoreable, if that can happen.
pub struct Column {
    /// As a trial line says it.
    pub line: &'static str,
    /// As the rescore verdict says it.
    pub short: &'static str,
    /// As the summary table heads it.
    pub heading: &'static str,
    pub count: &'static str,
    pub of: &'static str,
    pub unscoreable: Option<&'static str>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Task {
    Ledgers,
    Projects,
}

impl Task {
    fn parse(text: &str) -> Result<Task, String> {
        match text {
            "ledgers" => Ok(Task::Ledgers),
            "projects" => Ok(Task::Projects),
            other => Err(format!(
                "unknown --task {other}; expected ledgers or projects"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Task::Ledgers => "ledgers",
            Task::Projects => "projects",
        }
    }

    fn root_task(self) -> &'static str {
        match self {
            Task::Ledgers => ledgers::ROOT_TASK,
            Task::Projects => projects::ROOT_TASK,
        }
    }

    fn columns(self) -> &'static [Column] {
        match self {
            Task::Ledgers => &ledgers::COLUMNS,
            Task::Projects => &projects::COLUMNS,
        }
    }

    /// What the root's final message is judged as.
    fn answer(self) -> &'static str {
        match self {
            Task::Ledgers => "totals",
            Task::Projects => "answer",
        }
    }
}

/// A benchmark's fixture, and what it says the right answer is.
pub enum Fixture {
    Ledgers(ledgers::Fixture),
    Projects(projects::Fixture),
}

impl Fixture {
    fn write(task: Task, dir: &Path) -> Result<Fixture, String> {
        Ok(match task {
            Task::Ledgers => Fixture::Ledgers(ledgers::Fixture::write(dir)?),
            Task::Projects => Fixture::Projects(projects::Fixture::write(dir)?),
        })
    }

    /// A fixture already on disk, read back the way the benchmark would score
    /// it. A benchmark is rescored against the fixture its trials faced,
    /// never against today's generator.
    pub fn read(dir: &Path) -> Result<Fixture, String> {
        if dir.join(projects::ROOT).is_dir() {
            Ok(Fixture::Projects(projects::Fixture::read(dir)?))
        } else {
            Ok(Fixture::Ledgers(ledgers::Fixture::read(dir)?))
        }
    }

    pub fn task(&self) -> Task {
        match self {
            Fixture::Ledgers(_) => Task::Ledgers,
            Fixture::Projects(_) => Task::Projects,
        }
    }

    fn dir(&self) -> &Path {
        match self {
            Fixture::Ledgers(fixture) => &fixture.dir,
            Fixture::Projects(fixture) => &fixture.dir,
        }
    }

    /// The expected answer, in one line.
    pub fn describe(&self) -> String {
        match self {
            Fixture::Ledgers(fixture) => fixture.describe(),
            Fixture::Projects(fixture) => fixture.describe(),
        }
    }

    pub fn score(&self, agents: &[Observed], root_handoff: &str) -> Score {
        match self {
            Fixture::Ledgers(fixture) => ledgers::score(agents, root_handoff, fixture),
            Fixture::Projects(fixture) => projects::score(agents, root_handoff, fixture),
        }
    }
}

pub async fn command(args: &Args) -> Result<ExitCode, String> {
    args.known(&[crate::COMMON, &["grid", "reps", "budget", "task"]].concat())?;
    let task = Task::parse(&args.one("task", "ledgers"))?;
    let reps: usize = args.number("reps", 1)?;
    let budget: f64 = args.number("budget", 5.00)?;
    let default_grid = format!(
        "{}@{}",
        args.one("model", "anthropic/claude-sonnet-5"),
        args.provider()
    );
    let grid = args.list("grid", &default_grid);
    let cuts = args
        .list("cut", "before")
        .iter()
        .map(|c| Cut::parse(c))
        .collect::<Result<Vec<_>, _>>()?;
    let identities = args
        .list("identity", "agent")
        .iter()
        .map(|i| Identity::parse(i))
        .collect::<Result<Vec<_>, _>>()?;
    let modes = args
        .list("mode", "fork")
        .iter()
        .map(|m| Mode::parse(m))
        .collect::<Result<Vec<_>, _>>()?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let bench_dir = args.runs_dir().join(format!("{stamp}-bench"));
    std::fs::create_dir_all(&bench_dir).map_err(|e| format!("create bench directory: {e}"))?;
    let fixture = Fixture::write(task, &bench_dir.join("fixture"))?;
    let (first_model, first_provider) = grid[0].split_once('@').unwrap_or((grid[0].as_str(), ""));
    let sample = args.config_with(
        first_model,
        first_provider,
        Framing {
            cut: cuts[0],
            identity: identities[0],
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
    println!("{}", fixture.describe());

    let mut combos = Vec::new();
    for target in &grid {
        let (model, provider) = target.split_once('@').unwrap_or((target.as_str(), ""));
        for &cut in &cuts {
            for &identity in &identities {
                for &mode in &modes {
                    combos.push((
                        model.to_string(),
                        provider.to_string(),
                        Framing {
                            cut,
                            identity,
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
                framing.identity.name(),
                framing.mode.name()
            );
            let config =
                args.config_with(model, provider, *framing, BENCH_MAX_DEPTH, BENCH_MAX_COST)?;
            let mut session = Session::open(
                config,
                fixture.dir(),
                &bench_dir,
                &label,
                false,
                false,
                args.session_id(),
            )?;
            session.say(task.root_task());
            // A trial that faults — including one that reaches its own cap —
            // is a scored trial. Only the whole-benchmark budget stops the
            // benchmark.
            let outcome = session.turn().await;
            let ending = session.finish(&outcome);
            let observed = Observed::from_records(&ending.agents);
            let scored = fixture.score(&observed, &ending.handoff);
            spent += ending.cost;
            let facts = TrialFacts {
                trial: trial as u64,
                rep: rep as u64,
                model: model.clone(),
                provider: provider.clone(),
                cut: framing.cut.name().to_string(),
                identity: framing.identity.name().to_string(),
                mode: framing.mode.name().to_string(),
                outcome: outcome.short().to_string(),
                detail: outcome.label(),
                cost: ending.cost,
                millis: ending.agents.iter().map(|a| a.millis).max().unwrap_or(0),
                fault_kind: ending.fault_kind,
            };
            let row = trial_row(task, &facts, &observed, &scored);
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

/// One table per benchmark: every row of one run was scored by the same one.
pub fn summarise(rows: &[serde_json::Value]) -> String {
    let columns = rows.first().map(task_of).unwrap_or(Task::Ledgers).columns();
    let headings: String = columns
        .iter()
        .map(|c| format!(" {} |", c.heading))
        .collect();
    let mut text = format!(
        "| combo | trials | invalid |{headings} structure ok | correct | mean cost | child cache read | child first-request cache | shared-part hits | cache written | all-agent cache read | mean wall |\n|{}\n",
        " --- |".repeat(columns.len() + 12)
    );
    let mut combos: Vec<String> = Vec::new();
    for row in rows {
        let combo = combo_of(row);
        if !combos.contains(&combo) {
            combos.push(combo);
        }
    }
    for combo in combos {
        let all: Vec<&serde_json::Value> =
            rows.iter().filter(|row| combo_of(row) == combo).collect();
        // Everything past this point is computed over the trials that are
        // evidence about the model. A provider fault is reported, and then
        // kept out of every rate and every mean.
        let group: Vec<&&serde_json::Value> = all
            .iter()
            .filter(|row| row["valid"].as_bool().unwrap_or(true))
            .collect();
        let invalid = all.len() - group.len();
        if group.is_empty() {
            text.push_str(&format!(
                "| {combo} | {} | {invalid} |{}\n",
                all.len(),
                " — |".repeat(columns.len() + 9)
            ));
            continue;
        }
        let n = group.len() as f64;
        let sum = |key: &str| {
            group
                .iter()
                .map(|r| r[key].as_f64().unwrap_or(0.0))
                .sum::<f64>()
        };
        let counted = |key: &str| {
            group
                .iter()
                .filter(|r| r[key].as_bool().unwrap_or(false))
                .count()
        };
        let scores: String = columns
            .iter()
            .map(|column| {
                let aside = match column.unscoreable.map(|key| sum(key) as u64) {
                    Some(count) if count > 0 => format!(" ({count} unscoreable)"),
                    _ => String::new(),
                };
                format!(
                    " {}/{}{aside} |",
                    sum(column.count) as u64,
                    sum(column.of) as u64
                )
            })
            .collect();
        text.push_str(&format!(
            "| {combo} | {} | {invalid} |{scores} {}/{} | {}/{} | ${:.4} | {:.1}% | {:.1}% | {}/{} | {} | {:.1}% | {:.1}s |\n",
            all.len(),
            counted("structure_ok"),
            group.len(),
            counted("correct"),
            group.len(),
            sum("cost") / n,
            share(sum("child_cached_in"), sum("child_uncached_in")),
            share(sum("child_first_cached_in"), sum("child_first_uncached_in")),
            sum("shared_hits") as u64,
            sum("later_siblings") as u64,
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
        row["identity"].as_str().unwrap_or(""),
        row["mode"].as_str().unwrap_or(""),
    )
}
