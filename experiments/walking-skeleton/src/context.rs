//! The session context: an event log plus projections. The log is the
//! source of truth; the request view is a *projection* of it, cached and
//! extended incrementally on append (cache-friendly append mode), or
//! recomputed from scratch by `rebuild` (a distinct operation — no
//! compaction policy yet, but the seam is real).
//!
//! `request_parts` is the single projection of what the model sees: the
//! request builder sends it and the dump renders it — shared code, so the
//! dump cannot silently miss something the model gets (tool schemas,
//! composed system prompt, ...).
//!
//! Contributions (skills, tools, environment facts) follow the design
//! rule: a contribution that exists from the start is composed into the
//! system prompt / tools field; one added or changed while the context is
//! active becomes an update appended to the context.
//!
//! Piggyback ordering lives here too: events are stored as facts in
//! arrival order, and the request view holds back user events that arrive
//! inside an open tool exchange, flushing them after the tool result, so a
//! `tool_calls` message is never split from its `tool` results on the
//! wire.

use crate::events::{AssistantMessage, Contribution, Event, EventKind, Outcome, now_ms};
use crate::provider::WireMessage;

/// One message of the request view, tagged with which event produced it
/// and whether it was piggybacked (held back to keep a tool exchange
/// intact, so its wire position differs from its arrival order). The model
/// sees only the message; the tags feed the dump projection.
#[derive(Clone)]
pub struct WireEntry {
    pub message: WireMessage,
    pub seq: u64,
    pub piggybacked: bool,
}

/// Everything the model sees, projected from the log. The request builder
/// and the dump both consume exactly this.
pub struct RequestParts {
    pub messages: Vec<WireMessage>,
    pub tools: Vec<serde_json::Value>,
}

