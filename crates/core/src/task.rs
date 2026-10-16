//! Tasks: one agent's mandate over its whole life, and the agent loop as a
//! fold.

use std::collections::BTreeMap;

use imbl::{OrdMap, Vector};
use serde::{Deserialize, Serialize};

use crate::call::{CallEvent, Caller, Outcome as CallOutcome};
use crate::context::{EntryId, Message, Note, Rendered, Role, Root};
use crate::delivery::{Content, Delivery, Prompt};
use crate::environment::Binding;
use crate::id::{
    CallId, DeliveryId, EnvironmentId, LimbId, ModelCallId, TaskId, Timestamp, ValueId,
};
use crate::journal::{Author, Fact, Record, Rejected};
use crate::model_call::{Ending, ModelCallEvent, Stop};
use crate::resource::{Change, Observation, ResourceKey, Version};
use crate::state::State;

/// The fold of the facts about one task: everything the brain, a face or
/// the scheduler needs to know about it now.
///
/// **Concept.** A task is the unit of structured concurrency and the thing
/// Max watches, talks to, and is asked to attend to. It has a [`Mandate`],
/// a [`Scope`] that owns it, a [`Mode`] that says what the end of a turn
/// means, an [environment](crate::environment::Environment) that is its role
/// for life, the limbs that fill that environment's places (its own clone),
/// and a [`Life`]: starting, live in a [`Context`], or done. A task is an
/// identity; its context is a value. The head moves through the context tree
/// (a fork starts a task on its parent's response, a handover moves a task
/// to a new root, a resume starts a task on another's last entry, a rewind
/// moves it back), and none of that copies or edits a context.
///
/// Creation and the first root are separate facts. A spawn creates its
/// members in the batch that dispatches it, so a scope's membership is
/// atomic with the call that owns it; a fresh member is then
/// [`Life::Starting`] until every limb it can reach has connected and
/// described itself (a clone made, a process started), when the brain gives
/// it its first root ([`Move::Root`]).
///
/// Its life is folded from the facts that concern it (`Task::apply`, run
/// only by [`State::apply`]): its own events, its model calls, the
/// settlements of the calls it waits on, deliveries to it, and changes to
/// what it embodies. The life is never written down as a fact, so it cannot
/// disagree with its history; "what is it waiting on" and "is it waiting on
/// Max" are read off it. What only a live task has (a context, a phase, a
/// pending drain, Max owing it a word, its run of failed requests) lives in
/// [`Live`], so no reader re-proves that a running task has a context.
///
/// **Vision.** Structured concurrency (a parent waiting on children is
/// [`Phase::Acting`] on its spawn call, like waiting on any tool, and a task
/// is done only when nothing it started is still running and nothing sent
/// to it is unread); the A.1.3 problem (the mandate is data, so the harness
/// can state it at the fork, after each tool round and after a handover);
/// user-facing, autonomous and standing tasks, which differ only in
/// [`Mode`]; the throbber; restart (the life says what to redo); and Max's
/// words never lost (a pending delivery stops a turn from completing the
/// task, and survives a handover).
///
/// **Existing harnesses.** OpenCode, Codex and Oh My Pi each keep a session
/// or thread with a status the agent loop keeps current as it runs, and
/// start a subagent as a child session from a fresh prompt by default (Oh
/// My Pi: `ref-oh-my-pi/packages/coding-agent/src/prompts/system/subagent-user-prompt.md`;
/// Codex can instead fork a filtered copy). Pi's `durable` package comes
/// closest, with owned tasks and joins, but each task is a replaceable
/// record with a per-kind checkpoint that must be migrated. None treats the
/// mandate as data the harness can restate, and none folds status from
/// history.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: TaskId,
    /// Short and descriptive, growing with depth (`tk-prodsync-findfiles`).
    pub name: String,
    /// Its role, and everything it can reach. Fixed for life, and inherited
    /// by forks, resumes and asides: models do not reliably honour a role
    /// that changes mid-context. A new version of the environment reaches
    /// it at its next root.
    pub environment: EnvironmentId,
    /// The limb filling each of its environment's
    /// [places](crate::environment::Environment::places), by plugin kind.
    /// Fixed for life, and shared by its forks.
    pub places: BTreeMap<String, LimbId>,
    pub scope: Scope,
    pub origin: Origin,
    /// The meta call that created it; `None` when Max did. Whoever made
    /// that call gave the mandate ([`State::call_owner`]).
    pub by: Option<CallId>,
    pub mode: Mode,
    pub mandate: Mandate,
    /// No new requests under this task until released; running work carries
    /// on. Max pausing a subtree, or the brain after a long outage.
    pub held: bool,
    /// When its life or phase last changed: how long the throbber has shown
    /// what it shows.
    pub since: Timestamp,
    /// Inputs no request has rendered yet. A turn cannot complete over one
    /// that [wakes](Input::wakes). They are for the task, not its context, so
    /// they survive a new root and ride into a resume.
    pub pending: Vec<Input>,
    /// Calls it started in the background that have not settled. It cannot
    /// finish while one runs, and each one's settlement wakes it.
    pub background: Vec<CallId>,
    /// Max's actions in its chat that have not settled. Each settlement
    /// rides into its next request without waking it.
    pub max_actions: Vec<CallId>,
    pub life: Life,
}

