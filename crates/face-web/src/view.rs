//! What a tab is shown: the folded state as one row per object, and one
//! line per fact for the brain's log.
//!
//! Ids are sent as strings: JSON numbers cannot hold them.

use std::collections::{BTreeMap, BTreeSet};

use harness_core::call::{Caller, Outcome as CallOutcome, Owner};
use harness_core::delivery::Content;
use harness_core::environment::{Effort, Environment, LimbKind, Reach};
use harness_core::fact::Fact;
use harness_core::id::{CallId, LimbId, TaskId, Timestamp, ToolUseId, ValueId};
use harness_core::model_call::{Ending, ModelCallEvent, Window};
use harness_core::state::{Ended, State, Waiting};
use harness_core::task::{
    Drain, Draining, Input, Mode, Move, Onto, Origin, Outcome, Pause, Phase, Scope, Task, TaskEvent,
};
use harness_face_tab::{DraftRow, FaceKey, FaceRow};
use harness_provider::Model;
use serde_json::{Value, json};

/// A stored value as text, for the page: a mandate, a call's input.
type Text<'a> = &'a dyn Fn(ValueId) -> Option<Vec<u8>>;

fn text(value: Text, id: ValueId) -> Option<String> {
    value(id).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// The model and effort a task's next request runs at, and while its cache
/// is warm, until when and how many tokens a change would reread without it.
fn tuning(value: Text, t: &Task, env: &Environment, last: Option<&Ended>) -> Value {
    let warm = last.and_then(|ended| {
        let u = ended.usage?;
        Some(json!({
            "until": ended.at.at.0 + harness_provider::request::CACHED_FOR_MS,
            "tokens": u.input + u.cache_read + u.cache_write_short + u.cache_write_long + u.output,
        }))
    });
    json!({
        "model": t.tuning.model(env),
        "effort": t.tuning.effort.or_else(|| harness_provider::effort(&value(env.params)?)),
        "warm": warm,
    })
}

/// Every tab's rows, as of the state: each task, each spawn's scope, Max's
/// queue, the harness as a whole, and his drafts that can still be sent or
/// were held. `models` are the ones on offer, newest first.
pub fn rows(
    state: &State,
    value: Text,
    models: &[Model],
    source: &str,
    now: Timestamp,
) -> BTreeMap<FaceKey, FaceRow> {
    let mut rows = BTreeMap::new();
    // Each task's newest request: what an interrupt names.
    let mut turns = BTreeMap::new();
    for (id, record) in state.model_calls() {
        let newest = turns.entry(record.call.task).or_insert(id);
        *newest = (*newest).max(id);
    }
    for t in state.tasks() {
        let mut row = task(state, value, t, state.last_turn(t.id), now);
        row["turn"] = json!(turns.get(&t.id).map(|id| id.to_string()));
        rows.insert(FaceKey::Task(t.id), FaceRow::Task(row));
    }
    let queue: Vec<Value> = state
        .queue()
        .iter()
        .map(|q| {
            json!({
                "task": q.task.to_string(),
                "since": q.since.0,
                "why": q.why,
            })
        })
        .collect();
    rows.insert(FaceKey::Queue, FaceRow::Queue(Value::Array(queue)));
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
                "effort": value(e.params).and_then(|p| harness_provider::effort(&p)),
                "places": e.places,
            })
        })
        .collect();
    let provider = state
        .limbs()
        .find(|l| l.kind == LimbKind::Provider)
        .map(|l| l.id);
    let harness = json!({
        "environments": environments,
        "models": harness_provider::current(models)
            .iter()
            .map(|m| json!({"id": m.id, "name": m.name}))
            .collect::<Vec<_>>(),
        "efforts": Effort::ALL,
        "workspaces": workspaces,
        "launch": state.launch().map(|(_, l)| l),
        "probation": state.probation().verdict(),
        "source": source,
        "relaunch": relaunch(state, value),
        "superseded": state
            .limbs()
            .filter(|l| state.superseded(l.id).is_some())
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        "limited": state.limited(now).map(|t| t.0),
        "provider": provider.map(|l| l.to_string()),
        "usage": usage(state, value, provider),
    });
    rows.insert(FaceKey::Harness, FaceRow::Harness(harness));
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
    for id in spawns {
        let Some(record) = state.call(id) else {
            continue;
        };
        let (task, background) = match record.call.caller {
            Caller::Agent { task, .. } => (task, false),
            Caller::Background { task, .. } => (task, true),
            Caller::Max { .. } | Caller::Meta { .. } => continue,
        };
        let scope = json!({
            "owner": task.to_string(),
            "background": background,
            "made": record.made.seq.n,
            "settled": record.settled.is_some(),
        });
        rows.insert(FaceKey::Scope(id), FaceRow::Scope(scope));
    }
    // A sent draft is a delivery in its session; a discarded one is gone.
    // What is left can still be sent, or was held with its words kept: a
    // finished session's draft is the words that continue it.
    for d in state.drafts() {
        if d.sent.is_some() || d.discarded {
            continue;
        }
        let row = DraftRow {
            session: d.session,
            text: d.text.and_then(|id| text(value, id)),
            held: d.held.as_ref().map(|why| why.to_string()),
            from: d.from,
            at: d.at,
        };
        rows.insert(FaceKey::Draft(d.id), FaceRow::Draft(row));
    }
    rows
}

