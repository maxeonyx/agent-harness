//! The workspace limb: a shell and file tools in one directory, speaking the
//! core model's [`ToLimb`] and [`FromLimb`].
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
//! Everything it says is journal facts about itself, its own calls and its
//! files ([`FromLimb::Facts`]): its [`Description`] right after its hello; a
//! settlement, with the bytes of its output, and a [`Change`] naming the
//! call for each file the call wrote, in one batch.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. An environment either
//! holds this limb or does not.
//!
//! The brain runs it as a process of its own, `harness-limb-workspace` or
//! `harness-limb-user` ([`process`]).
//!
//! A call is run on its own thread, after the calls its task made before it
//! in the same workspace have ended: a model writes one response's tool uses
//! in the order it means them to happen (write a file, then commit it), so
//! they run in that order. Background work and other tasks run alongside.
//! Each call is recorded on disk as started before
//! it has any effect and as settled before the settlement is sent. So
//! [`ToLimb::Run`] is idempotent per call, however often a crash of the
//! brain or of the limb makes it arrive: it is answered from the record, left
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
use std::io::{self, PipeReader};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::environment::{Description, FromLimb, Instruction, Limb, LimbKind, ToLimb};
use harness_core::id::{CallId, Digest, InstanceId, LimbId, TaskId, ValueId};
use harness_core::journal::{Batch, Fact};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::value::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::json;

use harness_runtime::Cancel;

use crate::record::{Calls, Recorded};

/// The resource kinds this limb owns.
const FILE: &str = "file";
const INSTRUCTIONS: &str = "instructions";

/// The plugin kind this limb is, which an environment's place names.
pub const KIND: &str = "workspace";

/// How long its calls get to stop when the limb shuts down without being
/// told how long: its input closed, or it was signalled.
pub const SHUTDOWN: Duration = Duration::from_secs(5);

/// A workspace's own settings: the `config` of its [`Limb`] definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The directory the tools work in. Must exist, unless `clone` makes it.
    pub dir: PathBuf,
    /// Made before the limb first says hello, if `dir` does not exist yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clone: Option<Clone>,
    /// A user limb's set of Max's skills, by name: the index lists only
    /// these and `skill` loads only these. Every skill when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
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
    pub fn user(id: LimbId, dir: PathBuf, skills: Option<Vec<String>>) -> Limb {
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
    /// Where the limb keeps its record of calls. Outside the directory, so
    /// the agents' files stay theirs.
    pub state: PathBuf,
}

