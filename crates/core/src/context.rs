//! Contexts: exactly what a model was sent, as an immutable tree of entries.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, DeliveryId, Digest, ModelCallId, TaskId, Timestamp};
use crate::resource::Observation;

/// Exact JSON text as sent to or received from the provider. Never
/// re-serialised: what was stored is what is sent again.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Wire(pub String);

/// The identity of an entry, and so of the context that ends at it: the
/// digest of its bytes and its parent's identity.
///
/// It commits to the model and every byte from the root, and to nothing that
/// was not sent. So two contexts with the same id are the same input to the
/// same model and share the provider's cache, and a context's id is the key
/// for "was this prefix sent with a breakpoint recently?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryId(pub Digest);

/// One node of the context tree: a root, or one wire message after a parent.
///
/// **Concept.** A context is the chain of entries from a root to a head,
/// and it is a value. Each entry is one message exactly as it went over the
/// wire, never edited after it is stored. Entries are content-addressed and
/// name their parent, so a context can only grow by appending, two contexts
/// with a common prefix share those entries, and the tree's branch points
/// are exactly where the provider caches of its contexts diverge. A context
/// is named by its head's [`EntryId`].
///
/// Fork, fresh start, shared seed, handover, resume, rewind and aside are
/// all a task's head moving to an entry ([`crate::task`]): to the parent's
/// response (a fork), to a new root (a fresh start or a handover), to a
/// finished task's last entry (a resume), back along its own chain (a
/// rewind). Fresh siblings that are given the same shared brief and
/// attachments render the same entries onto the same root, so the shared
/// seed Max describes in `handoff-improvements.md` is one set of nodes and
/// one cache prefix by construction, not by bookkeeping.
///
/// Why each entry exists is not in it. Provenance is in the record that
/// first sent it ([`crate::model_call::ModelCall::renders`]) and the
/// versions it embodies are in the facts it renders
/// ([`crate::call::Output::observes`]). That is the separation Bx makes
/// between an artifact (`hash + immutable value`) and the build record that
/// says how it was made; it keeps identical bytes one node, as they are one
/// prefix to the provider.
///
/// **Vision.** The context lifecycle, which Max ranks among the most
/// interesting parts: append vs rebuild (`context-and-agent-loop.md`),
/// fork-by-default for cache reuse, handovers with attachments
/// (`handoff-improvements.md`), cache-aware compaction (`compaction.md`),
/// "AGENTS.md changed after turn N" (`context-updates.md`), backend cache
/// state that survives a relaunch (`tech.md`), and "compaction never
/// destroys the transcript" (critique §3): nothing is deleted, a handover
/// starts a new root.
///
/// **Why bytes, not a typed history.** The provider caches by exact prefix
/// and binds each thinking block's signature to the prefix that produced it.
/// A history re-rendered from a neutral form on every request breaks both
/// whenever the renderer changes, and this harness edits its own code. So
/// the stored form is the wire form and there is no conversion step that
/// could drift. Cache breakpoints are not stored in entries: they are placed
/// at send time and recorded on the request (the provider-wire spike found
/// that moving them does not change what the prefix matches).
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entry {
    Root(Root),
    Message(Message),
}

/// What leads every request of a context, fixed for its life.
///
/// The model is part of it because the provider's cache and its thinking
/// signatures are per model. Changing the model, the tools, the role or the
/// environment's instructions needs a new root: "append or rebuild?" is the
/// choice between [`Entry::Message`] and [`Entry::Root`], and a context
/// whose instructions changed in place cannot be expressed. The first block
/// of `system` is the provider identity string the subscription requires.
///
/// Which limb runs each tool is not here: it is the task's
/// [`crate::environment::Binding`], because identical bytes can be served by
/// different limbs (a fork whose REPL was cloned).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Root {
    pub model: String,
    /// The exact `system` blocks: identity, role, the environment's
    /// instructions, the skill index.
    pub system: Wire,
    /// The exact `tools` array, including tools declared deferred so a later
    /// message can surface them without breaking the prefix.
    pub tools: Wire,
}

