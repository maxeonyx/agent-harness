//! The fold of the journal: every state anyone reads.

use std::collections::{BTreeMap, BTreeSet};

use crate::attention::{Queued, Ranking};
use crate::call::{Call, CallEvent, Caller, Outcome as CallOutcome};
use crate::context::Entry;
use crate::context::EntryId;
use crate::delivery::{Content, Delivery, Prompt};
use crate::environment::Environment;
use crate::environment::{Fork, Limb, LimbKind, Reach};
use crate::id::{
    CallId, DeliveryId, EnvironmentId, FaceId, InstanceId, LimbId, ModelCallId, Seq, TaskId,
    Timestamp, ValueId,
};
use crate::journal::{Author, Fact, Launched, Record, Rejected};
use crate::model_call::{Ending, ModelCall, ModelCallEvent, Purpose, Quota, Stop, Usage};
use crate::resource::{Change, ResourceKey, Version};
use crate::task::{
    Created, Drain, Mode, Move, Origin, Scope, Start, Stopper, Task, TaskEvent, TaskState,
};

/// Everything the journal says is true now.
///
/// **Concept.** The fold of every record, and the only door into the model.
/// [`State::apply`] folds one record, or refuses it and changes nothing;
/// [`crate::journal::Journal`] is the only caller that keeps it in step
/// with a store. Every view a face, the scheduler or the brain's loop needs
/// is a method here, computed from the folded identities rather than kept
/// beside them, so none can disagree with another: who reads a call is
/// read off the tasks ([`Task::awaited`]), what a task is waiting on is read
/// off its state, and the brain's whole to-do list is [`State::todo`].
///
/// It holds the context tree itself ([`State::entry`], [`State::context`]),
/// learned from the facts that carry entries, so a request's head, a
/// breakpoint and a rewind are checked against the tree, and a face renders
/// any context from the fold and the value store alone.
///
/// What it guarantees, by refusing every record that would break it: a task
/// is in exactly one [`TaskState`]; no request leaves a tool use unanswered,
/// is sent while its task drains, or grows a context other than its task's;
/// a scope's call settles only when every member is done, nothing joins a
/// closed scope, and a meta limb starts only what it lists; a call by an
/// agent goes only along its binding; a standing task lives only in Max's
/// scope; a fork starts where its source is and never through a limb that
/// refuses forks; a settled call moves again only from unknown to its real
/// outcome; a meta call is never run twice; a limb states only its own
/// calls' outcomes and its own resources' changes; and Max's facts say what
/// he had seen.
///
/// What it leaves to the brain, as policy over these views: what to render
/// and when (notes, wording, breakpoints), keep-alive and handover timing,
/// retry and give-up thresholds, holding siblings until the first one's
/// first byte, and the scheduler's ranking.
///
/// **Vision.** "Strictly events + derived views"; the tree Max watches
/// ([`State::waiting_on`] is the throbber); his queue ([`State::queue`]);
/// restart (a fresh brain folds the journal and carries on from
/// [`State::todo`] with nothing else to recover); structured concurrency
/// (stops cascade, scopes close).
///
/// **Existing harnesses.** Each keeps its live state in memory beside its
/// store and reconciles on load: Oh My Pi rebuilds a session's context from
/// its entry tree on resume, Codex reconstructs a thread from its rollout,
/// OpenCode's session status lives in memory only, so a restart forgets
/// which subagents were running. Here there is no live state but the fold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    last: Option<Seq>,
    launch: Option<(Mark, Launched)>,
    entries: BTreeMap<EntryId, Entry>,
    tasks: BTreeMap<TaskId, Task>,
    calls: BTreeMap<CallId, CallRecord>,
    model_calls: BTreeMap<ModelCallId, ModelCallRecord>,
    deliveries: BTreeMap<DeliveryId, Delivered>,
    environments: BTreeMap<EnvironmentId, Environment>,
    limbs: BTreeMap<LimbId, Limb>,
    reach: BTreeMap<LimbId, Reached>,
    resources: BTreeMap<ResourceKey, Current>,
    ranking: Ranking,
    focus: BTreeMap<FaceId, TaskId>,
    limited_until: Option<Timestamp>,
}

/// Where and when something was recorded. The position orders it against
/// every other fact (was this sent before the current connection?); the
/// time is for people.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub seq: Seq,
    pub at: Timestamp,
}

impl Mark {
    fn of(record: &Record) -> Mark {
        Mark {
            seq: record.seq,
            at: record.at,
        }
    }
}

/// A call and where it is in its life.
#[derive(Clone, Debug, PartialEq)]
pub struct CallRecord {
    pub call: Call,
    /// The limb that runs it: its caller's, or for an agent, its task's
    /// binding's route for the tool. `None` when the binding has no such
    /// tool, which the brain answers as a failed call
    /// ([`Todo::Unroutable`]).
    pub limb: Option<LimbId>,
    pub made: Mark,
    pub dispatched: Option<Dispatch>,
    /// When its limb was last asked to stop it.
    pub stopping: Option<Mark>,
    pub settled: Option<Settlement>,
    /// The real outcome of a call first settled as unknown.
    pub late: Option<Settlement>,
}

