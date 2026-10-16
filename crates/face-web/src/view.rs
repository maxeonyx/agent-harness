//! What the page is sent: the folded state's views as JSON, and one line
//! per fact.
//!
//! Ids are sent as hex strings: JSON numbers cannot hold a u128.

use std::collections::BTreeSet;

use harness_core::call::{Caller, Outcome as CallOutcome};
use harness_core::delivery::Content;
use harness_core::environment::LimbKind;
use harness_core::id::{CallId, TaskId, Timestamp, ToolUseId, ValueId};
use harness_core::journal::{Fact, Record};
use harness_core::model_call::{Ending, ModelCallEvent};
use harness_core::state::{Ended, State, Waiting};
use harness_core::task::{
    Drain, Input, Mode, Move, Origin, Outcome, Phase, Scope, Task, TaskEvent,
};
use serde_json::{Value, json};

/// A stored value as text, for the page: a mandate, a call's input.
type Text<'a> = &'a dyn Fn(ValueId) -> Option<Vec<u8>>;

fn text(value: Text, id: ValueId) -> Option<String> {
    value(id).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// The tasks, the scopes that own them, what each waits on, Max's queue and
/// his subscription, as of the last fold.
pub fn view(state: &State, value: Text, now: Timestamp) -> Value {
    let quota = state.quota();
    let tasks: Vec<Value> = state
        .tasks()
        .map(|t| task(state, value, t, state.last_turn(t.id), now))
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
    // The workspaces, by directory, which is a workspace limb's name: what a
    // new task can be given, or a new limb made for.
    let workspaces: Vec<Value> = state
        .limbs()
        .filter(|l| matches!(&l.kind, LimbKind::Plugin { kind, .. } if kind == "workspace"))
        .map(|l| json!({"id": l.id.to_string(), "dir": l.name}))
        .collect();
    let environments: Vec<Value> = state
        .environments()
        .map(|e| {
            json!({
                "id": e.id.to_string(),
                "name": e.name,
                "model": e.model,
                "places": e.places,
            })
        })
        .collect();
    // Each scope: the spawn call that opened it, and the tool use it
    // answers, so a page can put a scope's members beside the spawn in its
    // owner's context.
    let spawns: BTreeSet<CallId> = state
        .tasks()
        .filter_map(|t| match t.scope {
            Scope::Spawn(call) => Some(call),
            Scope::Max => None,
        })
        .collect();
    let scopes: serde_json::Map<String, Value> = spawns
        .into_iter()
        .filter_map(|id| {
            let record = state.call(id)?;
            let (task, background) = match record.call.caller {
                Caller::Agent { task, .. } => (task, false),
                Caller::Background { task, .. } => (task, true),
                Caller::Max { .. } | Caller::Meta { .. } => return None,
            };
            let scope = json!({
                "owner": task.to_string(),
                "background": background,
                "made": record.made.seq.0,
                "settled": record.settled.is_some(),
            });
            Some((id.to_string(), scope))
        })
        .collect();
    json!({
        "seq": state.last().map_or(0, |s| s.0),
        "scopes": scopes,
        "environments": environments,
        "workspaces": workspaces,
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
    let window = |w: Option<&harness_core::model_call::Window>| {
        w.map(|w| json!({"used": w.used, "resets": w.resets.map(|t| t.0)}))
    };
    json!({
        "at": ended.at.at.0,
        "five_hour": window(quota.and_then(|q| q.five_hour.as_ref())),
        "seven_day": window(quota.and_then(|q| q.seven_day.as_ref())),
        "on_plan": quota.is_none_or(|q| q.on_plan),
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
    let outcome = match t.outcome() {
        Some(Outcome::Result { .. }) => json!({"kind": "result"}),
        Some(Outcome::Failed { reason }) => json!({"kind": "failed", "reason": reason}),
        Some(Outcome::Cancelled { by }) => {
            json!({"kind": "cancelled", "by": format!("{by:?}")})
        }
        None => Value::Null,
    };
    // The spawn whose scope Max can open an aside in: the one its round
    // waits on while members of it run.
    let scoped = match t.phase() {
        Some(Phase::Acting(step)) if state.scoped(t) => step
            .outstanding
            .iter()
            .find(|c| state.members(Scope::Spawn(**c)).any(|m| !m.is_done()))
            .map(|c| c.to_string()),
        _ => None,
    };
    json!({
        "id": t.id.to_string(),
        "name": t.name,
        "created": {"seq": t.created.seq.0, "at": t.created.at.0},
        "started_by": t.by.and_then(|c| tool_use(state, c)).map(|t| t.0.clone()),
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
        "head": t.head().map(|h| h.to_string()),
        "environment": state.environment(t.environment).map(|e| e.name.clone()),
        "places": t.places.iter().map(|(place, limb)| (place.clone(), state.limb(*limb).map_or_else(|| limb.to_string(), |l| l.name.clone()))).collect::<std::collections::BTreeMap<String, String>>(),
        "held": t.held,
        "scoped": scoped,
        "draining": t.drain().map(|d| match d {
            Drain::ToMax => "to Max".to_string(),
            Drain::Stop(by) => format!("stop ({by:?})"),
            Drain::Fail(reason) => format!("fail: {reason}"),
        }),
        "failures": t.live().map_or(0, |l| l.failures),
        "trouble": last.map_or(Value::Null, trouble),
        "pending": pending_words(state, value, t),
        "background": t.background.iter().map(|c| call(state, value, *c)).collect::<Vec<_>>(),
        "tokens": last.and_then(|e| e.usage).map(|u| json!({
            "context": u.input + u.cache_read + u.cache_write_short + u.cache_write_long,
            "output": u.output,
        })),
        "outcome": outcome,
        "waiting": state.waiting_on(t.id, now).map(|w| waiting(state, value, &w)),
    })
}

/// Max's words delivered to the task that no request has rendered yet: what
/// he sent that the agent has not read.
fn pending_words(state: &State, value: Text, t: &Task) -> Vec<Value> {
    t.pending
        .iter()
        .filter_map(|p| match p {
            Input::Delivery(id) => state.delivery(*id),
            _ => None,
        })
        .filter_map(|d| match d.delivery.content {
            Content::Words(words) => Some(json!({"at": d.at.at.0, "content": text(value, words)})),
            _ => None,
        })
        .collect()
}

/// The tool use an agent's call answers, directly or through the
/// `background` call that started it.
fn tool_use(state: &State, call: CallId) -> Option<&ToolUseId> {
    match &state.call(call)?.call.caller {
        Caller::Agent { tool_use, .. } => Some(tool_use),
        Caller::Background { from, .. } => tool_use(state, *from),
        Caller::Max { .. } | Caller::Meta { .. } => None,
    }
}

/// The throbber: what the task is waiting on, and since when.
fn waiting(state: &State, value: Text, w: &Waiting) -> Value {
    let calls =
        |calls: &[CallId]| -> Vec<Value> { calls.iter().map(|c| call(state, value, *c)).collect() };
    match w {
        Waiting::Starting { limbs, since } => json!({
            "kind": "starting",
            "since": since.0,
            "limbs": limbs.iter().map(|l| state.limb(*l).map_or_else(|| l.to_string(), |l| l.name.clone())).collect::<Vec<_>>(),
        }),
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
                    Caller::Agent { task, .. } | Caller::Background { task, .. } => name(task),
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
                    Origin::Fresh { environment, .. } => state
                        .environment(*environment)
                        .map_or_else(|| environment.to_string(), |e| e.name.clone()),
                    Origin::Fork { from } => format!("forked from {}", name(*from)),
                    Origin::Resume { from } => format!("resuming {}", name(*from)),
                    Origin::Aside { from } => format!("an aside of {}", name(*from)),
                };
                format!("{} created ({how})", c.name)
            }
            TaskEvent::ModeSet(mode) => format!("{} is now {mode:?}", name(*id)),
            TaskEvent::Renamed(new) => format!("{} renamed to {new}", name(*id)),
            TaskEvent::Moved(Move::Root(start)) => {
                format!("{} has a new root, on {}", name(*id), start.root.model)
            }
            TaskEvent::Moved(Move::Handover { start, .. }) => {
                format!(
                    "{} handed over to a new root, on {}",
                    name(*id),
                    start.root.model
                )
            }
            TaskEvent::Moved(Move::Rewind { .. }) => format!("{} rewound", name(*id)),
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
        Fact::Described { limb, description } => {
            let limb = state.limb(*limb).map_or("?".into(), |l| l.name.clone());
            format!("{limb} described: {} tools", description.tools.len())
        }
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
