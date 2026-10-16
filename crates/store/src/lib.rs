//! The durable home of one brain's journal and of the immutable values its
//! facts name, in one SQLite file: the core's [`journal::Store`].
//!
//! The journal is the only truth, and [`journal::Journal`] is its one
//! writer: it stamps, checks and folds each batch before handing it here.
//! So the store does three things and nothing else: it appends a batch's
//! values and records in one transaction, it gives the records back in
//! order (to fold, or to a face catching up a page at a time), and it gives
//! a value's bytes back by [`ValueId`].
//!
//! # Schema
//!
//! ```sql
//! record(seq, at, kind, subject, by, fact)  -- the journal
//! value(id, bytes)                          -- every byte it names
//! ```
//!
//! A record keeps its fact as the core's own JSON, exactly as serialised, so
//! the facts stay the only compatibility surface: a change to how the core
//! folds them never migrates a row. Beside it are the columns its queries
//! need an index on: `kind`, the fact's variant (`"Task"`, `"ModelCall"`,
//! ...), and `subject`, the identity it belongs to, as the same hex string
//! the fact holds it as:
//!
//! ```sql
//! -- Tokens per task: each request's usage, joined to the task it was for.
//! SELECT sent.fact ->> '$.ModelCall[1].Sent.task' AS task,
//!        sum(ended.fact ->> '$.ModelCall[1].Ended.usage.input') AS input,
//!        sum(ended.fact ->> '$.ModelCall[1].Ended.usage.output') AS output
//! FROM record AS ended
//! JOIN record AS sent ON sent.subject = ended.subject
//!   AND sent.fact -> '$.ModelCall[1].Sent' IS NOT NULL
//! WHERE ended.kind = 'ModelCall'
//!   AND ended.fact -> '$.ModelCall[1].Ended' IS NOT NULL
//! GROUP BY task;
//! ```
//!
//! Values sit apart from the journal, so the large bytes (system prompts,
//! tool schemas, messages, tool output) never sit in the rows a fold or a
//! range scan reads.
//!
//! # Durability
//!
//! Write-ahead log, with a full sync at each commit: an append that
//! returned survives a crash of the process or the machine, and one that had
//! not returned is either wholly there or wholly absent. Readers in other
//! processes (an analytics query in the `sqlite3` shell) never block the
//! brain.

use std::fmt;
use std::path::Path;

use harness_core::id::{Seq, Timestamp, ValueId};
use harness_core::journal::{self, Author, Fact, Record};
use harness_core::value::Bytes;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// Bumped by any change to the tables below; a store written by another
/// version is refused rather than misread.
const VERSION: i64 = 4;

const SCHEMA: &str = "
CREATE TABLE record (
    seq INTEGER PRIMARY KEY CHECK (seq > 0),
    at INTEGER NOT NULL,
    kind TEXT NOT NULL,
    subject TEXT,
    by TEXT NOT NULL,
    fact TEXT NOT NULL
) STRICT;
CREATE INDEX record_subject ON record (subject, seq) WHERE subject IS NOT NULL;
CREATE INDEX record_kind ON record (kind, seq);
CREATE INDEX record_at ON record (at);

CREATE TABLE value (
    id BLOB PRIMARY KEY,
    bytes BLOB NOT NULL
) STRICT;
";

/// One brain's store: its journal and the values the journal names.
///
/// The journal has one writer at a time ([`journal::Launched`]), so a store
/// is opened by one brain and is not shared between threads.
pub struct Store {
    db: Connection,
}

impl Store {
    /// Open the store at `path`, creating it if it does not exist.
    pub fn open(path: impl AsRef<Path>) -> Result<Store, Error> {
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "user_version", VERSION)?;
            }
            VERSION => {}
            found => return Err(Error::Version { found }),
        }
        tx.commit()?;
        Ok(Store { db })
    }

    /// Up to `limit` records after position `after`, in order: how a face
    /// that has seen the journal up to `after` catches up, a page at a time.
    /// `Seq(0)` reads from the start.
    pub fn read(&self, after: Seq, limit: u64) -> Result<Vec<Record>, Error> {
        let mut records = Vec::new();
        self.each(after, limit, &mut |record| records.push(record))?;
        Ok(records)
    }

    /// The bytes named `id`, if they are stored.
    pub fn value(&self, id: ValueId) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .db
            .prepare_cached("SELECT bytes FROM value WHERE id = ?")?
            .query_row([id.0.0], |row| row.get(0))
            .optional()?)
    }

    /// Call `f` on up to `limit` records after `after`, in order, without
    /// holding more than one in memory.
    fn each(&self, after: Seq, limit: u64, f: &mut dyn FnMut(Record)) -> Result<(), Error> {
        let mut select = self.db.prepare_cached(
            "SELECT seq, at, by, fact FROM record WHERE seq > ? ORDER BY seq LIMIT ?",
        )?;
        // SQLite's LIMIT is signed; a negative one means no limit.
        let limit = i64::try_from(limit).unwrap_or(-1);
        let mut rows = select.query(params![after.0, limit])?;
        while let Some(row) = rows.next()? {
            let by: String = row.get(2)?;
            let fact: String = row.get(3)?;
            f(Record {
                seq: Seq(row.get(0)?),
                at: Timestamp(row.get(1)?),
                by: serde_json::from_str::<Author>(&by)?,
                fact: serde_json::from_str::<Fact>(&fact)?,
            });
        }
        Ok(())
    }
}

