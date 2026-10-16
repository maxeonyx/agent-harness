//! The loop: act on each item of the to-do list, commit what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::context::{Entry, EntryId, Message, Note, Rendered, Role};
use harness_core::delivery::{Content, Delivery, Prompt};
use harness_core::environment::{Description, Environment, FromLimb, LimbKind, Reach, ToLimb};
use harness_core::id::{
    CallId, DeliveryId, FaceId, InstanceId, LimbId, ModelCallId, Seq, TaskId, Timestamp, ToolUseId,
    ValueId,
};
use harness_core::journal::{self, Author, Batch, Fact, Launched, Record};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Stop};
use harness_core::resource::{Change, Observation, ResourceKey};
use harness_core::state::{State, Todo};
use harness_core::task::{
    Context, Created, Drain, Input, Mandate, Mode, Move, Origin, Outcome as TaskOutcome, Scope,
    Start as TaskStart, Stopper, Task, TaskEvent,
};
use harness_core::value::Bytes;
use harness_face_web::{Act, Feed, Running};
use harness_limb_workspace::{Clone, Settings, user};

use crate::process::Process;
use harness_provider::{Client, Finished, Progress};
use harness_store::Store;

use crate::max;
use crate::meta::{self, How};
use crate::{policy, render, roles};

/// How long the loop sleeps when nothing arrives: the resolution of backoff
/// and fan-out timing.
const TICK: Duration = Duration::from_millis(250);
/// How often the role files are read again.
const ROLES_EVERY: Duration = Duration::from_secs(2);

/// What reaches the loop from the threads it started, and from faces.
pub struct Event(What);

enum What {
    Limb(LimbId, FromLimb),
    /// A plugin limb's process has stopped talking.
    Exited(LimbId),
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

/// A task the command line starts in Max's scope, as his: a role, a
/// directory to work in, and a mandate. Like every task he starts, it is a
/// chat with him, done when he says.
pub struct Start {
    pub role: String,
    pub workspace: PathBuf,
    pub name: String,
    pub mandate: String,
}

type Refused = journal::Error<harness_store::Error>;

/// One brain: the only writer of its journal.
pub struct Brain {
    journal: journal::Journal<Store>,
    /// The face, handed the state after each commit.
    face: Sender<State>,
    client: Arc<Client>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    /// Each request's streamed events, for the face.
    feed: Feed,
    meta: LimbId,
    /// Max's limb: his answers to `ask`.
    max: LimbId,
    /// The user limb: `~/.claude`.
    user: LimbId,
    /// Where the role files are, and when they were last read.
    roles: PathBuf,
    roles_read: Instant,
    /// Why the role files last could not be read, said once.
    roles_error: Option<String>,
    /// The plugin limbs this brain has started, by id.
    limbs: HashMap<LimbId, Process>,
    /// Those started that have not said hello yet: a clone being made.
    connecting: HashSet<LimbId>,
    /// Where each limb keeps its record of calls: `<state>/<limb>`.
    state_dir: PathBuf,
    /// The context of each request without a first byte yet: fan-out gating.
    cold: HashMap<ModelCallId, HashSet<EntryId>>,
    /// Each request in flight: the thread that sends it, joined when it
    /// reports how it ended, and the flag that abandons it.
    requests: HashMap<ModelCallId, (Arc<AtomicBool>, JoinHandle<()>)>,
    /// The task this launch started, or else the first of Max's it found
    /// unfinished: the one whose result it prints.
    root: Option<TaskId>,
    started: Instant,
    reported: bool,
}

impl Brain {
    /// Fold the journal, define the roles read from `roles`, and start the
    /// command line's task. What the last writer left in flight is on the
    /// to-do list.
    #[allow(clippy::too_many_arguments)]
    pub fn launch(
        store: Store,
        face: Sender<State>,
        client: Client,
        state_dir: PathBuf,
        roles: &Path,
        start: Option<Start>,
        Inbox { events, inbox }: Inbox,
        feed: Feed,
    ) -> Result<Brain, String> {
        let journal =
            journal::Journal::open(store).map_err(|e| format!("the journal folds: {e}"))?;
        let _ = face.send(journal.state().clone());
        let shared = |is: &dyn Fn(&LimbKind) -> bool| {
            let state = journal.state();
            state
                .environments()
                .flat_map(|e| e.limbs.iter())
                .copied()
                .find(|l| state.limb(*l).is_some_and(|l| is(&l.kind)))
        };
        let meta = shared(&|k| *k == LimbKind::Meta);
        let max = shared(&|k| *k == LimbKind::Max);
        let user_limb =
            shared(&|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == user::KIND));
        let mut brain = Brain {
            journal,
            face,
            client: Arc::new(client),
            events,
            inbox,
            feed,
            meta: meta.unwrap_or_else(LimbId::generate),
            max: max.unwrap_or_else(LimbId::generate),
            user: LimbId::generate(),
            roles: roles.to_path_buf(),
            roles_read: Instant::now(),
            roles_error: None,
            limbs: HashMap::new(),
            connecting: HashSet::new(),
            state_dir,
            cold: HashMap::new(),
            requests: HashMap::new(),
            root: None,
            started: Instant::now(),
            reported: false,
        };
        let mut facts = vec![Fact::Launched(Launched {
            build: concat!("harness-brain ", env!("CARGO_PKG_VERSION")).into(),
            by: None,
        })];
        if meta.is_none() {
            facts.push(Fact::Limb(meta::definition(brain.meta)));
        }
        if max.is_none() {
            facts.push(Fact::Limb(max::definition(brain.max)));
        }
        let user_id = user_limb.unwrap_or_else(LimbId::generate);
        if user_limb.is_none() {
            let home = std::env::var("HOME").map_err(|e| format!("$HOME: {e}"))?;
            let config = PathBuf::from(home).join(".claude");
            facts.push(Fact::Limb(Settings::user(user_id, config)));
        }
        brain.user = user_id;
        brain.append(Batch {
            values: Vec::new(),
            facts,
        });
        brain.define()?;
        brain.root = match start {
            Some(start) => Some(brain.start(start)?),
            None => brain
                .state()
                .tasks()
                .find(|t| t.scope == Scope::Max && !t.is_done())
                .map(|t| t.id),
        };
        Ok(brain)
    }

