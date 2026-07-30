//! The face: an append-only CLI. One select loop over user input and the
//! session event stream. The face never talks to the provider; it emits
//! user events to the brain and *projects* session events for display —
//! its own view, distinct from the model's view of the same events.

use crate::context::Context;
use crate::events::{Event, EventKind, Outcome};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

pub fn print_help(base_url: &str, model: &str) {
    println!("walking-skeleton — provider {base_url} model {model}");
    println!("  <text>        stage a user message (appends, never triggers)");
    println!("  /open <path>  simulate user file-open activity (appends, never triggers)");
    println!("  /end          end the turn (triggers inference)");
    println!("  /cancel       cancel in-flight work (request or tool call)");
    println!("  /rebuild      rebuild the context from the event log");
    println!("  /dump         open the model view (markdown) in $EDITOR, default nano");
    println!("  /quit         exit");
}

/// Run the face until the session closes its event stream.
///
/// The face is two owned pieces: an input thread (reads and parses stdin,
/// emits user events straight to the brain) and this render loop (a pure
/// consumer of the session event stream). The input thread's lifetime is
/// structured: it exits on /quit, EOF, or a closed channel — every path
/// that ends the session goes through it, so it is always joinable when
/// the render loop finishes.
///
/// `context` is the shared session log: in this co-located deployment the
/// face reads it directly for queries like /dump. A remote face would keep
/// or request enough of the log instead — data still crosses the role
/// boundary as events, never as file paths (no shared filesystem is
/// assumed; the dump file below lives on the *face's* filesystem).
pub async fn run(
    user_tx: mpsc::Sender<EventKind>,
    mut bus_rx: broadcast::Receiver<Event>,
    context: Arc<Mutex<Context>>,
) {
    // Stdin is read (and parsed — including the /open file read, which is
    // fine to block on here) by a dedicated thread, so that /dump can hand
    // the terminal to an editor with *no read pending on the tty*: after
    // sending /dump the thread parks until the editor is done.
    let (resume_tx, resume_rx) = std::sync::mpsc::channel::<()>();
    let input_thread = std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let Some(kind) = parse_line(line.trim()) else {
                continue;
            };
            let is_quit = matches!(kind, EventKind::Quit);
            let is_dump = matches!(kind, EventKind::DumpRequest);
            if user_tx.blocking_send(kind).is_err() {
                return; // brain is gone
            }
            if is_quit {
                return;
            }
            if is_dump && resume_rx.recv().is_err() {
                return;
            }
        }
        // EOF: end the session.
        let _ = user_tx.blocking_send(EventKind::Quit);
    });

    loop {
        match bus_rx.recv().await {
            Ok(event) => {
                if let Some(line) = render(&event) {
                    println!("{line}");
                }
                if matches!(event.kind, EventKind::DumpRequest) {
                    // Our own /dump coming back on the bus: the log now
                    // provably includes everything up to the request.
                    // Project the dump face-side. The editor owns the
                    // terminal until it exits; bus events buffer, the
                    // input thread is parked.
                    let dump = context.lock().expect("context poisoned").dump_view();
                    dump_into_editor(dump).await;
                    println!("[face] returned from dump");
                    let _ = resume_tx.send(());
                }
                if matches!(event.kind, EventKind::SessionClosed) {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                println!("[face] lagged; skipped {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }

    // The session only ends via /quit or EOF — both of which end the input
    // thread — so this join does not wait on a blocked read.
    let joined = tokio::task::spawn_blocking(move || input_thread.join()).await;
    if !matches!(joined, Ok(Ok(()))) {
        println!("[face] input thread failed");
    }
}

/// Write the dump to a temp file on the face's own filesystem, open it in
/// the user's editor (a plain command name; default nano), wait for exit.
async fn dump_into_editor(dump: String) {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let result = tokio::task::spawn_blocking(move || {
        let path = std::env::temp_dir().join(format!(
            "skeleton-dump-{}-{}.md",
            std::process::id(),
            crate::events::now_ms()
        ));
        std::fs::write(&path, dump)
            .map_err(|e| format!("failed to write dump {}: {e}", path.display()))?;
        println!("[face] dump written to {}", path.display());
        std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .map_err(|e| format!("failed to launch editor: {e}"))
    })
    .await;
    match result {
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => println!("[face] editor exited with {status}"),
        Ok(Err(e)) => println!("[face] {e}"),
        Err(e) => println!("[face] editor task failed: {e}"),
    }
}

/// Parse one input line into a user event, or handle it locally.
fn parse_line(line: &str) -> Option<EventKind> {
    if line.is_empty() {
        return None;
    }
    match line {
        "/quit" => Some(EventKind::Quit),
        "/end" => Some(EventKind::TurnEnd),
        "/cancel" => Some(EventKind::CancelRequest),
        "/rebuild" => Some(EventKind::RebuildRequest),
        "/dump" => Some(EventKind::DumpRequest),
        _ => {
            if let Some(path) = line.strip_prefix("/open ") {
                // A user tool: the face reads the file and emits the facts.
                // Face view (rich) and model view (compressed) are both
                // projections of the one event.
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let head: Vec<&str> = content.lines().take(20).collect();
                        Some(EventKind::FileOpened {
                            path: path.to_string(),
                            bytes: content.len(),
                            head: head.join("\n"),
                        })
                    }
                    Err(e) => {
                        println!("[face] could not open {path}: {e}");
                        None
                    }
                }
            } else {
                Some(EventKind::UserMessage {
                    text: line.to_string(),
                })
            }
        }
    }
}

/// The face's projection of a session event. Returns None for events this
/// face does not display.
fn render(event: &Event) -> Option<String> {
    match &event.kind {
        EventKind::UserMessage { .. } => Some("[face] staged user message".to_string()),
        EventKind::FileOpened { path, bytes, .. } => {
            Some(format!("[face] opened {path} ({bytes} bytes)"))
        }
        EventKind::CancelRequest => Some("[face] cancel requested".to_string()),
        EventKind::RequestAttempt { request_id, .. } => {
            Some(format!("[brain] request {request_id} in flight"))
        }
        EventKind::RequestOutcome { outcome, .. } => match outcome {
            Outcome::Ok { value } => value
                .text
                .clone()
                .filter(|t| !t.is_empty())
                .map(|text| format!("[agent] {text}")),
            Outcome::Err { error } => Some(format!("[brain] request failed: {error}")),
            Outcome::Cancelled { reason } => Some(format!("[brain] request cancelled: {reason}")),
            Outcome::Panicked { payload } => Some(format!("[brain] request panicked: {payload}")),
        },
        EventKind::ToolCallAttempt {
            name, arguments, ..
        } => Some(format!("[limb] tool call: {name}({arguments})")),
        EventKind::ToolCallOutcome { outcome, .. } => match outcome {
            Outcome::Ok { value } => Some(format!("[limb] tool result: {} bytes", value.len())),
            Outcome::Err { error } => Some(format!("[limb] tool error: {error}")),
            Outcome::Cancelled { reason } => Some(format!("[limb] tool cancelled: {reason}")),
            Outcome::Panicked { payload } => Some(format!("[limb] tool panicked: {payload}")),
        },
        EventKind::TurnOutcome { outcome } => match outcome {
            Outcome::Ok { .. } => Some("[brain] turn complete".to_string()),
            Outcome::Err { error } => Some(format!("[brain] turn failed: {error}")),
            Outcome::Cancelled { reason } => Some(format!("[brain] turn cancelled: {reason}")),
            Outcome::Panicked { payload } => Some(format!("[brain] turn panicked: {payload}")),
        },
        EventKind::ContextRebuilt { wire_messages } => Some(format!(
            "[brain] context rebuilt ({wire_messages} wire messages)"
        )),
        EventKind::SessionClosed => Some("[brain] session closed".to_string()),
        EventKind::SessionStarted { .. }
        | EventKind::ContributionAdded { .. }
        | EventKind::TurnEnd
        | EventKind::RebuildRequest
        | EventKind::DumpRequest
        | EventKind::Quit => None,
    }
}
