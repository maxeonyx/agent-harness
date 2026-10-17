//! The workspace limb: a shell and file tools in one directory, a follower
//! of the brain on the substrate's link.
//!
//! **Concept.** A workspace is a [`Limb`] that owns a directory on a
//! machine. It runs the coding tools an engineer needs (`bash`, `read`,
//! `write`, `edit`, `grep`; see [`tools`]) as [`harness_core::call::Call`]s,
//! and its files are resources of kind `file`: a read's result
//! observes the version it returned, a write's the version it produced, and
//! a write or edit whose caller has not seen the current version is
//! [refused](Outcome::Refused) before it touches anything. A caller who has
//! never seen an existing file is treated as having seen it absent, so
//! nothing is overwritten blind. Forks share their source's workspace; the
//! notices and refusals are how siblings coordinate in it.
//!
//! Everything it says is records of its own place, about itself, its own
//! calls and its files, kept in its own [`Log`] until the brain has
//! included them: its [`Description`] on each connection; a settlement,
//! with the bytes of its output, and a [`Change`] naming the call for each
//! file the call wrote, in one batch. What it hears is the brain's rows for
//! it ([`LimbRow`]): the calls to run, the resources to watch, and when to
//! shut down.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. An environment either
//! holds this limb or does not.
//!
//! The brain runs it as a process of its own, `harness-limb-workspace` or
//! `harness-limb-user` ([`process`]).
//!
//! A call is run on its own thread, marked on disk as started before it has
//! any effect, and settled in the log before it stops running. So a call
//! row runs at most once, however often a crash of the brain or of the limb
//! makes it arrive: it is left to the settlement the log holds, left
//! running, run for the first time, rerun if its tool allows
//! ([`tools::rerun`]), or settled as [`Outcome::Unknown`] when the instance
//! running it was cut off. Never a failure that did not happen.
//!
//! **Vision.** No permission prompts; process trees owned by their call;
//! interrupted calls whose outcome is unknown kept distinct from failed
//! ones (critique §5); "trunc (head + tail + grep for avoiding context
//! overload) [...] use it by default for commands", with the agent required
//! to name what it greps for ([`trunc`]); the shared-workspace race.
//!
//! **Existing harnesses.** OpenCode cuts tool output at 2000 lines or
//! 50 KiB and writes the rest to a file
//! (`ref-opencode/packages/core/src/tool-output.ts:13`); Codex cuts the
//! middle of a string to a byte or token budget
//! (`ref-codex/codex-rs/utils/string/src/truncate.rs:36`) and, like this
//! limb, kills a command's process group
//! (`ref-codex/codex-rs/core/src/exec.rs:59`).
//!
//! **Different here.** The agent chooses what survives truncation; the
//! stale-write check costs the model nothing, because the brain sends what
//! the context has observed; and a limb that outlives the brain can still
//! say how a call ended.

mod files;
mod git;
pub mod process;
mod record;
mod shell;
pub mod tools;
pub mod trunc;
pub mod user;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Outcome, Output};
use harness_core::environment::{
    Description, Instruction, Limb, LimbDown, LimbKey, LimbKind, LimbRow, LimbUp,
};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, Digest, LimbId, PlaceId, Timestamp, ValueId};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_store::Store;
use harness_substrate::{Log, Replica, Restored, Up};
use serde::{Deserialize, Serialize};
use serde_json::json;

use harness_runtime::Cancel;

use crate::record::Marks;

/// The resource kinds this limb owns.
const FILE: &str = "file";
const INSTRUCTIONS: &str = "instructions";

/// The plugin kind this limb is, which an environment's place names.
pub const KIND: &str = "workspace";

/// How long its calls get to stop when the limb shuts down without being
/// told how long: its link gone for good, or it was signalled.
pub const SHUTDOWN: Duration = Duration::from_secs(5);

/// A workspace's own settings: the `config` of its [`Limb`] definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The directory the tools work in. Must exist, unless `clone` makes it.
    pub dir: PathBuf,
    /// Made before the limb first connects, if `dir` does not exist yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clone: Option<Clone>,
}

/// A clone of a repository, made for one agent: its own directory and its
/// own branch, which it commits and pushes to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clone {
    /// What `git clone` is given: a URL or a path.
    pub from: String,
    /// Checked out from `origin/<branch>` if it exists, otherwise made from
    /// `origin/<base>`.
    pub branch: String,
    pub base: String,
}

