//! The agent's page: how an agent talks to Max.
//!
//! **Concept.** A task's page is its standing report to Max. A task has one
//! when it holds the user plugin: the tool set that faces Max (the tool
//! review's model), which a task holds when it has him, and which holds the
//! page tools, `ask`, `open_chat`, `choose_theme` and `pass_on_style`. The
//! page is what opens when he clicks the task, read top to bottom: the
//! vision's "an agent answers with a page", and "a question is a page plus a
//! node, answered by talking".
//!
//! **Always the same order.** A page says three things, in the order Max
//! needs them, and the harness sets them out, not the agent:
//! 1. Where this ends ([`Sheet::end`]): the eventual state, for him to agree
//!    with before anything else. If he doesn't, nothing below matters.
//! 2. Where we are ([`Sheet::now`]).
//! 3. What it needs from him: every open ask of the task and of the
//!    user-facing tasks under it ([`crate::ask::Ask`]), each standing on its
//!    own. Derived, never written, so the list is always current, and an
//!    orchestrator never retells a child's question.
//!
//! Then, folded, [`Sheet::more`]: detail he opens if he wants it. The first
//! screen holds the end alone. An end he has seen before (`Fact::Seen`
//! names the version he last saw) shrinks to one line above where we are,
//! and comes back whole when the agent changes it.
//!
//! **The agent writes content; the design system draws it.** The agent
//! gives typed parts, and short markdown inside them. It never sets type,
//! colour or layout. The design system ([`crate::design::Design`]) draws
//! every page in one design language: italic is where this ends, bold is a
//! question for Max, and regular type is everything else. Its task's theme
//! ([`crate::design::Theme`]) gives the page its own ground, palette and
//! display face. So Max learns one way of reading that holds for every
//! agent, while each agent still looks like no other. What he says about
//! the look goes to the agent that owns it ([`crate::design::Feedback`]).
//!
//! **What is typed and what is written.** What the harness acts on is
//! structured: what the agent needs from Max, what a page points at
//! ([`Ref`]), the task's name ([`Description`]), the page's end and where
//! we are, and whose move anything is ([`crate::source::Whose`]). A body is
//! GFM markdown with the design system's few components (live names,
//! figures, a fold). It has no headings and no bold, since those are the
//! language's, and no CSS, colours or script. So a body reads on the phone,
//! redraws in any theme, and can't draw a control that is not bound to an
//! ask. One parser, in Rust, reads a body: the brain checks every edit with
//! it, and the tab draws with it, compiled to wasm, so what was checked is
//! what is drawn.
//!
//! **Current by construction.** Three rules keep the page Max reads current:
//! - What a machine can know is bound, never written. A [`Ref`] draws its
//!   thing's state now, whoever changed it, and the asks are drawn from the
//!   asks themselves.
//! - A turn that owes Max a page reaches [`crate::task::Phase::AwaitingMax`]
//!   only with the page touched after the turn's last other call
//!   ([`Presented`]).
//! - While the task works, its page is drawn as of its version, and its
//!   progress (the summaries of its calls, which the harness already holds)
//!   comes first.
//!
//! **A shape, not a refusal.** A page tool says no only when the world did:
//! a write over a part another task changed since this one saw it, or an
//! edit whose text is not in the body. Everything else is made impossible
//! to get wrong. Every `page_write` states the end and where we are, so a
//! page can't lack them. A theme is chosen from a list of the offered ones. A
//! heading, bold, or a link to a kind that does not exist is drawn as plain
//! text and listed as a problem. A task with no title or theme yet is drawn
//! with its role's name, in its area's first offered theme.
//!
//! **Not yet folded.** These types are the model. The facts that carry
//! them are the build's to wire: `Fact::Page(TaskId, PageEvent)`,
//! `Fact::Ask(AskId, AskEvent)`, `Fact::Kind(SourceKind)`,
//! `Fact::Design(Design)`, `TaskEvent::Described(Description)`,
//! `TaskEvent::Themed(String, Identity)` (locked: folded only for a task
//! with no theme; its [`crate::design::Identity`] planned as it chooses), `Phase::AwaitingMax` carrying an `Option<Presented>`,
//! `Prompt::Present`, `Content::Answer(Answer)`,
//! `Fact::Feedback(FeedbackId, FeedbackEvent)`, Max's words carrying the
//! [`crate::design::Screen`] he was looking at, its [`Spot`] included, a
//! `says` on `resource::Change`, and `attention::Ranked` naming an
//! [`crate::ask::Owed`]. The tools that make them are `describe` in the
//! meta tool set, for every task, and `page_read`, `page_write`,
//! `page_edit`, `page_publish`, `ask`, `choose_theme` and `pass_on_style`
//! in the user plugin. Those are declared only to a task that has Max, so
//! no call is refused for not having him.