impl CallRecord {
    /// The outcome a context is shown: the real one, if it arrived late.
    pub fn outcome(&self) -> Option<&CallOutcome> {
        self.late
            .as_ref()
            .or(self.settled.as_ref())
            .map(|s| &s.outcome)
    }
}

/// A call's sends to its limb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dispatch {
    /// The first send: since when it has been running.
    pub first: Mark,
    /// The latest send, which decides whether it must be sent again.
    pub last: Mark,
    /// The instance it was last sent to; `None` for a limb the brain runs.
    pub instance: Option<InstanceId>,
}

/// How a call ended, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Settlement {
    pub outcome: CallOutcome,
    pub at: Mark,
}

/// A request and what came of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelCallRecord {
    pub call: ModelCall,
    pub sent: Mark,
    pub first_byte: Option<Mark>,
    pub ended: Option<Ended>,
    /// The entry its response is, for a turn that responded.
    pub response: Option<EntryId>,
    /// The calls of its response's tool uses, in order.
    pub calls: Vec<CallId>,
}

/// How a request ended, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub at: Mark,
    pub ending: Ending,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

/// A delivery, when, and by whom.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivered {
    pub delivery: Delivery,
    pub at: Mark,
    pub by: Author,
}

/// A resource's current version, who moved it there, and what its owner
/// said a context should be shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Current {
    pub version: Version,
    pub by: Option<CallId>,
    pub content: Option<ValueId>,
    pub at: Mark,
}

/// The brain's connection to a limb, as last recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct Reached {
    pub reach: Reach,
    pub at: Mark,
}

/// Something a task's context embodies that has moved since, and that it
/// has not been told of: one line of its next request's notice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stale {
    pub resource: ResourceKey,
    pub has: Version,
    pub now: Version,
    /// The call that moved it, whose caller the notice names.
    pub by: Option<CallId>,
    /// What its limb said to show rather than announce.
    pub content: Option<ValueId>,
}

/// What a task is waiting on, and since when: the throbber, and the answer
/// to "why is nothing happening?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// On the brain to send its next request.
    Brain {
        since: Timestamp,
    },
    /// Held by Max, on itself or an ancestor.
    Held {
        since: Timestamp,
    },
    /// On the subscription's limit.
    Limited {
        until: Timestamp,
        since: Timestamp,
    },
    /// On the model: sent at `since`, streaming once its first byte came.
    Model {
        call: ModelCallId,
        since: Timestamp,
        streaming: bool,
    },
    /// On its round's calls: tools, its children (a spawn), attachments. A
    /// face shows a call whose limb is unreached as waiting on reconnect.
    Calls {
        calls: Vec<CallId>,
        since: Timestamp,
    },
    /// On Max, since `since`: its turn ended, he interrupted, or it asked.
    Max {
        since: Timestamp,
        ask: Option<CallId>,
    },
    /// On a change to what it embodies, or the background work it started.
    Listening {
        background: Vec<CallId>,
        since: Timestamp,
    },
    Done {
        since: Timestamp,
    },
}

/// One thing the brain should do now. The whole of its loop: act on each,
/// commit what happened, repeat. Each item stays in the list until the fact
/// that does it is committed, so a restart needs nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    /// Build and send the task's next request, rendering every pending
    /// input that the policy includes (every result, always).
    Send(TaskId),
    /// A request an earlier brain sent and never saw end: end it as
    /// [`Ending::Lost`], after which its task sends again.
    Lost(ModelCallId),
    /// Send the call to its limb (`ToLimb::Run`), or for a limb the brain
    /// runs, run it and commit its effects with the dispatch. A plugin's
    /// call is due again whenever its limb has connected since it was last
    /// sent.
    Dispatch(CallId),
    /// The call's tool is not in its task's binding: settle it as failed,
    /// saying so.
    Unroutable(CallId),
    /// Stop the call. If it was dispatched, ask its limb (`ToLimb::Stop`)
    /// and commit [`CallEvent::Stopping`]; otherwise settle it as
    /// interrupted.
    Interrupt(CallId),
    /// Stop reading the task's response.
    Abandon(ModelCallId),
    /// Stop a member of a scope whose owner is stopping.
    Stop(TaskId),
    /// Every member of this spawn's scope is done: settle it with their
    /// outcomes.
    Settle(CallId),
}