impl journal::Store for Store {
    type Error = Error;

    fn records(&self, each: &mut dyn FnMut(Record)) -> Result<(), Error> {
        self.each(Seq(0), u64::MAX, each)
    }

    fn has(&self, value: ValueId) -> Result<bool, Error> {
        Ok(self
            .db
            .prepare_cached("SELECT 1 FROM value WHERE id = ?")?
            .exists([value.0.0])?)
    }

    /// Also refuses records that do not continue the journal from its last
    /// stored position: a second writer on the same file, which only the
    /// store can see.
    fn append(&mut self, values: &[Bytes], records: &[Record]) -> Result<(), Error> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let last: u64 = tx.query_row("SELECT coalesce(max(seq), 0) FROM record", [], |row| {
            row.get(0)
        })?;
        let mut value = tx
            .prepare_cached("INSERT INTO value (id, bytes) VALUES (?, ?) ON CONFLICT DO NOTHING")?;
        for bytes in values {
            value.execute(params![bytes.id().0.0, &bytes[..]])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO record (seq, at, kind, subject, by, fact) VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        for (record, expected) in records.iter().zip(last + 1..) {
            if record.seq != Seq(expected) {
                return Err(Error::OutOfOrder {
                    expected: Seq(expected),
                    found: record.seq,
                });
            }
            let (kind, subject) = index(&record.fact);
            insert.execute(params![
                record.seq.0,
                record.at.0,
                kind,
                subject,
                serde_json::to_string(&record.by)?,
                serde_json::to_string(&record.fact)?,
            ])?;
        }
        drop((value, insert));
        tx.commit()?;
        Ok(())
    }
}

/// The variant of a fact and the identity it belongs to: the columns a
/// record is indexed by. The names are the variants' serde names, so they
/// match the top-level key of the record's JSON.
fn index(fact: &Fact) -> (&'static str, Option<String>) {
    match fact {
        Fact::Launched(_) => ("Launched", None),
        Fact::Task(id, _) => ("Task", Some(id.to_string())),
        Fact::ModelCall(id, _) => ("ModelCall", Some(id.to_string())),
        Fact::Call(id, _) => ("Call", Some(id.to_string())),
        Fact::Delivery(delivery) => ("Delivery", Some(delivery.id.to_string())),
        Fact::Change(_) => ("Change", None),
        Fact::Environment(environment) => ("Environment", Some(environment.id.to_string())),
        Fact::Limb(limb) => ("Limb", Some(limb.id.to_string())),
        Fact::Described { limb, .. } => ("Described", Some(limb.to_string())),
        Fact::Reach { limb, .. } => ("Reach", Some(limb.to_string())),
        Fact::Ranking(_) => ("Ranking", None),
        Fact::Focus(focus) => ("Focus", Some(focus.face.to_string())),
    }
}

/// Why the store could not do what was asked.
#[derive(Debug)]
pub enum Error {
    /// The database failed: the file, the disk, a lock held too long.
    Sqlite(rusqlite::Error),
    /// A stored record is not JSON the core can read: written by a build
    /// whose facts this one does not know.
    Json(serde_json::Error),
    /// The file was written by another version of this schema.
    Version { found: i64 },
    /// Records that do not continue the journal: another writer appended
    /// since this one folded.
    OutOfOrder { expected: Seq, found: Seq },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(error) => write!(f, "store database: {error}"),
            Error::Json(error) => write!(f, "stored record unreadable: {error}"),
            Error::Version { found } => {
                write!(f, "store schema version {found}, expected {VERSION}")
            }
            Error::OutOfOrder { expected, found } => write!(
                f,
                "records do not continue the journal: expected seq {}, found {}",
                expected.0, found.0
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(error) => Some(error),
            Error::Json(error) => Some(error),
            Error::Version { .. } | Error::OutOfOrder { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        Error::Sqlite(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Error {
        Error::Json(error)
    }
}
