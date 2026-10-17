//! The loop: act on each item of the to-do list, commit what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::context::{Entry, EntryId, Message, Note, Rendered, Role, Span};
use harness_core::delivery::{Content, Delivery, Prompt, Read};
use harness_core::environment::{Description, Hello, LimbKey, LimbKind, LimbRow, Reach};
use harness_core::fact::{Batch, Fact, Launched, Refusal};
use harness_core::id::{
    CallId, DeliveryId, LimbId, ModelCallId, ReplicaId, TaskId, Time, Timestamp, ToolUseId, ValueId,
};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Stop};
use harness_core::resource::Observation;
use harness_core::schema::{self, Record};
use harness_core::state::{State, Todo};
use harness_core::task::{
    Context, Created, Drain, Draining, Input, Mandate, Mode, Move, Origin, Outcome as TaskOutcome,
    Scope, Start as TaskStart, Stopper, Task, TaskEvent,
};
use harness_core::value::Bytes;
use harness_face_web::{Feed, Folded, Running};
use harness_limb_workspace::{Clone, Settings, user};
use harness_substrate::{Publisher, Up, View, What as Said};

use crate::link::{self, Conn, Wire};
use crate::process::Process;
use harness_provider::{Client, Finished, Progress};
use harness_runtime::{Cancel, signals};
use harness_store::Store;

use crate::meta::{self, How};
use crate::{max, web};
use crate::{policy, render, roles};

/// How long the loop sleeps when nothing arrives: the resolution of backoff
/// and fan-out timing.
const TICK: Duration = Duration::from_millis(250);

/// How long after launch the brain waits for the limbs an earlier brain
/// left running to reconnect, before starting its own.
const REATTACH: Duration = Duration::from_secs(2);

/// How long the brain waits before starting again a limb whose process
/// ended without connecting.
const RESTART: Duration = Duration::from_secs(30);

/// What reaches the loop from the threads it started, from its limbs'
/// links, and from faces.
pub struct Event(What);

enum What {
    Link(Conn, link::Said),
    FirstByte(ModelCallId),
    Finished(ModelCallId, Result<Finished, harness_provider::Error>),
    /// A signal asking the brain's process to stop.
    Signal(libc::c_int),
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
            let _ = events.send(Event(What::Tab(Box::new(record))));
        })
    }

    /// Have `SIGINT`, `SIGTERM` and `SIGHUP` reach the brain as events: the
    /// first shuts it down gracefully, the next at once.
    pub fn signals(&self) -> Result<(), String> {
        let events = self.events.clone();
        let stops = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];
        signals::forward(&stops, move |s| {
            let _ = events.send(Event(What::Signal(s)));
        })
        .map_err(|e| format!("signals: {e}"))
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

/// One limb's link, while it is connected.
struct Link {
    conn: Conn,
    replica: ReplicaId,
    wire: Wire,
    publisher: Publisher<LimbKey, LimbRow, Refusal>,
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
    meta: LimbId,
    /// Max's limb: his answers to `ask`.
    max: LimbId,
    /// The provider's web search and fetch.
    web: LimbId,
    /// The roles limb, co-hosted.
    roles: Option<LimbId>,
    /// The plugin limbs this brain has started, by id, until they exit.
    limbs: HashMap<LimbId, Process>,
    /// When a limb's process last ended without connecting.
    failed: HashMap<LimbId, Instant>,
    /// The limbs connected now.
    links: HashMap<LimbId, Link>,
    /// The next number for a co-hosted link: process links count up from
    /// 1, these down from the top.
    hosted: Conn,
    /// Where each limb keeps its log: `<state>/<limb>`, and the socket.
    state_dir: PathBuf,
    socket: PathBuf,
    /// The context of each request without a first byte yet: fan-out gating.
    cold: HashMap<ModelCallId, std::collections::HashSet<EntryId>>,
    /// Each request in flight: the thread that sends it, joined when it
    /// reports how it ended, and its stop.
    requests: HashMap<ModelCallId, (Cancel, JoinHandle<()>)>,
    /// The task this launch started, or else the first of Max's it found
    /// unfinished: the one whose result it prints.
    root: Option<TaskId>,
    started: Instant,
    reported: bool,
}

