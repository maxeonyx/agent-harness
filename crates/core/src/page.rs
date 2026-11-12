//! The agent's page: how an agent talks to Max.
//!
//! **Concept.** A task's page is its standing report for whoever reads its
//! node: Max, for a task that has him ([`crate::task::Mode::UserFacing`],
//! [`crate::task::Mode::Standing`]). It is read top to bottom, like the
//! reports agents write, and it is what opens when he clicks the task: the
//! vision's "an agent answers with a page", and "a question is a page plus a
//! node, answered by talking".
//!
//! **What is typed and what is written.** What the harness acts on is
//! structured: what the agent needs from Max ([`crate::ask::Ask`]), what a
//! page points at ([`Ref`]), the task's name, theme and standing
//! ([`Description`]), and whose move anything is
//! ([`crate::source::Whose`]). What only Max reads is the body: GFM
//! markdown, the language agents already write in, with the design system's
//! components ([`crate::design::Design`]) for layout, figures and live
//! references. A body carries no CSS and no script, so the look is the
//! design system's and the task's theme, it reads on the phone, and no agent
//! can draw a control that is not bound to an ask. One parser, in Rust,
//! reads a body: the brain checks every edit with it, and the tab draws with
//! it, compiled to wasm, so what was checked is what is drawn.
//!
//! **Current by construction.** Three rules make the page Max reads current:
//! - What a machine can know is bound, never written: a [`Ref`] draws its
//!   thing's state now, whoever changed it, so a PR's checks or a child's
//!   phase are never an agent's stale account of them (Underview's "agents'
//!   accounts go stale, sources don't").
//! - A turn that owes Max a page reaches [`crate::task::Phase::AwaitingMax`]
//!   only with the page touched after the turn's last other call
//!   ([`Presented`]).
//! - While the task works, its page is drawn as of its version, and its
//!   progress (the summaries of its calls, which the harness already holds)
//!   comes first.
//!
//! **Not yet folded.** These types are the model. The facts that carry
//! them are the build's to wire: `Fact::Page(TaskId, PageEvent)`,
//! `Fact::Ask(AskId, AskEvent)`, `Fact::Kind(SourceKind)`,
//! `Fact::Design(Design)`, `TaskEvent::Described(Description)`,
//! `Phase::AwaitingMax` carrying an `Option<Presented>`, `Prompt::Present`,
//! `Content::Answer(Answer)`, a `says` on `resource::Change`, a `facing`
//! member of a spawn, and `attention::Ranked` naming an
//! [`crate::ask::Owed`].

use serde::{Deserialize, Serialize};

use crate::id::{AskId, CallId, TaskId, Timestamp, ValueId};
use crate::source::ItemKey;

/// A task's page: the stage its agent edits, and the versions Max was shown.
///
/// **Concept.** One per task that presents. A fresh task has its own; a
/// fork's page tools act on the page of its nearest ancestor that is not a
/// fork, since a fork is its source doing a step, and can build its parent's
/// page with the parent's whole context. Not a file: the stage and every
/// version are facts, in every face's view like the rest of the brain's
/// state, the same at work and at home, versioned without git.
///
/// Serves: the deliverable is a page, never markdown in a chat; "the agent
/// should get tools to edit, stage, test, iterate on these pages before it
/// ends its turn"; pages and actions as values in the database.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub task: TaskId,
    /// What the agent is editing, unseen by Max. `None` before its first
    /// edit.
    pub stage: Option<Stage>,
    /// What Max was shown, oldest first, kept for the task's life: "since you
    /// last looked" compares the version his [`crate::fact::Fact::Seen`]
    /// names with the newest.
    pub versions: Vec<Version>,
}

/// The body the agent is working on.
///
/// **Concept.** The page's working copy. Every edit names the revision it
/// was made against, and one against an older revision is refused in band,
/// naming who edited since, as a stale file write is: two forks building one
/// page never silently lose each other's work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    /// The whole body after the newest edit: markdown with components.
    pub source: ValueId,
    pub rev: u32,
    /// The call that made the newest edit: who a refusal names, and where
    /// the edit falls among the turn's calls, for the gate ([`Presented`]).
    pub by: CallId,
}

