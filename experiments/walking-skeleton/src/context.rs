//! The session context: an event log plus projections. The log is the
//! source of truth; the model view is a *projection* of it, cached and
//! extended incrementally on append (cache-friendly append mode), or
//! recomputed from scratch by `rebuild` (a distinct operation — no
//! compaction policy yet, but the seam is real).
//!
//! Piggyback ordering lives here: events are stored as facts in arrival
//! order, and the model view holds back user events that arrive inside an
//! open tool exchange, flushing them after the tool result, so a
//! `tool_calls` message is never split from its `tool` results on the wire.

use crate::events::{AssistantMessage, Event, EventKind, Outcome, now_ms};
use crate::provider::WireMessage;

pub struct Context {
    log: Vec<Event>,
    next_seq: u64,
    /// Cached model view, extended incrementally by `append`.
    wire: Vec<WireMessage>,
    /// User-emitted wire messages held back while a tool exchange is open.
    held: Vec<WireMessage>,
    /// Tool call ids awaiting an outcome (open exchange when non-empty).
    open_calls: Vec<String>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            log: Vec::new(),
            next_seq: 0,
            wire: Vec::new(),
            held: Vec::new(),
            open_calls: Vec::new(),
        }
    }

    /// Append an event to the log (and incrementally to the cached model
    /// view). Returns the stamped event for broadcasting to consumers.
    pub fn append(&mut self, kind: EventKind) -> Event {
        let event = Event {
            seq: self.next_seq,
            ts_ms: now_ms(),
            kind,
        };
        self.next_seq += 1;
        self.log.push(event.clone());
        Self::project_into(&event, &mut self.wire, &mut self.held, &mut self.open_calls);
        event
    }

    /// Rebuild: recompute the model view as a fresh projection of the whole
    /// log. Today this yields the same view as incremental appending; the
    /// point is that rebuild is a distinct operation with a distinct
    /// call site (fresh context, explicit user request), where a compaction
    /// policy will later live.
    pub fn rebuild(&mut self) -> usize {
        self.wire.clear();
        self.held.clear();
        self.open_calls.clear();
        let log = std::mem::take(&mut self.log);
        for event in &log {
            Self::project_into(event, &mut self.wire, &mut self.held, &mut self.open_calls);
        }
        self.log = log;
        self.wire.len()
    }

    /// The model view: system prompt + the projected wire messages. Any
    /// held-back user messages are flushed by the projection before this is
    /// called for a request (an exchange is always closed by an outcome
    /// event before the next request is built).
    pub fn model_view(&self, system_prompt: &str) -> Vec<WireMessage> {
        let mut messages = vec![WireMessage {
            role: "system".to_string(),
            content: Some(system_prompt.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }];
        messages.extend(self.wire.iter().cloned());
        messages.extend(self.held.iter().cloned());
        messages
    }

    /// Project one event into the wire view. Not every event becomes a wire
    /// message — attempts, turn ends, cancels and rebuilds are facts about
    /// the session, not model context.
    fn project_into(
        event: &Event,
        wire: &mut Vec<WireMessage>,
        held: &mut Vec<WireMessage>,
        open_calls: &mut Vec<String>,
    ) {
        let user_message = |content: String| WireMessage {
            role: "user".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
        };
        match &event.kind {
            EventKind::UserMessage { text } => {
                let msg = user_message(text.clone());
                if open_calls.is_empty() {
                    wire.push(msg);
                } else {
                    held.push(msg);
                }
            }
            EventKind::FileOpened { path, head, .. } => {
                // Model view of user activity: compressed, framed as user
                // activity. The face projects the same event differently.
                let msg = user_message(format!(
                    "[user activity] opened file {path}; first lines:\n{head}"
                ));
                if open_calls.is_empty() {
                    wire.push(msg);
                } else {
                    held.push(msg);
                }
            }
            EventKind::RequestOutcome { outcome, .. } => {
                if let Outcome::Ok { value } = outcome {
                    Self::project_assistant(value, wire, open_calls);
                }
                // Err / Cancelled / Panicked requests contribute nothing to
                // model context; the staged user content simply rides the
                // next attempt.
            }
            EventKind::ToolCallOutcome { call_id, outcome } => {
                let content = match outcome {
                    Outcome::Ok { value } => value.clone(),
                    Outcome::Err { error } => format!("[tool error] {error}"),
                    Outcome::Cancelled { reason } => {
                        format!("[tool cancelled] {reason}")
                    }
                    Outcome::Panicked { payload } => {
                        format!("[tool panicked] {payload}")
                    }
                };
                wire.push(WireMessage {
                    role: "tool".to_string(),
                    content: Some(content),
                    tool_calls: None,
                    tool_call_id: Some(call_id.clone()),
                });
                open_calls.retain(|id| id != call_id);
                if open_calls.is_empty() {
                    wire.append(held);
                }
            }
            // Facts about the session that are not model context.
            EventKind::TurnEnd
            | EventKind::CancelRequest
            | EventKind::Quit
            | EventKind::RebuildRequest
            | EventKind::RequestAttempt { .. }
            | EventKind::ToolCallAttempt { .. }
            | EventKind::TurnOutcome { .. }
            | EventKind::ContextRebuilt { .. }
            | EventKind::SessionClosed => {}
        }
    }

    fn project_assistant(
        message: &AssistantMessage,
        wire: &mut Vec<WireMessage>,
        open_calls: &mut Vec<String>,
    ) {
        if message.tool_calls.is_empty() {
            wire.push(WireMessage {
                role: "assistant".to_string(),
                content: message.text.clone(),
                tool_calls: None,
                tool_call_id: None,
            });
        } else {
            wire.push(WireMessage {
                role: "assistant".to_string(),
                content: message.text.clone(),
                tool_calls: Some(message.tool_calls.clone()),
                tool_call_id: None,
            });
            open_calls.extend(message.tool_calls.iter().map(|c| c.id.clone()));
        }
    }
}
