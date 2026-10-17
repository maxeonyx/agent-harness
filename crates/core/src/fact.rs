//! Facts: what the harness's places say, who said it, and why the brain
//! refuses what it refuses.

use serde::{Deserialize, Serialize};

use crate::attention::{Focus, Ranking};
use crate::call::CallEvent;
use crate::delivery::{Content, Delivery};
use crate::draft::DraftEvent;
use crate::environment::{Description, Environment, Limb, Reach};
use crate::id::{
    CallId, DeliveryId, EventId, LimbId, ModelCallId, PlaceId, Seq, TaskId, Time, Timestamp,
    ValueId,
};
use crate::model_call::ModelCallEvent;
use crate::resource::Change;
use crate::task::TaskEvent;
use crate::value::Bytes;

/// One fact as its decider folds it: where it took effect, the decider's
/// clock there, who stated it, and the fact's own identity.
///
/// **Concept.** The core's view of one fact of a
/// [`harness_substrate::Record`]. Every state anyone reads (a task's state,
/// the context tree, the tree Max watches, his queue, whether a prefix is
/// warm, a day's cost) is a fold of these: [`crate::state::State`]. Nothing
/// is recorded twice, so nothing can disagree with anything else, and only
/// the facts are a compatibility surface: every fold can be rewritten by a
/// self-modifying harness without migrating data.
///
/// What must be a fact: something becomes one when a participant acts on
/// it, recorded at the detail its consequences need. The model's
/// observation is kept exactly (the context it was sent, and what each
/// request newly included). Max's is kept as the view his face had shown
/// him ([`Fact::Seen`]). Streamed tokens are not facts ([`crate::live`])
/// until Max interrupts, when what he watched is kept with the abandoned
/// request.
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
/// **Different here.** Every participant is a place with its own records;
/// the brain includes them into one order of its own, and folds each fact
/// with who stated it, which it derives from where the record came from.
#[derive(Clone, Debug)]
pub struct Stated<'a> {
    pub seq: Seq,
    pub at: Timestamp,
    pub by: Author,
    pub fact: &'a Fact,
    /// The fact's identity, and the id of whatever it creates.
    pub event: EventId,
}

/// Who stated a fact, derived from the place its record came from: never
/// stated, so never forged.
///
/// The brain states what it decides and observes: its own place. A limb
/// states facts about its own calls and resources, and no others: the
/// place the brain last connected it as ([`crate::environment::Reach`]).
/// Max states his words, his calls, his answers to asks and his controls
/// over the tree, from a place the brain accepted as one of his faces
/// ([`Fact::Face`]). A record from any other place is refused
/// ([`Rejected::Author`]). Only a person acts on a picture that may be out
/// of date, so only Max's facts say what his face had shown him. That
/// answers "did he say yes before or after the agent changed its plan?",
/// and lets his words be rendered with a note when the agent has moved on
/// since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Author {
    Brain,
    Limb(LimbId),
    /// Max, through one face, having seen the brain's view as of `seen`
    /// (the record's [`Fact::Seen`]).
    Max {
        face: PlaceId,
        seen: Option<Time>,
    },
}

/// What a fact says, and which identity it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fact {
    /// A brain started writing.
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
    /// What a plugin limb offers and says about itself, on its current
    /// connection.
    Described {
        limb: LimbId,
        description: Description,
    },
    /// What the brain knows of its connection to a limb, and which place
    /// speaks for it.
    Reach {
        limb: LimbId,
        reach: Reach,
    },
    /// That a place is one of Max's faces: a tab or a phone the brain
    /// accepted. Stated by the brain.
    Face(PlaceId),
    /// The brain time the view on Max's screen was as of when he did what
    /// the rest of this record says. A face puts one in every record it acts
    /// in, so what he had seen is a fact, kept with the act.
    Seen(Time),
    /// Max's words before they are a delivery.
    Draft(DeliveryId, DraftEvent),
    /// The attention scheduler's (or Max's) whole new order of tasks.
    Ranking(Ranking),
    /// Which task the face stating it shows Max.
    Focus(Focus),
}

