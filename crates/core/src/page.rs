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
//! **Always the same order.** A page is a report that brings Max's picture
//! of the work level with the agent's. Three things can differ, and each
//! has its own authority: where the work is going (his), where it stands
//! (the agent's, and his to believe or not), and how it gets from one to
//! the other (the agent's to propose and his to redirect). So a page says,
//! in the order he needs it, and the harness sets it out, not the agent:
//! 1. The lead ([`Sheet::lead`]): why he is reading this now, in one
//!    sentence. The same sentence his queue and the map show, so a click
//!    opens on what he clicked.
//! 2. Where this ends ([`Sheet::end`]): the eventual state, shown concretely
//!    enough that he can point at a sentence and say "no". If he doesn't
//!    agree with it, nothing below matters.
//! 3. Where we are ([`Sheet::now`]): facts, the concrete thing first.
//! 4. The plan: the task's steps ([`Step`]), drawn by the harness as a route
//!    with his asks on the steps they gate, then the agent's argument for
//!    it ([`Sheet::plan`]). Most of a page is here, and it means nothing
//!    without 2 and 3.
//! 5. What it needs from him: every open ask of the task and of the
//!    user-facing tasks under it ([`crate::ask::Ask`]), in the plan's order,
//!    each standing on its own. Derived, never written, so the list is
//!    always current, and an orchestrator never retells a child's question.
//!
//! The first screen holds the lead and the end. An end he has seen before
//! (`Fact::Seen` names the version he last saw) shrinks to its point, and
//! comes back whole when the agent changes it. Detail sits in a fold beside
//! the claim it backs, never in a section of its own, so the why behind
//! anything he reads is one step from it. How to fill each part is the user
//! plugin's report guide, which a task reads with its page tools.
//!
//! **The agent writes content; the design system draws it.** The agent
//! gives typed parts, and short markdown inside them. It never sets type,
//! colour or layout, and writes no labels: which part is which is said by
//! the order and the design language, never by an "Where we are" heading.
//! The design system ([`crate::design::Design`]) draws every page in one
//! design language, so what marks a question for Max, an end or a live
//! name is the same for every agent. Its task's theme
//! ([`crate::design::Theme`]) gives the page its own ground, palette and
//! display face. So Max learns one way of reading that holds for every
//! agent, while each agent still looks like no other. What he says about
//! the look goes to the agent that owns it ([`crate::design::Feedback`]).
//!
//! **What is typed and what is written.** What the harness acts on is
//! structured: what the agent needs from Max, what a page points at
//! ([`Ref`]), the task's name and plan ([`Description`]), the page's lead
//! and the end's point, and whose move anything is
//! ([`crate::source::Whose`]). A body is
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
//! to get wrong. Every `page_write` states the lead, the end's point and
//! where we are, so a page can't lack them. A theme is chosen from a list of the free ones. A
//! heading, bold, or a link to a kind that does not exist is drawn as plain
//! text and listed as a problem. A task with no title or theme yet is drawn
//! with its role's name, in its area's first free theme.
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
/// **Concept.** The page's working copy. `page_write` states the lead, the
/// end's point and where we are every time, and replaces a body when it is
/// given one. `page_edit` applies each change to the body its old text
/// occurs in once, as file edits do, so two forks building one page merge
/// by text. A write says no only over a part another task
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

/// What the agent writes on its page: the lead, where this ends, where we
/// are, and the argument for its plan.
///
/// **Concept.** The written half of a report, in parts the harness sets out
/// in a fixed order, so every page reads the same way and the agent spends
/// its words on content, never on arrangement, labels or emphasis. The
/// other half is live and never written here: the plan's steps, the asks,
/// and the state of everything a [`Ref`] names.
///
/// Serves: "I want to see that we agree on the eventual state. Then I want
/// to see where we are. And then finally, I'm ready for my asks"; "the
/// agent can communicate to me its idea of the eventual state, can
/// communicate to me the, in fact, current state, and then the plan [...]
/// most of the pages should be about the third part, the plan, but the plan
/// is meaningless without the first two"; "I need the question. I need the
/// problem. Whatever hooks me"; "the agent should be given quite a
/// structured way to put its information in".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// Why Max is reading this now, in one plain sentence: what changed
    /// since the version he last saw, and what it needs from him, in his
    /// name for the work. Bad news first. If his last words asked
    /// something, it answers them. "Since 31 Oct, 29 deploys to home have
    /// gone through the gate and none went back." Also what his queue and
    /// the map say about the task. Never a label, a count of what is below, or "nothing needs
    /// you": the harness says that, from the asks.
    pub lead: String,
    pub end: End,
    /// Where we are: facts, visual first. The concrete thing comes before
    /// the conclusion it supports: the real log line, the count and where it
    /// was counted, the refusal as the agent saw it. What a machine knows is
    /// named ([`Ref`]), never retyped. What the agent couldn't find out is
    /// said as a fact. When the work is done, this is the end reached, and
    /// it shows the proof. Body markdown.
    pub now: ValueId,
    /// The plan's argument, drawn after the harness draws the task's steps
    /// ([`Description::plan`]): why this route over the ones Max would think
    /// of, each killed in a sentence with its reason; the decisions the
    /// agent took for him, each with the question it answers, so he can
    /// overturn one by saying so; and where the plan stops short of the end,
    /// and what that costs. Body markdown. `None`: the steps say it all.
    pub plan: Option<ValueId>,
}

