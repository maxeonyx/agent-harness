//! Every word this harness says to a model.
//!
//! The system prompt and the tool list are identical for every agent in a
//! run — root and child alike — because Anthropic caches tools, then system,
//! then messages, and any difference between parent and child breaks the
//! child's inherited prefix. So nothing here may be conditioned on an agent's
//! place in the tree. Framing that *is* per-agent lives in a tool result or a
//! user message, at the tail.
//!
//! A child's tail is two parts: the shared part, the same bytes for every
//! sibling, and then its own. The two `--identity` wordings of them are the
//! experimental treatment.

use serde_json::{Value, json};

pub const SYSTEM_PROMPT: &str = "\
You are an agent in a harness that runs agents as structured concurrency.

You can read one directory, and you can split your work across several agents \
at once with the `task` tool. Calling `task` suspends you until every agent you \
launched has finished; the call then returns each one's report.

Your own report is your final message — the last thing you write before ending \
your turn without calling a tool. Write it for whoever gave you your work: it \
is all they see of what you did.";

pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List the entries of a directory, relative to the run directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory path relative to the run directory. Use \".\" for the run directory itself." }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a UTF-8 text file, relative to the run directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to the run directory." }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "task",
                "description": "Split your work across agents that run at the same time. You are suspended until all of them finish; the call then returns every agent's report, in the order you declared them. Each agent may split its own work the same way. Write what every agent needs once, in `shared`, and keep each agent's own `task` short: only what is its alone. A forked agent starts from your context as it stands right now and pays for it out of the cache, so fork when the work is a continuation of yours; a fresh agent starts from `shared` and its `task` alone, so choose fresh when your context would only be noise to it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "shared": { "type": "string", "description": "Written once, and given to every agent before its own `task`: the goal, the rules, and whatever else they all need." },
                        "agents": {
                            "type": "array",
                            "description": "The agents to launch, in the order you want their reports back.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string", "description": "Short unique name, used to label this agent's report and to refer to it in `after`." },
                                    "task": { "type": "string", "description": "This agent's own part, given after `shared`: what it alone must do. Keep it short, and do not repeat `shared`." },
                                    "after": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "Names of agents in this same call that must finish first. Their reports are given to this agent after its `task`."
                                    },
                                    "fresh": { "type": "boolean", "description": "Start this agent from its assignment alone instead of forking your context. Default false." }
                                },
                                "required": ["name", "task"]
                            }
                        }
                    },
                    "required": ["shared", "agents"]
                }
            }
        }),
    ]
}

/// The framing under test: what a forked child inherits, how its own part
/// names it, and whether it is forked at all. One point in the space the
/// benchmark sweeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Framing {
    pub cut: Cut,
    pub identity: Identity,
    pub mode: crate::agent::Mode,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// The parent's messages through its `task` call, and the shared part as
    /// the answer to that call.
    Result,
    /// The parent's messages before its `task` call, and the shared part as a
    /// user message.
    Before,
    /// The parent's messages through its `task` call, which already holds
    /// the shared part; the answer to that call is the child's own part.
    Call,
}

impl Cut {
    pub fn parse(text: &str) -> Result<Cut, String> {
        match text {
            "result" => Ok(Cut::Result),
            "before" => Ok(Cut::Before),
            "call" => Ok(Cut::Call),
            other => Err(format!(
                "unknown --cut {other}; expected result, before or call"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Cut::Result => "result",
            Cut::Before => "before",
            Cut::Call => "call",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Identity {
    /// The split launched new agents, and the child is one of them.
    Agent,
    /// No new agent: the conversation carries on with one of the tasks it
    /// split its work into.
    Task,
}

impl Identity {
    pub fn parse(text: &str) -> Result<Identity, String> {
        match text {
            "agent" => Ok(Identity::Agent),
            "task" => Ok(Identity::Task),
            other => Err(format!(
                "unknown --identity {other}; expected agent or task"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Identity::Agent => "agent",
            Identity::Task => "task",
        }
    }
}

const AGENT_SPLIT: &str = "This work was handed to these agents, running at the same time:";
const AGENT_SHARED: &str =
    "Each is given what follows, written once for all of them, and then an assignment of its own.";
const AGENT_OWN: &str = "Your assignment:";
const AGENT_STOP: &str = "The other agents are doing their assignments right now, so anything you do towards them is done twice and thrown away. Your final message is exactly what the agent that launched you receives. It is the whole of your report, so put the answer in it rather than pointing at work you did earlier. When your assignment is done, write that message and end your turn without calling a tool. Do not carry on into the work that comes after it.";
const AGENT_AFTER: &str = "Reports from the agents you were told to start after:";

const TASK_SPLIT: &str = "This work was split into these tasks, running at the same time, each as its own continuation of this conversation:";
const TASK_SHARED: &str =
    "Each continuation is given what follows, written once for all of them, and then its own task.";
const TASK_OWN: &str = "Your next task, and only this one, is";
const TASK_STOP: &str = "The other tasks are being done right now, so anything you do towards them is done twice and thrown away. When this task is done, write its report as your final message and end your turn without calling a tool. That message is all that comes back as this task's result, so put the answer in it rather than pointing at work you did earlier. Do not carry on into the work that comes after it.";
const TASK_AFTER: &str = "Reports from the tasks this one was told to start after:";

/// The part every sibling is given, byte for byte: who the work went to, and
/// what the parent wrote once for all of them.
pub fn shared_part(identity: Identity, names: &[String], shared: &str) -> String {
    let names = names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let (split, given) = match identity {
        Identity::Agent => (AGENT_SPLIT, AGENT_SHARED),
        Identity::Task => (TASK_SPLIT, TASK_SHARED),
    };
    let mut text = format!("{split} {names}. {given}");
    if !shared.is_empty() {
        text.push_str("\n\n");
        text.push_str(shared);
    }
    text
}

/// This child's own part: its name, its task, the instruction to stop, and
/// the reports of any siblings it was declared to start after.
pub fn own_part(
    identity: Identity,
    name: &str,
    task: &str,
    dependencies: &[(String, String)],
) -> String {
    let (mut text, after) = match identity {
        Identity::Agent => (
            format!("You are agent `{name}`.\n\n{AGENT_OWN}\n{task}\n\n{AGENT_STOP}"),
            AGENT_AFTER,
        ),
        Identity::Task => (
            format!("{TASK_OWN} `{name}`:\n{task}\n\n{TASK_STOP}"),
            TASK_AFTER,
        ),
    };
    if !dependencies.is_empty() {
        text.push_str(&format!("\n\n{after}\n"));
        for (dependency, handoff) in dependencies {
            text.push_str(&format!("\n## `{dependency}`\n{handoff}\n"));
        }
    }
    text
}

/// The one tool result the suspended parent is resumed with.
pub fn scope_result(reports: &[(String, String, String)]) -> String {
    let mut text = String::from(
        "Every agent you launched has finished. Their reports, in the order you declared them:\n",
    );
    for (name, outcome, handoff) in reports {
        text.push_str(&format!("\n## `{name}` — {outcome}\n{handoff}\n"));
    }
    text
}

pub fn depth_limit_error(max_depth: usize) -> String {
    format!(
        "Error: this run allows agents no more than {max_depth} levels below the root, and you are at the limit. Do this work yourself."
    )
}
