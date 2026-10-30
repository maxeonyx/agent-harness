//! The workspace tool set: a shell and file tools in any directory on the
//! limb's machine, and the `user` set of Max's own instructions and skills
//! ([`user`]).
//!
//! **Concept.** One [`Workspace`] per limb serves every working directory
//! its agents have ([`harness_core::limb::Workdir`]). It runs the coding
//! tools an engineer needs (`bash`, `read`, `write`, `edit`, `grep`; see
//! [`tools`]) as [`harness_core::call::Call`]s, each in its caller's
//! directory (the row's `dir`), and its files are resources of kind `file`,
//! named by absolute canonical path, so two directories that nest see each
//! other's writes: a read's result observes the version it returned, a
//! write's the version it produced, and a write or edit whose caller has
//! not seen the current version is [refused](Outcome::Refused) before it
//! touches anything. A caller who has never seen an existing file is
//! treated as having seen it absent, so nothing is overwritten blind. The
//! refusal shows the file as a read would, which counts as seeing it, so a
//! retry written after it goes through. Forks and siblings share a
//! directory; the notices and refusals are how they coordinate in it.
//!
//! Each directory the brain asks for ([`LimbRow::Workdir`]) is made ready on
//! a thread of its own (made, or cloned for its agent's branch), and the
//! limb says what it found as `Fact::Workdir`: its instructions, or why it
//! cannot be made, which fails only the agents starting there. It states
//! them again on each new link and whenever they change.
//!
//! What it keeps is per limb, not per directory: one table of the
//! resources live contexts have seen, rescanned after every call (`stat`
//! first, reading only what moved), so a command's writes, or Max's in his
//! editor, reach the contexts that read them; per agent, what its own calls
//! did ([`Own`]); and one lock held across a file tool's check and its
//! write. A change the rescan finds is a command's only if the file's time
//! falls inside the command's run; otherwise it is a notice to everyone who
//! saw the file, the command's caller too.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. A role either holds
//! this set or does not.
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

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use harness_core::call::{Call, Caller, Outcome};
use harness_core::fact::{Batch, Fact};
use harness_core::id::{CallId, Digest, TaskId};
pub use harness_core::limb::WORKSPACE;
use harness_core::limb::{self, Branch, Instruction, LimbKey, LimbRow, Offer, Prepared};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_limb::{Opening, Report, Rows, Running, ToolSet, Voice, guard, lock};
use harness_substrate::Time;

use crate::record::{Marks, Outputs};

/// The resource kinds this set owns.
const FILE: &str = "file";
const INSTRUCTIONS: &str = "instructions";

/// Open the workspace set: it has no settings of its own. Ends what an
/// earlier instance's commands left running.
pub fn open(_offer: &Offer, opening: &Opening) -> Result<Arc<dyn ToolSet>, String> {
    let state = &opening.state;
    let inner = Inner {
        voice: opening.voice.clone(),
        marks: Marks::open(state.join("running")).map_err(|e| e.to_string())?,
        outputs: Outputs::open(state.join("outputs")).map_err(|e| e.to_string())?,
        dirs: Mutex::default(),
        agents: Mutex::default(),
        own: Mutex::default(),
        watched: Mutex::default(),
        files: Mutex::default(),
    };
    // An instance that was killed mid-command left its processes running,
    // each carrying its call's mark, in its call's scope.
    match inner.marks.orphans() {
        Ok(orphans) => {
            for call in orphans {
                if let Err(e) = shell::end_orphan(call) {
                    eprintln!("ending what call {call} left running: {e}");
                }
                inner.marks.ran(call);
            }
        }
        Err(e) => eprintln!("the commands an earlier instance left running: {e}"),
    }
    Ok(Arc::new(Workspace {
        inner: Arc::new(inner),
    }))
}

/// The workspace set of one limb.
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    voice: Arc<Voice>,
    marks: Marks,
    outputs: Outputs,
    /// Each directory the brain asks for, by the name it gave.
    dirs: Mutex<BTreeMap<PathBuf, Dir>>,
    /// The agents working here, by name: what their commands' scopes say.
    agents: Mutex<BTreeMap<TaskId, String>>,
    /// What each task's own calls did, so that it is not refused for it.
    own: Mutex<HashMap<TaskId, Own>>,
    /// The resources the brain asked to hear about, as last seen.
    watched: Mutex<HashMap<ResourceKey, Seen>>,
    /// Held across a file tool's check and its write, so two calls cannot
    /// both pass the check on one version.
    files: Mutex<()>,
}

