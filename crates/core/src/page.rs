//! The agent's page: how an agent talks to Max.
//!
//! **Concept.** A task's page is its standing report to Max. A task has one
//! when it holds the user plugin: the tool set that faces Max, which holds
//! the page tools, `ask`, `open_chat`, `choose_theme` and `pass_on_style`.
//! The page is what opens when he clicks the task, read top to bottom: the
//! vision's "an agent answers with a page", and "a question is a page plus a
//! node, answered by talking".
//!
//! **Always the same order.** A page says five things, in the order Max
//! needs them, and the harness sets them out, not the agent:
//! 1. The lead ([`Lead`]): this moment's question or news, the hook that
//!    tells him why he is reading. A new lead fills his first screen alone,
//!    on the theme's ground.
//! 2. Where this ends ([`Sheet::end`]): the eventual state, for him to agree
//!    with before anything else. If he doesn't, nothing below matters.
//! 3. Where we are ([`Sheet::now`]), as fact.
//! 4. The plan from one to the other ([`Step`]), with each open ask
//!    ([`crate::ask::Ask`]) inside the step it unblocks. Most of a page is
//!    here. The plan is the task's own ([`Description::plan`]), so the map
//!    draws the same steps, and the asks are drawn from the asks themselves:
//!    neither is retold on the page, so both are always current.
//! 5. Detail, folded ([`Sheet::more`]).
//!
//! An end and a now he has seen before (`Fact::Seen` is later than the
//! version that last changed them) shrink: the lead takes a short band, the
//! end folds to its claim, and what changed since he looked carries a bar
//! in the left gutter. Nothing on the page names its own parts: no
//! headings over them, no labels, no "scroll down" cues. Each part's claim
//! frames it in its own words ("When this is done, …", "Today …").
//!
//! **The agent writes content; the design system draws it.** The agent
//! gives typed parts, and short markdown inside them ([`crate::body`]). It
//! never sets type, colour or layout. The design system
//! ([`crate::design::Design`]) draws every page in one language, and the
//! task's theme ([`crate::design::Theme`]) gives it its own ground, palette
//! and display face. So Max learns one way of reading that holds for every
//! agent, while each agent still looks like no other. What he says about
//! the look goes to the agent that owns it ([`crate::design::Feedback`]).
//!
//! **Current by construction.** Three rules keep the page Max reads current:
//! - What a machine can know is bound, never written. A [`Ref`] draws its
//!   thing's state now, whoever changed it; a plan step bound to one takes
//!   its state from it; and the asks are drawn from the asks themselves.
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
//! to get wrong, or drawn as well as it can be and listed for the agent as
//! a [`crate::body::Problem`]. Every `page_write` states the lead, the end
//! and where we are, so a page can't lack them. A task with no title or
//! theme yet is drawn with its role's name, in its area's first free theme.
//!
//! **Not yet folded.** These types are the model. The facts that carry
//! them are the build's to wire: `Fact::Page(TaskId, PageEvent)`,
//! `Fact::Ask(AskId, AskEvent)`, `Fact::Kind(SourceKind)`,
//! `Fact::Design(Design)`, `TaskEvent::Described(Description)`,
//! `TaskEvent::Themed(String)` (locked: folded only for a task with no
//! theme), `Phase::AwaitingMax` carrying an `Option<Presented>`,
//! `Prompt::Present`, `Content::Answer(Answer)`,
//! `Content::Feedback(Feedback)`, Max's words carrying the [`Spot`] he was
//! reading, a `says` on `resource::Change`, and `attention::Ranked` naming
//! an [`crate::ask::Owed`]. The tools that make them are `describe` in the
//! meta tool set, for every task, and `page_read`, `page_write`,
//! `page_edit`, `page_publish`, `page_look`, `ask`, `choose_theme` and
//! `pass_on_style` in the user plugin. Those are declared only to a task
//! that has Max, so no call is refused for not having him.

use serde::{Deserialize, Serialize};

use crate::id::{AskId, CallId, TaskId, Timestamp, ValueId};
use crate::source::{ItemKey, Whose};

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
/// ends its turn"; "it should start me at the top of the report and not the
/// bottom".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub task: TaskId,
    /// What the agent is editing, unseen by Max. `None` before its first
    /// write. `page_look` draws it as Max would see it, at a phone's width
    /// and a desktop's, and returns the screens and the problems, so the
    /// agent looks at its page before he does.
    pub stage: Option<Stage>,
    /// What Max was shown, oldest first, kept for the task's life.
    pub versions: Vec<Version>,
}