// TODO(user direction, 2026-07-30): the shared session log is append-only,
// so Arc<Mutex<Context>> is heavier than needed — a lock-free append-only
// structure (e.g. boxcar) or a single-threaded async model would do; a
// mutex is only really required for cleanup-style operations (compaction /
// log rewriting), which don't exist yet. Arc<Mutex> for now.
pub struct Context {
    log: Vec<Event>,
    next_seq: u64,
    /// Base system prompt, from `session_started`.
    system_base: String,
    /// Fact contributions (name → text), latest value per name.
    facts: Vec<(String, String)>,
    /// Tool contributions (name → schema), latest value per name.
    tools: Vec<(String, serde_json::Value)>,
    /// The context becomes active at the first request attempt; from then
    /// on contribution changes append updates instead of silently editing
    /// the composed prompt.
    active: bool,
    /// Cached conversation view, extended incrementally by `append`.
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
            system_base: String::new(),
            facts: Vec::new(),
            tools: Vec::new(),
            active: false,
            wire: Vec::new(),
            held: Vec::new(),
            open_calls: Vec::new(),
        }
    }

    /// Append an event to the log (and incrementally to the cached
    /// projections). Returns the stamped event for broadcasting.
    pub fn append(&mut self, kind: EventKind) -> Event {
        let event = Event {
            seq: self.next_seq,
            ts_ms: now_ms(),
            kind,
        };
        self.next_seq += 1;
        self.log.push(event.clone());
        self.project(&event);
        event
    }

    /// Rebuild: recompute the projections as a fresh pass over the whole
    /// log. Today this yields the same view as incremental appending; the
    /// point is that rebuild is a distinct operation with a distinct call
    /// site, where a compaction policy will later live.
    pub fn rebuild(&mut self) -> usize {
        self.system_base.clear();
        self.facts.clear();
        self.tools.clear();
        self.active = false;
        self.wire.clear();
        self.held.clear();
        self.open_calls.clear();
        let log = std::mem::take(&mut self.log);
        for event in &log {
            self.project(event);
        }
        self.log = log;
        self.wire.len()
    }

    /// The single projection of what the model sees right now: composed
    /// system message, conversation, and tool schemas. Requests are built
    /// from this and the dump renders this — same code, by design.
    pub fn request_parts(&self) -> RequestParts {
        let mut messages = vec![WireMessage {
            role: "system".to_string(),
            content: Some(self.composed_system()),
            tool_calls: None,
            tool_call_id: None,
        }];
        messages.extend(self.wire.iter().map(|e| e.message.clone()));
        messages.extend(self.held.iter().map(|e| e.message.clone()));
        RequestParts {
            messages,
            tools: self.tools.iter().map(|(_, def)| def.clone()).collect(),
        }
    }

    fn composed_system(&self) -> String {
        let mut s = self.system_base.clone();
        if !self.facts.is_empty() {
            s.push_str("\n\n[environment]\n");
            for (name, text) in &self.facts {
                s.push_str(&format!("- {name}: {text}\n"));
            }
        }
        s
    }
    /// Snapshot everything the dump projection needs. Cheap linear clones:
    /// callers take this under the shared-log lock, release the lock, and
    /// render outside it, so a large session cannot stall the brain's
    /// appends behind markdown rendering.
    pub fn dump_snapshot(&self) -> DumpSnapshot {
        DumpSnapshot {
            parts: self.request_parts(),
            log: self.log.clone(),
            wire: self.wire.clone(),
            held: self.held.clone(),
        }
    }

    /// Project one event into the cached views. Not every event becomes
    /// wire content — attempts, turn ends, cancels and rebuilds are facts
    /// about the session, not model context.
    fn project(&mut self, event: &Event) {
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
                self.system_base = system_prompt.clone();
            }
            EventKind::ContributionAdded { name, contribution } => {
                let update_text = match contribution {
                    Contribution::Fact { text } => {
                        upsert(&mut self.facts, name, text.clone());
                        format!("[environment update] {name}: {text}")
                    }
                    Contribution::Tool { def } => {
                        upsert(&mut self.tools, name, def.clone());
                        format!(
                            "[environment update] tool {name} added or changed: {}",
                            serde_json::to_string(def).unwrap_or_default()
                        )
                    }
                };
                // From the start → composed into system prompt / tools.
                // While the context is active → an update appended to the
                // context (the composed views also change; cache-friendly
                // notification vs rebuild is a later, real policy).
                if self.active {
                    self.wire.push(entry(
                        WireMessage {
                            role: "system".to_string(),
                            content: Some(update_text),
                            tool_calls: None,
                            tool_call_id: None,
                        },
                        false,
                    ));
                }
            }
            EventKind::UserMessage { text } => {
                let msg = user_message(text.clone());
                if self.open_calls.is_empty() {
                    self.wire.push(entry(msg, false));
                } else {
                    self.held.push(entry(msg, true));
                }
            }
            EventKind::FileOpened { path, head, .. } => {
                // Model view of user activity: compressed, framed as user
                // activity. The face projects the same event differently.
                let msg = user_message(format!(
                    "[user activity] opened file {path}; first lines:\n{head}"
                ));
                if self.open_calls.is_empty() {
                    self.wire.push(entry(msg, false));
                } else {
                    self.held.push(entry(msg, true));
                }
            }
            EventKind::RequestAttempt { .. } => {
                self.active = true;
            }
            EventKind::RequestOutcome { outcome, .. } => {
                if let Outcome::Ok { value } = outcome {
                    self.project_assistant(value, seq);
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
                self.wire.push(entry(
                    WireMessage {
                        role: "tool".to_string(),
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: Some(call_id.clone()),
                    },
                    false,
                ));
                self.open_calls.retain(|id| id != call_id);
                if self.open_calls.is_empty() {
                    let flushed = std::mem::take(&mut self.held);
                    self.wire.extend(flushed);
                }
            }
            // Facts about the session that are not model context.
            EventKind::TurnEnd
            | EventKind::CancelRequest
            | EventKind::Quit
            | EventKind::RebuildRequest
            | EventKind::DumpRequest
            | EventKind::ToolCallAttempt { .. }
            | EventKind::TurnOutcome { .. }
            | EventKind::ContextRebuilt { .. }
            | EventKind::SessionClosed => {}
        }
    }

    fn project_assistant(&mut self, message: &AssistantMessage, seq: u64) {
        let tool_calls = if message.tool_calls.is_empty() {
            None
        } else {
            self.open_calls
                .extend(message.tool_calls.iter().map(|c| c.id.clone()));
            Some(message.tool_calls.clone())
        };
        self.wire.push(WireEntry {
            message: WireMessage {
                role: "assistant".to_string(),
                content: message.text.clone(),
                tool_calls,
                tool_call_id: None,
            },
            seq,
            piggybacked: false,
        });
    }
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

