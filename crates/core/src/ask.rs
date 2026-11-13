//! What an agent needs from Max: questions and actions.
//!
//! **Concept.** An ask is a call of the `ask` tool, in the `max` set: its
//! input is an [`Ask`], its identity is the call's ([`crate::id::CallId`]),
//! and its result is how it settled ([`Settled`]). So an ask needs no
//! machinery of its own. An agent that can go on without the answer runs it
//! in the background and keeps working, and the answer reaches it as the
//! call landing; one that can't ends its turn waiting on it. It settles
//! once. It never outlives its asker, since a task that ends stops its
//! calls. A deadline is the call's own ([`Otherwise::at`]). Not yet folded:
//! see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{CallId, DeliveryId, TaskId, Timestamp, ValueId};
use crate::page::Ref;
use crate::source::Whose;

/// The tool an ask is a call of.
pub const ASK: &str = "ask";

/// One thing an agent needs from Max: the input of an `ask` call.
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
/// small hollow ring (will be), and its now, from a small filled dot (is),
/// end first as a plan step reads; its why, unmarked; what shows it; the
/// answers as pills; and what the agent does without him, in one small
/// line, in its own words. No labels. It is drawn whole on
/// its asker's page, inside the plan step it unblocks, and in his queue.
/// On a page above its asker it is one line, its question, in the step
/// bound to the asker, and a press opens the asker's page at the ask: what
/// he says on a page goes to that page's agent, so each draft has one
/// addressee. Once it is not his move (someone else's, or settled) the
/// question loses its bold and the answers give way to one small line
/// saying what happened, so bold on a page is always a count of what is
/// his.
///
/// Serves: "even an individual ask needs to have eventual state, current
/// state, and then why is this ask important for getting from current
/// state to eventual state"; "if I don't agree with that logic, I'm not
/// going to answer the question, I'm going to reject the question. And
/// that's perfectly okay"; "the agent also needs the ability to create
/// actions for me, which should be part of this page in a structured way";
/// "inline actions where possible"; the vision's explicit question tools,
/// asked in the background.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    /// The agent's name for it, unique among its open asks. Asking again
    /// with the same key revises it: the open call settles as
    /// [`Settled::Revised`], and what Max had pressed for it moves to the
    /// new one.
    pub key: String,
    /// What Max is asked, in words he can answer by talking: "Where should
    /// the work brain live?" Plain text.
    pub question: String,
    /// Where this ask ends: what is true once it is answered and acted on,
    /// in a few words. "The work brain runs where every limb can reach
    /// it." Often narrower than its step, so never left to the page around
    /// it. Plain text.
    pub end: String,
    /// Where it is now, in a few words: "No work brain; the laptop runs a
    /// test one." Inline markdown, naming live things ([`Ref`]).
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
    /// reviewer's move until the review comes back, and a PR he merged
    /// himself settles the ask as [`Settled::Done`].
    pub about: Option<Ref>,
    /// What the agent does without his answer. `None`: it waits for him.
    pub otherwise: Option<Otherwise>,
}

/// What an agent does if Max doesn't answer, and from when.
///
/// **Concept.** The default, said where he can see it and plan around it,
/// in one small line under the answers. So most asks never block: an
/// agent working while he is away goes on with its default, and his
/// answer redirects it. "Until you say, the first slice is built on the
/// defaults"; "No answer by Mon 9:00, and I start on the laptop", which
/// becomes "Since Mon 9:00, going with the laptop".
///
/// Serves: Max's "when I'm away [...] a question for me goes to it
/// [max-advocate] instead of waiting, and its answer stands in for mine";
/// each ask brought to him "with a default".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Otherwise {
    /// What the agent does: "build the first slice on the defaults".
    pub then: String,
    /// `None`: it goes on with `then` now, the ask running in the
    /// background, open until he answers. A time: the call settles
    /// [`Settled::Lapsed`] then, and the agent goes on with `then`; what he
    /// says after reaches it as words.
    pub at: Option<Timestamp>,
}

/// What kind of answer an ask wants.
///
/// **Concept.** Which answers are pills. Every kind also takes his words,
/// and every kind ends with "Wrong question", the rejection, set last and
/// as plainly as the others. The pill the agent proposes is filled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Want {
    /// A go: "Go" and "Wrong question". How an agent asks him to agree on
    /// where its work ends before it starts something costly, outward or
    /// one-way: the ask's end is the page's.
    Go,
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
    /// "Wrong question". His yes, once sent, makes the call as his own act.
    /// This is what replaces a permission prompt: a decision asked once, in
    /// the open, never a call held up while it waits.
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

/// A pill Max pressed, carried by his words when he sends them.
///
/// **Concept.** Answering is talking. His page opens listening; he reads,
/// presses answers on the asks he agrees with, says what he wants to, and
/// one send delivers his words and his presses together, to the agent whose
/// page it is. Each press settles its ask as [`Settled::Answered`] with that
/// delivery, so the agent wakes once, with everything he said. Until he
/// sends, a press is part of his draft: pressing again takes it back. Only
/// his face's chrome makes one: nothing an agent writes can answer for him.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Press {
    pub ask: CallId,
    pub verdict: Verdict,
}

/// The pill he pressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// "Go", for [`Want::Go`]; "Done", for [`Want::Do`]; the yes, for
    /// [`Want::Approve`].
    Yes,
    /// The no, for [`Want::Approve`]: he agrees with the frame, and won't.
    No,
    /// The key of the option chosen, for [`Want::Choose`].
    Chose(String),
    /// "Wrong question": he won't answer, because the frame is wrong. His
    /// words, and the [`crate::page::Spot`] he was reading, say which part
    /// and how. The agent asks again, revised, or withdraws it.
    Reject,
}

/// How an ask settled: the result of its `ask` call.
///
/// **Concept.** Every way an ask ends, as the one result the call returns,
/// so the agent learns it the way it learns anything a call did, in the
/// foreground or when a background call lands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Settled {
    /// He pressed an answer and sent it, with whatever words came with it.
    Answered {
        delivery: DeliveryId,
        verdict: Verdict,
    },
    /// He answered by talking, and the agent closed the ask on his words
    /// with `cite`, naming the delivery they came in. The ask is drawn
    /// quietly with those words under it, quoted from the delivery, so the
    /// agent can close an ask on what he said but never answer it for him.
    Cited { delivery: DeliveryId },
    /// Its `about` finished, whoever finished it.
    Done,
    /// [`Otherwise::at`] passed with no answer.
    Lapsed,
    /// The asker took it back with `withdraw`: no longer needed, or
    /// overtaken. Max sees why where it was.
    Withdrawn { why: String },
    /// Asked again under the same key, by that call.
    Revised { by: CallId },
}

/// Where an ask stands: derived, never stored.
///
/// **Concept.** Whose move, checked rather than guessed. An open ask with
/// no `about` is Max's move. One about an item is whoever its newest
/// reading says: off his plate, drawn quietly with that party's mark, while
/// Sam reviews or CI runs, and back in his queue when the source says it is
/// his again, so an action reaches him only when it is ready to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskState {
    Open { whose: Whose },
    Settled(Settled),
}

/// One thing Max owes, as his queue lists it.
///
/// **Concept.** The queue's unit, which `attention::Ranked` names. A task
/// waiting on him with open asks is listed by its asks, one per thing to
/// do, each drawn whole since it carries its own frame, so an ask the agent
/// made in the background reaches him while it works. A task waiting with
/// none is listed by itself, by its lead: it wants a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Owed {
    Ask(CallId),
    Reply(TaskId),
}
