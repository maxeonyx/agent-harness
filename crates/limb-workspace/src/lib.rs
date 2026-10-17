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
//! nothing is overwritten blind. The refusal shows the file as a read
//! would, which counts as seeing it, so a retry written after it goes
//! through. Forks share their source's workspace; the
//! notices and refusals are how siblings coordinate in it.
//!
//! Everything it says is records of its own replica, about itself, its own
//! calls and its files, kept in its own [`Log`] until the brain has
//! included them: its [`Description`] on each connection; a settlement,
//! with the bytes of its output, and a [`Change`] naming the call for each
//! file the call wrote, in one batch. What it hears is the brain's rows for
//! it ([`LimbRow`]): the calls to run and the resources to watch.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. An environment either
//! holds this limb or does not.
//!
//! It runs as a server of its own ([`harness_limb`]), `harness-limb-workspace` or
//! `harness-limb-user` ([`process`]).
//!
//! A call is run on its own thread, after the calls its task made before it
//! in the same workspace have ended: a model writes one response's tool uses
//! in the order it means them to happen (write a file, then commit it), so
//! they run in that order. Background work and other tasks run alongside.
//! Each call is marked on disk as started before it has any effect, and
//! settled in the log before it stops running. So a call row runs at most
//! once, however often a crash of the brain or of the limb makes it arrive:
//! it is left to the settlement the log holds, left
//! running, run for the first time, rerun if its tool allows
//! ([`tools::rerun`]), or settled as [`Outcome::Unknown`] when the instance
//! running it was cut off. Never a failure that did not happen.
//!
//! **Vision.** No permission prompts; process trees owned by their call;
//! interrupted calls whose outcome is unknown kept distinct from failed
//! ones (critique §5); "trunc (head + tail + grep for avoiding context
//! overload) [...] use it by default for commands", with what it leaves
//! out kept where the agent can read it ([`trunc`]); the shared-workspace
//! race.
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

use std::collections::{HashMap, VecDeque};
use std::io::{self, PipeReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::environment::{
    Description, Host, Instruction, Limb, LimbDown, LimbKey, LimbKind, LimbRow,
};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, Digest, LimbId, ReplicaId, TaskId, Timestamp, ValueId};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_store::Store;
use harness_substrate::{Log, Mirror, Restored, Time, Up};
use serde::{Deserialize, Serialize};
use serde_json::json;

use harness_limb::{Out, SHUTDOWN};
use harness_runtime::Cancel;

use crate::record::{Marks, Outputs};

/// The resource kinds this limb owns.
const FILE: &str = "file";
const INSTRUCTIONS: &str = "instructions";

/// The plugin kind this limb is, which an environment's replica names.
pub const KIND: &str = "workspace";