/// The page the agent is working on.
///
/// **Concept.** The page's working copy. `page_write` states the lead, the
/// end and where we are every time, and replaces the detail when it is
/// given it. `page_edit` applies each change to a body where its old text
/// occurs once, as file edits do, so two forks building one page merge by
/// text. A write says no only over a part another task changed after this
/// one last saw it, since that work would be lost. A body with problems is
/// still taken: the call's result lists them, each saying how to fix it, so
/// the agent iterates rather than retries.
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
/// **Concept.** Immutable once published. Its bodies are markdown in the
/// language's few components ([`crate::body`]), never CSS or colours, so an
/// old version still draws after the design system changes. The plan and
/// the asks are not in it: they are drawn live, as they stand now, so a
/// version never shows a step as waiting that has finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub n: u32,
    pub sheet: Sheet,
    /// When it was published: where it sits among the chat's entries, and
    /// what `Fact::Seen` is compared with for "since you last looked".
    pub at: Timestamp,
    /// The `page_publish` call that made it, or `None` when the brain
    /// published the stage as the turn ended.
    pub by: Option<CallId>,
}

/// What the agent writes on its page: the lead, where this ends, where we
/// are, and the detail.
///
/// **Concept.** The written half of a page, in parts the harness sets out
/// in a fixed order, so every page reads the same way and the agent spends
/// its words on content, never on arrangement or emphasis. The plan is the
/// task's ([`Description::plan`]), and the asks are their own values.
///
/// Serves: "I want to see that we agree on the eventual state. Then I want
/// to see where we are. And then finally, I'm ready for my asks"; "the
/// agent should be given quite a structured way to put its information
/// in"; "more content, less noise".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    pub lead: Lead,
    /// Where this ends: the world as it will be when the work is done.
    /// "When this is done, every Claude Code session on the work laptop runs
    /// under the harness, and stopping one takes under a second." A state,
    /// not a task list: what will be true, not what the agent will do. Its
    /// body shows what will be true as a task list (`- [ ]` not yet, `- [x]`
    /// already true), and gets a figure of its own only when the end is a
    /// thing to look at, such as a screen.
    pub end: Claim,
    /// Where we are, as fact: "Today, a session runs only in a terminal,
    /// and stopping one takes 8.2 s." Its body shows it: one figure of the
    /// system, with what is to come drawn dashed, so this one figure shows
    /// the end and the now together; then the facts, naming live things
    /// ([`Ref`]) instead of retyping their state. The map and the queue say
    /// this claim about the task. When the work is done, it is the end
    /// reached, and the face draws it so.
    pub now: Claim,
    /// Detail he opens if he wants it: files, runs, the long version. Drawn
    /// after the plan, each closed on every visit.
    pub more: Vec<crate::body::Fold>,
}

/// The lead: this moment's question or news, the first thing Max reads.
///
/// **Concept.** The hook: what he needs to know to be on the same page,
/// and why he should read on. Not the task's title, which the window's
/// header already shows: the question it now turns on ("Where should the
/// work brain live?"), the problem ("Stopping a session takes 8 seconds"),
/// or the news ("Every session at work now runs under the harness"). Drawn
/// in bold display type at the lead size, on the theme's ground, alone on
/// his first screen when it is new to him, and as a short band over the
/// end when he has seen it. A lead over 70 characters, or equal to the
/// title, is drawn and listed as a problem.
///
/// Serves: "I need the core framing, the core navigational aid. I need the
/// question. I need the problem. Whatever hooks me"; "I like the very
/// clear, bold title for the question"; "stuff should only come once it's
/// relevant. As I scroll down."
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    /// Plain text, one line on a desktop, two on a phone.
    pub title: String,
    /// One or two plain sentences under it, in reading type, when the title
    /// alone would not tell him why it matters now.
    pub hook: Option<String>,
}

/// One of the page's two states: where this ends, or where we are.
///
/// **Concept.** A claim Max can agree with or dispute, and what shows it.
/// The claim is one sentence, drawn at the heading size in the display
/// face, and framed by its own words rather than a label. The body under
/// it shows the claim, visual first: a figure, a table, a task list, then a
/// few sentences. Because the claim is all he needs to agree, a seen end
/// folds to its claim alone, and opens to its body when pressed.
///
/// Serves: "the agent can communicate to me its idea of the eventual state,
/// can communicate to me the, in fact, current state"; "demonstrate through
/// detailed and concrete diagrams, text [...] that it is on the same page
/// as me"; "visual first".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// One plain sentence. More than one, or over 160 characters, is drawn
    /// and listed as a problem.
    pub says: String,
    /// Body markdown ([`crate::body::Body`]). `None`: the claim says it all.
    pub shows: Option<ValueId>,
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
    /// is the gate's cheapest pass. Never refused.
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
/// drawn with its state now, a plan step's subject, an ask's subject, an
/// item node on the map, and what wakes an agent when it changes. In
/// markdown it is a link's target: `task:<id>`, or `<kind>:<name>` for an
/// item, such as `github.pr:maxeonyx/agent-harness#241`
/// ([`crate::body::Inline::Live`]). The brain resolves each when the stage
/// is edited. One it cannot resolve is drawn as the link's plain words and
/// listed as a problem, naming the kinds there are. A live name is drawn
/// with its state's mark ([`crate::design::Tone`]), never as a pill: in the
/// design language, a pill is an answer.
///
/// Serves: "the harness should be in charge of getting anything that can be
/// gotten automatically"; "it's up to date because the harness is built with
/// that just as a foundational assumption"; Underview's "what the source
/// knows, the source says".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ref {
    /// A task: its title (Max's rename wins), its now-claim, and whose move
    /// it is. An orchestrator's page names its children by these.
    Task(TaskId),
    /// A thing outside the harness that a source reads: a PR, a ticket.
    Item(ItemKey),
}

