//! What an agent needs from Max: questions and actions.
//!
//! **Concept.** An ask is a question or an action for Max with an identity
//! of its own ([`crate::id::AskId`]): made by an agent's `ask` call, drawn
//! on pages, ranked in his queue, and settled by his answer, by the harness
//! seeing it done, or by the agent withdrawing it. A page says why; the ask
//! is what he acts on. Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{AskId, DeliveryId, TaskId, Timestamp, ValueId};
use crate::page::Ref;
use crate::source::Whose;

/// One thing an agent needs from Max.
///
/// **Concept.** The structured half of a page: what Max acts on, as data
/// the harness can rank, route and settle, rather than prose he has to find.
/// An ask is live from the agent's `ask` call, so the agent can ask in the
/// background and keep working; blocking on it is just ending its turn. A
/// page draws an ask where its body places it, after the paragraph that
/// says why, never in the lead; one of its task's open asks that the body
/// does not place is drawn at the page's end. An orchestrator's page can
/// place a child's. An ask asked mid-work, before a page places it, opens on
/// its own from the map: its title, its why and its answers. `ask` is
/// declared only to a task that has Max, so no call is refused for not
/// having him: an agent that does not have him puts what it needs in its
/// result, to an ancestor that does, since only an agent that has Max can
/// hand to Max.
///
/// Serves: questions and user actions as nodes, one per wait (vision); "the
/// agent shouldn't have to worry about those kind of things"; an action
/// arrives ready to do, with the why before the button; maintain, cancel or
/// reissue a question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub asker: TaskId,
    /// The agent's name for it, unique among its open asks. Asking again
    /// with the same key revises it, keeping its identity and whatever Max
    /// has drafted for it.
    pub key: String,
    /// What it changes in the world: "Merge #231: page tools".
    pub title: String,
    /// Why it exists, in body markdown, read before the buttons: the chain
    /// from what Max wants to this ask.
    pub why: ValueId,
    pub want: Want,
    /// What it is about. An item's reading decides whose move the ask is
    /// and when it is done: a PR whose review was requested is the
    /// reviewer's move until the review comes back.
    pub about: Option<Ref>,
    /// When it stops being worth answering, and the agent goes on without
    /// him.
    pub expires: Option<Timestamp>,
}

/// What kind of answer an ask wants. Every kind also takes no, "the
/// question is wrong", and his words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Want {
    /// A question: his words are the answer.
    Answer,
    /// Pick one; `recommended` names the option the agent would take.
    Choose {
        options: Vec<Choice>,
        recommended: Option<String>,
    },
    /// Something only he can do outside the harness: review the PR, go to
    /// the website and do this. His "done" settles it, or its `about`
    /// finishing does.
    Do { link: Option<String> },
    /// A call the agent prepared and must not make on its own say-so, shown
    /// exactly as it will run: send the draft email, request Sam's review,
    /// widen an environment. His yes makes it, as his own act. This is what
    /// replaces a permission prompt: a decision asked once, in the open,
    /// with its enduring fix, and never a call held up while it waits.
    Approve { call: Prepared },
}

/// One option of a [`Want::Choose`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub key: String,
    pub label: String,
    /// What choosing it means, in body markdown.
    pub detail: Option<ValueId>,
}

/// A call ready to make: the tool, on the asker's binding, and its exact
/// input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prepared {
    pub tool: String,
    pub input: ValueId,
}

/// What happens to an ask, stated as the effect of its asker's `ask` call.
/// Its identity is the event that first asked it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskEvent {
    /// Asked, or revised by its key while open.
    Asked(Box<Ask>),
    /// The asker took it back: no longer needed, or overtaken. Max sees why
    /// where it was, and words he had drafted for it go to the asker as
    /// words.
    Withdrawn { why: String },
}

/// Max's answer to an ask: a delivery to the asker, as his words are.
///
/// **Concept.** Answering is talking. He presses a choice, dictates, or
/// both, into the ask's own draft ([`crate::draft::Draft`]); sending
/// delivers it to the asker as `Content::Answer`, waking it as his words
/// do. So drafts, two faces racing, and what the agent has heard need
/// nothing new. Only his face's chrome makes one: nothing an agent writes
/// can answer for him.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub ask: AskId,
    pub verdict: Option<Verdict>,
    pub words: Option<ValueId>,
}

/// The button he pressed, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Done, for [`Want::Do`]; yes, for [`Want::Approve`].
    Yes,
    No,
    /// The key of the option chosen, for [`Want::Choose`].
    Chose(String),
    /// The question is wrong; his words say why.
    Wrong,
}

/// Where an ask stands: derived, never stored.
///
/// **Concept.** Whose move, checked rather than guessed. An open ask with no
/// `about` is Max's move. One about an item is whoever its newest reading
/// says: off his plate, drawn quietly with that party's mark, while Sam
/// reviews or CI runs, and back in his queue when the source says it is his
/// again, so an action reaches him only when it is ready to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskState {
    Open {
        whose: Whose,
    },
    /// His answer was delivered.
    Answered {
        delivery: DeliveryId,
    },
    /// Its `about` finished (merged, closed), whoever finished it.
    Done,
    Withdrawn {
        why: String,
    },
    Expired,
}

/// One thing Max owes, as his queue lists it.
///
/// **Concept.** The queue's unit, which `attention::Ranked` names. A task
/// waiting on him with open asks is listed by its asks, one per thing to do,
/// so an ask the agent made in the background reaches him while it works.
/// A task waiting with none is listed by itself: it wants a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Owed {
    Ask(AskId),
    Reply(TaskId),
}