/// The subscription's usage: the newer of what the latest response's
/// headers reported and what Max's latest usage reading returned, each
/// window by name, as of when; whether a reading is running; and why the
/// last one failed, if none has succeeded since.
/// Where the latest relaunch of the brain stands, as far as it concerns the
/// build running now: `under way` while the builds limb is building and
/// rehearsing it; `took over` once it settled in this launch's span, when
/// `probation` says how far the build has proven itself; `went back` with
/// the builds limb's reason, the content of its change of the service it
/// relaunched, when that change came after this launch. `null` when the
/// running build did not come from a relaunch.
fn relaunch(state: &State, value: Text) -> Value {
    let builds = |limb: Option<LimbId>| {
        limb.and_then(|l| state.limb(l))
            .is_some_and(|l| matches!(&l.kind, LimbKind::Plugin { kind, .. } if kind == "builds"))
    };
    let Some((_, call)) = state
        .calls()
        .filter(|(_, c)| c.call.tool == "relaunch" && builds(c.limb))
        .last()
    else {
        return Value::Null;
    };
    let Some(settled) = &call.settled else {
        return json!({"stage": "under way"});
    };
    let CallOutcome::Completed(output) = &settled.outcome else {
        return Value::Null;
    };
    let since = state.launch().map(|(mark, _)| mark.seq);
    let back = output
        .observes
        .iter()
        .filter_map(|o| state.resource(&o.resource))
        .find(|c| Some(c.at.seq) > since)
        .and_then(|c| c.content);
    match back {
        Some(why) => json!({"stage": "went back", "why": text(value, why)}),
        None if Some(settled.at.seq) > since => json!({"stage": "took over"}),
        None => Value::Null,
    }
}

fn usage(state: &State, value: Text, provider: Option<LimbId>) -> Value {
    let window = |w: &Window| json!({"used": w.used, "resets": w.resets.map(|t| t.0)});
    let mut at = None;
    let mut windows = serde_json::Map::new();
    let mut on_plan = true;
    if let Some(ended) = state.quota()
        && let Some(quota) = &ended.quota
    {
        at = Some(ended.at.at);
        on_plan = quota.on_plan;
        for (name, w) in [
            ("five_hour", &quota.five_hour),
            ("seven_day", &quota.seven_day),
        ] {
            if let Some(w) = w {
                windows.insert(name.into(), window(w));
            }
        }
    }
    let (mut refreshing, mut failed) = (false, None);
    let readings = state.calls().map(|(_, c)| c).filter(|c| {
        provider.is_some_and(|p| c.limb == Some(p)) && matches!(c.call.caller, Caller::Max { .. })
    });
    for call in readings {
        let Some(settled) = &call.settled else {
            refreshing = true;
            continue;
        };
        let output = settled.outcome.output();
        let said = || {
            output
                .and_then(|o| text(value, o.model))
                .map(|t| visible(&t))
        };
        match &settled.outcome {
            CallOutcome::Completed(_) => {
                failed = None;
                let reading = output
                    .and_then(|o| value(o.full?))
                    .and_then(|b| serde_json::from_slice::<BTreeMap<String, Window>>(&b).ok());
                if let Some(reading) = reading
                    && at.is_none_or(|at| at.0 < settled.at.at.0)
                {
                    at = Some(settled.at.at);
                    windows = reading
                        .iter()
                        .map(|(n, w)| (n.clone(), window(w)))
                        .collect();
                }
            }
            _ => failed = Some(said().unwrap_or_else(|| "it did not finish".into())),
        }
    }
    json!({
        "at": at.map(|t| t.0),
        "windows": windows,
        "on_plan": on_plan,
        "refreshing": refreshing,
        "failed": failed,
    })
}

