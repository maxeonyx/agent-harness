//! The durable home of one brain's journal and of the immutable values its
//! facts name, in one SQLite file.
//!
//! The journal is the only truth ([`harness_core::journal::Record`]), so the
//! store does three things and nothing else: it appends a batch of records
//! all or nothing, it gives them back in order (to fold into a
//! [`State`], or to a face catching up), and it keeps the values named by
//! their content: context entries by [`EntryId`], other bytes by
//! [`ContentId`]. A value is stored before the fact that names it
//! ([`Fact::values`]), and a message is stored after its parent: the store
//! refuses a batch naming a value it does not hold, and a message whose
//! parent it does not hold.
//!
//! # Schema
//!
//! ```sql
//! record(seq, at, kind, subject, by, fact)        -- the journal
//! entry(id, parent, model, system, tools, role, content)  -- the context tree
//! content(id, bytes)                              -- other values by digest
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
//! Entries are kept apart from the journal, one column per field, so the
//! large text (system prompts, tool schemas, message content) never sits in
//! the rows a fold or a range scan reads.
//!
//! # Durability
//!
//! Write-ahead log, with a full sync at each commit: a batch that
//! [`Store::append`] returned from survives a crash of the process or the
//! machine, and a batch it had not returned from is either wholly there or
//! wholly absent. Readers in other processes (an analytics query in the
//! `sqlite3` shell) never block the brain.

use std::fmt;
use std::path::Path;

use harness_core::context::{Entry, EntryId, Message, Role, Root, Wire};
use harness_core::id::{ContentId, Digest, Seq, Timestamp};
use harness_core::journal::{Author, Fact, Record, Rejected, Value};
use harness_core::state::State;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// Bumped by any change to the tables below; a store written by another
/// version is refused rather than misread.
const VERSION: i64 = 2;

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

CREATE TABLE entry (
    id BLOB PRIMARY KEY,
    parent BLOB REFERENCES entry (id),
    model TEXT,
    system TEXT,
    tools TEXT,
    role TEXT CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT,
    CHECK (
        (parent IS NULL AND model IS NOT NULL AND system IS NOT NULL AND tools IS NOT NULL
            AND role IS NULL AND content IS NULL)
        OR (parent IS NOT NULL AND model IS NULL AND system IS NULL AND tools IS NULL
            AND role IS NOT NULL AND content IS NOT NULL)
    )
) STRICT;
CREATE INDEX entry_parent ON entry (parent) WHERE parent IS NOT NULL;

CREATE TABLE content (
    id BLOB PRIMARY KEY,
    bytes BLOB NOT NULL
) STRICT;
";

/// One brain's store: its journal and the values the journal names.
///
/// The journal has one writer at a time ([`harness_core::journal::Launched`]),
/// so a store is opened by one brain and is not shared between threads.
pub struct Store {
    db: Connection,
}

