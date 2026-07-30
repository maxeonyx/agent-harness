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

/// One message of the model view, tagged with which event produced it and
/// whether it was piggybacked (held back to keep a tool exchange intact,
/// so its wire position differs from its arrival order). The model sees
/// only the message; the tags feed the dump projection.
#[derive(Clone)]
pub struct WireEntry {
    pub message: WireMessage,
    pub seq: u64,
    pub piggybacked: bool,
}

// TODO(user direction, 2026-07-30): the shared session log is append-only,
// so Arc<Mutex<Context>> is heavier than needed — a lock-free append-only
// structure (e.g. boxcar) or a single-threaded async model would do; a
// mutex is only really required for cleanup-style operations (compaction /
// log rewriting), which don't exist yet. Arc<Mutex> for now.
pub struct Context {
    log: Vec<Event>,
    next_seq: u64,
    /// Cached model view, extended incrementally by `append`.
    wire: Vec<WireEntry>,
    /// User-emitted wire entries held back while a tool exchange is open.
    held: Vec<WireEntry>,
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

    /// The model view: the projected wire messages (the system message is
    /// itself projected from the `session_started` event). Any held-back
    /// user messages are flushed by the projection before this is called
    /// for a request (an exchange is always closed by an outcome event
    /// before the next request is built).
    pub fn model_view(&self) -> Vec<WireMessage> {
        let mut messages: Vec<WireMessage> = self.wire.iter().map(|e| e.message.clone()).collect();
        messages.extend(self.held.iter().map(|e| e.message.clone()));
        messages
    }

    /// The dump projection: the model view rendered as markdown, in wire
    /// order (what the model sees), with everything the model *cannot* see
    /// in HTML comments — non-wire events interleaved by arrival order,
    /// piggyback annotations where wire order departs from chronology, and
    /// currently-held entries flagged as such. Any consumer with the log
    /// can compute this; it is not brain-private.
    pub fn dump_view(&self) -> String {
        use std::fmt::Write;
        let visible: std::collections::HashSet<u64> = self
            .wire
            .iter()
            .chain(self.held.iter())
            .map(|e| e.seq)
            .collect();
        let mut emitted: std::collections::HashSet<u64> = std::collections::HashSet::new();

        let mut out = String::new();
        out.push_str("# walking-skeleton context dump — the model view\n\n");
        out.push_str(
            "<!-- Everything in HTML comments (like this) is invisible to the model. \
             Everything else renders the exact wire context, in wire order. -->\n\n",
        );

        let comments_before =
            |out: &mut String, bound: Option<u64>, emitted: &mut std::collections::HashSet<u64>| {
                for event in &self.log {
                    if bound.is_some_and(|b| event.seq >= b)
                        || visible.contains(&event.seq)
                        || emitted.contains(&event.seq)
                    {
                        continue;
                    }
                    emitted.insert(event.seq);
                    let kind = serde_json::to_string(&event.kind).unwrap_or_default();
                    let _ = writeln!(out, "<!-- seq {}: {} -->", event.seq, kind);
                }
            };

        for entry in &self.wire {
            comments_before(&mut out, Some(entry.seq), &mut emitted);
            if entry.piggybacked {
                let _ = writeln!(
                    out,
                    "<!-- seq {}: arrived while a tool exchange was open; \
                     the model sees it here, after the exchange -->",
                    entry.seq
                );
            }
            Self::render_message(&mut out, &entry.message);
        }
        for entry in &self.held {
            let _ = writeln!(
                out,
                "<!-- seq {}: currently held: a tool exchange is open; \
                 will be placed after it -->",
                entry.seq
            );
            Self::render_message(&mut out, &entry.message);
        }
        comments_before(&mut out, None, &mut emitted);
        out
    }

    fn render_message(out: &mut String, message: &WireMessage) {
        use std::fmt::Write;
        match &message.tool_call_id {
            Some(id) => {
                let _ = writeln!(out, "## {} ({id})\n", message.role);
            }
            None => {
                let _ = writeln!(out, "## {}\n", message.role);
            }
        }
        if let Some(content) = &message.content
            && !content.is_empty()
        {
            out.push_str(content);
            out.push_str("\n\n");
        }
        if let Some(calls) = &message.tool_calls {
            out.push_str("```json tool_calls\n");
            out.push_str(&serde_json::to_string_pretty(calls).unwrap_or_default());
            out.push_str("\n```\n\n");
        }
    }

    /// Project one event into the wire view. Not every event becomes a wire
    /// message — attempts, turn ends, cancels and rebuilds are facts about
    /// the session, not model context.
    fn project_into(
        event: &Event,
        wire: &mut Vec<WireEntry>,
        held: &mut Vec<WireEntry>,
        open_calls: &mut Vec<String>,
    ) {
        let seq = event.seq;
        let entry = |message: WireMessage, piggybacked: bool| WireEntry {
            message,
            seq,
            piggybacked,
        };
        let user_message = |content: String| WireMessage {
            role: "user".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
        };
        match &event.kind {
            EventKind::SessionStarted { system_prompt } => {
                // The system prompt is model-visible, so it enters the
                // wire view from the log like everything else.
                wire.push(entry(
                    WireMessage {
                        role: "system".to_string(),
                        content: Some(system_prompt.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    false,
                ));
            }
            EventKind::UserMessage { text } => {
                let msg = user_message(text.clone());
                if open_calls.is_empty() {
                    wire.push(entry(msg, false));
                } else {
                    held.push(entry(msg, true));
                }
            }
            EventKind::FileOpened { path, head, .. } => {
                // Model view of user activity: compressed, framed as user
                // activity. The face projects the same event differently.
                let msg = user_message(format!(
                    "[user activity] opened file {path}; first lines:\n{head}"
                ));
                if open_calls.is_empty() {
                    wire.push(entry(msg, false));
                } else {
                    held.push(entry(msg, true));
                }
            }
            EventKind::RequestOutcome { outcome, .. } => {
                if let Outcome::Ok { value } = outcome {
                    Self::project_assistant(value, seq, wire, open_calls);
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
                wire.push(entry(
                    WireMessage {
                        role: "tool".to_string(),
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: Some(call_id.clone()),
                    },
                    false,
                ));
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
            | EventKind::DumpRequest
            | EventKind::RequestAttempt { .. }
            | EventKind::ToolCallAttempt { .. }
            | EventKind::TurnOutcome { .. }
            | EventKind::ContextRebuilt { .. }
            | EventKind::SessionClosed => {}
        }
    }

    fn project_assistant(
        message: &AssistantMessage,
        seq: u64,
        wire: &mut Vec<WireEntry>,
        open_calls: &mut Vec<String>,
    ) {
        if message.tool_calls.is_empty() {
            wire.push(WireEntry {
                message: WireMessage {
                    role: "assistant".to_string(),
                    content: message.text.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                },
                seq,
                piggybacked: false,
            });
        } else {
            wire.push(WireEntry {
                message: WireMessage {
                    role: "assistant".to_string(),
                    content: message.text.clone(),
                    tool_calls: Some(message.tool_calls.clone()),
                    tool_call_id: None,
                },
                seq,
                piggybacked: false,
            });
            open_calls.extend(message.tool_calls.iter().map(|c| c.id.clone()));
        }
    }
}