/// One wire message after `parent`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub parent: EntryId,
    pub role: Role,
    /// The exact `content` array of the message.
    pub content: Wire,
}

/// The role of a message on the wire. Consecutive messages of one role are
/// allowed; the provider joins them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    /// A mid-conversation operator message, where the provider accepts one
    /// without breaking the cached prefix (designer B's provider notes,
    /// October 2026; verify on the wire before relying on it). Otherwise the
    /// harness speaks in user content.
    System,
}

impl Entry {
    /// The entry's identity: BLAKE3 over a tagged, length-prefixed encoding
    /// of its fields. Stable across harness builds; changing it is a
    /// migration of every stored context.
    pub fn id(&self) -> EntryId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"harness-entry-1");
        let mut field = |bytes: &[u8]| {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        };
        match self {
            Entry::Root(root) => {
                field(b"root");
                field(root.model.as_bytes());
                field(root.system.0.as_bytes());
                field(root.tools.0.as_bytes());
            }
            Entry::Message(message) => {
                field(b"message");
                field(&message.parent.0.0);
                field(match message.role {
                    Role::User => b"user",
                    Role::Assistant => b"assistant",
                    Role::System => b"system",
                });
                field(message.content.0.as_bytes());
            }
        }
        EntryId(Digest(*hasher.finalize().as_bytes()))
    }

    pub fn parent(&self) -> Option<EntryId> {
        match self {
            Entry::Root(_) => None,
            Entry::Message(message) => Some(message.parent),
        }
    }
}

/// Something a request newly included, and so the provenance of the entries
/// it appended.
///
/// The provider has two speaking roles, so Max's words, his actions, an
/// environment's events, attachments and the harness's own notes all reach
/// the model as user content or tool results. This keeps what actually
/// happened, so an inspector never confuses "the agent read this file" with
/// "Max had this file open" or "this was attached at the handover".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rendered {
    /// A call's settlement: a tool result for one of the task's own tool uses
    /// (a spawn's result is its children's), an attachment shown as a tool
    /// call the task made, a background call's result, or one of Max's
    /// actions in the chat.
    Call(CallId),
    /// Max's words, an environment's event, or a turn the harness asked for.
    Delivery(DeliveryId),
    /// Something the harness says.
    Note(Note),
}

/// What the harness says to a model, and why. Each kind is rendered from
/// data the harness holds; the wording is policy and the fork-framing spike
/// decides it for forks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    /// The task's mandate: at a fork, as user text after the answers to the
    /// forked response's tool uses; at a fresh start, after the shared brief;
    /// after a handover, at the top.
    Mandate,
    /// A restatement of the mandate after a tool round, so a deep fork
    /// remembers it is A.1.3 and stops.
    Reminder,
    /// The neutral answer, in a fork's first message, to a tool use in the
    /// forked response: the spawn that started the fork, the parent's other
    /// calls, or for an aside the call its source is waiting on. Their real
    /// results go to the parent's branch.
    Elsewhere { call: CallId },
    /// Things this context embodies have changed since. `told` are the
    /// versions announced, so the next request announces only what has moved
    /// again; the context still embodies the old versions until it looks.
    Stale { told: Vec<Observation> },
    /// How long since the context last heard anything, when that is long
    /// enough to matter ("more than 1h", `context-updates.md`).
    Clock { since: Timestamp },
    /// Siblings in the task's scope as this context last heard of them; the
    /// next request announces only the difference.
    Siblings {
        running: Vec<TaskId>,
        done: Vec<TaskId>,
    },
    /// This context begins where another ended (a handover, a resume, a
    /// reseed), and what is and is not carried over.
    Boundary { from: Option<EntryId> },
    /// The visible text of a response Max interrupted, quoted so that his
    /// correction has something to refer to.
    Interrupted { call: ModelCallId },
    /// The last response stopped short of ending its turn (its output limit,
    /// a provider pause): carry on.
    Continue,
}
