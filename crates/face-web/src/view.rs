//! What a tab is shown: the folded state as one row per object, and one
//! line per fact for the brain's log.
//!
//! Ids are sent as strings: JSON numbers cannot hold them.

use std::collections::BTreeMap;

use harness_core::call::{Caller, Outcome as CallOutcome, Owner};
use harness_core::delivery::Content;
use harness_core::fact::Fact;
use harness_core::id::{CallId, LimbId, TaskId, Timestamp, ToolUseId, ValueId};
use harness_core::limb::{Defined, Host, Prepared, Reach};
use harness_core::model_call::{Ending, ModelCallEvent, Window};
use harness_core::role::{Effort, Role};
use harness_core::state::{Ended, Hold, State, Waiting};
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

/// The text of a mandate, which may be one of Max's messages as he sent
/// it: its text blocks, a blank line apart.
fn plain(mandate: &str) -> String {
    match serde_json::from_str::<Vec<Value>>(mandate) {
        Ok(blocks) => (blocks.iter())
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        Err(_) => mandate.to_owned(),
    }
}

/// The model and effort a task's next request runs at, and while its cache
/// is warm, until when and how many tokens a change would reread without it.
fn tuning(value: Text, t: &Task, role: &Role, last: Option<&Ended>) -> Value {
    let warm = last.and_then(|ended| {
        let u = ended.usage?;
        Some(json!({
            "until": ended.at.at.0 + harness_provider::request::CACHED_FOR_MS,
            "tokens": u.input + u.cache_read + u.cache_write_short + u.cache_write_long + u.output,
        }))
    });
    json!({
        "model": t.tuning.model(role),
        "effort": t.tuning.effort.or_else(|| harness_provider::effort(&value(role.params)?)),
        "warm": warm,
    })
}

/// Every tab's rows, as of the state: each task, Max's queue, the harness
/// as a whole, and his drafts that can still be sent or were held.
/// `models` are the ones on offer, newest first.
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
    // The directories tasks work in, each once: what a new task is offered.
    let mut workspaces: Vec<Value> = Vec::new();
    for workdir in state.tasks().filter_map(|t| state.workdir(t)) {
        let row = json!({
            "limb": workdir.limb.to_string(),
            "dir": workdir.dir,
        });
        if !workspaces.contains(&row) {
            workspaces.push(row);
        }
    }
    let roles: Vec<Value> = state
        .roles()
        .map(|r| {
            json!({
                "id": r.id.to_string(),
                "name": r.name,
                "model": r.model,
                "effort": value(r.params).and_then(|p| harness_provider::effort(&p)),
                "workdir": r.works_in_a_directory(),
            })
        })
        .collect();
    let brain = state.brain().map(|l| l.id);
    let harness = json!({
        "roles": roles,
        "models": harness_provider::current(models)
            .iter()
            .map(|m| json!({"id": m.id, "name": m.name}))
            .collect::<Vec<_>>(),
        "efforts": Effort::ALL,
        "workspaces": workspaces,
        // Where a new task's directory is: the limb on the brain's machine.
        "local": state.limbs().find(|l| l.host == Host::Local).map(|l| l.id.to_string()),
        "launch": state.launch().and_then(|(_, l)| l),
        "probation": state.probation().verdict(),
        "source": source,
        "relaunch": relaunch(state, value),
        "superseded": state
            .limbs()
            .filter(|l| state.superseded(l.id).is_some())
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        "limited": state.limited(now).map(|t| t.0),
        "signedOut": match state.hold(now) {
            Some(Hold::SignedOut { since }) => Some(since.0),
            _ => None,
        },
        "brain": brain.map(|l| l.to_string()),
        "usage": usage(state, value, brain),
    });
    rows.insert(FaceKey::Harness, FaceRow::Harness(harness));
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