impl State {
    /// Fold one record, or refuse it and change nothing.
    pub fn apply(&mut self, record: &Record) -> Result<(), Rejected> {
        let expected = self.next();
        if record.seq != expected {
            return Err(Rejected::OutOfOrder { expected });
        }
        if !self.authorised(&record.by, &record.fact) {
            return Err(Rejected::Author);
        }
        let mark = Mark::of(record);
        match &record.fact {
            Fact::Launched(launched) => {
                if let Some(call) = launched.by
                    && !self.calls.contains_key(&call)
                {
                    return Err(Rejected::Unknown);
                }
                self.launch = Some((mark, launched.clone()));
                // A connection belongs to the brain that made it.
                self.reach.clear();
            }
            Fact::Task(id, TaskEvent::Created(created)) => self.create(*id, created, record)?,
            Fact::Task(id, event) => self.task_event(*id, event, record)?,
            Fact::ModelCall(id, event) => self.fold_model_call(*id, event, record)?,
            Fact::Call(id, event) => self.fold_call(*id, event, record)?,
            Fact::Delivery(delivery) => self.deliver(delivery, record)?,
            Fact::Change(change) => self.change(change, record)?,
            Fact::Environment(environment) => {
                if !environment.limbs.iter().all(|l| self.limbs.contains_key(l)) {
                    return Err(Rejected::Unknown);
                }
                self.environments
                    .insert(environment.id, environment.clone());
            }
            Fact::Limb(limb) => {
                self.limbs.insert(limb.id, limb.clone());
            }
            Fact::Reach { limb, reach } => {
                let defined = self.limbs.get(limb).ok_or(Rejected::Unknown)?;
                if !matches!(defined.kind, LimbKind::Plugin { .. }) {
                    return Err(Rejected::NotForTask);
                }
                self.reach.insert(
                    *limb,
                    Reached {
                        reach: reach.clone(),
                        at: mark,
                    },
                );
            }
            Fact::Ranking(ranking) => self.ranking = ranking.clone(),
            Fact::Focus(focus) => match focus.task {
                Some(task) => {
                    self.focus.insert(focus.face, task);
                }
                None => {
                    self.focus.remove(&focus.face);
                }
            },
        }
        self.last = Some(record.seq);
        Ok(())
    }

    /// Whether `by` may state `fact`. The brain states anything but Max's
    /// words. A limb states only how its own dispatched calls ended and how
    /// its own resources changed. Max states his words, his calls and his
    /// answers, his controls over the tree, his own presence, and the
    /// definitions of environments and limbs; never what a model or a limb
    /// did.
    fn authorised(&self, by: &Author, fact: &Fact) -> bool {
        match by {
            Author::Brain => !matches!(
                fact,
                Fact::Delivery(Delivery {
                    content: Content::Words(_),
                    ..
                })
            ),
            Author::Limb(limb) => match fact {
                Fact::Call(id, CallEvent::Settled(_)) => self
                    .calls
                    .get(id)
                    .is_some_and(|c| c.limb == Some(*limb) && c.dispatched.is_some()),
                Fact::Change(change) => {
                    change.resource.limb == *limb
                        && change.by.is_none_or(|call| {
                            self.calls.get(&call).is_some_and(|c| c.limb == Some(*limb))
                        })
                }
                _ => false,
            },
            Author::Max { face: _, seen } => {
                *seen < self.next()
                    && match fact {
                        Fact::Task(_, event) => match event {
                            TaskEvent::Created(created) => created.scope == Scope::Max,
                            TaskEvent::Drain(drain) => {
                                matches!(drain, Drain::ToMax | Drain::Stop(Stopper::Max))
                            }
                            TaskEvent::Moved(movement) => matches!(movement, Move::Rewind { .. }),
                            TaskEvent::ModeSet(_) | TaskEvent::Renamed(_) | TaskEvent::Held(_) => {
                                true
                            }
                        },
                        Fact::Delivery(delivery) => matches!(
                            delivery.content,
                            Content::Words(_) | Content::Prompt(Prompt::Done)
                        ),
                        Fact::Call(_, CallEvent::Made(call)) => {
                            matches!(call.caller, Caller::Max { .. })
                        }
                        Fact::Call(id, CallEvent::Settled(_)) => self
                            .calls
                            .get(id)
                            .and_then(|c| c.limb)
                            .is_some_and(|l| self.is_max(l)),
                        Fact::Change(change) => self.is_max(change.resource.limb),
                        Fact::Environment(_)
                        | Fact::Limb(_)
                        | Fact::Ranking(_)
                        | Fact::Focus(_) => true,
                        Fact::Launched(_)
                        | Fact::ModelCall(..)
                        | Fact::Call(..)
                        | Fact::Reach { .. } => false,
                    }
            }
        }
    }

    fn create(&mut self, id: TaskId, created: &Created, record: &Record) -> Result<(), Rejected> {
        if self.tasks.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        if let Scope::Spawn(spawn) = created.scope {
            let call = self.calls.get(&spawn).ok_or(Rejected::Unknown)?;
            if call.settled.is_some() {
                return Err(Rejected::Finished);
            }
            let Some(LimbKind::Meta { starts }) =
                call.limb.and_then(|l| self.limbs.get(&l)).map(|l| &l.kind)
            else {
                return Err(Rejected::WrongScope);
            };
            let allowed = match &created.origin {
                Origin::Fresh { agent, start } => starts
                    .iter()
                    .any(|s| s.agent == *agent && s.environment == start.binding.environment),
                // The parent's fork, or a member forking itself to add a
                // sibling.
                Origin::Fork { from } => {
                    self.call_owner(spawn) == Some(*from)
                        || self
                            .tasks
                            .get(from)
                            .is_some_and(|t| t.scope == created.scope)
                }
                Origin::Resume { .. } | Origin::Aside { .. } => false,
            };
            if !allowed || created.mode == Mode::Standing {
                return Err(Rejected::WrongScope);
            }
        }
        match &created.origin {
            Origin::Fresh { start, .. } => self.check_start(start)?,
            Origin::Fork { from } | Origin::Aside { from } => {
                let source = self.tasks.get(from).ok_or(Rejected::Unknown)?;
                let refuses = source.binding.routes.iter().any(|r| {
                    self.limbs
                        .get(&r.limb)
                        .is_some_and(|l| l.fork == Fork::Refuse)
                });
                if refuses {
                    return Err(Rejected::WrongEnvironment);
                }
            }
            Origin::Resume { .. } => {}
        }
        let task = Task::new(id, created, record.at, self)?;
        if let Origin::Fresh { start, .. } = &created.origin {
            self.entries
                .insert(start.root.id(), Entry::Root(start.root.clone()));
        }
        self.tasks.insert(id, task);
        Ok(())
    }

