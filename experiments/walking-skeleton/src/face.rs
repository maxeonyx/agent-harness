//! The face: an append-only CLI. One select loop over user input and the
//! session event stream. The face never talks to the provider; it emits
//! user events to the brain and *projects* session events for display —
//! its own view, distinct from the model's view of the same events.

use crate::events::{Event, EventKind, Outcome};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, mpsc};

pub fn print_help(base_url: &str, model: &str) {
    println!("walking-skeleton — provider {base_url} model {model}");
    println!("  <text>        stage a user message (appends, never triggers)");
    println!("  /open <path>  simulate user file-open activity (appends, never triggers)");
    println!("  /end          end the turn (triggers inference)");
    println!("  /cancel       cancel in-flight work (request or tool call)");
    println!("  /rebuild      rebuild the context from the event log");
    println!("  /quit         exit");
}

/// Run the face loop until the session closes its event stream.
pub async fn run(user_tx: mpsc::Sender<EventKind>, mut bus_rx: broadcast::Receiver<Event>) {
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    let mut stdin_open = true;
    loop {
        tokio::select! {
            line = lines.next_line(), if stdin_open => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(kind) = parse_line(line.trim())
                            && user_tx.send(kind).await.is_err()
                        {
                            break; // brain is gone
                        }
                    }
                    Ok(None) | Err(_) => {
                        stdin_open = false;
                        let _ = user_tx.send(EventKind::Quit).await;
                    }
                }
            }
            event = bus_rx.recv() => {
                match event {
                    Ok(event) => {
                        if let Some(line) = render(&event) {
                            println!("{line}");
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
        }
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
        EventKind::TurnEnd | EventKind::RebuildRequest | EventKind::Quit => None,
    }
}