use serde::{Deserialize, Serialize};

use crate::id::{AskId, CallId, TaskId, Timestamp, ValueId};
use crate::source::ItemKey;

/// A task's page: the stage its agent edits, and the versions Max was shown.
///
/// **Concept.** One per task that holds the user plugin. A fork's page
/// tools act on the page of its nearest ancestor that is not a fork, since
/// a fork is its source doing a step, and can build its parent's page with
/// the parent's whole context. Not a file: the stage and every version are
/// facts, in every face's view like the rest of the brain's state, the same
/// at work and at home, and versioned without git.
///
/// **Where it sits.** In the task's chat, at the point each version was
/// published, so scrolling up from a page shows the chat that led to it.
/// When the task waits on Max, its chat opens at the newest version's top,
/// not at the bottom. While it works, its chat opens at its progress, below
/// the newest version, which is drawn as one line, out of date. A version a
/// newer one replaced also shrinks to one line.
///
/// Serves: the deliverable is a page, never markdown in a chat; "the agent
/// should get tools to edit, stage, test, iterate on these pages before it
/// ends its turn"; pages and actions as values in the database.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub task: TaskId,
    /// What the agent is editing, unseen by Max. `None` before its first
    /// write.
    pub stage: Option<Stage>,
    /// What Max was shown, oldest first, kept for the task's life. "Since
    /// you last looked" compares the version his `Fact::Seen` names with the
    /// newest.
    pub versions: Vec<Version>,
}

/// The page the agent is working on.
///
/// **Concept.** The page's working copy. `page_write` states the end and
/// where we are every time, and replaces the body or the detail when it is
/// given them. `page_edit` applies each change to the body or the detail
/// where its old text occurs once, as file edits do, so two forks building
/// one page merge by text. A write says no only over a part another task
/// changed after this one last saw it, since that work would be lost. A
/// body with problems is still taken: the call's result lists them, each
/// saying how to fix it, so the agent iterates rather than retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub sheet: Sheet,
    /// The call that made the newest edit: whose work a write would replace,
    /// and where the edit falls among the turn's calls, for the gate
    /// ([`Presented`]).
    pub by: CallId,
}

/// A page Max was shown.
///
/// **Concept.** Immutable once published. Its parts name design system
/// components and palette tokens, never CSS or colours, so an old version
/// still draws after the design system changes. Its asks are not in it:
/// they are drawn live, as they stand now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub n: u32,
    pub sheet: Sheet,
    /// When it was published: where it sits among the chat's entries.
    pub at: Timestamp,
    /// The `page_publish` call that made it, or `None` when the brain
    /// published the stage as the turn ended.
    pub by: Option<CallId>,
}

/// What the agent writes on its page: where this ends, where we are, and
/// the detail.
///
/// **Concept.** The written half of a page, in parts the harness sets out
/// in a fixed order, so every page reads the same way and the agent spends
/// its words on content, never on arrangement or emphasis.
///
/// Serves: "I want to see that we agree on the eventual state. Then I want
/// to see where we are. And then finally, I'm ready for my asks"; "the
/// agent should be given quite a structured way to put its information in".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// Where this ends: the world as it will be when the work is done, in a
    /// sentence or two Max can agree with or reject. "Ingest never loses a
    /// row to GitHub's rate limit, and the nightly run still finishes by 7
    /// am." A state, not a task list: it says what will be true, not what
    /// the agent will do. Plain text, set in the theme's display face in
    /// italic, which in the design language means only this.
    pub end: String,
    pub now: Now,
    /// Detail, drawn folded after the asks: files, runs, the long version.
    /// Body markdown. `None`: there is none.
    pub more: Option<ValueId>,
}

/// Where we are.
///
/// **Concept.** The page's update: one sentence Max reads first, and a body
/// that shows it, visual first. When the work is done, it is the end
/// reached, and the face draws it so.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Now {
    /// The update in one plain sentence: "The fix is written and tested; it
    /// needs a reviewer." Set in the theme's display face, upright and
    /// regular. Also what the map and the queue say about the task.
    pub point: String,
    /// What shows it: a figure first where one can replace a paragraph, then
    /// a few sentences that name live things ([`Ref`]) instead of retyping
    /// their state. Body markdown. `None`: the point says it all.
    pub body: Option<ValueId>,
}

