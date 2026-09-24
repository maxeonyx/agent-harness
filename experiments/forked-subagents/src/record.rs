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
    pub fn create(base: &Path, label: &str) -> Result<Recorder, String> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let dir = base.join(format!("{stamp}-{label}"));
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
