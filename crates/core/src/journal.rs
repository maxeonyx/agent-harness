//! The journal: one brain's append-only record of everything that happened,
//! and the one way facts get into it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attention::{Focus, Ranking};
use crate::call::CallEvent;
use crate::delivery::{Content, Delivery};
use crate::environment::{Description, Environment, Limb, Reach};
use crate::id::{CallId, FaceId, LimbId, ModelCallId, Seq, TaskId, Timestamp, ValueId};
use crate::model_call::ModelCallEvent;
use crate::resource::Change;
use crate::state::State;
use crate::task::TaskEvent;
use crate::value::Bytes;

/// One fact, at its position in one brain's journal.
///
/// **Concept.** The journal is the harness's only source of truth. Every
/// fact a participant states is appended once, with the next [`Seq`], in
/// the same transaction as the values it names, and never changed. Each
/// fact belongs to exactly one identity. Every state anyone reads (a task's
/// state, the context tree, the tree Max watches, his queue, whether a
/// prefix is warm, a day's cost) is a fold of the journal:
/// [`crate::state::State`]. Nothing is recorded twice, so nothing can
/// disagree with anything else, and only the facts are a compatibility
/// surface: every fold can be rewritten by a self-modifying harness without
/// migrating data.
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
/// typed by the identity they belong to, with who stated each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub seq: Seq,
    pub at: Timestamp,
    pub by: Author,
    pub fact: Fact,
}

/// Who stated a fact.
///
/// The brain states what it decides and observes. A limb states facts about
/// its own calls and resources, and no others: how a call it ran ended, that
/// a file it owns changed and which call changed it. Max states his words,
/// his calls, his answers to asks and his controls over the tree. Only a
/// person acts on a picture that may be out of date, so only Max's facts say
/// how far his face had got. That answers "did he say yes before or after
/// the agent changed its plan?", and lets his words be rendered with a note
/// when the agent has moved on since. What each author may state is checked
/// by the fold, so a confused limb cannot settle another limb's call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Author {
    Brain,
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
    /// The brain writing the journal is going away, and must have exited
    /// `within` milliseconds of this fact ([`crate::cancel`]): it was asked
    /// by a signal (Ctrl-C, `SIGTERM`). Its tasks are not stopped; it starts
    /// nothing new, lets requests in flight finish until the deadline and
    /// cuts them at it, and shuts its limbs down. Stated again with less time
    /// to make it harder. A launch with no shutdown before the next one
    /// ended in a crash.
    Shutdown {
        within: u64,
    },
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
    /// What a plugin limb offers and says about itself, on its current
    /// connection.
    Described {
        limb: LimbId,
        description: Description,
    },
    /// What the brain knows of its connection to a limb.
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
    /// The values this fact names, each of which must be stored with it or
    /// before it. Written by full destructuring, so a field added to a fact
    /// cannot be missed here.
    pub fn values(&self) -> Vec<ValueId> {
        let mut values = Vec::new();
        match self {
            Fact::Launched(Launched { build: _, by: _ }) | Fact::Shutdown { within: _ } => {}
            Fact::Task(_, event) => event.values(&mut values),
            Fact::ModelCall(_, event) => event.values(&mut values),
            Fact::Call(_, event) => event.values(&mut values),
            Fact::Delivery(Delivery {
                id: _,
                to: _,
                content,
                read: _,
            }) => match content {
                Content::Words(words) => values.push(*words),
                Content::Prompt(_) => {}
            },
            Fact::Change(Change {
                resource: _,
                version: _,
                by: _,
                content,
            }) => values.extend(*content),
            Fact::Environment(Environment {
                id: _,
                name: _,
                role,
                model: _,
                params,
                limbs: _,
                places: _,
                starts: _,
                source: _,
            }) => values.extend([*role, *params]),
            Fact::Limb(Limb {
                id: _,
                name: _,
                kind: _,
            }) => {}
            Fact::Described {
                limb: _,
                description: _,
            } => {}
            Fact::Reach { limb: _, reach: _ } => {}
            Fact::Ranking(Ranking { order: _ }) => {}
            Fact::Focus(Focus { face: _, task: _ }) => {}
        }
        values
    }
}

/// The boundary between two writers of one journal.
///
/// After a crash, a relaunch onto a newer build, or the relaunch an agent
/// asked for (`by`: the call that asked, which the new brain settles, so a
/// self-upgrade has a real outcome). The journal has one writer at a time;
/// everything after this record was written by `build`, which is how an
/// operator links a stuck subtree to an upgrade and how a rolled-back build
/// knows which facts are newer than it. It is also the whole of restart:
/// what the earlier writer had in flight is what [`State::todo`] says to
/// redo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Launched {
    pub build: String,
    pub by: Option<CallId>,
}

/// Facts one author states together, with the bytes they name.
///
/// **Concept.** The unit of atomicity: committed whole or not at all, with
/// consecutive positions. A response and the calls of its tool uses; a
/// spawn's dispatch and its members; a settlement, its output and the
/// changes the call made. Values travel with the facts that name them, from
/// whoever produced the bytes (the provider's response, a remote limb's
/// command output, Max's pasted image), into the one value store.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub values: Vec<Bytes>,
    pub facts: Vec<Fact>,
}

