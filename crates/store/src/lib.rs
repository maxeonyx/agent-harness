//! A place's [`Storage`] in one SQLite file: the records it holds, the
//! values they name, and the place's own [`PlaceId`].
//!
//! The brain keeps its records here, and so does a limb process for its
//! own [`harness_substrate::Log`]. The substrate decides what to keep; the
//! store only holds it: [`Storage::commit`] puts and drops records in one
//! transaction, and [`Storage::load`] gives every record back.
//!
//! # Schema
//!
//! ```sql
//! meta(place)                       -- this place's id, minted on first open
//! record(place, n, clock, vals, record)
//! value(id, bytes)                  -- every byte a record names
//! ```
//!
//! `place` is the 16-hex [`PlaceId`] and `n` the record's number there, so
//! `(place, n)` is its [`Time`]. `record` is the substrate's JSON of the
//! [`Record`], without the bytes of a [`Batch`]'s `values`: those go to
//! `value` by [`ValueId`], and `vals` lists their ids in order (32 bytes
//! each), so the batch is put back whole on load. Values sit apart from the
//! records, so the large bytes (prompts, messages, tool output) are stored
//! once and never sit in the rows a load reads. A dropped record leaves its
//! values in place: collecting values is not built.
//!
//! # Durability
//!
//! Write-ahead log, with a full sync at each commit: a commit that returned
//! survives a crash of the process or the machine, and one that had not is
//! wholly there or wholly absent. A second connection to the same file (a
//! face reading value bytes, the `sqlite3` shell) never blocks the writer.

use std::fmt;
use std::io::Read;
use std::path::Path;

use harness_core::fact::Batch;
use harness_core::id::{Digest, PlaceId, Time, ValueId};
use harness_core::schema::{Harness, Record};
use harness_core::value::Bytes;
use harness_substrate::{Storage, What};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// Bumped by any change to the tables below; a file written by another
/// version is refused rather than misread.
const VERSION: i64 = 5;

const SCHEMA: &str = "
CREATE TABLE meta (
    place TEXT NOT NULL
) STRICT;

CREATE TABLE record (
    place TEXT NOT NULL,
    n INTEGER NOT NULL CHECK (n > 0),
    clock INTEGER NOT NULL,
    vals BLOB NOT NULL,
    record TEXT NOT NULL,
    PRIMARY KEY (place, n)
) STRICT, WITHOUT ROWID;

CREATE TABLE value (
    id BLOB PRIMARY KEY,
    bytes BLOB NOT NULL
) STRICT;
";

/// One place's storage. Opened by the one process of that place; other
/// connections to the file only read.
pub struct Store {
    db: Connection,
    place: PlaceId,
}

impl Store {
    /// Open the store at `path`, creating it, and minting its place, if it
    /// does not exist.
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
                tx.execute("INSERT INTO meta (place) VALUES (?)", [mint()?.to_string()])?;
                tx.pragma_update(None, "user_version", VERSION)?;
            }
            VERSION => {}
            found => return Err(Error::Version { found }),
        }
        let place: String = tx.query_row("SELECT place FROM meta", [], |row| row.get(0))?;
        tx.commit()?;
        let place = place.parse().map_err(|_| Error::Corrupt("meta.place"))?;
        Ok(Store { db, place })
    }

    /// The place whose records this is.
    pub fn place(&self) -> PlaceId {
        self.place
    }

    /// The bytes named `id`, if they are stored.
    pub fn value(&self, id: ValueId) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .db
            .prepare_cached("SELECT bytes FROM value WHERE id = ?")?
            .query_row([id.0.0], |row| row.get(0))
            .optional()?)
    }
}

impl Storage<Harness> for Store {
    type Error = Error;

    /// Every record, in `(place, n)` order.
    fn load(&self) -> Result<Vec<Record>, Error> {
        let mut select = self
            .db
            .prepare_cached("SELECT vals, record FROM record ORDER BY place, n")?;
        let mut rows = select.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let vals: Vec<u8> = row.get(0)?;
            let json: String = row.get(1)?;
            let mut record: Record = serde_json::from_str(&json)?;
            if let What::Said(batch) = &mut record.what {
                for id in vals.chunks(32) {
                    let id = ValueId(Digest(
                        id.try_into().map_err(|_| Error::Corrupt("record.vals"))?,
                    ));
                    let bytes = self.value(id)?.ok_or(Error::Missing(id))?;
                    batch.values.push(Bytes(bytes));
                }
            }
            records.push(record);
        }
        Ok(records)
    }

    fn commit(&mut self, put: &[Record], drop: &[Time]) -> Result<(), Error> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut remove = tx.prepare_cached("DELETE FROM record WHERE place = ? AND n = ?")?;
        for time in drop {
            remove.execute(params![time.place.to_string(), time.n])?;
        }
        let mut value = tx
            .prepare_cached("INSERT INTO value (id, bytes) VALUES (?, ?) ON CONFLICT DO NOTHING")?;
        let mut insert = tx.prepare_cached(
            "INSERT INTO record (place, n, clock, vals, record) VALUES (?, ?, ?, ?, ?)",
        )?;
        for record in put {
            let mut vals = Vec::new();
            let json = match &record.what {
                What::Said(batch) => {
                    for bytes in &batch.values {
                        let id = bytes.id();
                        value.execute(params![id.0.0, &bytes[..]])?;
                        vals.extend(id.0.0);
                    }
                    serde_json::to_string(&Record {
                        what: What::Said(Batch {
                            values: Vec::new(),
                            facts: batch.facts.clone(),
                        }),
                        ..*record
                    })?
                }
                What::Included(_) => serde_json::to_string(record)?,
            };
            insert.execute(params![
                record.at.place.to_string(),
                record.at.n,
                record.clock.0,
                vals,
                json
            ])?;
        }
        std::mem::drop((remove, value, insert));
        tx.commit()?;
        Ok(())
    }
}

/// A new place's id, from the OS's entropy.
fn mint() -> Result<PlaceId, Error> {
    let mut bytes = [0; 8];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(PlaceId(u64::from_le_bytes(bytes)))
}

/// Why the store could not do what was asked.
#[derive(Debug)]
pub enum Error {
    /// The database failed: the file, the disk, a lock held too long.
    Sqlite(rusqlite::Error),
    /// A stored record is not JSON the substrate can read: written by a
    /// build whose facts this one does not know.
    Json(serde_json::Error),
    /// The OS gave no entropy to mint the place's id.
    Entropy(std::io::Error),
    /// The file was written by another version of this schema.
    Version { found: i64 },
    /// A column does not hold what this schema writes there.
    Corrupt(&'static str),
    /// A record names a value the store does not hold.
    Missing(ValueId),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(error) => write!(f, "store database: {error}"),
            Error::Json(error) => write!(f, "stored record unreadable: {error}"),
            Error::Entropy(error) => write!(f, "no entropy for a new place id: {error}"),
            Error::Version { found } => {
                write!(f, "store schema version {found}, expected {VERSION}")
            }
            Error::Corrupt(column) => write!(f, "store column {column} is malformed"),
            Error::Missing(id) => write!(f, "a stored record names value {id}, not stored"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(error) => Some(error),
            Error::Json(error) => Some(error),
            Error::Entropy(error) => Some(error),
            Error::Version { .. } | Error::Corrupt(_) | Error::Missing(_) => None,
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

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Entropy(error)
    }
}
