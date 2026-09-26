//! The provider wire: the transcript's message types, and the two APIs that
//! carry them.
//!
//! A message is stored exactly as it is sent, so a forked child can clone its
//! parent's `Vec<Message>` and serialize to the same bytes. The transcript is
//! chat-completions shaped; the Claude backend translates it as it is sent,
//! which is a pure function of the messages and keeps that true. Nothing in
//! here knows about agents.

use crate::anthropic;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Backend {
    /// OpenRouter's chat-completions API, paid per token from a key.
    OpenRouter {
        base_url: String,
        api_key: Option<String>,
        /// Pinned routing. Unpinned, OpenRouter sent parallel forks to a
        /// backend that had never seen the parent's prefix (probe,
        /// 2026-09-24).
        provider: Option<String>,
    },
    /// Anthropic's Messages API on Max's Claude subscription, with the token
    /// opencode keeps in `credentials`.
    Claude {
        base_url: String,
        credentials: PathBuf,
    },
}

impl Backend {
    pub fn name(&self) -> &'static str {
        match self {
            Backend::OpenRouter { .. } => "openrouter",
            Backend::Claude { .. } => "claude",
        }
    }

    pub fn provider(&self) -> Option<&str> {
        match self {
            Backend::OpenRouter { provider, .. } => provider.as_deref(),
            Backend::Claude { .. } => None,
        }
    }

    /// The system messages every agent in a run starts with.
    pub fn system(&self, prompt: &str) -> Vec<Message> {
        let mut system = Vec::new();
        if let Backend::Claude { .. } = self {
            system.push(Message::new("system", anthropic::IDENTITY));
        }
        system.push(Message::new("system", prompt));
        system
    }

    /// The request body, exactly as it is sent and recorded.
    pub fn body(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[Value],
        session_id: &str,
    ) -> Value {
        match self {
            Backend::OpenRouter { provider, .. } => serde_json::to_value(ChatRequest {
                model: model.to_string(),
                messages: messages.to_vec(),
                tools: tools.to_vec(),
                cache_control: serde_json::json!({ "type": "ephemeral" }),
                provider: provider
                    .as_ref()
                    .map(|slug| serde_json::json!({ "order": [slug], "allow_fallbacks": false })),
                session_id: session_id.to_string(),
            })
            .expect("a request of strings and JSON values serializes"),
            Backend::Claude { .. } => anthropic::body(model, messages, tools),
        }
    }
}

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
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    tools: Vec<Value>,
    /// Anthropic-style prompt caching, switched on for the whole request.
    cache_control: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<Value>,
    /// Shared by every agent in one run, for sticky routing. Empty means
    /// the field is not sent at all.
    #[serde(skip_serializing_if = "String::is_empty")]
    session_id: String,
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

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Usage,
    #[serde(default)]
    provider: Option<String>,
}

/// What one HTTP attempt came back with. Retrying is the caller's decision,
/// because only the caller knows whether the run has been cancelled and
/// whether another attempt is still inside the spend cap — both of which must
/// be checked per attempt, not per logical request.
pub enum Attempt {
    Answered(Box<Sent>),
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
    pub message: Message,
    pub usage: Usage,
    /// Who answered: OpenRouter's upstream provider, or which of the
    /// subscription's limits the request was billed to.
    pub via: String,
    /// The raw response body, for the wire log.
    pub body: Value,
}

/// Seconds, which is what both APIs send. An HTTP-date `Retry-After` is
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

fn classify(code: u16, reason: String, wait: Option<Duration>) -> Attempt {
    match retryable_status(code) {
        Some(rate_limited) => Attempt::Transient(Retryable {
            reason,
            rate_limited,
            retry_after: wait,
        }),
        None => Attempt::Fatal(reason),
    }
}

pub async fn send_once(client: &reqwest::Client, backend: &Backend, body: &Value) -> Attempt {
    let request = match backend {
        Backend::OpenRouter {
            base_url, api_key, ..
        } => {
            let request = client
                .post(format!(
                    "{}/chat/completions",
                    base_url.trim_end_matches('/')
                ))
                .json(body);
            match api_key {
                Some(key) => request.bearer_auth(key),
                None => request,
            }
        }
        Backend::Claude {
            base_url,
            credentials,
        } => match anthropic::access_token(credentials) {
            Ok(token) => client
                .post(format!("{}/v1/messages", base_url.trim_end_matches('/')))
                .bearer_auth(token)
                .header("anthropic-version", anthropic::VERSION)
                .header("anthropic-beta", anthropic::OAUTH_BETA)
                .json(body),
            Err(error) => return Attempt::Fatal(error),
        },
    };
    let response = match request.send().await {
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
    let claim = response
        .headers()
        .get("anthropic-ratelimit-unified-representative-claim")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("?")
        .to_string();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let reason = format!("provider returned {status}: {text}");
        if let Backend::Claude { .. } = backend
            && anthropic::is_identity_rejection(status.as_u16(), &text)
        {
            return Attempt::Fatal(format!(
                "Anthropic rejected the request identity: system[0] was not accepted as Claude Code's. This is a 429 but not a rate limit, so it is not retried. {reason}"
            ));
        }
        return classify(status.as_u16(), reason, wait);
    }
    let body: Value = match serde_json::from_str(&text) {
        Ok(body) => body,
        Err(error) => {
            return Attempt::Fatal(format!("response was not JSON: {error}; body: {text}"));
        }
    };
    match backend {
        Backend::OpenRouter { .. } => openrouter_reply(body, &text, wait),
        Backend::Claude { .. } => match anthropic::reply(&body) {
            Ok((message, usage)) => Attempt::Answered(Box::new(Sent {
                message,
                usage,
                via: format!("claude subscription, billed to {claim}"),
                body,
            })),
            Err(error) => Attempt::Fatal(error),
        },
    }
}

fn openrouter_reply(body: Value, text: &str, wait: Option<Duration>) -> Attempt {
    // OpenRouter reports upstream failures as a 200 carrying an `error`
    // object — including rate limits, which arrive as `"code": 429` with an
    // HTTP 200. Reading only the HTTP status treats a "come back shortly" as
    // permanent, which is how a whole grid of trials died in seconds.
    if let Some(error) = body.get("error") {
        let code = error
            .get("code")
            .and_then(|code| code.as_u64())
            .unwrap_or(0) as u16;
        return classify(code, format!("provider returned an error: {text}"), wait);
    }
    let response: ChatResponse = match serde_json::from_value(body.clone()) {
        Ok(response) => response,
        Err(error) => {
            return Attempt::Fatal(format!("could not parse response: {error}; body: {text}"));
        }
    };
    let Some(choice) = response.choices.into_iter().next() else {
        return Attempt::Fatal(format!("provider returned no choices: {text}"));
    };
    Attempt::Answered(Box::new(Sent {
        message: choice.message,
        usage: response.usage,
        via: response.provider.unwrap_or_else(|| "?".to_string()),
        body,
    }))
}
