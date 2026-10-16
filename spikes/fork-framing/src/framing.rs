//! The framings under test: how a fork's context starts, and what is
//! appended after each of its tool rounds.
//!
//! Most framings fork at the parent's spawning response, which holds one
//! `spawn` tool use per sibling. The provider requires the next user message
//! to answer every one of them, so that message is where the framing lives.
//! `Rewind` instead forks at the parent's request, before its response. The
//! mandate wording is shared, so framings differ in where it is placed and
//! what surrounds it, not in how well it is written.

use serde_json::{Value, json};

use crate::scenario::Part;

/// One framing: a point on each axis that could matter.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Framing {
    pub name: &'static str,
    /// Fork at the parent's request, before its spawning response, so the
    /// fork never sees the spawn calls. Otherwise fork at the response.
    pub rewind: bool,
    /// What the fork's own spawn call returns. Ignored when rewinding.
    pub own: Own,
    /// What every other spawn call returns. Ignored when rewinding.
    pub others: Others,
    /// Say plainly that the agent that wrote the spawns is another agent.
    pub identity: bool,
    /// Restate the mandate after each tool round.
    pub reminder: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Own {
    /// The mandate text the parent wrote, and nothing else.
    Raw,
    /// The full mandate: place in the work, siblings, and that ending the
    /// turn completes the fork.
    Mandate,
    /// The same neutral answer as every other call, with the full mandate
    /// following as user text.
    UserText,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Others {
    /// "Spawned A.3."
    Spawned,
    /// "Forked a child for A.3."
    Forked,
    /// "Not yours: A.3 is being done by another fork..."
    NotYours,
}

const fn preset(
    name: &'static str,
    own: Own,
    others: Others,
    identity: bool,
    reminder: bool,
) -> Framing {
    Framing {
        name,
        rewind: false,
        own,
        others,
        identity,
        reminder,
    }
}

pub const ALL: [Framing; 8] = [
    // What a naive fork does.
    preset("bare", Own::Raw, Others::Spawned, false, false),
    // Design A: neutral answers, the brief as a user message.
    preset("user", Own::UserText, Others::Forked, false, false),
    preset("user-remind", Own::UserText, Others::Forked, false, true),
    // Design B without, then with, its reminder.
    preset("tool", Own::Mandate, Others::NotYours, false, false),
    preset("tool-remind", Own::Mandate, Others::NotYours, false, true),
    // Design B with neutral answers for the other calls.
    preset("tool-neutral", Own::Mandate, Others::Forked, false, true),
    preset("identity", Own::Mandate, Others::NotYours, true, true),
    Framing {
        name: "rewind",
        rewind: true,
        own: Own::UserText,
        others: Others::Forked,
        identity: true,
        reminder: true,
    },
];

impl Framing {
    pub fn parse(s: &str) -> Option<Framing> {
        ALL.into_iter().find(|f| f.name == s)
    }

    /// The fork's starting context, given `ctx`, which ends with the parent's
    /// spawning response.
    pub fn open(self, ctx: &[Value], fork: &Fork) -> Vec<Value> {
        let mut msgs = ctx.to_vec();
        if self.rewind {
            msgs.pop();
            if let Some(Value::Array(blocks)) = msgs.last_mut().map(|m| &mut m["content"]) {
                blocks.push(json!({ "type": "text", "text": self.mandate(fork) }));
            }
        } else {
            msgs.push(json!({ "role": "user", "content": self.answers(fork) }));
        }
        msgs
    }

    fn mandate(self, fork: &Fork) -> String {
        match self.identity {
            true => format!("{}\n\n{}", fork.identity(), fork.mandate()),
            false => fork.mandate(),
        }
    }

    /// An answer to every tool use in the parent's spawning response.
    fn answers(self, fork: &Fork) -> Vec<Value> {
        let mut blocks: Vec<Value> = fork
            .calls
            .iter()
            .map(|call| {
                let text = match (call.part == fork.own.id, self.own, self.others) {
                    (true, Own::Raw, _) => call.mandate.clone(),
                    (true, Own::Mandate, _) => self.mandate(fork),
                    (_, _, Others::Spawned) => format!("Spawned {}.", call.part),
                    (_, _, Others::Forked) => format!("Forked a child for {}.", call.part),
                    (true, Own::UserText, Others::NotYours) => format!("Forked a child for {}.", call.part),
                    (false, _, Others::NotYours) => format!(
                        "Not yours: {} is being done by another fork, and its result goes to the owner of {}.",
                        call.part, fork.forker.id
                    ),
                };
                json!({ "type": "tool_result", "tool_use_id": call.id, "content": text })
            })
            .collect();
        if self.own == Own::UserText {
            blocks.push(json!({ "type": "text", "text": self.mandate(fork) }));
        }
        blocks
    }

    /// Appended to the user message carrying each round of tool results.
    pub fn reminder(self, fork: &Fork) -> Option<String> {
        self.reminder.then(|| {
            format!(
                "<system-reminder>You are the fork for {own} only: {siblings} belong to other forks, and \
                `{index}` to the owner of {forker}. When {own} is done, end your turn with your \
                result.</system-reminder>",
                own = fork.own.id,
                siblings = fork.siblings(),
                index = fork.forker.path,
                forker = fork.forker.id,
            )
        })
    }
}

/// One spawn tool use in the parent's response.
pub struct Call {
    pub id: String,
    pub part: String,
    pub mandate: String,
}

/// Everything a framing may say about a fork.
pub struct Fork<'a> {
    /// The parts from the root to this fork's part.
    pub chain: Vec<&'a Part>,
    pub forker: &'a Part,
    pub own: &'a Part,
    pub calls: Vec<Call>,
}

impl Fork<'_> {
    fn parent_mandate(&self) -> &str {
        self.calls
            .iter()
            .find(|c| c.part == self.own.id)
            .map_or("", |c| c.mandate.as_str())
    }

    fn siblings(&self) -> String {
        let ids: Vec<_> = self
            .forker
            .children
            .iter()
            .filter(|p| p.id != self.own.id)
            .map(|p| p.id)
            .collect();
        ids.join(" and ")
    }

    fn mandate(&self) -> String {
        let path: Vec<_> = self
            .chain
            .iter()
            .map(|p| format!("{} {}", p.id, p.title))
            .collect();
        format!(
            "You are the fork for {own}. Your place in the work: {path}.\n\n\
            Your part is {own} only. Your parent's mandate for it: {mandate}\n\n\
            {siblings} are being done by other forks at the same time, and {forker}'s index `{index}` is \
            written by its owner after every fork has returned. When {own} is done, end your turn: your last \
            message is your result, and it goes to the owner of {forker}, who continues from there.",
            own = self.own.id,
            path = path.join(" > "),
            mandate = self.parent_mandate(),
            siblings = self.siblings(),
            forker = self.forker.id,
            index = self.forker.path,
        )
    }

    fn identity(&self) -> String {
        format!(
            "You are not the owner of {forker}. That agent split {forker} with spawn; it is still running, \
            and is waiting for your result. You are a new agent, forked from it here with a copy of its \
            conversation.",
            forker = self.forker.id
        )
    }
}
