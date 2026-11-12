//! The agent's page: how an agent talks to Max.
//!
//! **Concept.** A task's page is its standing report for whoever reads its
//! node: Max, for a task that has him ([`crate::task::Mode::UserFacing`],
//! [`crate::task::Mode::Standing`]). It is read top to bottom, like the
//! reports agents write, and it is what opens when he clicks the task: the
//! vision's "an agent answers with a page", and "a question is a page plus a
//! node, answered by talking".
//!
//! **One thing at a time.** A page opens on its [`Lead`] alone: the question
//! Max is asked, or the point, and the few lines that set the scene. That is
//! all he sees until he scrolls; everything else comes in the order he needs
//! it, each ask after the paragraph that says why. Nothing sits above the
//! lead: the map beside the chat is the overview, and the chat's header
//! already names the task and its state.
//!
//! **The beauty is the theme's.** How a page looks is its task's theme
//! ([`crate::design::Theme`]): a ground drawn for that task, a palette and
//! two typefaces. The lead sits on the ground like a cover, and the body
//! reads on the theme's paper below it. So a page can say one thing at a
//! time and still be unmistakably its agent's, and an agent spends its
//! words on content, never on decoration.
//!
//! **What is typed and what is written.** What the harness acts on is
//! structured: what the agent needs from Max ([`crate::ask::Ask`]), what a
//! page points at ([`Ref`]), the task's name and theme ([`Description`]),
//! the page's lead, and whose move anything is ([`crate::source::Whose`]).
//! What only Max reads is the body: GFM markdown, the language agents
//! already write in, with the design system's few components
//! ([`crate::design::Design`]) for figures, live references and asks. A
//! body carries no CSS, no colours and no script, so it reads on the phone,
//! redraws in any theme, and no agent can draw a control that is not bound
//! to an ask. One parser, in Rust, reads a body: the brain checks every edit
//! with it, and the tab draws with it, compiled to wasm, so what was checked
//! is what is drawn.
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
//! **A shape, not a refusal.** A page tool says no only when the world did:
//! a whole write over a stage another task changed since this one saw it,
//! or an edit whose text is not in the body. Everything else is made
//! impossible to get wrong. The lead is its own two fields, so a page can't
//! lack one. A theme is picked from a list of the free ones. An ask the body
//! does not place is drawn at the page's end. A link to a kind that does not
//! exist is drawn as plain text and listed as a problem. A task with no title
//! or theme yet is drawn with its role's name, in its area's first theme.
//!
//! **Not yet folded.** These types are the model. The facts that carry
//! them are the build's to wire: `Fact::Page(TaskId, PageEvent)`,
//! `Fact::Ask(AskId, AskEvent)`, `Fact::Kind(SourceKind)`,
//! `Fact::Design(Design)`, `TaskEvent::Described(Description)`,
//! `Phase::AwaitingMax` carrying an `Option<Presented>`, `Prompt::Present`,
//! `Content::Answer(Answer)`, a `says` on `resource::Change`, and
//! `attention::Ranked` naming an [`crate::ask::Owed`]. The tools that make
//! them: `page_read`, `page_write`, `page_edit`, `page_publish` and
//! `describe`, for a role that holds the `page` tool set; and `ask`, with
//! `open_chat` (the tool review's task A2), declared only to a task that
//! has Max, so no call is refused for not having him.

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
/// **Where it sits.** In the task's chat, at the point each version was
/// published, so scrolling up from a page shows the chat that led to it.
/// When the task waits on Max, its chat opens at the newest version's top,
/// not at the bottom. While it works, its chat opens at its progress, below
/// the newest version, which is drawn out of date. A version a newer one
/// replaced shrinks to one line.
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
    /// What Max was shown, oldest first, kept for the task's life: "since you
    /// last looked" compares the version his [`crate::fact::Fact::Seen`]
    /// names with the newest.
    pub versions: Vec<Version>,
}

/// The page the agent is working on.
///
/// **Concept.** The page's working copy. `page_edit` changes the lead's
/// fields it is given, and applies each body change where its old text
/// occurs once, as file edits do, so two forks building one page merge by
/// text. `page_write` replaces the stage whole, and says no only when another
/// task changed it after this one last saw it, since that work would be
/// lost. A body with problems is still taken: the call's result lists them,
/// each saying how to fix it, so the agent iterates rather than retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub lead: Lead,
    /// Everything below the lead: markdown with components.
    pub body: ValueId,
    /// The call that made the newest edit: whose work a write would replace,
    /// and where the edit falls among the turn's calls, for the gate
    /// ([`Presented`]).
    pub by: CallId,
}

/// A page Max was shown.
///
/// **Concept.** Immutable once published. Its body names design system
/// components and palette tokens, never CSS or colours, so an old version
/// still draws after the design system or the task's theme changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub n: u32,
    pub lead: Lead,
    pub body: ValueId,
    /// When it was published: where it sits among the chat's entries.
    pub at: Timestamp,
    /// The `page_publish` call that made it, or `None` when the brain
    /// published the stage as the turn ended.
    pub by: Option<CallId>,
}

