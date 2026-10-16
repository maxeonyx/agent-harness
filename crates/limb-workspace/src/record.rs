//! What the limb keeps on disk: its record of calls, and full outputs.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use harness_core::call::Outcome;
use harness_core::id::{CallId, ContentId, Digest, InstanceId};
use serde::{Deserialize, Serialize};

/// The limb's durable record of its calls, so that it can say how one ended
/// after the brain, or the instance that ran it, has gone.
///
/// A call is recorded as started, with the instance running it, before it
/// has any effect, and as settled before the settlement is sent. So a call
/// recorded as started by another instance and never settled was cut off
/// part way: nobody knows what it did.
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

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub instance: InstanceId,
    pub outcome: Option<Outcome>,
}

impl Calls {
    pub fn open(dir: PathBuf) -> io::Result<Calls> {
        let groups = dir.join("groups");
        fs::create_dir_all(&groups)?;
        Ok(Calls { dir, groups })
    }

    pub fn get(&self, call: CallId) -> io::Result<Option<Entry>> {
        match fs::read(self.path(call)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn put(&self, call: CallId, entry: &Entry) -> io::Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(io::Error::other)?;
        replace(&self.path(call), &bytes)
    }

    fn path(&self, call: CallId) -> PathBuf {
        self.dir.join(format!("{:032x}.json", call.0))
    }

    pub fn running(&self, call: CallId, group: &Group) -> io::Result<()> {
        let bytes = serde_json::to_vec(group).map_err(io::Error::other)?;
        replace(&self.groups.join(format!("{:032x}.json", call.0)), &bytes)
    }

    pub fn ran(&self, call: CallId) {
        let _ = fs::remove_file(self.groups.join(format!("{:032x}.json", call.0)));
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

/// Full outputs too large for a context, named by their digest.
pub struct Blobs {
    dir: PathBuf,
}

impl Blobs {
    pub fn open(dir: PathBuf) -> io::Result<Blobs> {
        fs::create_dir_all(&dir)?;
        Ok(Blobs { dir })
    }

    /// Start writing one output.
    pub fn spool(&self, name: &str) -> io::Result<Spool> {
        let temp = self.dir.join(format!(".{name}"));
        Ok(Spool {
            file: File::create(&temp)?,
            temp,
            hasher: blake3::Hasher::new(),
            dir: self.dir.clone(),
        })
    }

    /// Where a stored output is.
    pub fn path(&self, id: ContentId) -> PathBuf {
        self.dir.join(hex(&id.0))
    }
}

/// One output being written to the store.
pub struct Spool {
    file: File,
    temp: PathBuf,
    hasher: blake3::Hasher,
    dir: PathBuf,
}

impl Spool {
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.hasher.update(bytes);
        self.file.write_all(bytes)
    }

    /// Keep the output, named by its digest.
    pub fn keep(self) -> io::Result<ContentId> {
        self.file.sync_all()?;
        let id = ContentId(Digest(*self.hasher.finalize().as_bytes()));
        fs::rename(&self.temp, self.dir.join(hex(&id.0)))?;
        Ok(id)
    }

    /// The output was small enough to send whole: drop it.
    pub fn discard(self) -> io::Result<()> {
        fs::remove_file(&self.temp)
    }
}

pub fn hex(digest: &Digest) -> String {
    digest.0.iter().map(|b| format!("{b:02x}")).collect()
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