impl Store {
    /// Open the store at `path`, creating it if it does not exist.
    pub fn open(path: impl AsRef<Path>) -> Result<Store, Error> {
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        db.pragma_update(None, "foreign_keys", true)?;
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

    /// The journal folded from its start: everything that is true now.
    ///
    /// A record the fold refuses means the journal holds what the model
    /// forbids, which no correct brain appends, so it is an error rather
    /// than a state.
    pub fn fold(&self) -> Result<State, Error> {
        let mut state = State::default();
        self.each(Seq(0), u64::MAX, |record| {
            state.apply(&record).map_err(|rejected| Error::Rejected {
                seq: record.seq,
                rejected,
            })
        })?;
        Ok(state)
    }

    /// Append `batch`, all or nothing. Its records must continue the journal
    /// from its last position with no gap, and every value they name must
    /// already be stored.
    ///
    /// The store does not fold the batch: the brain already has, on its
    /// state, which is how it knew to write it.
    pub fn append(&mut self, batch: &[Record]) -> Result<(), Error> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let last: u64 = tx.query_row("SELECT coalesce(max(seq), 0) FROM record", [], |row| {
            row.get(0)
        })?;
        let mut entry = tx.prepare_cached("SELECT 1 FROM entry WHERE id = ?")?;
        let mut content = tx.prepare_cached("SELECT 1 FROM content WHERE id = ?")?;
        let mut insert = tx.prepare_cached(
            "INSERT INTO record (seq, at, kind, subject, by, fact) VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        for (record, expected) in batch.iter().zip(last + 1..) {
            if record.seq != Seq(expected) {
                return Err(Error::OutOfOrder {
                    expected: Seq(expected),
                    found: record.seq,
                });
            }
            for value in record.fact.values() {
                let stored = match value {
                    Value::Entry(id) => entry.exists([id.0.0])?,
                    Value::Content(id) => content.exists([id.0.0])?,
                };
                if !stored {
                    return Err(Error::NotStored {
                        seq: record.seq,
                        value,
                    });
                }
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
        drop((entry, content, insert));
        tx.commit()?;
        Ok(())
    }

    /// Up to `limit` records after position `after`, in order: how a face
    /// that has seen the journal up to `after` catches up, a page at a time.
    /// `Seq(0)` reads from the start.
    pub fn read(&self, after: Seq, limit: u64) -> Result<Vec<Record>, Error> {
        let mut records = Vec::new();
        self.each(after, limit, |record| {
            records.push(record);
            Ok(())
        })?;
        Ok(records)
    }

    /// Store `entry`, whose parent must already be stored, and return its id.
    /// Storing an entry again is a no-op: the same id is the same bytes.
    pub fn put_entry(&self, entry: &Entry) -> Result<EntryId, Error> {
        let id = entry.id();
        let mut insert = self.db.prepare_cached(
            "INSERT INTO entry (id, parent, model, system, tools, role, content)
             VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT DO NOTHING",
        )?;
        let inserted = match entry {
            Entry::Root(root) => insert.execute(params![
                id.0.0,
                None::<[u8; 32]>,
                root.model,
                root.system.0,
                root.tools.0,
                None::<&str>,
                None::<&str>,
            ]),
            Entry::Message(message) => insert.execute(params![
                id.0.0,
                message.parent.0.0,
                None::<&str>,
                None::<&str>,
                None::<&str>,
                role_name(message.role),
                message.content.0,
            ]),
        };
        match inserted {
            Ok(_) => Ok(id),
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY =>
            {
                Err(Error::NoParent(entry.parent().unwrap_or(id)))
            }
            Err(other) => Err(other.into()),
        }
    }

    /// The entry named `id`, if it is stored.
    pub fn entry(&self, id: EntryId) -> Result<Option<Entry>, Error> {
        let mut select = self.db.prepare_cached(
            "SELECT parent, model, system, tools, role, content FROM entry WHERE id = ?",
        )?;
        let row = select
            .query_row([id.0.0], |row| {
                Ok((
                    row.get::<_, Option<[u8; 32]>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .optional()?;
        // The table's CHECK guarantees one of these two shapes.
        Ok(row.map(|row| match row {
            (Some(parent), _, _, _, Some(role), Some(content)) => Entry::Message(Message {
                parent: EntryId(Digest(parent)),
                role: role_from_name(&role),
                content: Wire(content),
            }),
            (_, Some(model), Some(system), Some(tools), _, _) => Entry::Root(Root {
                model,
                system: Wire(system),
                tools: Wire(tools),
            }),
            _ => unreachable!("the entry table's CHECK admits only roots and messages"),
        }))
    }

    /// Store `bytes` and return their id. Storing them again is a no-op.
    pub fn put_content(&self, bytes: &[u8]) -> Result<ContentId, Error> {
        let id = ContentId(Digest::of(bytes));
        self.db
            .prepare_cached("INSERT INTO content (id, bytes) VALUES (?, ?) ON CONFLICT DO NOTHING")?
            .execute(params![id.0.0, bytes])?;
        Ok(id)
    }

    /// The bytes named `id`, if they are stored.
    pub fn content(&self, id: ContentId) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .db
            .prepare_cached("SELECT bytes FROM content WHERE id = ?")?
            .query_row([id.0.0], |row| row.get(0))
            .optional()?)
    }

    /// Call `f` on up to `limit` records after `after`, in order, without
    /// holding more than one in memory.
    fn each(
        &self,
        after: Seq,
        limit: u64,
        mut f: impl FnMut(Record) -> Result<(), Error>,
    ) -> Result<(), Error> {
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
            })?;
        }
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
        Fact::Reach { limb, .. } => ("Reach", Some(limb.to_string())),
        Fact::Ranking(_) => ("Ranking", None),
        Fact::Focus(focus) => ("Focus", Some(focus.face.to_string())),
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

fn role_from_name(name: &str) -> Role {
    match name {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        _ => unreachable!("the entry table's CHECK admits only these roles"),
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
    /// A batch that does not continue the journal.
    OutOfOrder { expected: Seq, found: Seq },
    /// The journal holds a record its fold refuses.
    Rejected { seq: Seq, rejected: Rejected },
    /// A message whose parent is not stored.
    NoParent(EntryId),
    /// A record naming a value that is not stored.
    NotStored { seq: Seq, value: Value },
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
                "batch does not continue the journal: expected seq {}, found {}",
                expected.0, found.0
            ),
            Error::Rejected { seq, rejected } => {
                write!(f, "journal record {} does not fold: {rejected:?}", seq.0)
            }
            Error::NoParent(parent) => write!(f, "entry's parent {} is not stored", parent.0),
            Error::NotStored { seq, value } => {
                let (kind, digest) = match value {
                    Value::Entry(id) => ("entry", id.0),
                    Value::Content(id) => ("content", id.0),
                };
                write!(
                    f,
                    "record {} names {kind} {digest}, which is not stored",
                    seq.0
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(error) => Some(error),
            Error::Json(error) => Some(error),
            _ => None,
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
