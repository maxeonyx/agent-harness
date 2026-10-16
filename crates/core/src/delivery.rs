//! Deliveries: what is addressed to an agent that is not the result of a call.

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, AgentId, ValueId};

/// Something addressed to an agent: Max's words, or a turn the harness asks
/// for.
///
/// **Concept.** A delivery is part of its agent's fold from the moment it is
/// made (the agent's pending inputs) until a request renders it, and every
/// delivery wakes an agent that waits. Nothing is pushed into a context while
/// a request or a tool round is in flight: the provider forbids it (every
/// tool use needs its result first), and a model's thought would come to
/// depend on text that arrived after it began. So Max's words typed while a
/// designer streams its last response are read before its result goes
/// anywhere, and words typed during a long tool round arrive straight after
/// the results. To be heard sooner he interrupts; to talk to an agent waiting
/// on its children he opens an aside.
///
/// What an environment does between turns is not a delivery. A watcher's
/// result, a page event, a change to Max's queue: each is a new version of a
/// resource its limb owns ([`crate::resource::Change`]), which reaches every
/// context that embodies it as one notice at its next request, and wakes a
/// standing agent that is listening. So nothing a limb says needs an
/// addressee, and nothing piles up.
///
/// **Vision.** The user turn; "what drives API requests" (exactly: a
/// settled tool round, Max ending his turn, a harness prompt such as the
/// cache-expiry handover, a standing agent's event, a background result);
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
    pub to: AgentId,
    pub content: Content,
}

/// What was delivered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Content {
    /// Max's words: user content blocks (text, a pasted image, a dictated
    /// passage), written having seen the journal up to his record's `seen`.
    /// They end his turn and clear the agent from his queue, and the agent has
    /// heard them, so it can quote them on ([`crate::agent::Mandate::quotes`]).
    Words(ValueId),
    /// A turn the harness asks for.
    Prompt(Prompt),
}

/// Why the harness asks for a turn the model did not otherwise have.
///
/// Only prompts something produces are here. A cache-expiry handover and a
/// two-stage handover are each a new variant when the brain's policy first
/// asks for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prompt {
    /// Max marked an agent done: write the result for the owner (for an
    /// aside, what came of the talk since it was opened). It makes the agent
    /// autonomous, so the turn it asks for is its last.
    Done,
    /// An autonomous turn ended with background work still running (an ask
    /// of Max included): it will wake the agent when it ends, and the agent
    /// cannot finish before; stop it if it is not needed, or hand it to the
    /// agent's owner if the owner should see it through
    /// ([`crate::call::CallEvent::Handed`]). Said once per agent
    /// ([`crate::state::Todo::Linger`]). This is how an agent whose
    /// provisional choice Max has not answered decides whether to wait for
    /// him.
    Lingering,
    /// Every other member of the scope this aside is in is done: the agent
    /// it was opened on is waiting only on it, and carries on once Max marks
    /// it done. Said once per aside ([`crate::state::Todo::Unblock`]).
    Unblocked,
    /// The agent is leaving, and everything it owned has exited: leave the
    /// work where someone can pick it up, start nothing new, and say what
    /// was done and what is left. Its last turn; the reply is its report
    /// ([`crate::agent::Outcome::Cancelled`]). Said once, and only to an
    /// agent whose cancel gave it the turn ([`crate::state::Todo::Tidy`]).
    TidyUp,
}
