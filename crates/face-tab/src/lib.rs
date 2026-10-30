//! A browser tab as a replica on the substrate.
//!
//! Each tab of Max's face is a follower of the brain: a [`ReplicaId`] with a
//! timeline of its own. What he does there is a record at the tab, written
//! to the tab's own storage first and shipped to the brain until the
//! brain's standing covers it; what he sees is a [`Mirror`] of the rows the
//! brain derives for the tab. This crate is that tab, the same code in the
//! page (built for `wasm32`, [`wasm`]) and in anything else that speaks for
//! Max over the link (a demo's script, [`Tab`] natively).
//!
//! - **Rows.** [`FaceKey`] and [`FaceRow`] are what the brain's face derives
//!   per tab and what the tab holds: one row per object, so a step ships
//!   only what changed. The face's server derives them; nothing else does.
//! - **Acts.** A tab's record is parsed into core's [`Batch`] here, at the
//!   boundary, and refused before it is stored if it does not parse: the
//!   page cannot ship a malformed fact. Whether the brain accepts it is the
//!   brain's to say, in the tab's [`Standing`].
//! - **Storage.** The page's storage is IndexedDB, which is asynchronous,
//!   and [`Storage`] is not. So a tab's storage is [`TabStore`]: the records
//!   in memory, and every change also queued as a [`Write`] for the
//!   embedding to make durable. The page stores the writes, waits for the
//!   transaction, and only then ships the record.
//!
//! **Vision.** "The browser is a replica"; "probably the shared
//! implementation is wise if we can make it low cost or zero cost";
//! "ideally, we only need to implement something once".

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use harness_core::fact::{Batch, Refusal};
use harness_core::id::{DeliveryId, ParseError, TaskId, Timestamp};
use harness_core::live::Live;
use harness_core::schema::{Harness, Record};
use harness_substrate::{Log, Mirror, ReplicaId, Restored, Standing, Status, Storage, Time};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// Which row of a tab's view: one per object the page draws.
///
/// Written as a string (`task:{id}`), since a JSON object's keys are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FaceKey {
    /// The harness as a whole: its build, its roles and working directories,
    /// the subscription's use.
    Harness,
    /// Max's queue, in order.
    Queue,
    Task(TaskId),
    Draft(DeliveryId),
}

/// One row of a tab's view.
///
/// The rows the page only draws are JSON, as the page reads them: the face
/// derives them from the fold and nothing in the tab looks inside. A
/// [`DraftRow`] is typed, since the tab reasons about drafts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FaceRow {
    Harness(Value),
    Queue(Value),
    Task(Value),
    Draft(DraftRow),
}

/// A draft of Max's beside a session, as every one of his tabs sees it.
///
/// **Concept.** The brain's [`harness_core::draft::Draft`], with its words.
/// A tab shows a draft while it can still be sent or has been held: once
/// sent, its words are a delivery in the session, and the row is gone.
/// Whether a Send of the tab's delivered, was held, or opened a new draft
/// is read off these rows and the tab's standing, the same on every device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftRow {
    pub session: TaskId,
    /// The newest text, in the brain's order: the content blocks, as JSON.
    pub text: Option<String>,
    /// Why its newest Send did not deliver, with its words kept.
    pub held: Option<String>,
    /// The draft whose Send, or concurrent edit, opened this one.
    pub from: Option<DeliveryId>,
    /// For an edit, the entry its Send moves the session back to.
    pub at: Option<harness_core::context::EntryId>,
}

/// What the brain ships a tab.
pub type Down = harness_substrate::Down<FaceKey, FaceRow, Refusal, Live>;
/// What a tab ships the brain.
pub type Up = harness_substrate::Up<Harness, Live>;
/// A tab's whole view.
pub type View = harness_substrate::View<FaceKey, FaceRow, Refusal>;

/// One change a tab's storage must make durable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Write {
    Put(Record),
    Drop(Time),
}

/// A tab's storage: its records in memory, each change queued for the
/// embedding to make durable before it ships anything.
#[derive(Clone, Default)]
pub struct TabStore {
    records: BTreeMap<Time, Record>,
    writes: Rc<RefCell<Vec<Write>>>,
}

impl TabStore {
    /// Storage holding what the embedding loaded.
    pub fn of(records: Vec<Record>) -> TabStore {
        TabStore {
            records: records.into_iter().map(|r| (r.at, r)).collect(),
            writes: Rc::default(),
        }
    }
}

impl Storage<Harness> for TabStore {
    type Error = Infallible;

    fn load(&self) -> Result<Vec<Record>, Infallible> {
        Ok(self.records.values().cloned().collect())
    }

