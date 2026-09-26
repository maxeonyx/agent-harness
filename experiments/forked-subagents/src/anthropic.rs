//! Anthropic's Messages API, on Max's Claude subscription.
//!
//! Anthropic accepts a subscription token only with the `oauth-2025-04-20`
//! beta, and Opus only when `system[0]` is the Claude Code identity string;
//! without it the answer is a 429 that is not a rate limit. Both were
//! measured for Max's opencode plugin
//! (`~/.config/opencode/plugins/anthropic-subscription/index.js`). Its third
//! trick, editing opencode's own prompt, is not needed here: that prompt is
//! fingerprinted as a third-party app, and this one is not.
//!
//! The token is the one opencode keeps and refreshes. It is read before every
//! request and never refreshed here: refresh tokens rotate, so spending one
//! would sign opencode out.

use crate::wire::{Message, PromptTokensDetails, ToolCall, ToolCallFunction, Usage};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;

pub const VERSION: &str = "2023-06-01";
pub const OAUTH_BETA: &str = "oauth-2025-04-20";
pub const IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";
const MAX_TOKENS: u64 = 16_000;

/// The subscription serves Anthropic's models under their own names:
/// `anthropic/claude-sonnet-5` is `claude-sonnet-5`.
pub fn model_id(model: &str) -> Result<&str, String> {
    model.strip_prefix("anthropic/").ok_or_else(|| {
        format!(
            "the claude backend serves Anthropic models only, named anthropic/<model>; got {model}"
        )
    })
}

/// The request body for a chat-completions transcript. Every system message
/// becomes a system block, in order, and a run of tool results becomes one
/// user message, which is where Anthropic wants the answers to one turn's
/// calls. Caching is switched on for the whole request, as it is on
/// OpenRouter.
pub fn body(model: &str, transcript: &[Message], tools: &[Value]) -> Value {
    let system: Vec<Value> = transcript
        .iter()
        .filter(|message| message.role == "system")
        .map(text_block)
        .collect();
    let mut messages: Vec<Value> = Vec::new();
    for message in transcript.iter().filter(|message| message.role != "system") {
        match message.role.as_str() {
            "user" => messages.push(json!({ "role": "user", "content": [text_block(message)] })),
            "assistant" => {
                let mut content: Vec<Value> = message
                    .reasoning_details
                    .iter()
                    .flat_map(|blocks| blocks.as_array().cloned().unwrap_or_default())
                    .collect();
                if message
                    .content
                    .as_deref()
                    .is_some_and(|text| !text.is_empty())
                {
                    content.push(text_block(message));
                }
                for call in message.tool_calls.iter().flatten() {
                    let input: Value = serde_json::from_str(&call.function.arguments)
                        .expect("tool arguments are Anthropic's own `input`, re-serialized");
                    content.push(json!({
                        "type": "tool_use", "id": call.id, "name": call.function.name, "input": input,
                    }));
                }
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            "tool" => {
                let result = json!({
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id,
                    "content": message.content.as_deref().unwrap_or(""),
                });
                match messages.last_mut() {
                    Some(last) if is_tool_results(last) => {
                        last["content"].as_array_mut().unwrap().push(result)
                    }
                    _ => messages.push(json!({ "role": "user", "content": [result] })),
                }
            }
            other => panic!("a transcript message has role {other}"),
        }
    }
    json!({
        "model": model_id(model).expect("the model was checked when the run was configured"),
        "max_tokens": MAX_TOKENS,
        "cache_control": { "type": "ephemeral" },
        "system": system,
        "messages": messages,
        "tools": tools.iter().map(tool).collect::<Vec<_>>(),
    })
}

fn text_block(message: &Message) -> Value {
    json!({ "type": "text", "text": message.content.as_deref().unwrap_or("") })
}

fn is_tool_results(message: &Value) -> bool {
    message["role"] == "user"
        && message["content"]
            .as_array()
            .is_some_and(|blocks| blocks.iter().all(|b| b["type"] == "tool_result"))
}

fn tool(schema: &Value) -> Value {
    let function = &schema["function"];
    json!({
        "name": function["name"],
        "description": function["description"],
        "input_schema": function["parameters"],
    })
}

#[derive(Deserialize)]
struct Response {
    content: Vec<Value>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    output_tokens: u64,
}

/// The assistant message and its usage. The subscription is not billed per
/// token, so every request costs $0.
///
/// Thinking blocks are kept whole, signature and all, in the message's
/// `reasoning_details`, and sent back at the head of the same turn: Anthropic
/// refuses a tool result whose turn has lost its thinking.
pub fn reply(body: &Value) -> Result<(Message, Usage), String> {
    let response: Response = serde_json::from_value(body.clone())
        .map_err(|e| format!("could not parse response: {e}; body: {body}"))?;
    if response.stop_reason.as_deref() == Some("max_tokens") {
        return Err(format!(
            "the response hit max_tokens ({MAX_TOKENS}) before it finished"
        ));
    }
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut thinking = Vec::new();
    for block in response.content {
        match block["type"].as_str().unwrap_or("") {
            "text" => text.push_str(block["text"].as_str().unwrap_or("")),
            "tool_use" => calls.push(ToolCall {
                id: block["id"].as_str().unwrap_or("").to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: block["name"].as_str().unwrap_or("").to_string(),
                    arguments: block["input"].to_string(),
                },
            }),
            "thinking" | "redacted_thinking" => thinking.push(block),
            other => return Err(format!("the response had a `{other}` block: {body}")),
        }
    }
    let usage = response.usage;
    let message = Message {
        role: "assistant".to_string(),
        content: Some(text),
        tool_calls: (!calls.is_empty()).then_some(calls),
        tool_call_id: None,
        reasoning_details: (!thinking.is_empty()).then_some(Value::Array(thinking)),
    };
    Ok((
        message,
        Usage {
            prompt_tokens: usage.input_tokens
                + usage.cache_creation_input_tokens
                + usage.cache_read_input_tokens,
            completion_tokens: usage.output_tokens,
            cost: 0.0,
            prompt_tokens_details: PromptTokensDetails {
                cached_tokens: usage.cache_read_input_tokens,
                cache_write_tokens: usage.cache_creation_input_tokens,
            },
        },
    ))
}

