//! A journal: one replica's append-only record of the facts it holds, the
//! one way facts get into it, and how a replica that decides answers one
//! that proposes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agent::AgentEvent;
use crate::attention::{Focus, Ranking};
use crate::call::CallEvent;
use crate::delivery::{Content, Delivery};
use crate::environment::{Description, Environment, Limb, Reach};
use crate::id::{AgentId, CallId, FaceId, LimbId, ModelCallId, Seq, Timestamp, ValueId};
use crate::model_call::ModelCallEvent;
use crate::resource::Change;
use crate::state::State;
use crate::value::Bytes;

/// One fact, at its position in one replica's journal.
///
/// **Concept.** There is no single journal. Every replica keeps its own
/// (the brain its durable one; a limb its record of each call; a face, Max's
/// actions and its copy of what the brain sent it), and each replica's state
/// is a fold of its own: [`crate::state::State`] is the brain's. A fact is
/// appended once, with that journal's next [`Seq`], in the same transaction
/// as the values it names, and never changed. Facts cross between journals
/// only two ways. A replica sends a decider what it proposes, and the decider
/// answers with a [`Decision`]: accepted into its journal, already there, or
/// refused, the refusal itself a fact. A reader receives the records of a
/// decider's journal it follows, in order: a face, the brain's.
///
/// Every state anyone reads (an agent's state, the context tree, the map
/// Max watches, his queue, whether a prefix is warm, a day's cost) is a fold
/// of a journal. Nothing is recorded twice, so nothing can disagree with
/// anything else, and only the facts are a compatibility surface: every fold
/// can be rewritten by a self-modifying harness without migrating data.
///
/// What must be a fact: something becomes one when a participant acts on
/// it, recorded at the detail its consequences need. The model's observation
/// is kept exactly (the context it was sent, and what each request newly
/// included). Max's is kept as the journal position his face had shown him.
/// A refusal is acted on (Max is shown it, a limb stops resending, an
/// operator finds a bug), so it is a fact too. Streamed tokens are not facts
/// ([`crate::live`]) until Max interrupts, when what he watched is kept with
/// the abandoned request.
///
/// **Vision.** "Strictly events + derived views" (`tech.md`); "every replica
/// has its own journal", with consistency causal and an invalid action kept
/// in band; a brain that "picks up where it left off" after a crash or a
/// relaunch onto its own new code; faces on several devices that resume
/// exactly; work and home brains that keep their own data and can be
/// connected; analytics and timesheets over session data, "clear where it
/// came from".
///
/// **Existing harnesses.** Codex keeps an append-only JSONL rollout per
/// thread and treats its SQLite as a rebuildable index
/// (`ref-codex/codex-rs/thread-store/src/local/mod.rs:135`); Pi and Oh My
/// Pi keep a JSONL entry tree per session; OpenCode and Goose keep mutable
/// session and message rows in SQLite and broadcast change events beside
/// them. Each records one conversation at a time, so nothing orders a spawn
/// against its child's first request; none records what the environment did
/// between turns or what Max had seen when he acted; and a rejected action
/// is an error message, never a record.
///
/// **Different here.** The brain's journal holds every participant's facts
/// about the objects it decides, typed by the object they belong to, with
/// who stated each, and with what it refused beside what it accepted.
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
///
/// `seen` is one position because every object has one decider: what Max
/// acts on is in one brain's journal, so the causal context of his act is
/// how far he had seen of that one journal. A face connected to two brains
/// sends each its own position.
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

/// What a record says, and which object it belongs to.
///
/// **Deciders.** Every fact is about one object, the identity its variant
/// names: an agent, a request, a call, a limb, an environment, a resource,
/// a face. Each object has one decider, the replica whose journal it was
/// created in, and only that replica accepts the facts that change its
/// course. A rule that needs coordination is a rule about one object
/// (refusing an agent's second ending, a call's second settlement, a member
/// joining a closed scope, whose decider is the spawn call's), and it
/// reads other objects only through what never reverts (an agent being
/// done or leaving, Max's words being heard, a call's owner), so a stale
/// read can refuse what is valid but never accept what is not. So no
/// sequencer orders everything: each object's decider orders its own, and
/// everything else merges as it arrives. Within one brain the decider is
/// that brain for everything it holds; work and home brains each decide
/// their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fact {
    /// A brain started writing this journal.
    Launched(Launched),
    Agent(AgentId, AgentEvent),
    ModelCall(ModelCallId, ModelCallEvent),
    Call(CallId, CallEvent),
    /// Made once; included by a later request of its agent.
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
    /// The attention scheduler's (or Max's) whole new order of agents.
    Ranking(Ranking),
    /// Which agent one face shows Max.
    Focus(Focus),
    /// A batch this journal's writer refused. Recorded in band, so Max's
    /// face shows what happened to his act, a limb stops resending, and a bug
    /// in the brain is a record rather than a crash. Its facts are what was
    /// proposed; the values they name are kept when they were sent, so
    /// Max's refused words are never lost, but need not be.
    Refused(Refusal),
}

