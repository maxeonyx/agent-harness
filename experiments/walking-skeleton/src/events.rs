//! The event vocabulary. An event is about its emitter, not *for* anyone —
//! consumers (face, model-context view, recorder) project events as they
//! need. Nothing in flight ends without an outcome event: attempts and
//! outcomes are separate facts, and the outcome is four-valued (ok / err /
//! cancelled / panicked). Cancelled is not an error.

use crate::provider::ToolCall;
use serde::Serialize;

/// Four-valued outcome, after asupersync's model: cancellation is a
/// first-class resolution, distinct from failure.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome<T> {
    Ok { value: T },
    Err { error: String },
    Cancelled { reason: String },
    Panicked { payload: String },
}

/// What the provider answered with (projection-neutral facts).
#[derive(Serialize, Clone, Debug)]
pub struct AssistantMessage {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

/// One contribution to the model's environment.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Contribution {
    /// A textual environment fact (time, hostname, model, ...): composed
    /// into the system prompt.
    Fact { text: String },
    /// A tool definition (OpenAI function schema): sent as the request's
    /// `tools` field.
    Tool { def: serde_json::Value },
}

/// One event in the session log. `seq` is assigned by the session log at
/// append time.
#[derive(Serialize, Clone, Debug)]
pub struct Event {
    pub seq: u64,
    pub ts_ms: u128,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventKind {
    // ---- emitter: the brain, at session start ----
    /// Everything the model sees must be derivable from the log by any
    /// consumer, so the system prompt enters the session as a fact, not as
    /// brain-private config.
    SessionStarted {
        system_prompt: String,
    },
    /// A system-prompt / environment contribution: skills, tools,
    /// environment facts like time, hostname, model. A contribution that
    /// exists from the start is composed into the system prompt (or the
    /// request's tools field); one added or changed while the context is
    /// active becomes an update appended to the context.
    ContributionAdded {
        name: String,
        contribution: Contribution,
    },

    // ---- emitter: the user, via a face ----
    UserMessage {
        text: String,
    },
    /// The user opened a file in their own tooling. The event carries the
    /// facts; the face and the model view each project their own view.
    FileOpened {
        path: String,
        bytes: usize,
        head: String,
    },
    TurnEnd,
    CancelRequest,
    Quit,
    /// The user asked for an explicit context rebuild.
    RebuildRequest,
    /// The user asked to introspect the model view (/dump).
    DumpRequest,

    // ---- emitter: the agent loop (brain) ----
    RequestAttempt {
        request_id: u64,
        model: String,
    },
    RequestOutcome {
        request_id: u64,
        outcome: Outcome<AssistantMessage>,
    },
    ToolCallAttempt {
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallOutcome {
        call_id: String,
        outcome: Outcome<String>,
    },
    /// A turn (one /end trigger and its tool-call loop) resolved.
    TurnOutcome {
        outcome: Outcome<()>,
    },
    /// The context was rebuilt from the event log (fresh projection),
    /// as opposed to incrementally appended.
    ContextRebuilt {
        wire_messages: usize,
    },
    SessionClosed,
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