/// What happens to a page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageEvent {
    /// The stage after a `page_write` or `page_edit`.
    Edited(Sheet),
    /// Max is shown the stage: by `page_publish` mid-turn, so he can read on
    /// while the agent works, or by the brain as a turn ends with the page
    /// touched after the turn's other calls. A stage equal to the newest
    /// version makes no new version: it says that version still holds, which
    /// is the gate's cheapest pass, and the face can say "checked current at
    /// 10:42". Never refused.
    Published,
}

/// Whether the page Max is about to read is current: carried by a turn that
/// ends on his move.
///
/// **Concept.** The gate that makes "current by construction" a state, not
/// a hope. When a turn of a task that owes Max a page ends, the brain looks
/// at the page. If it was touched (edited or published) after the turn's
/// last other call, the brain publishes the stage, and the task waits on
/// Max, [`Presented::Current`]. If not, the task gets one more turn
/// (`Prompt::Present`: "your page is version 4, from before Max's message
/// at 10:02; update it, or publish it as it is if it still holds, then end
/// your turn"). A second turn that ends without touching it waits on Max,
/// [`Presented::Stale`], drawn as such beside the turn's last message. It
/// never loops, and a stale page is never shown as current. The same rule
/// serves a Claude Code session, whose extra turn is a user message on its
/// stream-json input, so it needs no hook that an enterprise policy could
/// block.
///
/// A task owes Max a page when it holds the user plugin. A task without it
/// is a plain chat, and its turns end as they do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Presented {
    Current {
        version: u32,
    },
    /// Not touched after a reminder: the newest version, if any, is drawn
    /// marked out of date.
    Stale {
        version: Option<u32>,
    },
}

/// Something a page or an ask points at, whose state the harness knows
/// without the agent.
///
/// **Concept.** One pointer for every live thing: a name in a page's prose
/// drawn with its state now, an ask's subject, an item node on the map, and
/// what wakes an agent when it changes. In markdown it is a link's target
/// where the prose names the thing: `task:<id>`, or `<kind>:<name>` for an
/// item, such as `github.pr:maxeonyx/agent-harness#231`. The brain resolves
/// each when the stage is edited. One it cannot resolve is drawn as the
/// link's plain text and listed as a problem, naming the kinds there are.
/// A live name is drawn as the name with a dot in its state's tone after
/// it, never as a pill: in the design language, a pill is something Max
/// can press.
///
/// Serves: "the harness should be in charge of getting anything that can be
/// gotten automatically"; Underview's "what the source knows, the source
/// says" and "check before ask".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ref {
    /// A task: its title, its phase and whose move it is. An orchestrator's
    /// page names its children by these.
    Task(TaskId),
    /// A thing outside the harness that a source reads: a PR, a ticket.
    Item(ItemKey),
}

/// Where on a page Max was reading when he spoke.
///
/// **Concept.** His words carry what he was looking at (Underview's "know
/// what Max was looking at"), set by the face from the scroll when he
/// sends. So "no, that's not where this is going", said while the end fills
/// the screen, reaches the agent as a rejection of the end, with no button
/// and no form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spot {
    End,
    Now,
    Ask(AskId),
    More,
}

/// What a task says about itself: its name and what it produces.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. Every task has one, with or without a page, so
/// every node on the map has a face. Where the work stands is not here: it
/// is the page's [`Now::point`], and what a machine knows comes from the
/// harness.
///
/// The theme is not here either. It is chosen once, with `choose_theme`
/// from the offered ones, folded as `TaskEvent::Themed` with the identity
/// the harness plans for it then ([`crate::design::Identity`]), and locked:
/// the tool
/// is declared only while the task has no theme, and a second choice in the
/// same context leaves the first, as its result says. An agent's look is
/// how Max finds it again, so it never changes under him.
///
/// Serves: chats that start alike no longer look alike; "easy for me to see
/// whereabouts I'm at"; "maybe they get to choose from a set of themes
/// initially, but they of course have to lock it in".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// What the work does: "Retry budget for the ingest worker". A name Max
    /// gave ([`crate::task::TaskEvent::Renamed`]) is shown over it, and
    /// `describe`'s result says so.
    pub title: Option<String>,
    /// What it produces, which the map draws as its nodes. Replaced whole.
    pub work: Option<Vec<Work>>,
}

/// One thing a task produces, or plans to.
///
/// **Concept.** Underview's stream without a separate stream: a PR, a
/// child, a step not started yet. A planned one is drawn dotted (the
/// vision's plans as provisional nodes), and becomes its PR when the agent
/// gives it an `of`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    pub title: String,
    /// What it is, once it exists. `None`: planned, not started.
    pub of: Option<Ref>,
}
