//! Tasks: one agent's mandate over its whole life, and the agent loop as a
//! fold.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::call::{CallEvent, Caller};
use crate::context::{EntryId, Rendered};
use crate::delivery::{Content, Delivery};
use crate::environment::Binding;
use crate::id::{CallId, DeliveryId, ModelCallId, TaskId, Timestamp};
use crate::journal::{Author, Fact, Record, Rejected};
use crate::model_call::{Ending, ModelCallEvent, Purpose, Stop};
use crate::resource::{Observation, ResourceKey, Version};

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
/// that concern it ([`Task::apply`]): its own events, its model calls, the
/// settlements of the calls it waits on, and deliveries to it. The state is
/// never written down as a fact, so it cannot disagree with its history;
/// "what is it waiting on" and "is it waiting on Max" are read off it.
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
    /// The agent kind (role) whose prompt is in the root. Fixed for life:
    /// models do not reliably honour a role that changes mid-context.
    pub agent: String,
    pub scope: Scope,
    pub origin: Origin,
    pub mode: Mode,
    pub mandate: Mandate,
    pub head: EntryId,
    /// Which limb runs each of its context's tools. Set when a root is made
    /// and kept until the next one, so a redefined environment takes effect
    /// at the task's next handover, never under it.
    pub binding: Binding,
    pub state: TaskState,
    /// Inputs no request has rendered yet. A turn cannot complete over one
    /// that [wakes](Wakes::Always).
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
    /// count ([`Drain::Fail`]).
    pub failures: u32,
    /// The version of each resource its context embodies, and what it has
    /// been told since. Maintained by [`crate::state::State`], which can see
    /// the calls a request rendered.
    pub knows: BTreeMap<ResourceKey, Known>,
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
    /// so do asides and standing tasks such as the attention scheduler.
    Max,
    /// The scope the spawn call opened.
    Spawn(CallId),
}

/// How a task's first context came to be. It decides how the first request
/// is framed and how the tree shows the task; the head says where it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// A new root in its environment, then its shared brief and attachments,
    /// then its own mandate.
    Fresh,
    /// The parent's spawning response. The first request answers every tool
    /// use in it: the spawn with the mandate, every other with a note that it
    /// belongs elsewhere, so no fork waits on its parent's other work.
    Fork { from: TaskId },
    /// A finished task's last entry, continued with a new mandate.
    Resume { from: TaskId },
    /// A busy task's head, opened by Max as a user-facing fork so he can talk
    /// to it while it waits; its result is delivered back to `from`.
    Aside { from: TaskId },
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
    /// The end of a turn waits for the next event from its environment or a
    /// word from Max. Only in Max's scope, since a scope holding one would
    /// never close.
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
    pub text: String,
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
    /// A request is in flight.
    Thinking(ModelCallId),
    /// The calls of one round are outstanding: a response's tool uses, or a
    /// fresh context's attachments. A spawn is one of them, so is an ask of
    /// Max.
    Acting(Step),
    /// Max's move: a user-facing turn ended, or he interrupted.
    AwaitingMax,
    /// The turn ended and the task waits for something that wakes it: a
    /// standing task's next event, or an autonomous task's background work.
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
    pub wakes: Wakes,
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
    pub wakes: Wakes,
}

/// Which waits an input ends.
///
/// Only a few things drive a request (`context-and-agent-loop.md`): the
/// agent's own tool loop, Max ending his turn, a harness prompt, a standing
/// task's event, and work the agent asked to hear back from. Everything
/// else rides on the next request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wakes {
    /// Ends any wait.
    Always,
    /// Ends only [`TaskState::Listening`]: an environment's event.
    Listening,
    /// Never causes a request: Max's activity in the chat, a round's results.
    Never,
}

