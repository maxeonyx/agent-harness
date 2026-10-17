//! The browser limb: a real browser that agents drive with scripts, as
//! calls of the harness's core model.
//!
//! **Concept.** A browser is a [`Limb`] of the `browser` kind, shared by the
//! tasks whose environments hold it, as a workspace is shared by forks. It
//! drives one browser over the Chrome DevTools Protocol ([`cdp`]): one it
//! launches, with a profile of its own in the limb's state, or one it
//! attaches to at an endpoint, such as a dedicated Chrome on Max's desktop
//! through a tunnel ([`chrome`], [`Settings`]).
//!
//! A tab an agent opens is held by a running call: the `tab` tool, run in
//! the background, whose background name is the tab's name. So a tab is
//! owned as background work is ([`harness_core::call::Owner`]): it closes
//! when its owner stops the call, it keeps an agent from finishing until
//! it decides, and `pass` hands it, open, to the task that started its
//! owner or to Max. The brain says who owns each call in its row, so the
//! limb lets a script use only its own task's tabs and Max's: the tabs
//! passed to him, and those he opened himself, which no call holds and no
//! script can close. A tab is a resource of kind `tab`, named by its
//! target id, whose version is its URL: a result that read a tab observes
//! it, so Max navigating the tab is a notice at the agent's next turn.
//!
//! The tools read pages with scripts rather than pictures: `browser` runs
//! a script that drives tabs in a QuickJS sandbox ([`harness_script`]),
//! `page` runs JavaScript in one tab, and `screenshot` is the costly
//! exception ([`tools`]).
//!
//! It runs as a server of its own ([`harness_limb`]), `harness-limb-browser`.

mod cdp;
mod chrome;
pub mod tools;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output, Owner};
use harness_core::environment::{
    Description, Host, Instruction, Limb, LimbDown, LimbKey, LimbKind, LimbRow,
};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, Digest, LimbId, TaskId, Timestamp};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_limb::Out;
use harness_runtime::Cancel;
use harness_store::Store;
use harness_substrate::{Log, Mirror, Restored, Up};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::tools::{BROWSER, PAGE, SCREENSHOT, TAB};

/// The plugin kind this limb is.
pub const KIND: &str = "browser";

/// How long one DevTools command may take.
const COMMAND: Duration = Duration::from_secs(30);
/// How long a `browser` script may run.
const SCRIPT: Duration = Duration::from_secs(120);
/// The most of a result a context is shown.
const SHOWN: usize = 40_000;

/// A browser limb's own settings: the `config` of its [`Limb`] definition.
/// With neither `endpoint` nor `chrome`, it launches Playwright's
/// chrome-headless-shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// A browser to attach to rather than launch: its DevTools endpoint,
    /// `host:port` (as a tunnel exposes it) or a `ws://` URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// The browser to launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome: Option<PathBuf>,
    /// Launch it with `--headless=new`: for a full Chrome on a machine
    /// with no display.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub headless: bool,
}

impl Settings {
    /// Settings from one word: an endpoint (`host:port`, `http://…`,
    /// `ws://…`), or else the path of a browser to launch. Empty for the
    /// default.
    pub fn parse(word: &str) -> Settings {
        let word = word.trim();
        let endpoint = word.starts_with("ws://")
            || word.starts_with("http://")
            || word
                .rsplit_once(':')
                .is_some_and(|(_, port)| port.parse::<u16>().is_ok());
        match word {
            "" => Settings::default(),
            _ if endpoint => Settings {
                endpoint: Some(word.into()),
                ..Settings::default()
            },
            _ => Settings {
                chrome: Some(word.into()),
                ..Settings::default()
            },
        }
    }

    pub fn of(limb: &Limb) -> Option<Settings> {
        match &limb.kind {
            LimbKind::Plugin { kind, config, .. } if kind == KIND => {
                serde_json::from_str(config).ok()
            }
            _ => None,
        }
    }

    pub fn definition(&self, id: LimbId) -> Limb {
        Limb {
            id,
            name: KIND.into(),
            kind: LimbKind::Plugin {
                kind: KIND.into(),
                config: serde_json::to_string(self).expect("settings are JSON"),
                host: Host::Local,
            },
        }
    }

    /// Where the browser is, in words for the instructions.
    fn place(&self) -> String {
        match (&self.endpoint, &self.chrome) {
            (Some(endpoint), _) => {
                format!("a browser at {endpoint}, which Max can see and use while you work")
            }
            (None, _) => "a browser on this machine with no window: Max cannot see it".into(),
        }
    }
}

/// One running instance of the browser limb.
pub struct Browser {
    inner: Arc<Inner>,
    rows: Mutex<Mirror<LimbKey, LimbRow, Refusal>>,
}

