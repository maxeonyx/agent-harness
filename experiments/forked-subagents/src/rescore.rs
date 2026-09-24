//! `forks rescore <bench-dir>` — score a benchmark that has already been paid
//! for, again, with today's scorer.
//!
//! The trial directories are the index, not `trials.json`: two benchmarks
//! started in the same second share a directory and the second one to finish
//! overwrites the first's `trials.json`, so it can be missing trials that were
//! actually run. Each trial directory is self-contained.
//!
//! `wire.jsonl` is the ground truth for what an agent did. It holds every
//! request body, and a request body holds the tool results of the turn before
//! it — which is how a read that failed is told from one that worked.
//! `summary.json` supplies the tree: who each agent's parent was, what its
//! assignment said, what it reported.
//!
//! Nothing is written back into the run directory. The evidence is read-only.

use crate::Args;
use crate::agent::FaultKind;
use crate::bench::{Fixture, Observed, ReadAttempt, TrialFacts, score, summarise, trial_row};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub async fn command(args: &Args) -> Result<ExitCode, String> {
    args.known(&["grid"])?;
    let dir = args
        .positional
        .first()
        .ok_or("forks rescore needs a benchmark directory: forks rescore <bench-dir>")?;
    let dir = PathBuf::from(dir);
    let fixture = Fixture::read(&dir.join("fixture"))?;

    println!("rescoring {}", dir.display());
    println!(
        "fixture totals: {}",
        fixture
            .totals
            .iter()
            .map(|(name, total)| format!("{name}={total:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let old = old_scores(&dir);
    let mut trials: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.join("summary.json").is_file())
        .collect();
    trials.sort();
    if trials.is_empty() {
        return Err(format!("no trial directories in {}", dir.display()));
    }

    let mut rows = Vec::new();
    let mut changes = Vec::new();
    let (mut recorded, mut read_back, mut unknown) = (0usize, 0usize, 0usize);
    for (number, trial) in trials.iter().enumerate() {
        let (facts, agents, root_handoff, provenance) = read_trial(trial, number as u64 + 1)?;
        match provenance {
            Provenance::Recorded => recorded += 1,
            Provenance::ReadBack => read_back += 1,
            Provenance::Unknown => unknown += 1,
            Provenance::NoFault => {}
        }
        let scored = score(&agents, &root_handoff, &fixture);
        let row = trial_row(&facts, &agents, &scored);
        let key = format!(
            "{}|{}|{}|{}|{}",
            facts.model, facts.cut, facts.words, facts.mode, facts.rep
        );
        changes.push(compare(
            trial.file_name().unwrap().to_string_lossy().as_ref(),
            old.get(&key),
            &row,
        ));
        rows.push(row);
    }

    println!("\n{}", changes.join("\n"));
    println!("\n{}", summarise(&rows));
    println!(
        "{} trial directories rescored; `trials.json` listed {}.",
        trials.len(),
        old.len()
    );
    if recorded + read_back + unknown > 0 {
        println!(
            "faulted trials: {recorded} recorded why, {read_back} read back from the fault message, {unknown} unclassified (kept, since discarding evidence on a guess is worse)."
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// The scores this benchmark recorded when it ran, keyed so a trial directory
/// can find its own row even when two benchmarks shared the directory and
/// their trial numbers collided.
fn old_scores(dir: &Path) -> BTreeMap<String, serde_json::Value> {
    let Ok(text) = std::fs::read_to_string(dir.join("trials.json")) else {
        return BTreeMap::new();
    };
    let Ok(rows): Result<Vec<serde_json::Value>, _> = serde_json::from_str(&text) else {
        return BTreeMap::new();
    };
    rows.into_iter()
        .map(|row| {
            let key = format!(
                "{}|{}|{}|{}|{}",
                row["model"].as_str().unwrap_or(""),
                row["cut"].as_str().unwrap_or(""),
                row["words"].as_str().unwrap_or(""),
                row["mode"].as_str().unwrap_or(""),
                row["rep"].as_u64().unwrap_or(0),
            );
            (key, row)
        })
        .collect()
}

fn verdict(row: &serde_json::Value) -> String {
    let n = |key: &str| row[key].as_f64().unwrap_or(0.0) as u64;
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
    let aside = |count: u64| {
        if count > 0 {
            format!("[{count}?]")
        } else {
            String::new()
        }
    };
    format!(
        "leaf {}/{}{} region {}/{}{} policy {}/{} structure {} totals {}",
        n("leaf_overreach"),
        n("leaves"),
        aside(n("leaves_unscoreable")),
        n("region_overreach"),
        n("regions"),
        aside(n("regions_unscoreable")),
        n("policy_rereads"),
        n("below_root"),
        flag("structure_ok"),
        flag("correct"),
    )
}

fn compare(label: &str, old: Option<&serde_json::Value>, new: &serde_json::Value) -> String {
    let combo = format!(
        "{}@{} {}/{}/{}",
        new["model"].as_str().unwrap_or(""),
        new["provider"].as_str().unwrap_or(""),
        new["cut"].as_str().unwrap_or(""),
        new["words"].as_str().unwrap_or(""),
        new["mode"].as_str().unwrap_or(""),
    );
    let fresh = verdict(new);
    match old {
        None => format!("{label}\n  {combo}\n  old  (not in trials.json)\n  new  {fresh}"),
        Some(old) => {
            let was = verdict(old);
            let mark = if was == fresh { "unchanged" } else { "CHANGED" };
            format!("{label}\n  {combo}\n  old  {was}\n  new  {fresh}   [{mark}]")
        }
    }
}

/// One recorded trial, rebuilt into the same shape a live trial is scored in.
fn read_trial(
    dir: &Path,
    number: u64,
) -> Result<(TrialFacts, Vec<Observed>, String, Provenance), String> {
    let summary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("summary.json"))
            .map_err(|e| format!("read {}/summary.json: {e}", dir.display()))?,
    )
    .map_err(|e| format!("parse {}/summary.json: {e}", dir.display()))?;
    let text = |key: &str| summary[key].as_str().unwrap_or("").to_string();

    let rep = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once("-rep"))
        .and_then(|(_, rep)| rep.parse().ok())
        .unwrap_or(0);
    let (kind, provenance) = fault_kind_with_provenance(&summary);
    let facts = TrialFacts {
        trial: number,
        rep,
        model: text("model"),
        provider: text("provider"),
        cut: text("cut"),
        words: text("words"),
        mode: text("mode"),
        outcome: text("outcome"),
        detail: text("detail"),
        cost: summary["cost"].as_f64().unwrap_or(0.0),
        millis: summary["millis"].as_u64().unwrap_or(0) as u128,
        fault_kind: kind,
    };

    let wire = read_wire(&dir.join("wire.jsonl"))?;
    let empty = Vec::new();
    let agents = summary["agents"].as_array().unwrap_or(&empty);
    if agents.is_empty() {
        return Err(format!("{}/summary.json records no agents", dir.display()));
    }
    let observed = agents
        .iter()
        .map(|agent| {
            let path = agent["path"].as_str().unwrap_or("").to_string();
            let seen = wire.get(&path).cloned().unwrap_or_default();
            Observed {
                depth: agent["depth"].as_u64().unwrap_or(0) as usize,
                children: agent["children"].as_array().map(Vec::len).unwrap_or(0),
                task: agent["task"].as_str().map(str::to_string),
                handoff: agent["handoff"].as_str().unwrap_or("").to_string(),
                reads: seen.reads,
                forked: seen.forked,
                cached_in: seen.usages.iter().map(|u| u.0).sum(),
                uncached_in: seen.usages.iter().map(|u| u.1).sum(),
                written_in: seen.usages.iter().map(|u| u.2).sum(),
                first_cached_in: seen.usages.first().map(|u| u.0).unwrap_or(0),
                first_uncached_in: seen.usages.first().map(|u| u.1).unwrap_or(0),
                path,
            }
        })
        .collect();
    Ok((facts, observed, text("root_handoff"), provenance))
}

/// Why a recorded run faulted. Runs made since the harness started recording
/// `fault_kind` say so outright; older ones are read back from the fault
/// message, which is the only evidence they carry. Every message the harness
/// has ever produced is covered, and anything unrecognised is left
/// unclassified rather than guessed at — an unclassified fault keeps the
/// trial, because silently discarding evidence is worse than keeping a
/// doubtful row.
pub enum Provenance {
    /// The run recorded why it faulted.
    Recorded,
    /// Read back from the fault message, which is all an older run carries.
    ReadBack,
    /// It faulted, and nothing on disk says why.
    Unknown,
    NoFault,
}

fn fault_kind_with_provenance(summary: &serde_json::Value) -> (Option<FaultKind>, Provenance) {
    if summary["fault"].is_null() {
        return (None, Provenance::NoFault);
    }
    let recorded = summary["fault_kind"].as_str().is_some();
    match fault_kind(summary) {
        Some(kind) if recorded => (Some(kind), Provenance::Recorded),
        Some(kind) => (Some(kind), Provenance::ReadBack),
        None => (None, Provenance::Unknown),
    }
}

fn fault_kind(summary: &serde_json::Value) -> Option<FaultKind> {
    if let Some(name) = summary["fault_kind"].as_str() {
        return match name {
            "provider" => Some(FaultKind::Provider),
            "budget" => Some(FaultKind::Budget),
            "runaway" => Some(FaultKind::Runaway),
            "panic" => Some(FaultKind::Panic),
            _ => None,
        };
    }
    let fault = summary["fault"].as_str()?;
    let reason = fault
        .split_once(": ")
        .map(|(_, rest)| rest)
        .unwrap_or(fault);
    if reason.starts_with("spend cap reached") {
        Some(FaultKind::Budget)
    } else if reason.starts_with("agent ran past") {
        Some(FaultKind::Runaway)
    } else if reason.starts_with("agent task panicked") {
        Some(FaultKind::Panic)
    } else if reason.starts_with("provider returned")
        || reason.starts_with("request failed")
        || reason.starts_with("response was not JSON")
        || reason.starts_with("could not parse response")
        || reason.starts_with("rate limited for longer than")
        || reason.contains("attempts failed")
    {
        Some(FaultKind::Provider)
    } else {
        None
    }
}

#[derive(Clone, Default)]
struct Seen {
    reads: Vec<ReadAttempt>,
    forked: bool,
    /// cached, uncached, written — one entry per answered request, in order.
    usages: Vec<(u64, u64, u64)>,
}

/// Replay one trial's wire log into what each agent did.
fn read_wire(path: &Path) -> Result<BTreeMap<String, Seen>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    // Per agent: every tool call it made, and every tool result that came
    // back. A call is matched to its result by id; the result of a turn shows
    // up in the next request's messages, so both directions are collected
    // across the whole log before anything is judged.
    let mut calls: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let mut results: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut seen: BTreeMap<String, Seen> = BTreeMap::new();
    let mut unreadable = 0usize;

    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            unreadable += 1;
            continue;
        };
        let agent = entry["agent"].as_str().unwrap_or("").to_string();
        let body = &entry["body"];
        match entry["kind"].as_str() {
            Some("request") => {
                for message in body["messages"].as_array().into_iter().flatten() {
                    if message["role"] == "tool"
                        && let Some(id) = message["tool_call_id"].as_str()
                    {
                        results.entry(agent.clone()).or_default().insert(
                            id.to_string(),
                            message["content"].as_str().unwrap_or("").to_string(),
                        );
                    }
                }
            }
            Some("response") => {
                let usage = &body["usage"];
                let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
                let cached = usage["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                let written = usage["prompt_tokens_details"]["cache_write_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                seen.entry(agent.clone()).or_default().usages.push((
                    cached,
                    prompt.saturating_sub(cached),
                    written,
                ));
                for call in body["choices"][0]["message"]["tool_calls"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    calls.entry(agent.clone()).or_default().push((
                        call["id"].as_str().unwrap_or("").to_string(),
                        call["function"]["name"].as_str().unwrap_or("").to_string(),
                        call["function"]["arguments"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                    ));
                }
            }
            _ => {}
        }
    }

    for (agent, made) in calls {
        let answers = results.remove(&agent).unwrap_or_default();
        let entry = seen.entry(agent).or_default();
        for (id, name, arguments) in made {
            if name == "task" {
                entry.forked = true;
            }
            if name != "read_file" {
                continue;
            }
            let target = serde_json::from_str::<serde_json::Value>(&arguments)
                .ok()
                .and_then(|value| value.get("path")?.as_str().map(str::to_string))
                .unwrap_or_default();
            // A call whose result never reached a later request is a read we
            // cannot confirm happened, and is not counted as one.
            let ok = answers
                .get(&id)
                .is_some_and(|answer| !answer.starts_with("Error:"));
            entry.reads.push(ReadAttempt { path: target, ok });
        }
    }
    if unreadable > 0 {
        // Two benchmarks that shared a run directory also shared this file,
        // and their writes interleaved. Whatever is left is partial, and
        // saying so is the only honest thing to do with it.
        eprintln!(
            "  WARNING {}: {unreadable} line(s) of wire.jsonl could not be read; this trial's evidence is incomplete",
            path.display()
        );
    }
    Ok(seen)
}
