//! The Anthropic Messages API on Max's Claude subscription, in terms of
//! `harness-core`.
//!
//! One request is one [`harness_core::model_call::ModelCall`]. The client
//! turns it into wire bytes from the context it extends and the values its
//! entries name ([`request::body`]), streams the response as
//! [`harness_core::live::Live`] visible text, and reports its lifecycle facts: `Sent`,
//! `FirstByte`, and how it ended, with usage, quota, and the bytes the
//! ending names ([`Client::send`]).
//!
//! **Bytes.** The body is spliced from stored value bytes. A breakpoint is
//! the only addition: `cache_control` is inserted before the closing brace
//! of the last block of the entry it names. The response's content is
//! written once, when it is assembled from the stream; a tool use's input
//! is kept as the model streamed it. Nothing is parsed and written again.
//!
//! **Usage.** [`Client::usage`] reads the plan's usage windows from the
//! subscription's own endpoint (`/api/oauth/usage`), for Max's call to the
//! provider limb; every response's headers report the 5-hour and 7-day
//! windows too, in its [`harness_core::model_call::Quota`].
//!
//! **The subscription.** Three things, measured in Max's OpenCode plugin
//! and the provider-wire spike: the `oauth-2025-04-20` beta (added here),
//! the identity string [`IDENTITY`] as `system[0]` (the root's, checked
//! here, because a root without it is refused with a fake 429), and a
//! system prompt free of a fingerprinted sentence (the root's author's to
//! keep). Every response's billing claim is checked: anything but a plan
//! window is reported as off the plan, and ends the request as a failure
//! before its body is read. A 429 whose
//! unified status is `rejected` is the plan's limit, ending
//! [`harness_core::model_call::Ending::Limited`] until its reset; any other
//! refusal is a failure of that request.
//!
//! **The login.** [`Login`] is the one owner of the claude.ai credential,
//! for every Anthropic endpoint (dictation's WebSocket uses
//! [`Token::bearer`] too). It reads Claude Code's token for each request
//! and never refreshes it.
//!
//! **Cancellation.** A request runs on a socket of its own ([`harness_runtime::Socket`])
//! through a minimal HTTP/1.1 exchange, so a stop cuts it at once wherever it is
//! blocked: connecting, in the TLS handshake, before the first byte, or
//! mid-stream ([`Client::send`]).
//!
//! **Left to the brain:** breakpoint placement, holding a fan-out until a
//! first byte, retries, and recording the facts in the journal.

mod client;
mod http;
mod login;
pub mod request;
mod response;

pub use client::{Client, Finished, Progress};
pub use login::{Login, LoginError, Token};
pub use request::{IDENTITY, RequestError};

use harness_core::environment::Effort;

/// A model the subscription offers, as a face lists it and a role file may
/// name it.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    /// The role file's name for it, as in Claude Code: `opus`.
    pub alias: &'static str,
    /// The provider's name for it, as a root names it.
    pub id: &'static str,
    /// What a face calls it: `Opus`.
    pub name: &'static str,
}

/// The current model of each family, most capable first: what Max chooses
/// a chat's model from. The provider lists more (`GET /v1/models`, read
/// 2026-10-18 NZT): older versions, and families the harness does not use.
pub const MODELS: [Model; 3] = [
    Model {
        alias: "opus",
        id: "claude-opus-5-5",
        name: "Opus",
    },
    Model {
        alias: "sonnet",
        id: "claude-sonnet-5-5",
        name: "Sonnet",
    },
    Model {
        alias: "haiku",
        id: "claude-haiku-5-5",
        name: "Haiku",
    },
];

/// The provider's name for a model a role file names: its alias's id, or
/// the name as given.
pub fn model_id(name: &str) -> &str {
    MODELS
        .iter()
        .find(|m| m.alias == name)
        .map_or(name, |m| m.id)
}

/// The effort request parameters ask for (`output_config.effort`), if they
/// name one the harness uses.
pub fn effort(params: &[u8]) -> Option<Effort> {
    let params: serde_json::Value = serde_json::from_slice(params).ok()?;
    params["output_config"]["effort"].as_str()?.parse().ok()
}

/// The request parameters with `effort` in place of theirs; `None` when
/// they are not a JSON object.
pub fn with_effort(params: &[u8], effort: Effort) -> Option<Vec<u8>> {
    let mut params: serde_json::Value = serde_json::from_slice(params).ok()?;
    params
        .as_object_mut()?
        .entry("output_config")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()?
        .insert("effort".into(), effort.as_str().into());
    serde_json::to_vec(&params).ok()
}

/// How many tokens a model's context holds: 1M for the current Opus and
/// Sonnet models without a beta, 200k for Haiku.
pub fn context_window(model: &str) -> u64 {
    if model.contains("haiku") {
        200_000
    } else {
        1_000_000
    }
}

/// Why a request was not sent.
#[derive(Debug)]
pub enum Error {
    Login(LoginError),
    Request(RequestError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Login(e) => e.fmt(f),
            Error::Request(e) => write!(f, "request: {}", e.0),
        }
    }
}

impl std::error::Error for Error {}

impl From<LoginError> for Error {
    fn from(e: LoginError) -> Error {
        Error::Login(e)
    }
}

impl From<RequestError> for Error {
    fn from(e: RequestError) -> Error {
        Error::Request(e)
    }
}
