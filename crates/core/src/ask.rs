//! What an agent needs from Max: questions and actions.
//!
//! **Concept.** An ask is a question or an action for Max with an identity
//! of its own ([`crate::id::AskId`]). It is made by an agent's `ask` call,
//! drawn inside its plan step on pages, ranked in his queue, and settled by
//! his answer, by the harness seeing it done, or by the agent's `withdraw`.
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{AskId, DeliveryId, TaskId, Timestamp, ValueId};
use crate::page::Ref;
use crate::source::Whose;

/// One thing an agent needs from Max.
///
/// **Concept.** What Max acts on, as data the harness can rank, route and
/// settle, rather than prose he has to find. An ask stands on its own, so
/// it carries its own frame: the question, where it ends, where it is now,
/// and why this ask is what gets from one to the other. If he doesn't agree
/// with the frame, he rejects the ask rather than answering it
/// ([`Verdict::Reject`]). That is expected, and as useful to the agent as a
/// yes.
///
/// **How it is drawn.** The bold question; then its end, hanging from a
/// small hollow ring (will be); its now, from a small filled dot (is); its
/// why, unmarked; what shows it; the answers as pills; and what the agent
/// does without him, in one small line. No labels. It is drawn whole
/// wherever it is: inside its plan step on its asker's page, inside the
/// step bound to its asker on every page above, in his queue, from its
/// node on the map. Away from its asker's page a small line says who asked
/// and when. Once it is not his move (someone else's, answered, done,
/// withdrawn) the question loses its bold and the answers give way to one
/// small line saying what happened, so bold on a page is always a count
/// of what is his.
///
/// An ask is live from the agent's `ask` call, so the agent can ask in the
/// background and keep working; blocking on it is just ending its turn.
///
/// Serves: "even an individual ask needs to have eventual state, current
/// state, and then why is this ask important for getting from current
/// state to eventual state"; "if I don't agree with that logic, I'm not
/// going to answer the question, I'm going to reject the question. And
/// that's perfectly okay"; "the agent also needs the ability to create
/// actions for me, which should be part of this page in a structured way";
/// "inline actions where possible".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub asker: TaskId,
    /// The agent's name for it, unique among its open asks. Asking again
    /// with the same key revises it, keeping its identity and whatever Max
    /// has drafted for it.
    pub key: String,
    /// What Max is asked, in words he can answer by talking: "Where should
    /// the work brain live?" Plain text.
    pub question: String,
    /// Where this ask ends: what is true once it is answered and acted on.
    /// "The work brain runs where every limb can reach it." Often narrower
    /// than its page's end, so never left to the page around it. Plain
    /// text.
    pub end: String,
    /// Where it is now: "There is no work brain; the laptop runs a test
    /// one." Inline markdown, naming live things ([`Ref`]).
    pub now: String,
    /// Why this ask gets from now to the end, and why it is his to answer:
    /// "Limbs on the laptop and on EC2 both need to reach it, and where it
    /// lives decides what IT has to approve." Inline markdown.
    pub why: String,
    /// What shows the choice, between the why and the answers: the screens
    /// to choose between, a table of the options, a figure. Body markdown
    /// ([`crate::body::Body`]). An approval's prepared call is drawn there
    /// too, from [`Want::Approve`].
    pub shows: Option<ValueId>,
    pub want: Want,
    /// The plan step it unblocks, by its key ([`crate::page::Step::key`]).
    /// `None`, or a key not in the plan: the first step not yet behind him,
    /// which the call's result names. A page with no plan draws its asks
    /// after the now.
    pub step: Option<String>,
    /// What it is about. An item's reading decides whose move the ask is
    /// and when it is done: a PR whose review was requested is the
    /// reviewer's move until the review comes back.
    pub about: Option<Ref>,
    /// What the agent does without his answer. `None`: its step waits for
    /// him.
    pub otherwise: Option<Otherwise>,
}

/// What an agent does if Max doesn't answer, and from when.
///
/// **Concept.** The default, said where he can see it and plan around it,
/// in one small line under the answers. So most asks never block: an
/// agent working while he is away goes on with its default, and his
/// answer, whenever it comes, redirects it. The ask stays his to answer
/// either way; only the line changes: "Until you say, I build it on the
/// defaults", or "No answer by Mon 9:00, and I start on the laptop", which
/// becomes "Since Mon 9:00, going with the laptop". The brain wakes the
/// asker at `from`.
///
/// Serves: Max's "when I'm away [...] a question for me goes to it
/// [max-advocate] instead of waiting, and its answer stands in for mine";
/// each ask brought to him "with a default".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Otherwise {
    /// What the agent does: "build the first slice on the defaults".
    pub then: String,
    /// `None`: it goes on with `then` now. A time: its step waits for him
    /// until then.
    pub from: Option<Timestamp>,
}