/// A fact the journal refuses, because accepting it would break the model:
/// end something twice, leave a tool use unanswered, move a context off the
/// tree, outlive an owner, or address something that cannot hear it.
///
/// Each variant is one rule, so whoever is refused (Max at his face, a limb,
/// an engineer reading a log) learns which rule fired, and a face can act on
/// one by matching it rather than its wording. A refusal of the brain's own
/// facts is a bug in the brain. A refusal of Max's or a limb's is a race the
/// rules answer: he stops a task that has just finished, a limb repeats a
/// settlement after reconnecting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    /// Not the next position.
    OutOfOrder { expected: Seq },
    /// Names an identity the journal has never seen.
    Unknown,
    /// Creates an identity that already exists, a second environment or
    /// limb of one name, or runs a call the brain runs a second time.
    Exists,
    /// Addresses a task that is done, a call that has settled, or a scope
    /// that has closed.
    Finished,
    /// Resumes a task that has not finished.
    NotFinished,
    /// The task has no context yet: its limbs are still connecting.
    Starting,
    /// Not at a point that accepts this: the task is thinking or acting and
    /// this waits for its move, a spawn's scope still has members running,
    /// or a call was never sent.
    Busy,
    /// The task is draining, so it may start nothing new.
    Draining,
    /// A stop of a member whose deadline is later than its owner's: nothing
    /// a task owns may outlive it.
    Late,
    /// A stop's expiry before its deadline.
    Early,
    /// A request that leaves a tool use of the previous response unanswered.
    Unanswered,
    /// A standing task outside Max's scope, a task joining a scope its
    /// creator's call cannot add to, or an aside outside the scope its
    /// source is waiting on.
    WrongScope,
    /// A task created by something other than a running agent's meta call,
    /// or something its creator cannot start: a fresh task in an environment
    /// its own does not start, a fork of another task, a resume of a task it
    /// did not own, an aside or a standing task.
    CannotStart,
    /// A route to a limb outside its task's environment and places, an
    /// attachment its context does not route, or a place left empty or
    /// filled by a limb of another kind.
    WrongEnvironment,
    /// A route, or a call of Max's, naming a tool its limb's current
    /// description does not offer that caller, or a limb that has not
    /// described itself.
    Unoffered,
    /// A dispatch of a call no limb runs: its tool is not in its context.
    Unroutable,
    /// A dispatch of a call to Max: he answers, nothing is sent.
    ForMax,
    /// A dispatch to, or a description from, a limb not connected to this
    /// brain.
    Unreached,
    /// A binding that routes one tool name to two limbs, or an environment
    /// with two places of one kind.
    Ambiguous,
    /// A new version of a limb that changes what kind of limb it is.
    Redefined,
    /// A head, breakpoint or rewind that is not on the task's chain.
    Detached,
    /// Routed to a task or limb it does not concern.
    NotForTask,
    /// A quote that is not Max's words heard by the task quoting them.
    Unheard,
    /// Its author may not state it.
    Author,
    /// Names a value that is neither stored nor in its batch.
    Missing(ValueId),
    /// Names a value in its batch larger than [`Bytes::LIMIT`].
    TooLarge(ValueId),
    /// The change would leave an autonomous task waiting on nothing, so it
    /// would never move again: mark it done ([`crate::delivery::Prompt::Done`]),
    /// or say something with the change.
    Idle,
    /// An interrupt to a task whose round waits on a scope with members
    /// still running: the move cannot be Max's until the scope closes, so
    /// his face offers an aside instead ([`State::scoped`] says so first).
    Scoped,
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let said = match self {
            Rejected::OutOfOrder { expected } => {
                return write!(f, "out of order: the next position is {}", expected.0);
            }
            Rejected::Missing(value) => {
                return write!(f, "value {value} is neither stored nor sent");
            }
            Rejected::TooLarge(value) => {
                return write!(f, "value {value} is larger than the limit");
            }
            Rejected::Unknown => "names something the journal has never seen",
            Rejected::Exists => "creates something that already exists",
            Rejected::Finished => "addresses something already finished",
            Rejected::NotFinished => "resumes a task that has not finished",
            Rejected::Starting => "the task has not started: its limbs are still connecting",
            Rejected::Busy => {
                "not now: the task is thinking or acting, or the call is not at that point"
            }
            Rejected::Draining => "the task is draining and may start nothing new",
            Rejected::Late => "a member's stop may not end later than its owner's",
            Rejected::Early => "the deadline has not come",
            Rejected::Unanswered => "leaves a tool use unanswered",
            Rejected::WrongScope => "not allowed in that scope",
            Rejected::CannotStart => "its creator cannot start that",
            Rejected::WrongEnvironment => "outside the task's environment",
            Rejected::Unoffered => "the limb does not offer that tool to that caller",
            Rejected::Unroutable => "no limb runs that tool in its context",
            Rejected::ForMax => "a question for Max is answered by him, never sent",
            Rejected::Unreached => "the limb is not connected",
            Rejected::Ambiguous => "one name used twice",
            Rejected::Redefined => "a limb's new version changes its kind",
            Rejected::Detached => "not on the task's chain",
            Rejected::NotForTask => "does not concern what it is routed to",
            Rejected::Unheard => "quotes words Max did not say to the quoting task",
            Rejected::Author => "its author may not state it",
            Rejected::Scoped => "the task is waiting on its children; talk to it through an aside",
            Rejected::Idle => {
                "the task would wait on nothing; mark it done, or say something with the change"
            }
        };
        f.write_str(said)
    }
}