/// Where this ends: the eventual state, for Max to agree with before
/// anything else.
///
/// **Concept.** The agent's picture of the world when the work is done,
/// shown so that a wrong picture is easy to catch. The body demonstrates it
/// the first time and whenever it changes; the point is what he sees once
/// he has seen it. A state, never a task list or a slogan: each sentence
/// says what will be true, concretely enough that he could point at it and
/// say "no, that's wrong".
///
/// Serves: "I want like agreement on the eventual state. That's like one of
/// the first and most important things"; "demonstrate through detailed and
/// concrete diagrams, text, blah, blah, blah, that it is on the same page as
/// me in terms of [...] the eventual state".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct End {
    /// One or two plain sentences: "A deploy never interrupts a session: a
    /// new build proves itself on a copy of home before it takes over."
    /// What the end shrinks to once seen, so it stands without the body.
    pub point: String,
    /// What shows it: the world when done, often in the same picture as
    /// [`Sheet::now`], drawn as it will be; what Max will see or do, in his
    /// nouns; his words where they set the destination, and the agent's own
    /// additions marked as its own, since invented requirements hide there;
    /// what is deliberately left out. Body markdown. `None`: the point says
    /// it all, as for a small fix.
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
    Plan,
    Ask(AskId),
}

/// What a task says about itself: its name and what it produces.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. Every task has one, with or without a page, so
/// every node on the map has a face. Where the work stands is not here: it
/// is the page's [`Sheet::lead`], and what a machine knows comes from the
/// harness.
///
/// The theme is not here either. It is chosen once, with `choose_theme`
/// from the free ones, folded as `TaskEvent::Themed`, and locked: the tool
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
    /// Its plan: the steps from where it started to its end, done ones
    /// included, so the route reads whole. The map draws them as the task's
    /// nodes, and its page draws them as the route. Replaced whole.
    pub plan: Option<Vec<Step>>,
}

/// One step of a task's plan: a piece of work Max would recognise.
///
/// **Concept.** The vision's plans as provisional nodes, and the one list
/// the map, the page and the attention scheduler read, so they can't
/// disagree. A step is the size of a PR, a child task, or a piece of the
/// agent's own work that changes something he would notice. Its state is
/// bound where it can be: a step that has become a PR or a child shows that
/// thing's state now, so the route stays current when the agent's words go
/// stale. An ask names the step it gates ([`crate::ask::Ask::step`]), so
/// what an answer unblocks is that step and every step after it: how the
/// queue ranks an ask, how one ask says it waits on another, and how an
/// orchestrator's page shows a child's ask on the child's step without
/// retelling it.
///
/// Serves: "Plans show as provisional nodes [...] planned pull requests,
/// planned design agents, planned user questions" (vision); "dependency
/// diagrams, essentially, on actions for me, on chats, on agents to each
/// other"; "some unblock hours of agent work and others unblock nothing"
/// (Underview's stories).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The agent's name for it, unique in the plan, for `after` and for
    /// asks. Never drawn: Max sees the title, and a code is not a name.
    pub key: String,
    /// What it makes true, in Max's nouns, short enough for a node: "The
    /// advocate can search your words", not "T6" or "Implement history
    /// set".
    pub title: String,
    /// The steps that must be done before it starts. A planned step whose
    /// `after` are all done is next.
    pub after: Vec<String>,
    pub state: StepState,
}

/// Where a step stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    /// Not started: drawn dotted.
    Planned,
    /// Under way in the agent's own work.
    Doing,
    /// Done in the agent's own work.
    Done,
    /// It is this PR or task now, and that thing's state is the step's: a
    /// merged PR or a finished child is a done step, whoever finished it.
    Is(Ref),
}
