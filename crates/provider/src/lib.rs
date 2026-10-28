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
//! **Models.** [`Client::models`] reads the models on offer from the
//! provider (`/v1/models`, newest first); an alias as in Claude Code
//! (`opus`) means the newest of its [`family`] ([`resolve`]). One more
//! model is served by the client itself: [`SCRIPTED`], which answers from a
//! script in the conversation, at no cost and with no call, in the
//! provider's wire format, so the rest of the harness runs as it does for
//! a real model. [`Client::exchange`] is one call on the login, for
//! whatever the harness has no call for (`harness-api`, a binary here).
//!
//! **Checking a build.** [`Client::count`] has the provider count a
//! request's input tokens (`/v1/messages/count_tokens`), which generates
//! nothing and costs nothing on the plan but checks the request's shape as
//! a send would; [`Client::ping`] streams a response a few tokens long. A
//! new build runs both before it takes over (`harness-brain --check`).
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
//! [`Token::bearer`] too). It is the harness's own sign-in, made the way
//! Claude Code makes its own, and refreshed before it expires and on a 401.
//!
//! **Cancellation.** A request runs on a socket of its own ([`harness_runtime::Socket`])
//! through a minimal HTTP/1.1 exchange, so a stop cuts it at once wherever it is
//! blocked: connecting, in the TLS handshake, before the first byte, or
//! mid-stream ([`Client::send`]).
//!
//! **Trust.** Every connection to Anthropic, dictation's WebSocket too,
//! trusts the operating system's certificate store, read when its client is
//! made ([`tls`]). A CA the system stops trusting is distrusted here on the
//! next restart, with no rebuild.
//!
//! **Left to the brain:** breakpoint placement, holding a fan-out until a
//! first byte, retries, and recording the facts in the journal.

mod client;
mod http;
mod login;
pub mod request;
mod response;
mod scripted;

pub use client::{Client, Counted, Finished, Progress};
pub use http::{Body, Response, TrustError, tls};
pub use login::{Login, LoginError, Token};
pub use request::{IDENTITY, RequestError};
pub use scripted::{SCRIPTED, scripted};

use harness_core::environment::Effort;

/// A model on offer, as a face lists it and a role file may name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// The provider's name for it, as a root names it: `claude-opus-5-5`.
    pub id: String,
    /// What a face calls it: the provider's display name.
    pub name: String,
}

/// A model's family, which an alias as in Claude Code names: `opus` for
/// `claude-opus-4-5-20251101`; a model outside the provider's naming is a
/// family of its own.
pub fn family(id: &str) -> &str {
    id.strip_prefix("claude-")
        .and_then(|rest| rest.split('-').next())
        .unwrap_or(id)
}

/// The model a role file's `name` means, from `ids` in order: the id
/// itself, or for an alias (`opus`, `sonnet`, `haiku`) the first id of its
/// [`family`]. The provider lists its models newest first
/// ([`Client::models`]), so an alias means the newest.
pub fn resolve<'a>(name: &str, mut ids: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    ids.find(|id| *id == name || family(id) == name)
}

/// The newest model of each family, in the provider's order: what Max
/// chooses a chat's model from.
pub fn current(models: &[Model]) -> Vec<&Model> {
    let mut seen = Vec::new();
    models
        .iter()
        .filter(|m| {
            let new = !seen.contains(&family(&m.id));
            seen.push(family(&m.id));
            new
        })
        .collect()
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
    /// The exchange broke, or was cut ([`harness_runtime::Cancelled`]).
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Login(e) => e.fmt(f),
            Error::Request(e) => write!(f, "request: {}", e.0),
            Error::Io(e) => write!(f, "the exchange: {e}"),
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
