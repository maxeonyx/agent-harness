//! Deliveries: what is addressed to a task that is not the result of a call.

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, TaskId, ValueId};

/// Something addressed to a task: Max's words, a turn the harness asks for,
/// or what Max said in an aside.
///
/// **Concept.** A delivery is part of its task's fold from the moment it is
/// made (the task's pending inputs) until a request renders it, and every
/// delivery wakes a task that waits. Nothing is pushed into a context while
/// a request or a tool round is in flight: the provider forbids it (every
/// tool use needs its result first), and a model's thought would come to
/// depend on text that arrived after it began. So Max's words typed while a
/// designer streams its last response are read before its result goes
/// anywhere, and words typed during a long tool round arrive straight after
/// the results. To be heard sooner he interrupts; to talk to a task waiting
/// on its children he opens an aside.
///
/// What an environment does between turns is not a delivery. A watcher's
/// result, a page event, a change to Max's queue: each is a new version of a
/// resource its limb owns ([`crate::resource::Change`]), which reaches every
/// context that embodies it as one notice at its next request, and wakes a
/// standing task that is listening. So nothing a limb says needs an
/// addressee, and nothing piles up.
///
/// **Vision.** The user turn; "what drives API requests" (exactly: a
/// settled tool round, Max ending his turn, a harness prompt such as the
/// cache-expiry handover, a standing task's event, a background result);
/// "most other events only piggyback" (`context-and-agent-loop.md`).
///
/// **Existing harnesses.** OpenCode by default, Pi and Goose put a message
/// typed mid-turn into the next request after the current tool round
/// (`ref-opencode/packages/core/src/session/prompt.ts:50`); Codex preempts
/// the stream (`ref-codex/codex-rs/core/src/session/input_queue.rs:292`);
/// Oh My Pi's steering message "skips remaining tools"
/// (`ref-oh-my-pi/packages/agent/src/agent.ts:1384`), so typing there also
/// cancels the rest of the round. Here a delivery waits for the round, and
/// an interrupt is a separate act recorded on each call it stops.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    pub id: DeliveryId,
    pub to: TaskId,
    pub content: Content,
}

/// What was delivered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Content {
    /// Max's words: user content blocks (text, a pasted image, a dictated
    /// passage), written having seen the journal up to his record's `seen`.
    /// They end his turn and clear the task from his queue.
    Words(ValueId),
    /// A turn the harness asks for.
    Prompt(Prompt),
    /// The result of an aside Max held with a fork of this task while it
    /// waited: what he said there reaches the task itself.
    Aside(TaskId),
}

/// Why the harness asks for a turn the model did not otherwise have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prompt {
    /// Max marked a user-facing task done: write the result for the owner.
    Done,
    /// The first stage of a handover: how to write a good one; tidy up, then
    /// call the second stage.
    Handover,
    /// The context's cache is about to expire while it waits, and a handover
    /// now is expected to cost less than a cold resume.
    Expiring,
    /// An autonomous turn ended with background work still running: it will
    /// wake the task when it ends; stop it if it is not needed. Said once.
    Lingering,
}
