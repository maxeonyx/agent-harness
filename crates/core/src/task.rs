//! Tasks: one agent's mandate over its whole life, and the agent loop as a
//! fold.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::call::{CallEvent, Caller, Outcome as CallOutcome};
use crate::context::{EntryId, Message, Note, Rendered, Role, Root};
use crate::delivery::{Content, Delivery};
use crate::environment::Binding;
use crate::id::{CallId, DeliveryId, ModelCallId, TaskId, Timestamp, ValueId};
use crate::journal::{Author, Fact, Record, Rejected};
use crate::model_call::{Ending, ModelCallEvent, Purpose, Stop};
use crate::resource::{Observation, ResourceKey, Version};
use crate::state::State;

/// The fold of the facts about one task: everything the brain, a face or
/// the scheduler needs to know about it now.
///
/// **Concept.** A task is the unit of structured concurrency and the thing
/// Max watches, talks to, and is asked to attend to. It has a [`Mandate`],
/// a [`Scope`] that owns it, a [`Mode`] that says what the end of a turn
/// means, an environment [`Binding`], and a head: the entry its context
/// currently ends at. A task is an identity; its context is a value. The
/// head moves through the context tree (a fork starts a task on its
/// parent's response, a handover moves a task to a new root, a resume
/// starts a task on another's last entry, a rewind moves it back), and none
/// of that copies or edits a context.
///
/// Its [`TaskState`] says whose move it is, and is folded from the facts
/// that concern it (`Task::apply`, run only by [`State::apply`]): its own
/// events, its model calls, the settlements of the calls it waits on,
/// deliveries to it, and changes to what it embodies. The whole fold is here, including what its context
/// embodies ([`Task::knows`]); it reads the rest of the journal's state
/// (call outcomes, the context tree, current versions) but keeps none of it.
/// The state is never written down as a fact, so it cannot disagree with its
/// history; "what is it waiting on" and "is it waiting on Max" are read off
/// it.
///
/// **Vision.** Structured concurrency (a parent waiting on children is
/// [`TaskState::Acting`] on its spawn call, like waiting on any tool, and a
/// task is done only when nothing it started is still running and nothing
/// sent to it is unread); the A.1.3 problem (the mandate is data, so the
/// harness can state it at the fork, after each tool round and after a
/// handover); user-facing, autonomous and standing tasks, which differ only
/// in [`Mode`]; the throbber; restart (the state says what to redo); and
/// Max's words never lost (a pending delivery stops a turn from completing
/// the task, and survives a handover).
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
    /// The agent kind (role) whose prompt is in the root. Fixed for life,
    /// and inherited by forks, resumes and asides: models do not reliably
    /// honour a role that changes mid-context.
    pub agent: String,
    pub scope: Scope,
    pub origin: Origin,
    pub mode: Mode,
    pub mandate: Mandate,
    /// Where its current context began: its root, or the entry it was
    /// forked, resumed or opened at. A rewind cannot go before it.
    pub start: EntryId,
    pub head: EntryId,
    /// Which limb runs each of its context's tools. Set when a root is made
    /// and kept until the next one, so a redefined environment takes effect
    /// at the task's next handover, never under it.
    pub binding: Binding,
    pub state: TaskState,
    /// When it entered its current state: how long the throbber has shown
    /// what it shows.
    pub since: Timestamp,
    /// Inputs no request has rendered yet. A turn cannot complete over one
    /// that wakes.
    pub pending: Vec<Pending>,
    /// Calls outside the current round whose settlement will be an input:
    /// background work the task started, and Max's actions in its chat.
    pub watching: Vec<Watched>,
    /// Set when the task must stop doing what it is doing. No request is
    /// sent while it is set; it takes effect once nothing is outstanding.
    pub draining: Option<Drain>,
    /// No new requests under this task until released; running work carries
    /// on. Max pausing a subtree, or the brain after a long outage.
    pub held: bool,
    /// Since when Max has owed this task a word: set when a user-facing turn
    /// ends or Max interrupts, cleared by his words or by a change of mode.
    /// It survives the harness's own turns (a handover before the cache
    /// expires), so the task keeps its place in his queue through them.
    pub owed_since: Option<Timestamp>,
    /// Requests that failed in a row; the brain's policy gives up at some
    /// count ([`Drain::Fail`]). The account's limit is not a failure.
    pub failures: u32,
    /// The version of each resource its context embodies, and what it has
    /// been told since.
    pub knows: BTreeMap<ResourceKey, Known>,
    /// What each request since `start` changed in `knows`, so a rewind
    /// restores what the earlier context embodied.
    learned: Vec<Learned>,
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