struct Inner {
    limb: LimbId,
    /// Its definition's settings: the latest the brain shipped.
    settings: Mutex<Settings>,
    state: PathBuf,
    log: Mutex<Log<Harness, Store>>,
    out: Out,
    /// The connection to the browser, made again when it has closed.
    cdp: Mutex<Option<Arc<cdp::Cdp>>>,
    /// The browser it launched, if it did.
    process: Mutex<Option<Child>>,
    /// Where the browser's events go.
    events: Sender<cdp::Event>,
    /// Every page the browser has, by target id.
    targets: Mutex<HashMap<String, Target>>,
    /// The session each attached page's commands go to, by target id.
    sessions: Mutex<HashMap<String, String>>,
    /// Each session's count of loads, to wait for the next one.
    loads: Mutex<HashMap<String, u64>>,
    /// The tabs running `tab` calls hold, by call.
    held: Mutex<HashMap<CallId, Held>>,
    running: Mutex<HashMap<CallId, Running>>,
    /// For each task, the end of its latest call: its next call waits for
    /// it, so a response's tool uses run in the order it wrote them.
    latest: Mutex<HashMap<TaskId, Receiver<()>>>,
    watched: Mutex<HashMap<ResourceKey, Version>>,
    shutting_down: AtomicBool,
}

#[derive(Clone)]
struct Target {
    url: String,
    title: String,
}

/// A tab a running `tab` call holds.
#[derive(Clone)]
struct Held {
    /// Its background name.
    name: String,
    /// The task whose call opened it.
    task: TaskId,
    owner: Option<Owner>,
    /// Its target, once open.
    target: Option<String>,
}

struct Running {
    cancel: Cancel,
    thread: Option<JoinHandle<()>>,
}

impl harness_limb::Kind for Browser {
    fn open(
        definition: &Limb,
        log: Log<Harness, Store>,
        state: &Path,
        out: Out,
    ) -> Result<Browser, String> {
        let settings = Settings::of(definition).ok_or("the definition is not a browser's")?;
        let (events, inbox) = mpsc::channel();
        let inner = Arc::new(Inner {
            limb: definition.id,
            settings: Mutex::new(settings),
            state: state.to_path_buf(),
            log: Mutex::new(log),
            out,
            cdp: Mutex::default(),
            process: Mutex::default(),
            events,
            targets: Mutex::default(),
            sessions: Mutex::default(),
            loads: Mutex::default(),
            held: Mutex::default(),
            running: Mutex::default(),
            latest: Mutex::default(),
            watched: Mutex::default(),
            shutting_down: AtomicBool::new(false),
        });
        std::fs::create_dir_all(inner.marks()).map_err(|e| e.to_string())?;
        inner.connect()?;
        let weak = Arc::downgrade(&inner);
        thread::spawn(move || {
            for event in inbox {
                let Some(inner) = weak.upgrade() else { return };
                inner.event(event);
            }
        });
        Ok(Browser {
            inner,
            rows: Mutex::default(),
        })
    }

    fn apply(&self, down: LimbDown) -> Result<Vec<Record>, Restored> {
        let inner = &self.inner;
        let mut ship = Vec::new();
        let connected = matches!(down, LimbDown::View(_));
        let mut rows = lock(&self.rows);
        rows.apply(down);
        let Some(view) = rows.view() else {
            return Ok(ship);
        };
        {
            let mut log = lock(&inner.log);
            if connected {
                ship = log.unsent(&view.standing)?;
            }
            for (n, why) in &view.standing.refused {
                eprintln!("the brain refused this limb's record {n}: {why}");
            }
            if let Err(e) = log.cover(&view.standing) {
                eprintln!("cannot collect the log: {e}");
            }
        }
        if let Some(LimbRow::Definition(definition)) = view.rows.get(&LimbKey::Definition) {
            inner.define(definition);
        }
        if connected {
            inner.say(Batch {
                values: Vec::new(),
                facts: vec![Fact::Described {
                    limb: inner.limb,
                    description: inner.describe(),
                }],
            });
        }
        let here = lock(&inner.log).here();
        for (key, row) in &view.rows {
            match (key, row) {
                (
                    LimbKey::Call(id),
                    LimbRow::Call {
                        call,
                        stopping,
                        first_sent_to,
                        owner,
                    },
                ) => {
                    let input = match view.rows.get(&LimbKey::Value(call.input)) {
                        Some(LimbRow::Value(bytes)) => Some(bytes.clone()),
                        _ => None,
                    };
                    let earlier = *first_sent_to != here;
                    self.call(*id, call, input, *stopping, *owner, earlier);
                }
                (LimbKey::Watch, LimbRow::Watch { known }) => inner.watch(known),
                _ => {}
            }
        }
        // A call the brain no longer asks for has settled there.
        let running = lock(&inner.running);
        for call in inner.started() {
            if !running.contains_key(&call) && !view.rows.contains_key(&LimbKey::Call(call)) {
                let _ = std::fs::remove_file(inner.marks().join(call.to_string()));
            }
        }
        Ok(ship)
    }

