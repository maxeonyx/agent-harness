//! What a limb says, and how: its records, stored in its own log and then
//! shipped; the brain's rows for it, as they stand; and its tool sets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Outcome, Output, Owner};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::Digest;
use harness_core::id::{CallId, LimbId, ReplicaId, Timestamp, ValueId};
use harness_core::limb;
use harness_core::limb::{Defined, Description, Instruction, LimbDown, LimbKey, LimbRow, Offer};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_store::Store;
use harness_substrate::{Log, Mirror, Restored, Up, View, What};
use serde_json::json;

use crate::set::{Open, Opening, Running};
use crate::{Calls, Out, ToolSet, guard};

/// The brain's rows for a limb, as they stand.
pub type Rows = View<LimbKey, LimbRow, Refusal>;

/// A limb's records: what its tool sets say, each stored, then shipped.
pub struct Voice {
    limb: LimbId,
    log: Mutex<Log<Harness, Store>>,
    out: Out,
}

impl Voice {
    pub fn limb(&self) -> LimbId {
        self.limb
    }

    /// The replica its records are of, the same across instances on one
    /// state.
    pub fn here(&self) -> ReplicaId {
        lock(&self.log).here()
    }

    /// Say a batch at the limb's replica: stored, then shipped.
    pub fn say(&self, batch: Batch) {
        let record = lock(&self.log).say(now(), |_| batch);
        match record {
            Ok(record) => (self.out)(Up::Record(record)),
            // A call left unsettled is found started and cut off by the
            // next instance, which is still true.
            Err(e) => eprintln!("cannot write the limb's log: {e}"),
        }
    }

    /// Describe the limb: what a new link hears first.
    pub fn describe(&self, description: Description) {
        self.say(Batch {
            values: Vec::new(),
            facts: vec![Fact::Described {
                limb: self.limb,
                description,
            }],
        });
    }

    /// Ship what a running call has output so far.
    pub fn live(&self, live: Live) {
        let after = lock(&self.log).records().last().map(|r| r.at);
        if let Some(after) = after {
            (self.out)(Up::Live(harness_substrate::Live { after, live }));
        }
    }

    /// Whether the limb has said how the call ended and the brain has not
    /// yet included it.
    pub fn settled(&self, call: CallId) -> bool {
        lock(&self.log).records().any(|r| match &r.what {
            What::Said(batch) => batch
                .facts
                .iter()
                .any(|f| matches!(f, Fact::Call(id, CallEvent::Settled(_)) if *id == call)),
            What::Included(_) | What::Unread(_) => false,
        })
    }

    /// Say how the report's call ended.
    pub fn settle(&self, report: Report, outcome: Outcome) {
        match report.call {
            Some(call) => {
                self.say(report.batch(vec![Fact::Call(call, CallEvent::Settled(outcome))]))
            }
            None => eprintln!("a settlement of no call: {outcome:?}"),
        }
    }

    /// The call rows among `rows`.
    pub fn calls<'a>(&self, rows: &'a Rows) -> Vec<CallRow<'a>> {
        let here = self.here();
        let row = |key: &LimbKey, row: &'a LimbRow| match (key, row) {
            (
                LimbKey::Call(id),
                LimbRow::Call {
                    call,
                    stopping,
                    first_sent_to,
                    owner,
                    dir,
                },
            ) => Some(CallRow {
                id: *id,
                call,
                input: match rows.rows.get(&LimbKey::Value(call.input)) {
                    Some(LimbRow::Value(bytes)) => Some(bytes),
                    _ => None,
                },
                stopping: *stopping,
                owner: *owner,
                dir: dir.as_deref(),
                earlier: *first_sent_to != here,
            }),
            _ => None,
        };
        rows.rows.iter().filter_map(|(k, r)| row(k, r)).collect()
    }
}

/// One call row, as a kind runs it.
pub struct CallRow<'a> {
    pub id: CallId,
    pub call: &'a Call,
    /// Its input's bytes, if the brain shipped them.
    pub input: Option<&'a Bytes>,
    /// Stop it, gone by then.
    pub stopping: Option<Timestamp>,
    pub owner: Option<Owner>,
    /// The caller's working directory, as the brain named it.
    pub dir: Option<&'a Path>,
    /// It was first sent to an instance of this limb whose state is lost,
    /// which may have run it.
    pub earlier: bool,
}

/// What one call, or one report of changes, says: the bytes its output
/// names and the resources that changed.
pub struct Report {
    call: Option<CallId>,
    values: Vec<Bytes>,
    changes: Vec<Change>,
}

impl Report {
    pub fn new(call: Option<CallId>) -> Report {
        Report {
            call,
            values: Vec::new(),
            changes: Vec::new(),
        }
    }

    pub fn call(&self) -> Option<CallId> {
        self.call
    }

    pub fn value(&mut self, bytes: Vec<u8>) -> ValueId {
        let bytes = Bytes(bytes);
        let id = bytes.id();
        self.values.push(bytes);
        id
    }

    /// A tool result's content: one text block.
    pub fn text(&mut self, text: &str) -> ValueId {
        self.value(
            json!([{ "type": "text", "text": text }])
                .to_string()
                .into_bytes(),
        )
    }

