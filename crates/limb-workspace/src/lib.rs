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
mod record;
mod shell;
mod tools;
mod trunc;
pub mod user;

use std::collections::{HashMap, VecDeque};
use std::io::{self, PipeReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use harness_core::call::{Call, Caller, Outcome};
use harness_core::environment::{Description, Instruction, Limb, LimbKey, LimbRow};
use harness_core::id::{CallId, Digest, LimbId, TaskId};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_substrate::Time;
use serde::{Deserialize, Serialize};

use harness_limb::{CallRow, Report, Rows, SHUTDOWN, Voice, instant, lock};
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
        harness_limb::settings(limb, KIND).or_else(|| harness_limb::settings(limb, user::KIND))
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
        harness_limb::definition(id, name, kind.name(), self)
    }
}

/// The two plugin kinds this crate runs: one implementation over a
/// directory, with different tools and instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// An agent's working directory, with the coding tools.
    Workspace,
    /// Max's own agent configuration ([`user`]).
    User,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Workspace => KIND,
            Kind::User => user::KIND,
        }
    }
}

/// One running instance of the workspace limb: a [`harness_limb::Kind`].
/// Dropping it stops every running call, within [`SHUTDOWN`], and waits
/// until each has settled.
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    voice: Arc<Voice>,
    root: PathBuf,
    /// The clone it was made as, which its instructions say is the agent's
    /// own.
    clone: Option<Clone>,
    /// The file naming the skills a user limb offers.
    skills: Option<PathBuf>,
    kind: Kind,
    marks: Marks,
    outputs: Outputs,
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

impl harness_limb::Kind for Workspace {
    /// Make its directory (its clone, if it has one to make), and end what
    /// an earlier instance left running. Blocks while it clones. A
    /// directory given as `~/…` is in `$HOME`; any other must be absolute.
    /// Its marks of calls are in its `state`, outside the directory, so the
    /// agents' files stay theirs.
    fn open(definition: &Limb, voice: Arc<Voice>, state: &Path) -> Result<Workspace, String> {
        let kind = match &definition.kind {
            harness_core::environment::LimbKind::Plugin { kind, .. } if kind == KIND => {
                Kind::Workspace
            }
            harness_core::environment::LimbKind::Plugin { kind, .. } if kind == user::KIND => {
                Kind::User
            }
            _ => return Err("the definition is not of a workspace or user limb".into()),
        };
        let settings = Settings::of(definition).ok_or("the definition's settings do not parse")?;
        Workspace::open(kind, settings, voice, state).map_err(|e| e.to_string())
    }

    /// A whole view begins a link: the limb describes itself on it.
    fn apply(&self, rows: &Rows, connected: bool) {
        let inner = &self.inner;
        if connected {
            inner.voice.describe(inner.describe());
        }
        for row in inner.voice.calls(rows) {
            self.call(row);
        }
        if let Some(LimbRow::Watch { known }) = rows.rows.get(&LimbKey::Watch) {
            inner.watch(known);
        }
        // A call the brain no longer asks for has settled there.
        let running = lock(&inner.running);
        for call in inner.marks.started() {
            if !running.contains_key(&call) && !rows.rows.contains_key(&LimbKey::Call(call)) {
                inner.marks.forget(call);
            }
        }
    }

    /// Whether a call runs, or a process one left (a server it started
    /// with `setsid`, adopted by the limb) does.
    fn holds(&self) -> bool {
        !lock(&self.inner.running).is_empty() || !harness_runtime::children().is_empty()
    }

    /// Stop every running call, each to be gone `within` from now, and run
    /// no new one. Asked again with less time, it makes each stop harder.
    fn stop_all(&self, within: Duration) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + within;
        for running in lock(&self.inner.running).values() {
            running.cancel.request(deadline);
        }
    }

    /// Wait until every call has settled and nothing it started is left.
    fn join(&self) {
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

impl Workspace {
    fn open(
        kind: Kind,
        settings: Settings,
        voice: Arc<Voice>,
        state: &Path,
    ) -> io::Result<Workspace> {
        let Settings { dir, clone, skills } = settings;
        let dir = absolute(&dir)?;
        match &clone {
            Some(clone) => git::clone(clone, &dir)?,
            None => std::fs::create_dir_all(&dir).map_err(|e| {
                io::Error::new(e.kind(), format!("cannot make {}: {e}", dir.display()))
            })?,
        }
        std::fs::create_dir_all(state)?;
        let inner = Inner {
            voice,
            root: dir.canonicalize()?,
            clone,
            skills,
            kind,
            marks: Marks::open(state.join("calls"))?,
            outputs: Outputs::open(state.join("outputs"))?,
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
        })
    }

    /// One call row: run it at most once, however often it arrives, and
    /// stop it when the row says so.
    fn call(&self, row: CallRow) {
        let CallRow {
            id,
            call,
            input,
            stopping,
            earlier,
            ..
        } = row;
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
        if inner.voice.settled(id) {
            return;
        }
        let started = earlier || inner.marks.is_started(id);
        if stopping.is_some() {
            let outcome = if started {
                Outcome::Unknown
            } else {
                Outcome::Interrupted { partial: None }
            };
            return inner.voice.settle(Report::new(Some(id)), outcome);
        }
        // A limb shutting down starts nothing; the next instance runs it.
        if inner.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        // Cut off while an earlier instance ran it.
        if started && !tools::rerun(&call.tool) {
            return inner.voice.settle(Report::new(Some(id)), Outcome::Unknown);
        }
        let Some(input) = input.cloned() else {
            let mut report = Report::new(Some(id));
            let outcome = report.failed("the brain sent the call without its input");
            return inner.voice.settle(report, outcome);
        };
        if let Err(e) = inner.marks.start(id) {
            let mut report = Report::new(Some(id));
            let outcome = report.failed(&format!("cannot record the call: {e}"));
            return inner.voice.settle(report, outcome);
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
                inner.voice.settle(report, outcome);
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
        use harness_limb::Kind as _;
        self.stop_all(SHUTDOWN);
        self.join();
    }
}

impl Inner {
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
                report.changed(o.resource.clone(), version, false);
            }
        }
        drop(watched);
        if !report.changes().is_empty() {
            self.voice.say(report.batch(Vec::new()));
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
            limb: self.voice.limb(),
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
            report.changed(resource, version, true);
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
        for change in report.changes() {
            if let Some(known) = watched.get_mut(&change.resource) {
                *known = change.version;
            }
        }
        for (resource, known) in watched.iter_mut() {
            let Some(version) = self.version_of(resource) else {
                continue;
            };
            if version != *known {
                *known = version;
                report.changed(resource.clone(), version, command);
                if command {
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