impl Fact {
    /// The values this fact names, each of which must be stored with it or
    /// before it. Written by full destructuring, so a field added to a fact
    /// cannot be missed here.
    pub fn values(&self) -> Vec<ValueId> {
        let mut values = Vec::new();
        match self {
            Fact::Launched(Launched { build: _, by: _ }) => {}
            Fact::Agent(_, event) => event.values(&mut values),
            Fact::ModelCall(_, event) => event.values(&mut values),
            Fact::Call(_, event) => event.values(&mut values),
            Fact::Delivery(Delivery {
                id: _,
                to: _,
                content,
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
            Fact::Focus(Focus { face: _, agent: _ }) => {}
            Fact::Refused(Refusal {
                by: _,
                facts: _,
                index: _,
                why: _,
            }) => {}
        }
        values
    }
}

impl Fact {
    /// How long the fact is worth keeping. Written by full match, so a new
    /// fact must say.
    pub fn durability(&self) -> Durability {
        match self {
            Fact::ModelCall(_, ModelCallEvent::FirstByte) | Fact::Reach { .. } | Fact::Focus(_) => {
                Durability::Volatile
            }
            Fact::Described { .. } | Fact::Ranking(_) => Durability::Latest,
            Fact::Launched(_)
            | Fact::Agent(..)
            | Fact::ModelCall(..)
            | Fact::Call(..)
            | Fact::Delivery(_)
            | Fact::Change(_)
            | Fact::Environment(_)
            | Fact::Limb(_)
            | Fact::Refused(_) => Durability::History,
        }
    }
}

/// How long a fact is worth keeping: its durability class.
///
/// **Concept.** A journal's facts do not all deserve the same life.
/// `tech.md` lists forever, the session, the context and not durable, plus
/// "for some we store only a projection". Here the class is a property of
/// what the fact is, so it is decided once per kind of fact
/// ([`Fact::durability`]), not per record. A replica's own journal tier
/// does the rest: a face's journal is the session, and what is about one
/// context dies with it in the fold.
///
/// The store keeps every record today. What the classes allow, once the
/// brain starts from a snapshot (its [`State`] at a position, a cache any
/// build can discard and refold from history) and replay no longer re-checks
/// rules against what a `Latest` or `Volatile` fact said: a `Latest` record
/// is dropped once a newer one of the same key is behind a snapshot, and a
/// `Volatile` one is never written, so a click on a task or a reconnection
/// costs no disk and no startup time.
///
/// **Vision.** "Durability classes on facts", and startup and memory that
/// do not grow with all of history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Durability {
    /// Kept as long as the journal: what Max may come back to, what an agent
    /// was told, what it cost, what was refused.
    History,
    /// Superseded by the next fact of the same key: a limb's description,
    /// the ranking.
    Latest,
    /// Meaningful only to the process that stated it, and only while it
    /// runs: a first byte, a connection, where a face is looking.
    Volatile,
}

/// A batch the writer refused, kept as a fact.
///
/// **Concept.** An invalid action is still something that happened. The
/// replica where it happened keeps it in its own journal: a face that can
/// already tell Max's act is invalid keeps it there and never sends it on,
/// which is the partial mapping from a face's journal to the brain's. A
/// decider that refuses what reached it (a race the proposer could not
/// see: Max stops an agent that has just finished, a limb repeats an
/// outcome after the brain gave up) keeps the refusal in its own, so its
/// readers learn of it in order with everything else. A refusal of the
/// brain's own batch is a bug in the brain: the agent it concerns is
/// cancelled by the harness with the rule that fired, and everything else
/// carries on.
///
/// **Bx.** Shared: a claim that conflicts with what is recorded is kept
/// beside it rather than hidden, as Bx keeps every signed record of one
/// build. Not shared: Bx records claims and lets each consumer choose; here
/// one decider per object chooses at once, because what the harness does
/// next (a request, a command) cannot be undone afterwards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    /// Who proposed it.
    pub by: Author,
    /// What was proposed, in order.
    pub facts: Vec<Fact>,
    /// The fact that broke a rule.
    pub index: usize,
    pub why: Rejected,
}

/// What a decider did with a proposed batch ([`Journal::commit`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// Appended as these records.
    Accepted(Vec<Record>),
    /// Every fact is already in effect: a retry of something the journal
    /// holds, after a lost answer or a reconnection. Nothing is appended, and
    /// the proposer treats it as accepted.
    Duplicate,
    /// Refused whole; the refusal was appended as this record.
    Refused(Record),
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