/// A directory the brain asked for.
#[derive(Clone)]
enum Dir {
    Preparing,
    Ready {
        /// Where it is: absolute and canonical.
        root: PathBuf,
        branch: Option<Branch>,
        /// As last stated.
        instructions: Vec<Instruction>,
    },
    Failed,
}

/// A resource as the rescan last saw it.
#[derive(Clone)]
struct Seen {
    version: Version,
    /// The file's identity and times, which unchanged mean its bytes are.
    stat: Option<Stat>,
    /// When it last changed, as far as the disk says: the file's time, or
    /// its directory's once it is gone.
    modified: Option<SystemTime>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stat {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl Stat {
    fn of(meta: &fs::Metadata) -> Stat {
        Stat {
            dev: meta.dev(),
            ino: meta.ino(),
            len: meta.len(),
            mtime: (meta.mtime(), meta.mtime_nsec()),
            ctime: (meta.ctime(), meta.ctime_nsec()),
        }
    }
}

impl ToolSet for Workspace {
    fn describe(&self) -> limb::ToolSet {
        let text = tools::USE.to_string();
        limb::ToolSet {
            name: WORKSPACE.into(),
            instructions: vec![Instruction {
                observes: Observation {
                    resource: self.inner.key(INSTRUCTIONS, WORKSPACE),
                    version: Version::Is(Digest::of(text.as_bytes())),
                },
                text,
            }],
            tools: tools::tools(),
        }
    }

    /// Make each new directory ready, on a thread of its own, and forget
    /// those no longer asked for. A new link hears each again, and a
    /// directory that could not be made is tried again.
    fn apply(&self, rows: &Rows, connected: bool) {
        let inner = &self.inner;
        let mut wanted = BTreeMap::new();
        let mut agents = BTreeMap::new();
        for (key, row) in &rows.rows {
            if let (LimbKey::Workdir(dir), LimbRow::Workdir { branch, agents: a }) = (key, row) {
                wanted.insert(dir.clone(), branch.clone());
                agents.extend(a.iter().map(|(t, n)| (*t, n.clone())));
            }
        }
        *lock(&inner.agents) = agents;
        let mut restate = Vec::new();
        {
            let mut dirs = lock(&inner.dirs);
            dirs.retain(|dir, _| wanted.contains_key(dir));
            for (dir, branch) in wanted {
                match dirs.get(&dir) {
                    None => {}
                    Some(Dir::Failed) if connected => {}
                    Some(Dir::Ready { .. }) if connected => {
                        restate.push(dir);
                        continue;
                    }
                    Some(_) => continue,
                }
                dirs.insert(dir.clone(), Dir::Preparing);
                let inner = self.inner.clone();
                thread::spawn(move || inner.prepare(dir, branch));
            }
        }
        for dir in restate {
            inner.restate(&dir, true);
        }
        if let Some(LimbRow::Watch { known }) = rows.rows.get(&LimbKey::Watch) {
            inner.watch(known);
        }
    }

    fn rerun(&self, tool: &str) -> bool {
        tools::rerun(tool)
    }

    fn run(&self, running: &Running<'_>, report: &mut Report) -> Outcome {
        let Some(dir) = running.dir else {
            return report.failed("the call names no working directory to run in");
        };
        match absolute(dir).and_then(|d| d.canonicalize()) {
            Ok(root) => self.inner.run(&root, running, report),
            Err(e) => report.failed(&format!("its working directory {}: {e}", dir.display())),
        }
    }
}

/// `dir` as an absolute path: `~/…` in `$HOME`.
pub(crate) fn absolute(dir: &Path) -> io::Result<PathBuf> {
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

/// `path`, absolute, as one name however it was reached: the longest part
/// of it that exists resolved on disk (its links and `..`), and the rest
/// with `.` and `..` resolved by name.
fn canonical(path: &Path) -> PathBuf {
    let path = limb::normal(path);
    let mut existing = path.as_path();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let rest = path.strip_prefix(existing).unwrap_or(Path::new(""));
            return match rest.as_os_str().is_empty() {
                true => real,
                false => real.join(rest),
            };
        }
        match existing.parent() {
            Some(parent) => existing = parent,
            None => return path,
        }
    }
}

impl Inner {
    /// Make `dir` ready and say what was found, unless the brain stopped
    /// asking for it meanwhile.
    fn prepare(&self, dir: PathBuf, branch: Option<Branch>) {
        let made = guard(|| make(&dir, branch.as_ref())).and_then(|r| r);
        let state = match made {
            Ok(root) => Dir::Ready {
                instructions: dir_instructions(self, &root, branch.as_ref()),
                root,
                branch,
            },
            Err(why) => {
                eprintln!("the directory {}: {why}", dir.display());
                self.say_workdir(&dir, Prepared::Failed { why });
                let mut dirs = lock(&self.dirs);
                if let Some(d @ Dir::Preparing) = dirs.get_mut(&dir) {
                    *d = Dir::Failed;
                }
                return;
            }
        };
        let mut dirs = lock(&self.dirs);
        let Some(d @ Dir::Preparing) = dirs.get_mut(&dir) else {
            return;
        };
        *d = state;
        drop(dirs);
        self.restate(&dir, true);
    }

