//! The loop: act on each item of the to-do list, commit what happened,
//! repeat.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::build::{BuildId, Launch, Launched};
use harness_core::call::{Call, CallEvent, Caller, Outcome, Output, Owner};
use harness_core::context::{Entry, EntryId, Message, Note, Rendered, Role, Span};
use harness_core::delivery::{Content, Delivery, Prompt, Read};
use harness_core::environment::{Description, Hello, Host, LimbKey, LimbKind, LimbRow, Reach};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{
    CallId, DeliveryId, LimbId, ModelCallId, ReplicaId, TaskId, Time, Timestamp, ToolUseId, ValueId,
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
use harness_limb_workspace::{Clone, Settings, user};
use harness_substrate::{Error, Publisher, Up, View, What as Said};

use crate::link::{self, Conn, Wire};
use crate::process::Process;
use harness_provider::{Client, Finished, Progress};
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
pub struct Event(What);

enum What {
    Link(Conn, link::Said),
    FirstByte(ModelCallId),
    Finished(ModelCallId, Result<Finished, harness_provider::Error>),
    /// A reading of the plan's usage, for Max's call.
    Usage(CallId, Result<BTreeMap<String, Window>, String>),
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
    meta: LimbId,
    /// Max's limb, in a journal that has one: described as offering
    /// nothing ([`LimbKind::Max`]).
    max: Option<LimbId>,
    /// The provider: web search and fetch, and Max's usage reading.
    web: LimbId,
    /// The roles limb, co-hosted.
    roles: Option<LimbId>,
    /// What its roles' models resolve against.
    models: roles::Models,
    /// The links to plugin limbs this brain is making or has, by id,
    /// until their `attach` exits.
    limbs: HashMap<LimbId, Process>,
    /// When the brain last failed to reach a limb.
    failed: HashMap<LimbId, Instant>,
    /// The limbs whose link ended while they were superseded: their
    /// ending is a move to the build the brain starts, not a failure.
    moving: HashSet<LimbId>,
    /// The limbs connected now.
    links: HashMap<LimbId, Link>,
    /// The next number for a process link, counting up from 1.
    attached: Conn,
    /// The next number for a co-hosted link, counting down from the top.
    hosted: Conn,
    /// The model and context of each request without a first byte yet:
    /// fan-out gating.
    cold: HashMap<ModelCallId, (String, std::collections::HashSet<EntryId>)>,
    /// Each request in flight: the thread that sends it, joined when it
    /// reports how it ended, and its stop.
    requests: HashMap<ModelCallId, (Cancel, JoinHandle<()>)>,
    /// The first of Max's tasks this launch found unfinished: the one
    /// whose result it prints.
    root: Option<TaskId>,
    started: Instant,
    reported: bool,
}

impl Brain {
    /// Fold what the database holds, define the brain's own limbs if it is
    /// new (the roles limb over `roles`), and listen for limbs. What the
    /// last brain left in flight is on the to-do list.
    pub fn launch(
        store: Store,
        face: Sender<Folded>,
        client: Client,
        models: roles::Models,
        roles: Option<&Path>,
        Inbox { events, inbox }: Inbox,
        feed: Feed,
    ) -> Result<Brain, String> {
        let (decider, divergences) = schema::Brain::open(store.replica(), store)
            .map_err(|e| format!("the database folds: {e}"))?;
        for d in &divergences {
            errln!("harness-brain: skipping the record at {}: {}", d.at, d.why);
        }
        let state = decider.state();
        let find =
            |is: &dyn Fn(&LimbKind) -> bool| state.limbs().find(|l| is(&l.kind)).map(|l| l.id);
        let meta = find(&|k| *k == LimbKind::Meta);
        // A journal from before Max stopped being a limb with a tool.
        let max = find(&|k| *k == LimbKind::Max);
        let web = find(&|k| *k == LimbKind::Provider);
        let user_limb = find(&|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == user::KIND));
        let browser_limb = find(
            &|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == harness_limb_browser::KIND),
        );
        let builds_limb = find(
            &|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == harness_limb_builds::KIND),
        );
        let roles_limb =
            find(&|k| matches!(k, LimbKind::Plugin { kind, .. } if kind == roles::KIND));
        let at = decider.next();
        let exe = std::env::current_exe()
            .and_then(std::fs::read)
            .map_err(|e| format!("the brain's own executable: {e}"))?;
        let mut facts = vec![Fact::Launched(Launched {
            build: BuildId::of(&exe),
            source: crate::SOURCE.parse().ok(),
            limbs: limb_builds(),
            how: launch_how(state),
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
        // The harness's browser: `$HARNESS_BROWSER` names one to attach to
        // or launch, as the limb's settings say (empty for Playwright's
        // headless shell), and a new value is a new version of the limb.
        // Unset, the definition stays as it is.
        let wanted = std::env::var("HARNESS_BROWSER")
            .ok()
            .map(|word| harness_limb_browser::Settings::parse(&word));
        if let Some(id) = browser_limb
            && let Some(wanted) = &wanted
            && state
                .limb(id)
                .and_then(harness_limb_browser::Settings::of)
                .as_ref()
                != Some(wanted)
        {
            let limb = state.limb(id).expect("found among the limbs");
            facts.push(Fact::Limb(reconfigured(limb, wanted)));
        }
        let browser = define(&mut facts, browser_limb, &|id| {
            wanted.clone().unwrap_or_default().definition(id)
        });
        // The builds limb relaunches the brain's own service: the unit
        // this process runs in, which a new version follows.
        let unit = harness_limb_builds::Settings::here();
        if let Some(id) = builds_limb
            && state
                .limb(id)
                .and_then(harness_limb_builds::Settings::of)
                .as_ref()
                != Some(&unit)
        {
            let limb = state.limb(id).expect("found among the limbs");
            facts.push(Fact::Limb(reconfigured(limb, &unit)));
        }
        let builds = define(&mut facts, builds_limb, &|id| unit.definition(id));
        let shared: std::collections::BTreeMap<String, LimbId> = [
            ("meta", meta),
            ("user", user),
            ("web", web),
            ("browser", browser),
            ("builds", builds),
        ]
        .into_iter()
        .map(|(n, l)| (n.to_string(), l))
        .collect();
        let roles = match (roles_limb, roles_dir) {
            // A roles limb defined before a shared limb was is told of it,
            // and `--roles` moves it to another directory.
            (Some(id), dir) => {
                let limb = state.limb(id).expect("found among the limbs");
                if let Some(settings) = roles::Settings::of(limb) {
                    let mut limbs = settings.limbs.clone();
                    limbs.extend(shared);
                    let dir = dir.unwrap_or_else(|| settings.dir.clone());
                    let wanted = roles::Settings { dir, limbs };
                    if wanted != settings {
                        facts.push(Fact::Limb(reconfigured(limb, &wanted)));
                    }
                }
                Some(id)
            }
            (None, Some(dir)) => {
                let settings = roles::Settings { dir, limbs: shared };
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
            models,
            limbs: HashMap::new(),
            failed: HashMap::new(),
            moving: HashSet::new(),
            links: HashMap::new(),
            attached: 1,
            hosted: Conn::MAX,
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
        brain.host_roles();
        Ok(brain)
    }

    /// Start the roles limb on a thread of its own, linked by a channel.
    fn host_roles(&mut self) {
        let Some(id) = self.roles else {
            return errln!("harness-brain: no roles limb: a new database needs --roles");
        };
        let limb = self.state().limb(id).expect("the roles limb").clone();
        let models = self.models.clone();
        let (wire, down) = mpsc::channel();
        let conn = self.hosted;
        self.hosted -= 1;
        let replica = mint_replica();
        let events = self.events.clone();
        let build = self.state().launch().map(|(_, launched)| launched.build);
        let hello = Hello {
            limb: id,
            replica,
            build,
        };
        self.joined(conn, hello, wire);
        thread::spawn(move || {
            let up = |up| {
                let said = link::Said::Up(Box::new(up));
                let _ = events.send(Event(What::Link(conn, said)));
            };
            roles::run(&limb, replica, down, up, &models);
            let _ = events.send(Event(What::Link(conn, link::Said::Left)));
        });
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
            Todo::Cut(request) => {
                if let Some((cancel, _)) = self.requests.get(&request) {
                    cancel.now();
                }
            }
            Todo::Abandon(task) => self.stop_member(task),
            Todo::Tidy(task) => self.prompt(task, Prompt::Tidy),
            Todo::Expire(task) => self.fact(Fact::Task(task, TaskEvent::Expired)),
            Todo::Settle(call) => self.settle_scope(call),
            Todo::Linger(task) => self.prompt(task, Prompt::Lingering),
            Todo::Park(task) => self.prompt(task, Prompt::Park),
            Todo::Resume(task) => self.prompt(task, Prompt::Resume),
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

    /// Attach to a plugin limb, starting it if it is not running: it says
    /// hello once it is ready, which may take a while (a clone to make).
    /// Not while a link to it is made or being made, nor soon after the
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
        let definition = self.state().limb(limb).expect("a limb to connect").clone();
        match &definition.kind {
            LimbKind::Plugin {
                host: Host::Local, ..
            } => {}
            LimbKind::Plugin {
                host: Host::Ssh { destination },
                ..
            } => {
                let why = format!("on {destination}: limbs on other machines are not built yet");
                return self.unreached(limb, why);
            }
            // Hosted in the brain's own process at launch.
            _ => return,
        }
        let conn = self.attached;
        self.attached += 1;
        let events = self.events.clone();
        let said = move |conn, said| {
            let _ = events.send(Event(What::Link(conn, said)));
        };
        match Process::attach(&definition, conn, said) {
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
        if self.state().limb(limb).is_none_or(|l| l.kind.is_brains()) {
            return errln!("a link said it is limb {limb}, which is no plugin limb here");
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

    /// What a limb tells a root of `task`: the meta limb is described per
    /// environment, a plugin as it last said.
    fn description(&self, task: &Task, limb: LimbId) -> Option<Description> {
        if Some(limb) == self.max {
            return Some(Description {
                instructions: Vec::new(),
                tools: Vec::new(),
            });
        }
        if limb == self.web {
            return Some(web::description());
        }
        if limb != self.meta {
            return self.state().described(limb).map(|d| d.description.clone());
        }
        let state = self.state();
        let env = state.environment(task.environment)?;
        let roles: Vec<(&str, &str)> = env
            .starts
            .iter()
            .filter_map(|e| state.environment(*e))
            .map(|e| (e.name.as_str(), e.description.as_str()))
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
        Some(
            render::root(
                harness_provider::IDENTITY,
                task.tuning.model(env),
                &role,
                &refs,
            )
            .map(|new| {
                let start = TaskStart {
                    root: new.root,
                    binding: new.binding,
                    observes: env.source.iter().cloned().chain(new.observes).collect(),
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
            let ask = meta::compaction(&record.tool, &self.value(record.input)).ok()?;
            Some((call, ask))
        });
        let Some((call, ask)) = asked else {
            return false;
        };
        let checkpoint = self.state().checkpoint(task).filter(|_| !ask.fresh);
        let (onto, values, why) = match checkpoint {
            Some(at) => (Onto::Checkpoint(at), Vec::new(), render::Why::Checkpoint),
            None => match self.start_of(task.id) {
                None => return true,
                Some(Err(why)) => {
                    self.fact(Fact::Task(task.id, TaskEvent::Drain(Drain::Fail(why))));
                    return true;
                }
                Some(Ok((start, values))) => {
                    (Onto::Fresh(Box::new(start)), values, render::Why::Fresh)
                }
            },
        };
        let calls = round(task, Some(call));
        let carry = render::Carry {
            summary: Some(&ask.summary),
            why,
            words: &[],
            calls: &calls,
        };
        let report = ask.report.map(|r| render::blocks(&[render::text(&r)]));
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
            )) => {
                let reason = "the model declined to go on".to_string();
                return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
            }
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
                    let reason = reason.to_string();
                    return self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(reason))));
                }
                return match self.start_of(id) {
                    None => {}
                    Some(Err(why)) => self.fact(Fact::Task(id, TaskEvent::Drain(Drain::Fail(why)))),
                    Some(Ok((start, values))) => {
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
                };
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
                        let _ = events.send(Event(What::FirstByte(request)));
                    }
                    Progress::Text(chunk) => feed.push(Live::Model {
                        call: request,
                        chunk,
                    }),
                },
            );
            let _ = events.send(Event(What::Finished(request, finished)));
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
                    expects: meta::expects(context, context.binding.limb(&tool)),
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
        if limb == self.web {
            return self.read_usage(id, &call);
        }
        // The limb runs it from its row.
        self.fact(Fact::Call(id, CallEvent::Dispatched));
    }

    /// Run Max's call to the provider off the loop's thread, the way a
    /// request runs, and settle it when it reports. It only reads, so it is
    /// not stopped at shutdown: a later brain runs it again.
    fn read_usage(&mut self, id: CallId, call: &Call) {
        if call.tool != web::USAGE {
            let said = format!("The provider offers Max no tool `{}`.", call.tool);
            return self.fail(id, &said);
        }
        self.fact(Fact::Call(id, CallEvent::Dispatched));
        let client = self.client.clone();
        let events = self.events.clone();
        thread::spawn(move || {
            let reading = client.usage(&Cancel::new());
            let _ = events.send(Event(What::Usage(id, reading)));
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
            Err(why) => self.fail(id, &format!("Could not read the plan's usage: {why}")),
        }
    }

    /// Run a meta call: its members are created in the same batch as its
    /// dispatch, or it fails at once with nothing created. An `add_sibling`
    /// settles in that batch too.
    fn spawn(&mut self, id: CallId, call: &Call) {
        // The state lets Max call any tool of a limb the brain runs, and
        // the meta limb offers him none.
        let Caller::Agent { task: parent, .. } = call.caller else {
            let said = format!("The harness offers Max no tool `{}`.", call.tool);
            return self.fail(id, &said);
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
            let (model, failed) = render::failed(
                "You are being stopped: nothing new can start, and there is no next context. \
Finish tidying up and end your turn with your report.",
            );
            batch.values.push(model);
            batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
            return self.answer(id, batch);
        }
        let input = self.value(call.input);
        // A compaction happens once its round is over ([`Brain::compact`]).
        if meta::compacts(&call.tool) {
            let asked = if call.tool == meta::COMPACT_AND_CONTINUE
                && meta::repeats(self.state(), &parent)
            {
                Err("you compacted last and have made no other call since, so there is nothing to replace. \
Carry on with your next step, or, if your work is done, call `compact_and_report`"
                    .to_string())
            } else {
                meta::compaction(&call.tool, &input).map(|_| ())
            };
            let said = match asked {
                Ok(()) => "Compacting once this round is over.".to_string(),
                Err(why) => {
                    let (model, failed) = render::failed(&format!("Not compacted: {why}."));
                    batch.values.push(model);
                    batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
                    return self.answer(id, batch);
                }
            };
            let said = render::blocks(&[render::text(&said)]);
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
            return self.answer(id, batch);
        }
        if call.tool == meta::ATTACH {
            let said = meta::attach(self.state(), &|v| self.value(v), &parent, &input);
            let (model, outcome) = match said {
                Ok(said) => {
                    let output = Output {
                        model: said.id(),
                        full: None,
                        observes: Vec::new(),
                    };
                    (said, Outcome::Completed(output))
                }
                Err(message) => render::failed(&message),
            };
            batch.values.push(model);
            batch
                .facts
                .push(Fact::Call(id, CallEvent::Settled(outcome)));
            return self.answer(id, batch);
        }
        if call.tool == meta::BACKGROUND {
            match meta::background_call(self.state(), &parent, id, self.meta, &input) {
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
                    return self.answer(id, batch);
                }
                Err(message) => {
                    let (model, failed) = render::failed(&message);
                    batch.values.push(model);
                    batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
                    return self.answer(id, batch);
                }
            }
        }
        if call.tool == meta::STOP || call.tool == meta::PASS {
            let said = self.hand(&call.tool, &parent, &input, &mut batch);
            let (model, outcome) = match said {
                Ok(said) => {
                    let said = render::blocks(&[render::text(&said)]);
                    let output = Output {
                        model: said.id(),
                        full: None,
                        observes: Vec::new(),
                    };
                    (said, Outcome::Completed(output))
                }
                Err(message) => render::failed(&message),
            };
            batch.values.push(model);
            batch
                .facts
                .push(Fact::Call(id, CallEvent::Settled(outcome)));
            return self.answer(id, batch);
        }
        // A tool an older root declared that the harness no longer has.
        if call.tool != meta::SPAWN && call.tool != meta::ADD_SIBLING {
            let said = format!("The harness no longer has `{}`.", call.tool);
            let (model, failed) = render::failed(&said);
            batch.values.push(model);
            batch.facts.push(Fact::Call(id, CallEvent::Settled(failed)));
            return self.answer(id, batch);
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
                let (model, failed) = render::failed(&message);
                batch = Batch {
                    values: vec![model],
                    facts: vec![
                        Fact::Call(id, CallEvent::Dispatched),
                        Fact::Call(id, CallEvent::Settled(failed)),
                    ],
                };
            }
        }
        self.answer(id, batch);
    }

    /// Say the batch a meta call's dispatch makes. What an agent asks for
    /// may be what the state refuses: then the call fails with why, and
    /// nothing else is written.
    fn answer(&mut self, id: CallId, batch: Batch) {
        let Err(why) = self.say(batch) else {
            return;
        };
        let (model, failed) = render::failed(&format!(
            "Nothing was done: the harness refuses it ({why})."
        ));
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
                        let own = parent
                            .places
                            .get(kind)
                            .and_then(|l| state.limb(*l))
                            .map(|l| PathBuf::from(&l.name));
                        let dir = match (own, &from) {
                            (Some(own), Some(from)) => own.join(from),
                            (own, from) => own
                                .or(from.as_ref().map(PathBuf::from))
                                .ok_or(format!("you have no {kind} to clone from"))?,
                        };
                        let (from, dir) = clone_of(&dir, &branch);
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
    fn fail(&mut self, id: CallId, said: &str) {
        let (model, failed) = render::failed(said);
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
            What::Usage(call, reading) => self.read(call, reading),
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
                Up::Record(record) => self.heard(conn, record),
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
        if let Err(why) = self.say(batch) {
            panic!("the brain refuses its own batch (refused: {why}): {shown}");
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
                .is_some_and(|l| l != self.meta)
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
    let env = state
        .environment(task.environment)
        .expect("the task's environment");
    let model = task.tuning.model(env).to_owned();
    let params = env.params;
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
    // Max's effort, when it is not the environment's, as parameters of
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

/// The executable the brain starts for each kind of plugin limb: each
/// `harness-limb-<kind>` beside its own, by digest. A limb connected with
/// other bytes is superseded ([`harness_core::state::State::superseded`]).
fn limb_builds() -> BTreeMap<String, BuildId> {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    else {
        return BTreeMap::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return BTreeMap::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            // Not cargo's `.d` files beside them in a target directory.
            let kind = name
                .strip_prefix("harness-limb-")
                .filter(|k| !k.contains('.'))?;
            let kind = kind.to_owned();
            let bytes = std::fs::read(entry.path()).ok()?;
            Some((kind, BuildId::of(&bytes)))
        })
        .collect()
}

/// A new replica, for a face or a co-hosted limb this launch: 64 random bits.
fn mint_replica() -> ReplicaId {
    let mut bytes = [0; 8];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .expect("/dev/urandom reads");
    ReplicaId(u64::from_le_bytes(bytes))
}

/// A new clone for an agent started from a workspace at `dir`: cloned from
/// what `dir` itself was cloned from (or `dir`, if it has no `origin`), into
/// a sibling directory named for the repository and the branch, the way
/// Max names his (`agent-harness` on `dev-x` is `agent-harness-dev-x`).
pub fn clone_of(dir: &Path, branch: &str) -> (String, PathBuf) {
    let from = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    let repo = from
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("clone")
        .trim_end_matches(".git");
    let name = format!("{repo}-{}", branch.replace('/', "-"));
    let parent = dir.parent().unwrap_or(dir);
    (from, parent.join(name))
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

/// A new version of `limb` with `settings` as its config: the same kind on
/// the same host as the version recorded, which a journal from before
/// hosts were recorded reads as `Local`.
fn reconfigured(
    limb: &harness_core::environment::Limb,
    settings: &impl serde::Serialize,
) -> harness_core::environment::Limb {
    let mut limb = limb.clone();
    if let LimbKind::Plugin { config, .. } = &mut limb.kind {
        *config = serde_json::to_string(settings).expect("settings are JSON");
    }
    limb
}

/// How this launch came: [`Launch::Relaunched`] after a brain that shut
/// down, or by the builds limb's `relaunch` call its unit's environment
/// names (forward, or back after the new build failed: then the brain
/// before was killed); [`Launch::Started`] after a crash or on a new
/// database.
fn launch_how(state: &State) -> Launch {
    let by = std::env::var(harness_limb_builds::RELAUNCHED_BY)
        .ok()
        .and_then(|call| call.parse().ok())
        .filter(|call| state.call(*call).is_some());
    match state.launch() {
        Some((_, before)) if by.is_some() || state.shutdown().is_some() => Launch::Relaunched {
            from: before.build,
            by,
        },
        _ => Launch::Started,
    }
}
