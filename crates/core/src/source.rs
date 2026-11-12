//! What the world says about things outside the harness.
//!
//! **Concept.** An item is a thing a source can read: a PR, a ticket, a
//! deploy, a work agent's session. Its kind ([`SourceKind`]) says who reads
//! it. An item is a resource: [`crate::resource::ResourceKey`] with the
//! kind and the name, on the limb that reads it (the brain, for a claimed
//! kind). Each reading is a [`crate::resource::Change`] that carries what
//! the source says ([`Says`]). So "the state you saw has changed" and "wake
//! me when it is my move" reuse the one mechanism a file uses: a context
//! that publishes a page or an ask observes the items they name, and a
//! change the adapter calls news wakes it. Not yet folded: see
//! [`crate::page`].
//!
//! Serves: "the harness should be in charge of getting anything that can be
//! gotten automatically. GitHub PR statuses, agent statuses at work, the
//! ticketing system statuses"; "ideally, that's actually extensible within
//! the harness as opposed to part of the harness itself"; Underview's
//! "watching the outside world is code", "each fact has one writer", "never
//! write state the source owns".

use serde::{Deserialize, Serialize};

use crate::design::Tone;
use crate::id::{TaskId, Timestamp, ValueId};

/// An item as a page or an ask names it, before the brain finds the limb
/// that reads it: `github.pr` and `maxeonyx/agent-harness#231`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemKey {
    pub kind: String,
    pub name: String,
}

/// What a source says about one item, in the harness's terms.
///
/// **Concept.** One envelope for every kind, so the face draws any kind,
/// and an ask follows any kind, without either knowing kind words. The
/// adapter encodes the source's rules (an approval stands through new
/// commits; a PR with checks running is nobody's move yet), so `whose` is
/// checked, not guessed, and an action reaches Max ready to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Says {
    pub title: String,
    pub url: Option<String>,
    /// The source's own word for its state: "checks running", "approved".
    pub state: String,
    pub whose: Whose,
    /// Finished: merged, closed, deployed. Settles an ask about it.
    pub done: bool,
    /// The date that matters: due, or not startable before.
    pub when: Option<Timestamp>,
    /// What else Max sees when he looks closer at the item: "checks
    /// 14/14", "Sam: approved".
    pub details: Vec<Detail>,
}

/// One labelled fact about an item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detail {
    pub label: String,
    pub value: String,
    pub tone: Option<Tone>,
}

/// Whose move something is: one mark per party.
///
/// **Concept.** The field the harness acts on. It derives Max's queue (an
/// item or ask is his only when it says so), hands an ask on and brings it
/// back, and draws a node's fill. Underview's "blocked on who": one cue per
/// party, and "can't start yet" is nobody's move, with its date.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Whose {
    Max,
    Agent(TaskId),
    /// A colleague, by the name the source uses.
    Person(String),
    /// A machine or an outside party: CI, a vendor, the ops queue.
    Outside(String),
    Nobody {
        until: Option<Timestamp>,
    },
}

/// A kind of item and who reads it.
///
/// **Concept.** The extension point for status. A versioned definition, as
/// a role is: from a file, or defined by an agent's `source_define` call,
/// so a new kind (a ticket, an ops request, an email thread) is data in the
/// harness, not code in it. `source_put` offers only the claimed
/// kinds, so no agent writes what a reader owns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceKind {
    /// How refs name it: `github.pr`, `jira.ticket`.
    pub name: String,
    /// What Max calls one: "Pull request".
    pub label: String,
    /// A glyph from the design system, so each kind has its own shape.
    pub glyph: String,
    pub reader: Reader,
    /// The task that defined it, or `None` for one read from a file.
    pub by: Option<TaskId>,
}

/// Who reads a kind's items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reader {
    /// No reader: agents state readings with `source_put`, and each is drawn
    /// as a dated claim by its agent, since nothing checks it.
    Claimed,
    /// A limb's tool set reads it, where the credentials are: GitHub
    /// through `gh` on the work limb, so work credentials stay at work. The
    /// brain puts the items to watch in that limb's view.
    Watched { set: String },
    /// A script an agent wrote, run on the JavaScript sandbox limb every
    /// `every` seconds and on demand: an agent extends what the harness can
    /// watch without a build.
    Script { code: ValueId, every: u32 },
}
