//! The loop: act on each item of the to-do list, commit what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::build::{BuildId, Launch, Launched};
use harness_core::call::{Call, CallEvent, Caller, Outcome, Output, Owner};
use harness_core::context::{Entry, EntryId, Message, Note, Rendered, Role, Span};
use harness_core::delivery::{Content, Delivery, Prompt};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{
    CallId, DeliveryId, LimbId, ModelCallId, ReplicaId, TaskId, Time, Timestamp, ToolUseId, ValueId,
};
use harness_core::legacy::LimbKind;
use harness_core::limb::{
    Branch, Defined, Hello, Host, Limb, LimbKey, LimbRow, Offer, Prepared, Reach, ToolSet,
    WORKSPACE, Workdir, normal,
};
use harness_core::live::Live;
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Stop, Window};
use harness_core::schema::{self, Record};
use harness_core::state::{State, Todo};
use harness_core::task::{
    Abandoner, Compaction, Created, Drain, Draining, Input, Mandate, Mode, Move, Onto, Origin,
    Outcome as TaskOutcome, Scope, Start as TaskStart, Task, TaskEvent,
};
use harness_core::value::Bytes;
use harness_face_web::{Feed, Folded, Running};
use harness_substrate::{Error, Folding, Publisher, Schema, Up, View, What as Said};

use crate::link::{self, Conn, Wire};
use crate::process::Process;
use harness_provider::{Client, Finished, LoginError, Progress, ToolUse};
use harness_runtime::{Cancel, signals};
use harness_store::Store;

use crate::roles;
use harness_program::meta::{self, How};
use harness_program::{policy, render, web};

/// How long the loop sleeps when nothing arrives: the resolution of backoff
/// and fan-out timing.
const TICK: Duration = Duration::from_millis(250);

/// How long the brain waits before attaching again to a limb it could not
/// reach.
const RESTART: Duration = Duration::from_secs(30);

/// What reaches the loop from the threads it started, from its limbs'
/// links, and from faces.
pub enum Event {
    Link(Conn, link::Said),
    FirstByte(ModelCallId),
    Finished(ModelCallId, Result<Finished, harness_provider::Error>),
    /// A reading of the plan's usage, for Max's call.
    Usage(CallId, Result<BTreeMap<String, Window>, String>),
    /// A signal asking the brain's process to stop.
    Signal(libc::c_int),
    /// The role files as they read now, or why they cannot be read.
    Roles(Result<Vec<roles::File>, String>),
    /// A record from one of Max's tabs.
    Tab(Box<Record>),
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

    /// Where Max's tabs' records go: the brain, which includes each.
    pub fn include(&self) -> Box<dyn Fn(Record) + Send + Sync> {
        let events = self.events.clone();
        Box::new(move |record| {
            let _ = events.send(Event::Tab(Box::new(record)));
        })
    }

    /// Have `SIGINT`, `SIGTERM` and `SIGHUP` reach the brain as events: the
    /// first shuts it down gracefully, the next at once.
    pub fn signals(&self) -> Result<(), String> {
        let events = self.events.clone();
        let stops = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];
        signals::forward(&stops, move |s| {
            let _ = events.send(Event::Signal(s));
        })
        .map_err(|e| format!("signals: {e}"))
    }
}

/// One limb's link, while it is connected.
struct Link {
    conn: Conn,
    replica: ReplicaId,
    wire: Wire,
    publisher: Publisher<LimbKey, LimbRow, Refusal>,
    /// How it connected, until it first speaks: recorded then, so a limb
    /// that says hello and fails before it can serve (its directory cannot
    /// be made) leaves only why.
    unsaid: Option<Reach>,
}

/// One brain: the decider of everything its replicas say.
pub struct Brain {
    decider: schema::Brain<Store>,
    /// The face, handed the state after each change.
    face: Sender<Folded>,
    client: Arc<Client>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    /// Each request's streamed events, for the face.
    feed: Feed,
    /// The limb the brain is, which runs the calls of its own tool sets.
    brain: LimbId,
    /// The limb on the brain's machine: where a fresh task's directory is
    /// when its parent has none.
    local: LimbId,
    /// What its roles' models resolve against.
    models: roles::Models,
    /// The links to limbs this brain is making or has, by id, until their
    /// `attach` exits.
    limbs: HashMap<LimbId, Process>,
    /// When the brain last failed to reach a limb.
    failed: HashMap<LimbId, Instant>,
    /// The limbs whose link ended while they were superseded: their
    /// ending is a move to the build the brain starts, not a failure.
    moving: HashSet<LimbId>,
    /// The limbs connected now.
    links: HashMap<LimbId, Link>,
    /// The next number for a link, counting up from 1.
    attached: Conn,
    /// The model and context of each request without a first byte yet:
    /// fan-out gating.
    cold: HashMap<ModelCallId, (String, HashSet<EntryId>)>,
    /// Each request in flight: the thread that sends it, joined when it
    /// reports how it ended, and its stop.
    requests: HashMap<ModelCallId, (Cancel, JoinHandle<()>)>,
    /// The first of Max's tasks this launch found unfinished: the one
    /// whose result it prints.
    root: Option<TaskId>,
    started: Instant,
    reported: bool,
}

/// What a launch says first: its [`Launched`], and the brain's own limb and
/// the limb on its machine, defined anew where their offers changed.
/// Computed from the folded state without being said, so a gate can check
/// it before anything is written.
pub struct Opening {
    facts: Vec<Fact>,
    brain: LimbId,
    local: LimbId,
    /// The role files the brain reads.
    roles: Option<PathBuf>,
}

impl Opening {
    /// What a launch on `state` says first, at `at`, its record's time,
    /// reading the role files in `roles`, or where the last launch read
    /// them.
    pub fn of(state: &State, at: Time, roles: Option<&Path>) -> Result<Opening, String> {
        let exe = std::env::current_exe()
            .and_then(std::fs::read)
            .map_err(|e| format!("the brain's own executable: {e}"))?;
        let mut facts = vec![Fact::Launched(Launched {
            build: BuildId::of(&exe),
            source: crate::SOURCE.parse().ok(),
            limbs: limb_builds(),
            how: launch_how(state),
        })];
        let roles = match roles {
            Some(dir) => Some(
                dir.canonicalize()
                    .map_err(|e| format!("the roles {dir:?}: {e}"))?,
            ),
            // Where the last launch read them.
            None => (state.brain().and_then(|b| b.offer(roles::SET)))
                .and_then(roles::Settings::of)
                .map(|s| s.dir),
        };
        let id = |facts: &Vec<Fact>, found: Option<&Limb>| {
            found.map_or_else(|| LimbId(at.event(facts.len() as u32)), |l| l.id)
        };
        let limb = brain_limb(id(&facts, state.brain()), roles.as_deref());
        let brain = define(state, &mut facts, limb);
        let found = state.limbs().find(|l| l.host == Host::Local);
        let limb = local_limb(id(&facts, found), found, roles.as_deref())?;
        let local = define(state, &mut facts, limb);
        Ok(Opening {
            facts,
            brain,
            local,
            roles,
        })
    }

    /// `state` once this is said at `at`, or why the state refuses it.
    pub fn said(&self, state: &State, at: Time) -> Result<State, Refusal> {
        let mut state = state.clone();
        let batch = Batch {
            values: Vec::new(),
            facts: self.facts.clone(),
        };
        let folding = Folding {
            here: at,
            clock: now(),
            at,
            said: &batch,
        };
        schema::Harness::fold(&mut state, folding)?;
        Ok(state)
    }
}