    fn commit(&mut self, put: &[Record], drop: &[Time]) -> Result<(), Infallible> {
        let mut writes = self.writes.borrow_mut();
        for time in drop {
            self.records.remove(time);
            writes.push(Write::Drop(*time));
        }
        for record in put {
            self.records.insert(record.at, record.clone());
            writes.push(Write::Put(record.clone()));
        }
        Ok(())
    }
}

/// One tab: its log and its replica of the brain's rows.
pub struct Tab {
    log: Log<Harness, TabStore>,
    mirror: Mirror<FaceKey, FaceRow, Refusal>,
    writes: Rc<RefCell<Vec<Write>>>,
}

impl Tab {
    /// The tab at `here`, from what its storage held.
    pub fn open(here: ReplicaId, records: Vec<Record>) -> Tab {
        let storage = TabStore::of(records);
        let writes = Rc::clone(&storage.writes);
        let Ok(log) = Log::open(here, storage);
        Tab {
            log,
            mirror: Mirror::default(),
            writes,
        }
    }

    pub fn here(&self) -> ReplicaId {
        self.log.here()
    }

    /// The time the next record will have: what the facts it creates are
    /// named by.
    pub fn next(&self) -> Time {
        let n = self.log.records().last().map_or(0, |r| r.at.n);
        Time {
            replica: self.here(),
            n: n + 1,
        }
    }

    /// Say `batch`, as Max, having seen the view this tab holds: the
    /// record to ship once its writes are durable. `batch` is JSON in
    /// core's encoding; a [`harness_core::fact::Fact::Seen`] is added.
    pub fn say(&mut self, clock: Timestamp, batch: &str) -> Result<Record, serde_json::Error> {
        let mut batch: Batch = serde_json::from_str(batch)?;
        if let Some(view) = self.mirror.view() {
            batch.facts.push(harness_core::fact::Fact::Seen(view.as_of));
        }
        let Ok(record) = self.log.say(clock, |_| batch);
        Ok(record)
    }

    /// Take in what the brain shipped, and collect what its standing now
    /// covers. A [`Live`] is handed back for the page to show.
    pub fn down(&mut self, down: Down) -> Option<harness_substrate::Live<Live>> {
        if let harness_substrate::Down::Live(live) = down {
            return Some(live);
        }
        self.mirror.apply::<Live>(down);
        if let Some(view) = self.mirror.view() {
            let standing = view.standing.clone();
            let Ok(()) = self.log.cover(&standing);
        }
        None
    }

    /// What to ship on (re)connecting: every record the brain's standing,
    /// as of the view held, does not include.
    pub fn unsent(&self) -> Result<Vec<Record>, Restored> {
        let standing = self
            .mirror
            .view()
            .map_or_else(Standing::default, |v| v.standing.clone());
        self.log.unsent(&standing)
    }

    /// Where this tab's record `n` stands with the brain.
    pub fn status(&self, n: u64) -> Status<'_, Refusal> {
        match self.mirror.view() {
            Some(view) => view.standing.status(n),
            None => Status::Pending,
        }
    }

    /// The records held: those not yet included, and the newest.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.log.records()
    }

    pub fn view(&self) -> Option<&View> {
        self.mirror.view()
    }

    /// The writes made since the last call, for the embedding to make
    /// durable, in order.
    pub fn writes(&mut self) -> Vec<Write> {
        std::mem::take(&mut self.writes.borrow_mut())
    }

    /// Whether every record this tab said is included: a tab taken over
    /// from a closed one is retired once it is.
    pub fn settled(&self) -> bool {
        let newest = self.next().n - 1;
        matches!(self.mirror.view(), Some(v) if v.standing.upto >= newest)
    }
}

impl fmt::Display for FaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FaceKey::Harness => f.write_str("harness"),
            FaceKey::Queue => f.write_str("queue"),
            FaceKey::Task(id) => write!(f, "task:{id}"),
            FaceKey::Draft(id) => write!(f, "draft:{id}"),
        }
    }
}

impl FromStr for FaceKey {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<FaceKey, ParseError> {
        let invalid = ParseError {
            expected: "a row: harness, queue, task:{id} or draft:{id}",
        };
        Ok(match text.split_once(':') {
            None if text == "harness" => FaceKey::Harness,
            None if text == "queue" => FaceKey::Queue,
            Some(("task", id)) => FaceKey::Task(id.parse()?),
            Some(("draft", id)) => FaceKey::Draft(id.parse()?),
            _ => return Err(invalid),
        })
    }
}

impl Serialize for FaceKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FaceKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