    /// Start a task in Max's scope, as Max: through the command line, a
    /// face with no page.
    fn start(&mut self, start: Start) -> Result<TaskId, String> {
        let env = self
            .state()
            .environments()
            .find(|e| e.name == start.role)
            .ok_or(format!("no role `{}`", start.role))?
            .clone();
        let dir = start
            .workspace
            .canonicalize()
            .map_err(|e| format!("the workspace {:?}: {e}", start.workspace))?;
        let mut facts = Vec::new();
        let mut places = BTreeMap::new();
        for kind in &env.places {
            let limb = self.workspace_limb(&dir, None, &mut facts);
            places.insert(kind.clone(), limb);
        }
        let mandate = Bytes::from(start.mandate);
        let id = TaskId::generate();
        facts.push(Fact::Task(
            id,
            TaskEvent::Created(Box::new(Created {
                name: start.name,
                by: None,
                scope: Scope::Max,
                origin: Origin::Fresh {
                    environment: env.id,
                    places,
                },
                mode: Mode::UserFacing,
                mandate: Mandate {
                    text: mandate.id(),
                    brief: None,
                    quotes: Vec::new(),
                },
            })),
        ));
        let values = vec![mandate];
        let max = Author::Max {
            face: FaceId(0),
            seen: self.state().last().unwrap_or(Seq(0)),
        };
        self.commit(max, Batch { values, facts })
            .map_err(|e| format!("the task is refused: {e}"))?;
        Ok(id)
    }

    /// The workspace limb for `dir`, which is its name: the one already
    /// defined, or a new one whose definition goes in `facts`, made by
    /// cloning if `clone`.
    fn workspace_limb(&self, dir: &Path, clone: Option<Clone>, facts: &mut Vec<Fact>) -> LimbId {
        let name = dir.to_string_lossy();
        let new = facts.iter().find_map(|f| match f {
            Fact::Limb(l) if l.name == name => Some(l.id),
            _ => None,
        });
        if let Some(id) = new.or_else(|| self.state().limb_named(&name).map(|l| l.id)) {
            return id;
        }
        let id = LimbId::generate();
        let settings = Settings {
            dir: dir.to_path_buf(),
            clone,
        };
        facts.push(Fact::Limb(settings.definition(id, &name)));
        id
    }

