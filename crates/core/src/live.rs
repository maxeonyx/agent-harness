//! Live progress: what a face shows while something runs, never a fact.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, ModelCallId};

/// Progress a face shows as it happens: the schema's
/// [`harness_substrate::Live`] data, never stored.
///
/// **Concept.** Streamed tokens and a command's output so far matter only
/// while they arrive. Nothing acts on them (a tool use is acted on only when
/// its response is complete), so they are not facts: a restart loses them,
/// and the request is sent again or the call is sent again to its limb.
/// What survives is what a participant acted on. When Max interrupts a
/// response, the visible text he was reacting to is kept with the abandoned
/// request ([`crate::model_call::Ending::Cut`]); a call's output is
/// kept in its settlement. A limb sends it on the link that carries its
/// records ([`harness_substrate::Up::Live`]); the brain forwards it to faces
/// on theirs ([`harness_substrate::Down::Live`]).
///
/// **Vision.** The throbber ("streaming", "running, 40 lines so far"), and
/// a log small enough to replay and query.
///
/// **Existing harnesses.** OpenCode and Goose write streamed parts into
/// their message rows as they arrive; Codex and Pi persist the completed
/// item. Here only completed things are history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Live {
    /// Visible text of a response as it streams: what Max watches, and what
    /// is kept if he interrupts it. The provider's event stream stays in the
    /// provider.
    Model { call: ModelCallId, chunk: String },
    /// Output of a running call so far.
    Call { call: CallId, chunk: String },
}
