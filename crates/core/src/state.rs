//! The fold of the journal: every state anyone reads.

use std::collections::BTreeMap;

use crate::attention::{Queued, Ranking};
use crate::call::{Call, CallEvent, Caller, Outcome as CallOutcome, Status};
use crate::context::{Note, Rendered};
use crate::delivery::{Content, Delivery};
use crate::environment::{Environment, Limb, LimbKind, Reach};
use crate::id::{
    CallId, DeliveryId, EnvironmentId, FaceId, LimbId, ModelCallId, Seq, TaskId, Timestamp,
};
use crate::journal::{Author, Fact, Launched, Record, Rejected};
use crate::model_call::{Ending, ModelCall, ModelCallEvent, Purpose, Quota, Usage};
use crate::resource::{Observation, ResourceKey, Version, Writer};
use crate::task::{
    Created, Drain, Known, Mode, Move, Origin, Scope, Task, TaskEvent, TaskState, Wakes,
};

/// Everything the journal says is true now.
///
/// **Concept.** The only door into the model. [`State::apply`] folds one
/// record, or refuses it and changes nothing, so the brain can try a batch
/// on its state before appending it: a refused batch is never written, and
/// a race between Max and the brain (he stops a task that has just
/// finished) is answered from the same rules that keep the journal sound.
/// Every view a face, the scheduler or the brain's loop needs is a method
/// here, computed from the folded identities rather than kept beside them,
/// so none can disagree with another.
///
/// What it guarantees, by refusing every record that would break it: a
/// task is in exactly one [`TaskState`]; no request leaves a tool use
/// unanswered or is sent while its task drains; a scope's call settles only
/// when every member is done, and nothing joins a closed scope; a call by an
/// agent goes only along its binding's routes; a standing task lives only in
/// Max's scope; a fork starts where its source is; a settled call never
/// moves again.
///
/// What it leaves to the brain, as policy over these views: what to render
/// and when (notes, wording, breakpoints), keep-alive and handover timing,
/// retry and give-up thresholds, holding siblings until the first one's
/// first byte, and the scheduler's ranking.
///
/// **Vision.** "Strictly events + derived views"; the tree Max watches
/// ([`State::waiting_on`] is the throbber); his queue ([`State::queue`]);
/// restart (the brain's whole to-do list is [`State::todo`], so a fresh
/// brain folds the journal and carries on with nothing else to recover);
/// structured concurrency (stops cascade, scopes close).
///
/// **Existing harnesses.** Each keeps its live state in memory beside its
/// store and reconciles on load: Oh My Pi rebuilds a session's context from
/// its entry tree on resume, Codex reconstructs a thread from its rollout,
/// OpenCode's session status lives in memory only, so a restart forgets
/// which subagents were running.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    last: Option<Seq>,
    build: Option<Launched>,
    tasks: BTreeMap<TaskId, Task>,
    calls: BTreeMap<CallId, CallRecord>,
    model_calls: BTreeMap<ModelCallId, ModelCallRecord>,
    deliveries: BTreeMap<DeliveryId, Delivered>,
    environments: BTreeMap<EnvironmentId, Environment>,
    limbs: BTreeMap<LimbId, Limb>,
    reach: BTreeMap<LimbId, Reach>,
    resources: BTreeMap<ResourceKey, Current>,
    ranking: Ranking,
    focus: BTreeMap<FaceId, TaskId>,
    limited_until: Option<Timestamp>,
    /// The live tasks whose fold reads each call's settlement: the caller,
    /// every task a shared attachment was given to, the chat Max acted in.
    readers: BTreeMap<CallId, Vec<TaskId>>,
}

/// A call and where it is in its life.
#[derive(Clone, Debug, PartialEq)]
pub struct CallRecord {
    pub call: Call,
    pub made: Timestamp,
    /// The first dispatch.
    pub dispatched: Option<Timestamp>,
    pub status: Status,
}

/// A request and what came of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelCallRecord {
    pub call: ModelCall,
    pub sent: Timestamp,
    pub first_byte: Option<Timestamp>,
    pub ended: Option<Ended>,
}

/// How a request ended, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub at: Timestamp,
    pub ending: Ending,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

/// A delivery, when, and by whom.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivered {
    pub delivery: Delivery,
    pub at: Timestamp,
    pub by: Author,
}

/// A resource's current version, and who moved it there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Current {
    pub version: Version,
    pub by: Option<Writer>,
    pub at: Timestamp,
}