impl Brain {
    /// Launch on what `decider` folded: say `opening`, start reading the
    /// role files, and listen for limbs. What the last brain left in flight
    /// is on the to-do list.
    pub fn launch(
        decider: schema::Brain<Store>,
        Opening {
            facts,
            brain,
            local,
            roles,
        }: Opening,
        face: Sender<Folded>,
        client: Client,
        models: roles::Models,
        Inbox { events, inbox }: Inbox,
        feed: Feed,
    ) -> Brain {
        let mut brain = Brain {
            decider,
            face,
            client: Arc::new(client),
            events,
            inbox,
            feed,
            brain,
            local,
            models,
            limbs: HashMap::new(),
            failed: HashMap::new(),
            moving: HashSet::new(),
            links: HashMap::new(),
            attached: 1,
            cold: HashMap::new(),
            requests: HashMap::new(),
            root: None,
            started: Instant::now(),
            reported: false,
        };
        brain.append(Batch {
            values: Vec::new(),
            facts,
        });
        match roles {
            Some(dir) => brain.watch_roles(dir),
            None => errln!("harness-brain: no role files: a new database needs --roles"),
        }
        brain
    }

    /// Read the role files on a thread of their own, which hands each new
    /// reading to the loop.
    fn watch_roles(&self, dir: PathBuf) {
        let events = self.events.clone();
        thread::spawn(move || {
            roles::watch(&dir, |read| events.send(Event::Roles(read)).is_ok());
        });
    }

    /// Bring the roles up to the role files as they read now. A reading
    /// the state refuses changes nothing, and says why.
    fn roles(&mut self, read: Result<Vec<roles::File>, String>) {
        let batch = read.and_then(|files| {
            let current: Vec<&harness_core::role::Role> = self.state().roles().collect();
            let at = self.decider.next();
            roles::batch(&files, &current, &self.models, self.brain, at)
        });
        let refused = match batch {
            Ok(batch) if batch.facts.is_empty() => return,
            Ok(batch) => self.say(batch).map_err(|why| format!("{why:?}")),
            Err(why) => Err(why),
        };
        if let Err(why) = refused {
            errln!("harness-brain: the role files: {why}");
        }
    }

