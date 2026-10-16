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
//! **Left to the brain:** breakpoint placement, holding a fan-out until a
//! first byte, retries, and recording the facts in the journal.

mod client;
mod login;
pub mod request;
mod response;

pub use client::{Client, Finished, Progress};
pub use login::{Login, LoginError, Token};
pub use request::{IDENTITY, RequestError};

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