/// A workspace's own settings: the `config` of its [`Limb`] definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The directory the tools work in. Must exist, unless `clone` makes it.
    pub dir: PathBuf,
    /// Made before the limb first connects, if `dir` does not exist yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clone: Option<Clone>,
    /// A user limb's file naming the set of Max's skills it offers, a name
    /// a line (`#` for comments): the index lists only these and `skill`
    /// loads only these. Every skill when absent, or when the file is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<PathBuf>,
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
            LimbKind::Plugin { kind, config, .. } if Kind::of(kind).is_some() => {
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
    pub fn user(id: LimbId, dir: PathBuf, skills: Option<PathBuf>) -> Limb {
        Settings {
            dir,
            clone: None,
            skills,
        }
        .limb(id, "user", Kind::User)
    }

    fn limb(&self, id: LimbId, name: &str, kind: Kind) -> Limb {
        Limb {
            id,
            name: name.into(),
            kind: LimbKind::Plugin {
                kind: kind.name().into(),
                config: serde_json::to_string(self).expect("settings are JSON"),
                host: Host::Local,
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
    /// Where the limb keeps its marks of calls: its state. Outside the
    /// directory, so the agents' files stay theirs.
    pub state: PathBuf,
}

/// One running instance of the workspace limb.
///
/// A follower of the brain: it says its records into its own [`Log`] and
/// ships each through `out`; the brain ships back its rows
/// ([`Workspace::apply`]). Dropping it stops every running call, within
/// [`SHUTDOWN`], and waits until each has settled.
pub struct Workspace {
    inner: Arc<Inner>,
    rows: Mutex<Mirror<LimbKey, LimbRow, Refusal>>,
}

struct Inner {
    limb: LimbId,
    root: PathBuf,
    /// The clone it was made as, which its instructions say is the agent's
    /// own.
    clone: Option<Clone>,
    /// The file naming the skills a user limb offers.
    skills: Option<PathBuf>,
    kind: Kind,
    marks: Marks,
    outputs: Outputs,
    log: Mutex<Log<Harness, Store>>,
    out: Out,
    running: Mutex<HashMap<CallId, Running>>,
    /// For each task, the end of its latest call: it hangs up when that
    /// call ends, and the task's next call waits for that.
    latest: Mutex<HashMap<TaskId, PipeReader>>,
    /// What each task's own calls did, so that it is not refused for it.
    own: Mutex<HashMap<TaskId, Own>>,
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

/// `dir` as an absolute path: `~/…` in `$HOME`.
fn absolute(dir: &Path) -> io::Result<PathBuf> {
    if let Ok(rest) = dir.strip_prefix("~") {
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("no $HOME for `~`"))?;
        return Ok(PathBuf::from(home).join(rest));
    }
    if dir.is_absolute() {
        return Ok(dir.to_path_buf());
    }
    Err(io::Error::other(format!(
        "{} is not an absolute path",
        dir.display()
    )))
}

impl Workspace {
    /// Start an instance on its records `log`: make its directory (its clone,
    /// if it has one to make), and end what an earlier instance left running.
    /// Blocks while it clones. A directory given as `~/…` is in `$HOME`; any
    /// other must be absolute.
    pub fn open(config: Config, log: Log<Harness, Store>, out: Out) -> io::Result<Workspace> {
        let Settings { dir, clone, skills } = config.settings;
        let dir = absolute(&dir)?;
        match &clone {
            Some(clone) => git::clone(clone, &dir)?,
            None => std::fs::create_dir_all(&dir).map_err(|e| {
                io::Error::new(e.kind(), format!("cannot make {}: {e}", dir.display()))
            })?,
        }
        std::fs::create_dir_all(&config.state)?;
        let inner = Inner {
            limb: config.limb,
            root: dir.canonicalize()?,
            clone,
            skills,
            kind: config.kind,
            marks: Marks::open(config.state.join("calls"))?,
            outputs: Outputs::open(config.state.join("outputs"))?,
            log: Mutex::new(log),
            out,
            running: Mutex::default(),
            latest: Mutex::default(),
            own: Mutex::default(),
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

    /// The replica its records are of, the same across instances on one
    /// state directory.
    pub fn replica(&self) -> ReplicaId {
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
    pub fn apply(&self, down: LimbDown) -> Result<Vec<Record>, Restored> {
        let inner = &self.inner;
        let mut ship = Vec::new();
        let connected = matches!(down, LimbDown::View(_));
        let mut replica = lock(&self.rows);
        replica.apply(down);
        let Some(view) = replica.view() else {
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
        if connected {
            inner.say(Batch {
                values: Vec::new(),
                facts: vec![Fact::Described {
                    limb: inner.limb,
                    description: inner.describe(),
                }],
            });
        }
        for (key, row) in &view.rows {
            match (key, row) {
                (
                    LimbKey::Call(id),
                    LimbRow::Call {
                        call,
                        stopping,
                        first_sent_to,
                        ..
                    },
                ) => {
                    let input = match &view.rows.get(&LimbKey::Value(call.input)) {
                        Some(LimbRow::Value(bytes)) => Some(bytes),
                        _ => None,
                    };
                    let earlier = *first_sent_to != self.replica();
                    self.call(*id, call, input, *stopping, earlier);
                }
                (LimbKey::Watch, LimbRow::Watch { known }) => inner.watch(known),
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
        Ok(ship)
    }

    /// One call row: run it at most once, however often it arrives, and
    /// stop it when the row says so. `earlier`: it was first sent to an
    /// instance of this limb whose state is lost, which may have run it.
    fn call(
        &self,
        id: CallId,
        call: &Call,
        input: Option<&Bytes>,
        stopping: Option<Timestamp>,
        earlier: bool,
    ) {
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
        let started = earlier || inner.marks.is_started(id);
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
        // This call's end: the writer, held until it has settled. The
        // task's next call waits on the reader.
        let (before, ended) = match (&call.caller, io::pipe()) {
            (Caller::Agent { task, .. }, Ok((reader, writer))) => {
                (lock(&inner.latest).insert(*task, reader), Some(writer))
            }
            _ => (None, None),
        };
        let thread = {
            let inner = self.inner.clone();
            let cancel = cancel.clone();
            let call = call.clone();
            thread::spawn(move || {
                let mut report = Report::new(Some(id));
                let started = SystemTime::now();
                let mut outcome = match after(before, &cancel) {
                    Ok(()) => {
                        let call = inner.as_written(id, call.clone());
                        inner.execute(id, &call, &input.0, &cancel, &mut report)
                    }
                    Err(_) => Outcome::Interrupted { partial: None },
                };
                let ran = (started, SystemTime::now());
                let made = inner.rescan(&mut report, call.tool == "bash");
                if let Outcome::Completed(output)
                | Outcome::Failed(output)
                | Outcome::Interrupted {
                    partial: Some(output),
                } = &mut outcome
                {
                    output.observes.extend(made);
                }
                inner.did(id, &call, &outcome, ran);
                inner.settle(report, outcome);
                drop(ended);
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

    /// Whether a call runs, or a process one left (a server it started
    /// with `setsid`, adopted by the limb) does.
    pub fn holds(&self) -> bool {
        !lock(&self.inner.running).is_empty() || !harness_runtime::children().is_empty()
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

/// Wait until the call `before` ends (its pipe hangs up), unless this call
/// is asked to stop first.
fn after(before: Option<PipeReader>, cancel: &Cancel) -> io::Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    let mut waiter = cancel.waiter()?;
    loop {
        if cancel.is_requested() {
            return Err(harness_runtime::Cancelled::error());
        }
        if waiter.wait(&[(before.as_raw_fd(), libc::POLLIN)], None)? == harness_runtime::Wake::Ready
        {
            return Ok(());
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
        Outcome::Completed(self.output(said, observes))
    }

    pub(crate) fn output(&mut self, said: &str, observes: Vec<Observation>) -> Output {
        Output {
            model: self.text(said),
            full: None,
            observes,
        }
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
    /// Say a batch at this limb's replica: stored, then shipped.
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
            harness_substrate::What::Included(_) | harness_substrate::What::Unread(_) => false,
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
        let index = user::index(&self.root, self.offered().as_deref());
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
                Some(Version::Is(Digest::of(
                    user::index(&self.root, self.offered().as_deref()).as_bytes(),
                )))
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
    /// call when it was a command, whose writes the limb cannot see; and
    /// returned, as what the command's caller has seen, since it made them:
    /// its context is not told of its own writes. A sibling's command that
    /// wrote the same file at the same moment is attributed to both.
    fn rescan(&self, report: &mut Report, command: bool) -> Vec<Observation> {
        let mut made = Vec::new();
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
                if by.is_some() {
                    made.push(Observation {
                        resource: resource.clone(),
                        version,
                    });
                }
            }
        }
        made
    }
}

/// What one task's own calls did.
struct Own {
    /// The record that made its latest response's calls, which their ids
    /// share, and what those calls have observed so far.
    response: Time,
    observed: Vec<Observation>,
    /// What that response's refusals have shown, which the later ones of
    /// it need not show again.
    shown: Vec<Observation>,
    /// When its latest commands ran, oldest first.
    commands: VecDeque<(SystemTime, SystemTime)>,
}

/// Commands remembered for each task.
const COMMANDS: usize = 50;

/// How far a file's time may lag the clock: the kernel stamps files from
/// a coarse clock, a tick behind the one a command's start is read from.
const COARSE: Duration = Duration::from_millis(20);

impl Inner {
    /// `call` as its caller wrote it: one response's tool uses run in the
    /// order written (see the crate docs), so each was written knowing what
    /// the earlier ones would leave. It expects what its context knew when
    /// the response was made, updated by what those calls observed: an
    /// agent's second `edit` of a file in one response is not refused for
    /// its first.
    fn as_written(&self, id: CallId, mut call: Call) -> Call {
        let Caller::Agent { task, .. } = &call.caller else {
            return call;
        };
        let own = lock(&self.own);
        let Some(own) = own.get(task).filter(|o| o.response == id.0.at) else {
            return call;
        };
        for o in &own.observed {
            call.expects.retain(|e| e.resource != o.resource);
            call.expects.push(o.clone());
        }
        call
    }

    /// Remember what `call` did: what its outcome observed, for the later
    /// calls of its response, and when it ran, if it was a command.
    fn did(&self, id: CallId, call: &Call, outcome: &Outcome, ran: (SystemTime, SystemTime)) {
        let Caller::Agent { task, .. } = &call.caller else {
            return;
        };
        let mut own = lock(&self.own);
        let own = own.entry(*task).or_insert_with(|| Own {
            response: id.0.at,
            observed: Vec::new(),
            shown: Vec::new(),
            commands: VecDeque::new(),
        });
        if own.response != id.0.at {
            own.response = id.0.at;
            own.observed.clear();
            own.shown.clear();
        }
        match outcome {
            Outcome::Refused {
                shown: Some(shown), ..
            } => own.shown.extend(shown.observes.iter().cloned()),
            outcome => {
                if let Some(output) = outcome.output() {
                    own.observed.extend(output.observes.iter().cloned());
                }
            }
        }
        if call.tool == "bash" {
            own.commands.push_back(ran);
            if own.commands.len() > COMMANDS {
                own.commands.pop_front();
            }
        }
    }

    /// Whether an earlier refusal of call `id`'s response showed what
    /// `observes` names.
    pub(crate) fn shown_before(
        &self,
        id: CallId,
        caller: &Caller,
        observes: &[Observation],
    ) -> bool {
        let Caller::Agent { task, .. } = caller else {
            return false;
        };
        lock(&self.own)
            .get(task)
            .filter(|own| own.response == id.0.at)
            .is_some_and(|own| observes.iter().all(|o| own.shown.contains(o)))
    }

    /// Whether `caller`'s own command last changed `path`: it changed while
    /// one ran. A command's other writes reach this limb only as changes
    /// to what some context has seen ([`Inner::rescan`]); a file it made
    /// that nobody has seen, it has seen as its maker.
    pub(crate) fn made_by(&self, caller: &Caller, path: &Path) -> bool {
        let Caller::Agent { task, .. } = caller else {
            return false;
        };
        let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else {
            return false;
        };
        lock(&self.own).get(task).is_some_and(|own| {
            own.commands
                .iter()
                .any(|(start, end)| (*start - COARSE..=*end).contains(&modified))
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