/// Why a fact is refused, because accepting it would break the model: end
/// something twice, leave a tool use unanswered, move a context off the
/// tree, outlive an owner, or address something that cannot hear it.
///
/// Each variant is one rule, so whoever is refused (Max at his face, a limb,
/// an engineer reading a log) learns which rule fired, and a face can act on
/// one by matching it rather than its wording. It is part of a fact
/// ([`Refusal`]), so it is in band.
///
/// What deciding each takes (invariant confluence):
///
/// - **The object's decider**, because it reads the object's current
///   course, which a concurrent fact may change: `Finished`, `NotFinished`,
///   `Starting`, `Busy`, `Draining`, `Unanswered`, `Idle` and `Scoped` (one
///   agent or call); `WrongScope` and `CannotStart` (the scope, decided with
///   the spawn call that owns it); `Exists` for a name (the namespace);
///   `Unreached` (one brain's connection to a limb).
/// - **Any replica holding what the fact names**, because it reads only
///   definitions, values, the context tree and sets that only grow:
///   `Unknown` (at a replica that has not yet received the referent, wait
///   for it rather than refuse), `Exists` for an id, `WrongEnvironment`,
///   `Unoffered`, `Unroutable`, `ForMax`, `Ambiguous`, `Redefined`,
///   `Detached`, `NotForAgent`, `Unheard`, `Author`, `Missing`, `TooLarge`.
///   A face checks these before Max acts and keeps what fails in its own
///   journal.
/// - **One journal's own integrity**: `OutOfOrder`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejected {
    /// Not the next position.
    OutOfOrder { expected: Seq },
    /// Names an identity the journal has never seen.
    Unknown,
    /// Creates an identity that already exists, a second environment or
    /// limb of one name, or runs a call the brain runs a second time.
    Exists,
    /// Addresses an agent that is done, a call that has settled, or a scope
    /// that has closed.
    Finished,
    /// Resumes an agent that has not finished.
    NotFinished,
    /// The agent has no context yet: its limbs are still connecting.
    Starting,
    /// Not at a point that accepts this: the agent is thinking or acting and
    /// this waits for its move, a spawn's scope still has members running,
    /// or a call was never sent.
    Busy,
    /// The agent is draining: interrupted, it starts nothing new this turn;
    /// leaving, it starts nothing that would outlive it and hears nothing
    /// but its tidy-up prompt.
    Draining,
    /// A request that leaves a tool use of the previous response unanswered.
    Unanswered,
    /// A standing agent outside Max's scope, an agent joining a scope its
    /// creator's call cannot add to, or an aside outside the scope its
    /// source is waiting on.
    WrongScope,
    /// An agent created by something other than a running agent's meta call,
    /// or something its creator cannot start: a fresh agent in an environment
    /// its own does not start, a fork of another agent, a resume of an agent it
    /// did not own, an aside or a standing agent.
    CannotStart,
    /// A route to a limb outside its agent's environment and places, an
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
    /// A head, breakpoint or rewind that is not on the agent's chain.
    Detached,
    /// Routed to an agent or limb it does not concern.
    NotForAgent,
    /// A quote that is not Max's words heard by the agent quoting them.
    Unheard,
    /// Its author may not state it.
    Author,
    /// Names a value that is neither stored nor in its batch.
    Missing(ValueId),
    /// Names a value in its batch larger than [`Bytes::LIMIT`].
    TooLarge(ValueId),
    /// The change would leave an autonomous agent waiting on nothing, so it
    /// would never move again: mark it done ([`crate::delivery::Prompt::Done`]),
    /// or say something with the change.
    Idle,
    /// An interrupt to an agent whose round waits on a scope with members
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
            Rejected::NotFinished => "resumes an agent that has not finished",
            Rejected::Starting => "the agent has not started: its limbs are still connecting",
            Rejected::Busy => {
                "not now: the agent is thinking or acting, or the call is not at that point"
            }
            Rejected::Draining => "the agent is interrupted or leaving, and may not start that",
            Rejected::Unanswered => "leaves a tool use unanswered",
            Rejected::WrongScope => "not allowed in that scope",
            Rejected::CannotStart => "its creator cannot start that",
            Rejected::WrongEnvironment => "outside the agent's environment",
            Rejected::Unoffered => "the limb does not offer that tool to that caller",
            Rejected::Unroutable => "no limb runs that tool in its context",
            Rejected::ForMax => "a question for Max is answered by him, never sent",
            Rejected::Unreached => "the limb is not connected",
            Rejected::Ambiguous => "one name used twice",
            Rejected::Redefined => "a limb's new version changes its kind",
            Rejected::Detached => "not on the agent's chain",
            Rejected::NotForAgent => "does not concern what it is routed to",
            Rejected::Unheard => "quotes words Max did not say to the quoting agent",
            Rejected::Author => "its author may not state it",
            Rejected::Scoped => "the agent is waiting on its children; talk to it through an aside",
            Rejected::Idle => {
                "the agent would wait on nothing; mark it done, or say something with the change"
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

/// A journal's one writer, and the decider for every object in it: its
/// store and its fold, kept in step.
///
/// **Concept.** Every fact enters through [`Journal::commit`], whoever
/// proposed it. A retry of what is already in effect is answered as a
/// duplicate and appends nothing. Otherwise positions and the time are
/// stamped, every value a fact names is checked to be stored or in the
/// batch, the batch is folded into a copy of the state, and the batch is
/// appended; or, if any fact breaks a rule, the refusal is appended instead
/// and the batch leaves nothing else behind. Either way the proposer gets a
/// [`Decision`] and every reader sees the same history, so the brain answers
/// a race with Max or a limb from the same rules that keep the journal
/// sound, and nothing else in the harness assigns a position, folds ahead
/// of the store, or appends without folding.
///
/// A batch folds into a copy of the state, and the copy replaces it only
/// once the store has the batch. [`State`] is persistent, so the copy costs
/// nothing until it diverges.
pub struct Journal<S> {
    store: S,
    state: State,
}

/// Why a commit or an open failed: never a rule, which is a [`Decision`].
#[derive(Debug)]
pub enum Error<E> {
    /// The store failed; nothing was appended.
    Store(E),
    /// A stored record does not fold: the journal was written by a model
    /// this build cannot read.
    Unreadable { seq: Seq, why: Rejected },
    /// A refusal could not itself be recorded: a bug in the model.
    Unrecorded(Rejected),
}

impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Store(error) => write!(f, "journal store: {error}"),
            Error::Unreadable { seq, why } => {
                write!(f, "journal record {} does not fold: {why}", seq.0)
            }
            Error::Unrecorded(why) => write!(f, "a refusal does not fold: {why}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Store(error) => Some(error),
            Error::Unreadable { .. } | Error::Unrecorded(_) => None,
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

    /// Decide `batch`, proposed by `by` at `at`: append it as the next
    /// records, answer that it is already in effect, or append its refusal.
    pub fn commit(
        &mut self,
        by: Author,
        batch: Batch,
        at: Timestamp,
    ) -> Result<Decision, Error<S::Error>> {
        let Batch { values, facts } = batch;
        if !facts.is_empty() && facts.iter().all(|f| self.state.in_effect(f, &by)) {
            return Ok(Decision::Duplicate);
        }
        let records = self.stamp(&by, facts, at);
        let refused = match self.unstored(&values, &records).map_err(Error::Store)? {
            Some(refused) => refused,
            None => match self.state.with(&records) {
                Ok(next) => {
                    self.store.append(&values, &records).map_err(Error::Store)?;
                    self.state = next;
                    return Ok(Decision::Accepted(records));
                }
                Err(refused) => refused,
            },
        };
        let (index, why) = refused;
        let refusal = Refusal {
            by,
            facts: records.into_iter().map(|r| r.fact).collect(),
            index,
            why,
        };
        let record = Record {
            seq: self.state.next(),
            at,
            by: Author::Brain,
            fact: Fact::Refused(refusal),
        };
        let records = std::slice::from_ref(&record);
        let next = self
            .state
            .with(records)
            .map_err(|(_, why)| Error::Unrecorded(why))?;
        let kept: Vec<Bytes> = values
            .into_iter()
            .filter(|v| v.len() <= Bytes::LIMIT)
            .collect();
        self.store.append(&kept, records).map_err(Error::Store)?;
        self.state = next;
        Ok(Decision::Refused(record))
    }

    /// The facts as records at the next positions.
    fn stamp(&self, by: &Author, facts: Vec<Fact>, at: Timestamp) -> Vec<Record> {
        let first = self.state.next().0;
        facts
            .into_iter()
            .zip(first..)
            .map(|(fact, seq)| Record {
                seq: Seq(seq),
                at,
                by: by.clone(),
                fact,
            })
            .collect()
    }

    /// The first value a record names that is neither stored nor sent
    /// within the limit, if any.
    fn unstored(
        &self,
        values: &[Bytes],
        records: &[Record],
    ) -> Result<Option<(usize, Rejected)>, S::Error> {
        let given: BTreeMap<ValueId, usize> = values.iter().map(|v| (v.id(), v.len())).collect();
        for (index, record) in records.iter().enumerate() {
            for value in record.fact.values() {
                let why = match given.get(&value) {
                    Some(len) if *len > Bytes::LIMIT => Rejected::TooLarge(value),
                    Some(_) => continue,
                    None if self.store.has(value)? => continue,
                    None => Rejected::Missing(value),
                };
                return Ok(Some((index, why)));
            }
        }
        Ok(None)
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
