use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt;

use crate::record::{Record, Schema};
use crate::time::Time;

/// Where a replica keeps the records it holds: its own, and those it has
/// included. The brain's is SQLite, a limb's its state directory, a tab's
/// IndexedDB.
///
/// **Concept.** A set of records keyed by time, changed atomically. The
/// substrate decides what to keep and calls [`Storage::commit`] before it
/// lets anything take effect, so a replica never acts on a record it has not
/// stored, and a crash at any point leaves storage as of a whole commit.
pub trait Storage<S: Schema> {
    type Error: fmt::Debug;

    /// Every record held, in any order.
    fn load(&self) -> Result<Vec<Record<S>>, Self::Error>;

    /// Add `put` and remove `drop`, in one transaction.
    fn commit(&mut self, put: &[Record<S>], drop: &[Time]) -> Result<(), Self::Error>;
}

/// Storage in memory: a replica's storage for a demonstration, and a replica
/// that is gone when its process is (a tab without IndexedDB). Cloning it
/// is a crash: the clone opens as the replica would after a restart.
#[derive(Clone, Debug)]
pub struct Memory<S: Schema> {
    records: BTreeMap<Time, Record<S>>,
}

impl<S: Schema> Default for Memory<S> {
    fn default() -> Self {
        Memory {
            records: BTreeMap::new(),
        }
    }
}

impl<S: Schema> Memory<S> {
    /// Every record held, by time.
    pub fn records(&self) -> impl Iterator<Item = &Record<S>> {
        self.records.values()
    }
}

impl<S: Schema> Storage<S> for Memory<S> {
    type Error = Infallible;

    fn load(&self) -> Result<Vec<Record<S>>, Infallible> {
        Ok(self.records.values().cloned().collect())
    }

    fn commit(&mut self, put: &[Record<S>], drop: &[Time]) -> Result<(), Infallible> {
        for time in drop {
            self.records.remove(time);
        }
        for record in put {
            self.records.insert(record.at, record.clone());
        }
        Ok(())
    }
}