    /// A new root and its binding must stay inside one environment, route
    /// each tool once, and attach only the harness's calls along the
    /// binding.
    fn check_start(&self, start: &Start) -> Result<(), Rejected> {
        let environment = self
            .environments
            .get(&start.binding.environment)
            .ok_or(Rejected::Unknown)?;
        let mut tools = BTreeSet::new();
        for route in &start.binding.routes {
            if !environment.limbs.contains(&route.limb) {
                return Err(Rejected::WrongEnvironment);
            }
            if !tools.insert(route.tool.as_str()) {
                return Err(Rejected::Ambiguous);
            }
        }
        for id in &start.awaits {
            let call = self.calls.get(id).ok_or(Rejected::Unknown)?;
            let Caller::Meta { limb, .. } = call.call.caller else {
                return Err(Rejected::NotForTask);
            };
            if start.binding.limb(&call.call.tool) != Some(limb) {
                return Err(Rejected::WrongEnvironment);
            }
        }
        Ok(())
    }

    fn task_event(
        &mut self,
        id: TaskId,
        event: &TaskEvent,
        record: &Record,
    ) -> Result<(), Rejected> {
        let task = self.tasks.get(&id).ok_or(Rejected::Unknown)?;
        let mut root = None;
        match event {
            TaskEvent::Moved(Move::Handover { call, start }) => {
                let made = self.calls.get(call).ok_or(Rejected::Unknown)?;
                if !matches!(made.call.caller, Caller::Agent { task, .. } if task == id) {
                    return Err(Rejected::NotForTask);
                }
                if start.binding.environment != task.binding.environment {
                    return Err(Rejected::WrongEnvironment);
                }
                self.check_start(start)?;
                root = Some(&start.root);
            }
            TaskEvent::Moved(Move::Reseed(start)) => {
                if start.binding.environment != task.binding.environment {
                    return Err(Rejected::WrongEnvironment);
                }
                self.check_start(start)?;
                root = Some(&start.root);
            }
            TaskEvent::Moved(Move::Rewind { to }) => {
                let on_chain = self.descends(task.head, *to) && self.descends(*to, task.start);
                if !on_chain || !self.ended_turn(*to) {
                    return Err(Rejected::Detached);
                }
            }
            _ => {}
        }
        let mut task = task.clone();
        task.apply(record, self)?;
        if let Some(root) = root {
            self.entries.insert(root.id(), Entry::Root(root.clone()));
        }
        self.tasks.insert(id, task);
        Ok(())
    }

    /// Whether some request's response is `entry` and ended its model turn.
    fn ended_turn(&self, entry: EntryId) -> bool {
        self.model_calls.values().any(|m| {
            m.response == Some(entry)
                && matches!(
                    &m.ended,
                    Some(Ended {
                        ending: Ending::Responded {
                            stop: Stop::EndTurn | Stop::Refusal,
                            ..
                        },
                        ..
                    })
                )
        })
    }

