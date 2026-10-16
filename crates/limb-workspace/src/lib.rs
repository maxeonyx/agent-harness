//! The workspace limb: a shell and file tools in one directory, speaking the
//! core model's [`ToLimb`] and [`FromLimb`].
//!
//! **Concept.** A workspace is a [`Limb`] that owns a directory on a
//! machine. It runs the coding tools an engineer needs (`bash`, `read`,
//! `write`, `edit`, `grep`; see [`tools`]) as [`harness_core::call::Call`]s,
//! and its files are [`ResourceKind::File`] resources: a read's result
//! observes the version it returned, a write's the version it produced, and
//! a write or edit whose caller has not seen the current version is
//! [refused](Outcome::Refused) before it touches anything. A caller who has
//! never seen an existing file is treated as having seen it absent, so
//! nothing is overwritten blind. Forks [share](Fork::Share) the workspace;
//! the notices and refusals are how siblings coordinate in it.
//!
//! Access control is the definition and nothing else: the limb has no
//! prompts and no checks on what a command may touch. An environment either
//! holds this limb or does not.
//!
//! A call is run on its own thread, and recorded on disk as started before
//! it has any effect and as settled before the settlement is sent, so after
//! a crash of the brain or of the limb, [`ToLimb::Recover`] is answered from
//! the record: the outcome, or [`FromLimb::Unknown`] when the instance that
//! ran it is gone. Never a failure that did not happen.
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

use harness_core::call::{Interruption, Outcome};
use harness_core::context::Wire;
use harness_core::environment::{
    Description, Fork, FromLimb, Host, Limb, LimbKind, Machine, ToLimb,
};
use harness_core::id::{CallId, Digest, InstanceId, LimbId};
use harness_core::resource::{Change, Observation, ResourceKey, ResourceKind, Version};
use serde_json::json;

use crate::record::{Blobs, Calls, Entry};
use crate::shell::Stop;

/// Where a workspace runs and keeps its records.
pub struct Config {
    pub limb: LimbId,
    /// The directory the tools work in. Must exist.
    pub root: PathBuf,
    /// Where the limb keeps its record of calls and the full outputs of
    /// truncated results. Outside `root`, so the agents' files stay theirs.
    pub state: PathBuf,
}

/// The limb's definition, as the fact that declares it.
pub fn definition(id: LimbId, name: &str, root: &Path) -> Limb {
    Limb {
        id,
        name: name.into(),
        host: Host::Process {
            machine: Machine::Local,
        },
        kind: LimbKind::Plugin {
            kind: "workspace".into(),
            config: Wire(json!({ "root": root }).to_string()),
        },
        fork: Fork::Share,
    }
}

