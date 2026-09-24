//! The provider wire: chat-completions message types and the HTTP client.
//!
//! A message is stored exactly as it is sent, so a forked child can clone its
//! parent's `Vec<Message>` and serialize to the same bytes. Nothing in here
//! knows about agents.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON-encoded arguments object, as the chat-completions format specifies.
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

fn function_type() -> String {
    "function".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Echoed back verbatim when the provider sends it; some models reject a
    /// later request whose assistant turn dropped its reasoning blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<serde_json::Value>,
}

impl Message {
    pub fn new(role: &str, content: &str) -> Self {
        Message {
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_details: None,
        }
    }

    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Message {
            role: "tool".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            reasoning_details: None,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<serde_json::Value>,
    /// Anthropic-style prompt caching, switched on for the whole request.
    pub cache_control: serde_json::Value,
    /// Pinned routing. Unpinned, OpenRouter sent parallel forks to a backend
    /// that had never seen the parent's prefix (probe, 2026-09-24).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<serde_json::Value>,
    /// Shared by every agent in one run, for sticky routing. Empty means
    /// the field is not sent at all.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session_id: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

/// Required, not defaulted: a spend cap cannot be enforced against a response
/// that did not say what it cost, so a response without usage is a fault.
#[derive(Deserialize, Debug, Clone)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    pub cost: f64,
    #[serde(default)]
    pub prompt_tokens_details: PromptTokensDetails,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub message: Message,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    #[serde(default)]
    pub choices: Vec<Choice>,
    pub usage: Usage,
    #[serde(default)]
    pub provider: Option<String>,
}

/// What one HTTP attempt came back with. Retrying is the caller's decision,
/// because only the caller knows whether the run has been cancelled and
/// whether another attempt is still inside the spend cap — both of which must
/// be checked per attempt, not per logical request.
pub enum Attempt {
    Answered(Sent),
    /// Worth another attempt: the network, a 429, a 5xx, a timeout.
    Transient(Retryable),
    /// An out-of-band fault. The harness cannot get past it, so the agent
    /// cannot be said to have completed.
    Fatal(String),
}

pub struct Retryable {
    pub reason: String,
    /// Rate limits deserve far more patience than a 5xx: the provider is
    /// telling us to come back, not failing.
    pub rate_limited: bool,
    /// What the provider asked us to wait, when it said.
    pub retry_after: Option<Duration>,
}

pub struct Sent {
    pub response: ChatResponse,
    /// The raw response body, for the wire log.
    pub body: serde_json::Value,
}

/// Seconds, which is what OpenRouter sends. An HTTP-date `Retry-After` is
/// ignored in favour of the backoff schedule rather than parsed.
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// A status that is worth another attempt, and whether it is a rate limit.
fn retryable_status(code: u16) -> Option<bool> {
    match code {
        429 => Some(true),
        408 | 500..=599 => Some(false),
        _ => None,
    }
}

pub async fn send_once(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    request: &ChatRequest,
) -> Attempt {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut builder = client.post(&url).json(request);
    if let Some(key) = api_key {
        builder = builder.bearer_auth(key);
    }
    let response = match builder.send().await {
        Ok(response) => response,
        Err(error) => {
            return Attempt::Transient(Retryable {
                reason: format!("request failed: {error}"),
                rate_limited: false,
                retry_after: None,
            });
        }
    };
    let status = response.status();
    let wait = retry_after(&response);
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let reason = format!("provider returned {status}: {text}");
        return match retryable_status(status.as_u16()) {
            Some(rate_limited) => Attempt::Transient(Retryable {
                reason,
                rate_limited,
                retry_after: wait,
            }),
            None => Attempt::Fatal(reason),
        };
    }
    let body: serde_json::Value = match serde_json::from_str(&text) {
        Ok(body) => body,
        Err(error) => {
            return Attempt::Fatal(format!("response was not JSON: {error}; body: {text}"));
        }
    };
    // OpenRouter reports upstream failures as a 200 carrying an `error`
    // object — including rate limits, which arrive as `"code": 429` with an
    // HTTP 200. Reading only the HTTP status treats a "come back shortly" as
    // permanent, which is how a whole grid of trials died in seconds.
    if let Some(error) = body.get("error") {
        let reason = format!("provider returned an error: {text}");
        let code = error
            .get("code")
            .and_then(|code| code.as_u64())
            .unwrap_or(0) as u16;
        return match retryable_status(code) {
            Some(rate_limited) => Attempt::Transient(Retryable {
                reason,
                rate_limited,
                retry_after: wait,
            }),
            None => Attempt::Fatal(reason),
        };
    }
    let response: ChatResponse = match serde_json::from_value(body.clone()) {
        Ok(response) => response,
        Err(error) => {
            return Attempt::Fatal(format!("could not parse response: {error}; body: {text}"));
        }
    };
    if response.choices.is_empty() {
        return Attempt::Fatal(format!("provider returned no choices: {text}"));
    }
    Attempt::Answered(Sent { response, body })
}