/// Where the latest relaunch of the brain stands, as far as it concerns the
/// build running now: `under way` while the `builds` set is building and
/// rehearsing it; `took over` once it settled in this launch's span, when
/// `probation` says how far the build has proven itself; `went back` with
/// the `builds` set's reason, the content of its change of the service it
/// relaunched, when that change came after this launch. `null` when the
/// running build did not come from a relaunch.
fn relaunch(state: &State, value: Text) -> Value {
    let builds = |limb: Option<LimbId>| {
        (limb.and_then(|l| state.resolve(l)))
            .and_then(|l| state.limb(l))
            .is_some_and(|l| l.offer("builds").is_some())
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

/// The subscription's usage: the newer of what the latest response's
/// headers reported and what Max's latest usage reading returned, each
/// window by name, as of when; whether a reading is running; and why the
/// last one failed, if none has succeeded since.
fn usage(state: &State, value: Text, brain: Option<LimbId>) -> Value {
    let window = |w: &Window| json!({"used": w.used, "resets": w.resets.map(|t| t.0)});
    let mut at = None;
    let mut windows = serde_json::Map::new();
    if let Some(ended) = state.quota()
        && let Some(quota) = &ended.quota
    {
        at = Some(ended.at.at);
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
        c.limb
            .is_some_and(|l| brain.is_some() && state.resolve(l) == brain)
            && matches!(c.call.caller, Caller::Max { .. })
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
        Ending::SignedOut => json!({
            "at": ended.at.at.0,
            "text": "the harness is not signed in to claude.ai",
        }),
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
        "mandate": text(value, t.mandate.text).map(|m| plain(&m)),
        "head": t.head().map(|h| h.to_string()),
        "compactions": t.head().map_or(0, |h| state.compactions_on(t.id, h)),
        // When its last request ended: a retry can end where it began.
        "ended": last.map(|e| e.at.seq.n),
        "role": state.role(t.role).map(|r| r.name.clone()),
        "window": state.role(t.role).map(|r| harness_provider::context_window(t.tuning.model(r))),
        "tuning": state.role(t.role).map(|r| tuning(value, t, r, last)),
        "workdir": state.workdir(t).map(|w| w.dir),
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
        // Whether its abandon is past its deadline, or had none: stopped hard.
        "stop": match t.drain() {
            Some(Draining::Abandoning(s)) => json!({"hard": s.hard}),
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
            }),
            Content::Prompt(prompt) => json!({
                "at": d.at.at.0,
                "prompt": format!("{prompt:?}"),
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
        Waiting::Held {
            hold: Hold::Limited { until },
            since,
        } => json!({"kind": "limited", "until": until.0, "since": since.0}),
        Waiting::Held {
            hold: Hold::SignedOut { .. },
            since,
        } => json!({"kind": "signedOut", "since": since.0}),
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
        Fact::SignedIn => "signed in to claude.ai: work carries on".into(),
        Fact::Task(id, event) => match event {
            TaskEvent::Created(c) => {
                let how = match &c.origin {
                    Origin::Fresh { role, .. } => state
                        .role(*role)
                        .map_or_else(|| role.to_string(), |r| r.name.clone()),
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
                        Ending::SignedOut => format!("{task}: waits for a sign-in to claude.ai"),
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
        Fact::Role(r) => format!("role {} defined", r.name),
        Fact::Limb(Defined::Limb(l)) => format!("limb {} defined", l.name),
        Fact::Limb(Defined::Legacy(l)) => format!("limb {} defined, as before", l.name),
        Fact::Described { limb, description } => {
            let limb = limb_name(state, *limb);
            let tools: usize = description.sets.iter().map(|s| s.tools.len()).sum();
            format!(
                "{limb} described: {} sets, {tools} tools",
                description.sets.len()
            )
        }
        Fact::Workdir {
            limb,
            dir,
            prepared,
        } => {
            let limb = limb_name(state, *limb);
            match prepared {
                Prepared::Ready { .. } => format!("{limb} has {} ready", dir.display()),
                Prepared::Failed { why } => {
                    format!("{limb} cannot make {}: {why}", dir.display())
                }
            }
        }
        Fact::Reach { limb, reach } => {
            let limb = limb_name(state, *limb);
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

/// A limb's name, of now or from before, or its id if it has none here.
fn limb_name(state: &State, limb: LimbId) -> String {
    (state.limb(limb).map(|l| &l.name))
        .or(state.legacy(limb).map(|l| &l.name))
        .map_or_else(|| limb.to_string(), String::clone)
}