/// One running instance of the workspace limb.
///
/// Everything it says goes to the `Sender` given at [`Workspace::open`],
/// [`FromLimb::Hello`] first. Dropping it stops every running call, within
/// [`SHUTDOWN`], and waits until each has settled.
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    limb: LimbId,
    instance: InstanceId,
    root: PathBuf,
    /// The clone it was made as, which its instructions say is the agent's
    /// own.
    clone: Option<Clone>,
    /// The skills a user limb offers; every one when `None`.
    skills: Option<Vec<String>>,
    kind: Kind,
    calls: Calls,
    out: Sender<FromLimb>,
    running: Mutex<HashMap<CallId, Running>>,
    /// For each task, the end of its latest call: it hangs up when that
    /// call ends, and the task's next call waits for that.
    latest: Mutex<HashMap<TaskId, PipeReader>>,
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
    /// Start an instance: make its clone if it has one to make, end what an
    /// earlier instance left running, say hello, and describe itself. Blocks
    /// while it clones.
    pub fn open(config: Config, out: Sender<FromLimb>) -> io::Result<Workspace> {
        let Settings { dir, clone, skills } = config.settings;
        if let Some(clone) = &clone {
            git::clone(clone, &dir)?;
        }
        let inner = Inner {
            limb: config.limb,
            instance: InstanceId::generate(),
            root: dir.canonicalize()?,
            clone,
            skills,
            kind: config.kind,
            calls: Calls::open(config.state.join("calls"))?,
            out,
            running: Mutex::default(),
            latest: Mutex::default(),
            shutting_down: AtomicBool::new(false),
            watched: Mutex::default(),
            files: Mutex::default(),
        };
        // An instance that was killed mid-command left its processes
        // running, each carrying its call's mark.
        for call in inner.calls.orphans()? {
            let mark = shell::mark(call);
            let mut waiter = Cancel::new().waiter()?;
            let deadline = Instant::now() + SHUTDOWN;
            let members = || harness_runtime::marked(&mark);
            harness_runtime::end(&mut waiter, deadline, Some(libc::SIGTERM), members, || None)?;
            inner.calls.ran(call);
        }
        inner.send(FromLimb::Hello {
            instance: inner.instance,
        });
        inner.send(FromLimb::Facts(Batch {
            values: Vec::new(),
            facts: vec![Fact::Described {
                limb: inner.limb,
                description: inner.describe(),
            }],
        }));
        Ok(Workspace {
            inner: Arc::new(inner),
        })
    }

    pub fn instance(&self) -> InstanceId {
        self.inner.instance
    }

    /// Act on one message from the brain. Never blocks on a call: calls run
    /// on their own threads and report when they settle.
    pub fn handle(&self, message: ToLimb) {
        let inner = &self.inner;
        match message {
            ToLimb::Run { id, call, input } => self.run(id, call, input),
            ToLimb::Stop { call, within } => inner.stop(call, within),
            ToLimb::Shutdown { within } => self.stop_all(Duration::from_millis(within)),
            ToLimb::Watch { known } => {
                let mut report = Report::new(None);
                let mut watched = lock(&inner.watched);
                for o in &known {
                    if watched.contains_key(&o.resource) {
                        continue;
                    }
                    let Some(version) = inner.version_of(&o.resource) else {
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
                if !report.changes.is_empty() {
                    inner.send(FromLimb::Facts(report.batch(Vec::new())));
                }
            }
        }
    }

    /// Run a call at most once, however often it arrives.
    fn run(&self, id: CallId, call: Call, input: Bytes) {
        let inner = &self.inner;
        let mut running = lock(&inner.running);
        // A limb shutting down starts nothing; the next instance runs it.
        if running.contains_key(&id) || inner.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        match inner.calls.get(id) {
            Ok(None) => {}
            Ok(Some(Recorded::Settled(batch))) => return inner.send(FromLimb::Facts(batch)),
            Ok(Some(Recorded::Started)) if tools::rerun(&call.tool) => {}
            // Cut off while an earlier instance ran it.
            Ok(Some(Recorded::Started)) => {
                return inner.settle(Report::new(Some(id)), Outcome::Unknown);
            }
            Err(e) => {
                let mut report = Report::new(Some(id));
                let outcome = report.failed(&format!("cannot read the limb's record: {e}"));
                return inner.settle(report, outcome);
            }
        }
        if let Err(e) = inner.calls.put(id, &Recorded::Started) {
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
            thread::spawn(move || {
                let mut report = Report::new(Some(id));
                let outcome = match after(before, &cancel) {
                    Ok(()) => inner.execute(id, &call, &input, &cancel, &mut report),
                    Err(_) => Outcome::Interrupted { partial: None },
                };
                inner.rescan(&mut report, call.tool == "bash");
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

/// What one call, or one answer to [`ToLimb::Watch`], says: the bytes its
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

    /// The facts, led by the settlement when there is one, so that a repeat
    /// the journal already has is refused at its first fact.
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
    fn send(&self, message: FromLimb) {
        // A brain that has gone hears nothing; the record still has it.
        let _ = self.out.send(message);
    }

    /// Record how a call ended, then say so.
    fn settle(&self, report: Report, outcome: Outcome) {
        let call = report.call.expect("a settlement is a call's");
        let batch = report.batch(vec![Fact::Call(call, CallEvent::Settled(outcome))]);
        // If the record cannot be written, a repeated Run finds the call
        // started and cut off, which is still true.
        let _ = self.calls.put(call, &Recorded::Settled(batch.clone()));
        self.send(FromLimb::Facts(batch));
    }

    fn stop(&self, call: CallId, within: u64) {
        if let Some(running) = lock(&self.running).get(&call) {
            let deadline = Instant::now() + Duration::from_millis(within);
            return running.cancel.request(deadline);
        }
        match self.calls.get(call) {
            Ok(Some(Recorded::Settled(batch))) => self.send(FromLimb::Facts(batch)),
            // Cut off, or never received: nobody knows what it did.
            _ => self.settle(Report::new(Some(call)), Outcome::Unknown),
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
        let index = user::index(&self.root, self.skills.as_deref());
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
                    user::index(&self.root, self.skills.as_deref()).as_bytes(),
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