    /// Run until killed. The root's result is printed when it is done; the
    /// face keeps serving.
    pub fn run(&mut self) {
        loop {
            if self.roles_read.elapsed() > ROLES_EVERY {
                let error = self.define().err();
                if error.is_some() && error != self.roles_error {
                    eprintln!(
                        "the roles are unchanged: {}",
                        error.as_deref().unwrap_or("")
                    );
                }
                self.roles_error = error;
            }
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

    /// Read the role files, and commit each role whose file changed as a
    /// new version of its environment, with the change to the file, so a
    /// task whose context embodies the old version is told.
    fn define(&mut self) -> Result<(), String> {
        self.roles_read = Instant::now();
        let (meta, max, user) = (self.meta, self.max, self.user);
        let current: Vec<Environment> = self.state().environments().cloned().collect();
        let (defined, values) = roles::environments(
            &roles::read(&self.roles)?,
            &current,
            &|name| match name {
                "meta" => Some(meta),
                "max" => Some(max),
                "user" => Some(user),
                _ => None,
            },
            meta,
        )?;
        let mut facts = Vec::new();
        for env in defined {
            if self.state().environment(env.id) == Some(&env) {
                continue;
            }
            if let Some(source) = &env.source
                && self.state().resource(&source.resource).map(|c| c.version)
                    != Some(source.version)
            {
                facts.push(Fact::Change(Change {
                    resource: source.resource.clone(),
                    version: source.version,
                    by: None,
                    content: None,
                }));
            }
            facts.push(Fact::Environment(env));
        }
        if !facts.is_empty() {
            self.append(Batch { values, facts });
        }
        Ok(())
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
            Todo::Connect(limb) => self.connect(limb),
            Todo::Root(task) => self.root(task),
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
                if let Some((abort, _)) = self.requests.get(&request) {
                    abort.store(true, Ordering::Relaxed);
                }
            }
            Todo::Stop(task) => self.fact(Fact::Task(
                task,
                TaskEvent::Drain(Drain::Stop(Stopper::Owner)),
            )),
            Todo::Settle(call) => self.settle_scope(call),
            Todo::Linger(task) => self.prompt(task, Prompt::Lingering),
            Todo::Unblock(aside) => self.prompt(aside, Prompt::Unblocked),
        }
    }

    /// Ask a task for a turn.
    fn prompt(&mut self, to: TaskId, prompt: Prompt) {
        self.fact(Fact::Delivery(Delivery {
            id: DeliveryId::generate(),
            to,
            content: Content::Prompt(prompt),
        }));
    }

    // Limbs.

    /// Start a plugin limb's process. It says hello once it is ready,
    /// which may take a while (a clone to make).
    fn connect(&mut self, limb: LimbId) {
        if self.limbs.contains_key(&limb) || !self.connecting.insert(limb) {
            return;
        }
        let definition = self.state().limb(limb).expect("a limb to connect");
        let events = self.events.clone();
        let said = move |message: Option<FromLimb>| {
            let what = match message {
                Some(message) => What::Limb(limb, message),
                None => What::Exited(limb),
            };
            let _ = events.send(Event(what));
        };
        match Process::start(definition, &self.state_dir.join(limb.to_string()), said) {
            Ok(process) => {
                self.limbs.insert(limb, process);
            }
            Err(e) => {
                self.connecting.remove(&limb);
                self.unreachable(limb, format!("its process cannot start: {e}"));
            }
        }
    }

    fn opened(&mut self, limb: LimbId, instance: InstanceId) {
        self.connecting.remove(&limb);
        if !self.limbs.contains_key(&limb) {
            return;
        }
        self.fact(Fact::Reach {
            limb,
            reach: Reach::Connected { instance },
        });
        // Report what changed while no brain was listening.
        let resources: Vec<ResourceKey> = self
            .state()
            .tasks()
            .filter(|t| !t.is_done())
            .filter_map(|t| t.context())
            .flat_map(|c| c.knows.keys())
            .filter(|r| r.limb == limb)
            .cloned()
            .collect();
        self.watch(resources);
    }

    /// A limb's process has stopped talking. One that never said hello
    /// cannot start; one that did is lost, and started again when needed.
    fn exited(&mut self, limb: LimbId) {
        self.limbs.remove(&limb);
        if self.connecting.remove(&limb) {
            let why = "its process ended before it was ready (its error is above)";
            return self.unreachable(limb, why.into());
        }
        eprintln!("limb {limb}: its process ended");
        self.fact(Fact::Reach {
            limb,
            reach: Reach::Lost,
        });
    }

    /// A limb cannot be started: every live task that needs it fails.
    fn unreachable(&mut self, limb: LimbId, why: String) {
        eprintln!("limb {limb}: {why}");
        let failing: Vec<TaskId> = self
            .state()
            .tasks()
            .filter(|t| !t.is_done() && t.drain().is_none())
            .filter(|t| self.state().limbs_of(t).contains(&limb))
            .map(|t| t.id)
            .collect();
        for task in failing {
            self.fact(Fact::Task(
                task,
                TaskEvent::Drain(Drain::Fail(format!("a limb it needs cannot start: {why}"))),
            ));
        }
    }

    /// What a limb tells a root of `task`: the meta limb is described per
    /// environment, a plugin as it last said.
    fn description(&self, task: &Task, limb: LimbId) -> Option<Description> {
        if limb == self.max {
            return Some(max::description(limb));
        }
        if limb != self.meta {
            return self.state().described(limb).map(|d| d.description.clone());
        }
        let state = self.state();
        let env = state.environment(task.environment)?;
        let roles: Vec<&str> = env
            .starts
            .iter()
            .filter_map(|e| state.environment(*e))
            .map(|e| e.name.as_str())
            .collect();
        let member = matches!(task.scope, Scope::Spawn(_));
        Some(meta::description(self.meta, &roles, member))
    }