impl Brain {
    /// Fold what the database holds, define the brain's own limbs if it is
    /// new (the roles limb over `roles`), listen for limbs, and start the
    /// command line's task. What the last brain left in flight is on the
    /// to-do list.
    #[allow(clippy::too_many_arguments)]
    pub fn launch(
        store: Store,
        face: Sender<Folded>,
        client: Client,
        state_dir: PathBuf,
        roles: Option<&Path>,
        start: Option<Start>,
        Inbox { events, inbox }: Inbox,
        feed: Feed,
    ) -> Result<Brain, String> {
        let (decider, divergences) = schema::Brain::open(store.replica(), store)
            .map_err(|e| format!("the database folds: {e}"))?;
        for d in &divergences {
            eprintln!(
                "harness-brain: the record at {} folds differently in this build, and is skipped: {}",
                d.at, d.why
            );
        }
        let state = decider.state();
        let find =
            |is: &dyn Fn(&LimbKind) -> bool| state.limbs().find(|l| is(&l.kind)).map(|l| l.id);
        let meta = find(&|k| *k == LimbKind::Meta);
        let max = find(&|k| *k == LimbKind::Max);
        let web = find(&|k| *k == LimbKind::Provider);
        let user_limb = find(&|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == user::KIND));
        let roles_limb =
            find(&|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == roles::KIND));
        std::fs::create_dir_all(&state_dir).map_err(|e| format!("{state_dir:?}: {e}"))?;
        let socket = state_dir.join("limbs.sock");
        let at = decider.next();
        let mut facts = vec![Fact::Launched(Launched {
            build: concat!("harness-brain ", env!("CARGO_PKG_VERSION")).into(),
            by: None,
        })];
        let define =
            |facts: &mut Vec<Fact>,
             found: Option<LimbId>,
             limb: &dyn Fn(LimbId) -> harness_core::environment::Limb| {
                found.unwrap_or_else(|| {
                    let id = LimbId(at.event(facts.len() as u32));
                    facts.push(Fact::Limb(limb(id)));
                    id
                })
            };
        let meta = define(&mut facts, meta, &meta::definition);
        let max = define(&mut facts, max, &max::definition);
        let web = define(&mut facts, web, &web::definition);
        let roles_dir = roles
            .map(|dir| {
                dir.canonicalize()
                    .map_err(|e| format!("the roles {dir:?}: {e}"))
            })
            .transpose()?;
        let home = std::env::var("HOME").map_err(|e| format!("$HOME: {e}"))?;
        let config = PathBuf::from(home).join(".claude");
        // The skills it offers are named in `skills.txt` beside the role
        // files, which the limb reads.
        let skills = roles_dir.as_ref().map(|dir| dir.join("skills.txt"));
        let user = define(&mut facts, user_limb, &|id| {
            Settings::user(id, config.clone(), skills.clone())
        });
        let roles = match (roles_limb, roles_dir) {
            (Some(id), _) => Some(id),
            (None, Some(dir)) => {
                let settings = roles::Settings {
                    dir,
                    limbs: [("meta", meta), ("max", max), ("user", user), ("web", web)]
                        .into_iter()
                        .map(|(n, l)| (n.to_string(), l))
                        .collect(),
                };
                Some(define(&mut facts, None, &|id| settings.definition(id)))
            }
            (None, None) => None,
        };
        let mut brain = Brain {
            decider,
            face,
            client: Arc::new(client),
            events,
            inbox,
            feed,
            meta,
            max,
            web,
            roles,
            limbs: HashMap::new(),
            failed: HashMap::new(),
            links: HashMap::new(),
            hosted: Conn::MAX,
            state_dir,
            socket,
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
        let events = brain.events.clone();
        link::listen(&brain.socket, move |conn, said| {
            let _ = events.send(Event(What::Link(conn, said)));
        })
        .map_err(|e| format!("the limbs' socket {:?}: {e}", brain.socket))?;
        brain.host_roles();
        brain.root = match start {
            Some(start) => {
                brain.await_role(&start.role);
                Some(brain.start(start)?)
            }
            None => brain
                .state()
                .tasks()
                .find(|t| t.scope == Scope::Max && !t.is_done())
                .map(|t| t.id),
        };
        Ok(brain)
    }

    /// Start the roles limb on a thread of its own, linked by a channel.
    fn host_roles(&mut self) {
        let Some(id) = self.roles else {
            return eprintln!("harness-brain: no roles limb: a new database needs --roles");
        };
        let limb = self.state().limb(id).expect("the roles limb").clone();
        let (wire, down) = mpsc::channel();
        let conn = self.hosted;
        self.hosted -= 1;
        let replica = mint_replica();
        let events = self.events.clone();
        self.joined(conn, Hello { limb: id, replica }, Wire::Channel(wire));
        thread::spawn(move || {
            let up = |up| {
                let said = link::Said::Up(Box::new(up));
                let _ = events.send(Event(What::Link(conn, said)));
            };
            roles::run(&limb, replica, down, up);
            let _ = events.send(Event(What::Link(conn, link::Said::Left)));
        });
    }

    /// Take in what arrives until the roles limb has stated `role`, for a
    /// while: on a new database, it has only just started reading.
    fn await_role(&mut self, role: &str) {
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until && !self.state().environments().any(|e| e.name == role) {
            if let Ok(event) = self.inbox.recv_timeout(TICK) {
                self.handle(event);
            }
        }
    }

    /// Start a task in Max's scope, as Max: through the command line, a
    /// face with no page, which is a replica of its own.
    fn start(&mut self, start: Start) -> Result<TaskId, String> {
        let env = self
            .state()
            .environments()
            .find(|e| e.name == start.role)
            .ok_or(format!(
                "no role `{}` (the roles limb states them once it has read its files)",
                start.role
            ))?
            .clone();
        let dir = start
            .workspace
            .canonicalize()
            .map_err(|e| format!("the workspace {:?}: {e}", start.workspace))?;
        // The workspace limbs are the brain's to define, in its own record.
        let at = self.decider.next();
        let mut defined = Vec::new();
        let mut places = BTreeMap::new();
        for kind in &env.places {
            let limb = self.workspace_limb(&dir, None, at, &mut defined);
            places.insert(kind.clone(), limb);
        }
        let face = mint_replica();
        defined.push(Fact::Face(face));
        self.append(Batch {
            values: Vec::new(),
            facts: defined,
        });
        let mandate = Bytes::from(start.mandate);
        let said = Time {
            replica: face,
            n: 1,
        };
        let id = TaskId(said.event(0));
        let facts = vec![Fact::Task(
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
        )];
        let record = Record {
            at: said,
            clock: now(),
            what: Said::Said(Batch {
                values: vec![mandate],
                facts,
            }),
        };
        self.include(record);
        match self.decider.standing(face).refused.get(&1) {
            Some(why) => Err(format!("the task is refused: {why}")),
            None => Ok(id),
        }
    }

    /// The workspace limb for `dir`, which is its name: the one already
    /// defined, or a new one whose definition goes in `facts`, to be said
    /// in the brain's record at `at`, made by cloning if `clone`.
    fn workspace_limb(
        &self,
        dir: &Path,
        clone: Option<Clone>,
        at: Time,
        facts: &mut Vec<Fact>,
    ) -> LimbId {
        let name = dir.to_string_lossy();
        let new = facts.iter().find_map(|f| match f {
            Fact::Limb(l) if l.name == name => Some(l.id),
            _ => None,
        });
        if let Some(id) = new.or_else(|| self.state().limb_named(&name).map(|l| l.id)) {
            return id;
        }
        let id = LimbId(at.event(facts.len() as u32));
        let settings = Settings {
            dir: dir.to_path_buf(),
            clone,
            skills: None,
        };
        facts.push(Fact::Limb(settings.definition(id, &name)));
        id
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
                self.fail(call, &said);
            }
            Todo::Interrupt(call) => self.interrupt(call),
            Todo::Abandon(request) => {
                if let Some((cancel, _)) = self.requests.get(&request) {
                    cancel.now();
                }
            }
            Todo::Stop(task) => self.stop_member(task),
            Todo::Tidy(task) => self.prompt(task, Prompt::Stopping),
            Todo::Expire(task) => self.fact(Fact::Task(task, TaskEvent::Expired)),
            Todo::Settle(call) => self.settle_scope(call),
            Todo::Linger(task) => self.prompt(task, Prompt::Lingering),
            Todo::Unblock(aside) => self.prompt(aside, Prompt::Unblocked),
        }
    }

    /// Ask a task for a turn.
    fn prompt(&mut self, to: TaskId, prompt: Prompt) {
        self.fact(Fact::Delivery(Delivery {
            id: DeliveryId(self.decider.next().event(0)),
            to,
            content: Content::Prompt(prompt),
            read: Read::Next,
        }));
    }

    /// Stop a member of a scope whose owner is stopping, with three quarters
    /// of the time the owner has left, so the owner has the rest for its
    /// own tidy-up turn after its members' reports are in.
    fn stop_member(&mut self, task: TaskId) {
        let state = self.state();
        let owner = state.owner(task).and_then(|o| state.task(o));
        let within = match owner.and_then(Task::drain) {
            Some(Draining::Stopping(s)) if s.hard => 0,
            Some(Draining::Stopping(s)) => policy::share(now(), s.deadline),
            _ => policy::ms(policy::MEMBER_STOP),
        };
        let stop = Drain::Stop {
            by: Stopper::Owner,
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
            eprintln!("harness-brain: {name} again: stopping now");
        } else {
            eprintln!(
                "harness-brain: {name}: shutting down; requests in flight have {}s to finish (again to stop now)",
                within.as_secs()
            );
        }
        // The limbs see it in their rows, and stop by its deadline.
        self.fact(Fact::Shutdown {
            within: policy::ms(within),
        });
    }

    /// Whether the brain has shut down: no request, limb or link is left.
    /// At its deadline, and the grace after it, what is left is killed,
    /// and a request that will not end is left to the process's exit.
    fn down(&mut self) -> bool {
        let Some(deadline) = self.state().shutdown() else {
            return false;
        };
        let killed = now().0 >= deadline.0 + policy::ms(policy::HARD_WAIT) as i64;
        if killed {
            for (limb, mut process) in self.limbs.drain() {
                eprintln!("harness-brain: limb {limb} did not exit in time: killed");
                process.kill();
            }
        }
        killed || (self.requests.is_empty() && self.limbs.is_empty() && self.links.is_empty())
    }

    // Limbs.

    /// Start a plugin limb's process, which connects back when it is ready:
    /// that may take a while (a clone to make). Not while one is running,
    /// nor just after launch, while the limbs an earlier brain left running
    /// reconnect, nor soon after its process ended without connecting.
    fn connect(&mut self, limb: LimbId) {
        let recently = |at: &Instant| at.elapsed() < RESTART;
        if self.links.contains_key(&limb)
            || self.limbs.contains_key(&limb)
            || self.started.elapsed() < REATTACH
            || self.failed.get(&limb).is_some_and(recently)
            || Some(limb) == self.roles
        {
            return;
        }
        let definition = self.state().limb(limb).expect("a limb to connect");
        match Process::start(
            definition,
            &self.state_dir.join(limb.to_string()),
            &self.socket,
        ) {
            Ok(process) => {
                self.limbs.insert(limb, process);
            }
            Err(e) => {
                eprintln!("limb {limb}: its process cannot start: {e}");
                self.failed.insert(limb, Instant::now());
            }
        }
    }

    /// Let go of the limb processes that have exited.
    fn reap(&mut self) {
        let exited: Vec<LimbId> = self
            .limbs
            .iter_mut()
            .filter_map(|(id, p)| p.exited().then_some(*id))
            .collect();
        for limb in exited {
            self.limbs.remove(&limb);
            if !self.links.contains_key(&limb) && self.state().shutdown().is_none() {
                eprintln!("limb {limb}: its process ended without connecting (its error is above)");
                self.failed.insert(limb, Instant::now());
            }
        }
    }

    /// A limb connected as `hello.replica`: its records are the limb's from
    /// now on, and it is shipped a whole view.
    fn joined(&mut self, conn: Conn, hello: Hello, wire: Wire) {
        let Hello { limb, replica } = hello;
        if self.state().limb(limb).is_none_or(|l| l.kind.is_brains()) {
            return eprintln!("a link said it is limb {limb}, which is no plugin limb here");
        }
        self.links.insert(
            limb,
            Link {
                conn,
                replica,
                wire,
                publisher: Publisher::default(),
            },
        );
        if self.state().connected(limb).map(|(p, _)| p) == Some(replica) {
            return self.publish();
        }
        self.fact(Fact::Reach {
            limb,
            reach: Reach::Connected { instance: replica },
        });
    }

    /// A link ended. If it was its limb's current one, the limb is lost
    /// until it connects again.
    fn left(&mut self, conn: Conn) {
        let Some(limb) = self
            .links
            .iter()
            .find_map(|(l, link)| (link.conn == conn).then_some(*l))
        else {
            return;
        };
        self.links.remove(&limb);
        eprintln!("limb {limb}: its link ended");
        self.fact(Fact::Reach {
            limb,
            reach: Reach::Lost,
        });
    }

    /// What a limb tells a root of `task`: the meta limb is described per
    /// environment, a plugin as it last said.
    fn description(&self, task: &Task, limb: LimbId) -> Option<Description> {
        if limb == self.max {
            return Some(max::description(limb));
        }
        if limb == self.web {
            return Some(web::description());
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

    /// Say a move to a new root. Its limbs watch what it embodies through
    /// their rows.
    fn moved(&mut self, id: TaskId, movement: Move, values: Vec<Bytes>) {
        self.append(Batch {
            values,
            facts: vec![Fact::Task(id, TaskEvent::Moved(movement))],
        });
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
        // A stopping task's tidy-up turn is its last: there is no next
        // context to hand over to.
        let ending = task.ends();
        if !ending && self.handover(&task) {
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
                if ending {
                    let reason = "its context filled during its tidy-up turn".to_string();
                    return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
                }
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
            let brief = render::brief(said.as_deref(), &quotes);
            appends.push(user(render::blocks(&brief), &mut values));
            renders.push(Span {
                from: Rendered::Note(Note::Brief),
                blocks: brief.len(),
            });
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
            base.iter().map(|(id, _)| *id).chain(appended).collect(),
        );
        let cancel = Cancel::new();
        let client = self.client.clone();
        let events = self.events.clone();
        let feed = self.feed.clone();
        let stop = cancel.clone();
        let thread = thread::spawn(move || {
            let base: Vec<(EntryId, &Entry)> = base.iter().map(|(id, e)| (*id, e)).collect();
            let finished = client.send(
                request,
                &call,
                &base,
                |id| bytes.get(&id).cloned(),
                &stop,
                |progress| match progress {
                    Progress::FirstByte => {
                        let _ = events.send(Event(What::FirstByte(request)));
                    }
                    Progress::Live(live) => feed.push(live),
                },
            );
            let _ = events.send(Event(What::Finished(request, finished)));
        });
        self.requests.insert(request, (cancel, thread));
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
        let at = self.decider.next();
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
                let id = CallId(at.event(batch.facts.len() as u32));
                batch.facts.push(Fact::Call(id, CallEvent::Made(call)));
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
        // The limb runs it from its row.
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
        // Only a stopping task's tidy-up turn makes calls while it ends.
        if parent.ends() {
            let (model, failed) = failed(
                "You are being stopped: nothing new can start, and there is no next context. \
Finish tidying up and end your turn with your report.",
            );
            batch.values.push(model);
            batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
            return self.append(batch);
        }
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
                    let made = CallId(self.decider.next().event(batch.facts.len() as u32));
                    batch.facts.extend([
                        Fact::Call(made, CallEvent::Made(started)),
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
        let at = self.decider.next();
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
                        let limb = self.workspace_limb(&dir, Some(clone), at, &mut batch.facts);
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
            let id = TaskId(at.event(batch.facts.len() as u32));
            batch
                .facts
                .push(Fact::Task(id, TaskEvent::Created(Box::new(created))));
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
                    Some(TaskOutcome::Cancelled { by, report }) => self.cancelled(*by, *report),
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

    /// What a cancelled member says to the task that reads its result.
    fn cancelled(&self, by: Stopper, report: Option<EntryId>) -> String {
        let by = match by {
            Stopper::Max => "Cancelled by Max",
            Stopper::Owner => "Cancelled, because you are being stopped",
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
            What::Signal(signal) => self.signal(signal),
            What::Tab(record) => {
                // A tab new to the brain is one of Max's faces.
                let replica = record.at.replica;
                if !self.state().is_face(replica) {
                    self.fact(Fact::Face(replica));
                }
                self.include(*record);
            }
            What::Link(conn, link::Said::Joined(hello, wire)) => self.joined(conn, hello, wire),
            What::Link(conn, link::Said::Up(up)) => match *up {
                // Only the current link speaks for its limb.
                Up::Record(record) if self.links.values().any(|l| l.conn == conn) => {
                    self.include(record)
                }
                Up::Record(_) => {}
                Up::Live(live) => self.feed.push(live.live),
            },
            What::Link(conn, link::Said::Left) => self.left(conn),
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
        let started: Vec<Running> = batch.facts.iter().filter_map(starts).collect();
        for running in &started {
            self.feed.start(*running);
        }
        let facts = batch.facts.clone();
        let at = match self.decider.say(now(), |_| batch) {
            Ok(at) => at,
            Err(e) => panic!("the brain refuses its own batch ({e}): {shown}"),
        };
        self.changed(at, &facts);
    }

    /// Include a record from a limb or a tab. A refusal is the replica's to
    /// see, in its standing; it is not a bug.
    fn include(&mut self, record: Record) {
        let from = record.at;
        let facts = match &record.what {
            Said::Said(batch) => batch.facts.clone(),
            Said::Included(_) => Vec::new(),
        };
        match self.decider.include(now(), record) {
            Ok(None) => {}
            Ok(Some(_)) => match self.decider.standing(from.replica).refused.get(&from.n) {
                Some(why) => {
                    println!("{:>7.1}s {from}  refused: {why}", self.elapsed());
                    self.publish();
                }
                None => self.changed(from, &facts),
            },
            Err(e) => eprintln!("harness-brain: cannot store the record at {from}: {e}"),
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
            if let Some(down) = link.publisher.publish(view) {
                link.wire.ship(down);
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
                .is_some_and(|l| l != self.meta)
        {
            let input = String::from_utf8_lossy(&self.value(call.input))
                .chars()
                .take(200)
                .collect::<String>();
            line = format!("{line} {input}");
        }
        println!("{:>7.1}s {at}  {line}", self.elapsed());
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
            TaskOutcome::Cancelled { by, report } => self.cancelled(*by, *report),
            other => format!("{other:?}"),
        };
        println!("\n== the task is done; its result:\n\n{said}\n");
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

pub fn now() -> Timestamp {
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

/// A new replica, for a face or a co-hosted limb this launch: 64 random bits.
fn mint_replica() -> ReplicaId {
    let mut bytes = [0; 8];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .expect("/dev/urandom reads");
    ReplicaId(u64::from_le_bytes(bytes))
}
