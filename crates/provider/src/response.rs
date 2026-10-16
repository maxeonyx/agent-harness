//! A streamed response, assembled into the assistant message it becomes.

use harness_core::model_call::{Stop, Usage};
use serde_json::{Map, Value};

/// The response so far: its content blocks, why it stopped, and its usage.
#[derive(Default)]
pub(crate) struct Assembly {
    blocks: Vec<Block>,
    pub(crate) stop: Option<Stop>,
    usage: Option<Map<String, Value>>,
}

/// One content block: the object `content_block_start` gave, with its deltas
/// applied, and the raw text of its streamed `input` if it has one.
struct Block {
    fields: Map<String, Value>,
    input: Option<String>,
}

impl Assembly {
    /// Applies one event's data. `Err` is a stream this does not understand,
    /// which must not be stored as if it were the model's.
    pub(crate) fn apply(&mut self, name: &str, data: &Value) -> Result<(), String> {
        match name {
            "message_start" => self.usage = data["message"]["usage"].as_object().cloned(),
            "content_block_start" => {
                let index = data["index"].as_u64();
                if index != Some(self.blocks.len() as u64) {
                    return Err(format!("content block {index:?} out of order"));
                }
                let fields = data["content_block"]
                    .as_object()
                    .cloned()
                    .ok_or("content_block_start without a block")?;
                self.blocks.push(Block {
                    fields,
                    input: None,
                });
            }
            "content_block_delta" => {
                let block = data["index"]
                    .as_u64()
                    .and_then(|i| self.blocks.get_mut(i as usize))
                    .ok_or("a delta for a block that has not started")?;
                block.apply(&data["delta"])?;
            }
            "message_delta" => {
                if let Some(reason) = data["delta"]["stop_reason"].as_str() {
                    self.stop = Some(stop(reason));
                }
                if let (Some(usage), Some(more)) = (&mut self.usage, data["usage"].as_object()) {
                    for (k, v) in more {
                        if !v.is_null() {
                            usage.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The `content` array of the assistant message: the first and only
    /// time these bytes are written.
    pub(crate) fn content(&self) -> Result<Vec<u8>, String> {
        let blocks: Result<Vec<_>, _> = self.blocks.iter().map(Block::render).collect();
        Ok(format!("[{}]", blocks?.join(",")).into_bytes())
    }

    /// The visible text so far, as a `content` array, if there is any.
    pub(crate) fn visible(&self) -> Option<Vec<u8>> {
        let text: Vec<_> = self
            .blocks
            .iter()
            .filter(|b| b.fields.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.render().ok())
            .collect();
        (!text.is_empty()).then(|| format!("[{}]", text.join(",")).into_bytes())
    }

    pub(crate) fn usage(&self) -> Option<Usage> {
        let usage = self.usage.as_ref()?;
        let n = |v: Option<&Value>| v.and_then(Value::as_u64).unwrap_or(0);
        let creation = usage.get("cache_creation");
        let (short, long) = match creation {
            Some(c) => (
                n(c.get("ephemeral_5m_input_tokens")),
                n(c.get("ephemeral_1h_input_tokens")),
            ),
            None => (n(usage.get("cache_creation_input_tokens")), 0),
        };
        Some(Usage {
            input: n(usage.get("input_tokens")),
            cache_read: n(usage.get("cache_read_input_tokens")),
            cache_write_short: short,
            cache_write_long: long,
            output: n(usage.get("output_tokens")),
        })
    }
}

impl Block {
    fn apply(&mut self, delta: &Value) -> Result<(), String> {
        let append = |fields: &mut Map<String, Value>, key: &str| {
            let more = delta[key]
                .as_str()
                .ok_or(format!("{key} delta without text"))?;
            let mut text = fields
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            text.push_str(more);
            fields.insert(key.into(), text.into());
            Ok::<(), String>(())
        };
        match delta["type"].as_str() {
            Some("text_delta") => append(&mut self.fields, "text"),
            Some("thinking_delta") => append(&mut self.fields, "thinking"),
            Some("signature_delta") => append(&mut self.fields, "signature"),
            Some("input_json_delta") => {
                let more = delta["partial_json"]
                    .as_str()
                    .ok_or("input delta without json")?;
                self.input.get_or_insert_default().push_str(more);
                Ok(())
            }
            Some("citations_delta") => {
                let citations = self
                    .fields
                    .entry("citations")
                    .or_insert_with(|| Value::Array(vec![]));
                match citations {
                    Value::Array(list) => list.push(delta["citation"].clone()),
                    _ => *citations = Value::Array(vec![delta["citation"].clone()]),
                }
                Ok(())
            }
            other => Err(format!("unknown delta type {other:?}")),
        }
    }

    /// The block's JSON. A streamed `input` is spliced in as the model wrote
    /// it, so its key order is the model's.
    fn render(&self) -> Result<String, String> {
        let Some(input) = self.input.as_deref().filter(|i| !i.trim().is_empty()) else {
            return Ok(Value::Object(self.fields.clone()).to_string());
        };
        serde_json::from_str::<Value>(input)
            .map_err(|e| format!("streamed tool input is not JSON: {e}"))?;
        let mut fields = self.fields.clone();
        fields.remove("input");
        let head = Value::Object(fields).to_string();
        let open = &head[..head.len() - 1];
        let comma = if open.ends_with('{') { "" } else { "," };
        Ok(format!("{open}{comma}\"input\":{input}}}"))
    }
}

fn stop(reason: &str) -> Stop {
    match reason {
        "end_turn" => Stop::EndTurn,
        "tool_use" => Stop::ToolUse,
        "max_tokens" => Stop::MaxTokens,
        "pause_turn" => Stop::PauseTurn,
        "refusal" => Stop::Refusal,
        "model_context_window_exceeded" => Stop::ContextWindow,
        other => Stop::Other(other.into()),
    }
}
