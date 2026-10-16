//! The loop: act on each item of the to-do list, commit what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::context::{Entry, EntryId, Message, Role};
use harness_core::environment::{
    Description, Environment, FromLimb, LimbKind, Reach, Startable, ToLimb,
};
use harness_core::id::{
    CallId, EnvironmentId, FaceId, LimbId, ModelCallId, Seq, TaskId, Timestamp, ToolUseId, ValueId,
};
use harness_core::journal::{self, Author, Batch, Fact, Launched, Record};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Purpose};
use harness_core::resource::Observation;
use harness_core::state::{State, Todo};
use harness_core::task::{
    Created, Drain, Giver, Mandate, Mode, Origin, Outcome as TaskOutcome, Scope, Start, Stopper,
    Task, TaskEvent, TaskState,
};
use harness_face_web::{Act, Feed, Running};
use harness_limb_workspace::{Config, Workspace};
use harness_provider::{Client, Finished, Progress};
use harness_store::Store;
use serde_json::Value;

use crate::reader::Reader;
use crate::render::{self, NewRoot};
use crate::{meta, policy};

/// How long the loop sleeps when nothing arrives: the resolution of backoff
/// and fan-out timing.
const TICK: Duration = Duration::from_millis(250);

/// What reaches the loop from the threads it started, and from faces.
pub struct Event(What);

enum What {
    Limb(FromLimb),
    FirstByte(ModelCallId),
    Finished(ModelCallId, Result<Finished, harness_provider::Error>),
    /// A batch of Max's facts from a face, answered with whether the journal
    /// took it.
    Max {
        face: FaceId,
        seen: Seq,
        batch: Batch,
        reply: Sender<Result<(), String>>,
    },
}

/// The loop's inbox, made before the brain so a face can hold its sender.
pub struct Inbox {
    events: Sender<Event>,
    inbox: Receiver<Event>,
}

impl Inbox {
    pub fn new() -> Inbox {
        let (events, inbox) = mpsc::channel();
        Inbox { events, inbox }
    }

    /// How Max's facts from the faces reach the brain.
    pub fn max(&self) -> Max {
        Max(self.events.clone())
    }
}

/// Max's facts, through the brain: each batch is committed as his, as far
/// as his face had seen, or refused back to him.
pub struct Max(Sender<Event>);

impl Act for Max {
    fn act(&self, face: FaceId, seen: Seq, batch: Batch) -> Result<(), String> {
        let (reply, answer) = mpsc::channel();
        let max = What::Max {
            face,
            seen,
            batch,
            reply,
        };
        let stopped = "the brain has stopped".to_string();
        self.0.send(Event(max)).map_err(|_| stopped.clone())?;
        answer.recv().map_err(|_| stopped)?
    }
}

/// What only the first launch of a journal is given.
pub struct First {
    pub workspace: PathBuf,
    pub name: String,
    /// The root's mode: autonomous, or a chat with Max.
    pub mode: Mode,
    pub mandate: String,
}

type Refused = journal::Error<harness_store::Error>;

/// One brain: the only writer of its journal.
pub struct Brain {
    journal: journal::Journal<Store>,
    /// The face's connection, woken after each commit.
    reader: Reader,
    client: Arc<Client>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    /// Each request's streamed events, for the face.
    feed: Feed,
    meta: LimbId,
    workspace: Workspace,
    workspace_limb: LimbId,
    /// What each limb tells a new root, as it last said.
    descriptions: BTreeMap<LimbId, Description>,
    /// The context of each request without a first byte yet: fan-out gating.
    cold: HashMap<ModelCallId, HashSet<EntryId>>,
    abort: HashMap<ModelCallId, Arc<AtomicBool>>,
    root: Option<TaskId>,
    started: Instant,
    reported: bool,
    /// A response was billed outside the plan: send nothing more.
    off_plan: bool,
}

