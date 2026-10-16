//! Live progress: what a face shows while something runs, never a fact.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, ModelCallId};

/// Progress a face shows as it happens, sent beside the journal and never in
/// it.
///
/// **Concept.** Streamed tokens and a command's output so far matter only
/// while they arrive. Nothing acts on them (a tool use is acted on only when
/// its response is complete), so they are not facts: a restart loses them,
/// and the request is sent again or the call is sent again to its limb.
/// What survives is what a participant acted on. When Max interrupts a
/// response, the visible text he was reacting to is kept with the abandoned
/// request ([`crate::model_call::Ending::Abandoned`]); a call's output is
/// kept in its settlement. A limb sends it in the same stream as its facts
/// ([`crate::environment::FromLimb::Live`]); the brain forwards it to faces
/// beside the journal's records.
///
/// **Vision.** The throbber ("streaming", "running, 40 lines so far"), and
/// a journal small enough to replay and query.
///
/// **Existing harnesses.** OpenCode and Goose write streamed parts into
/// their message rows as they arrive; Codex and Pi persist the completed
/// item. Here only completed things are history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Live {
    /// Response content of a request as it streams: the raw event.
    Model { call: ModelCallId, event: String },
    /// Output of a running call so far.
    Call { call: CallId, chunk: String },
}