/// Where a task is in its life. Exactly one holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Life {
    /// Created, with no context yet: some limb it can reach has not
    /// connected or described itself (its clone is being made). The brain
    /// gives it its first root once every one has
    /// ([`crate::state::Todo::Root`]). Max can stop it; it cannot be
    /// interrupted, forked, resumed or rewound, and makes no calls.
    Starting,
    Live(Live),
    /// Finished. Its context is kept for a resume and for Max to read; a task
    /// stopped while starting never had one.
    Done {
        outcome: Outcome,
        context: Option<Context>,
    },
}

/// What a live task has, and only a live task.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub context: Context,
    pub phase: Phase,
    /// Set when the task must stop doing what it is doing. No request is
    /// sent while it is set. It is pending only while something is
    /// outstanding (a request, a round, background work after a stop);
    /// otherwise it takes effect as it arrives.
    pub drain: Option<Drain>,
    /// Since when Max has owed this task a word: set when a user-facing turn
    /// ends or Max interrupts, cleared by his words or by a change of mode.
    /// It survives the harness's own turns (a handover before the cache
    /// expires), so the task keeps its place in his queue through them.
    pub owed_since: Option<Timestamp>,
    /// Requests that failed in a row; the brain's policy gives up at some
    /// count ([`Drain::Fail`]). The account's limit is not a failure.
    pub failures: u32,
}

impl Live {
    fn new(context: Context) -> Live {
        Live {
            context,
            phase: Phase::Ready,
            drain: None,
            owed_since: None,
            failures: 0,
        }
    }
}

/// Whose move it is in a live task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The task's move: the brain builds and sends its next request, unless
    /// it is held or the account is limited (derived, not stored here).
    Ready,
    /// A request is in flight. The calls of its tool uses are made with its
    /// response and recorded on the request.
    Thinking(ModelCallId),
    /// The calls of one round are outstanding: a response's tool uses, or a
    /// fresh context's attachments. A spawn is one of them, so is an ask of
    /// Max.
    Acting(Step),
    /// Max's move: a user-facing turn ended, or he interrupted.
    AwaitingMax,
    /// The turn ended and the task waits for something that wakes it: a
    /// change to what a standing task embodies, or background work.
    Listening,
}

/// The context a task is working in, and everything that belongs to it
/// rather than to the task: where it began, where it ends now, which limb
/// runs each of its tools, and what it embodies. A new root replaces the
/// whole of it; Max's words and actions pending for the task survive, since
/// they are for the task.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// Where it began: its root, or the entry it was forked, resumed or
    /// opened at. A rewind cannot go before it.
    pub start: EntryId,
    pub head: EntryId,
    /// Which limb runs each of its tools. Made with its root and kept until
    /// the next one, so a redefined environment takes effect at the task's
    /// next root, never under it.
    pub binding: Binding,
    /// The version of each resource it embodies, and what it has been told
    /// since. Persistent, so a fork's copy shares it until either changes.
    pub knows: OrdMap<ResourceKey, Known>,
    /// What each request since `start` changed in `knows`, so a rewind
    /// restores what the earlier context embodied. Persistent, so folding a
    /// fact into a long-lived task does not copy its history.
    learned: Vector<Learned>,
}

impl Context {
    /// A new root's context.
    fn rooted(start: &Start) -> Context {
        Context {
            start: start.root.id(),
            head: start.root.id(),
            binding: start.binding.clone(),
            knows: start
                .observes
                .iter()
                .map(|o| {
                    let known = Known {
                        has: o.version,
                        told: None,
                    };
                    (o.resource.clone(), known)
                })
                .collect(),
            learned: Vector::new(),
        }
    }

    /// A context that continues this one from its head: a fork's, a
    /// resume's, an aside's.
    fn continued(&self) -> Context {
        Context {
            start: self.head,
            head: self.head,
            binding: self.binding.clone(),
            knows: self.knows.clone(),
            learned: Vector::new(),
        }
    }

    /// Whether `change` moves something this context embodies that it has
    /// not been told of.
    fn moved_by(&self, change: &Change) -> bool {
        self.knows
            .get(&change.resource)
            .is_some_and(|known| known.has != change.version && known.told != Some(change.version))
    }

    /// The context now embodies these versions.
    fn observe(
        &mut self,
        observations: &[Observation],
        before: &mut Vec<(ResourceKey, Option<Known>)>,
    ) {
        for o in observations {
            let old = self.knows.insert(
                o.resource.clone(),
                Known {
                    has: o.version,
                    told: None,
                },
            );
            if !before.iter().any(|(k, _)| *k == o.resource) {
                before.push((o.resource.clone(), old));
            }
        }
    }

    /// The context has been told these resources moved, without seeing them.
    fn tell(
        &mut self,
        observations: &[Observation],
        before: &mut Vec<(ResourceKey, Option<Known>)>,
    ) {
        for o in observations {
            if let Some(known) = self.knows.get_mut(&o.resource) {
                let old = *known;
                known.told = Some(o.version);
                if !before.iter().any(|(k, _)| *k == o.resource) {
                    before.push((o.resource.clone(), Some(old)));
                }
            }
        }
    }

    /// Move the head back to `to`, and embody again only what the context
    /// embodied there.
    fn rewind(&mut self, to: EntryId, world: &State) {
        self.head = to;
        while self
            .learned
            .last()
            .is_some_and(|l| !world.descends(to, l.head))
        {
            let Some(learned) = self.learned.pop_back() else {
                break;
            };
            for (resource, before) in learned.before.into_iter().rev() {
                match before {
                    Some(known) => self.knows.insert(resource, known),
                    None => self.knows.remove(&resource),
                };
            }
        }
    }
}