impl Settings {
    /// The settings a limb definition holds.
    pub fn of(limb: &Limb) -> Option<Settings> {
        match &limb.kind {
            LimbKind::Plugin { kind, config } if Kind::of(kind).is_some() => {
                serde_json::from_str(config).ok()
            }
            _ => None,
        }
    }

    /// The definition of a workspace limb, as the fact that declares it.
    pub fn definition(&self, id: LimbId, name: &str) -> Limb {
        self.limb(id, name, Kind::Workspace)
    }

    /// The definition of a user limb over Max's config directory `dir`.
    pub fn user(id: LimbId, dir: PathBuf) -> Limb {
        Settings { dir, clone: None }.limb(id, "user", Kind::User)
    }

    fn limb(&self, id: LimbId, name: &str, kind: Kind) -> Limb {
        Limb {
            id,
            name: name.into(),
            kind: LimbKind::Plugin {
                kind: kind.name().into(),
                config: serde_json::to_string(self).expect("settings are JSON"),
            },
        }
    }
}

/// The two plugin kinds this crate runs: one implementation over a
/// directory, with different tools and instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An agent's working directory, with the coding tools.
    Workspace,
    /// Max's own agent configuration ([`user`]).
    User,
}

impl Kind {
    pub fn of(name: &str) -> Option<Kind> {
        match name {
            KIND => Some(Kind::Workspace),
            user::KIND => Some(Kind::User),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Workspace => KIND,
            Kind::User => user::KIND,
        }
    }
}

/// Where a workspace runs and keeps its records.
pub struct Config {
    pub limb: LimbId,
    pub kind: Kind,
    pub settings: Settings,
    /// Where the limb keeps its log and its marks of calls. Outside the
    /// directory, so the agents' files stay theirs.
    pub state: PathBuf,
}

/// What it ships, in order: to the link while there is one.
pub type Out = Box<dyn Fn(LimbUp) + Send + Sync>;

/// One running instance of the workspace limb.
///
/// A follower of the brain: it says its records into its own [`Log`] and
/// ships each through `out`; the brain ships back its rows
/// ([`Workspace::apply`]). Dropping it stops every running call, within
/// [`SHUTDOWN`], and waits until each has settled.
pub struct Workspace {
    inner: Arc<Inner>,
    rows: Mutex<Replica<LimbKey, LimbRow, Refusal>>,
}

/// What applying a view asks of the link.
pub struct Applied {
    /// Ship these first: what the brain had not included when it connected.
    pub ship: Vec<Record>,
    /// The brain asks the limb to be gone by then.
    pub shutdown: Option<Timestamp>,
}

struct Inner {
    limb: LimbId,
    root: PathBuf,
    /// The clone it was made as, which its instructions say is the agent's
    /// own.
    clone: Option<Clone>,
    kind: Kind,
    marks: Marks,
    log: Mutex<Log<Harness, Store>>,
    out: Out,
    running: Mutex<HashMap<CallId, Running>>,
    /// Shutting down: no new call runs, and a stopped one says why.
    shutting_down: AtomicBool,
    /// The resources the brain asked to hear about, at the version last
    /// reported.
    watched: Mutex<HashMap<ResourceKey, Version>>,
    /// Held across a file tool's check and its write, so two calls cannot
    /// both pass the check on one version.
    files: Mutex<()>,
}

struct Running {
    cancel: Cancel,
    thread: Option<JoinHandle<()>>,
}

impl Workspace {
    /// Start an instance: make its clone if it has one to make, open its
    /// log, and end what an earlier instance left running. Blocks while it
    /// clones.
    pub fn open(config: Config, out: Out) -> io::Result<Workspace> {
        let Settings { dir, clone } = config.settings;
        if let Some(clone) = &clone {
            git::clone(clone, &dir)?;
        }
        std::fs::create_dir_all(&config.state)?;
        let store = Store::open(config.state.join("limb.sqlite")).map_err(io::Error::other)?;
        let log = Log::open(store.place(), store).map_err(io::Error::other)?;
        let inner = Inner {
            limb: config.limb,
            root: dir.canonicalize()?,
            clone,
            kind: config.kind,
            marks: Marks::open(config.state.join("calls"))?,
            log: Mutex::new(log),
            out,
            running: Mutex::default(),
            shutting_down: AtomicBool::new(false),
            watched: Mutex::default(),
            files: Mutex::default(),
        };
        // An instance that was killed mid-command left its processes
        // running, each carrying its call's mark.
        for call in inner.marks.orphans()? {
            let mark = shell::mark(call);
            let mut waiter = Cancel::new().waiter()?;
            let deadline = Instant::now() + SHUTDOWN;
            let members = || harness_runtime::marked(&mark);
            harness_runtime::end(&mut waiter, deadline, Some(libc::SIGTERM), members, || None)?;
            inner.marks.ran(call);
        }
        Ok(Workspace {
            inner: Arc::new(inner),
            rows: Mutex::default(),
        })
    }

