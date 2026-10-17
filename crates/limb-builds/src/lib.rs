//! The builds limb: relaunches its brain onto the commit of a clone, as a
//! call of the harness's core model, and goes back when the new build
//! fails ([`harness_core::build`] holds the model).
//!
//! **Concept.** One per brain, a [`Limb`] of the `builds` kind whose
//! resource is the brain's own service, the systemd user unit its
//! definition names ([`Settings`]). Its `relaunch` tool refuses in band a
//! clone with uncommitted changes, a commit that does not descend from the
//! running one unless the call says `back`, and a second relaunch while
//! one is under way. Otherwise it builds and installs the commit, stops the
//! brain (it drains), starts the new build on the same database, and
//! settles the call: the settlement wakes the caller, whose next request is
//! the new build's first. Its result observes the service, so the limb
//! watching the new build ([`relaunch::Takeover::watch`]) and going back
//! is a change of that resource, saying why, at the caller's next turn.
//!
//! The limb is a process of the build that was running, and outlives the
//! brain it relaunches, so the way back never runs the new build's code.
//! It runs as a server of its own ([`harness_limb`]), `harness-limb-builds`,
//! whose command line also relaunches a unit by hand (`relaunch`), and
//! installs and starts a build for a brain's first launch (`install`,
//! `start`): what `dev/serve` runs.

pub mod relaunch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Outcome, Output};
use harness_core::environment::{
    Audience, Description, Host, Instruction, Limb, LimbDown, LimbKey, LimbKind, LimbRow, Tool,
};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, Digest, LimbId, Timestamp};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_limb::Out;
use harness_store::Store;
use harness_substrate::{Log, Mirror, Restored, Up};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::relaunch::Ended;

/// The plugin kind this limb is.
pub const KIND: &str = "builds";
/// Its one tool.
pub const RELAUNCH: &str = "relaunch";
/// The kind of its resource: the brain's service.
const SERVICE: &str = "service";

/// A builds limb's own settings: the `config` of its [`Limb`] definition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The brain's systemd user unit, `harness-NAME.service`. `None` for a
    /// brain no unit runs, such as a test brain: it cannot be relaunched.
    pub unit: Option<String>,
}

impl Settings {
    /// The settings for the brain this process is: the unit whose cgroup
    /// it is in, if that is a brain's.
    pub fn here() -> Settings {
        let cgroup = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
        let unit = cgroup
            .lines()
            .filter_map(|l| l.rsplit('/').next())
            .find(|u| u.starts_with("harness-") && u.ends_with(".service"))
            .map(String::from);
        Settings { unit }
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
}

/// One running instance of the builds limb.
pub struct Builds {
    inner: Arc<Inner>,
    rows: Mutex<Mirror<LimbKey, LimbRow, Refusal>>,
}

struct Inner {
    limb: LimbId,
    settings: Mutex<Settings>,
    state: PathBuf,
    log: Mutex<Log<Harness, Store>>,
    out: Out,
    /// The relaunch running, through its watch, by call.
    running: Mutex<HashMap<CallId, JoinHandle<()>>>,
    /// The brain would start another build of this limb: exit once no
    /// relaunch runs.
    superseded: AtomicBool,
    shutting_down: AtomicBool,
}

impl harness_limb::Kind for Builds {
    fn open(
        definition: &Limb,
        log: Log<Harness, Store>,
        state: &Path,
        out: Out,
    ) -> Result<Builds, String> {
        let settings = Settings::of(definition).ok_or("the definition is not a builds limb's")?;
        let inner = Arc::new(Inner {
            limb: definition.id,
            settings: Mutex::new(settings),
            state: state.to_path_buf(),
            log: Mutex::new(log),
            out,
            running: Mutex::default(),
            superseded: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
        });
        std::fs::create_dir_all(inner.marks()).map_err(|e| e.to_string())?;
        Ok(Builds {
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
        if let Some(LimbRow::Definition(definition)) = view.rows.get(&LimbKey::Definition)
            && let Some(settings) = Settings::of(definition)
        {
            *lock(&inner.settings) = settings;
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
                (LimbKey::Call(id), LimbRow::Call { call, stopping, .. }) => {
                    let input = match view.rows.get(&LimbKey::Value(call.input)) {
                        Some(LimbRow::Value(bytes)) => Some(bytes.clone()),
                        _ => None,
                    };
                    Inner::call(inner, *id, call, input, stopping.is_some());
                }
                (LimbKey::Superseded, LimbRow::Superseded { .. }) => {
                    inner.superseded.store(true, Ordering::SeqCst);
                }
                _ => {}
            }
        }
        inner.exit_if_superseded();
        Ok(ship)
    }

    fn stop_all(&self, _within: Duration) {
        // A relaunch under way is not stopped: stopping the brain mid-way
        // is the one thing it must not leave half done. The limb exits
        // once it has ended.
        self.inner.shutting_down.store(true, Ordering::SeqCst);
    }

    fn join(&self) {
        loop {
            let running = lock(&self.inner.running).drain().next();
            let Some((_, thread)) = running else { break };
            let _ = thread.join();
        }
    }
}

impl Inner {
    fn marks(&self) -> PathBuf {
        self.state.join("started")
    }

