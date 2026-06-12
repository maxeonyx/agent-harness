//! Simple pluggable recorder: append-only JSONL event log. Recording is its
//! own operation, distinct from appending context and triggering inference.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Recorder {
    path: PathBuf,
}

impl Recorder {
    pub fn from_env() -> Self {
        let path = std::env::var("SKELETON_RECORD")
            .unwrap_or_else(|_| "skeleton-session.jsonl".to_string());
        Recorder { path: path.into() }
    }

    pub fn record(&self, event: &str, data: serde_json::Value) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let line = serde_json::json!({ "ts": ts, "event": event, "data": data });
        let result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = result {
            eprintln!("[recorder] failed to write {}: {e}", self.path.display());
        }
    }
}