/// How a task's first context came to be. It decides where the head starts,
/// how the first request is framed, and how the tree shows the task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// A new root in an environment, for an agent kind; then its shared
    /// brief and attachments, then its own mandate. In a spawn's scope, only
    /// the kinds and environments its meta limb [starts](crate::environment::Startable).
    Fresh { agent: String, start: Box<Start> },
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
    /// in the finished chat.
    Resume { from: TaskId },
    /// A busy task's head, opened by Max as a user-facing fork so he can talk
    /// to it while it waits; its result is delivered back to `from`.
    Aside { from: TaskId },
}

/// A new root, and what is decided with it: a fresh task's first context,
/// a handover's, a reseed's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    pub root: Root,
    /// Made with the root's tools ([`Binding::new`]).
    pub binding: Binding,
    /// The versions the root embodies: the environment's instructions, the
    /// role prompt, the skill index, each tool's schema.
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

/// What a task was asked to do, and by whom.
///
/// The chain of mandates up the owner chain is the task's place in the
/// overall work: "A, then A.1, then A.1.3". Rendering that chain at a fork,
/// as a reminder after each tool round, and at the top of a handover is how
/// a deeply forked agent knows to stop after A.1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    pub text: ValueId,
    pub from: Giver,
}

/// Who gave a mandate: Max, or a task (the parent, or a sibling that added
/// this task to its own scope, which then says why).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Giver {
    Max,
    Task(TaskId),
}

/// Whose move it is. Exactly one holds at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
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
    /// change to what a standing task embodies, or an autonomous task's
    /// background work.
    Listening,
    Done(Outcome),
}

/// The outstanding calls of one round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The response whose tool uses these are; `None` for a fresh context's
    /// attachments, whose tool uses the harness writes when it renders them.
    pub response: Option<EntryId>,
    pub outstanding: Vec<CallId>,
}

/// An input waiting for a request to render it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub input: Input,
    /// Whether it ends a wait. Only a few things drive a request
    /// (`context-and-agent-loop.md`): the agent's own tool loop, Max's
    /// words, a harness prompt, work the agent asked to hear back from, and
    /// for a standing task a change to what it embodies. A round's results
    /// and Max's actions in the chat ride on the next request.
    pub wakes: bool,
}

/// What a pending input is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A settled call of the last round. The next request must render every
    /// one, since each tool use needs its result first.
    Result(CallId),
    /// A settled call outside the round: background work, Max's action in
    /// the chat, a late outcome.
    Activity(CallId),
    Delivery(DeliveryId),
}

impl Input {
    fn is(&self, rendered: &Rendered) -> bool {
        match (self, rendered) {
            (Input::Result(a) | Input::Activity(a), Rendered::Call(b)) => a == b,
            (Input::Delivery(a), Rendered::Delivery(b)) => a == b,
            _ => false,
        }
    }
}

/// A call outside the round that the task is told about when it settles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watched {
    pub call: CallId,
    /// Background work wakes the task; Max's actions in its chat do not.
    pub wakes: bool,
}

