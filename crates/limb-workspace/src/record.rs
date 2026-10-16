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
    /// The process groups of commands running now, so an instance that
    /// starts after one died can end what it left behind.
    groups: PathBuf,
}

/// A command's process group, and when its leader started, which tells it
/// apart from a later process given the same pid.
#[derive(Serialize, Deserialize)]
pub struct Group {
    pub group: i32,
    pub start: u64,
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
        let groups = dir.join("groups");
        fs::create_dir_all(&groups)?;
        Ok(Calls { dir, groups })
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

    pub fn running(&self, call: CallId, group: &Group) -> io::Result<()> {
        let bytes = serde_json::to_vec(group).map_err(io::Error::other)?;
        replace(&self.groups.join(format!("{call}.json")), &bytes)
    }

    pub fn ran(&self, call: CallId) {
        let _ = fs::remove_file(self.groups.join(format!("{call}.json")));
    }

    /// The groups recorded as running, forgetting them: called when an
    /// instance starts, so every one belongs to an instance that has gone.
    pub fn orphans(&self) -> io::Result<Vec<Group>> {
        let mut groups = Vec::new();
        for entry in fs::read_dir(&self.groups)? {
            let path = entry?.path();
            if let Ok(group) = serde_json::from_slice(&fs::read(&path)?) {
                groups.push(group);
            }
            fs::remove_file(path)?;
        }
        Ok(groups)
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
