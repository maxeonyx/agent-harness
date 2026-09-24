//! Every word this harness says to a model.
//!
//! The system prompt and the tool list are identical for every agent in a
//! run — root and child alike — because Anthropic caches tools, then system,
//! then messages, and any difference between parent and child breaks the
//! child's inherited prefix. So nothing here may be conditioned on an agent's
//! place in the tree. Framing that *is* per-agent lives in a tool result or a
//! user message, at the tail.
//!
//! The two `--words` texts are the experimental treatment: they are what the
//! benchmark is comparing.

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
                "description": "Split your work across agents that run at the same time. You are suspended until all of them finish; the call then returns every agent's report, in the order you declared them. Each agent may split its own work the same way. A forked agent starts from your context as it stands right now and pays for it out of the cache, so fork when the work is a continuation of yours; a fresh agent starts from its assignment alone, so choose fresh when your context would only be noise to it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agents": {
                            "type": "array",
                            "description": "The agents to launch, in the order you want their reports back.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string", "description": "Short unique name, used to label this agent's report and to refer to it in `after`." },
                                    "task": { "type": "string", "description": "This agent's assignment. It sees nothing you do not put here or in your shared context." },
                                    "after": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "Names of agents in this same call that must finish first. Their reports are included in this agent's assignment."
                                    },
                                    "fresh": { "type": "boolean", "description": "Start this agent from its assignment alone instead of forking your context. Default false." }
                                },
                                "required": ["name", "task"]
                            }
                        }
                    },
                    "required": ["agents"]
                }
            }
        }),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Words {
    Stop,
    Explained,
}

impl Words {
    pub fn parse(text: &str) -> Result<Words, String> {
        match text {
            "stop" => Ok(Words::Stop),
            "explained" => Ok(Words::Explained),
            other => Err(format!(
                "unknown --words {other}; expected stop or explained"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Words::Stop => "stop",
            Words::Explained => "explained",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    Full,
    Own,
    Before,
}

impl Cut {
    pub fn parse(text: &str) -> Result<Cut, String> {
        match text {
            "full" => Ok(Cut::Full),
            "own" => Ok(Cut::Own),
            "before" => Ok(Cut::Before),
            other => Err(format!(
                "unknown --cut {other}; expected full, own or before"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Cut::Full => "full",
            Cut::Own => "own",
            Cut::Before => "before",
        }
    }
}

/// The naive framing, the one that failed in the probe: the assignment, and
/// an instruction to stop.
fn stop_words() -> String {
    "Do only this, then stop.".to_string()
}

/// The treatment: say what the split is, that siblings hold the rest of it,
/// and what the final message is for.
fn explained_words(name: &str, siblings: &[String]) -> String {
    let mut text = String::new();
    if siblings.is_empty() {
        text.push_str(&format!(
            "You are one branch of a split, launched as `{name}` with this one assignment and no other."
        ));
    } else {
        let others = siblings
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!(
            "You are one branch of a split. Several agents were launched at once and given one assignment each; you are `{name}`, and the other assignments belong to {others}, who are working on them right now. Anything you do towards their assignments is work done twice and is thrown away. Do your own assignment and none of theirs."
        ));
    }
    text.push_str(
        "\n\nYour final message is exactly what the agent that launched you receives. It is the whole of your report — nothing else you write is passed on, so put the answer in it rather than pointing at work you did earlier. When your own assignment is done, write that message and end your turn without calling a tool. Do not carry on into the work that comes after it.",
    );
    text
}

/// The tail a child is given: its assignment, the framing under test, and the
/// reports of any agents it was declared to start after.
pub fn assignment(
    words: Words,
    name: &str,
    task: &str,
    siblings: &[String],
    dependencies: &[(String, String)],
) -> String {
    let mut text = format!("You are agent `{name}`.\n\nYour assignment:\n{task}\n\n");
    text.push_str(&match words {
        Words::Stop => stop_words(),
        Words::Explained => explained_words(name, siblings),
    });
    if !dependencies.is_empty() {
        text.push_str("\n\nReports from the agents you were told to start after:\n");
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