    /// Run until a signal has shut the brain down. The root's result is
    /// printed when it is done; the face keeps serving.
    pub fn run(&mut self) {
        loop {
            if self.down() {
                return;
            }
            self.reap();
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
        self.decider.state()
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
                self.fail_call(call, &said);
            }
            Todo::Interrupt(call) => self.interrupt(call),
            Todo::Cut(request) => {
                if let Some((cancel, _)) = self.requests.get(&request) {
                    cancel.now();
                }
            }
            Todo::Abandon(task) => self.stop_member(task),
            Todo::Tidy(task) => self.prompt(task, Prompt::Tidy),
            Todo::Expire(task) => self.fact(Fact::Task(task, TaskEvent::Expired)),
            Todo::Settle(call) => self.settle_scope(call),
            Todo::Aside(task) => self.aside(task),
            Todo::Linger(task) => self.prompt(task, Prompt::Lingering),
            Todo::Park(task) => self.prompt(task, Prompt::Park),
            Todo::Resume(task) => self.prompt(task, Prompt::Resume),
            Todo::Unblock(aside) => self.prompt(aside, Prompt::Unblocked),
            Todo::SignIn => {
                if let Ok(true) = self.client.login().signed_in() {
                    self.fact(Fact::SignedIn);
                }
            }
        }
    }

    /// Open Max's aside for his unread words to a task that waits on a
    /// scope: the first is its mandate, the rest go with it.
    fn aside(&mut self, from: TaskId) {
        let state = self.state();
        let task = state.task(from).expect("a task due an aside");
        let (Some(call), Some((_, text))) = (
            state.waits_on(task),
            state.unread_words(task).first().copied(),
        ) else {
            return;
        };
        let created = Created {
            name: format!("{}-aside", task.name),
            by: None,
            scope: Scope::Spawn(call),
            origin: Origin::Aside { from },
            mode: Mode::UserFacing,
            mandate: Mandate {
                text,
                brief: None,
                quotes: Vec::new(),
            },
            tuning: Default::default(),
        };
        let id = TaskId(self.decider.next().event(0));
        self.fact(Fact::Task(id, TaskEvent::Created(Box::new(created))));
    }

    /// Ask a task for a turn.
    fn prompt(&mut self, to: TaskId, prompt: Prompt) {
        self.fact(Fact::Delivery(Delivery {
            id: DeliveryId(self.decider.next().event(0)),
            to,
            content: Content::Prompt(prompt),
        }));
    }

    /// Stop a member of a scope whose owner is stopping, with three quarters
    /// of the time the owner has left, so the owner has the rest for its
    /// own tidy-up turn after its members' reports are in.
    fn stop_member(&mut self, task: TaskId) {
        let state = self.state();
        let owner = state.owner(task).and_then(|o| state.task(o));
        let within = match owner.and_then(Task::drain) {
            Some(Draining::Abandoning(s)) if s.hard => 0,
            Some(Draining::Abandoning(s)) => policy::share(now(), s.deadline),
            _ => policy::ms(policy::MEMBER_STOP),
        };
        let stop = Drain::Abandon {
            by: Abandoner::Owner,
            within,
        };
        self.fact(Fact::Task(task, TaskEvent::Drain(stop)));
    }

    // Shutting down.

    /// A signal to stop the brain's process: the first commits a shutdown
    /// that lets the requests in flight finish until its deadline, the next
    /// brings the deadline to now. Tasks are not stopped: the next launch
    /// carries them on.
    fn signal(&mut self, signal: libc::c_int) {
        let again = self.state().shutdown().is_some();
        let within = if again { Duration::ZERO } else { policy::DRAIN };
        let name = signals::name(signal);
        if again {
            errln!("harness-brain: {name} again: stopping now");
        } else {
            errln!(
                "harness-brain: {name}: shutting down; requests in flight have {}s to finish (again to stop now)",
                within.as_secs()
            );
        }
        self.fact(Fact::Shutdown {
            within: policy::ms(within),
        });
    }

    /// Whether the brain has shut down: no request is left, or its
    /// deadline and the grace after it have passed, leaving a request that
    /// will not end to the process's exit. Its limbs are not waited for:
    /// exiting closes their links, and their calls run on for the next
    /// brain.
    fn down(&mut self) -> bool {
        let Some(deadline) = self.state().shutdown() else {
            return false;
        };
        now().0 >= deadline.0 + policy::ms(policy::HARD_WAIT) as i64 || self.requests.is_empty()
    }

    // Limbs.

    /// Attach to a limb on a machine, starting it if it is not running (a
    /// limb from before limbs were per machine only if it is): it says
    /// hello once it is ready. Not while a link to it is made or being made, nor soon after the
    /// brain failed to reach it, nor while the brain shuts down.
    fn connect(&mut self, limb: LimbId) {
        let recently = |at: &Instant| at.elapsed() < RESTART;
        if self.links.contains_key(&limb)
            || self.limbs.contains_key(&limb)
            || self.failed.get(&limb).is_some_and(recently)
            || self.state().shutdown().is_some()
        {
            return;
        }
        // The brain's own sets run in its own process.
        let Some((host, existing)) = on_a_machine(self.state(), limb) else {
            return;
        };
        if let Host::Ssh { destination } = host {
            let why = format!("on {destination}: limbs on other machines are not built yet");
            return self.unreached(limb, why);
        }
        let conn = self.attached;
        self.attached += 1;
        let events = self.events.clone();
        let said = move |conn, said| {
            let _ = events.send(Event::Link(conn, said));
        };
        match Process::attach(limb, existing, conn, said) {
            Ok(process) => {
                self.limbs.insert(limb, process);
            }
            Err(e) => {
                errln!("limb {limb}: cannot attach: {e}");
                self.unreached(limb, format!("cannot attach: {e}"));
            }
        }
    }

    /// The brain could not reach `limb`: try again after [`RESTART`], and
    /// record why, unless that is already the last thing recorded of it.
    fn unreached(&mut self, limb: LimbId, why: String) {
        self.failed.insert(limb, Instant::now());
        if self.state().failed(limb) != Some(why.as_str()) {
            self.fact(Fact::Reach {
                limb,
                reach: Reach::Failed { why },
            });
        }
    }

    /// Let go of the links whose `attach` has exited, recording why of
    /// each that failed: the limb could not start, or ended.
    fn reap(&mut self) {
        let exited: Vec<(LimbId, bool)> = self
            .limbs
            .iter_mut()
            .filter_map(|(id, p)| p.exited().map(|status| (*id, status.success())))
            .collect();
        for (limb, success) in exited {
            let Some(process) = self.limbs.remove(&limb) else {
                continue;
            };
            if self.moving.remove(&limb) {
                self.connect(limb);
            } else if !success && self.state().shutdown().is_none() {
                self.unreached(limb, process.last_words());
            }
        }
    }

    /// A limb connected as `hello.replica`: its records are the limb's from
    /// now on, and it is shipped a whole view.
    fn joined(&mut self, conn: Conn, hello: Hello, wire: Wire) {
        let Hello {
            limb,
            replica,
            build,
        } = hello;
        if on_a_machine(self.state(), limb).is_none() {
            return errln!("a link said it is limb {limb}, which is no limb on a machine here");
        }
        let known = self.state().connected(limb).map(|(p, _)| p) == Some(replica);
        let unsaid = (!known).then_some(Reach::Connected {
            instance: replica,
            build,
        });
        self.links.insert(
            limb,
            Link {
                conn,
                replica,
                wire,
                publisher: Publisher::default(),
                unsaid,
            },
        );
        self.publish();
    }

    /// A record from link `conn`: the limb's, if the link is its current
    /// one, which is connected from its first record on.
    fn heard(&mut self, conn: Conn, record: Record) {
        let Some((&limb, link)) = self.links.iter_mut().find(|(_, l)| l.conn == conn) else {
            return;
        };
        if let Some(reach) = link.unsaid.take() {
            self.fact(Fact::Reach { limb, reach });
        }
        self.include(record);
    }

    /// A link ended. If it was its limb's current one, and it had spoken,
    /// the limb is lost until it connects again.
    fn left(&mut self, conn: Conn) {
        let Some(limb) = self
            .links
            .iter()
            .find_map(|(l, link)| (link.conn == conn).then_some(*l))
        else {
            return;
        };
        let link = self.links.remove(&limb).expect("found");
        errln!("limb {limb}: its link ended");
        if self.state().superseded(limb).is_some() {
            self.moving.insert(limb);
        }
        if link.unsaid.is_some() {
            return;
        }
        self.fact(Fact::Reach {
            limb,
            reach: Reach::Lost,
        });
    }

    /// The tool set `name` as `limb` gives it to a root of `task`: the
    /// brain's own as its code says, `meta` for the task's role; any other
    /// as its limb last described it. `None` for a set its limb has not
    /// described.
    fn tool_set(&self, task: &Task, name: &str, limb: LimbId) -> Option<ToolSet> {
        let state = self.state();
        if limb != self.brain {
            return state.described(limb)?.description.set(name).cloned();
        }
        Some(match name {
            meta::SET => {
                let role = state.role(task.role)?;
                let roles: Vec<(&str, &str)> = (role.starts.iter())
                    .filter_map(|r| state.role(*r))
                    .map(|r| (r.name.as_str(), r.description.as_str()))
                    .collect();
                let member = matches!(task.scope, Scope::Spawn(_));
                meta::set(self.brain, &roles, member)
            }
            web::SET => web::set(),
            // A set with nothing for an agent, such as `roles`.
            name => ToolSet {
                name: name.to_owned(),
                instructions: Vec::new(),
                tools: Vec::new(),
            },
        })
    }

    /// A starting task's first root, once every limb it reaches has
    /// described itself.
    fn root(&mut self, id: TaskId) {
        if let Some((start, values)) = self.restart(id) {
            self.moved(id, Move::Root(Box::new(start)), values);
        }
    }

    /// A new root for the task ([`Brain::start_of`]), or `None` while a limb
    /// has not described itself, or when it cannot be rendered: then the
    /// task fails.
    fn restart(&mut self, id: TaskId) -> Option<(TaskStart, Vec<Bytes>)> {
        match self.start_of(id)? {
            Ok(start) => Some(start),
            Err(why) => {
                self.fail_task(id, why);
                None
            }
        }
    }

    fn fail_task(&mut self, id: TaskId, why: impl Into<String>) {
        self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(why.into()))));
    }

    /// A new root for the task from its role's current version, its tool
    /// sets as their limbs describe them and its working directory's
    /// instructions, with the bytes it names; `None` while a limb has not
    /// described a set or the directory is not ready.
    fn start_of(&self, id: TaskId) -> Option<Result<(TaskStart, Vec<Bytes>), String>> {
        let state = self.state();
        let task = state.task(id)?;
        let Some(role) = state.role(task.role) else {
            return Some(Err(format!("its role {} is not defined", task.role)));
        };
        let mut sets = Vec::new();
        for (name, limb) in state.sets_of(task) {
            sets.push((limb, self.tool_set(task, &name, limb)?));
        }
        let refs: Vec<(LimbId, &ToolSet)> = sets.iter().map(|(l, s)| (*l, s)).collect();
        let workdir = match state.workdir(task) {
            None => Vec::new(),
            Some(w) => match &state.prepared(w.limb, &w.dir)?.prepared {
                Prepared::Ready { instructions } => instructions.clone(),
                Prepared::Failed { .. } => return None,
            },
        };
        let prompt = String::from_utf8_lossy(&self.value(role.prompt)).into_owned();
        Some(
            render::root(
                harness_provider::IDENTITY,
                task.tuning.model(role),
                &prompt,
                &refs,
                &workdir,
            )
            .map(|new| {
                let start = TaskStart {
                    root: new.root,
                    binding: new.binding,
                    observes: role.source.iter().cloned().chain(new.observes).collect(),
                    awaits: Vec::new(),
                };
                (start, new.values)
            }),
        )
    }

    /// Say a move of a task's head. Its limbs watch what it embodies
    /// through their rows.
    fn moved(&mut self, id: TaskId, movement: Move, values: Vec<Bytes>) {
        self.append(Batch {
            values,
            facts: vec![Fact::Task(id, TaskEvent::Moved(movement))],
        });
    }

    /// Compact the task if the round of its compaction call is over: what
    /// it did since its latest checkpoint, or its whole context when it
    /// asks for a fresh one or has no checkpoint, is replaced by what it
    /// carries. False when it asked for none.
    fn compact(&mut self, task: &Task) -> bool {
        let asked = task.pending.iter().find_map(|p| {
            let Input::Result(call) = *p else {
                return None;
            };
            let record = &self.state().call(call)?.call;
            if !meta::compacts(&record.tool)
                || self.state().has_compacted(call)
                || !matches!(
                    self.state().call(call)?.outcome(),
                    Some(Outcome::Completed(_))
                )
            {
                return None;
            }
            match meta::compaction(&record.tool, &self.value(record.input)) {
                Ok(ask) => Some((call, ask)),
                Err(why) => {
                    errln!("compaction call {call} was accepted but cannot compact: {why}");
                    None
                }
            }
        });
        let Some((call, ask)) = asked else {
            return false;
        };
        let checkpoint = self.state().checkpoint(task).filter(|_| !ask.fresh);
        let (onto, values, why) = match checkpoint {
            Some(at) => (Onto::Checkpoint(at), Vec::new(), render::Why::Checkpoint),
            None => match self.restart(task.id) {
                None => return true,
                Some((start, values)) => (Onto::Fresh(Box::new(start)), values, render::Why::Fresh),
            },
        };
        let calls = round(task, Some(call));
        let carry = render::Carry {
            summary: Some(&ask.summary),
            why,
            words: &[],
            calls: &calls,
        };
        let report = ask.report.map(|r| words(&r));
        self.compacted(task, Some(call), onto, carry, values, report);
        true
    }

    /// State a compaction of `task` onto `onto`, carrying `carry` with Max's
    /// words from what it replaces.
    fn compacted(
        &mut self,
        task: &Task,
        by: Option<CallId>,
        onto: Onto,
        carry: render::Carry,
        mut values: Vec<Bytes>,
        report: Option<Bytes>,
    ) {
        let since = match &onto {
            Onto::Checkpoint(at) => Some(*at),
            Onto::Fresh(_) => None,
        };
        let words = render::kept(self.state(), task, since);
        let carry = render::Carry {
            words: &words,
            ..carry
        };
        let turn = render::carry(self.state(), &|v| self.value(v), task, &carry);
        let content = render::blocks(&turn.blocks);
        let compaction = Compaction {
            by,
            onto,
            carry: content.id(),
            renders: turn.renders,
            report: report.as_ref().map(Bytes::id),
        };
        values.push(content);
        values.extend(report);
        self.moved(task.id, Move::Compact(Box::new(compaction)), values);
    }

    // Requests.

    fn send(&mut self, id: TaskId, now: Timestamp) {
        let task = self.state().task(id).expect("a task to send for").clone();
        // A stopping task's tidy-up turn is its last: it does not compact.
        let ending = task.ends();
        if !ending && self.compact(&task) {
            return;
        }
        let head = task.head().expect("a ready task has a context");
        let failures = task.live().map_or(0, |l| l.failures);
        // Nothing is in flight for a ready task, so every turn has ended.
        let last = self.state().last_turn(id).cloned();
        if failures > 0
            && let Some(Ending::Failed {
                status, message, ..
            }) = last.as_ref().map(|e| &e.ending)
        {
            let said = match status {
                Some(s) => format!("{s}: {message}"),
                None => message.clone(),
            };
            if policy::rejected(*status) {
                return self.fail_task(id, format!("the provider rejected the request, {said}"));
            }
            if failures >= policy::GIVE_UP {
                return self.fail_task(
                    id,
                    format!("the provider failed {failures} times in a row, last with {said}"),
                );
            }
        }
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
        // What follows is read off the head: the response it is, if any,
        // while nothing has moved on from it but its calls' results.
        let state = self.state();
        let said = state
            .response(head)
            .and_then(|m| Some((m, &m.ended.as_ref()?.ending)));
        match said {
            Some((
                _,
                Ending::Responded {
                    stop: Stop::Refused,
                    ..
                },
            )) => return self.fail_task(id, "the model declined to go on"),
            // A context the provider no longer accepts starts again from a
            // new root, with the mandate, Max's words and its calls' results.
            Some((
                overflowed,
                Ending::Responded {
                    stop: Stop::ContextWindow,
                    ..
                },
            )) => {
                let base = overflowed.call.base;
                let afresh = state.compacted(base).is_some()
                    && state
                        .entry(base)
                        .and_then(Entry::parent)
                        .is_some_and(|p| matches!(state.entry(p), Some(Entry::Root(_))));
                if ending || afresh {
                    let reason = if ending {
                        "its context filled during its tidy-up turn"
                    } else {
                        "its context does not fit the model's window even started afresh"
                    };
                    return self.fail_task(id, reason);
                }
                if let Some((start, values)) = self.restart(id) {
                    let calls = round(&task, None);
                    let carry = render::Carry {
                        summary: None,
                        why: render::Why::Full,
                        words: &[],
                        calls: &calls,
                    };
                    let onto = Onto::Fresh(Box::new(start));
                    self.compacted(&task, None, onto, carry, values, None);
                }
                return;
            }
            _ => {}
        }
        let Next {
            call,
            values,
            model,
        } = next(self.state(), &|v| self.value(v), &task, false);
        if policy::hold(
            head,
            &model,
            self.cold.values().map(|(m, c)| (m.as_str(), c)),
        ) {
            return;
        }
        let appended: Vec<EntryId> = call.entries().into_iter().map(|(e, _)| e).collect();

        let request = ModelCallId(self.decider.next().event(0));
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
            (
                model,
                base.iter().map(|(id, _)| *id).chain(appended).collect(),
            ),
        );
        let cancel = Cancel::new();
        let client = self.client.clone();
        let events = self.events.clone();
        let feed = self.feed.clone();
        let stop = cancel.clone();
        let thread = thread::spawn(move || {
            let base: Vec<(EntryId, &Entry)> = base.iter().map(|(id, e)| (*id, e)).collect();
            let finished = client.send(
                &call,
                &base,
                |id| bytes.get(&id).cloned(),
                &stop,
                |progress| match progress {
                    Progress::FirstByte => {
                        let _ = events.send(Event::FirstByte(request));
                    }
                    Progress::Text(chunk) => feed.push(Live::Model {
                        call: request,
                        chunk,
                    }),
                },
            );
            let _ = events.send(Event::Finished(request, finished));
        });
        self.requests.insert(request, (cancel, thread));
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
        let at = self.decider.next();
        let mut batch = Batch::default();
        let Finished {
            ending,
            value,
            usage,
            quota,
        } = finished.unwrap_or_else(|e| Finished {
            ending: match e {
                harness_provider::Error::Login(LoginError::SignedOut) => Ending::SignedOut,
                e => Ending::Failed {
                    status: None,
                    message: e.to_string(),
                    retry: None,
                },
            },
            value: None,
            usage: None,
            quota: None,
        });
        if let (Ending::Responded { .. }, Some(content)) = (&ending, &value) {
            for (call, input) in self.tool_uses(task, content) {
                batch.values.push(input);
                let id = CallId(at.event(batch.facts.len() as u32));
                batch.facts.push(Fact::Call(id, CallEvent::Made(call)));
            }
        }
        batch.values.extend(value.map(Bytes));
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
        let state = self.state();
        let task = state.task(task).expect("the responding task");
        let context = task.context().expect("a responding task has a context");
        harness_provider::tool_uses(content)
            .into_iter()
            .map(|ToolUse { id, name, input }| {
                let input = Bytes::from(input.as_str());
                let call = Call {
                    caller: Caller::Agent {
                        task: task.id,
                        tool_use: ToolUseId(id),
                    },
                    expects: meta::expects(state, context, context.binding.limb(&name)),
                    tool: name,
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
        let Some(limb) = record.limb else {
            return self.fail_call(id, "The harness routed this call nowhere.");
        };
        // The brain's own sets are told apart by tool name: Max's reading of
        // the plan's usage, and every other is `meta`'s.
        if self.state().resolve(limb) == Some(self.brain) {
            return match call.tool.as_str() {
                web::USAGE => self.read_usage(id, &call),
                _ => self.spawn(id, &call),
            };
        }
        // The limb runs it from its row.
        self.fact(Fact::Call(id, CallEvent::Dispatched));
    }

    /// Run Max's call to the provider off the loop's thread, the way a
    /// request runs, and settle it when it reports. It only reads, so it is
    /// not stopped at shutdown: a later brain runs it again.
    fn read_usage(&mut self, id: CallId, call: &Call) {
        if !matches!(call.caller, Caller::Max { .. }) {
            let said = format!("The harness offers agents no tool `{}`.", call.tool);
            return self.fail_call(id, &said);
        }
        self.fact(Fact::Call(id, CallEvent::Dispatched));
        let client = self.client.clone();
        let events = self.events.clone();
        thread::spawn(move || {
            let reading = client.usage(&Cancel::new());
            let _ = events.send(Event::Usage(id, reading));
        });
    }

    fn read(&mut self, id: CallId, reading: Result<BTreeMap<String, Window>, String>) {
        match reading {
            Ok(reading) => {
                let (values, outcome) = web::usage(&reading, now());
                self.append(Batch {
                    values,
                    facts: vec![Fact::Call(id, CallEvent::Settled(outcome))],
                });
            }
            Err(why) => self.fail_call(id, &format!("Could not read the plan's usage: {why}")),
        }
    }

    /// Run a meta call: its members are created in the same batch as its
    /// dispatch, or it fails at once with nothing created. An `add_sibling`
    /// settles in that batch too.
    fn spawn(&mut self, id: CallId, call: &Call) {
        // The state lets Max call any tool of the brain's, and `meta`
        // offers him none.
        let Caller::Agent { task: parent, .. } = call.caller else {
            let said = format!("The harness offers Max no tool `{}`.", call.tool);
            return self.fail_call(id, &said);
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
        let said = if parent.ends() {
            // Only a stopping task's tidy-up turn makes calls while it ends.
            Err(
                "You are being stopped: nothing new can start, and there is no next context. \
Finish tidying up and end your turn with your report."
                    .to_string(),
            )
        } else if meta::compacts(&call.tool) {
            // A compaction happens once its round is over ([`Brain::compact`]).
            let asked = if call.tool == meta::COMPACT_AND_CONTINUE
                && meta::repeats(self.state(), &parent)
            {
                Err("you compacted last and have made no other call since, so there is nothing to replace. \
Carry on with your next step, or, if your work is done, call `compact_and_report`"
                    .to_string())
            } else {
                meta::compaction(&call.tool, &input).map(|_| ())
            };
            asked
                .map(|()| words("Compacting once this round is over."))
                .map_err(|why| format!("Not compacted: {why}."))
        } else if call.tool == meta::ATTACH {
            meta::attach(self.state(), &|v| self.value(v), &parent, &input)
        } else if call.tool == meta::BACKGROUND {
            meta::background_call(self.state(), &parent, id, self.brain, &input).map(
                |(started, input)| {
                    let said = words(&format!(
                        "Started `{}` in the background: its result comes to you as a message when it settles, \
and you cannot finish until it has.",
                        started.tool
                    ));
                    let made = CallId(self.decider.next().event(batch.facts.len() as u32));
                    batch.values.push(input);
                    batch.facts.push(Fact::Call(made, CallEvent::Made(started)));
                    said
                },
            )
        } else if call.tool == meta::STOP || call.tool == meta::PASS {
            self.hand(&call.tool, &parent, &input, &mut batch)
                .map(|said| words(&said))
        } else if call.tool == meta::SPAWN || call.tool == meta::ADD_SIBLING {
            match self.members(id, &call.tool, &parent, &input, &mut batch) {
                Ok(names) if call.tool == meta::ADD_SIBLING => {
                    Ok(words(&format!("Added {} to your scope.", names.join(", "))))
                }
                // A spawn settles when its scope does.
                Ok(_) => return self.answer(id, batch),
                Err(message) => {
                    // Of what the refused call wrote, only its dispatch stands.
                    batch.values.clear();
                    batch.facts.truncate(1);
                    Err(message)
                }
            }
        } else {
            // A tool a root declared that the harness does not have.
            Err(format!("The harness has no `{}`.", call.tool))
        };
        settle(id, said, &mut batch);
        self.answer(id, batch)
    }
    /// Say the batch a meta call's dispatch makes. What an agent asks for
    /// may be what the state refuses: then the call fails with why, and
    /// nothing else is written.
    fn answer(&mut self, id: CallId, batch: Batch) {
        let Err(why) = self.say(batch) else {
            return;
        };
        let (model, failed) = render::failed(&format!("Nothing was done: {why}."));
        self.append(Batch {
            values: vec![model],
            facts: vec![
                Fact::Call(id, CallEvent::Dispatched),
                Fact::Call(id, CallEvent::Settled(failed)),
            ],
        });
    }

    /// A `stop` or `pass` of one of `parent`'s running background calls:
    /// its fact, appended to `batch`, and what the call says.
    fn hand(
        &self,
        tool: &str,
        parent: &Task,
        input: &[u8],
        batch: &mut Batch,
    ) -> Result<String, String> {
        let state = self.state();
        let (target, why) = meta::owned(state, parent, input)?;
        if tool == meta::STOP {
            batch.facts.push(Fact::Call(target, CallEvent::Abandon));
            return Ok(
                "Stopping it: its outcome comes to you as a message when it settles.".into(),
            );
        }
        let (to, whom) = match state.owner(parent.id) {
            Some(owner) => {
                let owner = state.task(owner).expect("the owning task");
                if owner.drain().is_some() {
                    return Err(format!(
                        "`{}`, which started you, is stopping, so it cannot take it",
                        owner.name
                    ));
                }
                (
                    Owner::Task(owner.id),
                    format!("`{}`, which started you", owner.name),
                )
            }
            None if parent.scope == Scope::Max => (Owner::Max, "Max".to_owned()),
            None => return Err("there is no one above you to pass it to".into()),
        };
        batch
            .facts
            .push(Fact::Call(target, CallEvent::Passed { to, why }));
        Ok(format!(
            "Passed it to {whom}: it keeps running, it is theirs to use and stop, and its outcome goes to them."
        ))
    }

    /// The facts creating a meta call's members, appended to `batch`; their
    /// names.
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
        let at = self.decider.next();
        let brief = spawn.brief.map(Bytes::from);
        let quotes = spawn
            .quotes
            .iter()
            .map(|label| meta::quoted(state, parent, label))
            .collect::<Result<Vec<_>, String>>()?;
        let mut names = Vec::new();
        for member in spawn.members {
            let origin = match member.how {
                How::Fork => Origin::Fork { from: parent.id },
                How::Resume(name) => {
                    let from = state
                        .tasks()
                        .filter(|t| state.owner(t.id) == Some(parent.id) && t.name == name)
                        // The latest: the one its newest scope holds.
                        .max_by_key(|t| match t.scope {
                            Scope::Spawn(call) => state.call(call).map(|c| c.made.seq),
                            Scope::Max => None,
                        })
                        .ok_or(format!("you started no agent named `{name}`"))?;
                    Origin::Resume { from: from.id }
                }
                How::Fresh {
                    role,
                    branch,
                    base,
                    from,
                } => {
                    let own = state.role(parent.role).ok_or("your role is not defined")?;
                    let starts: Vec<_> = state
                        .roles()
                        .filter(|r| own.starts.contains(&r.id))
                        .collect();
                    let role = *starts.iter().find(|r| r.name == role).ok_or_else(|| {
                        let names: Vec<&str> = starts.iter().map(|r| r.name.as_str()).collect();
                        let can = match names.is_empty() {
                            true => "only forks of itself".to_owned(),
                            false => names.join(", "),
                        };
                        format!(
                            "{} cannot start a fresh `{role}`: it can start {can}",
                            own.name
                        )
                    })?;
                    let workdir = match role.works_in_a_directory() {
                        true => Some(self.workdir(parent, from, branch, base)?),
                        false => None,
                    };
                    Origin::Fresh {
                        role: role.id,
                        workdir,
                        places: BTreeMap::new(),
                    }
                }
            };
            let fresh = matches!(origin, Origin::Fresh { .. });
            let text = Bytes::from(member.mandate);
            let created = Created {
                tuning: Default::default(),
                name: member.name.clone(),
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
            let id = TaskId(at.event(batch.facts.len() as u32));
            batch
                .facts
                .push(Fact::Task(id, TaskEvent::Created(Box::new(created))));
            names.push(member.name);
        }
        batch.values.extend(brief);
        Ok(names)
    }

    /// A fresh member's working directory: its parent's, the directory
    /// `from` names under it (or on the brain's machine, for a parent with
    /// none), or with a `branch`, a clone of its own of that repository,
    /// beside it, on that branch, made from `base`.
    fn workdir(
        &self,
        parent: &Task,
        from: Option<String>,
        branch: Option<String>,
        base: String,
    ) -> Result<Workdir, String> {
        let (limb, dir) = match (self.state().workdir(parent), from) {
            (Some(own), Some(from)) => (own.limb, own.dir.join(from)),
            (Some(own), None) => (own.limb, own.dir),
            (None, Some(from)) => (self.local, PathBuf::from(from)),
            (None, None) => {
                return Err("you have no working directory to share or clone from".into());
            }
        };
        let dir = normal(&dir);
        // Without a branch it works where it is pointed, as a fork does.
        let Some(name) = branch else {
            return Ok(Workdir {
                limb,
                dir,
                branch: None,
            });
        };
        let (from, dir) = clone_of(&dir, &name)?;
        Ok(Workdir {
            limb,
            dir: normal(&dir),
            branch: Some(Branch { from, name, base }),
        })
    }

    /// Every member of a spawn's scope is done: settle it with their results,
    /// in the order they were created, which is the order the spawn named
    /// them, then each sibling added.
    fn settle_scope(&mut self, id: CallId) {
        let mut observes = Vec::new();
        let results: Vec<(String, Vec<String>)> = self
            .state()
            .members(Scope::Spawn(id))
            .map(|t| {
                let said = match t.outcome() {
                    Some(TaskOutcome::Result { entry }) => {
                        let mut said = vec![render::text(&self.visible(*entry))];
                        for call in self.attached(*entry) {
                            said.extend(render::carried(
                                self.state(),
                                &|v| self.value(v),
                                "Its",
                                call,
                            ));
                            if let Some(output) =
                                self.state().call(call).and_then(|c| c.outcome()?.output())
                            {
                                observes.extend(output.observes.iter().cloned());
                            }
                        }
                        said
                    }
                    Some(TaskOutcome::Failed { reason }) => {
                        vec![render::text(&format!("Failed: {reason}"))]
                    }
                    Some(TaskOutcome::Abandoned { by, report }) => {
                        vec![render::text(&self.cancelled(*by, *report))]
                    }
                    None => unreachable!("a scope settles when every member is done"),
                };
                (t.name.clone(), said)
            })
            .collect();
        let model = meta::result(&results);
        let output = Output {
            model: model.id(),
            full: None,
            observes,
        };
        self.append(Batch {
            values: vec![model],
            facts: vec![Fact::Call(
                id,
                CallEvent::Settled(Outcome::Completed(output)),
            )],
        });
    }

    /// The calls a member's result carries: those of the compaction it
    /// carries on from, if no response of its own came between, or else the
    /// `attach` calls of its last response before the report.
    fn attached(&self, result: EntryId) -> Vec<CallId> {
        let state = self.state();
        let mut at = state.entry(result).and_then(Entry::parent);
        while let Some(entry) = at {
            if let Some(compacted) = state.compacted(entry) {
                return compacted
                    .renders
                    .iter()
                    .filter_map(|span| match span.from {
                        Rendered::Call(call) => Some(call),
                        _ => None,
                    })
                    .collect();
            }
            if let Some(request) = state.request(entry) {
                let attached: Vec<CallId> = request
                    .call
                    .renders
                    .iter()
                    .filter_map(|span| match span.from {
                        Rendered::Call(call)
                            if state
                                .call(call)
                                .is_some_and(|c| c.call.tool == meta::ATTACH) =>
                        {
                            Some(call)
                        }
                        _ => None,
                    })
                    .collect();
                if !attached.is_empty() {
                    return attached;
                }
            }
            at = match state.entry(entry) {
                Some(Entry::Message { parent, message }) if message.role == Role::User => {
                    Some(*parent)
                }
                _ => None,
            };
        }
        Vec::new()
    }

    /// What a cancelled member says to the task that reads its result.
    fn cancelled(&self, by: Abandoner, report: Option<EntryId>) -> String {
        let by = match by {
            Abandoner::Max => "Abandoned by Max",
            Abandoner::Owner => "Abandoned, because you are being abandoned",
        };
        match report {
            Some(entry) => format!("{by}. Its report:\n\n{}", self.visible(entry)),
            None => format!("{by}, before it could write a report."),
        }
    }

    /// Stop a call: ask its limb, with time to stop gracefully before the
    /// tasks reading it must have stopped; settle it at once if it never
    /// went, or give it up as unknown if its limb cannot be reached to ask.
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
        let within = match self.state().stop_by(id) {
            Some(by) => policy::share(now(), by),
            None => policy::ms(policy::CALL_GRACE),
        };
        // The limb stops it from its row.
        self.fact(Fact::Call(id, CallEvent::Stopping { within }));
    }

    /// Settle a call the brain answers itself as failed, saying why.
    fn fail_call(&mut self, id: CallId, said: &str) {
        let (model, failed) = render::failed(said);
        self.append(Batch {
            values: vec![model],
            facts: vec![Fact::Call(id, CallEvent::Settled(failed))],
        });
    }

    // What arrives.

    fn handle(&mut self, event: Event) {
        match event {
            Event::FirstByte(request) => self.first_byte(request),
            Event::Finished(request, finished) => self.finished(request, finished),
            Event::Usage(call, reading) => self.read(call, reading),
            Event::Signal(signal) => self.signal(signal),
            Event::Roles(read) => self.roles(read),
            Event::Tab(record) => {
                // A tab new to the brain is one of Max's faces.
                let replica = record.at.replica;
                if !self.state().is_face(replica) {
                    self.fact(Fact::Face(replica));
                }
                self.include(*record);
            }
            Event::Link(conn, link::Said::Joined(hello, wire)) => self.joined(conn, hello, wire),
            Event::Link(conn, link::Said::Up(up)) => match *up {
                // Only the current link speaks for its limb.
                Up::Record(record) => self.heard(conn, record),
                Up::Live(live) => self.feed.push(live.live),
            },
            Event::Link(conn, link::Said::Left) => self.left(conn),
        }
    }

    // The brain's records.

    /// Say one of the brain's own facts that names no value.
    fn fact(&mut self, fact: Fact) {
        self.append(Batch {
            values: Vec::new(),
            facts: vec![fact],
        });
    }

    /// Say the brain's own batch, at [`schema::Brain::next`], which the ids
    /// it creates are minted from. A refusal is a bug in the brain: nothing
    /// is written, and it stops.
    fn append(&mut self, batch: Batch) {
        let shown: String = format!("{:?}", batch.facts).chars().take(600).collect();
        if let Err(why) = self.say(batch) {
            panic!("the brain refuses its own batch ({why:?}): {shown}");
        }
    }

    /// Say the brain's own batch, or, if the state refuses it, nothing.
    fn say(&mut self, batch: Batch) -> Result<(), Refusal> {
        let started: Vec<Running> = batch.facts.iter().filter_map(starts).collect();
        for running in &started {
            self.feed.start(*running);
        }
        let facts = batch.facts.clone();
        match self.decider.say(now(), |_| batch) {
            Ok(at) => {
                self.changed(at, &facts);
                Ok(())
            }
            Err(Error::Refused(why)) => {
                for running in started {
                    self.feed.end(running);
                }
                Err(why)
            }
            Err(Error::Storage(e)) => panic!("harness-brain: cannot store its own batch: {e}"),
        }
    }

    /// Include a record from a limb or a tab. A refusal is the replica's to
    /// see, in its standing; it is not a bug.
    fn include(&mut self, record: Record) {
        let from = record.at;
        let facts = match &record.what {
            Said::Said(batch) => batch.facts.clone(),
            Said::Unread(unread) => {
                outln!(
                    "{:>7.1}s {from}  held unread: {}",
                    self.elapsed(),
                    unread.why
                );
                Vec::new()
            }
            Said::Included(_) => Vec::new(),
        };
        match self.decider.include(now(), record) {
            Ok(None) => {}
            Ok(Some(_)) => match self.decider.standing(from.replica).refused.get(&from.n) {
                Some(why) => {
                    outln!("{:>7.1}s {from}  refused: {why}", self.elapsed());
                    self.publish();
                }
                None => self.changed(from, &facts),
            },
            Err(e) => errln!("harness-brain: cannot store the record at {from}: {e}"),
        }
    }

    /// After the brain's state moved on by `facts`, said at `at`: log them,
    /// end the live progress of what they ended, and publish.
    fn changed(&mut self, at: Time, facts: &[Fact]) {
        for fact in facts {
            self.log(at, fact);
            if let Some(running) = ends(fact) {
                self.feed.end(running);
            }
        }
        self.publish();
    }

    /// Hand the face the state, and ship each linked limb what changed in
    /// its rows.
    fn publish(&mut self) {
        let state = self.state().clone();
        let as_of = self.decider.head();
        let _ = self.face.send(Folded {
            state: state.clone(),
            as_of,
            standing: self.decider.standings().clone(),
        });
        let store = self.decider.storage();
        for (limb, link) in &mut self.links {
            let mut rows = state.limb_rows(*limb);
            let inputs: Vec<ValueId> = rows
                .values()
                .filter_map(|r| match r {
                    LimbRow::Call { call, .. } => Some(call.input),
                    _ => None,
                })
                .collect();
            for id in inputs {
                if let Ok(Some(bytes)) = store.value(id) {
                    rows.insert(LimbKey::Value(id), LimbRow::Value(Bytes(bytes)));
                }
            }
            let view = View {
                as_of,
                standing: self.decider.standing(link.replica),
                rows,
            };
            // `Superseded` changes on a line of its own: a limb of a build
            // before it cannot read it, and drops that line whole.
            let mut before = view.clone();
            let held = link
                .publisher
                .shipped()
                .and_then(|v| v.rows.get(&LimbKey::Superseded))
                .cloned();
            match held {
                Some(row) => before.rows.insert(LimbKey::Superseded, row),
                None => before.rows.remove(&LimbKey::Superseded),
            };
            for view in [before, view] {
                if let Some(down) = link.publisher.publish(view) {
                    let _ = link.wire.send(down);
                }
            }
        }
    }

    fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn log(&self, at: Time, fact: &Fact) {
        let mut line = harness_face_web::describe(self.state(), fact);
        if let Fact::Call(id, CallEvent::Made(call)) = fact
            && self
                .state()
                .call(*id)
                .and_then(|c| c.limb)
                .is_some_and(|l| self.state().resolve(l) != Some(self.brain))
        {
            let input = String::from_utf8_lossy(&self.value(call.input))
                .chars()
                .take(200)
                .collect::<String>();
            line = format!("{line} {input}");
        }
        outln!("{:>7.1}s {at}  {line}", self.elapsed());
    }

    /// Print the result of the task this launch started once it is done.
    fn report(&mut self) {
        if self.reported {
            return;
        }
        if self.root.is_none() {
            let unfinished = self
                .state()
                .tasks()
                .find(|t| t.scope == Scope::Max && !t.is_done());
            self.root = unfinished.map(|t| t.id);
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
            TaskOutcome::Abandoned { by, report } => self.cancelled(*by, *report),
            other => format!("{other:?}"),
        };
        outln!("\n== the task is done; its result:\n\n{said}\n");
        self.reported = true;
    }

    /// A stored value's bytes. Every value a fact names is stored.
    fn value(&self, id: ValueId) -> Vec<u8> {
        self.decider
            .storage()
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

/// Where `limb` runs, if it is a process on a machine, and whether it is a
/// limb from before limbs were per machine.
fn on_a_machine(state: &State, limb: LimbId) -> Option<(Host, bool)> {
    match (state.limb(limb), state.legacy(limb)) {
        (Some(now), _) => (now.host != Host::Brain).then(|| (now.host.clone(), false)),
        (None, Some(old)) => match &old.kind {
            LimbKind::Plugin { host, .. } if *host != Host::Brain => Some((host.clone(), true)),
            _ => None,
        },
        (None, None) => None,
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

/// The request a task sends next, and the new values it names.
pub struct Next {
    pub call: ModelCall,
    pub values: Vec<Bytes>,
    /// The model it goes to: the task's, which its root may not name.
    pub model: String,
}

/// The request `task` sends next, rendered from `state` and the bytes
/// `value` reads. With `unanswered`, a tool use of its head with no result
/// yet is answered as done elsewhere, as a fork's first request answers its
/// parent's: what a check renders for a session still waiting on a call.
pub fn next(
    state: &State,
    value: &dyn Fn(ValueId) -> Vec<u8>,
    task: &Task,
    unanswered: bool,
) -> Next {
    let head = task.head().expect("a task that sends has a context");
    let role = state.role(task.role).expect("the task's role");
    let model = task.tuning.model(role).to_owned();
    let params = role.params;
    let calls = || {
        state
            .response(head)
            .map(|m| m.calls.clone())
            .unwrap_or_default()
    };

    // The first request of a context renders its beginning: at its
    // start, however often it has been sent from there. A context
    // compacted onto a fresh root has its mandate in its carry.
    let first = task.context().is_some_and(|c| c.start == head);
    let mut values = Vec::new();
    let mut appends = Vec::new();
    let mut renders = Vec::new();
    if first && let Some(brief) = render::briefing(value, task) {
        appends.push(user(render::blocks(&brief), &mut values));
        renders.push(Span {
            from: Rendered::Note(Note::Brief),
            blocks: brief.len(),
        });
    }
    let forked = match task.origin {
        Origin::Fork { .. } | Origin::Aside { .. } if first => calls(),
        _ if unanswered => calls()
            .into_iter()
            .filter(|c| !task.pending.contains(&Input::Result(*c)))
            .collect(),
        _ => Vec::new(),
    };
    let time = local(now());
    let window = harness_provider::context_window(&model);
    let turn = render::turn(
        state,
        value,
        task,
        first,
        &forked,
        &render::Now {
            time: &time,
            window,
        },
    );
    if !turn.blocks.is_empty() {
        appends.push(user(render::blocks(&turn.blocks), &mut values));
    }
    renders.extend(turn.renders);
    // Max's effort, when it is not the role's, as parameters of
    // its own.
    let params = match task.tuning.effort {
        Some(effort) => {
            let named = value(params);
            match harness_provider::with_effort(&named, effort) {
                Some(tuned) if harness_provider::effort(&named) != Some(effort) => {
                    let tuned = Bytes::from(tuned);
                    let id = tuned.id();
                    values.push(tuned);
                    id
                }
                _ => params,
            }
        }
        None => params,
    };
    let context = state.context(head);
    let rooted = match &context[0].1 {
        Entry::Root(root) => root.model != model,
        Entry::Message { .. } => unreachable!("a context starts at a root"),
    };
    let mut call = ModelCall {
        model: rooted.then_some(model.clone()),
        task: task.id,
        base: head,
        appends,
        renders,
        params,
        breakpoints: Vec::new(),
    };
    let appended: Vec<EntryId> = call.entries().into_iter().map(|(e, _)| e).collect();
    let shared: Vec<EntryId> = [head]
        .into_iter()
        .chain(appended.iter().rev().skip(1).copied())
        .collect();
    let checkpoint = state.checkpoint(task);
    call.breakpoints = policy::breakpoints(context[0].0, checkpoint, &shared, call.head());
    Next {
        call,
        values,
        model,
    }
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

/// A time in the machine's own zone, as an agent is told it: `Sun 18 Oct
/// 2026 10:42 NZDT`.
fn local(at: Timestamp) -> String {
    let secs = (at.0 / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut out = [0u8; 64];
    // SAFETY: `tm` and `out` outlive the calls, and `out`'s length is passed.
    let n = unsafe {
        if libc::localtime_r(&secs, &mut tm).is_null() {
            return format!("{} ms after the epoch", at.0);
        }
        libc::strftime(
            out.as_mut_ptr().cast(),
            out.len(),
            c"%a %-d %b %Y %H:%M %Z".as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&out[..n]).into_owned()
}

pub fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

/// The executable the brain starts for a limb on a machine
/// ([`crate::process::program`]), by digest, as [`Launched::limbs`]'s
/// `limb`. A limb connected with other bytes is superseded
/// ([`harness_core::state::State::superseded`]).
fn limb_builds() -> BTreeMap<String, BuildId> {
    let limb = crate::process::program();
    match limb.and_then(std::fs::read) {
        Ok(bytes) => BTreeMap::from([("limb".to_owned(), BuildId::of(&bytes))]),
        Err(_) => BTreeMap::new(),
    }
}

/// A new clone for an agent started from a workspace at `dir`: cloned from
/// what `dir` itself was cloned from (or `dir`, if it has no `origin`), into
/// a sibling directory named for the repository and the branch, the way
/// Max names his (`agent-harness` on `dev-x` is `agent-harness-dev-x`).
/// A `dir` in no repository has nothing to clone: refused, saying so.
pub fn clone_of(dir: &Path, branch: &str) -> Result<(String, PathBuf), String> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    if git(&["rev-parse", "--git-dir"]).is_none() {
        return Err(format!(
            "`{}` is not in a git repository, so there is nothing to clone branch `{branch}` from: \
name a repository in `from`, or omit `branch` to work in that directory",
            dir.display()
        ));
    }
    let from =
        git(&["remote", "get-url", "origin"]).unwrap_or_else(|| dir.to_string_lossy().into_owned());
    let repo = from
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("clone")
        .trim_end_matches(".git");
    let name = format!("{repo}-{}", branch.replace('/', "-"));
    let parent = dir.parent().unwrap_or(dir);
    Ok((from, parent.join(name)))
}

/// One text block saying `said`: what a call the brain runs answers.
fn words(said: &str) -> Bytes {
    render::blocks(&[render::text(said)])
}

/// Settle a call the brain runs in `batch`: completed, saying what `said`
/// holds, or failed, saying why.
fn settle(id: CallId, said: Result<Bytes, String>, batch: &mut Batch) {
    let (model, outcome) = match said {
        Ok(model) => {
            let output = Output {
                model: model.id(),
                full: None,
                observes: Vec::new(),
            };
            (model, Outcome::Completed(output))
        }
        Err(why) => render::failed(&why),
    };
    batch.values.push(model);
    batch
        .facts
        .push(Fact::Call(id, CallEvent::Settled(outcome)));
}

/// The results of a task's round that have not reached it, but `except`'s:
/// what a compaction carries.
fn round(task: &Task, except: Option<CallId>) -> Vec<CallId> {
    task.pending
        .iter()
        .filter_map(|p| match *p {
            Input::Result(c) if Some(c) != except => Some(c),
            _ => None,
        })
        .collect()
}

/// The id of `limb`, stating it in `facts` if it is new or differs from
/// what the state holds of it.
fn define(state: &State, facts: &mut Vec<Fact>, limb: Limb) -> LimbId {
    if state.limb(limb.id) != Some(&limb) {
        facts.push(Fact::Limb(Defined::Limb(limb.clone())));
    }
    limb.id
}

/// The brain's own limb, with the tool sets it offers: `meta`, `web`, and
/// `roles` over the role files in `roles`.
fn brain_limb(id: LimbId, roles: Option<&Path>) -> Limb {
    let mut offers = vec![offer(meta::SET, "{}"), offer(web::SET, "{}")];
    offers.extend(roles.map(|dir| {
        roles::Settings {
            dir: dir.to_path_buf(),
        }
        .offer()
    }));
    Limb {
        id,
        name: "brain".into(),
        host: Host::Brain,
        offers,
    }
}

/// The limb on the brain's machine, as its user, named for the machine,
/// with the tool sets it offers and their settings: `workspace`; `user`
/// over Max's config directory, with the skills `skills.txt` beside the
/// role files names; `browser`, the one `$HARNESS_BROWSER` names (unset,
/// as `found` had it); and `builds`, relaunching the brain's own service.
fn local_limb(id: LimbId, found: Option<&Limb>, roles: Option<&Path>) -> Result<Limb, String> {
    let home = std::env::var("HOME").map_err(|e| format!("$HOME: {e}"))?;
    let mut user = serde_json::json!({ "dir": PathBuf::from(home).join(".claude") });
    if let Some(dir) = roles {
        user["skills"] = serde_json::json!(dir.join("skills.txt"));
    }
    let browser = match std::env::var("HARNESS_BROWSER") {
        Ok(word) => browser(&word).to_string(),
        Err(_) => (found.and_then(|l| l.offer("browser")))
            .map_or_else(|| "{}".to_owned(), |o| o.config.clone()),
    };
    let builds = match unit() {
        Some(unit) => serde_json::json!({ "unit": unit }).to_string(),
        None => "{}".to_owned(),
    };
    Ok(Limb {
        id,
        name: found.map_or_else(hostname, |l| l.name.clone()),
        host: Host::Local,
        offers: vec![
            offer(WORKSPACE, "{}"),
            offer("user", &user.to_string()),
            offer("browser", &browser),
            offer("builds", &builds),
        ],
    })
}

/// The `browser` set's settings from one word: an endpoint to attach to
/// (`host:port`, `http://…`, `ws://…`), or else the path of a browser to
/// launch. Empty for the limb's default.
fn browser(word: &str) -> serde_json::Value {
    let word = word.trim();
    let endpoint = word.starts_with("ws://")
        || word.starts_with("http://")
        || (word.rsplit_once(':')).is_some_and(|(_, port)| port.parse::<u16>().is_ok());
    match word {
        "" => serde_json::json!({}),
        _ if endpoint => serde_json::json!({ "endpoint": word }),
        _ => serde_json::json!({ "chrome": word }),
    }
}

fn offer(set: &str, config: &str) -> Offer {
    Offer {
        set: set.into(),
        config: config.into(),
    }
}

/// The brain's systemd user unit, `harness-NAME.service`: the unit whose
/// cgroup this process is in, if that is a brain's. `None` for a brain no
/// unit runs, such as a test brain, which cannot be relaunched.
fn unit() -> Option<String> {
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    cgroup
        .lines()
        .filter_map(|l| l.rsplit('/').next())
        .find(|u| u.starts_with("harness-") && u.ends_with(".service"))
        .map(String::from)
}

/// This machine's name, as the limb on it is called.
fn hostname() -> String {
    let mut name = [0u8; 256];
    // SAFETY: `name` outlives the call, and its length is passed.
    let failed = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } != 0;
    let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
    match String::from_utf8_lossy(&name[..end]).into_owned() {
        name if !failed && !name.is_empty() => name,
        _ => "local".into(),
    }
}
/// The environment variable that tells a brain the `relaunch` call that
/// started it, which the `builds` set puts in its unit's environment.
const RELAUNCHED_BY: &str = "HARNESS_RELAUNCHED_BY";

/// How this launch came: [`Launch::Relaunched`] after a brain that shut
/// down, or by the builds limb's `relaunch` call its unit's environment
/// names (forward, or back after the new build failed: then the brain
/// before was killed); [`Launch::Started`] after a crash, on a new
/// database, or after a build this one cannot read.
fn launch_how(state: &State) -> Launch {
    let by = std::env::var(RELAUNCHED_BY)
        .ok()
        .and_then(|call| call.parse().ok())
        .filter(|call| state.call(*call).is_some());
    match state.launch() {
        Some((_, Some(before))) if by.is_some() || state.shutdown().is_some() => {
            Launch::Relaunched {
                from: before.build,
                by,
            }
        }
        _ => Launch::Started,
    }
}