impl Fact {
    /// The values this fact names, each of which must be stored with it or
    /// before it. Written by full destructuring, so a field added to a fact
    /// cannot be missed here.
    pub fn values(&self) -> Vec<ValueId> {
        let mut values = Vec::new();
        match self {
            Fact::Launched(Launched { build: _, by: _ }) => {}
            Fact::Task(_, event) => event.values(&mut values),
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
            Fact::Face(_) | Fact::Seen(_) => {}
            Fact::Draft(_, event) => match event {
                DraftEvent::Text(text) | DraftEvent::Send { text } => values.push(*text),
                DraftEvent::Opened { session: _ } | DraftEvent::Discarded => {}
            },
            Fact::Ranking(Ranking { order: _ }) => {}
            Fact::Focus(Focus { task: _ }) => {}
        }
        values
    }
}

/// The boundary between two processes of one brain place.
///
/// After a crash, a relaunch onto a newer build, or the relaunch an agent
/// asked for (`by`: the call that asked, which the new brain settles, so a
/// self-upgrade has a real outcome). The brain has one process at a time;
/// everything it writes after this record was written by `build`, which is how an
/// operator links a stuck subtree to an upgrade and how a rolled-back build
/// knows which facts are newer than it. It is also the whole of restart:
/// what the earlier writer had in flight is what [`crate::state::State::todo`] says to
/// redo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Launched {
    pub build: String,
    pub by: Option<CallId>,
}

/// Facts one place states together, with the bytes they name: what a
/// record of the harness says ([`crate::schema::Harness`]).
///
/// **Concept.** The unit of atomicity: folded whole or refused whole. A response and the calls of its tool uses; a
/// spawn's dispatch and its members; a settlement, its output and the
/// changes the call made. Values travel with the facts that name them, from
/// whoever produced the bytes (the provider's response, a remote limb's
/// command output, Max's pasted image), into the one value store.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub values: Vec<Bytes>,
    pub facts: Vec<Fact>,
}

/// Why the brain refuses a fact, because accepting it would break the model:
/// end something twice, leave a tool use unanswered, move a context off the
/// tree, outlive an owner, or address something that cannot hear it.
///
/// Each variant is one rule, so whoever is refused (Max at his face, a limb,
/// an engineer reading a log) learns which rule fired, and a face can act on
/// one by matching it rather than its wording. A refusal of the brain's own
/// facts is a bug in the brain. A refusal of Max's or a limb's is a race the
/// rules answer (he stops a task that has just finished), and it stays in
/// band: the brain holds the refused record and says why in the
/// [`harness_substrate::Included`] that took it in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejected {
    /// Names an identity the brain has never seen.
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
    /// Its author may not state it, or it comes from a place that is no
    /// one's, or claims Max had seen a view the brain has not yet shown.
    Author,
    /// Creates something under an id its own event did not mint.
    Minted,
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
    /// his face offers an aside instead ([`crate::state::State::scoped`] says so first).
    Scoped,
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let said = match self {
            Rejected::Missing(value) => {
                return write!(f, "value {value} is neither stored nor sent");
            }
            Rejected::TooLarge(value) => {
                return write!(f, "value {value} is larger than the limit");
            }
            Rejected::Unknown => "names something the brain has never seen",
            Rejected::Exists => "creates something that already exists",
            Rejected::Finished => "addresses something already finished",
            Rejected::NotFinished => "resumes a task that has not finished",
            Rejected::Starting => "the task has not started: its limbs are still connecting",
            Rejected::Busy => {
                "not now: the task is thinking or acting, or the call is not at that point"
            }
            Rejected::Draining => "the task is draining and may start nothing new",
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
            Rejected::Minted => "creates something under an id its event did not mint",
            Rejected::Scoped => "the task is waiting on its children; talk to it through an aside",
            Rejected::Idle => {
                "the task would wait on nothing; mark it done, or say something with the change"
            }
        };
        f.write_str(said)
    }
}

/// Why the brain did not fold a record: which of its facts broke which rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// The index of the fact in its batch.
    pub fact: u32,
    pub why: Rejected,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fact {} refused: {}", self.fact, self.why)
    }
}
