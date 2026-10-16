//! The loop: fold, act on each item of the to-do list, append what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Interruption, Outcome, Output, Status};
use harness_core::context::{Entry, EntryId, Message, Role, Wire};
use harness_core::environment::{
    Binding, Description, FromLimb, LimbKind, Reach, Startable, ToLimb,
};
use harness_core::id::{
    CallId, Digest, EnvironmentId, InstanceId, LimbId, ModelCallId, Seq, TaskId, Timestamp,
    ToolUseId,
};
use harness_core::journal::{Author, Fact, Launched, Record};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Purpose};
use harness_core::resource::Observation;
use harness_core::state::{State, Todo};
use harness_core::task::{
    Created, Drain, Giver, Mandate, Mode, Origin, Outcome as TaskOutcome, Scope, Stopper, Task,
    TaskEvent, TaskState,
};
use harness_limb_workspace::{Config, Workspace};
use harness_provider::{Client, Finished, Progress};
use harness_store::Store;
use serde_json::Value;

use crate::journal::Journal;
use crate::{meta, policy, render};

/// How long the loop sleeps when nothing arrives: the resolution of backoff
/// and fan-out timing.
const TICK: Duration = Duration::from_millis(250);

/// What reaches the loop from the threads it started.
enum Event {
    Limb(FromLimb),
    FirstByte(ModelCallId),
    Finished(ModelCallId, Result<Finished, harness_provider::Error>),
}

/// What only the first launch of a journal is given.
pub struct First {
    pub workspace: PathBuf,
    pub name: String,
    pub mandate: String,
}

/// One brain: the only writer of its journal.
pub struct Brain {
    store: Store,
    state: State,
    journal: Journal,
    client: Arc<Client>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    /// This launch, as the instance meta calls are dispatched to.
    instance: InstanceId,
    meta: LimbId,
    workspace: Workspace,
    workspace_limb: LimbId,
    /// What each limb tells a new root, as it said at this launch.
    descriptions: BTreeMap<LimbId, Description>,
    /// The context of each request without a first byte yet: fan-out gating.
    cold: HashMap<ModelCallId, HashSet<EntryId>>,
    abort: HashMap<ModelCallId, Arc<AtomicBool>>,
    interrupting: HashSet<CallId>,
    root: TaskId,
    started: Instant,
    reported: bool,
    /// A response was billed outside the plan: send nothing more.
    off_plan: bool,
}

impl Brain {
    /// Fold the journal, connect the limbs, define the world on a first
    /// launch, and recover what the last writer left unfinished.
    pub fn launch(
        store: Store,
        journal: Journal,
        client: Client,
        state_dir: PathBuf,
        first: Option<First>,
    ) -> Result<Brain, String> {
        let state = store
            .fold()
            .map_err(|e| format!("the journal folds: {e}"))?;
        let root = state.tasks().find(|t| t.scope == Scope::Max).map(|t| t.id);
        let (meta, workspace_limb, workspace_dir, starts) = match root {
            Some(root) => world(&state, root)?,
            None => {
                let first = first
                    .as_ref()
                    .ok_or("a new journal needs a workspace and a mandate")?;
                let dir = first
                    .workspace
                    .canonicalize()
                    .map_err(|e| format!("the workspace {:?}: {e}", first.workspace))?;
                (LimbId(new_id()), LimbId(new_id()), dir, Vec::new())
            }
        };

        let (events, inbox) = mpsc::channel();
        let (limb_out, limb_in) = mpsc::channel();
        let forward = events.clone();
        thread::spawn(move || {
            for message in limb_in {
                if forward.send(Event::Limb(message)).is_err() {
                    return;
                }
            }
        });
        let workspace = Workspace::open(
            Config {
                limb: workspace_limb,
                root: workspace_dir.clone(),
                state: state_dir,
            },
            limb_out,
        )
        .map_err(|e| format!("the workspace opens: {e}"))?;
        workspace.handle(ToLimb::Describe);
        let described = loop {
            match inbox.recv_timeout(Duration::from_secs(10)) {
                Ok(Event::Limb(FromLimb::Described(d))) => break d,
                Ok(_) => {}
                Err(_) => return Err("the workspace did not describe itself".into()),
            }
        };

        let mut brain = Brain {
            store,
            state,
            journal,
            client: Arc::new(client),
            events,
            inbox,
            instance: InstanceId(new_id()),
            meta,
            workspace_limb,
            workspace,
            descriptions: BTreeMap::new(),
            cold: HashMap::new(),
            abort: HashMap::new(),
            interrupting: HashSet::new(),
            root: root.unwrap_or(TaskId(0)),
            started: Instant::now(),
            reported: false,
            off_plan: false,
        };
        brain.descriptions.insert(workspace_limb, described);

        let launched = Fact::Launched(Launched {
            build: concat!("harness-brain ", env!("CARGO_PKG_VERSION")).into(),
            by: None,
        });
        let reach = Fact::Reach {
            limb: workspace_limb,
            reach: Reach::Connected {
                instance: brain.workspace.instance(),
                since: now(),
            },
        };
        match first.filter(|_| root.is_none()) {
            Some(first) => brain.define(first, workspace_dir, launched, reach),
            None => {
                brain
                    .descriptions
                    .insert(meta, meta_description(meta, &starts));
                brain.append(vec![(Author::Brain, launched), (Author::Brain, reach)]);
                brain.recover();
            }
        }
        Ok(brain)
    }