/// What kind of answer an ask wants.
///
/// **Concept.** Which answers are pills. Every kind also takes his words,
/// and every kind ends with "Wrong question", the rejection, set last and
/// as plainly as the others. The pill the agent proposes is filled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Want {
    /// A question: his words are the answer, and its one pill is "Wrong
    /// question".
    Answer,
    /// Pick one: a pill per option, with what it means beside it. A yes or
    /// no question is two options.
    Choose {
        options: Vec<Choice>,
        /// The key of the option the agent would take: its pill is filled.
        recommended: Option<String>,
    },
    /// Something only he can do outside the harness: review the PR, ask
    /// IT. Pills: the link (filled), "Done", "Wrong question". His "Done"
    /// settles it, or its `about` finishing does.
    Do {
        link: Option<Link>,
        /// Text he will need there, drawn literally with a copy button: the
        /// message to IT, the command to run.
        paste: Option<String>,
    },
    /// A call the agent prepared and must not make on its own say-so, shown
    /// exactly as it will run, and where. Pills: `yes` (filled), "No",
    /// "Wrong question". His yes makes the call, as his own act, with five
    /// seconds to take it back. This is what replaces a permission prompt:
    /// a decision asked once, in the open, never a call held up while it
    /// waits.
    Approve {
        call: Prepared,
        /// The yes pill's words, saying what happens: "Install it".
        yes: String,
    },
}

/// A link out, as a [`Want::Do`] gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
    /// The pill's words: "Open the IT request form".
    pub label: String,
}

/// One option of a [`Want::Choose`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub key: String,
    /// The pill's words: "An EC2 instance".
    pub label: String,
    /// What choosing it means, and what it costs, in inline markdown.
    pub means: Option<String>,
}

/// A call ready to make: the tool, on the asker's binding, and its exact
/// input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prepared {
    pub tool: String,
    pub input: ValueId,
}

/// What happens to an ask. Its identity is the event that first asked it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskEvent {
    /// Asked by `ask`, or revised by its key while open.
    Asked(Box<Ask>),
    /// The asker took it back with `withdraw`: no longer needed, or
    /// overtaken. Max sees why where it was, and words he had drafted for
    /// it go to the asker as words.
    Withdrawn { why: String },
}

/// Max's answer to an ask: a delivery to the asker, as his words are.
///
/// **Concept.** Answering is talking. He presses an answer, dictates, or
/// both; the words he speaks with an ask in view land in that ask, behind
/// an orange rule while he drafts. Pressing a pill sends at once, with any
/// words drafted for it. Sending delivers to the asker as
/// `Content::Answer`, waking it as his words do. Only his face's chrome
/// makes one: nothing an agent writes can answer for him.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub ask: AskId,
    pub reply: Reply,
}

/// What Max answered with: a pill, or his words alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    /// He pressed a pill, with any words he had drafted for the ask.
    Pressed {
        verdict: Verdict,
        words: Option<ValueId>,
    },
    /// He spoke or typed, and sent.
    Said { words: ValueId },
}

/// The pill he pressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Done, for [`Want::Do`]; the yes, for [`Want::Approve`].
    Yes,
    /// The no, for [`Want::Approve`]: he agrees with the frame, and won't.
    No,
    /// The key of the option chosen, for [`Want::Choose`].
    Chose(String),
    /// "Wrong question": he won't answer, because the frame is wrong. His
    /// words, and the [`crate::page::Spot`] he was reading, say which part
    /// and how. The agent revises the ask, or withdraws it.
    Reject,
}

/// Where an ask stands: derived, never stored.
///
/// **Concept.** Whose move, checked rather than guessed. An open ask with
/// no `about` is Max's move. One about an item is whoever its newest
/// reading says: off his plate, drawn quietly with that party's mark, while
/// Sam reviews or CI runs, and back in his queue when the source says it is
/// his again, so an action reaches him only when it is ready to do. Whether
/// an open ask holds its step up is read off [`Ask::otherwise`] and the
/// clock, not stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskState {
    Open {
        whose: Whose,
    },
    /// His answer was delivered: a pill, a rejection, or words.
    Answered {
        delivery: DeliveryId,
    },
    /// Its `about` finished, whoever finished it.
    Done,
    Withdrawn {
        why: String,
    },
}

/// One thing Max owes, as his queue lists it.
///
/// **Concept.** The queue's unit, which `attention::Ranked` names. A task
/// waiting on him with open asks is listed by its asks, one per thing to
/// do, so an ask the agent made in the background reaches him while it
/// works. A task waiting with none is listed by itself, by its lead: it
/// wants a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Owed {
    Ask(AskId),
    Reply(TaskId),
}