    fn fold_model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        record: &Record,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            ModelCallEvent::Sent(call) => {
                if self.model_calls.contains_key(&id) {
                    return Err(Rejected::Exists);
                }
                let task = self.tasks.get(&call.task).ok_or(Rejected::Unknown)?;
                if !self.entries.contains_key(&call.base) {
                    return Err(Rejected::Detached);
                }
                let entries = call.entries();
                let on_chain = |at: EntryId| {
                    entries.iter().any(|(e, _)| *e == at) || self.descends(call.base, at)
                };
                if !call.breakpoints.iter().all(|b| on_chain(b.at)) {
                    return Err(Rejected::Detached);
                }
                let task = match call.purpose {
                    Purpose::Turn => {
                        let mut task = task.clone();
                        task.apply(record, self)?;
                        Some(task)
                    }
                    // A keep-alive resends a context its task has, and
                    // moves nothing.
                    Purpose::KeepAlive if self.descends(task.head, call.head()) => None,
                    Purpose::KeepAlive => return Err(Rejected::Detached),
                };
                if let Some(task) = task {
                    self.tasks.insert(task.id, task);
                }
                self.entries.extend(entries);
                self.model_calls.insert(
                    id,
                    ModelCallRecord {
                        call: call.clone(),
                        sent: mark,
                        first_byte: None,
                        ended: None,
                        response: None,
                        calls: Vec::new(),
                    },
                );
            }
            ModelCallEvent::FirstByte => {
                let sent = self.model_calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if sent.ended.is_some() {
                    return Err(Rejected::Finished);
                }
                sent.first_byte.get_or_insert(mark);
            }
            ModelCallEvent::Ended {
                ending,
                usage,
                quota,
            } => {
                let sent = self.model_calls.get(&id).ok_or(Rejected::Unknown)?;
                if sent.ended.is_some() {
                    return Err(Rejected::Finished);
                }
                // Calls are made only for a response that completed.
                if !sent.calls.is_empty() && !matches!(ending, Ending::Responded { .. }) {
                    return Err(Rejected::Busy);
                }
                let turn = sent.call.purpose == Purpose::Turn;
                let response = match ending {
                    Ending::Responded { content, .. } if turn => Some(sent.call.response(*content)),
                    _ => None,
                };
                let task = if turn {
                    let mut task = self
                        .tasks
                        .get(&sent.call.task)
                        .ok_or(Rejected::Unknown)?
                        .clone();
                    task.apply(record, self)?;
                    Some(task)
                } else {
                    None
                };
                if let Some(task) = task {
                    self.tasks.insert(task.id, task);
                }
                if let Ending::Limited { until } = ending {
                    self.limited_until = self.limited_until.max(Some(*until));
                }
                let response = response.map(|entry| {
                    let entry_id = entry.id();
                    self.entries.insert(entry_id, entry);
                    entry_id
                });
                if let Some(sent) = self.model_calls.get_mut(&id) {
                    sent.ended = Some(Ended {
                        at: mark,
                        ending: ending.clone(),
                        usage: *usage,
                        quota: quota.clone(),
                    });
                    sent.response = response;
                }
            }
        }
        Ok(())
    }

    fn fold_call(
        &mut self,
        id: CallId,
        event: &CallEvent,
        record: &Record,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            CallEvent::Made(call) => self.make(id, call, record),
            CallEvent::Dispatched => {
                let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                let limb = made
                    .limb
                    .and_then(|l| self.limbs.get(&l))
                    .ok_or(Rejected::WrongEnvironment)?;
                let instance = match &limb.kind {
                    LimbKind::Max => return Err(Rejected::Busy),
                    // Run once, with its effects, so never twice.
                    LimbKind::Meta { .. } | LimbKind::Attention if made.dispatched.is_some() => {
                        return Err(Rejected::Busy);
                    }
                    LimbKind::Meta { .. } | LimbKind::Attention => None,
                    LimbKind::Plugin { .. } => {
                        Some(self.connected(limb.id).ok_or(Rejected::Busy)?.0)
                    }
                };
                if !self.acting_on(id, &made.call) {
                    return Err(Rejected::Busy);
                }
                let first = made.dispatched.map_or(mark, |d| d.first);
                if let Some(made) = self.calls.get_mut(&id) {
                    made.dispatched = Some(Dispatch {
                        first,
                        last: mark,
                        instance,
                    });
                }
                Ok(())
            }
            CallEvent::Stopping => {
                let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                if made.dispatched.is_none() {
                    return Err(Rejected::Busy);
                }
                made.stopping = Some(mark);
                Ok(())
            }
            CallEvent::Settled(outcome) => self.settle(id, outcome, record),
        }
    }

    fn make(&mut self, id: CallId, call: &Call, record: &Record) -> Result<(), Rejected> {
        if self.calls.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        let mut thinking = None;
        let mut reader = None;
        let limb = match &call.caller {
            Caller::Agent { task, tool_use } => {
                let task = self.tasks.get(task).ok_or(Rejected::Unknown)?;
                let limb = task.binding.limb(&call.tool);
                match (tool_use, &task.state) {
                    (Some(_), TaskState::Thinking(request)) => thinking = Some(*request),
                    (Some(_), _) => return Err(Rejected::Busy),
                    (None, _) if limb.is_none() => return Err(Rejected::WrongEnvironment),
                    (None, _) => {
                        let mut task = task.clone();
                        task.apply(record, self)?;
                        reader = Some(task);
                    }
                }
                limb
            }
            Caller::Max { limb, chat } => {
                if !self.limbs.contains_key(limb) {
                    return Err(Rejected::Unknown);
                }
                if let Some(chat) = chat {
                    let task = self.tasks.get(chat).ok_or(Rejected::Unknown)?;
                    if !task.binding.routes.iter().any(|r| r.limb == *limb) {
                        return Err(Rejected::WrongEnvironment);
                    }
                    let mut task = task.clone();
                    task.apply(record, self)?;
                    reader = Some(task);
                }
                Some(*limb)
            }
            Caller::Meta { call: by, limb } => {
                let by = self.calls.get(by).ok_or(Rejected::Unknown)?;
                if by.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                if !self.limbs.contains_key(limb) {
                    return Err(Rejected::Unknown);
                }
                Some(*limb)
            }
        };
        if let Some(request) = thinking
            && let Some(sent) = self.model_calls.get_mut(&request)
        {
            sent.calls.push(id);
        }
        if let Some(task) = reader {
            self.tasks.insert(task.id, task);
        }
        self.calls.insert(
            id,
            CallRecord {
                call: call.clone(),
                limb,
                made: Mark::of(record),
                dispatched: None,
                stopping: None,
                settled: None,
                late: None,
            },
        );
        Ok(())
    }

    fn settle(
        &mut self,
        id: CallId,
        outcome: &CallOutcome,
        record: &Record,
    ) -> Result<(), Rejected> {
        let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
        let late = match &made.settled {
            None => false,
            Some(first) if first.outcome == CallOutcome::Unknown && made.late.is_none() => true,
            Some(_) => return Err(Rejected::Finished),
        };
        if !late && self.members(Scope::Spawn(id)).any(|t| !t.is_done()) {
            return Err(Rejected::Busy);
        }
        // Who reads it: whoever awaits it now, or for a late outcome, the
        // context its first outcome reached, if it is still live.
        let readers: Vec<&Task> = if late {
            match made.call.caller {
                Caller::Agent { task, .. }
                | Caller::Max {
                    chat: Some(task), ..
                } => self
                    .tasks
                    .get(&task)
                    .filter(|t| !t.is_done())
                    .into_iter()
                    .collect(),
                Caller::Max { chat: None, .. } | Caller::Meta { .. } => Vec::new(),
            }
        } else {
            self.tasks.values().filter(|t| t.awaits(id)).collect()
        };
        let mut touched = Vec::with_capacity(readers.len());
        for reader in readers {
            let mut task = reader.clone();
            task.apply(record, self)?;
            touched.push(task);
        }
        for task in touched {
            self.tasks.insert(task.id, task);
        }
        let settlement = Settlement {
            outcome: outcome.clone(),
            at: Mark::of(record),
        };
        if let Some(made) = self.calls.get_mut(&id) {
            if late {
                made.late = Some(settlement);
            } else {
                made.settled = Some(settlement);
            }
        }
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, record: &Record) -> Result<(), Rejected> {
        if self.deliveries.contains_key(&delivery.id) {
            return Err(Rejected::Exists);
        }
        if let Content::Aside(aside) = delivery.content
            && !self.tasks.contains_key(&aside)
        {
            return Err(Rejected::Unknown);
        }
        let mut task = self
            .tasks
            .get(&delivery.to)
            .ok_or(Rejected::Unknown)?
            .clone();
        task.apply(record, self)?;
        self.tasks.insert(task.id, task);
        self.deliveries.insert(
            delivery.id,
            Delivered {
                delivery: delivery.clone(),
                at: Mark::of(record),
                by: record.by.clone(),
            },
        );
        Ok(())
    }

    fn change(&mut self, change: &Change, record: &Record) -> Result<(), Rejected> {
        if !self.limbs.contains_key(&change.resource.limb) {
            return Err(Rejected::Unknown);
        }
        if let Some(by) = change.by
            && !self.calls.contains_key(&by)
        {
            return Err(Rejected::Unknown);
        }
        let mut woken = Vec::new();
        for task in self.tasks.values() {
            if task.mode == Mode::Standing && task.state == TaskState::Listening {
                let mut task = task.clone();
                task.apply(record, self)?;
                woken.push(task);
            }
        }
        for task in woken {
            self.tasks.insert(task.id, task);
        }
        self.resources.insert(
            change.resource.clone(),
            Current {
                version: change.version,
                by: change.by,
                content: change.content,
                at: Mark::of(record),
            },
        );
        Ok(())
    }

    // Reading.

    /// The position the next record must have.
    pub fn next(&self) -> Seq {
        Seq(self.last.map_or(1, |s| s.0 + 1))
    }

    /// The last position folded.
    pub fn last(&self) -> Option<Seq> {
        self.last
    }

    /// The brain writing the journal now, and where it began.
    pub fn launch(&self) -> Option<(Mark, &Launched)> {
        self.launch
            .as_ref()
            .map(|(mark, launched)| (*mark, launched))
    }

    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// The context that ends at `head`, root first: exactly what a request
    /// with that head sent, given the values its entries name. Empty for an
    /// entry the journal has never carried.
    pub fn context(&self, head: EntryId) -> Vec<(EntryId, &Entry)> {
        let mut chain = Vec::new();
        let mut next = Some(head);
        while let Some(id) = next {
            let Some(entry) = self.entries.get(&id) else {
                break;
            };
            chain.push((id, entry));
            next = entry.parent();
        }
        chain.reverse();
        chain
    }

    /// Whether `ancestor` is on the chain ending at `entry`, itself included.
    pub fn descends(&self, entry: EntryId, ancestor: EntryId) -> bool {
        let mut next = Some(entry);
        while let Some(id) = next {
            if id == ancestor {
                return true;
            }
            next = self.entries.get(&id).and_then(Entry::parent);
        }
        false
    }

    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    pub fn call(&self, id: CallId) -> Option<&CallRecord> {
        self.calls.get(&id)
    }

    /// Every call, in id order.
    pub fn calls(&self) -> impl Iterator<Item = (CallId, &CallRecord)> {
        self.calls.iter().map(|(id, record)| (*id, record))
    }

    pub fn model_call(&self, id: ModelCallId) -> Option<&ModelCallRecord> {
        self.model_calls.get(&id)
    }

    /// Every request, in id order.
    pub fn model_calls(&self) -> impl Iterator<Item = (ModelCallId, &ModelCallRecord)> {
        self.model_calls.iter().map(|(id, record)| (*id, record))
    }

    pub fn delivery(&self, id: DeliveryId) -> Option<&Delivered> {
        self.deliveries.get(&id)
    }

    pub fn environment(&self, id: EnvironmentId) -> Option<&Environment> {
        self.environments.get(&id)
    }

    pub fn environments(&self) -> impl Iterator<Item = &Environment> {
        self.environments.values()
    }

    pub fn limb(&self, id: LimbId) -> Option<&Limb> {
        self.limbs.get(&id)
    }

    pub fn reach(&self, id: LimbId) -> Option<&Reached> {
        self.reach.get(&id)
    }

    /// The instance a limb is connected to under this brain, and since when.
    pub fn connected(&self, limb: LimbId) -> Option<(InstanceId, Mark)> {
        match self.reach.get(&limb)? {
            Reached {
                reach: Reach::Connected { instance },
                at,
            } => Some((*instance, *at)),
            Reached {
                reach: Reach::Lost, ..
            } => None,
        }
    }

    pub fn resource(&self, key: &ResourceKey) -> Option<&Current> {
        self.resources.get(key)
    }

    pub fn ranking(&self) -> &Ranking {
        &self.ranking
    }

    /// The task a face shows Max.
    pub fn focus(&self, face: FaceId) -> Option<TaskId> {
        self.focus.get(&face).copied()
    }

    /// Until when the account is limited, if it is at `now`.
    pub fn limited(&self, now: Timestamp) -> Option<Timestamp> {
        self.limited_until.filter(|until| *until > now)
    }

    /// The task that owns this one: the caller of its scope's spawn. `None`
    /// in Max's scope.
    pub fn owner(&self, task: TaskId) -> Option<TaskId> {
        match self.tasks.get(&task)?.scope {
            Scope::Max => None,
            Scope::Spawn(call) => self.call_owner(call),
        }
    }

    /// The task a call belongs to: its agent, or for the harness's call, the
    /// task whose call it runs for. Max's calls are his own.
    pub fn call_owner(&self, call: CallId) -> Option<TaskId> {
        match &self.calls.get(&call)?.call.caller {
            Caller::Agent { task, .. } => Some(*task),
            Caller::Max { .. } => None,
            Caller::Meta { call, .. } => self.call_owner(*call),
        }
    }

    /// The tasks in a scope. Scanned, not indexed: a few thousand tasks is
    /// the expected scale.
    pub fn members(&self, scope: Scope) -> impl Iterator<Item = &Task> {
        self.tasks.values().filter(move |t| t.scope == scope)
    }

    /// Whether the task or any ancestor is held.
    pub fn held(&self, task: TaskId) -> bool {
        let mut next = Some(task);
        while let Some(id) = next {
            if self.tasks.get(&id).is_some_and(|t| t.held) {
                return true;
            }
            next = self.owner(id);
        }
        false
    }

    /// Whether a call asks Max something.
    pub fn is_ask(&self, call: CallId) -> bool {
        self.calls
            .get(&call)
            .and_then(|c| c.limb)
            .is_some_and(|l| self.is_max(l))
    }

    fn is_max(&self, limb: LimbId) -> bool {
        self.limbs
            .get(&limb)
            .is_some_and(|l| l.kind == LimbKind::Max)
    }

    /// The task's outstanding question to Max, if its round has one.
    fn ask(&self, task: &Task) -> Option<CallId> {
        match &task.state {
            TaskState::Acting(step) => step.outstanding.iter().copied().find(|c| self.is_ask(*c)),
            _ => None,
        }
    }

    /// A tool use is acted on only once its response is complete and its
    /// task is acting on it.
    fn acting_on(&self, id: CallId, call: &Call) -> bool {
        match call.caller {
            Caller::Agent {
                task,
                tool_use: Some(_),
            } => self.tasks.get(&task).is_some_and(
                |t| matches!(&t.state, TaskState::Acting(step) if step.outstanding.contains(&id)),
            ),
            _ => true,
        }
    }

    /// What a task is waiting on now.
    pub fn waiting_on(&self, task: TaskId, now: Timestamp) -> Option<Waiting> {
        let t = self.tasks.get(&task)?;
        let since = t.since;
        Some(match &t.state {
            TaskState::Done(_) => Waiting::Done { since },
            TaskState::Ready if self.held(task) => Waiting::Held { since },
            TaskState::Ready => match self.limited(now) {
                Some(until) => Waiting::Limited { until, since },
                None => Waiting::Brain { since },
            },
            TaskState::Thinking(call) => {
                let sent = self.model_calls.get(call)?;
                Waiting::Model {
                    call: *call,
                    since: sent.sent.at,
                    streaming: sent.first_byte.is_some(),
                }
            }
            TaskState::Acting(step) => match self.ask(t) {
                Some(ask) => Waiting::Max {
                    since: t
                        .owed_since
                        .or(self.calls.get(&ask).map(|c| c.made.at))
                        .unwrap_or(since),
                    ask: Some(ask),
                },
                None => Waiting::Calls {
                    calls: step.outstanding.clone(),
                    since,
                },
            },
            TaskState::AwaitingMax => Waiting::Max {
                since: t.owed_since.unwrap_or(since),
                ask: None,
            },
            TaskState::Listening => Waiting::Listening {
                background: t
                    .watching
                    .iter()
                    .filter(|w| w.wakes)
                    .map(|w| w.call)
                    .collect(),
                since,
            },
        })
    }

    /// Max's queue, in order: every live task he owes a word or that has
    /// asked him something.
    pub fn queue(&self) -> Vec<Queued> {
        let queued = self
            .tasks
            .values()
            .filter(|t| !t.is_done())
            .filter_map(|t| {
                let ask = self.ask(t);
                let asked = ask.and_then(|a| self.calls.get(&a)).map(|c| c.made.at);
                Some(Queued {
                    task: t.id,
                    since: t.owed_since.or(asked)?,
                    ask,
                    why: None,
                })
            })
            .collect();
        self.ranking.order(queued)
    }

    /// What a task's context embodies that has moved since, and that it has
    /// not been told of: the next request's notice.
    pub fn stale(&self, task: &Task) -> Vec<Stale> {
        task.knows
            .iter()
            .filter_map(|(resource, known)| {
                let current = self.resources.get(resource)?;
                let moved = current.version != known.has && Some(current.version) != known.told;
                moved.then(|| Stale {
                    resource: resource.clone(),
                    has: known.has,
                    now: current.version,
                    by: current.by,
                    content: current.content,
                })
            })
            .collect()
    }

    /// Everything the brain should do now.
    ///
    /// Restart is not a mode: it is read off journal positions. A request
    /// sent before the current [`Launched`] with no end was lost. A plugin
    /// call is due whenever its limb's current connection (which is always
    /// this brain's) began after the call was last sent, so a restart of
    /// the brain, of the limb, or of the network between them all resend
    /// what is unsettled, and the limb answers each from its own record.
    pub fn todo(&self, now: Timestamp) -> Vec<Todo> {
        let launch = self.launch.as_ref().map(|(mark, _)| mark.seq);
        let lost = |sent: &ModelCallRecord| {
            sent.ended.is_none() && launch.is_some_and(|l| sent.sent.seq < l)
        };
        let mut todo: Vec<Todo> = self
            .model_calls
            .iter()
            .filter(|(_, sent)| lost(sent))
            .map(|(id, _)| Todo::Lost(*id))
            .collect();
        let mut readers: BTreeMap<CallId, Vec<&Task>> = BTreeMap::new();
        let mut scopes: BTreeMap<CallId, Vec<&Task>> = BTreeMap::new();
        for task in self.tasks.values() {
            if let Scope::Spawn(call) = task.scope {
                scopes.entry(call).or_default().push(task);
            }
            if task.is_done() {
                continue;
            }
            for call in task.awaited() {
                readers.entry(call).or_default().push(task);
            }
            match (&task.state, &task.draining) {
                (TaskState::Ready, None) if !self.held(task.id) && self.limited(now).is_none() => {
                    todo.push(Todo::Send(task.id));
                }
                (TaskState::Thinking(call), Some(_))
                    if !self.model_calls.get(call).is_some_and(lost) =>
                {
                    todo.push(Todo::Abandon(*call));
                }
                _ => {}
            }
        }
        for (id, record) in &self.calls {
            if record.settled.is_some() {
                continue;
            }
            if let Some(members) = scopes.get(id) {
                let stopping = self
                    .call_owner(*id)
                    .and_then(|o| self.tasks.get(&o))
                    .is_some_and(|o| ends(&o.draining));
                let live: Vec<&&Task> = members.iter().filter(|m| !m.is_done()).collect();
                if live.is_empty() {
                    todo.push(Todo::Settle(*id));
                } else if stopping {
                    let stop = live.iter().filter(|m| !ends(&m.draining));
                    todo.extend(stop.map(|m| Todo::Stop(m.id)));
                }
                continue;
            }
            let Some(limb) = record.limb.and_then(|l| self.limbs.get(&l)) else {
                todo.push(Todo::Unroutable(*id));
                continue;
            };
            let connection = self.connected(limb.id).map(|(_, mark)| mark.seq);
            let readers = readers.get(id).map_or(&[][..], Vec::as_slice);
            if interrupts(*id, &record.call, readers) {
                let due = match (&record.dispatched, &limb.kind) {
                    (None, _) => true,
                    (Some(_), LimbKind::Plugin { .. }) => {
                        connection.is_some_and(|c| record.stopping.is_none_or(|s| s.seq < c))
                    }
                    (Some(_), _) => false,
                };
                if due {
                    todo.push(Todo::Interrupt(*id));
                }
                continue;
            }
            let due = match &limb.kind {
                LimbKind::Max => false,
                LimbKind::Meta { .. } | LimbKind::Attention => record.dispatched.is_none(),
                LimbKind::Plugin { .. } => {
                    connection.is_some_and(|c| record.dispatched.is_none_or(|d| d.last.seq < c))
                }
            };
            if due && self.acting_on(*id, &record.call) {
                todo.push(Todo::Dispatch(*id));
            }
        }
        todo
    }
}

/// Whether a drain ends the task, rather than handing the move to Max.
fn ends(drain: &Option<Drain>) -> bool {
    matches!(drain, Some(Drain::Stop(_) | Drain::Fail(_)))
}

/// Whether every live task reading this call is draining in a way that
/// stops it. An interrupt stops the round, not background work; a stop
/// stops both. A shared attachment runs on while any sibling still wants
/// it. Max's own calls are never stopped for an agent.
fn interrupts(id: CallId, call: &Call, readers: &[&Task]) -> bool {
    if matches!(call.caller, Caller::Max { .. }) || readers.is_empty() {
        return false;
    }
    readers.iter().all(|t| match &t.draining {
        None => false,
        Some(Drain::ToMax) => {
            matches!(&t.state, TaskState::Acting(step) if step.outstanding.contains(&id))
        }
        Some(Drain::Stop(_) | Drain::Fail(_)) => true,
    })
}
