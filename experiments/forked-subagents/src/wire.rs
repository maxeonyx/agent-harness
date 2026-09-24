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

/// Everything that is not a completed response is an out-of-band fault: the
/// harness cannot get past it, so the agent cannot be said to have completed.
/// Transient shapes are retried first and only become a fault when the
/// retries run out.
pub struct Fault(pub String);

pub struct Sent {
    pub response: ChatResponse,
    /// The raw response body, for the wire log.
    pub body: serde_json::Value,
}

const ATTEMPTS: usize = 4;

pub async fn send(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    request: &ChatRequest,
) -> Result<Sent, Fault> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut backoff = Duration::from_millis(500);
    let mut last = String::new();
    for attempt in 1..=ATTEMPTS {
        if attempt > 1 {
            tokio::time::sleep(backoff).await;
            backoff *= 3;
        }
        let mut builder = client.post(&url).json(request);
        if let Some(key) = api_key {
            builder = builder.bearer_auth(key);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                last = format!("request failed: {error}");
                continue;
            }
        };
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status.is_success() {
            let body: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| Fault(format!("response was not JSON: {e}; body: {text}")))?;
            // OpenRouter reports some upstream failures as a 200 with an
            // `error` object and no choices.
            if body.get("error").is_some() {
                return Err(Fault(format!("provider returned an error: {text}")));
            }
            let response: ChatResponse = serde_json::from_value(body.clone())
                .map_err(|e| Fault(format!("could not parse response: {e}; body: {text}")))?;
            if response.choices.is_empty() {
                return Err(Fault(format!("provider returned no choices: {text}")));
            }
            return Ok(Sent { response, body });
        }
        let transient =
            status.as_u16() == 408 || status.as_u16() == 429 || status.is_server_error();
        last = format!("provider returned {status}: {text}");
        if !transient {
            return Err(Fault(last));
        }
    }
    Err(Fault(format!("{ATTEMPTS} attempts failed; last: {last}")))
}
