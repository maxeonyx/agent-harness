//! What a request renders, and the wording of everything the harness says to
//! a model. Policy: the core fixes what must be rendered (every result of
//! the last round) and leaves the rest here.

use harness_core::call::{Caller, Outcome};
use harness_core::context::{Entry, Note, Rendered, Role, Root};
use harness_core::delivery::Content;
use harness_core::environment::{Binding, Description};
use harness_core::id::{CallId, LimbId, ValueId};
use harness_core::resource::Observation;
use harness_core::state::{Stale, State};
use harness_core::task::{Giver, Input, Origin, Scope, Task};
use harness_core::value::Bytes;
use harness_provider::IDENTITY;
use serde_json::{Value, json};

/// The bytes of a stored value.
pub type Values<'a> = &'a dyn Fn(ValueId) -> Vec<u8>;

/// A new root in an environment: the identity, the role, then each limb's
/// instructions in `system`; the tools its binding routes.
pub struct NewRoot {
    pub root: Root,
    pub binding: Binding,
    /// The versions the root embodies.
    pub observes: Vec<Observation>,
    /// The bytes of `root.system` and `root.tools`.
    pub values: Vec<Bytes>,
}

pub fn root(model: &str, role: &str, limbs: &[(LimbId, &Description)]) -> Result<NewRoot, String> {
    let mut system = vec![text(IDENTITY), text(role)];
    let mut observes = Vec::new();
    for (_, description) in limbs {
        for instruction in &description.instructions {
            system.push(text(&instruction.text));
            observes.push(instruction.observes.clone());
        }
    }
    let (binding, tools) =
        Binding::new(limbs).map_err(|tool| format!("two limbs offer the tool `{tool}`"))?;
    let system = blocks(&system);
    let tools = Bytes::from(tools);
    Ok(NewRoot {
        root: Root {
            model: model.into(),
            system: system.id(),
            tools: tools.id(),
        },
        binding,
        observes,
        values: vec![system, tools],
    })
}

/// What every fresh member of a spawn shares, as its first message: the
/// same bytes, so the same entry and one cache prefix.
pub fn brief(said: &str) -> Bytes {
    blocks(&[text(&format!(
        "[harness] The brief every agent started with you was given:\n\n{said}"
    ))])
}

/// The next request's new content: what it renders, and the blocks of the
/// user message that carries them (empty when there is nothing new).
pub struct Turn {
    pub renders: Vec<Rendered>,
    pub blocks: Vec<String>,
}

/// Everything the task's next request includes, in the order the provider
/// needs: tool results first, then text. `forked` are the calls of the
/// response a fork's first request answers.
pub fn turn(state: &State, values: Values, task: &Task, first: bool, forked: &[CallId]) -> Turn {
    let mut turn = Turn {
        renders: Vec::new(),
        blocks: Vec::new(),
    };
    let mut texts = Vec::new();

    // A fork's first message answers every tool use of the forked response
    // with a neutral result, and gives its mandate as user text.
    for call in forked {
        turn.renders
            .push(Rendered::Note(Note::Elsewhere { call: *call }));
        turn.blocks.push(elsewhere(state, values, *call));
    }
    for pending in &task.pending {
        match pending.input {
            Input::Result(call) => {
                turn.renders.push(Rendered::Call(call));
                turn.blocks.push(result(state, values, call));
            }
            Input::Activity(call) => {
                turn.renders.push(Rendered::Call(call));
                texts.push(activity(state, values, call));
            }
            Input::Delivery(id) => {
                turn.renders.push(Rendered::Delivery(id));
                if let Some(delivered) = state.delivery(id) {
                    let content = &delivered.delivery.content;
                    // Words that arrive with a tool round would read as part
                    // of it; say whose they are.
                    if matches!(content, Content::Words(_)) && !turn.blocks.is_empty() {
                        texts.push(text("[harness] Max wrote this while you were working:"));
                    }
                    texts.extend(delivery(values, content));
                }
            }
        }
    }

    let stale: Vec<_> = state
        .stale(task)
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
        turn.renders.push(Rendered::Note(Note::Stale {
            told,
            shown: Vec::new(),
        }));
    }

    if first {
        turn.renders.push(Rendered::Note(Note::Mandate));
        texts.push(text(&match task.scope {
            Scope::Max => said(values, task.mandate.text),
            Scope::Spawn(_) => mandate(state, values, task),
        }));
    }

    let after_response = matches!(
        task.head().and_then(|h| state.entry(h)),
        Some(Entry::Message { message, .. }) if message.role == Role::Assistant
    );
    if turn.blocks.is_empty() && texts.is_empty() && after_response {
        // The last response stopped short of ending its turn.
        turn.renders.push(Rendered::Note(Note::Continue));
        texts.push(text("[harness] Your response was cut off. Carry on."));
    }
    turn.blocks.extend(texts);
    turn
}

/// Whether a stale resource is one a call this request renders observed: a
/// task's own write is not news to it.
fn observed_now(state: &State, renders: &[Rendered], stale: &Stale) -> bool {
    renders.iter().any(|r| {
        let Rendered::Call(call) = r else {
            return false;
        };
        state
            .call(*call)
            .and_then(|c| c.outcome())
            .and_then(Outcome::output)
            .is_some_and(|o| {
                o.observes
                    .iter()
                    .any(|ob| ob.resource == stale.resource && ob.version == stale.now)
            })
    })
}