/// Something a task's context embodies that has moved since, and that it
/// has not been told of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stale {
    pub resource: ResourceKey,
    pub has: Version,
    pub now: Version,
    pub by: Option<Writer>,
}

/// What a task is waiting on: the throbber, and the answer to "why is
/// nothing happening?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// On the brain to send its next request.
    Brain,
    /// Held by Max, on itself or an ancestor.
    Held,
    /// On the subscription's limit.
    Limited {
        until: Timestamp,
    },
    /// On the model: sent at `since`, streaming once its first byte came.
    Model {
        call: ModelCallId,
        since: Timestamp,
        streaming: bool,
    },
    /// On its round's calls: tools, its children (a spawn), attachments.
    Calls {
        calls: Vec<CallId>,
    },
    /// On Max, since `since`: its turn ended, he interrupted, or it asked.
    Max {
        since: Timestamp,
        ask: Option<CallId>,
    },
    /// On an environment's event, or the background work it started.
    Listening {
        background: Vec<CallId>,
    },
    Done,
}

/// One thing the brain should do now. The whole of its loop: act on each,
/// append what happened, fold, repeat. Each item is idempotent until the
/// fact that does it is appended, so a restart needs nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    /// Build and send the task's next request, rendering every pending
    /// input that the policy includes (every result, always).
    Send(TaskId),
    /// Send the call to its limb. After a restart, a dispatched call is
    /// asked about instead ([`crate::environment::ToLimb::Recover`]).
    Dispatch(CallId),
    /// Stop the call: ask its limb, or, if it was never dispatched, settle it
    /// as interrupted.
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
        let expected = Seq(self.last.map_or(1, |s| s.0 + 1));
        if record.seq != expected {
            return Err(Rejected::OutOfOrder { expected });
        }
        match &record.fact {
            Fact::Launched(launched) => self.build = Some(launched.clone()),
            Fact::Task(id, TaskEvent::Created(created)) => self.create(*id, created, record.at)?,
            Fact::Task(id, event) => self.task_event(*id, event, record)?,
            Fact::ModelCall(id, event) => self.fold_model_call(*id, event, record)?,
            Fact::Call(id, event) => self.fold_call(*id, event, record)?,
            Fact::Delivery(delivery) => self.deliver(delivery, record)?,
            Fact::Change(change) => {
                self.resources.insert(
                    change.resource.clone(),
                    Current {
                        version: change.version,
                        by: change.by,
                        at: record.at,
                    },
                );
            }
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
                if !self.limbs.contains_key(limb) {
                    return Err(Rejected::Unknown);
                }
                self.reach.insert(*limb, reach.clone());
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

    fn create(&mut self, id: TaskId, created: &Created, at: Timestamp) -> Result<(), Rejected> {
        if self.tasks.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        if let Scope::Spawn(call) = created.scope {
            let record = self.calls.get(&call).ok_or(Rejected::Unknown)?;
            if record.status.is_settled() {
                return Err(Rejected::Finished);
            }
            if !self.is_meta(record.call.limb) || created.mode == Mode::Standing {
                return Err(Rejected::WrongScope);
            }
        }
        if !self.environments.contains_key(&created.binding.environment) {
            return Err(Rejected::Unknown);
        }
        let mut task = Task::new(id, created);
        match created.origin {
            Origin::Fresh => {}
            Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } => {
                let source = self.tasks.get(&from).ok_or(Rejected::Unknown)?;
                if source.binding.environment != created.binding.environment {
                    return Err(Rejected::WrongEnvironment);
                }
                let resumable = matches!(created.origin, Origin::Resume { .. }) && source.is_done();
                if !resumable && source.head != created.head {
                    return Err(Rejected::Detached);
                }
                task.knows = source.knows.clone();
            }
        }
        observe(&mut task, &created.observes);
        self.attach(&mut task, &created.awaits, at)?;
        for call in &created.awaits {
            self.readers.entry(*call).or_default().push(id);
        }
        self.tasks.insert(id, task);
        Ok(())
    }

    /// Give a new context its attachments: they must be the harness's calls,
    /// and those already settled are its results at once.
    fn attach(&self, task: &mut Task, awaits: &[CallId], at: Timestamp) -> Result<(), Rejected> {
        for call in awaits {
            let record = self.calls.get(call).ok_or(Rejected::Unknown)?;
            if !matches!(record.call.caller, Caller::Meta { .. }) {
                return Err(Rejected::NotForTask);
            }
            if record.status.is_settled() {
                task.settled(*call, at)?;
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
        let mut task = self.tasks.get(&id).ok_or(Rejected::Unknown)?.clone();
        task.apply(record)?;
        match event {
            TaskEvent::Moved(Move::Handover {
                call,
                observes,
                awaits,
                ..
            }) => {
                task.knows.clear();
                observe(&mut task, observes);
                self.attach(&mut task, awaits, record.at)?;
                if let Some(readers) = self.readers.get_mut(call) {
                    readers.retain(|t| *t != id);
                }
                for call in awaits {
                    self.readers.entry(*call).or_default().push(id);
                }
            }
            TaskEvent::Moved(Move::Reseed { observes, .. }) => {
                task.knows.clear();
                observe(&mut task, observes);
            }
            _ => {}
        }
        self.tasks.insert(id, task);
        Ok(())
    }

    fn fold_model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        record: &Record,
    ) -> Result<(), Rejected> {
        match event {
            ModelCallEvent::Sent(call) => {
                if self.model_calls.contains_key(&id) {
                    return Err(Rejected::Exists);
                }
                let mut task = self.tasks.get(&call.task).ok_or(Rejected::Unknown)?.clone();
                if call.purpose == Purpose::Turn {
                    self.learn(&mut task, &call.renders)?;
                }
                task.apply(record)?;
                self.tasks.insert(call.task, task);
                self.model_calls.insert(
                    id,
                    ModelCallRecord {
                        call: call.clone(),
                        sent: record.at,
                        first_byte: None,
                        ended: None,
                    },
                );
            }
            ModelCallEvent::FirstByte => {
                let sent = self.model_calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if sent.ended.is_some() {
                    return Err(Rejected::Finished);
                }
                sent.first_byte.get_or_insert(record.at);
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
                let owner = sent.call.task;
                let responded = match ending {
                    Ending::Responded { calls, .. } => calls.as_slice(),
                    _ => &[],
                };
                for call in responded {
                    let made = self.calls.get(call).ok_or(Rejected::Unknown)?;
                    let answers = matches!(made.call.caller,
                        Caller::Agent { task, tool_use: Some(_) } if task == owner);
                    if !answers {
                        return Err(Rejected::NotForTask);
                    }
                    if made.status != Status::Made {
                        return Err(Rejected::Busy);
                    }
                }
                if sent.call.purpose == Purpose::Turn {
                    let mut task = self.tasks.get(&owner).ok_or(Rejected::Unknown)?.clone();
                    task.apply(record)?;
                    self.tasks.insert(owner, task);
                }
                for call in responded {
                    self.readers.insert(*call, vec![owner]);
                }
                if let Ending::Failed {
                    retry: Some(until), ..
                } = ending
                {
                    self.limited_until = self.limited_until.max(Some(*until));
                }
                if let Some(sent) = self.model_calls.get_mut(&id) {
                    sent.ended = Some(Ended {
                        at: record.at,
                        ending: ending.clone(),
                        usage: *usage,
                        quota: quota.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Update what a context embodies from what a request newly included.
    fn learn(&self, task: &mut Task, renders: &[Rendered]) -> Result<(), Rejected> {
        for rendered in renders {
            match rendered {
                Rendered::Call(call) => {
                    let record = self.calls.get(call).ok_or(Rejected::Unknown)?;
                    let Status::Settled { outcome, late } = &record.status else {
                        return Err(Rejected::Busy);
                    };
                    match late.as_ref().unwrap_or(outcome) {
                        CallOutcome::Completed(output)
                        | CallOutcome::Failed(output)
                        | CallOutcome::Interrupted {
                            partial: Some(output),
                            ..
                        } => observe(task, &output.observes),
                        CallOutcome::Refused { current } => told(task, current),
                        CallOutcome::Interrupted { partial: None, .. } | CallOutcome::Unknown => {}
                    }
                }
                Rendered::Delivery(delivery) => {
                    if !self.deliveries.contains_key(delivery) {
                        return Err(Rejected::Unknown);
                    }
                }
                Rendered::Note(Note::Stale { told: versions }) => told(task, versions),
                Rendered::Note(_) => {}
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
        let CallEvent::Made(call) = event else {
            let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
            let status = made.status.apply(event)?;
            if matches!(event, CallEvent::Settled(_))
                && self.members(Scope::Spawn(id)).any(|t| !t.is_done())
            {
                return Err(Rejected::Busy);
            }
            let mut touched = Vec::new();
            if matches!(event, CallEvent::Settled(_) | CallEvent::Late(_)) {
                for reader in self.readers.get(&id).into_iter().flatten() {
                    let task = &self.tasks[reader];
                    if task.is_done() {
                        continue;
                    }
                    let mut task = task.clone();
                    task.apply(record)?;
                    touched.push(task);
                }
            }
            for task in touched {
                self.tasks.insert(task.id, task);
            }
            let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
            if matches!(event, CallEvent::Dispatched { .. }) {
                made.dispatched.get_or_insert(record.at);
            }
            made.status = status;
            return Ok(());
        };
        if self.calls.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        if !self.limbs.contains_key(&call.limb) {
            return Err(Rejected::Unknown);
        }
        let mut reader = None;
        match &call.caller {
            Caller::Agent { task, tool_use } => {
                let task = self.tasks.get(task).ok_or(Rejected::Unknown)?;
                if !task.binding.routes(&call.tool, call.limb) {
                    return Err(Rejected::WrongEnvironment);
                }
                match tool_use {
                    Some(_) if !matches!(task.state, TaskState::Thinking(_)) => {
                        return Err(Rejected::Busy);
                    }
                    Some(_) => {}
                    None if task.draining.is_some() => return Err(Rejected::Draining),
                    None => reader = Some(task.clone()),
                }
            }
            Caller::Max { chat: Some(task) } => {
                reader = Some(self.tasks.get(task).ok_or(Rejected::Unknown)?.clone());
            }
            Caller::Max { chat: None } => {}
            Caller::Meta { call: by } => {
                let by = self.calls.get(by).ok_or(Rejected::Unknown)?;
                if by.status.is_settled() {
                    return Err(Rejected::Finished);
                }
            }
        }
        if let Some(mut task) = reader {
            task.apply(record)?;
            self.readers.insert(id, vec![task.id]);
            self.tasks.insert(task.id, task);
        }
        self.calls.insert(
            id,
            CallRecord {
                call: call.clone(),
                made: record.at,
                dispatched: None,
                status: Status::Made,
            },
        );
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
        task.apply(record)?;
        self.tasks.insert(task.id, task);
        self.deliveries.insert(
            delivery.id,
            Delivered {
                delivery: delivery.clone(),
                at: record.at,
                by: record.by.clone(),
            },
        );
        Ok(())
    }

    // Reading.

    /// The last position folded.
    pub fn last(&self) -> Option<Seq> {
        self.last
    }

    /// The build writing the journal now.
    pub fn build(&self) -> Option<&Launched> {
        self.build.as_ref()
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

    pub fn model_call(&self, id: ModelCallId) -> Option<&ModelCallRecord> {
        self.model_calls.get(&id)
    }

    pub fn delivery(&self, id: DeliveryId) -> Option<&Delivered> {
        self.deliveries.get(&id)
    }

    pub fn environment(&self, id: EnvironmentId) -> Option<&Environment> {
        self.environments.get(&id)
    }

    pub fn limb(&self, id: LimbId) -> Option<&Limb> {
        self.limbs.get(&id)
    }

    pub fn reach(&self, id: LimbId) -> Option<&Reach> {
        self.reach.get(&id)
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

    /// Whether the account is limited at `now`.
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
            Caller::Meta { call } => self.call_owner(*call),
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
            .and_then(|c| self.limbs.get(&c.call.limb))
            .is_some_and(|l| l.kind == LimbKind::Max)
    }

    fn is_meta(&self, limb: LimbId) -> bool {
        self.limbs
            .get(&limb)
            .is_some_and(|l| matches!(l.kind, LimbKind::Meta { .. }))
    }

    /// What a task is waiting on now.
    pub fn waiting_on(&self, task: TaskId, now: Timestamp) -> Option<Waiting> {
        let t = self.tasks.get(&task)?;
        Some(match &t.state {
            TaskState::Done(_) => Waiting::Done,
            TaskState::Ready if self.held(task) => Waiting::Held,
            TaskState::Ready => match self.limited(now) {
                Some(until) => Waiting::Limited { until },
                None => Waiting::Brain,
            },
            TaskState::Thinking(call) => {
                let sent = self.model_calls.get(call)?;
                Waiting::Model {
                    call: *call,
                    since: sent.sent,
                    streaming: sent.first_byte.is_some(),
                }
            }
            TaskState::Acting(step) => match step.outstanding.iter().find(|c| self.is_ask(**c)) {
                Some(ask) => Waiting::Max {
                    since: t.owed_since.unwrap_or(self.calls[ask].made),
                    ask: Some(*ask),
                },
                None => Waiting::Calls {
                    calls: step.outstanding.clone(),
                },
            },
            TaskState::AwaitingMax => Waiting::Max {
                since: t.owed_since.unwrap_or(now),
                ask: None,
            },
            TaskState::Listening => Waiting::Listening {
                background: t
                    .watching
                    .iter()
                    .filter(|w| w.wakes == Wakes::Always)
                    .map(|w| w.call)
                    .collect(),
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
                let ask = match &t.state {
                    TaskState::Acting(step) => {
                        step.outstanding.iter().copied().find(|c| self.is_ask(*c))
                    }
                    _ => None,
                };
                let since = t.owed_since.or_else(|| ask.map(|a| self.calls[&a].made))?;
                Some(Queued {
                    task: t.id,
                    since,
                    ask,
                    why: None,
                })
            })
            .collect();
        self.ranking.order(queued)
    }

    /// What a task's context embodies that has moved since, and that it has
    /// not been told of: the next request's notice.
    pub fn stale(&self, task: TaskId) -> Vec<Stale> {
        let Some(task) = self.tasks.get(&task) else {
            return Vec::new();
        };
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
                })
            })
            .collect()
    }

    /// Everything the brain should do now.
    pub fn todo(&self, now: Timestamp) -> Vec<Todo> {
        let mut todo = Vec::new();
        for task in self.tasks.values() {
            match (&task.state, &task.draining) {
                (TaskState::Ready, None) if !self.held(task.id) && self.limited(now).is_none() => {
                    todo.push(Todo::Send(task.id));
                }
                (TaskState::Thinking(call), Some(_)) => todo.push(Todo::Abandon(*call)),
                _ => {}
            }
        }
        for (id, record) in &self.calls {
            if record.status.is_settled() {
                continue;
            }
            let mut members = self.members(Scope::Spawn(*id)).peekable();
            if members.peek().is_some() {
                let stopping = self
                    .call_owner(*id)
                    .and_then(|o| self.tasks.get(&o))
                    .is_some_and(|o| matches!(o.draining, Some(Drain::Stop(_) | Drain::Fail(_))));
                let mut all_done = true;
                for member in members {
                    if member.is_done() {
                        continue;
                    }
                    all_done = false;
                    if stopping && !matches!(member.draining, Some(Drain::Stop(_) | Drain::Fail(_)))
                    {
                        todo.push(Todo::Stop(member.id));
                    }
                }
                if all_done {
                    todo.push(Todo::Settle(*id));
                }
                continue;
            }
            if self.interrupts(*id, &record.call) {
                todo.push(Todo::Interrupt(*id));
            } else if record.status == Status::Made && self.dispatchable(&record.call, *id) {
                todo.push(Todo::Dispatch(*id));
            }
        }
        todo
    }

    /// A tool use is acted on only once its response is complete, and the
    /// task is acting on it.
    fn dispatchable(&self, call: &Call, id: CallId) -> bool {
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

    /// Whether every live task reading this call is draining in a way that
    /// stops it. An interrupt stops the round, not background work; a stop
    /// stops both. A shared attachment runs on while any sibling still wants
    /// it. Max's own calls are never stopped for an agent.
    fn interrupts(&self, id: CallId, call: &Call) -> bool {
        if matches!(call.caller, Caller::Max { .. }) {
            return false;
        }
        let mut readers = self
            .readers
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|t| self.tasks.get(t))
            .filter(|t| !t.is_done())
            .peekable();
        readers.peek().is_some()
            && readers.all(|t| match &t.draining {
                None => false,
                Some(Drain::ToMax) => {
                    matches!(&t.state, TaskState::Acting(step) if step.outstanding.contains(&id))
                }
                Some(Drain::Stop(_) | Drain::Fail(_)) => true,
            })
    }
}

/// The context now embodies these versions.
fn observe(task: &mut Task, observations: &[Observation]) {
    for o in observations {
        task.knows.insert(
            o.resource.clone(),
            Known {
                has: o.version,
                told: None,
            },
        );
    }
}

/// The context has been told these resources moved, without seeing them.
fn told(task: &mut Task, observations: &[Observation]) {
    for o in observations {
        if let Some(known) = task.knows.get_mut(&o.resource) {
            known.told = Some(o.version);
        }
    }
}
