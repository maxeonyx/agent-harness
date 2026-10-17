//! The meta limb: the lifecycle tools the brain itself implements, as calls.
//!
//! `spawn` opens a scope: one call whose members are forks of the caller,
//! fresh agents in the roles its environment starts, or resumes of its
//! finished children, and which settles with every member's result once
//! the last is done. `add_sibling` adds one task to the caller's own scope
//! and returns at once. `handover` moves the caller to a new root once its
//! round is over, carrying only its note. `background` makes a call to
//! one of the caller's other tools that no round waits on: its result
//! arrives as a message, and the caller cannot finish until it has. Their effects (the members' `Created` facts, with a
//! limb for each new clone) are appended in the same batch as the
//! `Dispatched`, so a crash leaves both or neither, and a restart never
//! spawns twice.

use harness_core::call::{Call, Caller};
use harness_core::delivery::Content;
use harness_core::environment::{Audience, Description, Instruction, Limb, LimbKind, Tool};
use harness_core::id::{CallId, Digest, LimbId, ValueId};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::state::State;
use harness_core::task::{Context, Task};
use harness_core::value::Bytes;
use serde_json::{Value, json};

use crate::render::{blocks, text};

pub const SPAWN: &str = "spawn";
pub const ADD_SIBLING: &str = "add_sibling";
pub const HANDOVER: &str = "handover";
pub const BACKGROUND: &str = "background";

pub fn definition(id: LimbId) -> Limb {
    Limb {
        id,
        name: "meta".into(),
        kind: LimbKind::Meta,
    }
}

