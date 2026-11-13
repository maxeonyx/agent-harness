//! What the world says about things outside the harness.
//!
//! **Concept.** An item is a thing a source can read: a PR, a ticket, a
//! deploy, a work agent's session. Its kind ([`SourceKind`]) says who reads
//! it. An item is a resource: [`crate::resource::ResourceKey`] with the
//! kind and the name, on the limb that reads it (the brain, for a claimed
//! kind). Each reading is a [`crate::resource::Change`] that carries what
//! the source says ([`Says`]). So "the state you saw has changed" and "wake
//! me when it is my move" reuse the one mechanism a file uses: a context
//! that presents a page or asks observes the items they name, and a
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
/// that reads it: `github.pr` and `maxeonyx/agent-harness#241`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemKey {
    pub kind: String,
    pub name: String,
}

/// What a source says about one item, in the harness's terms.
///
/// **Concept.** One envelope for every kind, so the face draws any kind,
/// and an ask or a plan step follows any kind, without either knowing kind
/// words. The adapter encodes the source's rules (an approval stands
/// through new commits; a PR with checks running is nobody's move yet), so
/// `whose` is checked, not guessed, and an action reaches Max ready to do.
/// What it says is drawn as one mark ([`Says::tone`]) and, where there is
/// room, the source's own word for the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Says {
    /// What the item is called in a line of text: "#241", "OPS-88".
    pub name: String,
    pub title: String,
    pub url: Option<String>,
    /// The source's own word for its state: "checks running", "approved,
    /// yours to merge".
    pub state: String,
    pub whose: Whose,
    /// Something on it is broken: checks failed, a deploy errored. Drawn
    /// red, unless it is Max's move, which outranks it.
    pub failing: bool,
    /// It is over, and how. Settles an ask about it, and a plan step bound
    /// to it.
    pub ended: Option<Ended>,
    /// The date that matters: due, or not startable before.
    pub when: Option<Timestamp>,
    /// What else Max sees when he looks closer at the item: "checks
    /// 14/14", "Sam: approved".
    pub details: Vec<Detail>,
}

impl Says {
    /// The item's mark: done, then Max's move, then failing, then whoever
    /// else's move it is. An item that ended without being done (a PR
    /// closed unmerged) is quiet.
    pub fn tone(&self) -> Tone {
        match (&self.ended, &self.whose) {
            (Some(Ended::Done), _) => Tone::Done,
            (Some(Ended::Dropped), _) => Tone::Quiet,
            (None, Whose::Max) => Tone::Yours,
            (None, _) if self.failing => Tone::Failed,
            (None, Whose::Agent(_)) => Tone::Working,
            (None, Whose::Person(_) | Whose::Outside(_)) => Tone::Others,
            (None, Whose::Nobody { .. }) => Tone::Quiet,
        }
    }
}

/// How an item ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ended {
    /// What it was for happened: merged, resolved, deployed.
    Done,
    /// It stopped without that: closed unmerged, cancelled, won't do.
    Dropped,
}

/// One labelled fact about an item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detail {
    pub label: String,
    pub value: String,
    pub tone: Option<Tone>,
}

/// A live name as a page draws it: derived, never stored.
///
/// **Concept.** What the face needs to draw a [`crate::page::Ref`] honestly
/// on a page written at another time: the reading now, and what it said
/// when the page was presented. Prose written against the old state keeps
/// its words; the mark beside it shows the state now, and pointing at it
/// says what the state was when the page was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drawn {
    /// The newest reading. `None` while the first is on its way: drawn
    /// faded, with a dotted ring.
    pub says: Option<Says>,
    pub as_of: Option<Timestamp>,
    /// Why the reading may be out of date ("the work limb is offline since
    /// 14:10"): drawn faded, with the reason when pointed at.
    pub stale: Option<String>,
    /// The state's word when the page was presented, if it has changed
    /// since.
    pub was: Option<String>,
}

/// Whose move something is: one mark per party.
///
/// **Concept.** The field the harness acts on. It derives Max's queue (an
/// item or ask is his only when it says so), hands an ask on and brings it
/// back, and draws a node's mark. Underview's "blocked on who": one cue per
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
/// harness, not code in it. `source_put` offers only the claimed kinds, so
/// no agent writes what a reader owns. A kind has no icon of its own: on a
/// page, shape and colour say whose move a thing is, and only that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceKind {
    /// How refs name it: `github.pr`, `jira.ticket`.
    pub name: String,
    /// What Max calls one, as its popover says: "Pull request".
    pub label: String,
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