    fn holds(&self) -> bool {
        // A browser it launched dies with it, and Max's tabs in it.
        let inner = &self.inner;
        !lock(&inner.running).is_empty()
            || !lock(&inner.held).is_empty()
            || lock(&inner.process).is_some()
    }

    fn stop_all(&self, within: Duration) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + within;
        for running in lock(&self.inner.running).values() {
            running.cancel.request(deadline);
        }
    }

    fn join(&self) {
        loop {
            let thread = lock(&self.inner.running)
                .values_mut()
                .find_map(|r| r.thread.take());
            let Some(thread) = thread else { break };
            let _ = thread.join();
        }
        if let Some(mut process) = lock(&self.inner.process).take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Browser {
    /// One call row: run it at most once, however often it arrives.
    fn call(
        &self,
        id: CallId,
        call: &Call,
        input: Option<Bytes>,
        stopping: Option<Timestamp>,
        owner: Option<Owner>,
        earlier: bool,
    ) {
        let inner = &self.inner;
        let mut running = lock(&inner.running);
        if let Some(held) = lock(&inner.held).get_mut(&id) {
            held.owner = owner;
        }
        if let Some(running) = running.get(&id) {
            if let Some(by) = stopping {
                running.cancel.request(instant(by));
            }
            return;
        }
        if inner.settled(id) {
            return;
        }
        let mark = inner.marks().join(id.to_string());
        let started = earlier || mark.exists();
        if stopping.is_some() {
            let outcome = if started {
                Outcome::Unknown
            } else {
                Outcome::Interrupted { partial: None }
            };
            return inner.settle(id, Report::default(), outcome);
        }
        if inner.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        if started && !tools::rerun(&call.tool) {
            return inner.settle(id, Report::default(), Outcome::Unknown);
        }
        let mut report = Report::default();
        let Some(input) = input else {
            let outcome = report.failed("the brain sent the call without its input");
            return inner.settle(id, report, outcome);
        };
        if let Err(e) = std::fs::write(&mark, []) {
            let outcome = report.failed(&format!("cannot record the call: {e}"));
            return inner.settle(id, report, outcome);
        }
        let cancel = Cancel::new();
        let (before, ended) = match &call.caller {
            Caller::Agent { task, .. } => {
                let (ended, end) = mpsc::channel::<()>();
                (lock(&inner.latest).insert(*task, end), Some(ended))
            }
            _ => (None, None),
        };
        let thread = {
            let inner = self.inner.clone();
            let cancel = cancel.clone();
            let call = call.clone();
            thread::spawn(move || {
                let mut report = Report::default();
                let outcome = match after(before, &cancel) {
                    true => inner.execute(id, &call, &input.0, owner, &cancel, &mut report),
                    false => Outcome::Interrupted { partial: None },
                };
                inner.settle(id, report, outcome);
                lock(&inner.held).remove(&id);
                drop(ended);
                lock(&inner.running).remove(&id);
            })
        };
        running.insert(
            id,
            Running {
                cancel,
                thread: Some(thread),
            },
        );
    }
}

/// Wait for the task's call before this one to end, unless this one is
/// asked to stop first. Whether to go on.
fn after(before: Option<Receiver<()>>, cancel: &Cancel) -> bool {
    let Some(before) = before else { return true };
    loop {
        if cancel.is_requested() {
            return false;
        }
        match before.recv_timeout(Duration::from_millis(100)) {
            Err(RecvTimeoutError::Timeout) => {}
            _ => return true,
        }
    }
}

/// Who a call acts for: which tabs it may use.
#[derive(Clone, Copy)]
enum Who {
    Task(TaskId),
    Max,
}

impl Inner {
    fn marks(&self) -> PathBuf {
        self.state.join("started")
    }

    fn started(&self) -> Vec<CallId> {
        std::fs::read_dir(self.marks())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Take a new version of the definition: a browser elsewhere is
    /// reached from the next call on, and the tabs in the old one are gone.
    fn define(&self, definition: &Limb) {
        let Some(settings) = Settings::of(definition) else {
            return;
        };
        let mut current = lock(&self.settings);
        if *current == settings {
            return;
        }
        eprintln!("the browser is now {}", settings.place());
        *current = settings;
        drop(current);
        *lock(&self.cdp) = None;
        if let Some(mut old) = lock(&self.process).take() {
            let _ = old.kill();
            let _ = old.wait();
        }
    }

    /// The browser's connection, made (and the browser found or launched)
    /// if there is none.
    fn connect(&self) -> Result<Arc<cdp::Cdp>, String> {
        let mut cdp = lock(&self.cdp);
        if let Some(cdp) = &*cdp {
            return Ok(cdp.clone());
        }
        let mut process = lock(&self.process);
        if let Some(mut old) = process.take() {
            let _ = old.kill();
            let _ = old.wait();
        }
        let browser = chrome::open(&lock(&self.settings), &self.state)?;
        *process = browser.process;
        let events = self.events.clone();
        let connection = Arc::new(cdp::Cdp::connect(&browser.url, move |e| {
            let _ = events.send(e);
        })?);
        lock(&self.targets).clear();
        lock(&self.sessions).clear();
        let targets = connection.call(None, "Target.getTargets", json!({}), COMMAND)?;
        for info in targets["targetInfos"].as_array().into_iter().flatten() {
            self.target_info(info);
        }
        connection.call(
            None,
            "Target.setDiscoverTargets",
            json!({ "discover": true }),
            COMMAND,
        )?;
        *cdp = Some(connection.clone());
        Ok(connection)
    }

    fn target_info(&self, info: &Json) {
        if info["type"] != "page" {
            return;
        }
        let Some(id) = info["targetId"].as_str() else {
            return;
        };
        let target = Target {
            url: info["url"].as_str().unwrap_or_default().into(),
            title: info["title"].as_str().unwrap_or_default().into(),
        };
        lock(&self.targets).insert(id.into(), target);
    }

    /// One event from the browser.
    fn event(&self, event: cdp::Event) {
        let changed = match event.method.as_str() {
            "Target.targetCreated" | "Target.targetInfoChanged" => {
                self.target_info(&event.params["targetInfo"]);
                event.params["targetInfo"]["targetId"]
                    .as_str()
                    .map(str::to_string)
            }
            "Target.targetDestroyed" => {
                let id = event.params["targetId"].as_str().unwrap_or_default();
                lock(&self.targets).remove(id);
                lock(&self.sessions).remove(id);
                Some(id.to_string())
            }
            "Target.detachedFromTarget" => {
                let session = event.params["sessionId"].as_str().unwrap_or_default();
                lock(&self.sessions).retain(|_, s| s != session);
                None
            }
            "Page.loadEventFired" => {
                if let Some(session) = event.session {
                    *lock(&self.loads).entry(session).or_default() += 1;
                }
                None
            }
            cdp::CLOSED => {
                eprintln!("the browser's connection closed");
                *lock(&self.cdp) = None;
                lock(&self.targets).clear();
                lock(&self.sessions).clear();
                None
            }
            _ => None,
        };
        if let Some(target) = changed {
            let key = self.key(&target);
            let mut watched = lock(&self.watched);
            if let Some(known) = watched.get_mut(&key) {
                let version = self.version_of(&target);
                if version != *known {
                    *known = version;
                    drop(watched);
                    self.say(Batch {
                        values: Vec::new(),
                        facts: vec![Fact::Change(Change {
                            resource: key,
                            version,
                            by: None,
                            content: None,
                        })],
                    });
                }
            }
        }
    }

    fn key(&self, target: &str) -> ResourceKey {
        ResourceKey {
            limb: self.limb,
            kind: TAB.into(),
            name: target.into(),
        }
    }

    fn version_of(&self, target: &str) -> Version {
        match lock(&self.targets).get(target) {
            Some(t) => Version::Is(Digest::of(t.url.as_bytes())),
            None => Version::Gone,
        }
    }

    fn observe(&self, target: &str) -> Observation {
        Observation {
            resource: self.key(target),
            version: self.version_of(target),
        }
    }

    fn watch(&self, known: &[Observation]) {
        let mut changes = Vec::new();
        let mut watched = lock(&self.watched);
        watched.retain(|resource, _| known.iter().any(|o| o.resource == *resource));
        for o in known {
            if o.resource.kind != TAB || watched.contains_key(&o.resource) {
                continue;
            }
            let version = self.version_of(&o.resource.name);
            watched.insert(o.resource.clone(), version);
            if known
                .iter()
                .all(|k| k.resource != o.resource || k.version != version)
            {
                changes.push(Fact::Change(Change {
                    resource: o.resource.clone(),
                    version,
                    by: None,
                    content: None,
                }));
            }
        }
        drop(watched);
        if !changes.is_empty() {
            self.say(Batch {
                values: Vec::new(),
                facts: changes,
            });
        }
    }

    fn describe(&self) -> Description {
        let text = tools::instructions(&lock(&self.settings).place());
        Description {
            instructions: vec![Instruction {
                observes: Observation {
                    resource: ResourceKey {
                        limb: self.limb,
                        kind: "instructions".into(),
                        name: String::new(),
                    },
                    version: Version::Is(Digest::of(text.as_bytes())),
                },
                text,
            }],
            tools: tools::tools(),
        }
    }

    fn say(&self, batch: Batch) {
        let record = lock(&self.log).say(now(), |_| batch);
        match record {
            Ok(record) => (self.out)(Up::Record(record)),
            Err(e) => eprintln!("cannot write the limb's log: {e}"),
        }
    }

    fn settled(&self, call: CallId) -> bool {
        let log = lock(&self.log);
        log.records().any(|r| match &r.what {
            harness_substrate::What::Said(batch) => batch
                .facts
                .iter()
                .any(|f| matches!(f, Fact::Call(id, CallEvent::Settled(_)) if *id == call)),
            _ => false,
        })
    }

    fn settle(&self, call: CallId, report: Report, outcome: Outcome) {
        self.say(Batch {
            values: report.values,
            facts: vec![Fact::Call(call, CallEvent::Settled(outcome))],
        });
    }

    /// Run one call to its outcome.
    fn execute(
        self: &Arc<Self>,
        id: CallId,
        call: &Call,
        input: &[u8],
        owner: Option<Owner>,
        cancel: &Cancel,
        report: &mut Report,
    ) -> Outcome {
        let input: Json = match serde_json::from_slice(input) {
            Ok(input) => input,
            Err(e) => return report.failed(&format!("the input is not JSON: {e}")),
        };
        let who = match call.caller {
            Caller::Agent { task, .. } | Caller::Background { task, .. } => Who::Task(task),
            Caller::Max { .. } => Who::Max,
            Caller::Meta { .. } => return report.failed("the browser runs no meta call"),
        };
        let mut touched = Vec::new();
        let done = match call.tool.as_str() {
            TAB => return self.tab(id, call, &input, owner, cancel, report),
            BROWSER => self.browser(who, &input, cancel, &mut touched),
            PAGE => self.page(who, &input, &mut touched).map(|v| shown(&v)),
            SCREENSHOT => {
                return match self.screenshot(who, &input, &mut touched) {
                    Ok(jpeg) => {
                        let content = json!([{ "type": "image", "source": {
                            "type": "base64", "media_type": "image/jpeg", "data": jpeg } }]);
                        Outcome::Completed(Output {
                            model: report.value(content.to_string().into_bytes()),
                            full: None,
                            observes: touched.iter().map(|t| self.observe(t)).collect(),
                        })
                    }
                    Err(e) => report.failed(&e),
                };
            }
            other => Err(format!("the browser has no tool `{other}`")),
        };
        let observes = touched.iter().map(|t| self.observe(t)).collect();
        match done {
            Ok(text) => report.completed(&text, observes),
            Err(e) => report.failed(&e),
        }
    }

    /// A `tab` call: open a tab and hold it until the call is stopped or
    /// the tab is closed.
    fn tab(
        &self,
        id: CallId,
        call: &Call,
        input: &Json,
        owner: Option<Owner>,
        cancel: &Cancel,
        report: &mut Report,
    ) -> Outcome {
        let Caller::Background { task, name, .. } = &call.caller else {
            return report.failed(
                "run `tab` with `background`: its name is the tab's name, and the tab stays open \
                 while it runs",
            );
        };
        let url = input["url"].as_str().unwrap_or("about:blank");
        let opened = self
            .connect()
            .and_then(|cdp| cdp.call(None, "Target.createTarget", json!({ "url": url }), COMMAND));
        let target = match opened {
            Ok(created) => created["targetId"].as_str().unwrap_or_default().to_string(),
            Err(e) => return report.failed(&format!("cannot open a tab: {e}")),
        };
        lock(&self.held).insert(
            id,
            Held {
                name: name.clone(),
                task: *task,
                owner,
                target: Some(target.clone()),
            },
        );
        let mut seen = Target {
            url: url.into(),
            title: String::new(),
        };
        loop {
            if cancel.is_requested() {
                if let Ok(cdp) = self.connect() {
                    let _ = cdp.call(
                        None,
                        "Target.closeTarget",
                        json!({ "targetId": target }),
                        COMMAND,
                    );
                }
                let text = format!("Closed the tab `{name}`, at {}.", seen.url);
                return Outcome::Interrupted {
                    partial: Some(Output {
                        model: report.text(&text),
                        full: None,
                        observes: Vec::new(),
                    }),
                };
            }
            match lock(&self.targets).get(&target) {
                Some(now) => seen = now.clone(),
                None if lock(&self.cdp).is_some() => {
                    let text = format!("The tab `{name}` was closed, at {}.", seen.url);
                    return report.completed(&text, Vec::new());
                }
                None => {
                    let text = format!("The browser went away with the tab `{name}` in it.");
                    return report.failed(&text);
                }
            }
            thread::sleep(Duration::from_millis(200));
        }
    }

    /// The tabs `who` may use: `(name, target, whose)`.
    fn tabs(&self, who: Who) -> Vec<(String, String, &'static str)> {
        let held = lock(&self.held);
        let targets = lock(&self.targets);
        let mut tabs = Vec::new();
        for h in held.values() {
            let Some(target) = &h.target else { continue };
            if !targets.contains_key(target) {
                continue;
            }
            let owner = h.owner.unwrap_or(Owner::Task(h.task));
            // A task may still use a tab it opened and passed on: its own
            // calls after the `pass` in one response run after it.
            let whose = match (who, owner) {
                (Who::Task(t), Owner::Task(o)) if t == o => "yours",
                (Who::Task(t), _) if t == h.task => "yours, passed on",
                (_, Owner::Max) => "Max's",
                (Who::Max, Owner::Task(_)) => "an agent's",
                (Who::Task(_), Owner::Task(_)) => continue,
            };
            tabs.push((h.name.clone(), target.clone(), whose));
        }
        for target in targets.keys() {
            if !held.values().any(|h| h.target.as_ref() == Some(target)) {
                tabs.push((
                    target[..8.min(target.len())].into(),
                    target.clone(),
                    "Max's",
                ));
            }
        }
        tabs.sort_by(|a, b| {
            let others = |t: &(String, String, &str)| !t.2.starts_with("yours");
            (others(a), &a.0).cmp(&(others(b), &b.0))
        });
        tabs
    }

    /// The target of the tab `name` that `who` may use: its own first.
    fn find(&self, who: Who, name: &str) -> Result<String, String> {
        let tabs = self.tabs(who);
        tabs.iter()
            .find(|t| t.0 == name)
            .or_else(|| {
                tabs.iter()
                    .find(|t| t.1.starts_with(name) && name.len() >= 4)
            })
            .map(|t| t.1.clone())
            .ok_or_else(|| {
                let names: Vec<_> = tabs.iter().map(|t| format!("`{}`", t.0)).collect();
                format!(
                    "no tab `{name}` you can use; yours and Max's: {}",
                    if names.is_empty() {
                        "none".into()
                    } else {
                        names.join(", ")
                    }
                )
            })
    }

    /// The session a tab's commands go to, attaching to it once.
    fn session(&self, cdp: &cdp::Cdp, target: &str) -> Result<String, String> {
        if let Some(session) = lock(&self.sessions).get(target) {
            return Ok(session.clone());
        }
        let attached = cdp.call(
            None,
            "Target.attachToTarget",
            json!({ "targetId": target, "flatten": true }),
            COMMAND,
        )?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or("the browser gave no session")?
            .to_string();
        cdp.call(Some(&session), "Page.enable", json!({}), COMMAND)?;
        lock(&self.sessions).insert(target.into(), session.clone());
        Ok(session)
    }

    /// Send a command to a tab.
    fn command(&self, target: &str, method: &str, params: Json) -> Result<Json, String> {
        let cdp = self.connect()?;
        let session = self.session(&cdp, target)?;
        cdp.call(Some(&session), method, params, COMMAND)
    }

    /// Evaluate `expression` in a tab, awaiting a promise, and return its
    /// value as JSON, or what it threw.
    fn evaluate(&self, target: &str, expression: &str) -> Result<Json, String> {
        let mut result = self.command(
            target,
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "awaitPromise": true,
                "returnByValue": true,
                "userGesture": true,
                "timeout": COMMAND.as_millis() as u64,
            }),
        )?;
        if let Some(details) = result.get("exceptionDetails") {
            let said = details["exception"]["description"]
                .as_str()
                .or(details["text"].as_str())
                .unwrap_or("the script threw");
            return Err(said.to_string());
        }
        Ok(result["result"]["value"].take())
    }

    fn page(&self, who: Who, input: &Json, touched: &mut Vec<String>) -> Result<Json, String> {
        let target = self.find(who, input["tab"].as_str().unwrap_or_default())?;
        touched.push(target.clone());
        let body = input["script"].as_str().unwrap_or_default();
        self.evaluate(&target, &format!("(async () => {{\n{body}\n}})()"))
    }

    fn screenshot(
        &self,
        who: Who,
        input: &Json,
        touched: &mut Vec<String>,
    ) -> Result<String, String> {
        let target = self.find(who, input["tab"].as_str().unwrap_or_default())?;
        touched.push(target.clone());
        let cdp = self.connect()?;
        // A tab in the background of a windowed browser paints nothing.
        let _ = cdp.call(
            None,
            "Target.activateTarget",
            json!({ "targetId": target }),
            COMMAND,
        );
        let metrics = self.command(&target, "Page.getLayoutMetrics", json!({}))?;
        let view = &metrics["cssVisualViewport"];
        let width = view["clientWidth"].as_f64().unwrap_or(1280.0);
        let height = view["clientHeight"].as_f64().unwrap_or(900.0);
        let shot = self.command(
            &target,
            "Page.captureScreenshot",
            json!({
                "format": "jpeg",
                "quality": 70,
                "clip": {
                    "x": view["pageX"].as_f64().unwrap_or(0.0),
                    "y": view["pageY"].as_f64().unwrap_or(0.0),
                    "width": width,
                    "height": height,
                    "scale": (1280.0 / width).min(1.0),
                },
            }),
        )?;
        shot["data"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "the browser returned no picture".into())
    }

    /// A `browser` call: the agent's script, with the tabs it may use.
    fn browser(
        self: &Arc<Self>,
        who: Who,
        input: &Json,
        cancel: &Cancel,
        touched: &mut Vec<String>,
    ) -> Result<String, String> {
        let body = input["script"].as_str().unwrap_or_default();
        let touched_ = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let this = self.clone();
        let target = {
            let touched = touched_.clone();
            let this = this.clone();
            move |args: &[Json]| -> Result<String, String> {
                let name = args.first().and_then(Json::as_str).unwrap_or_default();
                let target = this.find(who, name)?;
                touched.borrow_mut().push(target.clone());
                Ok(target)
            }
        };
        let target = Rc::new(target);
        let arg = |args: &[Json], i: usize| -> String {
            args.get(i)
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let stop = cancel.clone();
        let functions: Vec<(&str, harness_script::Host)> = vec![
            ("__tabs", {
                let this = this.clone();
                Rc::new(move |_: &[Json]| {
                    let targets = lock(&this.targets).clone();
                    Ok(json!(
                        this.tabs(who)
                            .into_iter()
                            .map(|(tab, target, whose)| {
                                let t = targets.get(&target);
                                json!({ "tab": tab, "whose": whose,
                        "url": t.map(|t| t.url.clone()), "title": t.map(|t| t.title.clone()) })
                            })
                            .collect::<Vec<_>>()
                    ))
                })
            }),
            ("__find", {
                let target = target.clone();
                Rc::new(move |args: &[Json]| {
                    target(args)?;
                    Ok(args[0].clone())
                })
            }),
            ("__goto", {
                let target = target.clone();
                let this = this.clone();
                let cancel = cancel.clone();
                Rc::new(move |args: &[Json]| this.goto(&target(args)?, &arg(args, 1), &cancel))
            }),
            ("__eval", {
                let target = target.clone();
                let this = this.clone();
                Rc::new(move |args: &[Json]| this.evaluate(&target(args)?, &arg(args, 1)))
            }),
            ("__click", {
                let target = target.clone();
                let this = this.clone();
                Rc::new(move |args: &[Json]| this.click(&target(args)?, &arg(args, 1)))
            }),
            ("__fill", {
                let target = target.clone();
                let this = this.clone();
                Rc::new(move |args: &[Json]| {
                    this.fill(&target(args)?, &arg(args, 1), &arg(args, 2))
                })
            }),
            ("__press", {
                let target = target.clone();
                let this = this.clone();
                Rc::new(move |args: &[Json]| this.press(&target(args)?, &arg(args, 1)))
            }),
            ("__wait", {
                let target = target.clone();
                let this = this.clone();
                let cancel = cancel.clone();
                Rc::new(move |args: &[Json]| {
                    let ms = args.get(2).and_then(Json::as_u64).unwrap_or(10_000);
                    this.wait(
                        &target(args)?,
                        &arg(args, 1),
                        Duration::from_millis(ms),
                        &cancel,
                    )
                })
            }),
        ];
        let ran = harness_script::run(
            tools::PRELUDE,
            body,
            &functions,
            Instant::now() + SCRIPT,
            move || stop.is_requested(),
        );
        drop(functions);
        touched.extend(touched_.take());
        touched.sort();
        touched.dedup();
        let printed = ran.printed.join("\n");
        match ran.value {
            Ok(value) => {
                let mut text = shown(&value);
                if !printed.is_empty() {
                    text = format!("{printed}\n\nReturned: {text}");
                }
                Ok(text)
            }
            Err(e) if printed.is_empty() => Err(e),
            Err(e) => Err(format!("{printed}\n\n{e}")),
        }
    }

    fn goto(&self, target: &str, url: &str, cancel: &Cancel) -> Result<Json, String> {
        let cdp = self.connect()?;
        let session = self.session(&cdp, target)?;
        let before = lock(&self.loads).get(&session).copied().unwrap_or(0);
        let went = cdp.call(
            Some(&session),
            "Page.navigate",
            json!({ "url": url }),
            COMMAND,
        )?;
        if let Some(error) = went["errorText"].as_str() {
            return Err(format!("{url}: {error}"));
        }
        // A navigation within the document loads nothing.
        if went["loaderId"].is_null() {
            return Ok(Json::Null);
        }
        let deadline = Instant::now() + COMMAND;
        while lock(&self.loads).get(&session).copied().unwrap_or(0) == before {
            if cancel.is_requested() {
                return Err("stopped".into());
            }
            if Instant::now() >= deadline {
                return Err(format!("{url} did not finish loading within 30s"));
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(Json::Null)
    }

    fn click(&self, target: &str, selector: &str) -> Result<Json, String> {
        let at = self.evaluate(target, &tools::centre_script(selector))?;
        let (x, y) = (at["x"].as_f64(), at["y"].as_f64());
        for kind in ["mouseMoved", "mousePressed", "mouseReleased"] {
            self.command(
                target,
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )?;
        }
        Ok(Json::Null)
    }

    fn fill(&self, target: &str, selector: &str, text: &str) -> Result<Json, String> {
        if self.evaluate(target, &tools::fill_script(selector, text))? == "select" {
            return Ok(Json::Null);
        }
        if text.is_empty() {
            self.press(target, "Delete")?;
        } else {
            self.command(target, "Input.insertText", json!({ "text": text }))?;
        }
        Ok(Json::Null)
    }

    fn press(&self, target: &str, name: &str) -> Result<Json, String> {
        let (key, code, number, text) =
            tools::key(name).ok_or_else(|| format!("no key `{name}`"))?;
        let mut down = json!({ "type": "keyDown", "key": key, "code": code,
            "windowsVirtualKeyCode": number, "nativeVirtualKeyCode": number });
        if let Some(text) = text {
            down["text"] = json!(text);
        }
        self.command(target, "Input.dispatchKeyEvent", down)?;
        self.command(
            target,
            "Input.dispatchKeyEvent",
            json!({ "type": "keyUp", "key": key, "code": code,
                "windowsVirtualKeyCode": number, "nativeVirtualKeyCode": number }),
        )?;
        Ok(Json::Null)
    }

    fn wait(
        &self,
        target: &str,
        expression: &str,
        within: Duration,
        cancel: &Cancel,
    ) -> Result<Json, String> {
        let deadline = Instant::now() + within;
        loop {
            // A page between documents throws; that is not yet.
            if let Ok(Json::Bool(true)) = self.evaluate(target, &format!("!!({expression})")) {
                return Ok(Json::Null);
            }
            if cancel.is_requested() {
                return Err("stopped".into());
            }
            if Instant::now() >= deadline {
                return Err(format!("waited {}ms for {expression}", within.as_millis()));
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// A value as a context is shown it: JSON, a string as its text, cut to
/// [`SHOWN`].
fn shown(value: &Json) -> String {
    let text = match value {
        Json::String(s) => s.clone(),
        Json::Null => "null".into(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    if text.len() <= SHOWN {
        return text;
    }
    let mut end = SHOWN;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n… cut: {} more characters. Return less: select what you need in the script.",
        &text[..end],
        text.len() - end
    )
}

/// The bytes one call says: the values its outcome names.
#[derive(Default)]
struct Report {
    values: Vec<Bytes>,
}

impl Report {
    fn value(&mut self, bytes: Vec<u8>) -> harness_core::id::ValueId {
        let bytes = Bytes(bytes);
        let id = bytes.id();
        self.values.push(bytes);
        id
    }

    fn text(&mut self, text: &str) -> harness_core::id::ValueId {
        self.value(
            json!([{ "type": "text", "text": text }])
                .to_string()
                .into_bytes(),
        )
    }

    fn completed(&mut self, said: &str, observes: Vec<Observation>) -> Outcome {
        Outcome::Completed(Output {
            model: self.text(said),
            full: None,
            observes,
        })
    }

    fn failed(&mut self, message: &str) -> Outcome {
        Outcome::Failed(Output {
            model: self.text(message),
            full: None,
            observes: Vec::new(),
        })
    }
}

fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

fn instant(at: Timestamp) -> Instant {
    Instant::now() + Duration::from_millis(at.0.saturating_sub(now().0).max(0) as u64)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