/// What the meta limb tells a root in an environment that starts `roles`;
/// `member` when the task is in a scope it can add siblings to.
pub fn description(limb: LimbId, roles: &[&str], member: bool) -> Description {
    let mut said = String::from(
        "You can start other agents with `spawn`. One call starts a scope: a list of tasks that run at the same time, \
and returns when every one of them has finished, with each one's result (its last message). \
While they run you wait, so put everything that can run in parallel into one call.\n\n\
Each task is a fork of you, a fresh agent, or a resume of an agent you started before that has finished. \
A fork continues from this exact point of your context: it knows everything you know, so its mandate can be short, \
and it shares your cache, so it is cheap. It works in your directory with you. \
A resume continues that agent's own context with a new mandate, so a follow-up question keeps everything it learned. \
A fresh agent starts from nothing but its role, the shared `brief`, the `quotes` and its mandate, so say everything it needs. \
Each of Max's messages, mandates and answers is labelled `max-…`: name it in `quotes` to give his words to every fresh agent verbatim, rather than retelling them.",
    );
    if roles.is_empty() {
        said.push_str("\n\nYou cannot start fresh agents, only forks of yourself.");
    } else {
        said.push_str(&format!(
            "\n\nThe roles you can start fresh: {}. A fresh agent gets its own clone of your repository, on the \
`branch` you name, made from `base`; it commits and pushes there.",
            roles.join(", ")
        ));
    }
    said.push_str(
        "\n\n`handover` replaces your context with a fresh one that starts from your role, your mandate and the note \
you write: use it when your context has grown long, or before a new phase of the work. The note is all you keep, so \
write what the next context needs: where the work stands, what you decided and why, what is next, and Max's words verbatim.",
    );
    said.push_str(
        "\n\n`background` runs one of your other tools without waiting for it: it returns at once, the tool's result \
reaches you as a message when it settles, and you cannot finish until it has.",
    );
    if member {
        said.push_str(
            "\n\n`add_sibling` adds one task to the scope you are in, as a fork of you or in a role you can start, \
and returns at once: the agent that started you gets its result with yours.",
        );
    }
    let task = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Short and descriptive, such as `wordcount`." },
            "mandate": { "type": "string", "description": "What this agent must do, and how to check it is done." },
            "role": { "type": "string", "enum": roles, "description": "Start a fresh agent in this role. Omit, with `resume`, to fork yourself." },
            "resume": { "type": "string", "description": "The name of a finished agent you started, to continue it." },
            "branch": { "type": "string", "description": "A fresh agent's own branch, for its clone." },
            "base": { "type": "string", "description": "What its branch is made from if it does not exist yet. Default `main`." }
        },
        "required": ["name", "mandate"]
    });
    let quotes = json!({
        "type": "array",
        "items": { "type": "string" },
        "description": "Labels of Max's words (`max-…`) to give every fresh agent verbatim, after the brief."
    });
    let spawn = json!({
        "name": SPAWN,
        "description": "Start agents to do parts of your work in parallel, and wait for all of their results.",
        "input_schema": {
            "type": "object",
            "properties": {
                "tasks": { "type": "array", "minItems": 1, "items": task },
                "brief": { "type": "string", "description": "What every fresh agent in this call is given before its own mandate." },
                "quotes": quotes
            },
            "required": ["tasks"]
        }
    });
    let handover = json!({
        "name": HANDOVER,
        "description": "Continue in a fresh context that starts from your role, your mandate and this note. Call it alone, as the last thing in a turn.",
        "input_schema": {
            "type": "object",
            "properties": {
                "note": { "type": "string", "description": "Everything the next context needs to carry on." }
            },
            "required": ["note"]
        }
    });
    let background = json!({
        "name": BACKGROUND,
        "description": "Run one of your other tools without waiting for its result, which comes to you as a message.",
        "input_schema": {
            "type": "object",
            "properties": {
                "tool": { "type": "string", "description": "The tool's name." },
                "input": { "type": "object", "description": "The tool's input, as you would give it directly." }
            },
            "required": ["tool", "input"]
        }
    });
    let mut tools = vec![
        tool(SPAWN, spawn),
        tool(HANDOVER, handover),
        tool(BACKGROUND, background),
    ];
    if member {
        let sibling = json!({
            "name": ADD_SIBLING,
            "description": "Add one task to the scope you are in, and return at once.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task": task,
                    "brief": { "type": "string", "description": "What a fresh agent is given before its mandate." },
                    "quotes": quotes
                },
                "required": ["task"]
            }
        });
        tools.push(tool(ADD_SIBLING, sibling));
    }
    Description {
        instructions: vec![Instruction {
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: String::new(),
                },
                version: Version::Is(Digest::of(said.as_bytes())),
            },
            text: said,
        }],
        tools,
    }
}

fn tool(name: &str, definition: Value) -> Tool {
    Tool {
        name: name.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
    }
}

