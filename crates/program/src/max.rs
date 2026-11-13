//! The `max` tool set, which the brain offers to a task facing Max
//! ([`State::faces_max`]) on top of its role's sets: the tools that reach
//! him, and the instructions for talking with him.
//!
//! Its tools are the page and ask tools ([`crate::page`]), with the guide
//! to writing a page, and `open_chat`, which opens a user-facing chat for
//! him as background work of its caller: a background call whose scope's
//! one member is that chat.

use harness_core::call::{Call, Caller};
use harness_core::id::{CallId, Digest, LimbId};
use harness_core::limb::{Audience, Instruction, Tool, ToolSet};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::role::Role;
use harness_core::state::State;
use harness_core::task::Task;
use serde_json::{Value, json};

use crate::meta::{self, How, Member, Spawn};

pub use harness_core::limb::MAX as SET;
pub use harness_core::task::OPEN_CHAT;

/// How a task facing Max talks with him.
const FACING: &str = "This chat is Max's. End your turn when you have answered him or need something only he knows, \
by presenting your page. Ending your turn gives him the move, and what still runs keeps running; \
your work is done when he says so, not when you think it is. A fork of you continues this context, but its own turn \
ends its own work.\n\n\
He often watches you work, so talk to him as you go. Open with a line echoing what you take him to want, so he \
catches a misreading before you spend anything on it; when you move to a new step, say in a sentence what you're \
doing next. That's for him to follow along, not a report: keep it short.";

/// How to write a page and ask Max things: the design system's guide.
const GUIDE: &str = include_str!("page-guide.md");

/// What `open_chat` adds to the instructions, for a task that can start
/// fresh agents.
const OPENS: &str = "`open_chat` opens a chat for Max with a fresh agent in a role you can start, and returns at once \
with its label. It is his chat: it appears in his queue and waits on him whenever its turn ends, and you own it as \
background work, so you can `stop` it, and when he marks it done, its last message comes to you as a message. Open \
one when Max should talk to an agent himself, rather than through you.";

/// The `max` set the brain `limb` gives a root facing Max in a role that
/// starts `roles`; `dir` when the task has a working directory, which a
/// chat it opens works in unless told otherwise.
pub fn set(limb: LimbId, roles: &[&Role], dir: bool) -> ToolSet {
    let mut said = String::from(FACING);
    said.push_str("\n\n");
    said.push_str(GUIDE);
    let mut tools = crate::page::tools();
    if !roles.is_empty() {
        said.push_str("\n\n");
        said.push_str(OPENS);
        tools.push(open_chat_tool(roles, dir));
    }
    ToolSet {
        name: SET.into(),
        instructions: vec![Instruction {
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: SET.into(),
                },
                version: Version::Is(Digest::of(said.as_bytes())),
            },
            text: said,
        }],
        tools,
    }
}

fn open_chat_tool(roles: &[&Role], dir: bool) -> Tool {
    let mut properties = json!({
        "name": { "type": "string", "description": "Short and descriptive, such as `wordcount`." },
        "mandate": { "type": "string", "description": "What this agent is for and what Max needs from it: everything it needs, since it starts fresh." },
        "role": { "type": "string", "enum": roles.iter().map(|r| &r.name).collect::<Vec<_>>(), "description": "The role of the agent Max talks to." },
        "quotes": { "type": "array", "items": { "type": "string" }, "description": "Labels of Max's words (`max-…`) to give it verbatim." }
    });
    if roles.iter().any(|r| r.works_in_a_directory()) {
        properties["dir"] =
            meta::dir_field(dir, "Where it works, if its role works in a directory");
    }
    let definition = json!({
        "name": OPEN_CHAT,
        "description": "Open a chat for Max with a fresh agent, as background work of yours, and return at once.",
        "input_schema": {
            "type": "object",
            "properties": properties,
            "required": ["name", "mandate", "role"],
            "additionalProperties": false
        }
    });
    Tool {
        name: OPEN_CHAT.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
        held: false,
    }
}

/// An `open_chat` call's input, as the spawn of its one member: a fresh
/// agent, with Max's words it quotes.
pub fn open_chat(input: &[u8]) -> Result<Spawn, String> {
    let mut value: Value =
        serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let object = value.as_object_mut().ok_or("the input must be an object")?;
    // Every call's input may carry its `summary` ([`crate::render`]).
    object.remove("summary");
    let [name, mandate, role] = meta::fields(
        &value,
        "`open_chat`",
        ["name", "mandate", "role"],
        &["quotes", "dir"],
    )?;
    let member = Member {
        name,
        mandate,
        how: How::Fresh {
            role,
            dir: meta::string(&value, "dir")?,
        },
    };
    Ok(Spawn {
        members: vec![member],
        brief: None,
        quotes: meta::labels(&value)?,
    })
}

/// The background call an `open_chat` call `from` by `parent` makes, named
/// after its chat, with the same input (`call`'s).
pub fn open_chat_call(
    state: &State,
    parent: &Task,
    from: CallId,
    call: &Call,
    input: &[u8],
) -> Result<Call, String> {
    let name = open_chat(input)?.members.remove(0).name;
    if meta::named(state, parent, &name).is_some() {
        return Err(format!(
            "you already have background work named `{name}` running"
        ));
    }
    Ok(Call {
        caller: Caller::Background {
            task: parent.id,
            from,
            name,
        },
        expects: Vec::new(),
        tool: OPEN_CHAT.into(),
        input: call.input,
    })
}
