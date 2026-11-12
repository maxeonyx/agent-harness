//! What an agent needs from Max: questions and actions.
//!
//! **Concept.** An ask is a question or an action for Max with an identity
//! of its own ([`crate::id::AskId`]). It is made by an agent's `ask` call,
//! drawn on pages, ranked in his queue, and settled by his answer, by the
//! harness seeing it done, or by the agent withdrawing it. Not yet folded:
//! see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{AskId, DeliveryId, TaskId, Timestamp, ValueId};
use crate::page::Ref;
use crate::source::Whose;

/// One thing an agent needs from Max.
///
/// **Concept.** The structured part of a page: what Max acts on, as data
/// the harness can rank, route and settle, rather than prose he has to
/// find. An ask stands on its own. It is drawn the same wherever it
/// appears: on its asker's page, on the page of each user-facing task above
/// it, in his queue, and from its own node on the map. So it carries its
/// own frame: the question, where it ends, where it is now, and why this
/// step gets from one to the other. If he doesn't agree with the end or the
/// reasoning, he rejects the ask rather than answering it
/// ([`Verdict::Reject`]). That is expected, and it is as useful to the agent
/// as a yes.
///
/// An ask is live from the agent's `ask` call, so the agent can ask in the
/// background and keep working; blocking on it is just ending its turn.
/// Pages draw it after where we are; no agent places it or retells it.
/// `ask` is declared only to a task that has Max, through the user plugin.
/// An agent without him puts what it needs in its result, to an ancestor
/// that has him, since only an agent that has Max can hand to Max.
///
/// Serves: "even an individual ask needs to have eventual state, current
/// state, and then why is this ask important for getting from current
/// state to eventual state"; questions and user actions as nodes, one per
/// wait (vision); "the agent shouldn't have to worry about those kind of
/// things"; an action arrives ready to do, with the why before the button;
/// maintain, cancel or reissue a question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub asker: TaskId,
    /// The agent's name for it, unique among its open asks. Asking again
    /// with the same key revises it, keeping its identity and whatever Max
    /// has drafted for it.
    pub key: String,
    /// What Max is asked, in words he can answer by talking: "Can I send
    /// #231 to Sam for review?" Set in the theme's display face in bold,
    /// which in the design language means only this.
    pub question: String,
    /// Where this step ends: what is true once it is done. "#231 merged,
    /// with Sam's review." Often narrower than its page's end: the ask's own
    /// destination, not the whole work's. Drawn in italic, as every end is.
    pub end: String,
    /// Where it is now: "#231 is finished and green, and only I have seen
    /// it." Inline markdown, naming live things ([`Ref`]).
    pub now: String,
    /// Why this ask is what gets from now to the end, and why it is his:
    /// "Nothing merges without a review, and asking Sam puts work on him, so
    /// that's yours to say yes to." Inline markdown.
    pub why: String,
    pub want: Want,
    /// What it is about. An item's reading decides whose move the ask is
    /// and when it is done: a PR whose review was requested is the
    /// reviewer's move until the review comes back.
    pub about: Option<Ref>,
    /// When it stops being worth answering, and the agent goes on without
    /// him.
    pub expires: Option<Timestamp>,
}

/// What kind of answer an ask wants. Every kind also takes no, a rejection,
/// and his words.
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
    /// What choosing it means, in inline markdown.
    pub detail: Option<String>,
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
/// **Concept.** Answering is talking. He presses an answer, dictates, or
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

/// The answer he pressed, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Done, for [`Want::Do`]; yes, for [`Want::Approve`].
    Yes,
    /// He agrees with the frame, and the answer is no.
    No,
    /// The key of the option chosen, for [`Want::Choose`].
    Chose(String),
    /// He won't answer, because a part of the frame is wrong; his words say
    /// how. The agent revises that part, or withdraws the ask.
    Reject(Part),
}

/// A part of an ask's frame that Max can reject. In his face, he rejects a
/// part by pressing on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Part {
    /// That's not where this should end.
    End,
    /// That's not where things stand.
    Now,
    /// This step doesn't get from now to the end, or isn't his.
    Why,
}

/// Where an ask stands: derived, never stored.
///
/// **Concept.** Whose move, checked rather than guessed. An open ask with
/// no `about` is Max's move. One about an item is whoever its newest
/// reading says. It is off his plate, drawn quietly with that party's mark,
/// while Sam reviews or CI runs, and back in his queue when the source says
/// it is his again, so an action reaches him only when it is ready to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskState {
    Open {
        whose: Whose,
    },
    /// His answer was delivered: a verdict, a rejection, or words.
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
/// waiting on him with open asks is listed by its asks, one per thing to
/// do, so an ask the agent made in the background reaches him while it
/// works. A task waiting with none is listed by itself: it wants a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Owed {
    Ask(AskId),
    Reply(TaskId),
}