/// The identity rejection arrives as a 429 whose message is literally
/// "Error". Waiting it out can never work.
pub fn is_identity_rejection(status: u16, text: &str) -> bool {
    status == 429
        && serde_json::from_str::<Value>(text).is_ok_and(|body| body["error"]["message"] == "Error")
}

/// The access token opencode stored for its `anthropic` integration.
pub fn access_token(db: &Path) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Stored {
        access: String,
        expires: i64,
    }
    let connection =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("open opencode's credential store {}: {e}", db.display()))?;
    let value: String = connection
        .query_row(
            "SELECT value FROM credential WHERE integration_id = 'anthropic' ORDER BY time_updated DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| {
            format!(
                "no Claude credential in {}: {e}. Sign in to your Claude subscription in opencode first.",
                db.display()
            )
        })?;
    let stored: Stored = serde_json::from_str(&value).map_err(|e| {
        format!(
            "the Claude credential in {} is not an OAuth token: {e}",
            db.display()
        )
    })?;
    // A minute's margin, so the token does not expire in flight.
    if stored.expires <= chrono::Utc::now().timestamp_millis() + 60_000 {
        let at = chrono::DateTime::from_timestamp_millis(stored.expires)
            .map(|at| {
                at.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M %Z")
                    .to_string()
            })
            .unwrap_or_else(|| stored.expires.to_string());
        return Err(format!(
            "the Claude subscription token in {} expired at {at}. opencode refreshes it the next time it talks to Claude: send a message in an opencode session on an anthropic model, then try again.",
            db.display()
        ));
    }
    Ok(stored.access)
}