/// The text of a content array's text blocks.
fn visible(content: &str) -> String {
    serde_json::from_str::<Vec<Value>>(content)
        .unwrap_or_default()
        .iter()
        .filter_map(|b| b["text"].as_str())
        .collect()
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
        Ending::Responded { .. } | Ending::Cut { .. } | Ending::Lost => Value::Null,
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
        Some(Outcome::Abandoned { by, report }) => {
            json!({"kind": "abandoned", "by": format!("{by:?}"), "report": report.is_some()})
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
        "created": {"seq": t.created.seq.n, "at": t.created.at.0},
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
        // When its last request ended: a retry can end where it began.
        "turn": last.map(|e| e.at.seq.n),
        "environment": state.environment(t.environment).map(|e| e.name.clone()),
        "window": state.environment(t.environment).map(|e| harness_provider::context_window(t.tuning.model(e))),
        "tuning": state.environment(t.environment).map(|e| tuning(value, t, e, last)),
        "places": t.places.iter().map(|(place, limb)| (place.clone(), state.limb(*limb).map_or_else(|| limb.to_string(), |l| l.name.clone()))).collect::<std::collections::BTreeMap<String, String>>(),
        // Whether Max has paused it, how, and on which task: itself or an
        // ancestor.
        "pause": state.pause(t.id).map(|(how, by)| json!({
            "how": match how {
                Pause::Park => "park",
                Pause::Now | Pause::Off => "now",
            },
            "by": by.to_string(),
        })),
        "scoped": scoped,
        "draining": t.drain().map(|d| match d {
            Draining::ToMax => "to Max".to_string(),
            Draining::Abandoning(s) => format!("abandon ({:?})", s.by),
            Draining::Failing(reason) => format!("fail: {reason}"),
        }),
        // How far through its stop it is, and by when it must have exited.
        "stop": match t.drain() {
            Some(Draining::Abandoning(s)) => json!({
                "by": format!("{:?}", s.by),
                "deadline": s.deadline.0,
                "tidying": s.tidying,
                "hard": s.hard,
            }),
            _ => Value::Null,
        },
        "failures": t.live().map_or(0, |l| l.failures),
        "trouble": last.map_or(Value::Null, trouble),
        "pending": pending_words(state, value, t),
        "background": t.background.iter().map(|c| call(state, value, *c)).collect::<Vec<_>>(),
        // What it passed to Max that still runs: his now.
        "handed": state
            .handed()
            .filter(|(_, r)| r.passed.last().is_some_and(|h| h.by == t.id))
            .map(|(c, _)| call(state, value, c))
            .collect::<Vec<_>>(),
        "tokens": last.and_then(|e| e.usage).map(|u| json!({
            "context": u.input + u.cache_read + u.cache_write_short + u.cache_write_long,
            "output": u.output,
        })),
        "outcome": outcome,
        "waiting": state.waiting_on(t.id, now).map(|w| waiting(state, value, &w)),
    })
}

/// What was delivered to the task that no request has rendered yet: Max's
/// words the agent has not read, and a standard prompt (his Done, a stop).
fn pending_words(state: &State, value: Text, t: &Task) -> Vec<Value> {
    t.pending
        .iter()
        .filter_map(|p| match p {
            Input::Delivery(id) => state.delivery(*id),
            _ => None,
        })
        .map(|d| match d.delivery.content {
            Content::Words(words) => json!({
                "at": d.at.at.0,
                "content": text(value, words),
                "read": format!("{:?}", d.delivery.read),
            }),
            Content::Prompt(prompt) => json!({
                "at": d.at.at.0,
                "prompt": format!("{prompt:?}"),
                "read": format!("{:?}", d.delivery.read),
            }),
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
            "limbs": limbs.iter().map(|(l, _)| limb_name(state, *l)).collect::<Vec<_>>(),
            "failed": limbs.iter().filter_map(|(l, why)| Some(json!({"limb": limb_name(state, *l), "why": why.as_ref()?}))).collect::<Vec<_>>(),
        }),
        Waiting::Brain { since } => json!({"kind": "harness", "since": since.0}),
        Waiting::Paused { since } => json!({"kind": "held", "since": since.0}),
        Waiting::Parked { since } => json!({"kind": "parked", "since": since.0}),
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
        Waiting::Max { since } => json!({
            "kind": "max",
            "since": since.0,
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
        "name": match &record.call.caller {
            Caller::Background { name, .. } => Some(name),
            _ => None,
        },
        "owner": match record.owner {
            Owner::Task(task) => task.to_string(),
            Owner::Max => "max".into(),
        },
        "passed": record.passed.last().map(|h| json!({"by": h.by.to_string(), "why": h.why})),
        "abandoned": record.abandoned,
    })
}

