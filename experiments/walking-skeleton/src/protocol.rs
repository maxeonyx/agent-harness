use crate::provider::ToolCall;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome<T> {
    Ok { value: T },
    Err { error: String },
    Cancelled { reason: String },
    Panicked { payload: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssistantMessage {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Contribution {
    Fact { text: String },
    Tool { def: serde_json::Value },
}

pub enum BrainCommand {
    TurnEnd,
    Cancel,
    Rebuild,
    Quit,
}

pub enum BrainMsg {
    Command(BrainCommand),
    ToolOutcome {
        call_id: String,
        outcome: Outcome<String>,
    },
}

pub enum LimbMsg {
    Execute { call: ToolCall },
    Cancel,
}

pub enum DisplayItem {
    RequestStarted { request_id: u64 },
    RequestResolved { outcome: Outcome<AssistantMessage> },
    ToolStarted { name: String, arguments: String },
    ToolResolved { outcome: Outcome<String> },
    TurnResolved { outcome: Outcome<()> },
    ContextRebuilt { wire_messages: usize },
    SessionClosed,
}