/// What a task does once nothing is outstanding, instead of carrying on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drain {
    /// Max interrupted: the move becomes his, whatever the mode. It never
    /// passes through a spawn call (that would stop the scope); to talk to a
    /// task waiting on children, Max opens an aside.
    ToMax,
    /// The task is cancelled, and everything it owns with it.
    Stop(Stopper),
    /// The brain cannot carry the task on: the provider rejects its context,
    /// its environment is gone, retries are exhausted.
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
                    scope: _,
                    origin,
                    mode: _,
                    mandate,
                } = &**created;
                let Mandate { text, from: _ } = mandate;
                out.push(*text);
                match origin {
                    Origin::Fresh { agent: _, start } => start.values(out),
                    Origin::Fork { from: _ }
                    | Origin::Resume { from: _ }
                    | Origin::Aside { from: _ } => {}
                }
            }
            TaskEvent::Moved(Move::Handover { call: _, start } | Move::Reseed(start)) => {
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

/// A task begins. Where its head starts, its agent kind and its binding
/// follow from its origin: a fresh task's root, or its source's head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub name: String,
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
    /// current definitions, which is where a rebuild picks up a changed
    /// AGENTS.md, role or tool set.
    Handover { call: CallId, start: Box<Start> },
    /// To a new root the harness built without the agent's help: a boundary
    /// note and the mandate. For a context the provider will no longer
    /// accept, or a task with no handover tool (the scheduler, whose memory
    /// is the queue it observes).
    Reseed(Box<Start>),
    /// Back to a response on the task's own chain since its start that
    /// ended a model turn, so the next message is Max's. The abandoned
    /// branch stays in the tree, and the context embodies again only what
    /// it embodied there.
    Rewind { to: EntryId },
}