/// Where a journal is kept, from the model's side: records in order, and
/// values by id.
pub trait Store {
    type Error;

    /// Every record, in order.
    fn records(&self, each: &mut dyn FnMut(Record)) -> Result<(), Self::Error>;

    /// Whether the value is stored.
    fn has(&self, value: ValueId) -> Result<bool, Self::Error>;

    /// Store the values (each under [`Bytes::id`], idempotently) and append
    /// the records, in one transaction.
    fn append(&mut self, values: &[Bytes], records: &[Record]) -> Result<(), Self::Error>;
}

/// The journal's one writer: its store and its fold, kept in step.
///
/// **Concept.** Every fact enters through [`Journal::commit`]: positions
/// and the time are stamped, every value a fact names is checked to be
/// stored or in the batch, each fact is folded, and only then is the batch
/// appended. A refused batch is never written and leaves the state as it
/// was, so the brain answers a race with Max or a limb from the same rules
/// that keep the journal sound, and nothing else in the harness assigns a
/// position, folds ahead of the store, or appends without folding.
///
/// A batch folds into a copy of the state, and the copy replaces it only
/// once the store has the batch. [`State`] is persistent, so the copy costs
/// nothing until it diverges: a refusal anywhere in any author's batch, or a
/// store that fails, leaves the state exactly as it was, with nothing to
/// rebuild.
pub struct Journal<S> {
    store: S,
    state: State,
}

/// Why a commit or an open failed.
#[derive(Debug)]
pub enum Error<E> {
    /// The fact at `index` of the batch would break the model; nothing was
    /// appended.
    Refused { index: usize, why: Rejected },
    /// The store failed; nothing was appended.
    Store(E),
    /// A stored record does not fold: the journal was written by a model
    /// this build cannot read.
    Unreadable { seq: Seq, why: Rejected },
}

impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Refused { index, why } => {
                write!(f, "fact {index} of the batch refused: {why}")
            }
            Error::Store(error) => write!(f, "journal store: {error}"),
            Error::Unreadable { seq, why } => {
                write!(f, "journal record {} does not fold: {why}", seq.0)
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Store(error) => Some(error),
            Error::Refused { .. } | Error::Unreadable { .. } => None,
        }
    }
}

impl<S: Store> Journal<S> {
    /// Fold everything the store holds.
    pub fn open(store: S) -> Result<Journal<S>, Error<S::Error>> {
        let state = fold(&store)?;
        Ok(Journal { store, state })
    }

    /// What the journal says is true now. Cloning it is cheap and the clone
    /// never changes, so a face in the same process reads the brain's state
    /// as of any commit without folding the journal a second time.
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Append `batch` as the next records, stated by `by` at `at`, or
    /// refuse it whole. Returns the records as appended.
    pub fn commit(
        &mut self,
        by: Author,
        batch: Batch,
        at: Timestamp,
    ) -> Result<Vec<Record>, Error<S::Error>> {
        let Batch { values, facts } = batch;
        let given: BTreeMap<ValueId, usize> = values.iter().map(|v| (v.id(), v.len())).collect();
        let first = self.state.next().0;
        let records: Vec<Record> = facts
            .into_iter()
            .zip(first..)
            .map(|(fact, seq)| Record {
                seq: Seq(seq),
                at,
                by: by.clone(),
                fact,
            })
            .collect();
        for (index, record) in records.iter().enumerate() {
            for value in record.fact.values() {
                let why = match given.get(&value) {
                    Some(len) if *len > Bytes::LIMIT => Rejected::TooLarge(value),
                    Some(_) => continue,
                    None if self.store.has(value).map_err(Error::Store)? => continue,
                    None => Rejected::Missing(value),
                };
                return Err(Error::Refused { index, why });
            }
        }
        let next = self
            .state
            .with(&records)
            .map_err(|(index, why)| Error::Refused { index, why })?;
        self.store.append(&values, &records).map_err(Error::Store)?;
        self.state = next;
        Ok(records)
    }
}

fn fold<S: Store>(store: &S) -> Result<State, Error<S::Error>> {
    let mut state = State::default();
    let mut unreadable = None;
    store
        .records(&mut |record| {
            if unreadable.is_none()
                && let Err(why) = state.apply(&record)
            {
                unreadable = Some((record.seq, why));
            }
        })
        .map_err(Error::Store)?;
    match unreadable {
        Some((seq, why)) => Err(Error::Unreadable { seq, why }),
        None => Ok(state),
    }
}
