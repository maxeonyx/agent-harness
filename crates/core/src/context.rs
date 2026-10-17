//! Contexts: exactly what a model was sent, as an immutable tree of entries.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::{CallId, DeliveryId, Digest, ParseError, ValueId};
use crate::resource::Observation;

/// The identity of an entry, and so of the context that ends at it: the
/// digest of the entry's fields, which name its parent and its bytes.
///
/// It commits to the model and every byte from the root, and to nothing that
/// was not sent. So two contexts with the same id are the same input to the
/// same model and share the provider's cache, and a context's id is the key
/// for "was this prefix sent with a breakpoint recently?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryId(pub Digest);

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EntryId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<EntryId, ParseError> {
        text.parse().map(EntryId)
    }
}

/// One node of the context tree: a root, or one wire message after a parent.
///
/// **Concept.** A context is the chain of entries from a root to a head,
/// and it is a value. Each entry is one message exactly as it went over the
/// wire, never edited after it exists. Entries are content-addressed and
/// name their parent, so a context can only grow by appending, two contexts
/// with a common prefix share those entries, and the tree's branch points
/// are exactly where the provider caches of its contexts diverge. A context
/// is named by its head's [`EntryId`].
///
/// An entry is small structure; its bytes are [values](ValueId). So the
/// tree is a Merkle tree over the value store, and it lives in the brain's facts:
/// each entry arrives inside the fact that first sends or receives it (a
/// root in [`crate::task::Start`], a request's messages in
/// [`crate::model_call::ModelCall::appends`], a response in
/// [`crate::model_call::Ending::Responded`]), so the fold holds the whole
/// tree and refuses a head that does not grow along it. Nothing else
/// stores entries.
///
/// Fork, fresh start, shared seed, compaction, resume, rewind and aside
/// are all a task's head moving to an entry ([`crate::task::Move`]): to the
/// parent's response (a fork), to a new root (a fresh start), to an earlier
/// entry of its own chain with the carry of a compaction appended, or to a
/// new root with that carry (a compaction onto a checkpoint or a fresh
/// one), to a finished task's last entry (a resume), back along its own
/// chain (a rewind). Fresh siblings given the same shared brief and
/// attachments append the same messages to the same root, so the shared
/// seed Max describes in `handoff-improvements.md` is one set of nodes and
/// one cache prefix by construction, not by bookkeeping.
///
/// Why each entry exists is not in it. Provenance is in the request that
/// first sent it ([`crate::model_call::ModelCall::renders`]), or the
/// compaction that carried it ([`crate::task::Compaction::renders`]), and the
/// versions it embodies are in the facts it renders
/// ([`crate::call::Output::observes`]): Bx's separation of an artifact from
/// the build record that says how it was made, which keeps identical bytes
/// one node, as they are one prefix to the provider.
///
/// **Vision.** The context lifecycle, which Max ranks among the most
/// interesting parts: append vs rebuild (`context-and-agent-loop.md`),
/// fork-by-default for cache reuse, compaction with attachments
/// (`handoff-improvements.md`), cache-aware compaction (`compaction.md`),
/// "AGENTS.md changed after turn N" (`context-updates.md`), backend cache
/// state that survives a relaunch (`tech.md`), and "compaction never
/// destroys the transcript" (critique §3): nothing is deleted, and what a
/// compaction replaced stays a branch of the tree.
///
/// **Why bytes, not a typed history.** The provider caches by exact prefix
/// and binds each thinking block's signature to the prefix that produced it.
/// A history re-rendered from a neutral form on every request breaks both
/// whenever the renderer changes, and this harness edits its own code. So
/// the stored form is the wire form and there is no conversion step that
/// could drift. Cache breakpoints are not in entries: they are placed at
/// send time and recorded on the request (the provider-wire spike found that
/// moving them does not change what the prefix matches).
///
/// **Existing harnesses.** Every one rebuilds the request from stored
/// conversation on every call. Oh My Pi and Pi keep a JSONL tree of entries
/// with `parentId` and convert it to messages each request; their
/// compaction keeps a summary plus a replayed tail
/// (`ref-oh-my-pi/packages/coding-agent/src/session/session-context.ts:355`),
/// as OpenCode's does (`ref-opencode/packages/schema/src/session-message.ts:255`).
/// Codex forks by building a filtered copy of the parent's history with tool
/// calls and reasoning stripped
/// (`ref-codex/codex-rs/core/src/agent/control/spawn.rs:1023`), so a fork
/// shares neither its parent's context nor its cache.
///
/// **Different here.** The context sent is recorded, never re-derived: it
/// cannot change under the agent, an upgrade cannot re-render old turns, an
/// inspector shows any request byte for byte, and forks, seeds and resumes
/// share nodes rather than copies.
///
/// **Bx.** Git's and Bx's shape: content named by hash, structure named by
/// the hash of its encoding over those names. Bx pins a definition's
/// inputs; here the head pins the whole input of a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entry {
    Root(Root),
    Message { parent: EntryId, message: Message },
}

/// What leads every request of a context, fixed for its life.
///
/// The model is part of it because the provider's cache and its thinking
/// signatures are per model. Changing the model, the tools, the role or the
/// environment's instructions needs a new root: "append or rebuild?" is the
/// choice between a [`Message`] and a [`Root`], and a context whose
/// instructions changed in place cannot be expressed. The first block of
/// `system` is the provider identity string the subscription requires.
///
/// Which limb runs each tool is not here: it is the task's
/// [`crate::environment::Binding`], made from the same descriptions as
/// `tools` by [`crate::environment::Binding::new`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Root {
    pub model: String,
    /// The exact `system` blocks: identity, the environment's role, its
    /// limbs' instructions, the skill index.
    pub system: ValueId,
    /// The exact `tools` array, including tools whose definitions defer
    /// them, so a later message can surface them without breaking the
    /// prefix.
    pub tools: ValueId,
}