impl Brain {
    /// Fold the journal, connect the limbs, and define the world on a first
    /// launch. What the last writer left in flight is on the to-do list.
    pub fn launch(
        store: Store,
        reader: Reader,
        client: Client,
        state_dir: PathBuf,
        first: Option<First>,
        Inbox { events, inbox }: Inbox,
        feed: Feed,
    ) -> Result<Brain, String> {
        let journal =
            journal::Journal::open(store).map_err(|e| format!("the journal folds: {e}"))?;
        let state = journal.state();
        let root = state.tasks().find(|t| t.scope == Scope::Max).map(|t| t.id);
        let (meta, workspace_limb, workspace_dir, starts) = match root {
            Some(root) => world(state, root)?,
            None => {
                let first = first
                    .as_ref()
                    .ok_or("a new journal needs a workspace and a mandate")?;
                let dir = first
                    .workspace
                    .canonicalize()
                    .map_err(|e| format!("the workspace {:?}: {e}", first.workspace))?;
                (LimbId::generate(), LimbId::generate(), dir, Vec::new())
            }
        };

        let (limb_out, limb_in) = mpsc::channel();
        let forward = events.clone();
        thread::spawn(move || {
            for message in limb_in {
                if forward.send(Event(What::Limb(message))).is_err() {
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
        // Its Hello comes first; this launch records the connection itself.
        let described = loop {
            match inbox.recv_timeout(Duration::from_secs(10)) {
                Ok(Event(What::Limb(FromLimb::Described(d)))) => break d,
                Ok(_) => {}
                Err(_) => return Err("the workspace did not describe itself".into()),
            }
        };

        let mut brain = Brain {
            journal,
            reader,
            client: Arc::new(client),
            events,
            inbox,
            feed,
            meta,
            workspace_limb,
            workspace,
            descriptions: BTreeMap::new(),
            cold: HashMap::new(),
            abort: HashMap::new(),
            root,
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
            },
        };
        match first.filter(|_| root.is_none()) {
            Some(first) => brain.define(first, workspace_dir, launched, reach)?,
            None => {
                brain
                    .descriptions
                    .insert(meta, meta::description(meta, &kinds(&starts)));
                brain.append(Batch {
                    values: Vec::new(),
                    facts: vec![launched, reach],
                });
                // Report what changed while no brain was listening.
                let known: Vec<_> = brain
                    .state()
                    .tasks()
                    .filter(|t| !t.is_done())
                    .flat_map(|t| t.knows.iter())
                    .map(|(resource, known)| Observation {
                        resource: resource.clone(),
                        version: known.has,
                    })
                    .collect();
                brain.watch(&known);
            }
        }
        Ok(brain)
    }

    /// The first launch: the limbs, the environment, and the root task.
    fn define(
        &mut self,
        first: First,
        dir: PathBuf,
        launched: Fact,
        reach: Fact,
    ) -> Result<(), String> {
        let environment = EnvironmentId::generate();
        let limbs = [self.meta, self.workspace_limb];
        let starts = vec![Startable {
            agent: "engineer".into(),
            environment,
        }];
        self.descriptions
            .insert(self.meta, meta::description(self.meta, &kinds(&starts)));
        let kind = policy::kind("orchestrator").expect("the orchestrator kind");
        let NewRoot {
            root,
            binding,
            observes,
            mut values,
        } = render::root(kind, environment, &self.described(&limbs))?;
        let mandate = first.mandate.into_bytes();
        let root_task = TaskId::generate();
        let facts = vec![
            launched,
            Fact::Limb(meta::definition(self.meta, starts)),
            Fact::Limb(harness_limb_workspace::definition(
                self.workspace_limb,
                "workspace",
                &dir,
            )),
            Fact::Environment(Environment {
                id: environment,
                name: "dev".into(),
                limbs: limbs.to_vec(),
            }),
            reach,
            Fact::Task(
                root_task,
                TaskEvent::Created(Box::new(Created {
                    name: first.name,
                    scope: Scope::Max,
                    origin: Origin::Fresh {
                        agent: kind.name.into(),
                        start: Box::new(Start {
                            root,
                            binding,
                            observes: observes.clone(),
                            awaits: Vec::new(),
                        }),
                    },
                    mode: first.mode,
                    mandate: Mandate {
                        text: ValueId::of(&mandate),
                        from: Giver::Max,
                    },
                })),
            ),
        ];
        values.push(mandate);
        self.append(Batch { values, facts });
        self.root = Some(root_task);
        self.watch(&observes);
        Ok(())
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

    fn state(&self) -> &State {
        self.journal.state()
    }

    fn work(&mut self) {
        let now = now();
        for todo in self.state().todo(now) {
            // An earlier item may have moved this one on.
            if self.state().todo(now).contains(&todo) {
                self.act(todo, now);
            }
        }
    }

    fn act(&mut self, todo: Todo, now: Timestamp) {
        match todo {
            Todo::Send(task) => self.send(task, now),
            Todo::Lost(request) => self.fact(Fact::ModelCall(
                request,
                ModelCallEvent::Ended {
                    ending: Ending::Lost,
                    usage: None,
                    quota: None,
                },
            )),
            Todo::Dispatch(call) => self.dispatch(call),
            Todo::Unroutable(call) => {
                let said = format!(
                    "There is no tool `{}` in this context.",
                    self.call(call).tool
                );
                self.fail(call, &said);
            }
            Todo::Interrupt(call) => self.interrupt(call),
            Todo::Abandon(request) => {
                if let Some(abort) = self.abort.get(&request) {
                    abort.store(true, Ordering::Relaxed);
                }
            }
            Todo::Stop(task) => self.fact(Fact::Task(
                task,
                TaskEvent::Drain(Drain::Stop(Stopper::Owner)),
            )),
            Todo::Settle(call) => self.settle_scope(call),
        }
    }

    // Requests.

    fn send(&mut self, id: TaskId, now: Timestamp) {
        if self.off_plan {
            return;
        }
        let task = self.state().task(id).expect("a task to send for").clone();
        if task.failures >= policy::GIVE_UP {
            let reason = format!("the provider failed {} times in a row", task.failures);
            return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
        }
        let turns: Vec<_> = self
            .state()
            .model_calls()
            .filter(|(_, m)| m.call.task == id && m.call.purpose == Purpose::Turn)
            .map(|(_, m)| m)
            .collect();
        if task.failures > 0 {
            let last = turns
                .iter()
                .filter_map(|m| m.ended.as_ref())
                .map(|e| e.at.at);
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

        let first = turns.is_empty();
        let mut values = Vec::new();
        let mut appends = Vec::new();
        if first && let Some(brief) = self.brief(&task) {
            appends.push(user(render::brief(&brief), &mut values));
        }
        let forked = match task.origin {
            Origin::Fork { .. } if first => self.response_calls(task.head),
            _ => Vec::new(),
        };
        let turn = render::turn(self.state(), &|v| self.value(v), &task, first, &forked);
        if !turn.blocks.is_empty() {
            appends.push(user(render::blocks(&turn.blocks), &mut values));
        }
        values.push(policy::PARAMS.to_vec());
        let mut call = ModelCall {
            task: id,
            purpose: Purpose::Turn,
            base: task.head,
            appends,
            renders: turn.renders,
            params: ValueId::of(policy::PARAMS),
            breakpoints: Vec::new(),
        };
        let appended: Vec<EntryId> = call.entries().into_iter().map(|(e, _)| e).collect();
        let root = self.state().context(task.head)[0].0;
        let shared: Vec<EntryId> = [task.head]
            .into_iter()
            .chain(appended.iter().rev().skip(1).copied())
            .collect();
        call.breakpoints = policy::breakpoints(root, &shared, call.head());

        let request = ModelCallId::generate();
        self.append(Batch {
            values,
            facts: vec![Fact::ModelCall(request, ModelCallEvent::Sent(call.clone()))],
        });

        let base: Vec<(EntryId, Entry)> = self
            .state()
            .context(task.head)
            .into_iter()
            .map(|(id, entry)| (id, entry.clone()))
            .collect();
        let mut named = vec![call.params];
        named.extend(call.appends.iter().map(|m| m.content));
        for (_, entry) in &base {
            match entry {
                Entry::Root(root) => named.extend([root.system, root.tools]),
                Entry::Message { message, .. } => named.push(message.content),
            }
        }
        let bytes: HashMap<ValueId, Vec<u8>> =
            named.into_iter().map(|v| (v, self.value(v))).collect();
        self.cold.insert(
            request,
            base.iter().map(|(id, _)| *id).chain(appended).collect(),
        );
        let abort = Arc::new(AtomicBool::new(false));
        self.abort.insert(request, abort.clone());

        let client = self.client.clone();
        let events = self.events.clone();
        let feed = self.feed.clone();
        thread::spawn(move || {
            let base: Vec<(EntryId, &Entry)> = base.iter().map(|(id, e)| (*id, e)).collect();
            let finished = client.send(
                request,
                &call,
                &base,
                |id| bytes.get(&id).cloned(),
                |progress| {
                    match progress {
                        Progress::Fact(ModelCallEvent::FirstByte) => {
                            let _ = events.send(Event(What::FirstByte(request)));
                        }
                        Progress::Live(live) => feed.push(live),
                        Progress::Fact(_) => {}
                    }
                    if abort.load(Ordering::Relaxed) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            );
            let _ = events.send(Event(What::Finished(request, finished)));
        });
    }

    /// The shared brief a fresh member of a spawn's scope starts with.
    fn brief(&self, task: &Task) -> Option<String> {
        let (Origin::Fresh { .. }, Scope::Spawn(spawn)) = (&task.origin, task.scope) else {
            return None;
        };
        meta::parse(&self.value(self.call(spawn).input)).ok()?.brief
    }

    /// The calls a response made, in order: what a fork of it must answer.
    fn response_calls(&self, response: EntryId) -> Vec<CallId> {
        self.state()
            .model_calls()
            .find(|(_, m)| m.response == Some(response))
            .map(|(_, m)| m.calls.clone())
            .unwrap_or_default()
    }

    fn first_byte(&mut self, request: ModelCallId) {
        self.cold.remove(&request);
        self.fact(Fact::ModelCall(request, ModelCallEvent::FirstByte));
    }

    fn finished(
        &mut self,
        request: ModelCallId,
        finished: Result<Finished, harness_provider::Error>,
    ) {
        self.cold.remove(&request);
        self.abort.remove(&request);
        let task = self
            .state()
            .model_call(request)
            .expect("a request this brain sent")
            .call
            .task;
        let mut batch = Batch::default();
        let Finished {
            ending,
            value,
            usage,
            quota,
        } = finished.unwrap_or_else(|e| Finished {
            ending: Ending::Failed {
                status: None,
                message: e.to_string(),
                retry: None,
            },
            value: None,
            usage: None,
            quota: None,
        });
        if let (Ending::Responded { .. }, Some(content)) = (&ending, &value) {
            for (call, input) in self.tool_uses(task, content) {
                batch.values.push(input);
                batch
                    .facts
                    .push(Fact::Call(CallId::generate(), CallEvent::Made(call)));
            }
        }
        batch.values.extend(value);
        if let Some(claim) = quota.as_ref().and_then(|q| q.claim.as_deref())
            && claim != "five_hour"
            && !claim.starts_with("seven_day")
        {
            eprintln!("billed as {claim:?}, outside the plan: sending nothing more");
            self.off_plan = true;
        }
        batch.facts.push(Fact::ModelCall(
            request,
            ModelCallEvent::Ended {
                ending,
                usage,
                quota,
            },
        ));
        self.append(batch);
    }

    /// A call for each tool use of a response, with its input's bytes. Its
    /// limb is the task's binding's; a tool it does not route is answered
    /// as unroutable.
    fn tool_uses(&self, task: TaskId, content: &[u8]) -> Vec<(Call, Vec<u8>)> {
        let task = self.state().task(task).expect("the responding task");
        let blocks: Vec<Value> = serde_json::from_slice(content).unwrap_or_default();
        blocks
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| {
                let tool = b["name"].as_str().unwrap_or_default().to_owned();
                let limb = task.binding.limb(&tool);
                let input = b["input"].to_string().into_bytes();
                let call = Call {
                    caller: Caller::Agent {
                        task: task.id,
                        tool_use: Some(ToolUseId(b["id"].as_str().unwrap_or_default().into())),
                    },
                    expects: task
                        .knows
                        .iter()
                        .filter(|(r, _)| Some(r.limb) == limb)
                        .map(|(resource, known)| Observation {
                            resource: resource.clone(),
                            version: known.has,
                        })
                        .collect(),
                    tool,
                    input: ValueId::of(&input),
                };
                (call, input)
            })
            .collect()
    }

    // Calls.

    fn dispatch(&mut self, id: CallId) {
        let record = self.state().call(id).expect("a call to dispatch");
        let call = record.call.clone();
        if record.limb == Some(self.meta) {
            return self.spawn(id, &call);
        }
        self.fact(Fact::Call(id, CallEvent::Dispatched));
        let input = self.value(call.input);
        self.workspace.handle(ToLimb::Run { id, call, input });
    }

    /// Open a spawn's scope: its members are created in the same batch as
    /// its dispatch, or it fails at once with nothing created.
    fn spawn(&mut self, id: CallId, call: &Call) {
        let Caller::Agent { task: parent, .. } = call.caller else {
            unreachable!("only agents call spawn")
        };
        let parent = self
            .state()
            .task(parent)
            .expect("the spawning task")
            .clone();
        let mut batch = Batch {
            values: Vec::new(),
            facts: vec![Fact::Call(id, CallEvent::Dispatched)],
        };
        let mut observed = Vec::new();
        match self.members(id, &parent, &self.value(call.input), &mut batch.values) {
            Ok(members) => {
                for (task, created) in members {
                    if let Origin::Fresh { start, .. } = &created.origin {
                        observed.extend(start.observes.clone());
                    }
                    batch
                        .facts
                        .push(Fact::Task(task, TaskEvent::Created(Box::new(created))));
                }
            }
            Err(message) => {
                let (model, failed) = failed(&message);
                batch.values.push(model);
                batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
            }
        }
        self.append(batch);
        self.watch(&observed);
    }

    fn members(
        &self,
        call: CallId,
        parent: &Task,
        input: &[u8],
        values: &mut Vec<Vec<u8>>,
    ) -> Result<Vec<(TaskId, Created)>, String> {
        let spawn = meta::parse(input)?;
        let starts = match &self.state().limb(self.meta).expect("the meta limb").kind {
            LimbKind::Meta { starts } => starts.clone(),
            _ => unreachable!("the meta limb is a meta limb"),
        };
        let mut members = Vec::new();
        for member in spawn.members {
            let mandate = member.mandate.into_bytes();
            let mandate_id = ValueId::of(&mandate);
            values.push(mandate);
            let origin = match member.fresh {
                None => Origin::Fork { from: parent.id },
                Some(agent) => {
                    let startable = starts
                        .iter()
                        .find(|s| s.agent == agent)
                        .ok_or(format!("`{agent}` is not a kind of agent you can start"))?;
                    let kind = policy::kind(&agent).ok_or(format!("no agent kind `{agent}`"))?;
                    let limbs = self
                        .state()
                        .environment(startable.environment)
                        .ok_or(format!("the environment of `{agent}` is not defined"))?
                        .limbs
                        .clone();
                    let new = render::root(kind, startable.environment, &self.described(&limbs))?;
                    values.extend(new.values);
                    Origin::Fresh {
                        agent,
                        start: Box::new(Start {
                            root: new.root,
                            binding: new.binding,
                            observes: new.observes,
                            awaits: Vec::new(),
                        }),
                    }
                }
            };
            let created = Created {
                name: format!("{}-{}", parent.name, member.name),
                scope: Scope::Spawn(call),
                origin,
                mode: Mode::Autonomous,
                mandate: Mandate {
                    text: mandate_id,
                    from: Giver::Task(parent.id),
                },
            };
            members.push((TaskId::generate(), created));
        }
        Ok(members)
    }

    /// Every member of a spawn's scope is done: settle it with their results,
    /// in the order the spawn named them.
    fn settle_scope(&mut self, id: CallId) {
        let order = meta::names(&self.value(self.call(id).input));
        let mut members: Vec<&Task> = self.state().members(Scope::Spawn(id)).collect();
        members.sort_by_key(|t| order.iter().position(|n| t.name.ends_with(n.as_str())));
        let results: Vec<(String, String)> = members
            .iter()
            .map(|t| {
                let said = match &t.state {
                    TaskState::Done(TaskOutcome::Result { entry }) => self.visible(*entry),
                    TaskState::Done(TaskOutcome::Failed { reason }) => format!("Failed: {reason}"),
                    TaskState::Done(TaskOutcome::Cancelled { by }) => {
                        format!("Cancelled by {by:?}.")
                    }
                    _ => unreachable!("a scope settles when every member is done"),
                };
                (t.name.clone(), said)
            })
            .collect();
        let model = meta::result(&results);
        let output = Output {
            model: ValueId::of(&model),
            full: None,
            observes: Vec::new(),
        };
        self.append(Batch {
            values: vec![model],
            facts: vec![Fact::Call(
                id,
                CallEvent::Settled(Outcome::Completed(output)),
            )],
        });
    }

    /// Stop a call: ask its limb, or settle it at once if it never went.
    fn interrupt(&mut self, id: CallId) {
        let record = self.state().call(id).expect("a call to interrupt");
        if record.dispatched.is_none() {
            return self.append(Batch {
                values: Vec::new(),
                facts: vec![Fact::Call(
                    id,
                    CallEvent::Settled(Outcome::Interrupted { partial: None }),
                )],
            });
        }
        self.fact(Fact::Call(id, CallEvent::Stopping));
        self.workspace.handle(ToLimb::Stop { call: id });
    }

    /// Settle a call the brain answers itself as failed, saying why.
    fn fail(&mut self, id: CallId, said: &str) {
        let (model, failed) = failed(said);
        self.append(Batch {
            values: vec![model],
            facts: vec![Fact::Call(id, CallEvent::Settled(failed))],
        });
    }

    // What arrives.

    fn handle(&mut self, Event(event): Event) {
        match event {
            What::FirstByte(request) => self.first_byte(request),
            What::Finished(request, finished) => self.finished(request, finished),
            What::Max {
                face,
                seen,
                batch,
                reply,
            } => {
                // Max acts on a picture that may be out of date: a refusal
                // (the task finished meanwhile) is his to see, not a bug.
                let done = self
                    .commit(Author::Max { face, seen }, batch)
                    .map_err(|e| match e {
                        journal::Error::Refused { why, .. } => format!("{why:?}"),
                        e => e.to_string(),
                    });
                let _ = reply.send(done);
            }
            What::Limb(FromLimb::Hello { instance }) => self.fact(Fact::Reach {
                limb: self.workspace_limb,
                reach: Reach::Connected { instance },
            }),
            What::Limb(FromLimb::Described(description)) => {
                self.descriptions.insert(self.workspace_limb, description);
            }
            What::Limb(FromLimb::Facts(batch)) => self.limb_facts(batch),
            What::Limb(FromLimb::Live(live)) => self.feed.push(live),
        }
    }

    /// Commit what the workspace says about its calls and files. A repeat
    /// after a reconnection is refused by the journal and dropped. A report
    /// of the version the journal already has, or a watch's report of one no
    /// live context knows otherwise, is not a change.
    fn limb_facts(&mut self, mut batch: Batch) {
        let state = self.state();
        batch.facts.retain(|fact| match fact {
            Fact::Change(change) => match state.resource(&change.resource) {
                Some(current) => current.version != change.version,
                None if change.by.is_some() => true,
                None => state.tasks().filter(|t| !t.is_done()).any(|t| {
                    t.knows
                        .get(&change.resource)
                        .is_some_and(|k| k.has != change.version)
                }),
            },
            _ => true,
        });
        if batch.facts.is_empty() {
            return;
        }
        let observed: Vec<Observation> = batch
            .facts
            .iter()
            .filter_map(|f| match f {
                Fact::Call(_, CallEvent::Settled(outcome)) => outcome.output(),
                _ => None,
            })
            .flat_map(|o| o.observes.clone())
            .collect();
        match self.commit(Author::Limb(self.workspace_limb), batch) {
            Ok(()) => self.watch(&observed),
            Err(e) => eprintln!("the workspace's facts are refused: {e}"),
        }
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

    // The journal.

    /// Commit one of the brain's own facts that names no value.
    fn fact(&mut self, fact: Fact) {
        self.append(Batch {
            values: Vec::new(),
            facts: vec![fact],
        });
    }

    /// Commit the brain's own batch. A refusal is a bug in the brain:
    /// nothing is appended, and it stops.
    fn append(&mut self, batch: Batch) {
        let shown: String = format!("{:?}", batch.facts).chars().take(600).collect();
        if let Err(e) = self.commit(Author::Brain, batch) {
            panic!("the journal refuses the brain's batch ({e}): {shown}");
        }
    }

    /// Commit a batch. What it starts running gets a place in the live
    /// feed before a face can see it start; what it ends leaves the feed
    /// once the journal holds how it ended.
    fn commit(&mut self, by: Author, batch: Batch) -> Result<(), Refused> {
        let started: Vec<Running> = batch.facts.iter().filter_map(starts).collect();
        for running in &started {
            self.feed.start(*running);
        }
        let records = match self.journal.commit(by, batch, now()) {
            Ok(records) => records,
            Err(e) => {
                for running in started {
                    self.feed.end(running);
                }
                return Err(e);
            }
        };
        for record in &records {
            self.log(record);
            if let Some(running) = ends(&record.fact) {
                self.feed.end(running);
            }
        }
        if let Some(last) = self.state().last() {
            self.reader.grew(last);
        }
        Ok(())
    }

    fn log(&self, record: &Record) {
        let mut line = harness_face_web::describe(self.state(), record);
        if let Fact::Call(id, CallEvent::Made(call)) = &record.fact
            && self.state().call(*id).and_then(|c| c.limb) == Some(self.workspace_limb)
        {
            let input = String::from_utf8_lossy(&self.value(call.input))
                .chars()
                .take(200)
                .collect::<String>();
            line = format!("{line} {input}");
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
        let Some(TaskState::Done(outcome)) = self
            .root
            .and_then(|r| self.state().task(r))
            .map(|t| &t.state)
        else {
            return;
        };
        let said = match outcome {
            TaskOutcome::Result { entry } => self.visible(*entry),
            other => format!("{other:?}"),
        };
        println!("\n== the root is done; its result:\n\n{said}\n");
        self.reported = true;
    }

    /// A stored value's bytes. Every value a fact names is stored.
    fn value(&self, id: ValueId) -> Vec<u8> {
        self.journal
            .store()
            .value(id)
            .unwrap_or_else(|e| panic!("the store reads: {e}"))
            .unwrap_or_else(|| panic!("the value {id} is stored"))
    }

    fn call(&self, id: CallId) -> &Call {
        &self.state().call(id).expect("a known call").call
    }

    /// The visible text of an assistant entry.
    fn visible(&self, entry: EntryId) -> String {
        match self.state().entry(entry) {
            Some(Entry::Message { message, .. }) => render::visible(&self.value(message.content)),
            _ => String::new(),
        }
    }

    fn described(&self, limbs: &[LimbId]) -> Vec<(LimbId, &Description)> {
        limbs.iter().map(|l| (*l, &self.descriptions[l])).collect()
    }
}

/// A failed outcome saying why, and its output's bytes.
/// What a fact starts running, with live progress to show.
fn starts(fact: &Fact) -> Option<Running> {
    match fact {
        Fact::ModelCall(id, ModelCallEvent::Sent(_)) => Some(Running::Model(*id)),
        Fact::Call(id, CallEvent::Dispatched) => Some(Running::Call(*id)),
        _ => None,
    }
}

/// What a fact ends.
fn ends(fact: &Fact) -> Option<Running> {
    match fact {
        Fact::ModelCall(id, ModelCallEvent::Ended { .. }) => Some(Running::Model(*id)),
        Fact::Call(id, CallEvent::Settled(_)) => Some(Running::Call(*id)),
        _ => None,
    }
}

fn failed(said: &str) -> (Vec<u8>, Outcome) {
    let model = render::blocks(&[render::text(said)]);
    let output = Output {
        model: ValueId::of(&model),
        full: None,
        observes: Vec::new(),
    };
    (model, Outcome::Failed(output))
}

/// A user message of these bytes, which go in the batch.
fn user(content: Vec<u8>, values: &mut Vec<Vec<u8>>) -> Message {
    let message = Message {
        role: Role::User,
        content: ValueId::of(&content),
    };
    values.push(content);
    message
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
            LimbKind::Plugin { kind, config, .. } if kind == "workspace" => {
                let config: Value = serde_json::from_str(config).map_err(|e| e.to_string())?;
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

fn kinds(starts: &[Startable]) -> Vec<&str> {
    starts.iter().map(|s| s.agent.as_str()).collect()
}

fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}