/// One line saying what a record did, naming tasks as the state knows them.
pub fn describe(state: &State, fact: &Fact) -> String {
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
                match &c.call.caller {
                    Caller::Background { name, .. } if !name.is_empty() => {
                        format!("{by}'s {} `{name}`", c.call.tool)
                    }
                    _ => format!("{by}'s {}", c.call.tool),
                }
            },
        )
    };
    match fact {
        Fact::Launched(l) => match &l.source {
            Some(source) => format!("brain launched, {source}"),
            None => format!("brain launched, build {:.12}", l.build.to_string()),
        },
        Fact::Shutdown { within } => format!("brain shutting down, within {}s", within / 1000),
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
            TaskEvent::Tuned(tuning) => format!(
                "{} tuned to {} at {} effort",
                name(*id),
                tuning.model.as_deref().unwrap_or("its role's model"),
                tuning.effort.map_or("its role's", |e| e.as_str()),
            ),
            TaskEvent::Moved(Move::Root(start)) => {
                format!("{} has a new root, on {}", name(*id), start.root.model)
            }
            TaskEvent::Moved(Move::Compact(c)) => {
                let by = if c.by.is_some() {
                    ""
                } else {
                    " (by the harness)"
                };
                let onto = match &c.onto {
                    Onto::Checkpoint(_) => "onto its latest checkpoint".to_string(),
                    Onto::Fresh(start) => format!("onto a new root, on {}", start.root.model),
                };
                format!("{} compacted {onto}{by}", name(*id))
            }
            TaskEvent::Moved(Move::Back { to }) => format!("{} moved back to {to}", name(*id)),
            TaskEvent::Drain(Drain::ToMax { .. }) => format!("{} interrupted", name(*id)),
            TaskEvent::Drain(Drain::Abandon { by, within: 0 }) => {
                format!("{} abandoned by {by:?}, now", name(*id))
            }
            TaskEvent::Drain(Drain::Abandon { by, within }) => format!(
                "{} abandoned by {by:?}, to exit within {}s",
                name(*id),
                within / 1000
            ),
            TaskEvent::Expired => format!("{} is past its deadline: stopped hard", name(*id)),
            TaskEvent::Drain(Drain::Fail(why)) => format!("{} failing: {why}", name(*id)),
            TaskEvent::Pause(Pause::Now) => format!("{} paused", name(*id)),
            TaskEvent::Pause(Pause::Park) => format!("{} paused for now", name(*id)),
            TaskEvent::Pause(Pause::Off) => format!("{} resumed", name(*id)),
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
                        Ending::Cut { .. } => format!("{task}: response cut"),
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
                E::Stopping { within } => {
                    format!("{} stopping, within {}s", call_name(*id), within / 1000)
                }
                E::Passed { to, why } => {
                    let to = match to {
                        Owner::Task(task) => name(*task),
                        Owner::Max => "Max".into(),
                    };
                    format!("{} passed to {to}: {why}", call_name(*id))
                }
                E::Abandon => format!("{} abandoned", call_name(*id)),
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
                Reach::Connected {
                    build: Some(build), ..
                } => format!("{limb} connected, build {:.12}", build.to_string()),
                Reach::Connected { build: None, .. } => format!("{limb} connected"),
                Reach::Lost => format!("{limb} lost"),
                Reach::Failed { why } => {
                    format!("{limb} failed to start: {why}")
                }
            }
        }
        Fact::Ranking(r) => format!("queue ranked: {} placed", r.order.len()),
        Fact::Focus(_) => "a face changed focus".into(),
        Fact::Face(replica) => format!("tab {replica} is Max's"),
        Fact::Seen(at) => format!("as seen at {at}"),
        Fact::Draft(id, event) => {
            use harness_core::draft::DraftEvent as E;
            match event {
                E::Opened { session, at: None } => {
                    format!("draft {id} opened for {}", name(*session))
                }
                E::Opened {
                    session,
                    at: Some(at),
                } => format!(
                    "draft {id} opened for {}, to edit after {at}",
                    name(*session)
                ),
                E::Text(_) => format!("draft {id} edited"),
                E::Send { .. } => format!("draft {id} sent"),
                E::Discarded => format!("draft {id} discarded"),
            }
        }
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

/// A limb's name, or its id if it has none here.
fn limb_name(state: &State, limb: LimbId) -> String {
    state
        .limb(limb)
        .map_or_else(|| limb.to_string(), |l| l.name.clone())
}
