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
/// step gets from one to the other. The frame is drawn as a few sentences
/// read in that order, never as labelled rows. If he doesn't agree with the
/// end or the reasoning, he rejects the ask rather than answering it
/// ([`Verdict::Reject`]). That is expected, and it is as useful to the agent
/// as a yes.
///
/// An ask is live from the agent's `ask` call, so the agent can ask in the
/// background and keep working; blocking on it is just ending its turn.
/// Pages mark it on the step it gates and draw it whole after the plan; no
/// agent places it or retells it.
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
    /// What Max is asked, in words he can answer by talking, in his nouns:
    /// "Delete `add_sibling`?", never "Q3". The language marks it as a
    /// question for him, the same on every page.
    pub question: String,
    /// Where this step ends: what is true once it is done. "Agents start
    /// sub-agents with one tool, spawn." Often narrower than its page's end:
    /// the ask's own destination, never a copy of the page's.
    pub end: String,
    /// Where it is now, with the claim it is about quoted, so it jogs his
    /// memory: "Your process says 'a task may add siblings to its own
    /// scope', and in 15,513 calls no agent has called `add_sibling`." Body
    /// markdown, naming live things ([`Ref`]), with a figure where one shows
    /// it better.
    pub now: ValueId,
    /// Why this ask is what gets from now to the end, as a chain of
    /// consequences he can point at the broken link of, ending in why it is
    /// his: his judgement, his access, an obligation on someone else, or
    /// something that can't be undone. "Every agent reads about a tool none
    /// uses; deleting it changes a line of your process, which is yours to
    /// change." Inline markdown.
    pub why: String,
    pub want: Want,
    /// The step of the asker's plan ([`crate::page::Step`]) that waits on
    /// the answer. What the answer unblocks is that step and every step
    /// after it, which ranks the ask, and an ask on a step after another
    /// ask's step waits on that ask. `None`: no step waits on it.
    pub step: Option<String>,
    /// What it is about. An item's reading decides whose move the ask is
    /// and when it is done: a PR whose review was requested is the
    /// reviewer's move until the review comes back.
    pub about: Option<Ref>,
    /// When it stops being worth answering, and what the agent does then.
    /// `None`: it waits as long as he does.
    pub lapses: Option<Lapse>,
}

/// When an ask stops being worth answering, and what happens then.
///
/// **Concept.** What the agent does without him, said before the time, so
/// letting an ask lapse is an answer he can give on purpose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lapse {
    pub at: Timestamp,
    /// "I build it with the history set reading only the harness's chats."
    pub then: String,
}

/// What kind of answer an ask wants. Every kind also takes no, a rejection,
/// and his words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Want {
    /// A question: his words are the answer.
    Answer,
    /// Pick one; `recommended` names the option the agent would take. A
    /// recommendation is optional; the ask's why is not.
    Choose {
        options: Vec<Choice>,
        recommended: Option<String>,
    },
    /// Something only he can do outside the harness, such as approving a
    /// request in a console only he can sign in to. The ask's words say
    /// exactly what to do and where it can be done. `link` goes straight to
    /// the place only when what to do there will be obvious once he is
    /// there. His "done" settles it, or its `about` finishing does.
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
    /// What follows from choosing it: what happens next, what it costs,
    /// what it rules out. Inline markdown.
    pub means: Option<String>,
    /// What it will look like, when the options differ in what he will see:
    /// the options' figures are drawn side by side. Body markdown.
    pub figure: Option<ValueId>,
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
    /// He won't answer as asked: the end, where things stand or the
    /// reasoning is wrong, and his words say which. One press, never a form:
    /// the agent gets the ask with his words, and revises it or withdraws
    /// it.
    Reject,
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