    /// The place its records are of, the same across instances on one
    /// state directory.
    pub fn place(&self) -> PlaceId {
        lock(&self.inner.log).here()
    }

    pub fn limb(&self) -> LimbId {
        self.inner.limb
    }

    /// Take in what the brain shipped, and act on the rows: run each call
    /// it holds a row for and has no record of, stop those it asks to
    /// stop, watch what it asks. A whole view begins a connection: the
    /// limb ships what the brain has not included, then describes itself.
    /// Never blocks on a call.
    pub fn apply(&self, down: LimbDown) -> Result<Applied, Restored> {
        let inner = &self.inner;
        let mut ship = Vec::new();
        let connected = matches!(down, LimbDown::View(_));
        let mut replica = lock(&self.rows);
        replica.apply(down);
        let Some(view) = replica.view() else {
            return Ok(Applied {
                ship,
                shutdown: None,
            });
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
        if connected {
            inner.say(Batch {
                values: Vec::new(),
                facts: vec![Fact::Described {
                    limb: inner.limb,
                    description: inner.describe(),
                }],
            });
        }
        let mut shutdown = None;
        for (key, row) in &view.rows {
            match (key, row) {
                (LimbKey::Call(id), LimbRow::Call { call, stopping }) => {
                    let input = match &view.rows.get(&LimbKey::Value(call.input)) {
                        Some(LimbRow::Value(bytes)) => Some(bytes),
                        _ => None,
                    };
                    self.call(*id, call, input, *stopping);
                }
                (LimbKey::Watch, LimbRow::Watch { known }) => inner.watch(known),
                (LimbKey::Shutdown, LimbRow::Shutdown { by }) => shutdown = Some(*by),
                _ => {}
            }
        }
        // A call the brain no longer asks for has settled there.
        let running = lock(&inner.running);
        for call in inner.marks.started() {
            if !running.contains_key(&call) && !view.rows.contains_key(&LimbKey::Call(call)) {
                inner.marks.forget(call);
            }
        }
        Ok(Applied { ship, shutdown })
    }

    /// One call row: run it at most once, however often it arrives, and
    /// stop it when the row says so.
    fn call(&self, id: CallId, call: &Call, input: Option<&Bytes>, stopping: Option<Timestamp>) {
        let inner = &self.inner;
        let mut running = lock(&inner.running);
        if let Some(running) = running.get(&id) {
            if let Some(by) = stopping {
                running.cancel.request(instant(by));
            }
            return;
        }
        // Checked after `running`: a call's thread settles before it stops
        // running.
        if inner.settled(id) {
            return;
        }
        let started = inner.marks.is_started(id);
        if stopping.is_some() {
            let outcome = if started {
                Outcome::Unknown
            } else {
                Outcome::Interrupted { partial: None }
            };
            return inner.settle(Report::new(Some(id)), outcome);
        }
        // A limb shutting down starts nothing; the next instance runs it.
        if inner.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        // Cut off while an earlier instance ran it.
        if started && !tools::rerun(&call.tool) {
            return inner.settle(Report::new(Some(id)), Outcome::Unknown);
        }
        let Some(input) = input.cloned() else {
            let mut report = Report::new(Some(id));
            let outcome = report.failed("the brain sent the call without its input");
            return inner.settle(report, outcome);
        };
        if let Err(e) = inner.marks.start(id) {
            let mut report = Report::new(Some(id));
            let outcome = report.failed(&format!("cannot record the call: {e}"));
            return inner.settle(report, outcome);
        }
        let cancel = Cancel::new();
        let thread = {
            let inner = self.inner.clone();
            let cancel = cancel.clone();
            let call = call.clone();
            thread::spawn(move || {
                let mut report = Report::new(Some(id));
                let outcome = inner.execute(id, &call, &input.0, &cancel, &mut report);
                inner.rescan(&mut report, call.tool == "bash");
                inner.settle(report, outcome);
                let mut running = lock(&inner.running);
                running.remove(&id);
                // Orphans the limb adopted that have exited. Only while no
                // call runs, so no call's own command can be among them.
                if running.is_empty() {
                    harness_runtime::reap_children();
                }
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

    /// Stop every running call, each to be gone `within` from now, and run
    /// no new one: what shutting down asks of the calls. Asked again with
    /// less time, it makes each stop harder.
    pub fn stop_all(&self, within: Duration) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + within;
        for running in lock(&self.inner.running).values() {
            running.cancel.request(deadline);
        }
    }

    /// Wait until every call has settled and nothing it started is left.
    pub fn join(&self) {
        loop {
            let thread = lock(&self.inner.running)
                .values_mut()
                .find_map(|r| r.thread.take());
            let Some(thread) = thread else {
                return;
            };
            let _ = thread.join();
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.stop_all(SHUTDOWN);
        self.join();
    }
}

/// The wall clock, as records carry it.
pub fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

/// The moment a record's clock names.
pub fn instant(at: Timestamp) -> Instant {
    Instant::now() + Duration::from_millis(at.0.saturating_sub(now().0).max(0) as u64)
}

/// What one call, or one answer to a [`LimbRow::Watch`], says: the bytes its
/// output names and the files that changed.
pub(crate) struct Report {
    call: Option<CallId>,
    values: Vec<Bytes>,
    changes: Vec<Change>,
}

impl Report {
    fn new(call: Option<CallId>) -> Report {
        Report {
            call,
            values: Vec::new(),
            changes: Vec::new(),
        }
    }

    pub(crate) fn value(&mut self, bytes: Vec<u8>) -> ValueId {
        let bytes = Bytes(bytes);
        let id = bytes.id();
        self.values.push(bytes);
        id
    }

    /// A tool result's content: one text block.
    pub(crate) fn text(&mut self, text: &str) -> ValueId {
        self.value(
            json!([{ "type": "text", "text": text }])
                .to_string()
                .into_bytes(),
        )
    }

    pub(crate) fn completed(&mut self, said: &str, observes: Vec<Observation>) -> Outcome {
        Outcome::Completed(Output {
            model: self.text(said),
            full: None,
            observes,
        })
    }

    pub(crate) fn failed(&mut self, message: &str) -> Outcome {
        Outcome::Failed(Output {
            model: self.text(message),
            full: None,
            observes: Vec::new(),
        })
    }

    /// `resource` is now at `version`, by this report's call.
    fn changed(&mut self, resource: ResourceKey, version: Version) {
        self.changes.retain(|c| c.resource != resource);
        self.changes.push(Change {
            resource,
            version,
            by: self.call,
            content: None,
        });
    }

    /// The facts, led by the settlement when there is one.
    fn batch(self, settled: Vec<Fact>) -> Batch {
        Batch {
            values: self.values,
            facts: settled
                .into_iter()
                .chain(self.changes.into_iter().map(Fact::Change))
                .collect(),
        }
    }
}

impl Inner {
    /// Say a batch at this limb's place: stored, then shipped.
    fn say(&self, batch: Batch) {
        let record = lock(&self.log).say(now(), |_| batch);
        match record {
            Ok(record) => (self.out)(Up::Record(record)),
            // A call left unsettled is found started and cut off by the
            // next instance, which is still true.
            Err(e) => eprintln!("cannot write the limb's log: {e}"),
        }
    }

    /// Ship what a running call has output so far.
    pub(crate) fn live(&self, live: Live) {
        let after = lock(&self.log).records().last().map(|r| r.at);
        if let Some(after) = after {
            (self.out)(Up::Live(harness_substrate::Live { after, live }));
        }
    }

    /// Whether this limb has said how the call ended and the brain has not
    /// yet included it.
    fn settled(&self, call: CallId) -> bool {
        let log = lock(&self.log);
        log.records().any(|r| match &r.what {
            harness_substrate::What::Said(batch) => batch
                .facts
                .iter()
                .any(|f| matches!(f, Fact::Call(id, CallEvent::Settled(_)) if *id == call)),
            harness_substrate::What::Included(_) => false,
        })
    }

    /// Say how a call ended.
    fn settle(&self, report: Report, outcome: Outcome) {
        let call = report.call.expect("a settlement is a call's");
        self.say(report.batch(vec![Fact::Call(call, CallEvent::Settled(outcome))]));
    }

    /// Watch what the brain's live contexts embody: report each resource
    /// whose version is none the brain knows, and from then on each change.
    fn watch(&self, known: &[Observation]) {
        let mut report = Report::new(None);
        let mut watched = lock(&self.watched);
        watched.retain(|resource, _| known.iter().any(|o| o.resource == *resource));
        for o in known {
            if watched.contains_key(&o.resource) {
                continue;
            }
            let Some(version) = self.version_of(&o.resource) else {
                continue;
            };
            watched.insert(o.resource.clone(), version);
            let new = known
                .iter()
                .all(|k| k.resource != o.resource || k.version != version);
            if new {
                report.changed(o.resource.clone(), version);
            }
        }
        drop(watched);
        if !report.changes.is_empty() {
            self.say(report.batch(Vec::new()));
        }
    }

    fn describe(&self) -> Description {
        if self.kind == Kind::User {
            return self.describe_user();
        }
        let text = tools::instructions(&self.root, self.clone.as_ref());
        let version = Version::Is(Digest::of(text.as_bytes()));
        let mut instructions = vec![Instruction {
            observes: Observation {
                resource: self.key(INSTRUCTIONS, ""),
                version,
            },
            text,
        }];
        if let Ok(agents) = std::fs::read_to_string(self.root.join("AGENTS.md")) {
            instructions.push(Instruction {
                observes: Observation {
                    resource: self.key(INSTRUCTIONS, "AGENTS.md"),
                    version: Version::Is(Digest::of(agents.as_bytes())),
                },
                text: format!("Contents of AGENTS.md:\n\n{agents}"),
            });
        }
        Description {
            instructions,
            tools: tools::tools(),
        }
    }

    fn describe_user(&self) -> Description {
        let mut instructions = Vec::new();
        if let Ok(own) = std::fs::read_to_string(self.root.join(user::INSTRUCTIONS)) {
            instructions.push(Instruction {
                observes: Observation {
                    resource: self.key(INSTRUCTIONS, user::INSTRUCTIONS),
                    version: Version::Is(Digest::of(own.as_bytes())),
                },
                text: format!("Max's own instructions, for every agent:\n\n{own}"),
            });
        }
        let index = user::index(&self.root);
        instructions.push(Instruction {
            observes: Observation {
                resource: self.key(INSTRUCTIONS, user::INDEX),
                version: Version::Is(Digest::of(index.as_bytes())),
            },
            text: index,
        });
        Description {
            instructions,
            tools: user::tools(),
        }
    }

    /// The file whose contents are among the limb's instructions.
    fn instruction_file(&self) -> &'static str {
        if self.kind == Kind::User {
            user::INSTRUCTIONS
        } else {
            "AGENTS.md"
        }
    }

    fn key(&self, kind: &str, name: &str) -> ResourceKey {
        ResourceKey {
            limb: self.limb,
            kind: kind.into(),
            name: name.into(),
        }
    }

    /// Every resource a file is: the file, and the root's instructions when
    /// it is the root's `AGENTS.md`.
    fn keys(&self, name: &str) -> Vec<ResourceKey> {
        let mut keys = vec![self.key(FILE, name)];
        if name == self.instruction_file() {
            keys.push(self.key(INSTRUCTIONS, name));
        }
        keys
    }

    fn version_of(&self, resource: &ResourceKey) -> Option<Version> {
        match resource.kind.as_str() {
            INSTRUCTIONS if self.kind == Kind::User && resource.name == user::INDEX => {
                Some(Version::Is(Digest::of(user::index(&self.root).as_bytes())))
            }
            FILE | INSTRUCTIONS if !resource.name.is_empty() => {
                files::version(&self.root.join(&resource.name)).ok()
            }
            INSTRUCTIONS => Some(Version::Is(Digest::of(
                tools::instructions(&self.root, self.clone.as_ref()).as_bytes(),
            ))),
            _ => None,
        }
    }

    /// A file moved, written by the report's call.
    pub(crate) fn changed(&self, report: &mut Report, name: &str, version: Version) {
        for resource in self.keys(name) {
            report.changed(resource, version);
        }
    }

    /// After a call: add every watched resource that moved, which is how a
    /// command's writes reach the contexts that read them. Attributed to the
    /// call when it was a command, whose writes the limb cannot see.
    fn rescan(&self, report: &mut Report, command: bool) {
        let mut watched = lock(&self.watched);
        for change in &report.changes {
            if let Some(known) = watched.get_mut(&change.resource) {
                *known = change.version;
            }
        }
        let by = report.call.filter(|_| command);
        for (resource, known) in watched.iter_mut() {
            let Some(version) = self.version_of(resource) else {
                continue;
            };
            if version != *known {
                *known = version;
                report.changes.push(Change {
                    resource: resource.clone(),
                    version,
                    by,
                    content: None,
                });
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