/// Where on a page Max was reading when he spoke.
///
/// **Concept.** His words carry what he was looking at (Underview's "know
/// what Max was looking at"), set by the face from the scroll when he
/// sends: the ask he last pressed in, else the part in the reading band.
/// So "no, that's not where this is going", said while the end fills the
/// screen, reaches the agent as a rejection of the end, with no button and
/// no form. Also where a [`crate::body::Problem`] is, for the agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spot {
    Lead,
    End,
    Now,
    /// A plan step, by its key.
    Step(String),
    Ask(AskId),
    /// A fold of [`Sheet::more`], by its place.
    More(usize),
}

/// What a task says about itself: its name and its plan.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. Every task has one, with or without a page, so
/// every node on the map has a face and every plan is drawn the same on
/// the map and on the page.
///
/// The theme is not here. It is chosen once, with `choose_theme` from the
/// free ones, folded as `TaskEvent::Themed`, and locked: an agent's look is
/// how Max finds it again, so it never changes under him.
///
/// Serves: chats that start alike no longer look alike; "easy for me to see
/// whereabouts I'm at"; "most of the pages should be about the third part,
/// the plan"; the vision's plans as provisional nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// What the work does: "Claude Code under the harness". A name Max gave
    /// ([`crate::task::TaskEvent::Renamed`]) is shown over it, and
    /// `describe`'s result says so. The window's header, not the page's
    /// lead.
    pub title: Option<String>,
    /// How the work gets from where we are to where it ends, in order.
    /// Replaced whole: a step left out is gone, and the map's node for it
    /// with it.
    pub plan: Option<Vec<Step>>,
}

/// One step of a task's plan.
///
/// **Concept.** A thing the task does or produces on the way to its end: a
/// PR, a child task, a step not started yet. One value, drawn twice: as a
/// node on the map (planned ones dashed, as everything that will exist is),
/// and as a stop on the page's plan rail, with the asks it waits on inside
/// it. A step bound to a live thing ([`Step::of`]) takes its state from it
/// ([`StepState`]), so a merged PR shows its step done without the agent
/// saying so.
///
/// Drawn by relevance: steps behind him are one small line each (and more
/// than three fold into one line of their titles), the steps in play are
/// full, with their body and asks, and the steps ahead are their title and
/// a short body.
///
/// Serves: "then the plan, the current plan on how to get from A to B, and
/// including then asks"; the vision's stream of work as nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The agent's name for it, unique in the plan: how an ask names the
    /// step it sits in ([`crate::ask::Ask::step`]), and how a new plan keeps
    /// a step's node on the map.
    pub key: String,
    /// What it does: "Stop a session's whole process group". `None`: the
    /// name of what it is bound to, so a child task's step shows the
    /// child's own title, renamed or not.
    pub title: Option<String>,
    /// The PR, child task or other live thing this step is, once there is
    /// one. A step bound to a child task shows the child's now-claim, and
    /// the child's open asks sit inside it.
    pub of: Option<Ref>,
    pub said: Said,
    /// Body markdown: why this step, what it changes. Drawn in full while
    /// the step is in play, folded once it is behind him.
    pub body: Option<ValueId>,
}

/// Where the agent says a step stands.
///
/// **Concept.** The agent's word, for what no machine knows. A step bound to
/// a live thing is drawn by that thing's state when it has one, and the
/// agent's word only fills the gaps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Said {
    Planned,
    Doing,
    Done,
    /// It failed or was abandoned, and stays on the page with why, so the
    /// plan shows what was tried.
    Failed {
        why: String,
    },
}

/// Where a step stands as drawn: derived, never stored.
///
/// **Concept.** The step's node on the rail and on the map. Derived in
/// order: a bound thing that ended done makes it [`StepState::Done`]; a
/// step holding an open ask that is Max's move is his move, orange; a bound
/// thing's reading gives whose move it is; else the agent's [`Said`]
/// (`Doing` is the asker's own move). Filled marks are reached, hollow ones
/// are not, and the rail into a step is solid once it is reached and dashed
/// before.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Done,
    Failed {
        why: String,
    },
    /// Someone is moving it: drawn in that party's tone, with a halo.
    Moving(Whose),
    /// Not started; `until` when it can't start before a date.
    Planned {
        until: Option<Timestamp>,
    },
}