/// A handover call's note.
pub fn note(input: &[u8]) -> String {
    serde_json::from_slice::<Value>(input)
        .ok()
        .and_then(|v| v["note"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A background call's tool and its input.
pub fn background(input: &[u8]) -> Result<(String, Bytes), String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let tool = value["tool"].as_str().ok_or("`tool` must name a tool")?;
    if !value["input"].is_object() {
        return Err("`input` must be the tool's input object".into());
    }
    Ok((tool.to_owned(), Bytes::from(value["input"].to_string())))
}

/// A spawn or add_sibling call's input.
pub struct Spawn {
    pub members: Vec<Member>,
    pub brief: Option<String>,
    /// Labels of Max's words to quote.
    pub quotes: Vec<String>,
}

pub struct Member {
    pub name: String,
    pub mandate: String,
    pub how: How,
}

pub enum How {
    Fork,
    Fresh {
        role: String,
        branch: Option<String>,
        base: String,
    },
    Resume(String),
}

pub fn parse(tool: &str, input: &[u8]) -> Result<Spawn, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let tasks: Vec<&Value> = if tool == ADD_SIBLING {
        vec![&value["task"]]
    } else {
        value["tasks"]
            .as_array()
            .filter(|t| !t.is_empty())
            .ok_or("`tasks` must be a non-empty list")?
            .iter()
            .collect()
    };
    let members = tasks
        .into_iter()
        .map(|t| {
            let field = |k: &str| t[k].as_str().map(str::to_owned);
            let how = match (field("role"), field("resume")) {
                (Some(_), Some(_)) => {
                    return Err("a task has a `role` or a `resume`, not both".into());
                }
                (Some(role), None) => How::Fresh {
                    role,
                    branch: field("branch"),
                    base: field("base").unwrap_or_else(|| "main".into()),
                },
                (None, Some(from)) => How::Resume(from),
                (None, None) => How::Fork,
            };
            Ok(Member {
                name: field("name").ok_or("each task needs a `name`")?,
                mandate: field("mandate").ok_or("each task needs a `mandate`")?,
                how,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let brief = value["brief"].as_str().map(str::to_owned);
    let quotes = match &value["quotes"] {
        Value::Null => Vec::new(),
        Value::Array(labels) => labels
            .iter()
            .map(|l| l.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("`quotes` must be a list of labels")?,
        _ => return Err("`quotes` must be a list of labels".into()),
    };
    Ok(Spawn {
        members,
        brief,
        quotes,
    })
}

/// The task names a spawn's input gives, for the neutral answer a fork sees.
pub fn names(tool: &str, input: &[u8]) -> Vec<String> {
    parse(tool, input)
        .map(|s| s.members.into_iter().map(|m| m.name).collect())
        .unwrap_or_default()
}

/// A scope's result: each member's outcome under its name, as a tool
/// result's content.
pub fn result(members: &[(String, String)]) -> Bytes {
    let body: Vec<_> = members
        .iter()
        .map(|(name, said)| format!("## {name}\n\n{said}"))
        .collect();
    blocks(&[text(&body.join("\n\n"))])
}

/// What a call to `limb` expects: the versions the context knows of its
/// resources.
pub fn expects(context: &Context, limb: Option<LimbId>) -> Vec<Observation> {
    context
        .knows
        .iter()
        .filter(|(r, _)| Some(r.limb) == limb)
        .map(|(resource, known)| Observation {
            resource: resource.clone(),
            version: known.has,
        })
        .collect()
}

/// A `background` call's input as the call it starts, with its input's
/// bytes: routed by the caller's binding, never to the meta limb.
pub fn background_call(
    parent: &Task,
    from: CallId,
    meta: LimbId,
    input: &[u8],
) -> Result<(Call, Bytes), String> {
    let (tool, input) = background(input)?;
    let context = parent.context().ok_or("you have no context")?;
    let limb = context
        .binding
        .limb(&tool)
        .ok_or(format!("you have no tool `{tool}`"))?;
    if limb == meta {
        return Err(format!("`{tool}` cannot run in the background"));
    }
    let call = Call {
        caller: Caller::Background {
            task: parent.id,
            from,
        },
        expects: expects(context, Some(limb)),
        tool,
        input: input.id(),
    };
    Ok((call, input))
}

/// Max's words a label names, among those `parent` has heard: his messages,
/// the mandates he gave, his answers, and what briefs quoted.
pub fn quoted(state: &State, parent: &Task, label: &str) -> Result<ValueId, String> {
    let words = state.deliveries().filter_map(|d| match d.delivery.content {
        Content::Words(words) => Some(words),
        Content::Prompt(_) => None,
    });
    let mandates = state
        .tasks()
        .flat_map(|t| [t.mandate.text].into_iter().chain(t.mandate.quotes.clone()));
    let answers = state
        .calls()
        .filter_map(|(_, c)| c.outcome()?.output().map(|o| o.model));
    words
        .chain(mandates)
        .chain(answers)
        .filter(|v| state.heard(parent.id, *v))
        .find(|v| crate::render::label(*v) == label)
        .ok_or(format!(
            "no words of Max's you have heard are labelled `{label}`"
        ))
}