    /// The first launch: the limbs, the environment, and the root task.
    fn define(&mut self, first: First, dir: PathBuf, launched: Fact, reach: Fact) {
        let environment = EnvironmentId(new_id());
        let limbs = [self.meta, self.workspace_limb];
        let starts = vec![Startable {
            agent: "engineer".into(),
            environment,
        }];
        self.descriptions
            .insert(self.meta, meta_description(self.meta, &starts));
        let mut facts = vec![
            launched,
            Fact::Limb(meta::definition(self.meta, starts)),
            Fact::Limb(harness_limb_workspace::definition(
                self.workspace_limb,
                "workspace",
                &dir,
            )),
            Fact::Environment(harness_core::environment::Environment {
                id: environment,
                name: "dev".into(),
                limbs: limbs.to_vec(),
            }),
            reach,
        ];
        let kind = policy::kind("orchestrator").expect("the orchestrator kind");
        let (head, binding, observes) = self.new_root(kind, environment, &limbs);
        self.root = TaskId(new_id());
        facts.push(Fact::Task(
            self.root,
            TaskEvent::Created(Box::new(Created {
                name: first.name,
                agent: kind.name.into(),
                scope: Scope::Max,
                origin: Origin::Fresh,
                mode: Mode::Autonomous,
                mandate: Mandate {
                    text: first.mandate,
                    from: Giver::Max,
                },
                head,
                binding,
                observes: observes.clone(),
                awaits: Vec::new(),
            })),
        ));
        self.append(facts.into_iter().map(|f| (Author::Brain, f)).collect());
        self.watch(&observes);
    }

    /// What the last writer left in flight. A request with no end was lost
    /// (nothing it produced was stored) and is sent again; a dispatched call
    /// is asked about; the limb is asked to watch everything a live context
    /// embodies again, which also reports what changed while the brain was
    /// down.
    fn recover(&mut self) {
        let lost: Vec<_> = self
            .state
            .model_calls()
            .filter(|(_, m)| m.ended.is_none())
            .map(|(id, _)| {
                let ended = ModelCallEvent::Ended {
                    ending: Ending::Lost,
                    usage: None,
                    quota: None,
                };
                (Author::Brain, Fact::ModelCall(id, ended))
            })
            .collect();
        if !lost.is_empty() {
            self.append(lost);
        }
        let dispatched: Vec<_> = self
            .state
            .calls()
            .filter(|(_, c)| c.call.limb == self.workspace_limb)
            .filter(|(_, c)| matches!(c.status, Status::Dispatched { .. }))
            .map(|(id, _)| id)
            .collect();
        if !dispatched.is_empty() {
            self.workspace.handle(ToLimb::Recover { calls: dispatched });
        }
        let known: Vec<_> = self
            .state
            .tasks()
            .filter(|t| !t.is_done())
            .flat_map(|t| t.knows.iter())
            .map(|(resource, known)| Observation {
                resource: resource.clone(),
                version: known.has,
            })
            .collect();
        self.watch(&known);
    }

    /// Run until killed. The root's result is printed when it is done; the
    /// face keeps serving.
    pub fn run(&mut self) {
        loop {
            self.work();
            self.report();
            match self.inbox.recv_timeout(TICK) {
                Ok(event) => self.handle(event),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => unreachable!("the brain holds a sender"),
            }
            while let Ok(event) = self.inbox.try_recv() {
                self.handle(event);
            }
        }
    }