/// What happens to a page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageEvent {
    /// The stage after a `page_write` or `page_edit`.
    Edited { lead: Lead, body: ValueId },
    /// Max is shown the stage: by `page_publish` mid-turn (so he can read on
    /// while the agent works), or by the brain as a turn ends with the page
    /// touched after the turn's other calls. A stage equal to the newest
    /// version makes no new version: it says that version still holds, the
    /// gate's cheapest pass, and the face can say "checked current at
    /// 10:42". Never refused: an open ask of the task that the body does not
    /// place is drawn at the page's end.
    Published,
}

/// What a page opens on, and all Max sees before he scrolls.
///
/// **Concept.** The core framing, given as two fields of `page_write`, not
/// a heading the parser has to find, so a page cannot lack one. The point
/// is the question Max is asked, in words he can answer by talking, since
/// dictation is already running; or, when nothing is asked, the point. The
/// scene is what puts him on the same page: the problem, why he is reading,
/// what changes. Both are plain text, drawn as such: the point in the
/// theme's display face, the scene in its reading face, on the theme's
/// ground, as a cover. Length is advice, not a limit: a scene longer than a
/// first screen holds is listed as a problem. Recorded with each version,
/// so his queue and the map can say what a waiting task asks without
/// reading its body.
///
/// Serves: Max's "I need the question. I need the problem. Whatever hooks
/// me [...] and then I don't need any of the other crap yet"; reports are
/// read from the top.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    /// "Can I send PR #231 to Sam for review?"
    pub point: String,
    /// "Ingest batches that hit GitHub's rate limit now back off and retry
    /// [...] so it waits for your yes."
    pub scene: String,
}

/// Whether the page Max is about to read is current: carried by a turn that
/// ends on his move.
///
/// **Concept.** The gate that makes "current by construction" a state, not
/// a hope. When a turn of a task that owes Max a page ends, the brain looks
/// at the page. Touched (edited or published) after the turn's last other
/// call: it publishes the stage, and the task waits on Max,
/// [`Presented::Current`]. Not touched: the task gets one more turn
/// (`Prompt::Present`: "your page is version 4, from before Max's message
/// at 10:02; update it, or publish it as it is if it still holds, then end
/// your turn"). A second turn that ends without touching it waits on Max,
/// [`Presented::Stale`], drawn as such beside the turn's last message:
/// never a loop, and never a stale page shown as current. The same rule
/// serves a Claude Code session, whose extra turn is a user message on its
/// stream-json input, so it needs no hook an enterprise policy could block.
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
/// **Concept.** One pointer for every live thing: a name in a page's prose
/// drawn with its state now, an ask's subject, an item node on the map, and
/// what wakes an agent when it changes. In a body, as a link's target where
/// the prose names the thing: `task:<id>`; `ask:<key>` for the page's own
/// ask (which places it), or `ask:<id>` for another task's; and
/// `<kind>:<name>` for an item, such as
/// `github.pr:maxeonyx/agent-harness#231`. The brain resolves each when the
/// stage is edited. One it cannot resolve is drawn as the link's plain text
/// and listed as a problem, naming the kinds there are.
///
/// Serves: "the harness should be in charge of getting anything that can be
/// gotten automatically"; Underview's "what the source knows, the source
/// says" and "check before ask".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ref {
    /// A task: its title, its phase and whose move it is. An orchestrator's
    /// page names its children by these.
    Task(TaskId),
    /// An ask, its own or another task's: placed on an orchestrator's page,
    /// Max answers a child's ask there, and the answer goes to the child.
    Ask(AskId),
    /// A thing outside the harness that a source reads: a PR, a ticket.
    Item(ItemKey),
}

/// What a task says about itself: its name, its look and what it produces.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. The map node and the chat's header are drawn
/// from it, so the body never repeats it, and a task without a page still
/// has a face on the map. Where the work stands is not a field: for Max it
/// is the page's [`Lead`], and what a machine knows (its progress, its PRs'
/// states) comes from the harness.
///
/// Serves: chats that start alike no longer look alike; "easy for me to see
/// whereabouts I'm at"; the agent records its status, and the harness gets
/// what can be got automatically.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// What the work does: "Pages and asks for agents". A name Max gave
    /// ([`crate::task::TaskEvent::Renamed`]) is shown over it, and
    /// `describe`'s result says so.
    pub title: Option<String>,
    /// The task's theme ([`crate::design::Theme`]), picked from the list
    /// `describe` offers, made when its context starts: the themes of the
    /// family of the nearest ancestor that has one which no live task in
    /// that area holds, or the whole family when none is free; for a task
    /// with no themed ancestor, the themes of the families no live area
    /// holds. Siblings that pick at once can land on the same theme, and
    /// nothing breaks: each task's ground is drawn from its own id. Until
    /// the agent picks, the task is drawn in its area's first theme.
    pub theme: Option<String>,
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
