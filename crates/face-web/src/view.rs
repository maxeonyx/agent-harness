//! What the page is sent: the folded state's views as JSON, and one line
//! per fact.
//!
//! Ids are sent as hex strings: JSON numbers cannot hold a u128.

use std::collections::HashMap;

use harness_core::call::{Caller, Outcome as CallOutcome};
use harness_core::delivery::Content;
use harness_core::id::{CallId, TaskId, Timestamp, ValueId};
use harness_core::journal::{Fact, Record};
use harness_core::model_call::{Ending, ModelCallEvent, Purpose};
use harness_core::state::{Ended, State, Waiting};
use harness_core::task::{Drain, Mode, Origin, Outcome, Scope, Task, TaskEvent, TaskState};
use serde_json::{Value, json};

/// A stored value as text, for the page: a mandate, a call's input.
type Text<'a> = &'a dyn Fn(ValueId) -> Option<Vec<u8>>;

fn text(value: Text, id: ValueId) -> Option<String> {
    value(id).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// The tree, what each task waits on, Max's queue and his subscription, as
/// of the last fold.
pub fn view(state: &State, value: Text, now: Timestamp) -> Value {
    // Each task's last turn to end, and the last report of the quota.
    let mut last: HashMap<TaskId, &Ended> = HashMap::new();
    let mut quota: Option<&Ended> = None;
    for (_, m) in state.model_calls() {
        let Some(ended) = &m.ended else { continue };
        if m.call.purpose == Purpose::Turn {
            let slot = last.entry(m.call.task).or_insert(ended);
            if ended.at.seq > slot.at.seq {
                *slot = ended;
            }
        }
        if ended.quota.is_some() && quota.is_none_or(|q| ended.at.seq > q.at.seq) {
            quota = Some(ended);
        }
    }
    let tasks: Vec<Value> = state
        .tasks()
        .map(|t| task(state, value, t, last.get(&t.id).copied(), now))
        .collect();
    let queue: Vec<Value> = state
        .queue()
        .iter()
        .map(|q| {
            json!({
                "task": q.task.to_string(),
                "since": q.since.0,
                "ask": q.ask.map(|a| call(state, value, a)),
                "why": q.why,
            })
        })
        .collect();
    json!({
        "seq": state.last().map_or(0, |s| s.0),
        "build": state.launch().map(|(_, l)| l.build.clone()),
        "limited": state.limited(now).map(|t| t.0),
        "usage": quota.map(usage),
        "tasks": tasks,
        "queue": queue,
    })
}

/// The subscription as the latest response reported it.
fn usage(ended: &Ended) -> Value {
    let quota = ended.quota.as_ref();
    json!({
        "at": ended.at.at.0,
        "five_hour": quota.and_then(|q| q.five_hour),
        "seven_day": quota.and_then(|q| q.seven_day),
        "claim": quota.and_then(|q| q.claim.clone()),
    })
}

/// Why a task's last turn did not respond, if it did not.
fn trouble(ended: &Ended) -> Value {
    match &ended.ending {
        Ending::Failed {
            status, message, ..
        } => json!({
            "at": ended.at.at.0,
            "text": match status {
                Some(s) => format!("{s}: {message}"),
                None => message.clone(),
            },
        }),
        Ending::Limited { until } => json!({"at": ended.at.at.0, "until": until.0}),
        Ending::Responded { .. } | Ending::Abandoned { .. } | Ending::Lost => Value::Null,
    }
}

fn task(state: &State, value: Text, t: &Task, last: Option<&Ended>, now: Timestamp) -> Value {
    let (origin, from) = match t.origin {
        Origin::Fresh { .. } => ("fresh", None),
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
        "mandate": text(value, t.mandate.text),
        "head": t.head.to_string(),
        "environment": state.environment(t.binding.environment).map(|e| e.name.clone()),
        "held": t.held,
        "draining": t.draining.as_ref().map(|d| match d {
            Drain::ToMax => "to Max".to_string(),
            Drain::Stop(by) => format!("stop ({by:?})"),
            Drain::Fail(reason) => format!("fail: {reason}"),
        }),
        "failures": t.failures,
        "trouble": last.map_or(Value::Null, trouble),
        "pending": t.pending.len(),
        "outcome": outcome,
        "waiting": state.waiting_on(t.id, now).map(|w| waiting(state, value, &w)),
    })
}

