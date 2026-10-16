//! What the page is sent: the folded state's views as JSON, and one line
//! per fact.
//!
//! Ids are sent as hex strings: JSON numbers cannot hold a u128.

use harness_core::call::{Caller, Outcome as CallOutcome, Status};
use harness_core::context::EntryId;
use harness_core::delivery::Content;
use harness_core::id::{CallId, TaskId, Timestamp};
use harness_core::journal::{Fact, Record};
use harness_core::model_call::{Ending, ModelCallEvent};
use harness_core::state::{State, Waiting};
use harness_core::task::{Drain, Mode, Origin, Outcome, Scope, Task, TaskEvent, TaskState};
use serde_json::{Value, json};

/// The tree, what each task waits on, and Max's queue, as of the last fold.
pub fn view(state: &State, now: Timestamp) -> Value {
    let tasks: Vec<Value> = state.tasks().map(|t| task(state, t, now)).collect();
    let queue: Vec<Value> = state
        .queue()
        .iter()
        .map(|q| {
            json!({
                "task": q.task.to_string(),
                "since": q.since.0,
                "ask": q.ask.map(|a| call(state, a)),
                "why": q.why,
            })
        })
        .collect();
    json!({
        "seq": state.last().map_or(0, |s| s.0),
        "build": state.build().map(|b| b.build.clone()),
        "limited": state.limited(now).map(|t| t.0),
        "tasks": tasks,
        "queue": queue,
    })
}

fn task(state: &State, t: &Task, now: Timestamp) -> Value {
    let (origin, from) = match t.origin {
        Origin::Fresh => ("fresh", None),
        Origin::Fork { from } => ("fork", Some(from)),
        Origin::Resume { from } => ("resume", Some(from)),
        Origin::Aside { from } => ("aside", Some(from)),
    };
    let outcome = match &t.state {
        TaskState::Done(Outcome::Result { .. }) => json!({"kind": "result"}),
        TaskState::Done(Outcome::Failed { reason }) => json!({"kind": "failed", "reason": reason}),
        TaskState::Done(Outcome::Cancelled { by }) => {
            json!({"kind": "cancelled", "by": format!("{by:?}")})
        }
        _ => Value::Null,
    };
    json!({
        "id": t.id.to_string(),
        "name": t.name,
        "agent": t.agent,
        "mode": match t.mode {
            Mode::Autonomous => "autonomous",
            Mode::UserFacing => "user-facing",
            Mode::Standing => "standing",
        },
        "origin": origin,
        "from": from.map(|t| t.to_string()),
        "owner": state.owner(t.id).map(|t| t.to_string()),
        "scope": match t.scope {
            Scope::Max => None,
            Scope::Spawn(call) => Some(call.to_string()),
        },
        "mandate": t.mandate.text,
        "head": t.head.0.to_string(),
        "environment": state.environment(t.binding.environment).map(|e| e.name.clone()),
        "held": t.held,
        "draining": t.draining.as_ref().map(|d| match d {
            Drain::ToMax => "to Max".to_string(),
            Drain::Stop(by) => format!("stop ({by:?})"),
            Drain::Fail(reason) => format!("fail: {reason}"),
        }),
        "failures": t.failures,
        "pending": t.pending.len(),
        "outcome": outcome,
        "waiting": state.waiting_on(t.id, now).map(|w| waiting(state, &w)),
    })
}

/// The throbber: what the task is waiting on, and since when.
fn waiting(state: &State, w: &Waiting) -> Value {
    match w {
        Waiting::Brain => json!({"kind": "harness"}),
        Waiting::Held => json!({"kind": "held"}),
        Waiting::Limited { until } => json!({"kind": "limited", "until": until.0}),
        Waiting::Model {
            since, streaming, ..
        } => json!({"kind": "model", "since": since.0, "streaming": streaming}),
        Waiting::Calls { calls } => json!({
            "kind": "calls",
            "since": calls.iter().filter_map(|c| state.call(*c)).map(|c| c.made.0).min(),
            "calls": calls.iter().map(|c| call(state, *c)).collect::<Vec<_>>(),
        }),
        Waiting::Max { since, ask } => json!({
            "kind": "max",
            "since": since.0,
            "ask": ask.map(|a| call(state, a)),
        }),
        Waiting::Listening { background } => json!({
            "kind": "listening",
            "calls": background.iter().map(|c| call(state, *c)).collect::<Vec<_>>(),
        }),
        Waiting::Done => json!({"kind": "done"}),
    }
}

fn call(state: &State, id: CallId) -> Value {
    let Some(record) = state.call(id) else {
        return json!({"id": id.to_string()});
    };
    let children: Vec<String> = state
        .members(Scope::Spawn(id))
        .map(|t| t.id.to_string())
        .collect();
    json!({
        "id": id.to_string(),
        "tool": record.call.tool,
        "limb": state.limb(record.call.limb).map(|l| l.name.clone()),
        "input": record.call.input.0,
        "made": record.made.0,
        "status": match record.status {
            Status::Made => "made",
            Status::Dispatched { .. } => "running",
            Status::Settled { .. } => "settled",
        },
        "children": children,
    })
}

