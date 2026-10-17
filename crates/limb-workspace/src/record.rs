//! What the limb keeps on disk: its record of calls.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use harness_core::id::CallId;
use harness_core::journal::Batch;
use serde::{Deserialize, Serialize};

/// The limb's durable record of its calls, so that it can say how one ended
/// after the brain, or the instance that ran it, has gone.
///
/// A call is recorded as started before it has any effect, and as settled,
/// with the whole batch that says so, before that batch is sent. So a call
/// recorded as started that no instance is running was cut off part way:
/// nobody knows what it did.
pub struct Calls {
    dir: PathBuf,
    /// A file per call whose command may be running, made before the
    /// command starts and removed once nothing it started is left: so an
    /// instance that starts after one was killed finds every command it left
    /// behind by the call's mark ([`crate::shell::mark`]).
    running: PathBuf,
}

/// What the record says of one call.
#[derive(Serialize, Deserialize)]
pub enum Recorded {
    Started,
    /// The facts that settled it and the bytes they name, exactly as sent.
    Settled(Batch),
}

impl Calls {
    pub fn open(dir: PathBuf) -> io::Result<Calls> {
        let running = dir.join("running");
        fs::create_dir_all(&running)?;
        Ok(Calls { dir, running })
    }

    pub fn get(&self, call: CallId) -> io::Result<Option<Recorded>> {
        match fs::read(self.path(call)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn put(&self, call: CallId, recorded: &Recorded) -> io::Result<()> {
        let bytes = serde_json::to_vec(recorded).map_err(io::Error::other)?;
        replace(&self.path(call), &bytes)
    }

    fn path(&self, call: CallId) -> PathBuf {
        self.dir.join(format!("{call}.json"))
    }

    /// The call's command is about to start.
    pub fn running(&self, call: CallId) -> io::Result<()> {
        replace(&self.running.join(call.to_string()), &[])
    }

    /// Nothing the call's command started is left.
    pub fn ran(&self, call: CallId) {
        let _ = fs::remove_file(self.running.join(call.to_string()));
    }

    /// The calls whose commands may still be running: called when an
    /// instance starts, so each belongs to an instance that has gone.
    pub fn orphans(&self) -> io::Result<Vec<CallId>> {
        let mut calls = Vec::new();
        for entry in fs::read_dir(&self.running)? {
            let entry = entry?;
            if let Some(call) = entry.file_name().to_str().and_then(|n| n.parse().ok()) {
                calls.push(call);
            }
        }
        Ok(calls)
    }
}

/// Replace a file's content all at once: a reader sees the old bytes or the
/// new, never a mixture, and a crash leaves one of them.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = File::create(&temp)?;
    file.write_all(bytes)?;
    if let Ok(meta) = fs::metadata(path) {
        file.set_permissions(meta.permissions())?;
    }
    file.sync_all()?;
    fs::rename(&temp, path)
}