/// One wire message: its role and its exact `content` array.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: ValueId,
}

/// The role of a message on the wire. Consecutive messages of one role are
/// allowed; the provider joins them. The harness itself speaks in user
/// content: the subscription's provider accepts no other operator role
/// mid-conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    /// The provider's name for it, which is also how it is written anywhere.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A role name the provider does not have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no role named {:?}", self.0)
    }
}

impl std::error::Error for UnknownRole {}

impl FromStr for Role {
    type Err = UnknownRole;

    fn from_str(text: &str) -> Result<Role, UnknownRole> {
        match text {
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            _ => Err(UnknownRole(text.to_owned())),
        }
    }
}

impl Entry {
    /// The entry's identity. Stable across harness builds; changing it is a
    /// migration of every stored context.
    pub fn id(&self) -> EntryId {
        match self {
            Entry::Root(root) => root.id(),
            Entry::Message { parent, message } => message.id_after(*parent),
        }
    }

    pub fn parent(&self) -> Option<EntryId> {
        match self {
            Entry::Root(_) => None,
            Entry::Message { parent, .. } => Some(*parent),
        }
    }
}

impl Root {
    pub fn id(&self) -> EntryId {
        let Root {
            model,
            system,
            tools,
        } = self;
        digest(&[b"root", model.as_bytes(), &system.0.0, &tools.0.0])
    }
}

impl Message {
    /// The id this message has when it follows `parent`.
    pub fn id_after(&self, parent: EntryId) -> EntryId {
        let Message { role, content } = self;
        digest(&[
            b"message",
            &parent.0.0,
            role.as_str().as_bytes(),
            &content.0.0,
        ])
    }
}

/// BLAKE3 over a tagged, length-prefixed encoding of the fields.
fn digest(fields: &[&[u8]]) -> EntryId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"harness-entry-2");
    for field in fields {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    EntryId(Digest(*hasher.finalize().as_bytes()))
}

/// What a run of blocks the harness wrote was rendered from: the next
/// `blocks` content blocks of the user messages a request
/// [appended](crate::model_call::ModelCall::appends), or of a compaction's
/// [carry](crate::task::Compaction::carry). So whoever reads a context knows
/// who said each block, without reading the harness's wording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub from: Rendered,
    pub blocks: usize,
}

/// Something a request newly included, and so the provenance of the
/// messages it appended.
///
/// The provider has two speaking roles, so Max's words, his actions,
/// attachments and the harness's own notes all reach the model as user
/// content or tool results. This keeps what actually happened, so an
/// inspector never confuses "the agent read this file" with "Max had this
/// file open" or "this was attached when it compacted".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rendered {
    /// A call's settlement: a tool result for one of the task's own tool uses
    /// (a spawn's result is its children's), an attachment, a background
    /// call's result, one of Max's actions in the chat, or, in a
    /// compaction's carry, the output of a call whose tool use it replaced.
    Call(CallId),
    /// Max's words, or a turn the harness asked for.
    Delivery(DeliveryId),
    /// Something the harness says.
    Note(Note),
}

/// What the harness says to a model, and why. Each kind is rendered from
/// data the harness holds; the wording is policy, and the fork-framing spike
/// decided it for forks. Only notes the brain renders are here: a mandate
/// reminder after a tool round (the A.1.3 problem), a clock note, the
/// siblings' status and a quote of an interrupted response are each a new
/// variant when the brain first renders one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    /// What every member of a fresh task's spawn shares, verbatim: its
    /// mandate's brief, then Max's quoted words, each marked as his
    /// ([`crate::task::Mandate::quotes`]). After the attachments and before
    /// the task's own mandate, so siblings share one prefix.
    Brief,
    /// The task's mandate: at a fork, as user text after the answers to the
    /// forked response's tool uses; at a fresh start, after the shared brief;
    /// in the carry of a compaction onto a fresh root, after the framing.
    Mandate,
    /// The neutral answer, in a fork's first message, to a tool use in the
    /// forked response: the spawn that started the fork, the parent's other
    /// calls, or for an aside the call its source is waiting on. Their real
    /// results go to the parent's branch.
    Elsewhere { call: CallId },
    /// A member passed the task one of its running background calls
    /// ([`crate::task::Input::Passed`]): whose it was, why, and that it is
    /// the task's to stop or pass on now.
    Passed { call: CallId },
    /// Things this context embodies have changed since. `told` were only
    /// announced ("changed after you read it"), so the context still embodies
    /// the old versions and the next request announces only what moves
    /// again; `shown` were rendered with the content their limb attached
    /// ([`crate::resource::Change::content`]), so the context now embodies
    /// them.
    Stale {
        told: Vec<Observation>,
        shown: Vec<Observation>,
    },
    /// The opening of a compaction's carry: the harness's framing, in the
    /// agent's own voice ("I compacted my work since…"), the summary the
    /// agent wrote, and the background work it still owns. What it replaced
    /// is the branch ending at the task's head before it
    /// ([`crate::state::Compacted::from`]).
    Compacted,
    /// Max's words, verbatim, carried by a compaction from what it replaced,
    /// so a summary never stands in for what he said. They are one of his
    /// [said](crate::state::State::said) values: a message, a mandate or an
    /// answer.
    Kept { words: ValueId },
    /// The last response stopped short of ending its turn (its output limit,
    /// a provider pause): carry on. A response Max interrupted is quoted
    /// here too, from [`crate::model_call::Ending::Cut`].
    Continue,
}