impl Wakes {
    fn ends(self, state: &TaskState) -> bool {
        match self {
            Wakes::Always => matches!(state, TaskState::AwaitingMax | TaskState::Listening),
            Wakes::Listening => matches!(state, TaskState::Listening),
            Wakes::Never => false,
        }
    }
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
/// fact of another identity, folded in by [`Task::apply`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Created(Box<Created>),
    ModeSet(Mode),
    Renamed(String),
    Moved(Move),
    Drain(Drain),
    Held(bool),
}

/// A task begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub name: String,
    pub agent: String,
    pub scope: Scope,
    pub origin: Origin,
    pub mode: Mode,
    pub mandate: Mandate,
    /// Where its context starts: a root, a shared seed, the parent's
    /// response, another task's entry.
    pub head: EntryId,
    /// A fork, resume or aside is bound to its source's environment.
    pub binding: Binding,
    /// The versions a new root embodies: the environment's instructions, the
    /// role prompt, the skill index, each tool's schema. Empty for a fork,
    /// which inherits what its source knows.
    pub observes: Vec<Observation>,
    /// Attachments to run before the first request, shown to the task as
    /// tool calls it made: calls made by the spawn ([`Caller::Meta`]), shared
    /// by every member that names them, so N siblings share one read and one
    /// cache prefix.
    pub awaits: Vec<CallId>,
}

/// The head moved to an entry that does not extend it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Move {
    /// To a new root seeded by the task's own handover call, taking effect once
    /// it is the round's only outstanding call. The new context is that call's
    /// result, so the old context ends on it. The root is rendered from the
    /// environment's current definitions, which is where a rebuild picks up
    /// a changed AGENTS.md, role or tool set.
    Handover {
        call: CallId,
        head: EntryId,
        binding: Binding,
        observes: Vec<Observation>,
        awaits: Vec<CallId>,
    },
    /// To a new root the harness built without the agent's help: a boundary
    /// note and the mandate. For a context the provider will no longer
    /// accept, or a task with no handover tool (the scheduler, whose memory
    /// is the queue it observes).
    Reseed {
        head: EntryId,
        binding: Binding,
        observes: Vec<Observation>,
    },
    /// Back to an entry on the task's own chain that ends a model turn, so
    /// the next message is Max's. The abandoned branch stays in the tree.
    Rewind { head: EntryId },
}

impl Task {
    /// The task as its first event describes it. Its attachments are its
    /// first round; [`crate::state::State`] feeds in those already settled.
    pub fn new(id: TaskId, created: &Created) -> Task {
        Task {
            id,
            name: created.name.clone(),
            agent: created.agent.clone(),
            scope: created.scope,
            origin: created.origin,
            mode: created.mode,
            mandate: created.mandate.clone(),
            head: created.head,
            binding: created.binding.clone(),
            state: round(&created.awaits),
            pending: Vec::new(),
            watching: Vec::new(),
            draining: None,
            held: false,
            owed_since: None,
            failures: 0,
            knows: BTreeMap::new(),
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, TaskState::Done(_))
    }

    /// Whether background work it started is still running. An autonomous
    /// task cannot finish until it is not.
    pub fn has_background(&self) -> bool {
        self.watching.iter().any(|w| w.wakes == Wakes::Always)
    }

    /// Whether the task's fold reads this call's settlement.
    pub fn awaits(&self, call: CallId) -> bool {
        matches!(&self.state, TaskState::Acting(step) if step.outstanding.contains(&call))
            || self.watching.iter().any(|w| w.call == call)
    }

    /// Fold one fact that concerns this task, or refuse it.
    ///
    /// This is the whole agent loop. Three properties hold by construction:
    /// no request is sent while a drain is pending or a tool use of the last
    /// response is unanswered; a task is done only when nothing it started
    /// is outstanding and nothing waking is unread; and Max's words are never
    /// lost, to a turn that ends first or to a handover.
    pub fn apply(&mut self, record: &Record) -> Result<(), Rejected> {
        if self.is_done() {
            return Err(Rejected::Finished);
        }
        let at = record.at;
        match &record.fact {
            Fact::Task(id, event) if *id == self.id => self.event(event, at),
            Fact::ModelCall(id, event) => self.model_call(*id, event, at),
            Fact::Call(id, event) => self.call(*id, event, at),
            Fact::Delivery(delivery) if delivery.to == self.id => {
                self.deliver(delivery, &record.by);
                Ok(())
            }
            _ => Err(Rejected::NotForTask),
        }
    }