impl Task {
    /// The task its creation describes, reading its source and its
    /// attachments from the journal's state. Its attachments are its first
    /// round, and those already settled are its results at once.
    pub(crate) fn new(
        id: TaskId,
        created: &Created,
        at: Timestamp,
        world: &State,
    ) -> Result<Task, Rejected> {
        let source = match &created.origin {
            Origin::Fresh { .. } => None,
            Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } => {
                let source = world.task(*from).ok_or(Rejected::Unknown)?;
                let resume = matches!(created.origin, Origin::Resume { .. });
                match (resume, source.is_done()) {
                    (true, false) => return Err(Rejected::Busy),
                    (false, true) => return Err(Rejected::Finished),
                    _ => {}
                }
                Some((source, resume))
            }
        };
        let (agent, head, binding) = match (&created.origin, source) {
            (Origin::Fresh { agent, start }, _) => {
                (agent.clone(), start.root.id(), start.binding.clone())
            }
            (_, Some((source, _))) => (source.agent.clone(), source.head, source.binding.clone()),
            (_, None) => return Err(Rejected::Unknown),
        };
        let mut task = Task {
            id,
            name: created.name.clone(),
            agent,
            scope: created.scope,
            origin: created.origin.clone(),
            mode: created.mode,
            mandate: created.mandate.clone(),
            start: head,
            head,
            binding,
            state: TaskState::Ready,
            since: at,
            pending: Vec::new(),
            watching: Vec::new(),
            draining: None,
            held: false,
            owed_since: None,
            failures: 0,
            knows: BTreeMap::new(),
            learned: Vec::new(),
        };
        match (&created.origin, source) {
            (Origin::Fresh { start, .. }, _) => task.reroot(start, at, world)?,
            (_, Some((source, resume))) => {
                task.knows = source.knows.clone();
                if resume {
                    task.pending = source.pending.clone();
                    task.watching = source.watching.clone();
                }
            }
            (_, None) => {}
        }
        Ok(task)
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, TaskState::Done(_))
    }

    /// Whether background work it started is still running. An autonomous
    /// task cannot finish until it is not.
    pub fn has_background(&self) -> bool {
        self.watching.iter().any(|w| w.wakes)
    }

    /// The calls whose settlement its fold reads: its round's, and those it
    /// watches. This is the whole of "who reads a call", derived rather than
    /// indexed.
    pub fn awaited(&self) -> impl Iterator<Item = CallId> + '_ {
        let round = match &self.state {
            TaskState::Acting(step) => step.outstanding.as_slice(),
            _ => &[],
        };
        round
            .iter()
            .copied()
            .chain(self.watching.iter().map(|w| w.call))
    }

    pub fn awaits(&self, call: CallId) -> bool {
        self.awaited().any(|c| c == call)
    }

    /// Fold one fact that concerns this task, or refuse it and change
    /// nothing.
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
                let moved = self.knows.get(&change.resource).is_some_and(|known| {
                    known.has != change.version && known.told != Some(change.version)
                });
                if moved
                    && self.mode == Mode::Standing
                    && self.state == TaskState::Listening
                    && self.draining.is_none()
                {
                    self.enter(TaskState::Ready, at);
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
                self.watching.push(Watched {
                    call: *id,
                    wakes: false,
                });
                Ok(())
            }
            Fact::Call(id, CallEvent::Settled(_)) => {
                let i = self
                    .watching
                    .iter()
                    .position(|w| w.call == *id)
                    .ok_or(Rejected::Finished)?;
                self.watching.remove(i);
                self.pending.push(Pending {
                    input: Input::Activity(*id),
                    wakes: false,
                });
                Ok(())
            }
            _ => Err(Rejected::Finished),
        }
    }

    fn event(&mut self, event: &TaskEvent, at: Timestamp, world: &State) -> Result<(), Rejected> {
        use TaskState as S;
        let waiting = matches!(self.state, S::Ready | S::AwaitingMax | S::Listening);
        match event {
            TaskEvent::Created(_) => return Err(Rejected::Exists),
            TaskEvent::ModeSet(mode) => {
                if *mode == Mode::Standing && self.scope != Scope::Max {
                    return Err(Rejected::WrongScope);
                }
                self.mode = *mode;
                if *mode != Mode::UserFacing {
                    self.owed_since = None;
                }
                // A waiting task waits for what its new mode waits for. It
                // never completes here: the model has not seen the change.
                if matches!(self.state, S::AwaitingMax | S::Listening) && self.draining.is_none() {
                    self.wait(at, world);
                }
            }
            TaskEvent::Renamed(name) => self.name = name.clone(),
            TaskEvent::Moved(Move::Handover { call, start }) => {
                if self.draining.is_some() {
                    return Err(Rejected::Draining);
                }
                let result = Input::Result(*call);
                if self.state != S::Ready || !self.pending.iter().any(|p| p.input == result) {
                    return Err(Rejected::Busy);
                }
                self.reroot(start, at, world)?;
            }
            TaskEvent::Moved(Move::Reseed(start)) => {
                if self.draining.is_some() {
                    return Err(Rejected::Draining);
                }
                if !waiting {
                    return Err(Rejected::Busy);
                }
                self.reroot(start, at, world)?;
            }
            TaskEvent::Moved(Move::Rewind { to }) => {
                if !waiting {
                    return Err(Rejected::Busy);
                }
                self.head = *to;
                self.drop_results();
                while self
                    .learned
                    .last()
                    .is_some_and(|l| !world.descends(*to, l.head))
                {
                    let Some(learned) = self.learned.pop() else {
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
            TaskEvent::Drain(drain) => {
                // An ending outranks an interrupt; the first ending stands.
                if !matches!(self.draining, Some(Drain::Stop(_) | Drain::Fail(_))) {
                    self.draining = Some(drain.clone());
                }
                if !matches!(self.state, S::Thinking(_) | S::Acting(_)) {
                    self.drained(at);
                }
            }
            TaskEvent::Held(held) => self.held = *held,
        }
        Ok(())
    }

    /// Begin a new context at `start`'s root. The old context's results have
    /// nowhere to go in it; Max's words and actions stay pending, since they
    /// are for the task, not the context. A task waiting on Max still waits
    /// on him unless the new context has attachments to run.
    fn reroot(&mut self, start: &Start, at: Timestamp, world: &State) -> Result<(), Rejected> {
        self.start = start.root.id();
        self.head = self.start;
        self.binding = start.binding.clone();
        self.knows.clear();
        self.learned.clear();
        for o in &start.observes {
            self.knows.insert(
                o.resource.clone(),
                Known {
                    has: o.version,
                    told: None,
                },
            );
        }
        self.drop_results();
        if start.awaits.is_empty() {
            if self.state == TaskState::Ready {
                return Ok(());
            }
            if !matches!(self.state, TaskState::AwaitingMax | TaskState::Listening) {
                self.enter(TaskState::Ready, at);
            }
            return Ok(());
        }
        self.enter(
            TaskState::Acting(Step {
                response: None,
                outstanding: start.awaits.clone(),
            }),
            at,
        );
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
                if call.purpose == Purpose::KeepAlive {
                    return Ok(());
                }
                if self.draining.is_some() {
                    return Err(Rejected::Draining);
                }
                if self.state != TaskState::Ready {
                    return Err(Rejected::Busy);
                }
                if call.base != self.head {
                    return Err(Rejected::Detached);
                }
                let rendered = |p: &Pending| call.renders.iter().any(|r| p.input.is(r));
                if self
                    .pending
                    .iter()
                    .any(|p| matches!(p.input, Input::Result(_)) && !rendered(p))
                {
                    return Err(Rejected::Unanswered);
                }
                let head = call.head();
                self.learn(&call.renders, head, world)?;
                self.pending.retain(|p| !rendered(p));
                self.head = head;
                self.enter(TaskState::Thinking(id), at);
            }
            ModelCallEvent::FirstByte => {}
            ModelCallEvent::Ended { ending, .. } => {
                if self.state != TaskState::Thinking(id) {
                    return Err(Rejected::Busy);
                }
                match ending {
                    Ending::Responded { content, stop } => {
                        let entry = Message {
                            role: Role::Assistant,
                            content: *content,
                        }
                        .id_after(self.head);
                        let calls = world
                            .model_call(id)
                            .map(|m| m.calls.clone())
                            .unwrap_or_default();
                        self.head = entry;
                        self.failures = 0;
                        if !calls.is_empty() {
                            self.enter(
                                TaskState::Acting(Step {
                                    response: Some(entry),
                                    outstanding: calls,
                                }),
                                at,
                            );
                        } else if matches!(stop, Stop::EndTurn | Stop::Refusal) {
                            self.turn_ended(entry, at, world);
                        } else {
                            // Stopped short: the brain carries on with a
                            // continue note, or reseeds a context too long.
                            self.drained(at);
                        }
                    }
                    Ending::Failed { .. } => {
                        self.failures += 1;
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

    /// Update what the context embodies from what a request newly included,
    /// remembering what it was before.
    fn learn(
        &mut self,
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
                        CallOutcome::Refused { current } => self.tell(current, &mut before),
                        outcome => {
                            if let Some(output) = outcome.output() {
                                self.observe(&output.observes, &mut before);
                            }
                        }
                    }
                }
                Rendered::Delivery(delivery) => {
                    let pending = Input::Delivery(*delivery);
                    if !self.pending.iter().any(|p| p.input == pending) {
                        return Err(Rejected::NotForTask);
                    }
                }
                Rendered::Note(Note::Stale { told, shown }) => {
                    self.tell(told, &mut before);
                    self.observe(shown, &mut before);
                }
                Rendered::Note(Note::Boundary { from: Some(from) }) => {
                    world.entry(*from).ok_or(Rejected::Unknown)?;
                }
                Rendered::Note(Note::Interrupted { call }) => {
                    world.model_call(*call).ok_or(Rejected::Unknown)?;
                }
                Rendered::Note(_) => {}
            }
        }
        if !before.is_empty() {
            self.learned.push(Learned { head, before });
        }
        Ok(())
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

    fn call(
        &mut self,
        id: CallId,
        event: &CallEvent,
        at: Timestamp,
        world: &State,
    ) -> Result<(), Rejected> {
        match event {
            CallEvent::Made(call) => {
                let wakes = match &call.caller {
                    Caller::Agent {
                        task,
                        tool_use: None,
                    } if *task == self.id => {
                        if self.draining.is_some() {
                            return Err(Rejected::Draining);
                        }
                        true
                    }
                    Caller::Max {
                        chat: Some(task), ..
                    } if *task == self.id => false,
                    _ => return Err(Rejected::NotForTask),
                };
                self.watching.push(Watched { call: id, wakes });
                Ok(())
            }
            CallEvent::Settled(_) if world.call(id).is_some_and(|c| c.settled.is_some()) => {
                // The real outcome of a call it was told is unknown.
                self.pending.push(Pending {
                    input: Input::Activity(id),
                    wakes: true,
                });
                if self.draining.is_none() {
                    self.woken(at);
                }
                Ok(())
            }
            CallEvent::Settled(_) => self.settled(id, at),
            CallEvent::Dispatched | CallEvent::Stopping => Err(Rejected::NotForTask),
        }
    }

    /// A call this task waits on has settled: one of its round, or one it
    /// watches.
    fn settled(&mut self, call: CallId, at: Timestamp) -> Result<(), Rejected> {
        if let TaskState::Acting(step) = &mut self.state
            && let Some(i) = step.outstanding.iter().position(|c| *c == call)
        {
            step.outstanding.remove(i);
            let done = step.outstanding.is_empty();
            self.pending.push(Pending {
                input: Input::Result(call),
                wakes: false,
            });
            if done {
                self.drained(at);
            }
            return Ok(());
        }
        let i = self
            .watching
            .iter()
            .position(|w| w.call == call)
            .ok_or(Rejected::NotForTask)?;
        let watched = self.watching.remove(i);
        self.pending.push(Pending {
            input: Input::Activity(call),
            wakes: watched.wakes,
        });
        if self.draining.is_some() {
            // Stopping: the last background call to settle lets it finish.
            if self.state == TaskState::Listening && !self.has_background() {
                self.drained(at);
            }
        } else if watched.wakes {
            self.woken(at);
        }
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, by: &Author, at: Timestamp) {
        self.pending.push(Pending {
            input: Input::Delivery(delivery.id),
            wakes: true,
        });
        if matches!(delivery.content, Content::Words(_)) && matches!(by, Author::Max { .. }) {
            self.owed_since = None;
        }
        if self.draining.is_none() {
            self.woken(at);
        }
    }

    /// Something that wakes arrived: if the task is waiting, it is its move
    /// again.
    fn woken(&mut self, at: Timestamp) {
        if matches!(self.state, TaskState::AwaitingMax | TaskState::Listening) {
            self.enter(TaskState::Ready, at);
        }
    }

    /// The model ended its turn at `entry`: complete, wait, or carry on.
    fn turn_ended(&mut self, entry: EntryId, at: Timestamp, world: &State) {
        if self.draining.is_some() {
            return self.drained(at);
        }
        if self.pending.iter().any(|p| p.wakes) {
            // Max's words, or a prompt, arrived during the turn.
            return self.enter(TaskState::Ready, at);
        }
        if self.mode == Mode::Autonomous && !self.has_background() {
            return self.enter(TaskState::Done(Outcome::Result { entry }), at);
        }
        self.wait(at, world);
    }

    /// Enter the wait the mode asks for, unless something already ends it.
    fn wait(&mut self, at: Timestamp, world: &State) {
        if self.mode == Mode::UserFacing {
            return self.await_max(at);
        }
        let woken = self.pending.iter().any(|p| p.wakes)
            || (self.mode == Mode::Standing && !world.stale(self).is_empty());
        let state = if woken {
            TaskState::Ready
        } else {
            TaskState::Listening
        };
        self.enter(state, at);
    }

    fn await_max(&mut self, at: Timestamp) {
        if self.pending.iter().any(|p| p.wakes) {
            return self.enter(TaskState::Ready, at);
        }
        self.owed_since.get_or_insert(at);
        self.enter(TaskState::AwaitingMax, at);
    }

    /// Nothing in the round is outstanding: carry on, or do what the drain
    /// says.
    fn drained(&mut self, at: Timestamp) {
        match self.draining.clone() {
            None => self.enter(TaskState::Ready, at),
            Some(Drain::ToMax) => {
                self.draining = None;
                self.await_max(at);
            }
            Some(Drain::Stop(_) | Drain::Fail(_)) if self.has_background() => {
                self.enter(TaskState::Listening, at);
            }
            Some(Drain::Stop(by)) => {
                self.draining = None;
                self.enter(TaskState::Done(Outcome::Cancelled { by }), at);
            }
            Some(Drain::Fail(reason)) => {
                self.draining = None;
                self.enter(TaskState::Done(Outcome::Failed { reason }), at);
            }
        }
    }

    fn enter(&mut self, state: TaskState, at: Timestamp) {
        if self.state != state {
            self.state = state;
            self.since = at;
        }
    }

    fn drop_results(&mut self) {
        self.pending
            .retain(|p| !matches!(p.input, Input::Result(_)));
    }
}
