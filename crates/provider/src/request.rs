//! A request's wire bytes, spliced from stored entry bytes.

use harness_core::context::{Entry, EntryId, Message, Role, Root, Wire};
use harness_core::model_call::{Breakpoint, ModelCall, Ttl};

/// The identity string the subscription requires as `system[0]`. Without it
/// Opus refuses with a 429 that pretends to be a rate limit.
pub const IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// The provider's limit on `cache_control` markers per request.
const MAX_BREAKPOINTS: usize = 4;

/// Why a request could not be built. Nothing was sent.
#[derive(Debug)]
pub struct RequestError(pub String);

/// The JSON body of `call`, with every stored byte copied as it is.
///
/// `entry` looks up a stored entry by id. The body is the root's model,
/// tools and system, the chain of messages to `call.head`, and the members
/// of `call.params`; a breakpoint adds `cache_control` to the last block of
/// the entry it names (the last `system` block, for a root). Nothing is
/// parsed and written again, so what was stored is what is sent.
pub fn body(
    call: &ModelCall,
    mut entry: impl FnMut(EntryId) -> Option<Entry>,
) -> Result<String, RequestError> {
    let (root, messages) = chain(call.head, &mut entry)?;
    check_identity(&root.entry.system)?;
    let params = params(&call.params)?;
    if call.breakpoints.len() > MAX_BREAKPOINTS {
        return Err(RequestError(format!(
            "{} breakpoints; the provider allows {MAX_BREAKPOINTS}",
            call.breakpoints.len()
        )));
    }
    let ttl = |id: EntryId| {
        let mut at = call.breakpoints.iter().filter(|b| b.at == id).peekable();
        at.peek()?;
        Some(if at.any(|b| b.ttl == Ttl::Long) {
            Ttl::Long
        } else {
            Ttl::Short
        })
    };
    let marked = |wire: &Wire, id: EntryId| match ttl(id) {
        Some(ttl) => mark(&wire.0, ttl),
        None => Ok(wire.0.clone()),
    };
    let on_chain = |b: &Breakpoint| b.at == root.id || messages.iter().any(|(id, _)| *id == b.at);
    if let Some(stray) = call.breakpoints.iter().find(|b| !on_chain(b)) {
        return Err(RequestError(format!(
            "breakpoint at {:?} is not on the head's chain",
            stray.at
        )));
    }

    let mut out = String::new();
    out.push_str("{\"model\":");
    out.push_str(&serde_json::Value::from(root.entry.model.as_str()).to_string());
    out.push_str(",\"tools\":");
    out.push_str(&root.entry.tools.0);
    out.push_str(",\"system\":");
    out.push_str(&marked(&root.entry.system, root.id)?);
    out.push_str(",\"messages\":[");
    for (i, (id, message)) in messages.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => {
                return Err(RequestError(
                    "a mid-conversation system message is not verified on this wire".into(),
                ));
            }
        };
        out.push_str("{\"role\":\"");
        out.push_str(role);
        out.push_str("\",\"content\":");
        out.push_str(&marked(&message.content, *id)?);
        out.push('}');
    }
    out.push(']');
    if !params.is_empty() {
        out.push(',');
        out.push_str(params);
    }
    out.push('}');
    Ok(out)
}

struct RootAt {
    id: EntryId,
    entry: Root,
}

/// The root and the messages from it to `head`, in order.
fn chain(
    head: EntryId,
    entry: &mut impl FnMut(EntryId) -> Option<Entry>,
) -> Result<(RootAt, Vec<(EntryId, Message)>), RequestError> {
    let mut messages = Vec::new();
    let mut at = head;
    loop {
        let found = entry(at).ok_or_else(|| RequestError(format!("no entry {at:?}")))?;
        if found.id() != at {
            return Err(RequestError(format!(
                "the entry stored as {at:?} has another id"
            )));
        }
        match found {
            Entry::Root(root) => {
                messages.reverse();
                return Ok((
                    RootAt {
                        id: at,
                        entry: root,
                    },
                    messages,
                ));
            }
            Entry::Message(message) => {
                let parent = message.parent;
                messages.push((at, message));
                at = parent;
            }
        }
    }
}

fn check_identity(system: &Wire) -> Result<(), RequestError> {
    let blocks: serde_json::Value = serde_json::from_str(&system.0)
        .map_err(|e| RequestError(format!("the root's system is not JSON: {e}")))?;
    if blocks[0]["text"] != IDENTITY {
        return Err(RequestError(
            "the root's system[0] is not the subscription's identity string".into(),
        ));
    }
    Ok(())
}

/// The members of the params object, without its braces.
fn params(params: &Wire) -> Result<&str, RequestError> {
    let value: serde_json::Value = serde_json::from_str(&params.0)
        .map_err(|e| RequestError(format!("params are not JSON: {e}")))?;
    let object = value
        .as_object()
        .ok_or(RequestError("params are not a JSON object".into()))?;
    if object.get("stream") != Some(&serde_json::Value::Bool(true)) {
        return Err(RequestError("params must set \"stream\": true".into()));
    }
    if let Some(key) = ["model", "system", "tools", "messages"]
        .into_iter()
        .find(|k| object.contains_key(*k))
    {
        return Err(RequestError(format!(
            "params set {key:?}, which the context owns"
        )));
    }
    let text = params.0.trim();
    Ok(text[1..text.len() - 1].trim())
}

/// `blocks` (a JSON array of objects) with `cache_control` added to its last
/// block, spliced in before that block's closing brace.
fn mark(blocks: &str, ttl: Ttl) -> Result<String, RequestError> {
    let not_blocks = || RequestError("a breakpoint's entry does not end in a block".into());
    let trimmed = blocks.trim_end();
    let inner = trimmed.strip_suffix(']').ok_or_else(not_blocks)?.trim_end();
    if !inner.ends_with('}') {
        return Err(not_blocks());
    }
    let at = inner.len() - 1;
    let control = match ttl {
        Ttl::Short => r#","cache_control":{"type":"ephemeral"}"#,
        Ttl::Long => r#","cache_control":{"type":"ephemeral","ttl":"1h"}"#,
    };
    Ok([&blocks[..at], control, &blocks[at..]].concat())
}