/// One running instance of the workspace limb.
///
/// Everything it says goes to the `Sender` given at [`Workspace::open`],
/// [`FromLimb::Instance`] first. Dropping it stops every running call and
/// reaps its processes.
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    limb: LimbId,
    instance: InstanceId,
    root: PathBuf,
    calls: Calls,
    blobs: Blobs,
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
        let root = config.root.canonicalize()?;
        let inner = Inner {
            limb: config.limb,
            instance: new_instance(),
            root,
            calls: Calls::open(config.state.join("calls"))?,
            blobs: Blobs::open(config.state.join("blobs"))?,
            out,
            running: Mutex::default(),
            watched: Mutex::default(),
            files: Mutex::default(),
        };
        // An instance that died mid-command left its process trees running.
        for orphan in inner.calls.orphans()? {
            shell::kill_orphan(orphan.group, orphan.start);
        }
        inner.send(FromLimb::Instance(inner.instance));
        Ok(Workspace {
            inner: Arc::new(inner),
        })
    }

    pub fn instance(&self) -> InstanceId {
        self.inner.instance
    }

    /// Where a stored full output is on disk.
    pub fn full_output(&self, id: harness_core::id::ContentId) -> PathBuf {
        self.inner.blobs.path(id)
    }

    /// Act on one message from the brain. Never blocks on a call: calls run
    /// on their own threads and report when they settle.
    pub fn handle(&self, message: ToLimb) {
        let inner = &self.inner;
        match message {
            ToLimb::Describe => inner.send(FromLimb::Described(inner.describe())),
            ToLimb::Call {
                call,
                tool,
                input,
                expects,
            } => self.call(call, tool, input, expects),
            ToLimb::Interrupt { call } => {
                if let Some(running) = lock(&inner.running).get(&call) {
                    // The brain knows why it interrupts; the limb does not.
                    running.stop.set(Interruption::Stop);
                }
            }
            ToLimb::Recover { calls } => {
                for call in calls {
                    inner.recover(call);
                }
            }
            ToLimb::Watch { resources } => {
                for resource in resources {
                    if let Some(version) = inner.version_of(&resource) {
                        lock(&inner.watched).insert(resource.clone(), version);
                        inner.send(FromLimb::Changed(Change {
                            resource,
                            version,
                            by: None,
                        }));
                    }
                }
            }
            // Forks share this limb, so a fork's clone is the limb itself.
            ToLimb::Fork => inner.send(FromLimb::Forked { limb: inner.limb }),
        }
    }

    fn call(&self, call: CallId, tool: String, input: Wire, expects: Vec<Observation>) {
        let inner = &self.inner;
        let mut running = lock(&inner.running);
        if running.contains_key(&call) {
            return;
        }
        match inner.calls.get(call) {
            Ok(Some(Entry {
                outcome: Some(outcome),
                ..
            })) => return inner.send(FromLimb::Settled { call, outcome }),
            Ok(_) => {}
            Err(e) => {
                return inner.settle(
                    call,
                    files::failed(format!("cannot read the limb's record: {e}")),
                );
            }
        }
        let started = Entry {
            instance: inner.instance,
            outcome: None,
        };
        if let Err(e) = inner.calls.put(call, &started) {
            return inner.settle(call, files::failed(format!("cannot record the call: {e}")));
        }
        let stop = Arc::new(Stop::default());
        let thread = {
            let inner = self.inner.clone();
            let stop = stop.clone();
            thread::spawn(move || {
                let outcome = inner.execute(call, &tool, &input, &expects, &stop);
                inner.rescan();
                inner.settle(call, outcome);
                lock(&inner.running).remove(&call);
            })
        };
        running.insert(
            call,
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
            r.stop.set(Interruption::Stop);
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

impl Inner {
    fn send(&self, message: FromLimb) {
        // A brain that has gone hears nothing; the record still has it.
        let _ = self.out.send(message);
    }

    fn settle(&self, call: CallId, outcome: Outcome) {
        let entry = Entry {
            instance: self.instance,
            outcome: Some(outcome.clone()),
        };
        // If the record cannot be written, a recovery says the outcome is
        // unknown, which is still true.
        let _ = self.calls.put(call, &entry);
        self.send(FromLimb::Settled { call, outcome });
    }

    fn recover(&self, call: CallId) {
        if lock(&self.running).contains_key(&call) {
            return;
        }
        match self.calls.get(call) {
            Ok(Some(Entry {
                outcome: Some(outcome),
                ..
            })) => self.send(FromLimb::Settled { call, outcome }),
            _ => self.send(FromLimb::Unknown { call }),
        }
    }

    fn describe(&self) -> Description {
        let mut instructions = vec![(
            text_block(tools::INSTRUCTIONS),
            Observation {
                resource: self.key(ResourceKind::Instructions, ""),
                version: Version::Is(Digest::of(tools::INSTRUCTIONS.as_bytes())),
            },
        )];
        if let Ok(agents) = std::fs::read_to_string(self.root.join("AGENTS.md")) {
            instructions.push((
                text_block(&format!("Contents of AGENTS.md:\n\n{agents}")),
                Observation {
                    resource: self.key(ResourceKind::Instructions, "AGENTS.md"),
                    version: Version::Is(Digest::of(agents.as_bytes())),
                },
            ));
        }
        Description {
            instructions,
            tools: tools::tools(),
        }
    }

    fn key(&self, kind: ResourceKind, name: &str) -> ResourceKey {
        ResourceKey {
            limb: self.limb,
            kind,
            name: name.into(),
        }
    }

    /// Every resource a file is: the file, and the root's instructions when
    /// it is the root's `AGENTS.md`.
    fn keys(&self, name: &str) -> Vec<ResourceKey> {
        let mut keys = vec![self.key(ResourceKind::File, name)];
        if name == "AGENTS.md" {
            keys.push(self.key(ResourceKind::Instructions, name));
        }
        keys
    }

    fn version_of(&self, resource: &ResourceKey) -> Option<Version> {
        match resource.kind {
            ResourceKind::File | ResourceKind::Instructions if !resource.name.is_empty() => {
                files::version(&self.root.join(&resource.name)).ok()
            }
            ResourceKind::Instructions => {
                Some(Version::Is(Digest::of(tools::INSTRUCTIONS.as_bytes())))
            }
            _ => None,
        }
    }

    /// Tell the brain a file moved, as written by one of this limb's calls.
    fn changed(&self, name: &str, version: Version) {
        for resource in self.keys(name) {
            if let Some(known) = lock(&self.watched).get_mut(&resource) {
                *known = version;
            }
            self.send(FromLimb::Changed(Change {
                resource,
                version,
                by: None,
            }));
        }
    }

    /// After a call: report every watched resource that moved, which is how
    /// a command's writes reach the contexts that read them.
    fn rescan(&self) {
        let watched: Vec<(ResourceKey, Version)> = lock(&self.watched)
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for (resource, known) in watched {
            let Some(version) = self.version_of(&resource) else {
                continue;
            };
            if version != known {
                lock(&self.watched).insert(resource.clone(), version);
                self.send(FromLimb::Changed(Change {
                    resource,
                    version,
                    by: None,
                }));
            }
        }
    }
}

fn text_block(text: &str) -> Wire {
    Wire(json!({ "type": "text", "text": text }).to_string())
}

/// An id no other instance has had: this process, and this moment.
fn new_instance() -> InstanceId {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seed = format!("{}:{now}", std::process::id());
    let digest = Digest::of(seed.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest.0[..16]);
    InstanceId(u128::from_le_bytes(bytes))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