/// A body Max was shown.
///
/// **Concept.** Immutable once published. Its components name design system
/// elements, never CSS, so an old version still draws after the design
/// system changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub n: u32,
    pub source: ValueId,
    pub at: Timestamp,
    /// The `page_publish` call that made it, or `None` when the brain
    /// published the stage as the turn ended.
    pub by: Option<CallId>,
}

/// What happens to a page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageEvent {
    /// The agent wrote or edited the stage (`page_write`, `page_edit`).
    /// Refused when `base` is not the stage's revision. A body with problems
    /// is still taken, and the call's result lists them, each saying how to
    /// fix it, so the agent can iterate.
    Edited { base: Option<u32>, source: ValueId },
    /// The stage became a version: by `page_publish` mid-turn (so Max can
    /// answer an ask while the agent works on), or by the brain as a turn
    /// ends with the stage edited after the turn's other calls. Refused
    /// while the body has problems, or before the task has a title and a
    /// theme ([`Description`]).
    Published,
    /// The agent read the newest version and says it still holds: the
    /// gate's cheapest pass, for a turn that changed nothing Max needs to
    /// read. The face can say "checked current at 10:42".
    Kept,
}

/// Whether the page Max is about to read is current: carried by a turn that
/// ends on his move.
///
/// **Concept.** The gate that makes "current by construction" a state, not
/// a hope. When a turn of a task that owes Max a page ends, the brain looks
/// at the page. Touched (edited, published or kept) after the turn's last
/// other call: it publishes the stage if it differs from the newest version,
/// and the task waits on Max, [`Presented::Current`]. Not touched: the task
/// gets one more turn (`Prompt::Present`: "your page is version 4, from
/// before Max's message at 10:02; update it, or keep it if it still holds,
/// then end your turn"). A second turn that ends without touching it waits
/// on Max, [`Presented::Stale`], drawn as such beside the turn's last
/// message: never a loop, and never a stale page shown as current. The same
/// rule serves a Claude Code session, whose extra turn is a user message on
/// its stream-json input, so it needs no hook an enterprise policy could
/// block.
///
/// A task owes Max a page when it has him and its role holds the `page`
/// tool set. A chat whose role does not is a plain chat, and its turns end
/// as they do now.
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
/// **Concept.** One pointer for every live thing: a page's chip or card, an
/// ask's subject, an item node on the map, and what wakes an agent when it
/// changes. In a body: `task:<id>`; `ask:<key>` for the page's own ask, or
/// `ask:<id>` for another task's; and `<kind>:<name>` for an item, such as
/// `github.pr:maxeonyx/agent-harness#231`. The brain resolves each when the
/// stage is edited, and refuses an unknown kind naming the kinds there are.
///
/// Serves: "the harness should be in charge of getting anything that can be
/// gotten automatically"; Underview's "what the source knows, the source
/// says" and "check before ask".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ref {
    /// A task: its phase, whose move it is, its title and headline. An
    /// orchestrator's page shows its children by these.
    Task(TaskId),
    /// An ask, its own or another task's: placed on an orchestrator's page,
    /// Max answers a child's ask there, and the answer goes to the child.
    Ask(AskId),
    /// A thing outside the harness that a source reads: a PR, a ticket.
    Item(ItemKey),
}

/// What a task says about itself: its name, its look and where it stands.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. A name Max gives
/// ([`crate::task::TaskEvent::Renamed`]) stays over the agent's. The page's
/// frame (its head band) and the map node are drawn from it, so the body
/// never repeats it, and a task without a page still has a face on the map.
///
/// Serves: chats that start alike no longer look alike; "easy for me to see
/// whereabouts I'm at"; the agent records its status in one line, while
/// what a machine knows (its progress, its PRs' states) comes from the
/// harness.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// What the work does: "Pages and asks for agents".
    pub title: Option<String>,
    /// A theme from the design system's list ([`crate::design::Theme`]).
    /// Refused if it is not in the list, or if another live task in the
    /// same area holds it while one is free; the refusal lists the free
    /// ones.
    pub theme: Option<String>,
    /// Where it stands, in one line: the map node's subtitle, and the
    /// status the agent records ("PR up, waiting on Sam's review").
    pub headline: Option<String>,
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
