//! Server tool uses the provider will not answer, left out of a request.
//!
//! A server tool (`web_fetch`, `web_search`) runs inside a response: its
//! `server_tool_use` and its result are both blocks of the model's
//! message. When the model also makes a client tool use, the response can
//! stop for that before a server tool use has its result. The provider then
//! answers it at the start of the next response, but only when the request
//! ends with that message's tool results and nothing else: with any other
//! block after them it refuses the whole request (400, "tool use with id
//! ... was found without a corresponding ... result block"), and that
//! context could never be sent again. The record keeps the response as it
//! came; only what is sent leaves the unanswered use out.

use std::collections::HashSet;

use harness_core::context::Role;
use serde_json::Value;
use serde_json::value::RawValue;

/// `contents`, each message's content blocks in order with its role, with
/// every server tool use left out that has no result in any message and
/// that the provider will not answer next. The others' bytes are unchanged.
pub(crate) fn answerable(contents: &mut [(Role, Vec<u8>)]) {
    if !contents
        .iter()
        .any(|(_, c)| c.windows(USE.len()).any(|w| w == USE))
    {
        return;
    }
    let answered: HashSet<String> = contents
        .iter()
        .filter(|(role, _)| *role == Role::Assistant)
        .flat_map(|(_, c)| fields(c))
        .filter(|b| {
            b["type"]
                .as_str()
                .is_some_and(|t| t.ends_with("_tool_result"))
        })
        .filter_map(|b| b["tool_use_id"].as_str().map(String::from))
        .collect();
    // The last response, when only its tool results follow it: the
    // provider answers what it left unanswered.
    let last = contents
        .iter()
        .rposition(|(role, _)| *role == Role::Assistant);
    let pending = last.filter(|&i| {
        contents[i + 1..]
            .iter()
            .all(|(_, c)| fields(c).iter().all(|b| b["type"] == "tool_result"))
    });
    for (i, (role, content)) in contents.iter_mut().enumerate() {
        if *role != Role::Assistant || Some(i) == pending {
            continue;
        }
        let Ok(blocks) = serde_json::from_slice::<Vec<&RawValue>>(content) else {
            continue;
        };
        let kept: Vec<&str> = blocks
            .iter()
            .map(|b| b.get())
            .filter(|b| {
                let block: Value = serde_json::from_str(b).unwrap_or_default();
                block["type"] != "server_tool_use"
                    || block["id"].as_str().is_some_and(|id| answered.contains(id))
            })
            .collect();
        if kept.len() < blocks.len() {
            *content = format!("[{}]", kept.join(",")).into_bytes();
        }
    }
}

const USE: &[u8] = b"\"server_tool_use\"";

/// A message's content blocks as JSON values; none if it is not an array.
fn fields(content: &[u8]) -> Vec<Value> {
    serde_json::from_slice(content).unwrap_or_default()
}