    /// State a ready directory's instructions: `always`, or only if they
    /// changed since they were last stated.
    fn restate(&self, dir: &Path, always: bool) {
        let mut dirs = lock(&self.dirs);
        let Some(Dir::Ready {
            root,
            branch,
            instructions,
        }) = dirs.get_mut(dir)
        else {
            return;
        };
        let now = dir_instructions(self, root, branch.as_ref());
        if !always && now == *instructions {
            return;
        }
        *instructions = now.clone();
        drop(dirs);
        self.say_workdir(dir, Prepared::Ready { instructions: now });
    }

    fn say_workdir(&self, dir: &Path, prepared: Prepared) {
        self.voice.say(Batch {
            values: Vec::new(),
            facts: vec![Fact::Workdir {
                limb: self.voice.limb(),
                dir: dir.to_path_buf(),
                prepared,
            }],
        });
    }

    /// Run one call in `root`: as its caller wrote it, noting what it made
    /// and what it did.
    fn run(&self, root: &Path, running: &Running<'_>, report: &mut Report) -> Outcome {
        let started = SystemTime::now();
        let written = self.as_written(running.id, running.call.clone());
        let command = running.call.tool == "bash";
        let mut outcome = self.execute(root, running, &written, report);
        let ran = (started, SystemTime::now());
        let made = self.rescan(report, command.then_some(ran));
        if let Outcome::Completed(output)
        | Outcome::Failed(output)
        | Outcome::Interrupted {
            partial: Some(output),
        } = &mut outcome
        {
            output.observes.extend(made);
        }
        self.did(running.id, running.call, &outcome, ran);
        let ready: Vec<PathBuf> = lock(&self.dirs).keys().cloned().collect();
        for dir in ready {
            self.restate(&dir, false);
        }
        outcome
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
            let Some(seen) = self.current(&o.resource, None) else {
                continue;
            };
            let new = known
                .iter()
                .all(|k| k.resource != o.resource || k.version != seen.version);
            if new {
                report.changed(o.resource.clone(), seen.version, false);
            }
            watched.insert(o.resource.clone(), seen);
        }
        drop(watched);
        if !report.changes().is_empty() {
            self.voice.say(report.batch(Vec::new()));
        }
    }

    fn key(&self, kind: &str, name: &str) -> ResourceKey {
        ResourceKey {
            limb: self.voice.limb(),
            kind: kind.into(),
            name: name.into(),
        }
    }

    /// The ready directories' roots.
    fn roots(&self) -> Vec<PathBuf> {
        lock(&self.dirs)
            .values()
            .filter_map(|d| match d {
                Dir::Ready { root, .. } => Some(root.clone()),
                _ => None,
            })
            .collect()
    }

    /// Every resource a file is: the file, and a directory's instructions
    /// when it is a ready directory's `AGENTS.md`.
    fn keys(&self, name: &str) -> Vec<ResourceKey> {
        let mut keys = vec![self.key(FILE, name)];
        if self
            .roots()
            .iter()
            .any(|root| root.join("AGENTS.md").as_os_str() == name)
        {
            keys.push(self.key(INSTRUCTIONS, name));
        }
        keys
    }

