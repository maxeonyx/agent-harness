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
//! nothing is overwritten blind. Forks [share](Fork::Share) the workspace;
//! the notices and refusals are how siblings coordinate in it.
//!
//! Everything it says is journal facts about its own calls and files
//! ([`FromLimb::Facts`]): a settlement, with the bytes of its output, and a
//! [`Change`] naming the call for each file the call wrote, in one batch.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. An environment either
//! holds this limb or does not.
//!
//! A call is run on its own thread, and recorded on disk as started before
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
mod record;
mod shell;
pub mod tools;
pub mod trunc;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use harness_core::call::{Call, CallEvent, Outcome, Output};
use harness_core::environment::{
    Description, Fork, FromLimb, Host, Instruction, Limb, LimbKind, Machine, ToLimb,
};
use harness_core::id::{CallId, Digest, InstanceId, LimbId, ValueId};
use harness_core::journal::{Batch, Fact};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use serde_json::json;

use crate::record::{Calls, Recorded};
use crate::shell::Stop;

/// The resource kinds this limb owns.
const FILE: &str = "file";
const INSTRUCTIONS: &str = "instructions";

/// Where a workspace runs and keeps its records.
pub struct Config {
    pub limb: LimbId,
    /// The directory the tools work in. Must exist.
    pub root: PathBuf,
    /// Where the limb keeps its record of calls. Outside `root`, so the
    /// agents' files stay theirs.
    pub state: PathBuf,
}

/// The limb's definition, as the fact that declares it.
pub fn definition(id: LimbId, name: &str, root: &Path) -> Limb {
    Limb {
        id,
        name: name.into(),
        kind: LimbKind::Plugin {
            kind: "workspace".into(),
            config: json!({ "root": root }).to_string(),
            host: Host::Process(Machine::Local),
        },
        fork: Fork::Share,
    }
}

/// One running instance of the workspace limb.
///
/// Everything it says goes to the `Sender` given at [`Workspace::open`],
/// [`FromLimb::Hello`] first. Dropping it stops every running call and
/// reaps its processes.
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    limb: LimbId,
    instance: InstanceId,
    root: PathBuf,
    calls: Calls,
    out: Sender<FromLimb>,
    running: Mutex<HashMap<CallId, Running>>,
    /// The resources the brain asked to hear about, at the version last
    /// reported.
    watched: Mutex<HashMap<ResourceKey, Version>>,
    /// Held across a file tool's check and its write, so two calls cannot
    /// both pass the check on one version.
    files: Mutex<()>,
}

struct Running {
    stop: Arc<Stop>,
    thread: Option<JoinHandle<()>>,
}

impl Workspace {
    pub fn open(config: Config, out: Sender<FromLimb>) -> io::Result<Workspace> {
        let inner = Inner {
            limb: config.limb,
            instance: InstanceId::generate(),
            root: config.root.canonicalize()?,
            calls: Calls::open(config.state.join("calls"))?,
            out,
            running: Mutex::default(),
            watched: Mutex::default(),
            files: Mutex::default(),
        };
        // An instance that died mid-command left its process trees running.
        for orphan in inner.calls.orphans()? {
            shell::kill_orphan(orphan.group, orphan.start);
        }
        inner.send(FromLimb::Hello {
            instance: inner.instance,
        });
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
            ToLimb::Describe => inner.send(FromLimb::Described(inner.describe())),
            ToLimb::Run { id, call, input } => self.run(id, call, input),
            ToLimb::Stop { call } => inner.stop(call),
            ToLimb::Watch { resources } => {
                let mut report = Report::new(None);
                let mut watched = lock(&inner.watched);
                for resource in resources {
                    if let Some(version) = inner.version_of(&resource) {
                        watched.insert(resource.clone(), version);
                        report.changed(resource, version);
                    }
                }
                inner.send(FromLimb::Facts(report.batch(Vec::new())));
            }
        }
    }

    /// Run a call at most once, however often it arrives.
    fn run(&self, id: CallId, call: Call, input: Vec<u8>) {
        let inner = &self.inner;
        let mut running = lock(&inner.running);
        if running.contains_key(&id) {
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
        let stop = Arc::new(Stop::default());
        let thread = {
            let inner = self.inner.clone();
            let stop = stop.clone();
            thread::spawn(move || {
                let mut report = Report::new(Some(id));
                let outcome = inner.execute(id, &call, &input, &stop, &mut report);
                inner.rescan(&mut report, call.tool == "bash");
                inner.settle(report, outcome);
                lock(&inner.running).remove(&id);
            })
        };
        running.insert(
            id,
            Running {
                stop,
                thread: Some(thread),
            },
        );
    }

    /// Stop every running call, reap its processes, and wait for each to
    /// settle.
    pub fn shutdown(&self) {
        let running: Vec<Running> = lock(&self.inner.running).drain().map(|(_, r)| r).collect();
        for r in &running {
            r.stop.set();
        }
        for mut r in running {
            if let Some(thread) = r.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// What one call, or one answer to [`ToLimb::Watch`], says: the bytes its
/// output names and the files that changed.
pub(crate) struct Report {
    call: Option<CallId>,
    values: Vec<Vec<u8>>,
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
        let id = ValueId::of(&bytes);
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

    fn stop(&self, call: CallId) {
        if let Some(running) = lock(&self.running).get(&call) {
            return running.stop.set();
        }
        match self.calls.get(call) {
            Ok(Some(Recorded::Settled(batch))) => self.send(FromLimb::Facts(batch)),
            // Cut off, or never received: nobody knows what it did.
            _ => self.settle(Report::new(Some(call)), Outcome::Unknown),
        }
    }

    fn describe(&self) -> Description {
        let text = tools::instructions(&self.root);
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
        if name == "AGENTS.md" {
            keys.push(self.key(INSTRUCTIONS, name));
        }
        keys
    }

    fn version_of(&self, resource: &ResourceKey) -> Option<Version> {
        match resource.kind.as_str() {
            FILE | INSTRUCTIONS if !resource.name.is_empty() => {
                files::version(&self.root.join(&resource.name)).ok()
            }
            INSTRUCTIONS => Some(Version::Is(Digest::of(
                tools::instructions(&self.root).as_bytes(),
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
