//! Toy brain: owns the session context, the provider client and credentials,
//! and the agent loop. Appending context and triggering inference are
//! distinct operations; passive user activity only ever appends.

use crate::limb::Limb;
use crate::provider::{self, ChatRequest, ToolCall, WireMessage};
use crate::recorder::Recorder;
use serde_json::json;

const SYSTEM_PROMPT: &str = "You are a toy agent harness (walking skeleton). \
You can list files, read files, and run bash commands in the user's working \
directory via tools. Lines marked [user activity] describe things the user \
did in their own tools; they are context, not requests. Answer the user's \
typed messages, using tools when helpful.";

pub struct Config {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            base_url: std::env::var("SKELETON_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8089/v1".to_string()),
            api_key: std::env::var("SKELETON_API_KEY").ok(),
            model: std::env::var("SKELETON_MODEL").unwrap_or_else(|_| "fake-model".to_string()),
        }
    }
}

enum Entry {
    User(String),
    UserActivity(String),
    Assistant(String),
    AssistantToolCalls(Vec<ToolCall>),
    ToolResult { id: String, content: String },
}

pub struct Brain {
    config: Config,
    limb: Limb,
    recorder: Recorder,
    context: Vec<Entry>,
}

impl Brain {
    pub fn new(config: Config, limb: Limb, recorder: Recorder) -> Self {
        Brain {
            config,
            limb,
            recorder,
            context: Vec::new(),
        }
    }

    pub fn append_user_message(&mut self, text: String) {
        self.recorder
            .record("append_user_message", json!({ "text": text }));
        self.context.push(Entry::User(text));
    }

    pub fn append_user_activity(&mut self, text: String) {
        self.recorder
            .record("append_user_activity", json!({ "text": text }));
        self.context.push(Entry::UserActivity(text));
    }

    /// The agent loop: send a request, dispatch tool calls through the limb,
    /// repeat until the model answers with plain text.
    pub fn end_turn(&mut self) -> Result<(), String> {
        self.recorder.record("trigger_inference", json!({}));
        loop {
            let request = self.build_request();
            self.recorder.record(
                "request_sent",
                serde_json::to_value(&request).unwrap_or_default(),
            );
            let response = provider::send(
                &self.config.base_url,
                self.config.api_key.as_deref(),
                &request,
            )?;
            let message = response
                .choices
                .into_iter()
                .next()
                .ok_or("provider response had no choices")?
                .message;
            self.recorder.record(
                "response_received",
                serde_json::to_value(&message).unwrap_or_default(),
            );

            let tool_calls = message.tool_calls.unwrap_or_default();
            if tool_calls.is_empty() {
                let text = message.content.unwrap_or_default();
                println!("[agent] {text}");
                self.recorder
                    .record("append_assistant", json!({ "text": text }));
                self.context.push(Entry::Assistant(text));
                return Ok(());
            }

            self.context
                .push(Entry::AssistantToolCalls(tool_calls.clone()));
            for call in tool_calls {
                println!(
                    "[limb] tool call: {}({})",
                    call.function.name, call.function.arguments
                );
                self.recorder.record(
                    "tool_call",
                    json!({ "id": call.id, "name": call.function.name,
                            "arguments": call.function.arguments }),
                );
                let result = self
                    .limb
                    .execute(&call.function.name, &call.function.arguments);
                println!("[limb] tool result: {} bytes", result.len());
                self.recorder
                    .record("tool_result", json!({ "id": call.id, "content": result }));
                self.context.push(Entry::ToolResult {
                    id: call.id,
                    content: result,
                });
            }
        }
    }

    fn build_request(&self) -> ChatRequest {
        let mut messages = vec![WireMessage {
            role: "system".to_string(),
            content: Some(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }];
        for entry in &self.context {
            messages.push(match entry {
                Entry::User(text) => WireMessage {
                    role: "user".to_string(),
                    content: Some(text.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                Entry::UserActivity(text) => WireMessage {
                    role: "user".to_string(),
                    content: Some(format!("[user activity] {text}")),
                    tool_calls: None,
                    tool_call_id: None,
                },
                Entry::Assistant(text) => WireMessage {
                    role: "assistant".to_string(),
                    content: Some(text.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                Entry::AssistantToolCalls(calls) => WireMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(calls.clone()),
                    tool_call_id: None,
                },
                Entry::ToolResult { id, content } => WireMessage {
                    role: "tool".to_string(),
                    content: Some(content.clone()),
                    tool_calls: None,
                    tool_call_id: Some(id.clone()),
                },
            });
        }
        ChatRequest {
            model: self.config.model.clone(),
            messages,
            tools: Some(self.limb.tool_defs()),
        }
    }
}
