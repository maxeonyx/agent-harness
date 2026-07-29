//! The recorder: a consumer that persists the session event stream as
//! JSONL. Because events are facts (attempts and outcomes are separate
//! events, and every attempt gets an outcome), the recorder records facts —
//! it never writes an intention as if it had happened.

use crate::events::Event;
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::broadcast;

pub fn path_from_env() -> PathBuf {
    std::env::var("SKELETON_RECORD")
        .unwrap_or_else(|_| "skeleton-session.jsonl".to_string())
        .into()
}

/// Run until the event stream closes.
pub async fn run(path: PathBuf, mut bus_rx: broadcast::Receiver<Event>) {
    loop {
        match bus_rx.recv().await {
            Ok(event) => {
                let line = match serde_json::to_string(&event) {
                    Ok(line) => line,
                    Err(e) => {
                        eprintln!("[recorder] failed to serialize event: {e}");
                        continue;
                    }
                };
                let result = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .and_then(|mut f| writeln!(f, "{line}"));
                if let Err(e) = result {
                    eprintln!("[recorder] failed to write {}: {e}", path.display());
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[recorder] lagged; skipped {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