/// What a task's context knows of one resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    /// The version the context embodies.
    pub has: Version,
    /// The newest version it has been told of, if any: a notice already
    /// said "this changed after turn N", and need not say it again.
    pub told: Option<Version>,
}

/// What one request changed in what its context knows: each resource's
/// value before it.
#[derive(Clone, Debug, PartialEq)]
struct Learned {
    head: EntryId,
    before: Vec<(ResourceKey, Option<Known>)>,
}

/// Who owns a task: the nursery it lives in.
///
/// A scope is not stored as an identity of its own. A spawn call opens one
/// and owns it, and its members are the tasks whose scope it is. The call
/// settles, with every member's outcome, when the last member is done, and
/// that settlement is the scope's close. Members can add siblings while they
/// run, so the moment the last one finishes nobody is left who could add
/// another: completion and growth cannot race. No task outlives its scope
/// and no scope outlives its call.
///
/// A member's failure is one of the outcomes its parent reads, not an
/// exception that cancels its siblings: an agent's failure is information
/// the parent weighs (Trio's nurseries, without cancel-on-first-error).
///
/// **Existing harnesses.** OpenCode's `subagent` tool blocks by default, one
/// level deep (`ref-opencode/packages/core/src/tool/plugin/subagent.ts:129`);
/// Oh My Pi's `task` and Codex's `spawn_agent` return at once and deliver
/// results later (`ref-oh-my-pi/packages/coding-agent/src/tools/settings.ts:892`);
/// Goose's `delegate` cannot nest; Pi ships none. In none can a child add a
/// sibling, and an asynchronous child can outlive the turn that started it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Max's own scope. It never closes: the tasks he starts live in it, and
    /// so do asides, resumes and standing tasks such as the attention
    /// scheduler.
    Max,
    /// The scope the spawn call opened.
    Spawn(CallId),
}

/// How a task came to be. It decides its environment, where its first
/// context starts, how the first request is framed, and how the tree shows
/// the task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// A new task in an environment, with a limb for each of its places. It
    /// starts on a new root ([`Move::Root`]), then its attachments, its
    /// shared brief and its own mandate. Created by an agent, only in an
    /// environment its own [starts](crate::environment::Environment::starts).
    Fresh {
        environment: EnvironmentId,
        /// By plugin kind, one for each of the environment's places.
        places: BTreeMap<String, LimbId>,
    },
    /// The parent's spawning response, the source's head when the fork is
    /// made. The first request answers every tool use in it with the same
    /// neutral result, its own spawn included, so no fork waits on its
    /// parent's other work; the fork's mandate follows as user text in that
    /// message. The fork-framing spike measured this: a mandate put in a
    /// tool result does not override the identity Sonnet has from its
    /// parent's context, and user text does.
    Fork { from: TaskId },
    /// A finished task's last entry, continued with a new mandate. It
    /// inherits what its source had not yet rendered, such as Max's edits
    /// in the finished chat. Max resumes any finished task; an agent resumes
    /// a task of a scope it owned, so an orchestrator can ask its designer a
    /// follow-up with the designer's context intact.
    Resume { from: TaskId },
    /// A busy task's head, opened by Max as a user-facing fork so he can talk
    /// to it while it waits. Each time its turn ends, what it last said is
    /// delivered back to `from` ([`Content::Aside`]), so what Max said there
    /// reaches the task even if he never marks the aside done.
    Aside { from: TaskId },
}

/// A new root, and what is decided with it: a fresh task's first context,
/// a handover's, a rebuild's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    pub root: Root,
    /// Made with the root's tools ([`Binding::new`]).
    pub binding: Binding,
    /// The versions the root embodies: the environment's source, each
    /// limb's instructions (an `AGENTS.md`, the skills index), each tool's
    /// schema.
    pub observes: Vec<Observation>,
    /// Attachments to run before the first request, shown to the task as
    /// tool calls it made: calls made by the harness ([`Caller::Meta`]),
    /// shared by every member that names them, so N siblings share one read
    /// and one cache prefix.
    pub awaits: Vec<CallId>,
}

impl Start {
    fn values(&self, out: &mut Vec<ValueId>) {
        let Start {
            root,
            binding: _,
            observes: _,
            awaits: _,
        } = self;
        let Root {
            model: _,
            system,
            tools,
        } = root;
        out.push(*system);
        out.push(*tools);
    }
}

/// What the end of a model turn means for this task.
///
/// The one difference between an autonomous subagent, a chat with Max, and
/// a standing agent such as the attention scheduler. Max marking a
/// user-facing task done makes it autonomous for one last turn, which writes
/// the result (with [`crate::delivery::Prompt::Done`], so the model has
/// seen it before it does).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// The end of a turn completes the task, once nothing it started is
    /// still running; its last message is the result.
    Autonomous,
    /// The end of a turn gives the move to Max.
    UserFacing,
    /// The end of a turn waits for something its context embodies to change,
    /// or a word from Max. Only in Max's scope, since a scope holding one
    /// would never close.
    Standing,
}