/// One line saying what a record did, naming tasks as the state knows them.
pub fn describe(state: &State, record: &Record) -> String {
    let name = |id: TaskId| {
        state
            .task(id)
            .map_or_else(|| id.to_string(), |t| t.name.clone())
    };
    let call_name = |id: CallId| {
        state.call(id).map_or_else(
            || format!("call {id}"),
            |c| {
                let by = match c.call.caller {
                    Caller::Agent { task, .. } => name(task),
                    Caller::Max { .. } => "Max".into(),
                    Caller::Meta { call } => match state.call_owner(call) {
                        Some(task) => format!("the harness for {}", name(task)),
                        None => "the harness".into(),
                    },
                };
                format!("{by}'s {}", c.call.tool)
            },
        )
    };
    match &record.fact {
        Fact::Launched(l) => format!("brain launched, build {}", l.build),
        Fact::Task(id, event) => match event {
            TaskEvent::Created(c) => {
                let how = match c.origin {
                    Origin::Fresh => String::new(),
                    Origin::Fork { from } => format!(", forked from {}", name(from)),
                    Origin::Resume { from } => format!(", resuming {}", name(from)),
                    Origin::Aside { from } => format!(", an aside of {}", name(from)),
                };
                format!("{} created ({}{how})", c.name, c.agent)
            }
            TaskEvent::ModeSet(mode) => format!("{} is now {mode:?}", name(*id)),
            TaskEvent::Renamed(new) => format!("{} renamed to {new}", name(*id)),
            TaskEvent::Moved(m) => format!("{} moved: {m:?}", name(*id))
                .chars()
                .take(120)
                .collect(),
            TaskEvent::Drain(Drain::ToMax) => format!("{} interrupted", name(*id)),
            TaskEvent::Drain(Drain::Stop(by)) => format!("{} stopped by {by:?}", name(*id)),
            TaskEvent::Drain(Drain::Fail(why)) => format!("{} failing: {why}", name(*id)),
            TaskEvent::Held(true) => format!("{} held", name(*id)),
            TaskEvent::Held(false) => format!("{} released", name(*id)),
        },
        Fact::ModelCall(id, event) => {
            let task = state.model_call(*id).map(|m| name(m.call.task));
            let task = task.unwrap_or_else(|| "?".into());
            match event {
                ModelCallEvent::Sent(_) => format!("{task}: request sent"),
                ModelCallEvent::FirstByte => format!("{task}: first byte"),
                ModelCallEvent::Ended { ending, usage, .. } => {
                    let tokens = usage.map_or(String::new(), |u| {
                        format!(
                            " ({} in, {} cached, {} out)",
                            u.input, u.cache_read, u.output
                        )
                    });
                    match ending {
                        Ending::Responded { stop, calls, .. } => {
                            format!("{task}: responded, {stop:?}, {} calls{tokens}", calls.len())
                        }
                        Ending::Failed {
                            status, message, ..
                        } => match status {
                            Some(s) => format!("{task}: request failed, {s}: {message}"),
                            None => format!("{task}: request failed: {message}"),
                        },
                        Ending::Abandoned { .. } => format!("{task}: response abandoned"),
                        Ending::Lost => format!("{task}: request lost"),
                    }
                }
            }
        }
        Fact::Call(id, event) => {
            use harness_core::call::CallEvent as E;
            match event {
                E::Made(_) => format!("{} made", call_name(*id)),
                E::Dispatched { .. } => format!("{} dispatched", call_name(*id)),
                E::Settled(o) | E::Late(o) => format!("{} {}", call_name(*id), outcome(o)),
            }
        }
        Fact::Delivery(d) => {
            let what = match &d.content {
                Content::Words(_) => "words".to_string(),
                Content::Prompt(p) => format!("{p:?} prompt"),
                Content::Event { .. } => "an event".into(),
                Content::Aside(t) => format!("the result of aside {}", name(*t)),
            };
            format!("{what} delivered to {}", name(d.to))
        }
        Fact::Change(c) => format!("{} changed", c.resource.name),
        Fact::Environment(e) => format!("environment {} defined", e.name),
        Fact::Limb(l) => format!("limb {} defined", l.name),
        Fact::Reach { limb, reach } => {
            let limb = state.limb(*limb).map_or("?".into(), |l| l.name.clone());
            match reach {
                harness_core::environment::Reach::Connected { .. } => format!("{limb} connected"),
                harness_core::environment::Reach::Lost { .. } => format!("{limb} lost"),
            }
        }
        Fact::Ranking(r) => format!("queue ranked: {} placed", r.order.len()),
        Fact::Focus(_) => "a face changed focus".into(),
    }
}

fn outcome(o: &CallOutcome) -> &'static str {
    match o {
        CallOutcome::Completed(_) => "completed",
        CallOutcome::Failed(_) => "failed",
        CallOutcome::Refused { .. } => "refused",
        CallOutcome::Interrupted { .. } => "interrupted",
        CallOutcome::Unknown => "ended unknown",
    }
}

/// The entry its hex names.
pub fn parse_entry(hex: &str) -> Option<EntryId> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut digest = [0u8; 32];
    for (i, pair) in bytes.chunks(2).enumerate() {
        digest[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(EntryId(harness_core::id::Digest(digest)))
}