    /// A starting task's first root, once every limb it reaches has
    /// described itself.
    fn root(&mut self, id: TaskId) {
        match self.start_of(id) {
            None => {}
            Some(Err(why)) => self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(why)))),
            Some(Ok((start, values))) => self.moved(id, Move::Root(Box::new(start)), values),
        }
    }

    /// A new root for the task from its environment's current version and
    /// its limbs' descriptions, with the bytes it names; `None` while a limb
    /// has not described itself.
    fn start_of(&self, id: TaskId) -> Option<Result<(TaskStart, Vec<Bytes>), String>> {
        let task = self.state().task(id).expect("a task to root");
        let env = self
            .state()
            .environment(task.environment)
            .expect("the task's environment");
        let mut described = Vec::new();
        for limb in self.state().limbs_of(task) {
            described.push((limb, self.description(task, limb)?));
        }
        let refs: Vec<(LimbId, &Description)> = described.iter().map(|(l, d)| (*l, d)).collect();
        let role = String::from_utf8_lossy(&self.value(env.role)).into_owned();
        Some(render::root(&env.model, &role, &refs).map(|new| {
            let start = TaskStart {
                root: new.root,
                binding: new.binding,
                observes: env.source.iter().cloned().chain(new.observes).collect(),
                awaits: Vec::new(),
            };
            (start, new.values)
        }))
    }

    /// Commit a move to a new root, and watch what it embodies.
    fn moved(&mut self, id: TaskId, movement: Move, values: Vec<Bytes>) {
        let observes = match &movement {
            Move::Root(start) | Move::Handover { start, .. } => start.observes.clone(),
            Move::Rewind { .. } => Vec::new(),
        };
        self.append(Batch {
            values,
            facts: vec![Fact::Task(id, TaskEvent::Moved(movement))],
        });
        self.watch(observes.into_iter().map(|o| o.resource).collect());
    }

    /// The task's handover call, if the round that made it is over: the next
    /// request starts a new context instead.
    fn handover(&mut self, task: &Task) -> bool {
        let Some(call) = task.pending.iter().find_map(|p| match *p {
            Input::Result(call) if self.call(call).tool == meta::HANDOVER => Some(call),
            _ => None,
        }) else {
            return false;
        };
        match self.start_of(task.id) {
            None => {}
            Some(Err(why)) => self.fact(Fact::Task(task.id, TaskEvent::Drain(Drain::Fail(why)))),
            Some(Ok((start, values))) => {
                let start = Box::new(start);
                self.moved(task.id, Move::Handover { call, start }, values);
            }
        }
        true
    }

    // Requests.

    fn send(&mut self, id: TaskId, now: Timestamp) {
        // A response billed outside the plan stops every request.
        if let Some(quota) = self.state().quota().and_then(|e| e.quota.as_ref())
            && !quota.on_plan
        {
            return;
        }
        let task = self.state().task(id).expect("a task to send for").clone();
        if self.handover(&task) {
            return;
        }
        let head = task.head().expect("a ready task has a context");
        let failures = task.live().map_or(0, |l| l.failures);
        if failures >= policy::GIVE_UP {
            let reason = format!("the provider failed {failures} times in a row");
            return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
        }
        // Nothing is in flight for a ready task, so every turn has ended.
        let last = self.state().last_turn(id).cloned();
        if failures > 0
            && let Some(ended) = &last
        {
            let after = match ended.ending {
                Ending::Failed {
                    retry: Some(when), ..
                } => when.0,
                _ => ended.at.at.0 + policy::backoff_ms(failures),
            };
            if now.0 < after {
                return;
            }
        }
        match last.as_ref().map(|e| &e.ending) {
            Some(Ending::Responded {
                stop: Stop::Refused,
                ..
            }) => {
                let reason = "the model declined to go on".to_string();
                return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
            }
            // A context the provider no longer accepts starts again from a
            // new root, with a note of where the old one ended.
            Some(Ending::Responded {
                stop: Stop::ContextWindow,
                ..
            }) if !matches!(self.state().entry(head), Some(Entry::Root(_))) => {
                return match self.start_of(id) {
                    None => {}
                    Some(Err(why)) => self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(why)))),
                    Some(Ok((start, values))) => {
                        self.moved(id, Move::Root(Box::new(start)), values)
                    }
                };
            }
            _ => {}
        }
        if policy::hold(head, self.cold.values()) {
            return;
        }

        // A new root after earlier requests starts again from the mandate.
        let seed = match self.state().entry(head) {
            Some(Entry::Root(_)) => self.seed(id),
            _ => None,
        };
        let first = last.is_none() || seed.is_some();
        let mut values = Vec::new();
        let mut appends = Vec::new();
        let mut renders = Vec::new();
        if first
            && matches!(task.origin, Origin::Fresh { .. })
            && (task.mandate.brief.is_some() || !task.mandate.quotes.is_empty())
        {
            let said = task
                .mandate
                .brief
                .map(|b| String::from_utf8_lossy(&self.value(b)).into_owned());
            let quotes: Vec<_> = task
                .mandate
                .quotes
                .iter()
                .map(|q| (*q, render::words(&self.value(*q))))
                .collect();
            appends.push(user(render::brief(said.as_deref(), &quotes), &mut values));
            renders.push(Rendered::Note(Note::Brief));
        }
        let forked = match task.origin {
            Origin::Fork { .. } | Origin::Aside { .. } if first => self.response_calls(head),
            _ => Vec::new(),
        };
        let turn = render::turn(
            self.state(),
            &|v| self.value(v),
            &task,
            first,
            &forked,
            seed,
        );
        if !turn.blocks.is_empty() {
            appends.push(user(render::blocks(&turn.blocks), &mut values));
        }
        renders.extend(turn.renders);
        let params = self
            .state()
            .environment(task.environment)
            .expect("the task's environment")
            .params;
        let mut call = ModelCall {
            task: id,
            base: head,
            appends,
            renders,
            params,
            breakpoints: Vec::new(),
        };
        let appended: Vec<EntryId> = call.entries().into_iter().map(|(e, _)| e).collect();
        let root = self.state().context(head)[0].0;
        let shared: Vec<EntryId> = [head]
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
            .context(head)
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
        let client = self.client.clone();
        let events = self.events.clone();
        let feed = self.feed.clone();
        let flag = abort.clone();
        let thread = thread::spawn(move || {
            let base: Vec<(EntryId, &Entry)> = base.iter().map(|(id, e)| (*id, e)).collect();
            let finished = client.send(
                request,
                &call,
                &base,
                |id| bytes.get(&id).cloned(),
                |progress| {
                    match progress {
                        Progress::FirstByte => {
                            let _ = events.send(Event(What::FirstByte(request)));
                        }
                        Progress::Live(live) => feed.push(live),
                        Progress::Streaming => {}
                    }
                    if flag.load(Ordering::Relaxed) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            );
            let _ = events.send(Event(What::Finished(request, finished)));
        });
        self.requests.insert(request, (abort, thread));
    }

    /// Where the task's old context ended, and the note it handed over
    /// with, or why it ended without one: what a new root after earlier
    /// requests begins with.
    fn seed(&self, task: TaskId) -> Option<render::Seed> {
        let (_, last) = self
            .state()
            .model_calls()
            .filter(|(_, m)| m.call.task == task)
            .max_by_key(|(_, m)| m.sent.seq)?;
        let handover = last
            .calls
            .iter()
            .map(|c| &self.state().call(*c).expect("a response's call").call)
            .find(|c| c.tool == meta::HANDOVER);
        let note = match handover {
            Some(call) => meta::note(&self.value(call.input)),
            None => "Your previous context no longer fit the model's window, so it ended without a handover. \
Check your workspace for what you had already done before carrying on."
                .into(),
        };
        Some(render::Seed {
            from: last.response.or(Some(last.call.head())),
            note,
        })
    }

    /// The calls a response made, in order: what a fork of it must answer.
    fn response_calls(&self, response: EntryId) -> Vec<CallId> {
        self.state()
            .response(response)
            .map(|m| m.calls.clone())
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
        if let Some((_, thread)) = self.requests.remove(&request) {
            // Its last act was to send this event.
            thread.join().expect("a request's thread does not panic");
        }
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
        batch.values.extend(value.map(Bytes));
        if quota.as_ref().is_some_and(|q| !q.on_plan) {
            eprintln!("a response was billed outside the plan: sending nothing more");
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
    fn tool_uses(&self, task: TaskId, content: &[u8]) -> Vec<(Call, Bytes)> {
        let task = self.state().task(task).expect("the responding task");
        let context = task.context().expect("a responding task has a context");
        // Each block's fields, with a tool's input kept exactly as written.
        type Block<'a> = BTreeMap<String, &'a serde_json::value::RawValue>;
        let blocks: Vec<Block> = serde_json::from_slice(content).unwrap_or_default();
        let field = |b: &Block, key: &str| {
            b.get(key)
                .and_then(|v| serde_json::from_str::<String>(v.get()).ok())
                .unwrap_or_default()
        };
        blocks
            .iter()
            .filter(|b| field(b, "type") == "tool_use")
            .map(|b| {
                let tool = field(b, "name");
                let input = Bytes::from(b.get("input").map_or("{}", |i| i.get()));
                let call = Call {
                    caller: Caller::Agent {
                        task: task.id,
                        tool_use: ToolUseId(field(b, "id")),
                    },
                    expects: expects(context, context.binding.limb(&tool)),
                    tool,
                    input: input.id(),
                };
                (call, input)
            })
            .collect()
    }

    // Calls.

    fn dispatch(&mut self, id: CallId) {
        let record = self.state().call(id).expect("a call to dispatch");
        let call = record.call.clone();
        let limb = record.limb.expect("a routed call");
        if limb == self.meta {
            return self.spawn(id, &call);
        }
        let Some(workspace) = self.limbs.get(&limb) else {
            return eprintln!("call {id}: its limb {limb} is not running here; not dispatched");
        };
        let input = Bytes(self.value(call.input));
        workspace.send(&ToLimb::Run { id, call, input });
        self.fact(Fact::Call(id, CallEvent::Dispatched));
    }

    /// Run a meta call: its members are created in the same batch as its
    /// dispatch, or it fails at once with nothing created. An `add_sibling`
    /// settles in that batch too.
    fn spawn(&mut self, id: CallId, call: &Call) {
        let Caller::Agent { task: parent, .. } = call.caller else {
            unreachable!("only agents call the meta limb")
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
        let input = self.value(call.input);
        if call.tool == meta::HANDOVER {
            let said = render::blocks(&[render::text(
                "Handed over: your next request starts a new context from your note.",
            )]);
            let output = Output {
                model: said.id(),
                full: None,
                observes: Vec::new(),
            };
            batch.values.push(said);
            batch.facts.push(Fact::Call(
                id,
                CallEvent::Settled(Outcome::Completed(output)),
            ));
            return self.append(batch);
        }
        if call.tool == meta::BACKGROUND {
            match background(&parent, id, self.meta, &input) {
                Ok((started, input)) => {
                    let said = render::blocks(&[render::text(&format!(
                        "Started `{}` in the background: its result comes to you as a message when it settles, \
and you cannot finish until it has.",
                        started.tool
                    ))]);
                    let output = Output {
                        model: said.id(),
                        full: None,
                        observes: Vec::new(),
                    };
                    batch.values.extend([said, input]);
                    batch.facts.extend([
                        Fact::Call(CallId::generate(), CallEvent::Made(started)),
                        Fact::Call(id, CallEvent::Settled(Outcome::Completed(output))),
                    ]);
                    return self.append(batch);
                }
                Err(message) => {
                    let (model, failed) = failed(&message);
                    batch.values.push(model);
                    batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
                    return self.append(batch);
                }
            }
        }
        match self.members(id, &call.tool, &parent, &input, &mut batch) {
            Ok(names) if call.tool == meta::ADD_SIBLING => {
                let said = render::blocks(&[render::text(&format!(
                    "Added {} to your scope.",
                    names.join(", ")
                ))]);
                let output = Output {
                    model: said.id(),
                    full: None,
                    observes: Vec::new(),
                };
                batch.values.push(said);
                batch.facts.push(Fact::Call(
                    id,
                    CallEvent::Settled(Outcome::Completed(output)),
                ));
            }
            Ok(_) => {}
            Err(message) => {
                let (model, failed) = failed(&message);
                batch = Batch {
                    values: vec![model],
                    facts: vec![
                        Fact::Call(id, CallEvent::Dispatched),
                        Fact::Call(id, CallEvent::Settled(failed)),
                    ],
                };
            }
        }
        self.append(batch);
    }

    /// The facts creating a meta call's members, with a limb for each new
    /// clone, appended to `batch`; their names.
    fn members(
        &self,
        call: CallId,
        tool: &str,
        parent: &Task,
        input: &[u8],
        batch: &mut Batch,
    ) -> Result<Vec<String>, String> {
        let spawn = meta::parse(tool, input)?;
        let scope = match tool {
            meta::ADD_SIBLING => match parent.scope {
                Scope::Max => return Err("you are not in a scope to add a sibling to".into()),
                scope => scope,
            },
            _ => Scope::Spawn(call),
        };
        let state = self.state();
        let brief = spawn.brief.map(Bytes::from);
        let quotes = spawn
            .quotes
            .iter()
            .map(|label| quoted(state, parent, label))
            .collect::<Result<Vec<_>, String>>()?;
        let mut names = Vec::new();
        for member in spawn.members {
            let origin = match member.how {
                How::Fork => Origin::Fork { from: parent.id },
                How::Resume(name) => {
                    let named = format!("{}-{name}", parent.name);
                    let from = state
                        .tasks()
                        .filter(|t| state.owner(t.id) == Some(parent.id) && t.name == named)
                        // The latest: the one its newest scope holds.
                        .max_by_key(|t| match t.scope {
                            Scope::Spawn(call) => state.call(call).map(|c| c.made.seq),
                            Scope::Max => None,
                        })
                        .ok_or(format!("you started no agent named `{name}`"))?;
                    Origin::Resume { from: from.id }
                }
                How::Fresh { role, branch, base } => {
                    let env = state
                        .environments()
                        .find(|e| e.name == role)
                        .ok_or(format!("there is no role `{role}`"))?;
                    let mut places = BTreeMap::new();
                    for kind in &env.places {
                        let branch = branch
                            .clone()
                            .ok_or(format!("a fresh `{role}` needs a `branch` for its clone"))?;
                        // A workspace limb is named by its directory.
                        let dir = parent
                            .places
                            .get(kind)
                            .and_then(|l| state.limb(*l))
                            .map(|l| PathBuf::from(&l.name))
                            .ok_or(format!("you have no {kind} to clone from"))?;
                        let (from, dir) = policy::clone_of(&dir, &branch);
                        let clone = Clone {
                            from,
                            branch,
                            base: base.clone(),
                        };
                        let limb = self.workspace_limb(&dir, Some(clone), &mut batch.facts);
                        places.insert(kind.clone(), limb);
                    }
                    Origin::Fresh {
                        environment: env.id,
                        places,
                    }
                }
            };
            let fresh = matches!(origin, Origin::Fresh { .. });
            let text = Bytes::from(member.mandate);
            let created = Created {
                name: format!("{}-{}", parent.name, member.name),
                by: Some(call),
                scope,
                origin,
                mode: Mode::Autonomous,
                mandate: Mandate {
                    text: text.id(),
                    brief: brief.as_ref().filter(|_| fresh).map(Bytes::id),
                    quotes: if fresh { quotes.clone() } else { Vec::new() },
                },
            };
            batch.values.push(text);
            batch.facts.push(Fact::Task(
                TaskId::generate(),
                TaskEvent::Created(Box::new(created)),
            ));
            names.push(member.name);
        }
        batch.values.extend(brief);
        Ok(names)
    }

    /// Every member of a spawn's scope is done: settle it with their results,
    /// in the order they were created, which is the order the spawn named
    /// them, then each sibling added.
    fn settle_scope(&mut self, id: CallId) {
        let results: Vec<(String, String)> = self
            .state()
            .members(Scope::Spawn(id))
            .map(|t| {
                let said = match t.outcome() {
                    Some(TaskOutcome::Result { entry }) => self.visible(*entry),
                    Some(TaskOutcome::Failed { reason }) => format!("Failed: {reason}"),
                    Some(TaskOutcome::Cancelled { by }) => format!("Cancelled by {by:?}."),
                    None => unreachable!("a scope settles when every member is done"),
                };
                (t.name.clone(), said)
            })
            .collect();
        let model = meta::result(&results);
        let output = Output {
            model: model.id(),
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

    /// Stop a call: ask its limb, settle it at once if it never went, or
    /// give it up as unknown if its limb cannot be reached to ask.
    fn interrupt(&mut self, id: CallId) {
        let record = self.state().call(id).expect("a call to interrupt");
        let limb = record.limb;
        let settled = if record.dispatched.is_none() {
            Some(Outcome::Interrupted { partial: None })
        } else if limb.is_some_and(|l| self.state().connected(l).is_none()) {
            Some(Outcome::Unknown)
        } else {
            None
        };
        if let Some(outcome) = settled {
            return self.fact(Fact::Call(id, CallEvent::Settled(outcome)));
        }
        self.fact(Fact::Call(id, CallEvent::Stopping));
        if let Some(workspace) = limb.and_then(|l| self.limbs.get(&l)) {
            workspace.send(&ToLimb::Stop { call: id });
        }
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
                        journal::Error::Refused { why, .. } => why.to_string(),
                        e => e.to_string(),
                    });
                let _ = reply.send(done);
            }
            What::Limb(limb, FromLimb::Hello { instance }) => self.opened(limb, instance),
            What::Exited(limb) => self.exited(limb),
            What::Limb(limb, FromLimb::Facts(batch)) => self.limb_facts(limb, batch),
            What::Limb(_, FromLimb::Live(live)) => self.feed.push(live),
        }
    }

    /// Commit what a limb says about its calls and files. A repeat after a
    /// reconnection is refused by the journal and dropped.
    fn limb_facts(&mut self, limb: LimbId, batch: Batch) {
        let observed: Vec<ResourceKey> = batch
            .facts
            .iter()
            .filter_map(|f| match f {
                Fact::Call(_, CallEvent::Settled(outcome)) => outcome.output(),
                _ => None,
            })
            .flat_map(|o| o.observes.iter().map(|o| o.resource.clone()))
            .collect();
        match self.commit(Author::Limb(limb), batch) {
            Ok(()) => self.watch(observed),
            Err(e) => eprintln!("limb {limb}'s facts are refused: {e}"),
        }
    }

    /// Ask each limb to report changes to these resources, telling it every
    /// version the journal holds of each: its current version where one is
    /// recorded, otherwise each version a live context embodies.
    fn watch(&self, resources: Vec<ResourceKey>) {
        let state = self.state();
        let mut known: HashMap<LimbId, Vec<Observation>> = HashMap::new();
        for resource in resources {
            let versions: Vec<_> = match state.resource(&resource) {
                Some(current) => vec![current.version],
                None => state
                    .tasks()
                    .filter(|t| !t.is_done())
                    .filter_map(|t| t.context()?.knows.get(&resource))
                    .map(|k| k.has)
                    .collect(),
            };
            let of = known.entry(resource.limb).or_default();
            for version in versions {
                let o = Observation {
                    resource: resource.clone(),
                    version,
                };
                if !of.contains(&o) {
                    of.push(o);
                }
            }
        }
        for (limb, known) in known {
            if let Some(workspace) = self.limbs.get(&limb) {
                workspace.send(&ToLimb::Watch { known });
            }
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
        let _ = self.face.send(self.state().clone());
        Ok(())
    }

    fn log(&self, record: &Record) {
        let mut line = harness_face_web::describe(self.state(), record);
        if let Fact::Call(id, CallEvent::Made(call)) = &record.fact
            && self
                .state()
                .call(*id)
                .and_then(|c| c.limb)
                .is_some_and(|l| l != self.meta)
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

    /// Print the result of the task this launch started once it is done.
    fn report(&mut self) {
        if self.reported {
            return;
        }
        let Some(outcome) = self
            .root
            .and_then(|r| self.state().task(r))
            .and_then(Task::outcome)
        else {
            return;
        };
        let said = match outcome {
            TaskOutcome::Result { entry } => self.visible(*entry),
            other => format!("{other:?}"),
        };
        println!("\n== the task is done; its result:\n\n{said}\n");
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
}

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

/// A failed outcome saying why, and its output's bytes.
fn failed(said: &str) -> (Bytes, Outcome) {
    let model = render::blocks(&[render::text(said)]);
    let output = Output {
        model: model.id(),
        full: None,
        observes: Vec::new(),
    };
    (model, Outcome::Failed(output))
}

/// A user message of these bytes, which go in the batch.
fn user(content: Bytes, values: &mut Vec<Bytes>) -> Message {
    let message = Message {
        role: Role::User,
        content: content.id(),
    };
    values.push(content);
    message
}

fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

/// What a call to `limb` expects: the versions the context knows of its
/// resources.
fn expects(context: &Context, limb: Option<LimbId>) -> Vec<Observation> {
    context
        .knows
        .iter()
        .filter(|(r, _)| Some(r.limb) == limb)
        .map(|(resource, known)| Observation {
            resource: resource.clone(),
            version: known.has,
        })
        .collect()
}

/// A `background` call's input as the call it starts, with its input's
/// bytes: routed by the caller's binding, never to the meta limb.
fn background(
    parent: &Task,
    from: CallId,
    meta: LimbId,
    input: &[u8],
) -> Result<(Call, Bytes), String> {
    let (tool, input) = meta::background(input)?;
    let context = parent.context().ok_or("you have no context")?;
    let limb = context
        .binding
        .limb(&tool)
        .ok_or(format!("you have no tool `{tool}`"))?;
    if limb == meta {
        return Err(format!("`{tool}` cannot run in the background"));
    }
    let call = Call {
        caller: Caller::Background {
            task: parent.id,
            from,
        },
        expects: expects(context, Some(limb)),
        tool,
        input: input.id(),
    };
    Ok((call, input))
}

/// Max's words a label names, among those `parent` has heard: his messages,
/// the mandates he gave, his answers, and what briefs quoted.
fn quoted(state: &State, parent: &Task, label: &str) -> Result<ValueId, String> {
    let words = state.deliveries().filter_map(|d| match d.delivery.content {
        Content::Words(words) => Some(words),
        Content::Prompt(_) => None,
    });
    let mandates = state
        .tasks()
        .flat_map(|t| [t.mandate.text].into_iter().chain(t.mandate.quotes.clone()));
    let answers = state
        .calls()
        .filter_map(|(_, c)| c.outcome()?.output().map(|o| o.model));
    words
        .chain(mandates)
        .chain(answers)
        .filter(|v| state.heard(parent.id, *v))
        .find(|v| render::label(*v) == label)
        .ok_or(format!(
            "no words of Max's you have heard are labelled `{label}`"
        ))
}
