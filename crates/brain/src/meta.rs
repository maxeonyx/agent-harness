//! The meta limb: the lifecycle tools the brain itself implements, as calls.
//!
//! `spawn` opens a scope: one call whose members are forks of the caller or
//! fresh agents of the kinds the limb lists, and which settles with every
//! member's result once the last is done. Its effects (the members' `Created`
//! facts) are appended in the same batch as its `Dispatched`, so a crash
//! leaves both or neither, and a restart never spawns twice.

use harness_core::environment::{
    Audience, Description, Fork, Instruction, Limb, LimbKind, Startable, Tool,
};
use harness_core::id::{Digest, LimbId};
use harness_core::resource::{Observation, ResourceKey, Version};
use serde_json::{Value, json};

use crate::render::{blocks, text};

pub const SPAWN: &str = "spawn";

pub fn definition(id: LimbId, starts: Vec<Startable>) -> Limb {
    Limb {
        id,
        name: "meta".into(),
        kind: LimbKind::Meta { starts },
        fork: Fork::Share,
    }
}

const INSTRUCTIONS: &str = "\
You can start other agents with `spawn`. One call starts a scope: a list of tasks that run at the same time, \
and returns when every one of them has finished, with each one's result (its last message). \
While they run you wait; you cannot do anything else in the meantime, so put everything that can run in parallel into one call.

Each task is either a fork of you or a fresh agent. A fork continues from this exact point of your context: \
it knows everything you know, so its mandate can be short, and it shares your cache, so it is cheap. \
A fresh agent starts from nothing but its role, the shared `brief` and its mandate, so say everything it needs. \
Forks and fresh agents share this workspace with you and with each other: give them parts that do not touch the same files.";

/// What the meta limb tells each root, for the agent kinds it can start.
pub fn description(limb: LimbId, kinds: &[&str]) -> Description {
    let schema = json!({
        "name": SPAWN,
        "description": "Start agents to do parts of your work in parallel, and wait for all of their results.",
        "input_schema": {
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Short and descriptive, such as `wordcount`." },
                            "mandate": { "type": "string", "description": "What this agent must do, and how to check it is done." },
                            "fresh": { "type": "string", "enum": kinds, "description": "Start a fresh agent of this kind. Omit to fork yourself." }
                        },
                        "required": ["name", "mandate"]
                    }
                },
                "brief": { "type": "string", "description": "Context every fresh agent in this call is given before its mandate." }
            },
            "required": ["tasks"]
        }
    });
    Description {
        instructions: vec![Instruction {
            text: INSTRUCTIONS.into(),
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: String::new(),
                },
                version: Version::Is(Digest::of(INSTRUCTIONS.as_bytes())),
            },
        }],
        tools: vec![Tool {
            name: SPAWN.into(),
            definition: schema.to_string(),
            audience: Audience::Agent,
        }],
    }
}

/// A spawn call's input.
pub struct Spawn {
    pub members: Vec<Member>,
    pub brief: Option<String>,
}

pub struct Member {
    pub name: String,
    pub mandate: String,
    /// The agent kind of a fresh member; `None` for a fork.
    pub fresh: Option<String>,
}

pub fn parse(input: &[u8]) -> Result<Spawn, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let tasks = value["tasks"]
        .as_array()
        .filter(|t| !t.is_empty())
        .ok_or("`tasks` must be a non-empty list")?;
    let members = tasks
        .iter()
        .map(|t| {
            let field = |k: &str| t[k].as_str().map(str::to_owned);
            Ok(Member {
                name: field("name").ok_or("each task needs a `name`")?,
                mandate: field("mandate").ok_or("each task needs a `mandate`")?,
                fresh: field("fresh"),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let brief = value["brief"].as_str().map(str::to_owned);
    Ok(Spawn { members, brief })
}

/// The task names a spawn's input gives, for the neutral answer a fork sees.
pub fn names(input: &[u8]) -> Vec<String> {
    parse(input)
        .map(|s| s.members.into_iter().map(|m| m.name).collect())
        .unwrap_or_default()
}

/// A scope's result: each member's outcome under its name, as a tool
/// result's content.
pub fn result(members: &[(String, String)]) -> Vec<u8> {
    let body: Vec<_> = members
        .iter()
        .map(|(name, said)| format!("## {name}\n\n{said}"))
        .collect();
    blocks(&[text(&body.join("\n\n"))])
}
