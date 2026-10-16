//! What a request renders, and the wording of everything the harness says to
//! a model. Policy: the core fixes what must be rendered (every result of
//! the last round) and leaves the rest here.

use harness_core::call::{Caller, Outcome};
use harness_core::context::{Entry, Note, Rendered, Role, Root, Wire};
use harness_core::delivery::Content;
use harness_core::environment::{Description, Route};
use harness_core::id::{CallId, LimbId};
use harness_core::resource::Observation;
use harness_core::state::State;
use harness_core::task::{Giver, Input, Origin, Scope, Task};
use harness_provider::IDENTITY;
use serde_json::{Value, json};

use crate::policy::{self, Kind};

/// A new root for an agent kind over an environment's limbs: the identity,
/// the role, then each limb's instructions; every limb's tools. With the
/// routes that bind its tool names and the versions it embodies.
pub fn root(kind: &Kind, limbs: &[(LimbId, &Description)]) -> (Root, Vec<Route>, Vec<Observation>) {
    let mut system = vec![text(IDENTITY), text(kind.role)];
    let mut tools = Vec::new();
    let mut routes = Vec::new();
    let mut observes = Vec::new();
    for (limb, description) in limbs {
        for (instructions, observed) in &description.instructions {
            system.push(instructions.0.clone());
            observes.push(observed.clone());
        }
        for tool in &description.tools {
            tools.push(tool.definition.0.clone());
            routes.push(Route {
                tool: tool.name.clone(),
                limb: *limb,
            });
        }
    }
    let root = Root {
        model: policy::MODEL.into(),
        system: Wire(format!("[{}]", system.join(","))),
        tools: Wire(format!("[{}]", tools.join(","))),
    };
    (root, routes, observes)
}

/// A shared brief, as the first message of every fresh member that is given
/// it: the same bytes, so the same entry and one cache prefix.
pub fn brief(text_: &str) -> Wire {
    Wire(format!("[{}]", text(text_)))
}

/// The next request's new content: what it renders, and the blocks of the
/// user message that carries them (empty when there is nothing new).
pub struct Turn {
    pub renders: Vec<Rendered>,
    pub blocks: Vec<String>,
}

/// Everything the task's next request includes, in the order the provider
/// needs: tool results first, then text.
pub fn turn(
    state: &State,
    task: &Task,
    first: bool,
    head: &Entry,
    response_calls: &[CallId],
) -> Turn {
    let mut turn = Turn {
        renders: Vec::new(),
        blocks: Vec::new(),
    };
    let mut texts = Vec::new();

    // A fork's first message answers every tool use of the forked response
    // with a neutral result, and gives its mandate as user text.
    if first && let Origin::Fork { .. } = task.origin {
        for call in response_calls {
            turn.renders
                .push(Rendered::Note(Note::Elsewhere { call: *call }));
            turn.blocks.push(elsewhere(state, *call));
        }
    }
    for pending in &task.pending {
        match pending.input {
            Input::Result(call) => {
                turn.renders.push(Rendered::Call(call));
                turn.blocks.push(result(state, call));
            }
            Input::Activity(call) => {
                turn.renders.push(Rendered::Call(call));
                texts.push(activity(state, call));
            }
            Input::Delivery(id) => {
                turn.renders.push(Rendered::Delivery(id));
                if let Some(delivered) = state.delivery(id) {
                    texts.extend(delivery(&delivered.delivery.content));
                }
            }
        }
    }

    let stale: Vec<_> = state
        .stale(task.id)
        .into_iter()
        .filter(|s| !observed_now(state, &turn.renders, s))
        .collect();
    if !stale.is_empty() {
        let names: Vec<_> = stale.iter().map(|s| s.resource.name.as_str()).collect();
        texts.push(text(&format!(
            "[harness] Changed since you last saw them: {}. Read them again before relying on what you saw.",
            names.join(", ")
        )));
        let told = stale
            .iter()
            .map(|s| Observation {
                resource: s.resource.clone(),
                version: s.now,
            })
            .collect();
        turn.renders.push(Rendered::Note(Note::Stale { told }));
    }

    if first {
        turn.renders.push(Rendered::Note(Note::Mandate));
        texts.push(text(&match task.scope {
            Scope::Max => task.mandate.text.clone(),
            Scope::Spawn(_) => mandate(state, task),
        }));
    }

    if turn.blocks.is_empty()
        && texts.is_empty()
        && matches!(head, Entry::Message(m) if m.role == Role::Assistant)
    {
        // The last response stopped short of ending its turn.
        turn.renders.push(Rendered::Note(Note::Continue));
        texts.push(text("[harness] Your response was cut off. Carry on."));
    }
    turn.blocks.extend(texts);
    turn
}

/// Whether a stale resource is one a call this request renders observed: a
/// task's own write is not news to it.
fn observed_now(state: &State, renders: &[Rendered], stale: &harness_core::state::Stale) -> bool {
    renders.iter().any(|r| {
        let Rendered::Call(call) = r else {
            return false;
        };
        let Some(record) = state.call(*call) else {
            return false;
        };
        let harness_core::call::Status::Settled { outcome, .. } = &record.status else {
            return false;
        };
        match outcome {
            Outcome::Completed(o) | Outcome::Failed(o) => o
                .observes
                .iter()
                .any(|ob| ob.resource == stale.resource && ob.version == stale.now),
            _ => false,
        }
    })
}