/// The throbber: what the task is waiting on, and since when.
fn waiting(state: &State, value: Text, w: &Waiting) -> Value {
    let calls =
        |calls: &[CallId]| -> Vec<Value> { calls.iter().map(|c| call(state, value, *c)).collect() };
    match w {
        Waiting::Brain { since } => json!({"kind": "harness", "since": since.0}),
        Waiting::Held { since } => json!({"kind": "held", "since": since.0}),
        Waiting::Limited { until, since } => {
            json!({"kind": "limited", "until": until.0, "since": since.0})
        }
        Waiting::Model {
            call,
            since,
            streaming,
        } => json!({
            "kind": "model",
            "call": call.to_string(),
            "since": since.0,
            "streaming": streaming,
        }),
        Waiting::Calls { calls: c, since } => json!({
            "kind": "calls",
            "since": since.0,
            "calls": calls(c),
        }),
        Waiting::Max { since, ask } => json!({
            "kind": "max",
            "since": since.0,
            "ask": ask.map(|a| call(state, value, a)),
        }),
        Waiting::Listening { background, since } => json!({
            "kind": "listening",
            "since": since.0,
            "calls": calls(background),
        }),
        Waiting::Done { since } => json!({"kind": "done", "since": since.0}),
    }
}

fn call(state: &State, value: Text, id: CallId) -> Value {
    let Some(record) = state.call(id) else {
        return json!({"id": id.to_string()});
    };
    let children: Vec<String> = state
        .members(Scope::Spawn(id))
        .map(|t| t.id.to_string())
        .collect();
    let status = match (&record.settled, &record.dispatched) {
        (Some(_), _) => "settled",
        (None, Some(_)) => "running",
        (None, None) => "made",
    };
    json!({
        "id": id.to_string(),
        "tool": record.call.tool,
        "limb": record.limb.and_then(|l| state.limb(l)).map(|l| l.name.clone()),
        "input": text(value, record.call.input),
        "made": record.made.at.0,
        "status": status,
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
                    Caller::Meta { call, .. } => match state.call_owner(call) {
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
                let how = match &c.origin {
                    Origin::Fresh { agent, .. } => agent.clone(),
                    Origin::Fork { from } => format!("forked from {}", name(*from)),
                    Origin::Resume { from } => format!("resuming {}", name(*from)),
                    Origin::Aside { from } => format!("an aside of {}", name(*from)),
                };
                format!("{} created ({how})", c.name)
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
                        Ending::Responded { stop, .. } => {
                            let calls = state.model_call(*id).map_or(0, |m| m.calls.len());
                            format!("{task}: responded, {stop:?}, {calls} calls{tokens}")
                        }
                        Ending::Failed {
                            status, message, ..
                        } => match status {
                            Some(s) => format!("{task}: request failed, {s}: {message}"),
                            None => format!("{task}: request failed: {message}"),
                        },
                        Ending::Limited { .. } => format!("{task}: limited by the account"),
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
                E::Dispatched => format!("{} dispatched", call_name(*id)),
                E::Stopping => format!("{} stopping", call_name(*id)),
                E::Settled(o) => format!("{} {}", call_name(*id), outcome(o)),
            }
        }
        Fact::Delivery(d) => {
            let what = match &d.content {
                Content::Words(_) => "words".to_string(),
                Content::Prompt(p) => format!("{p:?} prompt"),
                Content::Aside(t) => format!("the result of aside {}", name(*t)),
            };
            format!("{what} delivered to {}", name(d.to))
        }
        Fact::Change(c) => match c.by {
            Some(by) => format!(
                "{} {} changed by {}",
                c.resource.kind,
                c.resource.name,
                call_name(by)
            ),
            None => format!("{} {} changed", c.resource.kind, c.resource.name),
        },
        Fact::Environment(e) => format!("environment {} defined", e.name),
        Fact::Limb(l) => format!("limb {} defined", l.name),
        Fact::Reach { limb, reach } => {
            let limb = state.limb(*limb).map_or("?".into(), |l| l.name.clone());
            match reach {
                harness_core::environment::Reach::Connected { .. } => format!("{limb} connected"),
                harness_core::environment::Reach::Lost => format!("{limb} lost"),
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
