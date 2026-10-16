//! The face's view of the brain's store: a second connection to the same
//! file, woken when the brain commits.

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use harness_core::id::{Seq, ValueId};
use harness_core::journal::Record;
use harness_face_web::Source;
use harness_store::Store;

/// The journal as a face reads it.
#[derive(Clone)]
pub struct Reader(Arc<Shared>);

struct Shared {
    store: Mutex<Store>,
    last: Mutex<Seq>,
    grew: Condvar,
}

impl Reader {
    pub fn open(path: &Path) -> Result<Reader, harness_store::Error> {
        Ok(Reader(Arc::new(Shared {
            store: Mutex::new(Store::open(path)?),
            last: Mutex::new(Seq(0)),
            grew: Condvar::new(),
        })))
    }

    /// The brain has committed up to `last`.
    pub fn grew(&self, last: Seq) {
        *lock(&self.0.last) = last;
        self.0.grew.notify_all();
    }
}

impl Source for Reader {
    fn read(&self, after: Seq, wait: Duration) -> Vec<Record> {
        let last = lock(&self.0.last);
        let (last, _) = self
            .0
            .grew
            .wait_timeout_while(last, wait, |l| *l <= after)
            .unwrap_or_else(|e| e.into_inner());
        drop(last);
        // A read error leaves the face where it is; it asks again.
        lock(&self.0.store)
            .read(after, u64::MAX)
            .unwrap_or_default()
    }

    fn value(&self, id: ValueId) -> Option<Vec<u8>> {
        lock(&self.0.store).value(id).ok().flatten()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