fn upsert<T>(list: &mut Vec<(String, T)>, name: &str, value: T) {
    match list.iter_mut().find(|(n, _)| n == name) {
        Some((_, existing)) => *existing = value,
        None => list.push((name.to_string(), value)),
    }
}

/// A point-in-time copy of the dump projection's inputs, renderable
/// without holding the shared-log lock.
pub struct DumpSnapshot {
    parts: RequestParts,
    log: Vec<Event>,
    wire: Vec<WireEntry>,
    held: Vec<WireEntry>,
}

impl DumpSnapshot {
    /// The dump projection: exactly `request_parts`, rendered as markdown
    /// in wire order, plus everything the model *cannot* see in HTML
    /// comments — non-wire events interleaved by arrival order, piggyback
    /// annotations where wire order departs from chronology, and
    /// currently-held entries flagged as such. Single linear pass over
    /// the log. Any consumer with the log can compute this; it is not
    /// brain-private.
    pub fn render(&self) -> String {
        use std::fmt::Write;

        // Events already represented in the rendered request: wire/held
        // entries, plus session start and pre-activation contributions
        // (composed into the system message / tools section).
        let mut represented: std::collections::HashSet<u64> = self
            .wire
            .iter()
            .chain(self.held.iter())
            .map(|e| e.seq)
            .collect();
        let mut activated = false;
        for event in &self.log {
            match &event.kind {
                EventKind::RequestAttempt { .. } => activated = true,
                EventKind::SessionStarted { .. } => {
                    represented.insert(event.seq);
                }
                EventKind::ContributionAdded { .. } if !activated => {
                    represented.insert(event.seq);
                }
                _ => {}
            }
        }

        let mut out = String::new();
        out.push_str("# walking-skeleton context dump — the model view\n\n");
        out.push_str(
            "<!-- Everything in HTML comments (like this) is invisible to the model. \
             Everything else renders exactly the request the brain would send right \
             now (shared code with the request builder). -->\n\n",
        );

        // The composed system message (parts.messages[0]).
        if let Some(system) = self.parts.messages.first() {
            render_message(&mut out, system);
        }

        // The request's tools field — the model sees these schemas.
        if !self.parts.tools.is_empty() {
            out.push_str("## tools\n\n");
            out.push_str("<!-- sent as the request's `tools` field -->\n");
            out.push_str("```json\n");
            out.push_str(&serde_json::to_string_pretty(&self.parts.tools).unwrap_or_default());
            out.push_str("\n```\n\n");
        }

        // Linear merge: one advancing pointer into the log. Wire entries
        // are in wire order; only piggybacked entries have out-of-order
        // seqs, and their preceding comments were already emitted.
        let mut next = 0usize;
        let mut comments_before = |out: &mut String, bound: Option<u64>| {
            while next < self.log.len() {
                let event = &self.log[next];
                if bound.is_some_and(|b| event.seq >= b) {
                    break;
                }
                next += 1;
                if represented.contains(&event.seq) {
                    continue;
                }
                let kind = serde_json::to_string(&event.kind).unwrap_or_default();
                let _ = writeln!(out, "<!-- seq {}: {} -->", event.seq, kind);
            }
        };

        for entry in &self.wire {
            comments_before(&mut out, Some(entry.seq));
            if entry.piggybacked {
                let _ = writeln!(
                    out,
                    "<!-- seq {}: arrived while a tool exchange was open; \
                     the model sees it here, after the exchange -->",
                    entry.seq
                );
            }
            render_message(&mut out, &entry.message);
        }
        for entry in &self.held {
            let _ = writeln!(
                out,
                "<!-- seq {}: currently held: a tool exchange is open; \
                 will be placed after it -->",
                entry.seq
            );
            render_message(&mut out, &entry.message);
        }
        comments_before(&mut out, None);
        out
    }
}
