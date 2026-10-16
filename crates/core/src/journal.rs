//! The journal: one brain's append-only record of everything that happened.

use serde::{Deserialize, Serialize};

use crate::attention::{Focus, Ranking};
use crate::call::{CallEvent, Outcome, Output};
use crate::context::{EntryId, Note, Rendered};
use crate::delivery::Delivery;
use crate::environment::{Environment, Limb, Reach};
use crate::id::{CallId, ContentId, FaceId, LimbId, ModelCallId, Seq, TaskId, Timestamp};
use crate::model_call::{Ending, ModelCallEvent};
use crate::resource::Change;
use crate::task::{Move, TaskEvent};

/// One fact, at its position in one brain's journal.
///
/// **Concept.** The journal is the harness's only source of truth. Every
/// fact the brain learns or decides is appended once, with the next
/// [`Seq`], in the same transaction as any effect it describes, and never
/// changed. Each fact belongs to exactly one identity. Every state anyone
/// reads (a task's state, the tree Max watches, his queue, whether a prefix
/// is warm, a day's cost) is a fold of the journal: [`crate::state::State`].
/// Nothing is recorded twice, so nothing can disagree with anything else,
/// and only the facts are a compatibility surface: every fold can be
/// rewritten by a self-modifying harness without migrating data.
///
/// What must be a fact: something becomes one when a participant acts on
/// it, recorded at the detail its consequences need. The model's
/// observation is kept exactly (the context it was sent, and what each
/// request newly included). Max's is kept as the journal position his face
/// had shown him. Streamed tokens are not facts ([`crate::live`]) until Max
/// interrupts, when what he watched is kept with the abandoned request.
///
/// **Vision.** "Strictly events + derived views" (`tech.md`); a brain that
/// "picks up where it left off" after a crash or a relaunch onto its own
/// new code; faces on several devices that resume exactly; analytics and
/// timesheets over session data; federated brains whose data stays "clear
/// where it came from".
///
/// **Existing harnesses.** Codex keeps an append-only JSONL rollout per
/// thread and treats its SQLite as a rebuildable index
/// (`ref-codex/codex-rs/thread-store/src/local/mod.rs:135`); Pi and Oh My
/// Pi keep a JSONL entry tree per session; OpenCode and Goose keep mutable
/// session and message rows in SQLite and broadcast change events beside
/// them. Each records one conversation at a time, so nothing orders a spawn
/// against its child's first request, and none records what the environment
/// did between turns or what Max had seen when he acted.
///
/// **Different here.** One log per brain holds every participant's facts,
/// typed by the identity they belong to, with who appended each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub seq: Seq,
    pub at: Timestamp,
    pub by: Author,
    pub fact: Fact,
}

/// Who put a fact in the journal.
///
/// The brain, and the limbs it speaks for, append having seen everything
/// before. Only a person acts on a picture that may be out of date, so only
/// Max's facts say how far his face had got. That answers "did he say yes
/// before or after the agent changed its plan?", and lets his words be
/// rendered with a note when the agent has moved on since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Author {
    Brain,
    /// A limb reported it: a file changed, a call settled.
    Limb(LimbId),
    /// Max, through one face, having received the journal up to `seen`.
    Max {
        face: FaceId,
        seen: Seq,
    },
}

/// What a record says, and which identity it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fact {
    /// A brain started writing this journal.
    Launched(Launched),
    Task(TaskId, TaskEvent),
    ModelCall(ModelCallId, ModelCallEvent),
    Call(CallId, CallEvent),
    /// Made once; included by a later request of its task.
    Delivery(Delivery),
    /// A resource moved to a new version.
    Change(Change),
    /// A new version of an environment's definition.
    Environment(Environment),
    /// A new version of a limb's definition.
    Limb(Limb),
    /// What the brain currently knows of a limb's process and connection.
    Reach {
        limb: LimbId,
        reach: Reach,
    },
    /// The attention scheduler's (or Max's) whole new order of tasks.
    Ranking(Ranking),
    /// Which task one face shows Max.
    Focus(Focus),
}

impl Fact {
    /// The stored values this fact names, each of which must be stored
    /// before the fact is appended: the entries a task or request points
    /// into, and a call's full output.
    pub fn values(&self) -> Vec<Value> {
        let mut values = Vec::new();
        let mut entry = |id: EntryId| values.push(Value::Entry(id));
        match self {
            Fact::Task(_, TaskEvent::Created(created)) => entry(created.head),
            Fact::Task(
                _,
                TaskEvent::Moved(
                    Move::Handover { head, .. } | Move::Reseed { head, .. } | Move::Rewind { head },
                ),
            ) => entry(*head),
            Fact::ModelCall(_, ModelCallEvent::Sent(call)) => {
                entry(call.head);
                call.breakpoints.iter().for_each(|b| entry(b.at));
                for rendered in &call.renders {
                    if let Rendered::Note(Note::Boundary { from: Some(from) }) = rendered {
                        entry(*from);
                    }
                }
            }
            Fact::ModelCall(
                _,
                ModelCallEvent::Ended {
                    ending: Ending::Responded { entry: id, .. },
                    ..
                },
            ) => entry(*id),
            Fact::Call(_, CallEvent::Settled(outcome) | CallEvent::Late(outcome)) => {
                if let Outcome::Completed(Output {
                    full: Some(full), ..
                })
                | Outcome::Failed(Output {
                    full: Some(full), ..
                })
                | Outcome::Interrupted {
                    partial:
                        Some(Output {
                            full: Some(full), ..
                        }),
                    ..
                } = outcome
                {
                    values.push(Value::Content(*full));
                }
            }
            _ => {}
        }
        values
    }
}

/// An immutable value a fact names, by its content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Entry(EntryId),
    Content(ContentId),
}

/// The boundary between two writers of one journal.
///
/// After a crash, a relaunch onto a newer build, or the relaunch an agent
/// asked for (`by`: the call that asked, which the new brain settles, so a
/// self-upgrade has a real outcome). The journal has one writer at a time;
/// everything after this record was written by `build`, which is how an
/// operator links a stuck subtree to an upgrade and how a rolled-back build
/// knows which facts are newer than it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Launched {
    pub build: String,
    pub by: Option<CallId>,
}

/// A fact the journal refuses, because accepting it would break the model:
/// end something twice, leave a tool use unanswered, outlive an owner, or
/// address something that cannot hear it.
///
/// A rejection is a bug in the brain, never control flow. The brain does not
/// append the batch and rebuilds its state from the journal, which never
/// held the rejected fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    /// Not the next position.
    OutOfOrder { expected: Seq },
    /// Names an identity the journal has never seen.
    Unknown,
    /// Creates an identity that already exists.
    Exists,
    /// Addresses a task that is done, a call that has settled, or a scope
    /// that has closed.
    Finished,
    /// The identity is not in a state that accepts this fact.
    Busy,
    /// The task is draining, so it may start nothing new.
    Draining,
    /// A request that leaves a tool use of the previous response unanswered.
    Unanswered,
    /// A standing task outside Max's scope.
    WrongScope,
    /// A fork, resume or aside bound to another environment than its source,
    /// or an agent's call to a tool its binding does not route to that limb.
    WrongEnvironment,
    /// A fork or aside that does not start at its source's head.
    Detached,
    /// Routed to a task it does not concern.
    NotForTask,
}