    pub fn output(&mut self, said: &str, observes: Vec<Observation>) -> Output {
        Output {
            model: self.text(said),
            full: None,
            observes,
        }
    }

    pub fn completed(&mut self, said: &str, observes: Vec<Observation>) -> Outcome {
        Outcome::Completed(self.output(said, observes))
    }

    pub fn failed(&mut self, message: &str) -> Outcome {
        Outcome::Failed(self.output(message, Vec::new()))
    }

    /// `resource` is now at `version`, by the report's call when `by`.
    pub fn changed(&mut self, resource: ResourceKey, version: Version, by: bool) {
        self.changes.retain(|c| c.resource != resource);
        self.changes.push(Change {
            resource,
            version,
            by: self.call.filter(|_| by),
            content: None,
            wakes: false,
        });
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// The facts, led by the settlement when there is one.
    pub fn batch(self, settled: Vec<Fact>) -> Batch {
        Batch {
            values: self.values,
            facts: settled
                .into_iter()
                .chain(self.changes.into_iter().map(Fact::Change))
                .collect(),
        }
    }
}

/// One running instance of a limb: its records, the brain's rows for it,
/// its tool sets, and its calls.
pub struct Instance {
    voice: Arc<Voice>,
    calls: Calls,
    state: PathBuf,
    /// The tool sets it can open, by name.
    known: &'static [(&'static str, Open)],
    /// Each set its definition offers, by name.
    sets: Mutex<BTreeMap<String, Opened>>,
    rows: Mutex<Mirror<LimbKey, LimbRow, Refusal>>,
}

/// A tool set opened from its offer, or why it could not be.
struct Opened {
    config: String,
    set: Result<Arc<dyn ToolSet>, String>,
    /// Its tools, by name, as it last described them.
    tools: Vec<String>,
}