    fn event(&mut self, event: &TaskEvent, at: Timestamp) -> Result<(), Rejected> {
        use TaskState as S;
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
                    self.wait(at);
                }
            }
            TaskEvent::Renamed(name) => self.name = name.clone(),
            TaskEvent::Moved(Move::Handover {
                call,
                head,
                binding,
                awaits,
                ..
            }) => {
                if self.draining.is_some() {
                    return Err(Rejected::Draining);
                }
                match &self.state {
                    S::Acting(step) if step.outstanding == [*call] => {}
                    _ => return Err(Rejected::Busy),
                }
                self.head = *head;
                self.binding = binding.clone();
                self.drop_results();
                self.state = round(awaits);
            }
            TaskEvent::Moved(Move::Reseed { head, binding, .. }) => {
                if self.draining.is_some() {
                    return Err(Rejected::Draining);
                }
                if !matches!(self.state, S::Ready | S::AwaitingMax | S::Listening) {
                    return Err(Rejected::Busy);
                }
                self.head = *head;
                self.binding = binding.clone();
                self.drop_results();
            }
            TaskEvent::Moved(Move::Rewind { head }) => {
                if !matches!(self.state, S::Ready | S::AwaitingMax | S::Listening) {
                    return Err(Rejected::Busy);
                }
                self.head = *head;
                self.drop_results();
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

    fn model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        at: Timestamp,
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
                let rendered = |p: &Pending| call.renders.iter().any(|r| p.input.is(r));
                if self
                    .pending
                    .iter()
                    .any(|p| matches!(p.input, Input::Result(_)) && !rendered(p))
                {
                    return Err(Rejected::Unanswered);
                }
                self.pending.retain(|p| !rendered(p));
                self.head = call.head;
                self.state = TaskState::Thinking(id);
            }
            ModelCallEvent::FirstByte => {}
            ModelCallEvent::Ended { ending, .. } => {
                if self.state != TaskState::Thinking(id) {
                    return Err(Rejected::Busy);
                }
                match ending {
                    Ending::Responded { entry, stop, calls } => {
                        self.head = *entry;
                        self.failures = 0;
                        if !calls.is_empty() {
                            self.state = TaskState::Acting(Step {
                                response: Some(*entry),
                                outstanding: calls.clone(),
                            });
                        } else if matches!(stop, Stop::EndTurn | Stop::Refusal) {
                            self.turn_ended(*entry, at);
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
                    Ending::Abandoned { .. } | Ending::Lost => self.drained(at),
                }
            }
        }
        Ok(())
    }

    fn call(&mut self, id: CallId, event: &CallEvent, at: Timestamp) -> Result<(), Rejected> {
        match event {
            CallEvent::Made(call) => {
                let wakes = match &call.caller {
                    Caller::Agent {
                        task,
                        tool_use: None,
                    } if *task == self.id => Wakes::Always,
                    Caller::Max { chat: Some(task) } if *task == self.id => Wakes::Never,
                    _ => return Err(Rejected::NotForTask),
                };
                self.watching.push(Watched { call: id, wakes });
                Ok(())
            }
            CallEvent::Dispatched { .. } => Ok(()),
            CallEvent::Settled(_) => self.settled(id, at),
            CallEvent::Late(_) => {
                self.pending.push(Pending {
                    input: Input::Activity(id),
                    wakes: Wakes::Always,
                });
                self.woken(Wakes::Always);
                Ok(())
            }
        }
    }

    /// A call this task waits on has settled: one of its round, or one it
    /// watches. Also how [`crate::state::State`] feeds in attachments that
    /// had settled before the task or its new context existed.
    pub fn settled(&mut self, call: CallId, at: Timestamp) -> Result<(), Rejected> {
        if let TaskState::Acting(step) = &mut self.state
            && let Some(i) = step.outstanding.iter().position(|c| *c == call)
        {
            step.outstanding.remove(i);
            let done = step.outstanding.is_empty();
            self.pending.push(Pending {
                input: Input::Result(call),
                wakes: Wakes::Never,
            });
            if done {
                self.drained(at);
            }
            return Ok(());
        }
        let Some(i) = self.watching.iter().position(|w| w.call == call) else {
            return Err(Rejected::NotForTask);
        };
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
        } else {
            self.woken(watched.wakes);
        }
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, by: &Author) {
        let wakes = delivery.content.wakes();
        self.pending.push(Pending {
            input: Input::Delivery(delivery.id),
            wakes,
        });
        if matches!(delivery.content, Content::Words(_)) && matches!(by, Author::Max { .. }) {
            self.owed_since = None;
        }
        if self.draining.is_none() {
            self.woken(wakes);
        }
    }

    /// An input arrived: if it ends the wait the task is in, it is the
    /// task's move again.
    fn woken(&mut self, wakes: Wakes) {
        if wakes.ends(&self.state) {
            self.state = TaskState::Ready;
        }
    }

    /// The model ended its turn at `entry`: complete, wait, or carry on.
    fn turn_ended(&mut self, entry: EntryId, at: Timestamp) {
        if self.draining.is_some() {
            return self.drained(at);
        }
        if self.pending.iter().any(|p| p.wakes == Wakes::Always) {
            // Max's words, or a prompt, arrived during the turn.
            self.state = TaskState::Ready;
            return;
        }
        if self.mode == Mode::Autonomous && !self.has_background() {
            self.state = TaskState::Done(Outcome::Result { entry });
            return;
        }
        self.wait(at);
    }

    /// Enter the wait the mode asks for, unless something pending ends it.
    fn wait(&mut self, at: Timestamp) {
        if self.mode == Mode::UserFacing {
            self.await_max(at);
        } else {
            self.state = TaskState::Listening;
            if self.pending.iter().any(|p| p.wakes.ends(&self.state)) {
                self.state = TaskState::Ready;
            }
        }
    }

    fn await_max(&mut self, at: Timestamp) {
        if self.pending.iter().any(|p| p.wakes == Wakes::Always) {
            self.state = TaskState::Ready;
            return;
        }
        self.owed_since.get_or_insert(at);
        self.state = TaskState::AwaitingMax;
    }

    /// Nothing in the round is outstanding: carry on, or do what the drain
    /// says.
    fn drained(&mut self, at: Timestamp) {
        match self.draining.clone() {
            None => self.state = TaskState::Ready,
            Some(Drain::ToMax) => {
                self.draining = None;
                self.await_max(at);
            }
            Some(Drain::Stop(_) | Drain::Fail(_)) if self.has_background() => {
                self.state = TaskState::Listening;
            }
            Some(Drain::Stop(by)) => {
                self.draining = None;
                self.state = TaskState::Done(Outcome::Cancelled { by });
            }
            Some(Drain::Fail(reason)) => {
                self.draining = None;
                self.state = TaskState::Done(Outcome::Failed { reason });
            }
        }
    }

    /// The old context's tool results have nowhere to go in a new one. Max's
    /// words and actions stay pending: they are for the task, not the
    /// context.
    fn drop_results(&mut self) {
        self.pending
            .retain(|p| !matches!(p.input, Input::Result(_)));
    }
}

/// The state a context starts in: its attachments' round, or ready.
fn round(awaits: &[CallId]) -> TaskState {
    if awaits.is_empty() {
        TaskState::Ready
    } else {
        TaskState::Acting(Step {
            response: None,
            outstanding: awaits.to_vec(),
        })
    }
}