    fn work(&mut self) {
        let now = now();
        for todo in self.state.todo(now) {
            // An earlier item may have moved this one on.
            if self.state.todo(now).contains(&todo) {
                self.act(todo, now);
            }
        }
    }

    fn act(&mut self, todo: Todo, now: Timestamp) {
        match todo {
            Todo::Send(task) => self.send(task, now),
            Todo::Dispatch(call) => self.dispatch(call),
            Todo::Interrupt(call) => self.interrupt(call),
            Todo::Abandon(request) => {
                if let Some(abort) = self.abort.get(&request) {
                    abort.store(true, Ordering::Relaxed);
                }
            }
            Todo::Stop(task) => self.append(vec![(
                Author::Brain,
                Fact::Task(task, TaskEvent::Drain(Drain::Stop(Stopper::Owner))),
            )]),
            Todo::Settle(call) => self.settle_scope(call),
        }
    }

    // Requests.

    fn send(&mut self, id: TaskId, now: Timestamp) {
        if self.off_plan {
            return;
        }
        let task = self.state.task(id).expect("a task to send for").clone();
        let turns: Vec<_> = self
            .state
            .model_calls()
            .filter(|(_, m)| m.call.task == id && m.call.purpose == Purpose::Turn)
            .map(|(_, m)| m)
            .collect();
        if task.failures >= policy::GIVE_UP {
            let reason = format!("the provider failed {} times in a row", task.failures);
            return self.append(vec![(
                Author::Brain,
                Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))),
            )]);
        }
        if task.failures > 0 {
            let last = turns.iter().filter_map(|m| m.ended.as_ref()).map(|e| e.at);
            if last
                .max()
                .is_some_and(|at| now.0 < at.0 + policy::backoff_ms(task.failures))
            {
                return;
            }
        }
        if policy::hold(task.head, self.cold.values()) {
            return;
        }

        let head_entry = self.entry(task.head);
        let first = turns.is_empty();
        let forked = match task.origin {
            Origin::Fork { .. } if first => self.response_calls(task.head),
            _ => Vec::new(),
        };
        let turn = render::turn(&self.state, &task, first, &head_entry, &forked);
        let head = if turn.blocks.is_empty() {
            task.head
        } else {
            self.put(Entry::Message(Message {
                parent: task.head,
                role: Role::User,
                content: Wire(format!("[{}]", turn.blocks.join(","))),
            }))
        };
        let chain = self.chain(head);
        let root = chain.last().expect("a chain ends at a root").0;
        let call = ModelCall {
            task: id,
            purpose: Purpose::Turn,
            head,
            renders: turn.renders,
            params: policy::params(),
            breakpoints: policy::breakpoints(root, task.head, head),
        };
        let request = ModelCallId(new_id());
        self.append(vec![(
            Author::Brain,
            Fact::ModelCall(request, ModelCallEvent::Sent(call.clone())),
        )]);
        self.cold
            .insert(request, chain.iter().map(|(id, _)| *id).collect());
        let abort = Arc::new(AtomicBool::new(false));
        self.abort.insert(request, abort.clone());

        let entries: HashMap<EntryId, Entry> = chain.into_iter().collect();
        let client = self.client.clone();
        let events = self.events.clone();
        thread::spawn(move || {
            let finished = client.send(
                request,
                &call,
                |id| entries.get(&id).cloned(),
                |progress| {
                    if let Progress::Fact(ModelCallEvent::FirstByte) = progress {
                        let _ = events.send(Event::FirstByte(request));
                    }
                    if abort.load(Ordering::Relaxed) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            );
            let _ = events.send(Event::Finished(request, finished));
        });
    }

    /// The calls a stored response made, in order: what a fork of it must
    /// answer.
    fn response_calls(&self, response: EntryId) -> Vec<CallId> {
        self.state
            .model_calls()
            .find_map(|(_, m)| match &m.ended.as_ref()?.ending {
                Ending::Responded { entry, calls, .. } if *entry == response => Some(calls.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn first_byte(&mut self, request: ModelCallId) {
        self.cold.remove(&request);
        self.append(vec![(
            Author::Brain,
            Fact::ModelCall(request, ModelCallEvent::FirstByte),
        )]);
    }

    fn finished(
        &mut self,
        request: ModelCallId,
        finished: Result<Finished, harness_provider::Error>,
    ) {
        self.cold.remove(&request);
        self.abort.remove(&request);
        let task = self
            .state
            .model_call(request)
            .expect("a request this brain sent")
            .call
            .task;
        let mut facts = Vec::new();
        let (ending, usage, quota) = match finished {
            Err(e) => (
                Ending::Failed {
                    status: None,
                    message: e.to_string(),
                    retry: None,
                },
                None,
                None,
            ),
            Ok(Finished {
                result: Err(ending),
                usage,
                quota,
            }) => (ending, usage, quota),
            Ok(Finished {
                result: Ok(reply),
                usage,
                quota,
            }) => {
                self.put(Entry::Message(reply.message.clone()));
                let mut calls = Vec::new();
                for call in self.tool_uses(task, &reply.message.content) {
                    let id = CallId(new_id());
                    calls.push(id);
                    facts.push((Author::Brain, Fact::Call(id, CallEvent::Made(call))));
                }
                (reply.ending(calls), usage, quota)
            }
        };
        if let Some(claim) = quota.as_ref().and_then(|q| q.claim.as_deref())
            && claim != "five_hour"
            && !claim.starts_with("seven_day")
        {
            eprintln!("billed as {claim:?}, outside the plan: sending nothing more");
            self.off_plan = true;
        }
        let ended = ModelCallEvent::Ended {
            ending,
            usage,
            quota,
        };
        facts.push((Author::Brain, Fact::ModelCall(request, ended)));
        self.append(facts);
    }

    /// A call for each tool use of a response, routed by the task's binding.
    fn tool_uses(&self, task: TaskId, content: &Wire) -> Vec<Call> {
        let task = self.state.task(task).expect("the responding task");
        let blocks: Vec<Value> = serde_json::from_str(&content.0).unwrap_or_default();
        blocks
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| {
                let tool = b["name"].as_str().unwrap_or_default().to_owned();
                // The provider only lets a model name a declared tool, and
                // every declared tool has a route.
                let limb = task
                    .binding
                    .routes
                    .iter()
                    .find(|r| r.tool == tool)
                    .map(|r| r.limb)
                    .expect("a declared tool");
                Call {
                    caller: Caller::Agent {
                        task: task.id,
                        tool_use: Some(ToolUseId(b["id"].as_str().unwrap_or_default().into())),
                    },
                    expects: task
                        .knows
                        .iter()
                        .filter(|(r, _)| r.limb == limb)
                        .map(|(resource, known)| Observation {
                            resource: resource.clone(),
                            version: known.has,
                        })
                        .collect(),
                    limb,
                    tool,
                    input: Wire(b["input"].to_string()),
                }
            })
            .collect()
    }

    // Calls.

    fn dispatch(&mut self, id: CallId) {
        let call = self
            .state
            .call(id)
            .expect("a call to dispatch")
            .call
            .clone();
        if call.limb == self.meta {
            return self.spawn(id, &call);
        }
        self.append(vec![(
            Author::Brain,
            Fact::Call(
                id,
                CallEvent::Dispatched {
                    instance: self.workspace.instance(),
                },
            ),
        )]);
        self.workspace.handle(ToLimb::Call {
            call: id,
            tool: call.tool,
            input: call.input,
            expects: call.expects,
        });
    }

    /// Open a spawn's scope: its members are created in the same batch as
    /// its dispatch, or it fails at once with nothing created.
    fn spawn(&mut self, id: CallId, call: &Call) {
        let Caller::Agent { task: parent, .. } = call.caller else {
            unreachable!("only agents call spawn")
        };
        let parent = self.state.task(parent).expect("the spawning task").clone();
        let mut facts = vec![(
            Author::Brain,
            Fact::Call(
                id,
                CallEvent::Dispatched {
                    instance: self.instance,
                },
            ),
        )];
        let mut observed = Vec::new();
        match self.members(id, &parent, &call.input) {
            Ok(members) => {
                for (task, created) in members {
                    observed.extend(created.observes.clone());
                    facts.push((
                        Author::Brain,
                        Fact::Task(task, TaskEvent::Created(Box::new(created))),
                    ));
                }
            }
            Err(message) => {
                let output = Output {
                    model: Wire(format!("[{}]", render::text(&message))),
                    full: None,
                    observes: Vec::new(),
                };
                facts.push((
                    Author::Brain,
                    Fact::Call(id, CallEvent::Settled(Outcome::Failed(output))),
                ));
            }
        }
        self.append(facts);
        self.watch(&observed);
    }

    fn members(
        &mut self,
        call: CallId,
        parent: &Task,
        input: &Wire,
    ) -> Result<Vec<(TaskId, Created)>, String> {
        let spawn = meta::parse(input)?;
        let starts = match &self.state.limb(self.meta).expect("the meta limb").kind {
            LimbKind::Meta { starts } => starts.clone(),
            _ => unreachable!("the meta limb is a meta limb"),
        };
        let mut members = Vec::new();
        for member in spawn.members {
            let mandate = Mandate {
                text: member.mandate,
                from: Giver::Task(parent.id),
            };
            let name = format!("{}-{}", parent.name, member.name);
            let created = match member.fresh {
                None => Created {
                    name,
                    agent: parent.agent.clone(),
                    scope: Scope::Spawn(call),
                    origin: Origin::Fork { from: parent.id },
                    mode: Mode::Autonomous,
                    mandate,
                    head: parent.head,
                    binding: parent.binding.clone(),
                    observes: Vec::new(),
                    awaits: Vec::new(),
                },
                Some(agent) => {
                    let startable = starts
                        .iter()
                        .find(|s| s.agent == agent)
                        .ok_or(format!("`{agent}` is not a kind of agent you can start"))?;
                    let kind = policy::kind(&agent).ok_or(format!("no agent kind `{agent}`"))?;
                    let limbs = self
                        .state
                        .environment(startable.environment)
                        .ok_or(format!("the environment of `{agent}` is not defined"))?
                        .limbs
                        .clone();
                    let (root, binding, observes) =
                        self.new_root(kind, startable.environment, &limbs);
                    let head = match &spawn.brief {
                        Some(brief) => self.put(Entry::Message(Message {
                            parent: root,
                            role: Role::User,
                            content: render::brief(brief),
                        })),
                        None => root,
                    };
                    Created {
                        name,
                        agent,
                        scope: Scope::Spawn(call),
                        origin: Origin::Fresh,
                        mode: Mode::Autonomous,
                        mandate,
                        head,
                        binding,
                        observes,
                        awaits: Vec::new(),
                    }
                }
            };
            members.push((TaskId(new_id()), created));
        }
        Ok(members)
    }

    /// Every member of a spawn's scope is done: settle it with their results,
    /// in the order the spawn named them.
    fn settle_scope(&mut self, id: CallId) {
        let input = &self.state.call(id).expect("the spawn").call.input;
        let order = meta::names(input);
        let mut members: Vec<&Task> = self.state.members(Scope::Spawn(id)).collect();
        members.sort_by_key(|t| order.iter().position(|n| t.name.ends_with(n.as_str())));
        let results: Vec<(String, String)> = members
            .iter()
            .map(|t| {
                let said = match &t.state {
                    TaskState::Done(TaskOutcome::Result { entry }) => match self.entry(*entry) {
                        Entry::Message(m) => render::visible(&m.content),
                        Entry::Root(_) => String::new(),
                    },
                    TaskState::Done(TaskOutcome::Failed { reason }) => format!("Failed: {reason}"),
                    TaskState::Done(TaskOutcome::Cancelled { by }) => {
                        format!("Cancelled by {by:?}.")
                    }
                    _ => unreachable!("a scope settles when every member is done"),
                };
                (t.name.clone(), said)
            })
            .collect();
        let output = Output {
            model: meta::result(&results),
            full: None,
            observes: Vec::new(),
        };
        self.append(vec![(
            Author::Brain,
            Fact::Call(id, CallEvent::Settled(Outcome::Completed(output))),
        )]);
    }

    fn interrupt(&mut self, id: CallId) {
        let record = self.state.call(id).expect("a call to interrupt");
        match record.status {
            Status::Made => self.append(vec![(
                Author::Brain,
                Fact::Call(
                    id,
                    CallEvent::Settled(Outcome::Interrupted {
                        by: Interruption::Stop,
                        partial: None,
                    }),
                ),
            )]),
            Status::Dispatched { .. }
                if record.call.limb == self.workspace_limb && self.interrupting.insert(id) =>
            {
                self.workspace.handle(ToLimb::Interrupt { call: id });
            }
            _ => {}
        }
    }

    // What arrives.

    fn handle(&mut self, event: Event) {
        match event {
            Event::FirstByte(request) => self.first_byte(request),
            Event::Finished(request, finished) => self.finished(request, finished),
            Event::Limb(FromLimb::Settled { call, outcome }) => self.limb_settled(call, outcome),
            Event::Limb(FromLimb::Unknown { call }) => self.limb_unknown(call),
            Event::Limb(FromLimb::Changed(change)) => {
                let current = self.state.resource(&change.resource).map(|c| c.version);
                if current != Some(change.version) {
                    self.append(vec![(
                        Author::Limb(self.workspace_limb),
                        Fact::Change(change),
                    )]);
                }
            }
            Event::Limb(_) => {}
        }
    }

    fn limb_settled(&mut self, id: CallId, outcome: Outcome) {
        let Some(record) = self.state.call(id) else {
            return;
        };
        self.interrupting.remove(&id);
        let event = match &record.status {
            Status::Dispatched { .. } => CallEvent::Settled(outcome.clone()),
            Status::Settled {
                outcome: Outcome::Unknown,
                late: None,
            } => CallEvent::Late(outcome.clone()),
            _ => return,
        };
        let output = match &outcome {
            Outcome::Completed(o) | Outcome::Failed(o) => Some(o),
            Outcome::Interrupted { partial, .. } => partial.as_ref(),
            _ => None,
        };
        if let Some(full) = output.and_then(|o| o.full) {
            // The limb keeps the full output on its own disk; the journal
            // names it, so the store must hold it too.
            let bytes = std::fs::read(self.workspace.full_output(full))
                .expect("the limb's full output is on disk");
            self.store.put_content(&bytes).expect("the store keeps it");
        }
        self.append(vec![(
            Author::Limb(self.workspace_limb),
            Fact::Call(id, event),
        )]);
        if let Some(output) = output {
            self.watch(&output.observes.clone());
        }
    }

    /// The limb has no record of a call: send it again if its tool allows a
    /// rerun, or settle it as unknown.
    fn limb_unknown(&mut self, id: CallId) {
        let Some(record) = self.state.call(id) else {
            return;
        };
        if !matches!(record.status, Status::Dispatched { .. }) {
            return;
        }
        let call = record.call.clone();
        let rerun = self.descriptions[&self.workspace_limb]
            .tools
            .iter()
            .any(|t| t.name == call.tool && t.rerun);
        if rerun {
            self.dispatch_again(id, call);
        } else {
            self.append(vec![(
                Author::Limb(self.workspace_limb),
                Fact::Call(id, CallEvent::Settled(Outcome::Unknown)),
            )]);
        }
    }

    fn dispatch_again(&mut self, id: CallId, call: Call) {
        self.append(vec![(
            Author::Brain,
            Fact::Call(
                id,
                CallEvent::Dispatched {
                    instance: self.workspace.instance(),
                },
            ),
        )]);
        self.workspace.handle(ToLimb::Call {
            call: id,
            tool: call.tool,
            input: call.input,
            expects: call.expects,
        });
    }

    /// Ask the workspace to report changes to what contexts embody.
    fn watch(&self, observed: &[Observation]) {
        let resources: Vec<_> = observed
            .iter()
            .filter(|o| o.resource.limb == self.workspace_limb)
            .map(|o| o.resource.clone())
            .collect();
        if !resources.is_empty() {
            self.workspace.handle(ToLimb::Watch { resources });
        }
    }

    // The journal and the store.

    /// Fold a batch into a copy of the state, then append it. A refused
    /// record is a bug in the brain: nothing is appended, and it stops.
    fn append(&mut self, facts: Vec<(Author, Fact)>) {
        let mut state = self.state.clone();
        let mut seq = state.last().map_or(0, |s| s.0);
        let at = now();
        let records: Vec<Record> = facts
            .into_iter()
            .map(|(by, fact)| {
                seq += 1;
                Record {
                    seq: Seq(seq),
                    at,
                    by,
                    fact,
                }
            })
            .collect();
        for record in &records {
            if let Err(rejected) = state.apply(record) {
                let shown: String = format!("{record:?}").chars().take(600).collect();
                panic!("the model refuses the brain's record ({rejected:?}): {shown}");
            }
        }
        self.store
            .append(&records)
            .unwrap_or_else(|e| panic!("the journal appends: {e}"));
        self.state = state;
        for record in &records {
            self.log(record);
        }
        self.journal.grew(Seq(seq));
    }

    fn log(&self, record: &Record) {
        let mut line = harness_face_web::describe(&self.state, record);
        if let Fact::Call(id, CallEvent::Made(call)) = &record.fact
            && call.limb == self.workspace_limb
        {
            let input: String = call.input.0.chars().take(110).collect();
            line = format!("{line} {input}");
            let _ = id;
        }
        println!(
            "{:>7.1}s {:>5}  {line}",
            self.started.elapsed().as_secs_f64(),
            record.seq.0
        );
    }

    /// Print the root's result once it is done.
    fn report(&mut self) {
        if self.reported {
            return;
        }
        let Some(TaskState::Done(outcome)) = self.state.task(self.root).map(|t| &t.state) else {
            return;
        };
        let said = match outcome {
            TaskOutcome::Result { entry } => match self.entry(*entry) {
                Entry::Message(m) => render::visible(&m.content),
                Entry::Root(_) => String::new(),
            },
            other => format!("{other:?}"),
        };
        println!("\n== the root is done; its result:\n\n{said}\n");
        self.reported = true;
    }

    fn put(&self, entry: Entry) -> EntryId {
        self.store
            .put_entry(&entry)
            .unwrap_or_else(|e| panic!("the store keeps an entry: {e}"))
    }

    fn entry(&self, id: EntryId) -> Entry {
        self.store
            .entry(id)
            .unwrap_or_else(|e| panic!("the store reads: {e}"))
            .expect("a named entry is stored")
    }

    /// The entries from `head` back to its root.
    fn chain(&self, head: EntryId) -> Vec<(EntryId, Entry)> {
        let mut chain = Vec::new();
        let mut at = Some(head);
        while let Some(id) = at {
            let entry = self.entry(id);
            at = entry.parent();
            chain.push((id, entry));
        }
        chain
    }

    /// A stored root for a kind in an environment, with its binding and the
    /// versions it embodies.
    fn new_root(
        &self,
        kind: &policy::Kind,
        environment: EnvironmentId,
        limbs: &[LimbId],
    ) -> (EntryId, Binding, Vec<Observation>) {
        let described: Vec<_> = limbs.iter().map(|l| (*l, &self.descriptions[l])).collect();
        let (root, routes, observes) = render::root(kind, &described);
        let head = self.put(Entry::Root(root));
        (
            head,
            Binding {
                environment,
                routes,
            },
            observes,
        )
    }
}

/// The meta limb, the workspace limb, the workspace's directory and the
/// startable kinds, as the journal defines them.
fn world(state: &State, root: TaskId) -> Result<(LimbId, LimbId, PathBuf, Vec<Startable>), String> {
    let task = state.task(root).expect("the root task");
    let env = state
        .environment(task.binding.environment)
        .ok_or("the root's environment")?;
    let (mut meta, mut workspace) = (None, None);
    for limb in env.limbs.iter().filter_map(|l| state.limb(*l)) {
        match &limb.kind {
            LimbKind::Meta { starts } => meta = Some((limb.id, starts.clone())),
            LimbKind::Plugin { kind, config } if kind == "workspace" => {
                let config: Value = serde_json::from_str(&config.0).map_err(|e| e.to_string())?;
                let dir = config["root"].as_str().ok_or("the workspace's root")?;
                workspace = Some((limb.id, PathBuf::from(dir)));
            }
            _ => {}
        }
    }
    let (meta, starts) = meta.ok_or("no meta limb")?;
    let (workspace, dir) = workspace.ok_or("no workspace limb")?;
    Ok((meta, workspace, dir, starts))
}

fn meta_description(limb: LimbId, starts: &[Startable]) -> Description {
    let kinds: Vec<&str> = starts.iter().map(|s| s.agent.as_str()).collect();
    meta::description(limb, &kinds)
}

fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

/// A generated id: unique across brains, from this process, this moment and
/// a counter.
fn new_id() -> u128 {
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seed = format!(
        "{}:{nanos}:{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let digest = Digest::of(seed.as_bytes());
    u128::from_be_bytes(digest.0[..16].try_into().expect("16 bytes"))
}
