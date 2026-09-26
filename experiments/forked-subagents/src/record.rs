//! The run directory: every request and response body, every agent's final
//! context, and the summary. Written under `runs.ignore/`, which is
//! gitignored.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Recorder {
    dir: PathBuf,
    wire: Mutex<std::fs::File>,
}

impl Recorder {
    /// Two runs started in the same second must not share a directory. They
    /// did, once: a luna grid and a sonnet grid landed in the same one and
    /// the second to finish overwrote the first's `trials.json`, losing the
    /// index of trials that had already been paid for. The timestamp is for
    /// reading; uniqueness comes from `create_dir` refusing to clobber.
    pub fn create(base: &Path, label: &str) -> Result<Recorder, String> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        std::fs::create_dir_all(base)
            .map_err(|e| format!("create runs directory {}: {e}", base.display()))?;
        let mut dir = base.join(format!("{stamp}-{label}"));
        let mut attempt = 2;
        loop {
            match std::fs::create_dir(&dir) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    dir = base.join(format!("{stamp}-{label}-{attempt}"));
                    attempt += 1;
                }
                Err(error) => {
                    return Err(format!("create run directory {}: {error}", dir.display()));
                }
            }
        }
        std::fs::create_dir_all(dir.join("agents"))
            .map_err(|e| format!("create run directory {}: {e}", dir.display()))?;
        let wire = std::fs::File::create(dir.join("wire.jsonl"))
            .map_err(|e| format!("create wire.jsonl: {e}"))?;
        Ok(Recorder {
            dir,
            wire: Mutex::new(wire),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn wire(&self, agent: &str, kind: &str, body: &serde_json::Value) {
        let entry = serde_json::json!({
            "at": chrono::Local::now().to_rfc3339(),
            "agent": agent,
            "kind": kind,
            "body": body,
        });
        let mut file = self.wire.lock().unwrap();
        let _ = writeln!(file, "{entry}");
        let _ = file.flush();
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.join(relative);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(error) = std::fs::write(&path, contents) {
            eprintln!("could not write {}: {error}", path.display());
        }
    }
}