/// A tool result for one of the task's own tool uses.
fn result(state: &State, call: CallId) -> String {
    let record = state.call(call).expect("a pending result is a known call");
    let Caller::Agent {
        tool_use: Some(tool_use),
        ..
    } = &record.call.caller
    else {
        unreachable!("a round's result answers a tool use")
    };
    let harness_core::call::Status::Settled { outcome, .. } = &record.status else {
        unreachable!("a pending result is settled")
    };
    let (content, error) = outcome_content(outcome);
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use.0,
        "content": content,
        "is_error": error,
    })
    .to_string()
}

/// A tool result's content, and whether it is an error.
fn outcome_content(outcome: &Outcome) -> (Value, bool) {
    let blocks = |wire: &Wire| serde_json::from_str::<Value>(&wire.0).unwrap_or(json!([]));
    let said = |s: &str| json!([{ "type": "text", "text": s }]);
    match outcome {
        Outcome::Completed(output) => (blocks(&output.model), false),
        Outcome::Failed(output) => (blocks(&output.model), true),
        Outcome::Refused { current } => {
            let names: Vec<_> = current.iter().map(|o| o.resource.name.as_str()).collect();
            (
                said(&format!(
                    "Refused, with no effect: {} changed since you last saw it. Read it again, then retry.",
                    names.join(", ")
                )),
                true,
            )
        }
        Outcome::Interrupted { by, partial } => {
            let mut content = partial.as_ref().map_or(json!([]), |o| blocks(&o.model));
            if let Value::Array(items) = &mut content {
                items.push(json!({ "type": "text", "text": format!(
                    "[interrupted ({by:?}); it may have had partial effects]"
                ) }));
            }
            (content, true)
        }
        Outcome::Unknown => (
            said(
                "The outcome of this call is unknown: the harness stopped while it was running, and this tool is \
                 not safe to run again blind. It may or may not have had its effects. Check before relying on either.",
            ),
            true,
        ),
    }
}

/// A call outside the round that has settled: in practice, the late outcome
/// of a call earlier said to be unknown.
fn activity(state: &State, call: CallId) -> String {
    let Some(record) = state.call(call) else {
        return text("[harness] A call you made has settled.");
    };
    let harness_core::call::Status::Settled { outcome, late } = &record.status else {
        return text("[harness] A call you made has settled.");
    };
    let (content, _) = outcome_content(late.as_ref().unwrap_or(outcome));
    let said: Vec<_> = content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect();
    text(&format!(
        "[harness] Your earlier `{}` call, whose outcome was unknown, did finish:\n{}",
        record.call.tool,
        said.join("\n")
    ))
}

fn delivery(content: &Content) -> Vec<String> {
    match content {
        Content::Words(words) => serde_json::from_str::<Vec<Value>>(&words.0)
            .unwrap_or_default()
            .iter()
            .map(Value::to_string)
            .collect(),
        Content::Prompt(prompt) => vec![text(&format!("[harness] {prompt:?}"))],
        Content::Event { event, .. } => vec![text(&format!("[event] {}", event.0))],
        Content::Aside(_) => vec![text("[harness] Max talked to a fork of you meanwhile.")],
    }
}

/// The neutral answer, in a fork, to one tool use of the forked response.
fn elsewhere(state: &State, call: CallId) -> String {
    let record = state.call(call).expect("a forked response's call is known");
    let Caller::Agent {
        tool_use: Some(tool_use),
        ..
    } = &record.call.caller
    else {
        unreachable!("a response's call answers a tool use")
    };
    let said = if record.call.tool == crate::meta::SPAWN {
        let names = crate::meta::names(&record.call.input);
        format!("Started {}.", names.join(", "))
    } else {
        "Done by the agent this context was forked from; the result went to it.".into()
    };
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use.0,
        "content": said,
    })
    .to_string()
}

/// A spawned task's mandate, as user text: its place in the work, its part,
/// whose its siblings' parts are, and when to stop.
fn mandate(state: &State, task: &Task) -> String {
    let owner = state.owner(task.id).and_then(|o| state.task(o));
    let owner_name = owner.map_or("your owner", |o| o.name.as_str());
    let siblings: Vec<_> = state
        .members(task.scope)
        .filter(|t| t.id != task.id)
        .map(|t| t.name.as_str())
        .collect();
    let mut said = match task.origin {
        Origin::Fork { .. } => format!(
            "You are now `{}`, a fork of `{owner_name}`. Everything above is `{owner_name}`'s context; \
             from here you do one part of its work, and only that part.\n\nYour part:\n{}",
            task.name, task.mandate.text
        ),
        _ => format!(
            "You are `{}`, started by `{owner_name}`.\n\nYour task:\n{}",
            task.name, task.mandate.text
        ),
    };
    if let (Origin::Fork { .. }, Some(owner)) = (task.origin, owner) {
        said.push_str(&format!(
            "\n\n`{owner_name}`'s own mandate, for context only:\n{}",
            owner.mandate.text
        ));
        if let Giver::Task(_) = owner.mandate.from
            && let Some(grand) = state.owner(owner.id).and_then(|g| state.task(g))
        {
            said.push_str(&format!(" (given by `{}`)", grand.name));
        }
    }
    if !siblings.is_empty() {
        said.push_str(&format!(
            "\n\nThe other parts ({}) are being done by other agents at the same time. They are not yours: \
             do not do them or start them again. `{owner_name}` combines the results after every part has returned.",
            siblings.join(", ")
        ));
    }
    said.push_str(&format!(
        "\n\nWhen your part is done, end your turn. Your last message is your result, and it goes to `{owner_name}`."
    ));
    said
}

/// A text content block.
pub fn text(text: &str) -> String {
    json!({ "type": "text", "text": text }).to_string()
}

/// The visible text of an assistant entry's content.
pub fn visible(content: &Wire) -> String {
    serde_json::from_str::<Vec<Value>>(&content.0)
        .unwrap_or_default()
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
