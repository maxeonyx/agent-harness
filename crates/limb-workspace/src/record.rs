//! What the limb keeps on disk beside its log: marks of its calls.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use harness_core::id::CallId;

/// Marks of the limb's calls, so that an instance can tell a call an
/// earlier one was cut off running from one never run.
///
/// A call is marked started before it has any effect, and its settlement is
/// in the log before it stops running. So a call marked started that no
/// instance is running and the log has not settled was cut off part way:
/// nobody knows what it did.
pub struct Marks {
    started: PathBuf,
    /// A file per call whose command may be running, made before the
    /// command starts and removed once nothing it started is left: so an
    /// instance that starts after one was killed finds every command it left
    /// behind by the call's mark ([`crate::shell::mark`]).
    running: PathBuf,
}

impl Marks {
    pub fn open(dir: PathBuf) -> io::Result<Marks> {
        let started = dir.join("started");
        let running = dir.join("running");
        fs::create_dir_all(&started)?;
        fs::create_dir_all(&running)?;
        Ok(Marks { started, running })
    }

    pub fn start(&self, call: CallId) -> io::Result<()> {
        replace(&self.started.join(call.to_string()), &[])
    }

    pub fn is_started(&self, call: CallId) -> bool {
        self.started.join(call.to_string()).exists()
    }

    pub fn started(&self) -> Vec<CallId> {
        calls(&self.started).unwrap_or_default()
    }

    /// The brain has the call's settlement.
    pub fn forget(&self, call: CallId) {
        let _ = fs::remove_file(self.started.join(call.to_string()));
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
        calls(&self.running)
    }
}

fn calls(dir: &Path) -> io::Result<Vec<CallId>> {
    let mut calls = Vec::new();
    for entry in fs::read_dir(dir)? {
        if let Some(call) = entry?.file_name().to_str().and_then(|n| n.parse().ok()) {
            calls.push(call);
        }
    }
    Ok(calls)
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
