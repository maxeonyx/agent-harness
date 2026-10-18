//! What every kind says, and how: its records, stored in its own log and
//! then shipped; and the brain's rows for it, as they stand.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::call::{Call, CallEvent, Outcome, Output, Owner};
use harness_core::environment::{Description, Host, Limb, LimbDown, LimbKey, LimbKind, LimbRow};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, LimbId, ReplicaId, Timestamp, ValueId};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_store::Store;
use harness_substrate::{Log, Mirror, Restored, Up, View, What};
use serde_json::json;

use crate::{Kind, Out};

/// The brain's rows for a limb, as they stand.
pub type Rows = View<LimbKey, LimbRow, Refusal>;

/// A limb's records: what its kind says, each stored, then shipped.
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
        let call = report.call.expect("a settlement is a call's");
        self.say(report.batch(vec![Fact::Call(call, CallEvent::Settled(outcome))]));
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

/// One running instance of a kind: the kind, its records, and the brain's
/// rows for it.
pub struct Instance<K> {
    kind: K,
    voice: Arc<Voice>,
    rows: Mutex<Mirror<LimbKey, LimbRow, Refusal>>,
}

impl<K: Kind> Instance<K> {
    /// Open the kind on its definition and its records `log`, kept in
    /// `state`; it ships what it says through `out`.
    pub fn open(
        definition: &Limb,
        log: Log<Harness, Store>,
        state: &Path,
        out: Out,
    ) -> Result<Instance<K>, String> {
        let voice = Arc::new(Voice {
            limb: definition.id,
            log: Mutex::new(log),
            out,
        });
        Ok(Instance {
            kind: K::open(definition, voice.clone(), state)?,
            voice,
            rows: Mutex::default(),
        })
    }

    pub fn kind(&self) -> &K {
        &self.kind
    }

    pub fn replica(&self) -> ReplicaId {
        self.voice.here()
    }

    /// Take in what the brain shipped, and have the kind act on the rows,
    /// returning the records to ship that the brain's standing does not
    /// cover. A whole view begins a link: the records it lacks are shipped
    /// first. Its standing past the log means the state was lost.
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
        self.kind.apply(view, connected);
        Ok(ship)
    }
}

/// A plugin's settings: the `config` of a definition of kind `kind`.
pub fn settings<T: serde::de::DeserializeOwned>(limb: &Limb, kind: &str) -> Option<T> {
    match &limb.kind {
        LimbKind::Plugin {
            kind: k, config, ..
        } if k == kind => serde_json::from_str(config).ok(),
        _ => None,
    }
}

/// The definition of a plugin limb of kind `kind` on this machine.
pub fn definition(id: LimbId, name: &str, kind: &str, settings: &impl serde::Serialize) -> Limb {
    Limb {
        id,
        name: name.into(),
        kind: LimbKind::Plugin {
            kind: kind.into(),
            config: serde_json::to_string(settings).expect("settings are JSON"),
            host: Host::Local,
        },
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

/// A mutex's guard, whether or not a holder panicked.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