/// What a task was asked to do.
///
/// The chain of mandates up the owner chain is the task's place in the
/// overall work: "A, then A.1, then A.1.3". Rendering that chain at a fork,
/// as a reminder after each tool round, and at the top of a handover is how
/// a deeply forked agent knows to stop after A.1.3.
///
/// A mandate Max gives is his words: the journal counts its text among the
/// words he has said, heard by the task he gave it to, so the task can quote
/// it on ([`State::heard`]). Who gave any other mandate is the caller of the
/// meta call that created the task ([`Task::by`]).
///
/// A fresh task's mandate also carries what every member of its spawn
/// shares, rendered before its own text and identical for each of them, so
/// N siblings are one cache prefix: the parent's brief, and Max's words the
/// parent quotes. A quote is the value of his words itself, so a brief
/// carries them verbatim by construction, the same words have the same
/// label wherever they are quoted, and the child can tell them from its
/// parent's paraphrase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    pub text: ValueId,
    /// What every member of the spawn is told first.
    pub brief: Option<ValueId>,
    /// Max's words, each one the creating task has heard: words he sent it,
    /// the mandate he gave it, his answer to its question, or a quote in its
    /// own mandate.
    pub quotes: Vec<ValueId>,
}

/// An input waiting for a request to render it. Whether it ends a wait is
/// what it is, never a flag beside it: only a few things drive a request
/// (`context-and-agent-loop.md`): the agent's own tool loop, Max's words, a
/// harness prompt, work the agent asked to hear back from, and for a
/// standing task a change to what it embodies. A round's results and Max's
/// actions in the chat ride on the next request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A settled call of the last round. The next request must render every
    /// one, since each tool use needs its result first.
    Result(CallId),
    /// A background call the task started has settled. Wakes it.
    Background(CallId),
    /// The real outcome of a call first settled as unknown. Wakes it.
    Late(CallId),
    /// One of Max's actions in its chat has settled. Rides along.
    MaxAction(CallId),
    /// Max's words, a turn the harness asks for, or an aside's return.
    /// Wakes it.
    Delivery(DeliveryId),
}

impl Input {
    /// Whether it ends a wait, and keeps a turn from completing the task.
    pub fn wakes(&self) -> bool {
        match self {
            Input::Background(_) | Input::Late(_) | Input::Delivery(_) => true,
            Input::Result(_) | Input::MaxAction(_) => false,
        }
    }

    fn is(&self, rendered: &Rendered) -> bool {
        match (self, rendered) {
            (
                Input::Result(a) | Input::Background(a) | Input::Late(a) | Input::MaxAction(a),
                Rendered::Call(b),
            ) => a == b,
            (Input::Delivery(a), Rendered::Delivery(b)) => a == b,
            _ => false,
        }
    }
}

/// The outstanding calls of one round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The response whose tool uses these are; `None` for a fresh context's
    /// attachments, whose tool uses the harness writes when it renders them.
    pub response: Option<EntryId>,
    pub outstanding: Vec<CallId>,
}

/// What a task does once nothing is outstanding, instead of carrying on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drain {
    /// Max interrupted: the move becomes his, whatever the mode. It never
    /// passes through a spawn call (that would stop the scope), so it is
    /// refused while the task's round waits on a scope with members still
    /// running ([`Rejected::Scoped`], and [`State::scoped`] beforehand); to
    /// talk to it then, Max opens an aside.
    ToMax,
    /// The task is cancelled, and everything it owns with it.
    Stop(Stopper),
    /// The brain cannot carry the task on: the provider rejects its context,
    /// the model refused, a limb it needs cannot be made, retries are
    /// exhausted.
    Fail(String),
}

/// Who stopped a task: only Max, or the stopping of its owner. No agent can
/// stop a task it does not own, and a parent waiting on its scope makes no
/// calls at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stopper {
    Max,
    /// The task that owns its scope is stopping.
    Owner,
}

/// How a task ended. Each reaches its owner as this member's result; a
/// cancellation says who, so a parent does not relaunch what Max stopped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Its last message, the assistant entry `entry`.
    Result {
        entry: EntryId,
    },
    Failed {
        reason: String,
    },
    Cancelled {
        by: Stopper,
    },
}

/// A fact only the task itself owns. Everything else that moves it is a
/// fact of another identity, folded in by `Task::apply`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Created(Box<Created>),
    ModeSet(Mode),
    Renamed(String),
    Moved(Move),
    Drain(Drain),
    Held(bool),
}

impl TaskEvent {
    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        match self {
            TaskEvent::Created(created) => {
                let Created {
                    name: _,
                    by: _,
                    scope: _,
                    origin: _,
                    mode: _,
                    mandate,
                } = &**created;
                let Mandate {
                    text,
                    brief,
                    quotes,
                } = mandate;
                out.push(*text);
                out.extend(*brief);
                out.extend(quotes);
            }
            TaskEvent::Moved(Move::Handover { call: _, start } | Move::Root(start)) => {
                start.values(out);
            }
            TaskEvent::Moved(Move::Rewind { to: _ })
            | TaskEvent::ModeSet(_)
            | TaskEvent::Renamed(_)
            | TaskEvent::Drain(_)
            | TaskEvent::Held(_) => {}
        }
    }
}

/// A task begins. Its environment and places, and for a fork, resume or
/// aside its context, follow from its origin.
///
/// Who may create one is checked by the journal. Max creates in his own
/// scope, with `by` empty, and his mandate is his words. The brain creates
/// only as the effect of an agent's meta call, `by`: in the scope that call
/// opens, or as a sibling in its caller's own spawn scope, and only what the
/// caller's environment can start. So the tree records who started each
/// task, and the brain cannot put a task in Max's scope that he did not
/// start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub name: String,
    /// The meta call that created it; `None` when Max did.
    pub by: Option<CallId>,
    pub scope: Scope,
    pub origin: Origin,
    pub mode: Mode,
    pub mandate: Mandate,
}