    /// One call row: run it at most once, however often it arrives. A
    /// relaunch the limb has a mark of and no thread for was cut off when
    /// an instance of the limb ended: what it did is unknown.
    fn call(self: &Arc<Self>, id: CallId, call: &Call, input: Option<Bytes>, stopping: bool) {
        let mut running = lock(&self.running);
        if running.contains_key(&id) || self.settled(id) {
            return;
        }
        let mut report = Report::default();
        let mark = self.marks().join(id.to_string());
        if mark.exists() {
            let outcome = report.failed(
                "the builds limb ended while this relaunch was under way: \
                 what it did is unknown; the brain's view says which build runs",
            );
            return self.settle(id, report, outcome);
        }
        if stopping {
            return self.settle(id, report, Outcome::Interrupted { partial: None });
        }
        if self.shutting_down.load(Ordering::SeqCst) {
            return;
        }
        let request = match (call.tool.as_str(), input) {
            (RELAUNCH, Some(input)) => {
                serde_json::from_slice::<Request>(&input.0).map_err(|e| format!("the input: {e}"))
            }
            (RELAUNCH, None) => Err("the brain sent the call without its input".into()),
            (other, _) => Err(format!("the builds limb has no tool `{other}`")),
        };
        let unit = lock(&self.settings).unit.clone();
        let (request, unit) = match (request, unit) {
            (Ok(request), Some(unit)) => (request, unit),
            (Err(why), _) => {
                let outcome = report.failed(&why);
                return self.settle(id, report, outcome);
            }
            (Ok(_), None) => {
                let outcome =
                    report.failed("no systemd unit runs this brain, so it cannot be relaunched");
                return self.settle(id, report, outcome);
            }
        };
        if let Err(e) = std::fs::write(&mark, []) {
            let outcome = report.failed(&format!("cannot record the call: {e}"));
            return self.settle(id, report, outcome);
        }
        let inner = self.clone();
        let thread = thread::spawn(move || {
            inner.relaunch(id, &unit, &request);
            lock(&inner.running).remove(&id);
            inner.exit_if_superseded();
        });
        running.insert(id, thread);
    }

    fn relaunch(&self, id: CallId, unit: &str, request: &Request) {
        let say = |line: &str| eprintln!("{unit}: {line}");
        let mut report = Report::default();
        let takeover = match relaunch::take_over(unit, &request.clone, request.back, &say) {
            Ok(takeover) => takeover,
            Err(why) => {
                let outcome = report.failed(&why);
                return self.settle(id, report, outcome);
            }
        };
        let (old, new) = (takeover.old.commit.clone(), takeover.new.commit.clone());
        let said = format!(
            "{unit} runs {new} now, which took over from {old}: this session carries on, on it. \
             It is on probation until it has had a response and a call settled by a limb; until then, \
             a refused request, its process ending, or a request unanswered for 5 minutes makes the harness \
             go back to {old}, and you will be told why as a change of `{unit}`."
        );
        let outcome = report.completed(&said, vec![self.observe(unit, &new)]);
        self.settle(id, report, outcome);
        match takeover.watch() {
            Ended::Proven => say(&format!("{new} is proven")),
            Ended::Back(why) => {
                say(&why);
                let content = Bytes(why.into_bytes());
                let observed = self.observe(unit, &old);
                self.say(Batch {
                    facts: vec![Fact::Change(Change {
                        resource: observed.resource,
                        version: observed.version,
                        by: None,
                        content: Some(content.id()),
                    })],
                    values: vec![content],
                });
            }
        }
    }

    /// The service as running `commit`.
    fn observe(&self, unit: &str, commit: &str) -> Observation {
        Observation {
            resource: ResourceKey {
                limb: self.limb,
                kind: SERVICE.into(),
                name: unit.into(),
            },
            version: Version::Is(Digest::of(commit.as_bytes())),
        }
    }

    fn exit_if_superseded(&self) {
        if self.superseded.load(Ordering::SeqCst) && lock(&self.running).is_empty() {
            eprintln!("superseded by another build of this limb: exiting");
            // The limb's own shutdown, as for any signal.
            // SAFETY: raise takes a signal number.
            unsafe { libc::raise(libc::SIGTERM) };
        }
    }

    fn describe(&self) -> Description {
        let unit = lock(&self.settings).unit.clone();
        let text = format!(
            "## Relaunching the harness\n\n\
`relaunch` moves the brain you run on ({}) onto the commit a clone has checked out: it builds and installs \
it, the brain stops (its requests in flight finish first), and the new build carries on every session, \
yours included, from the same records. Merge your work first: it refuses a clone with uncommitted changes, \
and a commit that does not descend from the one running unless you say `back`. Relaunch only at a moment \
no other agent's work would be lost. A broken build bricks nothing: until it is proven the harness watches it \
from the build before, and goes back on its own.\n",
            unit.as_deref()
                .unwrap_or("no systemd unit: it cannot be relaunched")
        );
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
            tools: vec![tool()],
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
        let _ = std::fs::remove_file(self.marks().join(call.to_string()));
    }
}

/// A `relaunch` call's input.
#[derive(Deserialize)]
struct Request {
    clone: PathBuf,
    #[serde(default)]
    back: bool,
}

fn tool() -> Tool {
    let definition = json!({
        "name": RELAUNCH,
        "description": "Relaunch the harness onto the commit a clone has checked out. It returns once the new build \
    has taken over; your next turn is its first request.",
        "input_schema": { "type": "object", "properties": {
            "clone": { "type": "string", "description": "The clone's directory: its HEAD is built." },
            "back": { "type": "boolean", "description": "Allow a commit that does not descend from the running one." },
        }, "required": ["clone"] },
    });
    Tool {
        name: RELAUNCH.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
    }
}

/// The bytes one call says: the values its outcome names.
#[derive(Default)]
struct Report {
    values: Vec<Bytes>,
}

impl Report {
    fn text(&mut self, text: &str) -> harness_core::id::ValueId {
        let bytes = Bytes(
            json!([{ "type": "text", "text": text }])
                .to_string()
                .into_bytes(),
        );
        let id = bytes.id();
        self.values.push(bytes);
        id
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
