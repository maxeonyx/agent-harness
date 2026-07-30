//! The recorder: a consumer that persists the session event stream as
//! JSONL. Because events are facts (attempts and outcomes are separate
//! events, and every attempt gets an outcome), the recorder records facts —
//! it never writes an intention as if it had happened.

use crate::events::Event;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

pub fn path_from_env() -> PathBuf {
    std::env::var("SKELETON_RECORD")
        .unwrap_or_else(|_| "skeleton-session.jsonl".to_string())
        .into()
}

/// Run until the event stream closes. The file is opened once and written
/// with async I/O so this consumer never blocks its worker thread.
pub async fn run(path: PathBuf, mut bus_rx: broadcast::Receiver<Event>) {
    let mut file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("[recorder] failed to open {}: {e}", path.display());
            return;
        }
    };
    loop {
        match bus_rx.recv().await {
            Ok(event) => {
                let mut line = match serde_json::to_string(&event) {
                    Ok(line) => line,
                    Err(e) => {
                        eprintln!("[recorder] failed to serialize event: {e}");
                        continue;
                    }
                };
                line.push('\n');
                if let Err(e) = file.write_all(line.as_bytes()).await {
                    eprintln!("[recorder] failed to write {}: {e}", path.display());
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[recorder] lagged; skipped {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    let _ = file.flush().await;
}