/// The head moved to an entry that does not extend it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Move {
    /// To a new root, written by the task's own handover call: once that
    /// call has settled with the seed and its round is over, the next
    /// request starts the new context instead of continuing the old one,
    /// which ends on it. The root is rendered from the environment's
    /// current version, which is where a rebuild picks up a changed
    /// AGENTS.md, role, model or tool set.
    Handover { call: CallId, start: Box<Start> },
    /// To a new root the harness built without the agent's help: a starting
    /// task's first, once its limbs have described themselves; or a boundary
    /// note and the mandate, for a context the provider will no longer
    /// accept, or a task with no handover tool (the scheduler, whose memory
    /// is the queue it observes).
    Root(Box<Start>),
    /// Back to a response on the task's own chain since its start that
    /// ended a model turn, so the next message is Max's. The abandoned
    /// branch stays in the tree, and the context embodies again only what
    /// it embodied there.
    Rewind { to: EntryId },
}

impl Task {
    /// The task its creation describes, reading its source from the
    /// journal's state. A fresh task starts with no context; any other
    /// continues its source's.
    pub(crate) fn new(
        id: TaskId,
        created: &Created,
        at: Timestamp,
        world: &State,
    ) -> Result<Task, Rejected> {
        let mut pending = Vec::new();
        let mut max_actions = Vec::new();
        let (environment, places, life) = match &created.origin {
            Origin::Fresh {
                environment,
                places,
            } => (*environment, places.clone(), Life::Starting),
            Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } => {
                let source = world.task(*from).ok_or(Rejected::Unknown)?;
                let resume = matches!(created.origin, Origin::Resume { .. });
                match (resume, source.is_done()) {
                    (true, false) => return Err(Rejected::NotFinished),
                    (false, true) => return Err(Rejected::Finished),
                    _ => {}
                }
                let context = source.context().ok_or(Rejected::Starting)?;
                if resume {
                    pending = source.pending.clone();
                    max_actions = source.max_actions.clone();
                }
                let life = Life::Live(Live::new(context.continued()));
                (source.environment, source.places.clone(), life)
            }
        };
        Ok(Task {
            id,
            name: created.name.clone(),
            environment,
            places,
            scope: created.scope,
            origin: created.origin.clone(),
            by: created.by,
            mode: created.mode,
            mandate: created.mandate.clone(),
            held: false,
            since: at,
            pending,
            background: Vec::new(),
            max_actions,
            life,
        })
    }

    /// Its live part, once it has started and until it is done.
    pub fn live(&self) -> Option<&Live> {
        match &self.life {
            Life::Live(live) => Some(live),
            Life::Starting | Life::Done { .. } => None,
        }
    }

    /// Its phase, while it is live.
    pub fn phase(&self) -> Option<&Phase> {
        self.live().map(|l| &l.phase)
    }

    /// The context it works in, or finished in.
    pub fn context(&self) -> Option<&Context> {
        match &self.life {
            Life::Live(live) => Some(&live.context),
            Life::Done { context, .. } => context.as_ref(),
            Life::Starting => None,
        }
    }

    /// Where its context ends now, once it has one.
    pub fn head(&self) -> Option<EntryId> {
        self.context().map(|c| c.head)
    }

    pub fn is_done(&self) -> bool {
        matches!(self.life, Life::Done { .. })
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        match &self.life {
            Life::Done { outcome, .. } => Some(outcome),
            Life::Starting | Life::Live(_) => None,
        }
    }

    /// The drain pending on it, if any.
    pub fn drain(&self) -> Option<&Drain> {
        self.live().and_then(|l| l.drain.as_ref())
    }

    /// Since when Max has owed it a word, if he does.
    pub fn owed_since(&self) -> Option<Timestamp> {
        self.live().and_then(|l| l.owed_since)
    }

    /// The calls whose settlement its fold reads: its round's, its
    /// background work, and Max's actions in its chat. This is the whole of
    /// "who reads a call", derived rather than indexed.
    pub fn awaited(&self) -> impl Iterator<Item = CallId> + '_ {
        let round = match self.phase() {
            Some(Phase::Acting(step)) => step.outstanding.as_slice(),
            _ => &[],
        };
        round
            .iter()
            .chain(&self.background)
            .chain(&self.max_actions)
            .copied()
    }

    pub fn awaits(&self, call: CallId) -> bool {
        self.awaited().any(|c| c == call)
    }

    fn live_mut(&mut self) -> Result<&mut Live, Rejected> {
        match &mut self.life {
            Life::Live(live) => Ok(live),
            Life::Starting => Err(Rejected::Starting),
            Life::Done { .. } => Err(Rejected::Finished),
        }
    }

    /// Fold one fact that concerns this task, or refuse it. The caller folds
    /// into a copy and keeps it only on success.
    ///
    /// This is the whole agent loop. Three properties hold by construction:
    /// no request is sent while a drain is pending or a tool use of the last
    /// response is unanswered; a task is done only when nothing it started
    /// is outstanding and nothing waking is unread; and Max's words are never
    /// lost, to a turn that ends first or to a handover.
    pub(crate) fn apply(&mut self, record: &Record, world: &State) -> Result<(), Rejected> {
        let at = record.at;
        if self.is_done() {
            return self.after_done(record);
        }
        match &record.fact {
            Fact::Task(id, event) if *id == self.id => self.event(event, at, world),
            Fact::ModelCall(id, event) => self.model_call(*id, event, at, world),
            Fact::Call(id, event) => self.call(*id, event, at, world),
            Fact::Delivery(delivery) if delivery.to == self.id => {
                self.deliver(delivery, &record.by, at);
                Ok(())
            }
            Fact::Change(change) => {
                // Read off the change itself, so the fold of a change never
                // depends on whether the world has recorded it yet.
                let wakes = self.mode == Mode::Standing
                    && self.live().is_some_and(|l| {
                        l.phase == Phase::Listening
                            && l.drain.is_none()
                            && l.context.moved_by(change)
                    });
                if wakes {
                    self.enter(Phase::Ready, at);
                }
                Ok(())
            }
            _ => Err(Rejected::NotForTask),
        }
    }

    /// A finished task folds only Max's actions in its chat, which ride into
    /// a resume of it. Provisional: Max's choice between that and refusing
    /// them is open.
    fn after_done(&mut self, record: &Record) -> Result<(), Rejected> {
        match &record.fact {
            Fact::Call(id, CallEvent::Made(call)) if matches!(call.caller, Caller::Max { chat: Some(t), .. } if t == self.id) =>
            {
                self.max_actions.push(*id);
                Ok(())
            }
            Fact::Call(id, CallEvent::Settled(_)) => {
                let i = self
                    .max_actions
                    .iter()
                    .position(|c| c == id)
                    .ok_or(Rejected::Finished)?;
                self.max_actions.remove(i);
                self.pending.push(Input::MaxAction(*id));
                Ok(())
            }
            _ => Err(Rejected::Finished),
        }
    }

    fn event(&mut self, event: &TaskEvent, at: Timestamp, world: &State) -> Result<(), Rejected> {
        match event {
            TaskEvent::Created(_) => return Err(Rejected::Exists),
            TaskEvent::ModeSet(mode) => {
                if *mode == Mode::Standing && self.scope != Scope::Max {
                    return Err(Rejected::WrongScope);
                }
                self.mode = *mode;
                if let Life::Live(live) = &mut self.life {
                    if *mode != Mode::UserFacing {
                        live.owed_since = None;
                    }
                    // A waiting task waits for what its new mode waits for.
                    // It never completes here: the model has not seen the
                    // change.
                    if matches!(live.phase, Phase::AwaitingMax | Phase::Listening)
                        && live.drain.is_none()
                    {
                        self.wait(at, world);
                        // An autonomous task left waiting on nothing would
                        // never move again: mark it done instead, or say
                        // something with the change.
                        if *mode == Mode::Autonomous
                            && self.phase() == Some(&Phase::Listening)
                            && self.background.is_empty()
                        {
                            return Err(Rejected::Idle);
                        }
                    }
                }
            }
            TaskEvent::Renamed(name) => self.name = name.clone(),
            TaskEvent::Moved(Move::Handover { call, start }) => {
                let live = self.live_mut()?;
                if live.drain.is_some() {
                    return Err(Rejected::Draining);
                }
                if live.phase != Phase::Ready || !self.pending.contains(&Input::Result(*call)) {
                    return Err(Rejected::Busy);
                }
                self.root(start, at, world)?;
            }
            TaskEvent::Moved(Move::Root(start)) => {
                if let Life::Live(live) = &self.life {
                    if live.drain.is_some() {
                        return Err(Rejected::Draining);
                    }
                    if !waiting(&live.phase) {
                        return Err(Rejected::Busy);
                    }
                }
                self.root(start, at, world)?;
            }
            TaskEvent::Moved(Move::Rewind { to }) => {
                let live = self.live_mut()?;
                if !waiting(&live.phase) {
                    return Err(Rejected::Busy);
                }
                live.context.rewind(*to, world);
                self.drop_results();
                // The head is a response that ended a turn: the next word is Max's.
                self.await_max(at);
            }
            TaskEvent::Drain(drain) => match &mut self.life {
                Life::Starting => match drain {
                    Drain::ToMax => return Err(Rejected::Starting),
                    Drain::Stop(by) => self.finish(Outcome::Cancelled { by: *by }, at),
                    Drain::Fail(reason) => self.finish(
                        Outcome::Failed {
                            reason: reason.clone(),
                        },
                        at,
                    ),
                },
                Life::Live(live) => {
                    // An ending outranks an interrupt; the first ending stands.
                    if !ends(live.drain.as_ref()) {
                        live.drain = Some(drain.clone());
                    }
                    if !matches!(live.phase, Phase::Thinking(_) | Phase::Acting(_)) {
                        self.drained(at);
                    }
                }
                Life::Done { .. } => return Err(Rejected::Finished),
            },
            TaskEvent::Held(held) => self.held = *held,
        }
        Ok(())
    }

    /// Begin a new context at `start`'s root. The old context's results have
    /// nowhere to go in it; Max's words and actions stay pending, since they
    /// are for the task, not the context. A task waiting on Max still waits
    /// on him unless the new context has attachments to run.
    fn root(&mut self, start: &Start, at: Timestamp, world: &State) -> Result<(), Rejected> {
        let context = Context::rooted(start);
        self.drop_results();
        let mut live = match std::mem::replace(&mut self.life, Life::Starting) {
            Life::Live(mut live) => {
                live.context = context;
                live
            }
            Life::Starting | Life::Done { .. } => {
                self.since = at;
                Live::new(context)
            }
        };
        let phase = if !start.awaits.is_empty() {
            Phase::Acting(Step {
                response: None,
                outstanding: start.awaits.clone(),
            })
        } else if matches!(live.phase, Phase::AwaitingMax | Phase::Listening) {
            live.phase.clone()
        } else {
            Phase::Ready
        };
        if live.phase != phase {
            live.phase = phase;
            self.since = at;
        }
        self.life = Life::Live(live);
        for call in &start.awaits {
            if world
                .call(*call)
                .ok_or(Rejected::Unknown)?
                .settled
                .is_some()
            {
                self.settled(*call, at)?;
            }
        }
        Ok(())
    }

    fn model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        at: Timestamp,
        world: &State,
    ) -> Result<(), Rejected> {
        match event {
            ModelCallEvent::Sent(call) => {
                if call.task != self.id {
                    return Err(Rejected::NotForTask);
                }
                let Task { pending, life, .. } = self;
                let Life::Live(live) = life else {
                    return Err(Rejected::Starting);
                };
                if live.drain.is_some() {
                    return Err(Rejected::Draining);
                }
                if live.phase != Phase::Ready {
                    return Err(Rejected::Busy);
                }
                if live.context.head != call.base {
                    return Err(Rejected::Detached);
                }
                let rendered = |p: &Input| call.renders.iter().any(|r| p.is(r));
                if pending
                    .iter()
                    .any(|p| matches!(p, Input::Result(_)) && !rendered(p))
                {
                    return Err(Rejected::Unanswered);
                }
                let head = call.head();
                learn(&mut live.context, pending, &call.renders, head, world)?;
                pending.retain(|p| !rendered(p));
                live.context.head = head;
                self.enter(Phase::Thinking(id), at);
            }
            ModelCallEvent::FirstByte => {}
            ModelCallEvent::Ended { ending, .. } => {
                let live = self.live_mut()?;
                if live.phase != Phase::Thinking(id) {
                    return Err(Rejected::Busy);
                }
                match ending {
                    Ending::Responded { content, stop } => {
                        let entry = Message {
                            role: Role::Assistant,
                            content: *content,
                        }
                        .id_after(live.context.head);
                        live.context.head = entry;
                        live.failures = 0;
                        let calls = world
                            .model_call(id)
                            .map(|m| m.calls.clone())
                            .unwrap_or_default();
                        if !calls.is_empty() {
                            self.enter(
                                Phase::Acting(Step {
                                    response: Some(entry),
                                    outstanding: calls,
                                }),
                                at,
                            );
                        } else if *stop == Stop::EndTurn {
                            self.turn_ended(entry, at, world);
                        } else {
                            // Stopped short of ending its turn: the brain
                            // carries on with a continue note, roots a
                            // context too long anew, or fails a refusal.
                            self.drained(at);
                        }
                    }
                    Ending::Failed { .. } => {
                        live.failures += 1;
                        self.drained(at);
                    }
                    Ending::Limited { .. } | Ending::Abandoned { .. } | Ending::Lost => {
                        self.drained(at);
                    }
                }
            }
        }
        Ok(())
    }

    fn call(
        &mut self,
        id: CallId,
        event: &CallEvent,
        at: Timestamp,
        world: &State,
    ) -> Result<(), Rejected> {
        match event {
            CallEvent::Made(call) => match &call.caller {
                Caller::Agent {
                    task,
                    tool_use: None,
                } if *task == self.id => {
                    if self.live_mut()?.drain.is_some() {
                        return Err(Rejected::Draining);
                    }
                    self.background.push(id);
                    Ok(())
                }
                Caller::Max {
                    chat: Some(task), ..
                } if *task == self.id => {
                    self.max_actions.push(id);
                    Ok(())
                }
                _ => Err(Rejected::NotForTask),
            },
            CallEvent::Settled(_) if world.call(id).is_some_and(|c| c.settled.is_some()) => {
                self.pending.push(Input::Late(id));
                if self.drain().is_none() {
                    self.woken(at);
                }
                Ok(())
            }
            CallEvent::Settled(_) => self.settled(id, at),
            CallEvent::Dispatched | CallEvent::Stopping => Err(Rejected::NotForTask),
        }
    }

    /// A call this task waits on has settled: one of its round, its
    /// background work, or one of Max's actions in its chat.
    fn settled(&mut self, call: CallId, at: Timestamp) -> Result<(), Rejected> {
        if let Life::Live(live) = &mut self.life
            && let Phase::Acting(step) = &mut live.phase
            && let Some(i) = step.outstanding.iter().position(|c| *c == call)
        {
            step.outstanding.remove(i);
            let done = step.outstanding.is_empty();
            self.pending.push(Input::Result(call));
            if done {
                self.drained(at);
            }
            return Ok(());
        }
        if let Some(i) = self.background.iter().position(|c| *c == call) {
            self.background.remove(i);
            self.pending.push(Input::Background(call));
            if self.drain().is_some() {
                // Stopping: the last background call to settle lets it finish.
                if self.phase() == Some(&Phase::Listening) && self.background.is_empty() {
                    self.drained(at);
                }
            } else {
                self.woken(at);
            }
            return Ok(());
        }
        let i = self
            .max_actions
            .iter()
            .position(|c| *c == call)
            .ok_or(Rejected::NotForTask)?;
        self.max_actions.remove(i);
        self.pending.push(Input::MaxAction(call));
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, by: &Author, at: Timestamp) {
        self.pending.push(Input::Delivery(delivery.id));
        // Marking a task done hands it its last turn: it finishes when that
        // turn ends.
        if delivery.content == Content::Prompt(Prompt::Done) {
            self.mode = Mode::Autonomous;
            if let Life::Live(live) = &mut self.life {
                live.owed_since = None;
            }
        }
        if matches!(delivery.content, Content::Words(_))
            && matches!(by, Author::Max { .. })
            && let Life::Live(live) = &mut self.life
        {
            live.owed_since = None;
        }
        if self.drain().is_none() {
            self.woken(at);
        }
    }

    /// Something that wakes arrived: if the task is waiting, it is its move
    /// again.
    fn woken(&mut self, at: Timestamp) {
        if matches!(self.phase(), Some(Phase::AwaitingMax | Phase::Listening)) {
            self.enter(Phase::Ready, at);
        }
    }

    fn wakes(&self) -> bool {
        self.pending.iter().any(Input::wakes)
    }

    /// The model ended its turn at `entry`: complete, wait, or carry on.
    fn turn_ended(&mut self, entry: EntryId, at: Timestamp, world: &State) {
        if self.drain().is_some() {
            return self.drained(at);
        }
        if self.wakes() {
            // Max's words, or a prompt, arrived during the turn.
            return self.enter(Phase::Ready, at);
        }
        if self.mode == Mode::Autonomous && self.background.is_empty() {
            return self.finish(Outcome::Result { entry }, at);
        }
        self.wait(at, world);
    }

    /// Enter the wait the mode asks for, unless something already ends it.
    fn wait(&mut self, at: Timestamp, world: &State) {
        if self.mode == Mode::UserFacing {
            return self.await_max(at);
        }
        let woken = self.wakes() || (self.mode == Mode::Standing && !world.stale(self).is_empty());
        let phase = if woken {
            Phase::Ready
        } else {
            Phase::Listening
        };
        self.enter(phase, at);
    }

    fn await_max(&mut self, at: Timestamp) {
        if self.wakes() {
            return self.enter(Phase::Ready, at);
        }
        if let Life::Live(live) = &mut self.life {
            live.owed_since.get_or_insert(at);
        }
        self.enter(Phase::AwaitingMax, at);
    }

    /// Nothing in the round is outstanding: carry on, or do what the drain
    /// says.
    fn drained(&mut self, at: Timestamp) {
        let Life::Live(live) = &mut self.life else {
            return;
        };
        match live.drain.clone() {
            None => self.enter(Phase::Ready, at),
            Some(Drain::ToMax) => {
                live.drain = None;
                self.await_max(at);
            }
            Some(Drain::Stop(_) | Drain::Fail(_)) if !self.background.is_empty() => {
                self.enter(Phase::Listening, at);
            }
            Some(Drain::Stop(by)) => self.finish(Outcome::Cancelled { by }, at),
            Some(Drain::Fail(reason)) => self.finish(Outcome::Failed { reason }, at),
        }
    }

    fn finish(&mut self, outcome: Outcome, at: Timestamp) {
        let context = match std::mem::replace(&mut self.life, Life::Starting) {
            Life::Live(live) => Some(live.context),
            Life::Starting => None,
            Life::Done { context, .. } => context,
        };
        self.life = Life::Done { outcome, context };
        self.since = at;
    }

    fn enter(&mut self, phase: Phase, at: Timestamp) {
        if let Life::Live(live) = &mut self.life
            && live.phase != phase
        {
            live.phase = phase;
            self.since = at;
        }
    }

    fn drop_results(&mut self) {
        self.pending.retain(|p| !matches!(p, Input::Result(_)));
    }
}

