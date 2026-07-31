//! OpenAI-compatible chat-completions wire types and the async HTTP client.
//! The same client talks to a real provider or the fake-provider server;
//! only the base URL and API key differ. Requests are cancellable by
//! dropping the future (the brain's request task races this future against
//! its cancellation token and is signalled and joined, never abandoned —
//! with the completed-response side of the race winning ties).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON-encoded arguments object, as the OpenAI format specifies.
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

fn function_type() -> String {
    "function".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WireMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub message: WireMessage,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

pub async fn send(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    request: &ChatRequest,
) -> Result<ChatResponse, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut req = client.post(&url).json(request);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let response = req
        .send()
        .await
        .map_err(|e| format!("provider request failed: {e}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("read response: {e}"))?;
    if !status.is_success() {
        return Err(format!("provider returned {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("parse response: {e}; body: {text}"))
}
