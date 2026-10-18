//! Deliveries: what is addressed to a task that is not the result of a call.

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, TaskId, ValueId};

/// Something addressed to a task: Max's words, or a turn the harness asks
/// for.
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
/// cache-expiry compaction, a standing task's event, a background result);
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
    /// When the task reads it.
    pub read: Read,
}

/// When a delivery reaches its task's context.
///
/// Max stages a message in his face, then sends it one of two ways. Either
/// way it is the task's from the moment it is made (it wakes a waiting
/// task, keeps a turn from completing, and survives a new root); they
/// differ only in which request renders it, which is the brain's to honour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Read {
    /// With the task's next request, after the tool round in flight if
    /// there is one: the model hears it mid-turn.
    #[default]
    Next,
    /// Once the model ends its turn: it does not interrupt the work, and
    /// the turn it ended is answered.
    TurnEnd,
}

/// What was delivered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Content {
    /// Max's words: user content blocks (text, a pasted image, a dictated
    /// passage), written having seen the brain's view as of his record's [`crate::fact::Fact::Seen`].
    /// They end his turn and clear the task from his queue, and the task has
    /// heard them, so it can quote them on ([`crate::task::Mandate::quotes`]).
    Words(ValueId),
    /// A turn the harness asks for.
    Prompt(Prompt),
}

/// Why the harness asks for a turn the model did not otherwise have.
///
/// Only prompts something produces are here. A compaction the harness asks
/// for (the cache is about to expire, Max asks from his face) is a new
/// variant when the brain's policy first asks for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prompt {
    /// Max marked a task done: write the result for the owner (for an
    /// aside, what came of the talk since it was opened). It makes the task
    /// autonomous, so the turn it asks for is its last.
    Done,
    /// An autonomous turn ended with background work still running: it will
    /// wake the task when it ends, and the task cannot finish before; stop
    /// it if it is not needed. Said once per task
    /// ([`crate::state::Todo::Linger`]).
    Lingering,
    /// Every other member of the scope this aside is in is done: the task
    /// it was opened on is waiting only on it, and carries on once Max marks
    /// it done. Said once per aside ([`crate::state::Todo::Unblock`]).
    Unblocked,
    /// The task is abandoned, and everything it started has stopped: leave
    /// what it was doing safe and understandable, then end the turn with a
    /// report of where things stand, before the deadline. The one turn an
    /// abandoned task gets ([`crate::cancel`]); said once, by the brain
    /// ([`crate::state::Todo::Tidy`]).
    Tidy,
    /// Max parked the task ([`crate::task::Pause::Park`]): bring the work to
    /// a safe point, start nothing new, keep or stop its background work as
    /// it sees fit, and end the turn saying where things stand. It is not
    /// the end of the task: the turn that reads this ends
    /// [`crate::task::Phase::Parked`]. Said once per park, by the brain
    /// ([`crate::state::Todo::Park`]).
    Park,
    /// Max resumed a task that parked or was asked to: carry on from where
    /// it left off. Said by the brain ([`crate::state::Todo::Resume`]).
    Resume,
}