/// Whether a live task is waiting for a move rather than running one.
fn waiting(phase: &Phase) -> bool {
    matches!(phase, Phase::Ready | Phase::AwaitingMax | Phase::Listening)
}

/// Whether a drain ends the task, rather than handing the move to Max.
pub(crate) fn ends(drain: Option<&Drain>) -> bool {
    matches!(drain, Some(Drain::Stop(_) | Drain::Fail(_)))
}

/// Update what a context embodies from what a request newly included,
/// remembering what it was before.
fn learn(
    context: &mut Context,
    pending: &[Input],
    renders: &[Rendered],
    head: EntryId,
    world: &State,
) -> Result<(), Rejected> {
    let mut before = Vec::new();
    for rendered in renders {
        match rendered {
            Rendered::Call(call) => {
                let outcome = world
                    .call(*call)
                    .ok_or(Rejected::Unknown)?
                    .outcome()
                    .ok_or(Rejected::Busy)?;
                match outcome {
                    CallOutcome::Refused { current } => context.tell(current, &mut before),
                    outcome => {
                        if let Some(output) = outcome.output() {
                            context.observe(&output.observes, &mut before);
                        }
                    }
                }
            }
            Rendered::Delivery(delivery) => {
                if !pending.contains(&Input::Delivery(*delivery)) {
                    return Err(Rejected::NotForTask);
                }
            }
            Rendered::Note(Note::Stale { told, shown }) => {
                context.tell(told, &mut before);
                context.observe(shown, &mut before);
            }
            Rendered::Note(Note::Boundary { from: Some(from) }) => {
                world.entry(*from).ok_or(Rejected::Unknown)?;
            }
            Rendered::Note(_) => {}
        }
    }
    if !before.is_empty() {
        context.learned.push_back(Learned { head, before });
    }
    Ok(())
}