/// A tool result for one of the task's own tool uses.
fn result(state: &State, values: Values, call: CallId) -> String {
    let record = state.call(call).expect("a pending result is a known call");
    let Caller::Agent {
        tool_use: Some(tool_use),
        ..
    } = &record.call.caller
    else {
        unreachable!("a round's result answers a tool use")
    };
    let outcome = record.outcome().expect("a pending result is settled");
    let (content, error) = outcome_content(values, outcome);
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use.0,
        "content": content,
        "is_error": error,
    })
    .to_string()
}

/// A tool result's content, and whether it is an error.
fn outcome_content(values: Values, outcome: &Outcome) -> (Value, bool) {
    let blocks = |id: ValueId| serde_json::from_slice::<Value>(&values(id)).unwrap_or(json!([]));
    let said = |s: &str| json!([{ "type": "text", "text": s }]);
    match outcome {
        Outcome::Completed(output) => (blocks(output.model), false),
        Outcome::Failed(output) => (blocks(output.model), true),
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
        Outcome::Interrupted { partial } => {
            let mut content = partial.as_ref().map_or(json!([]), |o| blocks(o.model));
            if let Value::Array(items) = &mut content {
                items.push(json!({ "type": "text", "text": "[interrupted; it may have had partial effects]" }));
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

/// A call outside the round that has settled: the late outcome of a call
/// earlier said to be unknown, or something Max did in the chat.
fn activity(state: &State, values: Values, call: CallId) -> String {
    let Some(record) = state.call(call) else {
        return text("[harness] A call you made has settled.");
    };
    let Some(outcome) = record.outcome() else {
        return text("[harness] A call you made has settled.");
    };
    let (content, _) = outcome_content(values, outcome);
    let said: Vec<_> = content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect();
    let what = match record.call.caller {
        Caller::Max { .. } => format!("Max ran `{}` here", record.call.tool),
        _ => format!(
            "Your earlier `{}` call, whose outcome was unknown, did finish",
            record.call.tool
        ),
    };
    text(&format!("[harness] {what}:\n{}", said.join("\n")))
}

fn delivery(values: Values, content: &Content) -> Vec<String> {
    match content {
        Content::Words(words) => serde_json::from_slice::<Vec<Value>>(&values(*words))
            .unwrap_or_default()
            .iter()
            .map(Value::to_string)
            .collect(),
        Content::Prompt(prompt) => vec![text(&format!("[harness] {prompt:?}"))],
        Content::Aside(_) => vec![text("[harness] Max talked to a fork of you meanwhile.")],
    }
}

/// The neutral answer, in a fork, to one tool use of the forked response.
fn elsewhere(state: &State, values: Values, call: CallId) -> String {
    let record = state.call(call).expect("a forked response's call is known");
    let Caller::Agent {
        tool_use: Some(tool_use),
        ..
    } = &record.call.caller
    else {
        unreachable!("a response's call answers a tool use")
    };
    let tool = record.call.tool.as_str();
    let said = if tool == crate::meta::SPAWN || tool == crate::meta::ADD_SIBLING {
        let names = crate::meta::names(tool, &values(record.call.input));
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
fn mandate(state: &State, values: Values, task: &Task) -> String {
    let owner = state.owner(task.id).and_then(|o| state.task(o));
    let owner_name = owner.map_or("your owner", |o| o.name.as_str());
    let siblings: Vec<_> = state
        .members(task.scope)
        .filter(|t| t.id != task.id)
        .map(|t| t.name.as_str())
        .collect();
    let part = said(values, task.mandate.text);
    let mut said_ = match task.origin {
        Origin::Fork { .. } => format!(
            "You are now `{}`, a fork of `{owner_name}`. Everything above is `{owner_name}`'s context; \
             from here you do one part of its work, and only that part.\n\nYour part:\n{part}",
            task.name
        ),
        _ => format!(
            "You are `{}`, started by `{owner_name}`.\n\nYour task:\n{part}",
            task.name
        ),
    };
    if let (Origin::Fork { .. }, Some(owner)) = (&task.origin, owner) {
        said_.push_str(&format!(
            "\n\n`{owner_name}`'s own mandate, for context only:\n{}",
            said(values, owner.mandate.text)
        ));
        if let Giver::Task(_) = owner.mandate.from
            && let Some(grand) = state.owner(owner.id).and_then(|g| state.task(g))
        {
            said_.push_str(&format!(" (given by `{}`)", grand.name));
        }
    }
    if !siblings.is_empty() {
        said_.push_str(&format!(
            "\n\nThe other parts ({}) are being done by other agents at the same time. They are not yours: \
             do not do them or start them again. `{owner_name}` combines the results after every part has returned.",
            siblings.join(", ")
        ));
    }
    said_.push_str(&format!(
        "\n\nWhen your part is done, end your turn. Your last message is your result, and it goes to `{owner_name}`."
    ));
    said_
}

/// A stored value read as text.
pub fn said(values: Values, id: ValueId) -> String {
    String::from_utf8_lossy(&values(id)).into_owned()
}

/// A text content block.
pub fn text(text: &str) -> String {
    json!({ "type": "text", "text": text }).to_string()
}

/// A content array of these blocks, as bytes.
pub fn blocks(blocks: &[String]) -> Bytes {
    Bytes::from(format!("[{}]", blocks.join(",")))
}

/// The visible text of an assistant entry's content.
pub fn visible(content: &[u8]) -> String {
    serde_json::from_slice::<Vec<Value>>(content)
        .unwrap_or_default()
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