impl Instance {
    /// An instance of `limb` on its records `log`, kept in `state`, which
    /// ships what it says through `out` and opens the sets `known`.
    pub fn open(
        limb: LimbId,
        log: Log<Harness, Store>,
        state: &Path,
        out: Out,
        known: &'static [(&'static str, Open)],
    ) -> std::io::Result<Instance> {
        let voice = Arc::new(Voice {
            limb,
            log: Mutex::new(log),
            out,
        });
        Ok(Instance {
            calls: Calls::open(voice.clone(), &state.join("calls/started"))?,
            voice,
            state: state.to_path_buf(),
            known,
            sets: Mutex::default(),
            rows: Mutex::default(),
        })
    }

    pub fn replica(&self) -> ReplicaId {
        self.voice.here()
    }

    /// Take in what the brain shipped, and act on the rows, returning the
    /// records to ship that the brain's standing does not cover. A whole
    /// view begins a link: the records it lacks are shipped first, and the
    /// limb describes its sets. Its standing past the log means the state
    /// was lost.
    pub fn apply(&self, down: LimbDown) -> Result<Vec<Record>, Restored> {
        let connected = matches!(down, LimbDown::View(_));
        let mut rows = lock(&self.rows);
        rows.apply(down);
        let Some(view) = rows.view() else {
            return Ok(Vec::new());
        };
        let mut ship = Vec::new();
        {
            let mut log = lock(&self.voice.log);
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
        let changed = match view.rows.get(&LimbKey::Definition) {
            Some(LimbRow::Definition(definition)) => self.define(definition, connected),
            _ => false,
        };
        let sets = self.opened();
        for (name, set, _) in &sets {
            if let Err(why) = guard(|| set.apply(view, connected)) {
                eprintln!("the tool set `{name}`, taking the brain's rows: {why}");
            }
        }
        if connected || changed {
            self.describe();
        }
        for row in self.voice.calls(view) {
            self.dispatch(row, &sets);
        }
        self.calls.forget(view);
        Ok(ship)
    }

    /// Open each set its definition offers that is not open, give each
    /// open one its offer's settings, and close those no longer offered.
    /// Whether any of that changed what it offers.
    fn define(&self, definition: &Defined, connected: bool) -> bool {
        let offers: &[Offer] = match definition {
            Defined::Limb(limb) => &limb.offers,
            Defined::Legacy(_) => {
                if connected {
                    eprintln!(
                        "the brain defines this limb as one from before limbs were per machine, \
                         which this build cannot run: it offers nothing"
                    );
                }
                &[]
            }
        };
        let mut sets = lock(&self.sets);
        let before = sets.len();
        sets.retain(|name, _| offers.iter().any(|o| o.set == *name));
        let mut changed = sets.len() != before;
        for offer in offers {
            match sets.get_mut(&offer.set) {
                Some(opened) if opened.config == offer.config => {}
                Some(opened) => {
                    changed = true;
                    opened.config = offer.config.clone();
                    match &opened.set {
                        Ok(set) => {
                            if let Err(why) = guard(|| set.define(&offer.config)).and_then(|r| r) {
                                eprintln!(
                                    "the tool set `{}`, taking new settings: {why}",
                                    offer.set
                                );
                            }
                        }
                        Err(_) => *opened = self.open_set(offer),
                    }
                }
                None => {
                    changed = true;
                    sets.insert(offer.set.clone(), self.open_set(offer));
                }
            }
        }
        changed
    }

    fn open_set(&self, offer: &Offer) -> Opened {
        let state = self.state.join(&offer.set);
        let set = match self.known.iter().find(|(name, _)| *name == offer.set) {
            None => Err(format!(
                "this build of the limb has no tool set `{}`",
                offer.set
            )),
            Some((_, open)) => std::fs::create_dir_all(&state)
                .map_err(|e| format!("{}: {e}", state.display()))
                .and_then(|()| {
                    let opening = Opening {
                        voice: self.voice.clone(),
                        state,
                    };
                    guard(|| open(offer, &opening)).and_then(|r| r)
                }),
        };
        let tools = match &set {
            Ok(set) => guard(|| set.describe())
                .map(|d| d.tools.into_iter().map(|t| t.name).collect())
                .unwrap_or_default(),
            Err(why) => {
                eprintln!("the tool set `{}` cannot open: {why}", offer.set);
                Vec::new()
            }
        };
        Opened {
            config: offer.config.clone(),
            set,
            tools,
        }
    }

    /// The sets that opened, with their tools.
    fn opened(&self) -> Vec<(String, Arc<dyn ToolSet>, Vec<String>)> {
        lock(&self.sets)
            .iter()
            .filter_map(|(name, o)| Some((name.clone(), o.set.clone().ok()?, o.tools.clone())))
            .collect()
    }

    /// Say what the limb offers: each set as it describes itself, and a set
    /// that could not open or describe itself with why, and no tools.
    fn describe(&self) {
        let mut sets = lock(&self.sets);
        let mut described = Vec::new();
        for (name, opened) in sets.iter_mut() {
            let set = opened
                .set
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|set| guard(|| set.describe()));
            let set = match set {
                Ok(set) => set,
                Err(why) => self.unavailable(name, &why),
            };
            opened.tools = set.tools.iter().map(|t| t.name.clone()).collect();
            described.push(set);
        }
        drop(sets);
        self.voice.describe(Description { sets: described });
    }

    /// A set that cannot be used here, described by why.
    fn unavailable(&self, name: &str, why: &str) -> limb::ToolSet {
        let text = format!("The tool set `{name}` is not available here: {why}");
        limb::ToolSet {
            name: name.into(),
            instructions: vec![Instruction {
                observes: Observation {
                    resource: ResourceKey {
                        limb: self.voice.limb(),
                        kind: "instructions".into(),
                        name: name.into(),
                    },
                    version: Version::Is(Digest::of(text.as_bytes())),
                },
                text,
            }],
            tools: Vec::new(),
        }
    }

    /// Run a call row with the set that has its tool.
    fn dispatch(&self, row: CallRow<'_>, sets: &[(String, Arc<dyn ToolSet>, Vec<String>)]) {
        let tool = row.call.tool.clone();
        let Some((_, set, _)) = sets.iter().find(|(_, _, tools)| tools.contains(&tool)) else {
            return self.calls.call(row, true, move |_, _, _, report| {
                report.failed(&format!("this limb offers no tool `{tool}`"))
            });
        };
        let set = set.clone();
        let rerun = guard(|| set.rerun(&tool)).unwrap_or(false);
        let (id, call, owner) = (row.id, row.call.clone(), row.owner);
        let dir = row.dir.map(Path::to_path_buf);
        let shutting_down = self.calls.shutting_down_flag();
        self.calls
            .call(row, rerun, move |input, cancel, limit, report| {
                let running = Running {
                    id,
                    call: &call,
                    input: &input.0,
                    owner,
                    dir: dir.as_deref(),
                    cancel,
                    limit,
                    shutting_down: &shutting_down,
                };
                set.run(&running, report)
            });
    }

    /// Whether anything would die with the process: a running call, what a
    /// set holds, or a process the limb started (a clone being made).
    pub fn holds(&self) -> bool {
        self.calls.any()
            || self
                .opened()
                .iter()
                .any(|(_, set, _)| guard(|| set.holds()).unwrap_or(false))
            || !harness_runtime::children().is_empty()
    }

    /// Stop every call, each to be gone `within` from now, and start
    /// nothing new. Asked again with less time, stop harder.
    pub fn stop_all(&self, within: Duration) {
        self.calls.stop_all(within);
    }

    /// Wait until every call has settled, then close every set.
    pub fn join(&self) {
        self.calls.join();
        for (name, set, _) in self.opened() {
            if let Err(why) = guard(|| set.close()) {
                eprintln!("the tool set `{name}`, closing: {why}");
            }
        }
    }
}

/// The wall clock, as records carry it.
pub fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX)),
    )
}

/// The moment a record's clock names.
pub fn instant(at: Timestamp) -> Instant {
    Instant::now() + Duration::from_millis(u64::try_from(at.0.saturating_sub(now().0)).unwrap_or(0))
}

/// A mutex's guard, whether or not a holder panicked.
pub fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