    /// A resource as it is now, reading a file only if its `stat` moved
    /// since `before`. `None` for one this set does not keep, or cannot
    /// read.
    fn current(&self, resource: &ResourceKey, before: Option<&Seen>) -> Option<Seen> {
        if resource.limb != self.voice.limb() {
            return None;
        }
        let text = |text: &str| Seen {
            version: Version::Is(Digest::of(text.as_bytes())),
            stat: None,
            modified: None,
        };
        match resource.kind.as_str() {
            INSTRUCTIONS if resource.name == WORKSPACE => Some(text(tools::USE)),
            FILE | INSTRUCTIONS if resource.name.starts_with('/') => {
                let path = canonical(Path::new(&resource.name));
                if resource.kind == INSTRUCTIONS
                    && let Some(Dir::Ready { root, branch, .. }) = lock(&self.dirs)
                        .values()
                        .find(|d| matches!(d, Dir::Ready { root, .. } if *root == path))
                {
                    return Some(text(&tools::instructions(root, branch.as_ref())));
                }
                seen(&path, before)
            }
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
    /// command's writes, and anyone's outside the harness, reach the
    /// contexts that read them. A change is the command's (`ran` is when it
    /// ran) only if the file's time falls inside its run: then it names the
    /// call, and is returned as what the command's caller has seen, since it
    /// made it, so its context is not told of its own writes. Any other
    /// change is a notice to everyone who saw the file. A sibling's command
    /// that wrote the same file at the same moment is attributed to both.
    fn rescan(
        &self,
        report: &mut Report,
        ran: Option<(SystemTime, SystemTime)>,
    ) -> Vec<Observation> {
        let mut made = Vec::new();
        let mut watched = lock(&self.watched);
        for change in report.changes() {
            if let Some(seen) = watched.get_mut(&change.resource) {
                seen.version = change.version;
                seen.stat = None;
            }
        }
        for (resource, before) in watched.iter_mut() {
            let Some(now) = self.current(resource, Some(before)) else {
                continue;
            };
            if now.version != before.version {
                let by = ran.is_some_and(|(start, end)| {
                    now.modified
                        .is_some_and(|m| (start - COARSE..=end).contains(&m))
                });
                report.changed(resource.clone(), now.version, by);
                if by {
                    made.push(Observation {
                        resource: resource.clone(),
                        version: now.version,
                    });
                }
            }
            *before = now;
        }
        made
    }
}

/// A file as it is now, read only if its `stat` moved since `before`.
fn seen(path: &Path, before: Option<&Seen>) -> Option<Seen> {
    match fs::metadata(path) {
        Ok(meta) => {
            let stat = Stat::of(&meta);
            if let Some(before) = before.filter(|b| b.stat == Some(stat)) {
                return Some(before.clone());
            }
            let version = files::version(path).ok()?;
            Some(Seen {
                version,
                stat: Some(stat),
                modified: meta.modified().ok(),
            })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some(Seen {
            version: Version::Gone,
            stat: None,
            modified: path
                .parent()
                .and_then(|p| fs::metadata(p).and_then(|m| m.modified()).ok()),
        }),
        Err(_) => None,
    }
}

/// Make the directory the brain named: its clone if it has a branch and
/// does not exist yet, otherwise the directory. Where it is.
fn make(dir: &Path, branch: Option<&Branch>) -> Result<PathBuf, String> {
    let path = absolute(dir).map_err(|e| e.to_string())?;
    match branch {
        Some(branch) => git::clone(branch, &path).map_err(|e| e.to_string())?,
        None => {
            fs::create_dir_all(&path).map_err(|e| format!("cannot make {}: {e}", path.display()))?
        }
    }
    path.canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A directory's own instructions: where it is and whose branch it is,
/// and what its `AGENTS.md` says.
fn dir_instructions(inner: &Inner, root: &Path, branch: Option<&Branch>) -> Vec<Instruction> {
    let text = tools::instructions(root, branch);
    let name = root.to_string_lossy();
    let mut instructions = vec![Instruction {
        observes: Observation {
            resource: inner.key(INSTRUCTIONS, &name),
            version: Version::Is(Digest::of(text.as_bytes())),
        },
        text,
    }];
    let agents = root.join("AGENTS.md");
    if let Ok(text) = fs::read_to_string(&agents) {
        instructions.push(Instruction {
            observes: Observation {
                resource: inner.key(INSTRUCTIONS, &agents.to_string_lossy()),
                version: Version::Is(Digest::of(text.as_bytes())),
            },
            text: format!("Contents of AGENTS.md:\n\n{text}"),
        });
    }
    instructions
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
    /// order written, so each was written knowing what the earlier ones
    /// would leave. It expects what its context knew when the response was
    /// made, updated by what those calls observed: an agent's second
    /// `edit` of a file in one response is not refused for its first.
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
    /// one ran. A command's other writes reach this set only as changes to
    /// what some context has seen ([`Inner::rescan`]); a file it made that
    /// nobody has seen, it has seen as its maker.
    pub(crate) fn made_by(&self, caller: &Caller, path: &Path) -> bool {
        let Caller::Agent { task, .. } = caller else {
            return false;
        };
        let Ok(modified) = fs::metadata(path).and_then(|m| m.modified()) else {
            return false;
        };
        lock(&self.own).get(task).is_some_and(|own| {
            own.commands
                .iter()
                .any(|(start, end)| (*start - COARSE..=*end).contains(&modified))
        })
    }
}
