//! A request's wire bytes, spliced from stored values.

use harness_core::context::{Entry, EntryId, Root};
use harness_core::id::ValueId;
use harness_core::model_call::ModelCall;

/// The identity string the subscription requires as `system[0]`. Without it
/// Opus refuses with a 429 that pretends to be a rate limit.
pub const IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// The provider's limit on `cache_control` markers per request.
const MAX_BREAKPOINTS: usize = 4;

/// Why a request could not be built. Nothing was sent.
#[derive(Debug)]
pub struct RequestError(pub String);

/// The JSON body of `call`, with every value's bytes copied as they are.
///
/// `base` is the context `call` extends, root first, as
/// [`harness_core::state::State::context`] gives it for `call.base`. `value`
/// looks up a value's bytes: stored ones, and the new ones `call` names
/// (its appends' contents and its params) that are not committed yet.
///
/// The body is the root's model, tools and system, the messages of `base`
/// and then `call.appends`, `"stream": true` (this client only streams),
/// and the members of `call.params`. A breakpoint
/// adds `cache_control` to the last block of the entry it names (the last
/// `system` block, for a root). Nothing is parsed and written again, so what
/// was stored is what is sent.
pub fn body(
    call: &ModelCall,
    base: &[(EntryId, &Entry)],
    mut value: impl FnMut(ValueId) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, RequestError> {
    let Some(&(root_id, Entry::Root(root))) = base.first() else {
        return Err(RequestError("the context does not start at a root".into()));
    };
    if base.last().map(|(id, _)| *id) != Some(call.base) {
        return Err(RequestError(format!(
            "the context given does not end at the request's base {}",
            call.base
        )));
    }
    let mut messages = Vec::new();
    for (id, entry) in &base[1..] {
        let Entry::Message { message, .. } = entry else {
            return Err(RequestError(format!("the context has a second root, {id}")));
        };
        messages.push((*id, message));
    }
    let mut parent = call.base;
    for message in &call.appends {
        parent = message.id_after(parent);
        messages.push((parent, message));
    }

    if call.breakpoints.len() > MAX_BREAKPOINTS {
        return Err(RequestError(format!(
            "{} breakpoints; the provider allows {MAX_BREAKPOINTS}",
            call.breakpoints.len()
        )));
    }
    let on_chain = |at: EntryId| at == root_id || messages.iter().any(|(id, _)| *id == at);
    if let Some(stray) = call.breakpoints.iter().find(|b| !on_chain(**b)) {
        return Err(RequestError(format!(
            "breakpoint at {stray} is not on the request's chain"
        )));
    }
    let mut bytes = |id: ValueId| {
        let found = value(id).ok_or_else(|| RequestError(format!("no value {id}")))?;
        if ValueId::of(&found) != id {
            return Err(RequestError(format!(
                "the value stored as {id} has another id"
            )));
        }
        Ok(found)
    };
    let marked = |found: Vec<u8>, at: EntryId| {
        if call.breakpoints.contains(&at) {
            mark(&found)
        } else {
            Ok(found)
        }
    };

    let Root {
        model,
        system,
        tools,
    } = root;
    let system = marked(bytes(*system)?, root_id)?;
    check_identity(&system)?;
    let tools = bytes(*tools)?;
    let params = bytes(call.params)?;
    let params = members(&params)?;

    let mut out = Vec::new();
    out.extend_from_slice(b"{\"model\":");
    out.extend_from_slice(
        serde_json::Value::from(model.as_str())
            .to_string()
            .as_bytes(),
    );
    out.extend_from_slice(b",\"tools\":");
    out.extend_from_slice(&tools);
    out.extend_from_slice(b",\"system\":");
    out.extend_from_slice(&system);
    out.extend_from_slice(b",\"messages\":[");
    for (i, (id, message)) in messages.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"role\":\"");
        out.extend_from_slice(message.role.as_str().as_bytes());
        out.extend_from_slice(b"\",\"content\":");
        out.extend_from_slice(&marked(bytes(message.content)?, *id)?);
        out.push(b'}');
    }
    out.extend_from_slice(b"],\"stream\":true");
    if !params.is_empty() {
        out.push(b',');
        out.extend_from_slice(params);
    }
    out.push(b'}');
    Ok(out)
}

fn check_identity(system: &[u8]) -> Result<(), RequestError> {
    let blocks: serde_json::Value = serde_json::from_slice(system)
        .map_err(|e| RequestError(format!("the root's system is not JSON: {e}")))?;
    if blocks[0]["text"] != IDENTITY {
        return Err(RequestError(
            "the root's system[0] is not the subscription's identity string".into(),
        ));
    }
    Ok(())
}

/// The members of the params object, without its braces.
fn members(params: &[u8]) -> Result<&[u8], RequestError> {
    let value: serde_json::Value = serde_json::from_slice(params)
        .map_err(|e| RequestError(format!("params are not JSON: {e}")))?;
    let object = value
        .as_object()
        .ok_or(RequestError("params are not a JSON object".into()))?;
    if let Some(key) = ["model", "system", "tools", "messages", "stream"]
        .into_iter()
        .find(|k| object.contains_key(*k))
    {
        return Err(RequestError(format!(
            "params set {key:?}, which the request writes itself"
        )));
    }
    let text = params.trim_ascii();
    Ok(text[1..text.len() - 1].trim_ascii())
}

/// `blocks` (a JSON array of objects) with `cache_control` added to its last
/// block, spliced in before that block's closing brace, for the provider's
/// default lifetime.
fn mark(blocks: &[u8]) -> Result<Vec<u8>, RequestError> {
    let not_blocks = || RequestError("a breakpoint's entry does not end in a block".into());
    let inner = blocks
        .trim_ascii_end()
        .strip_suffix(b"]")
        .ok_or_else(not_blocks)?
        .trim_ascii_end();
    if !inner.ends_with(b"}") {
        return Err(not_blocks());
    }
    let at = inner.len() - 1;
    let control: &[u8] = br#","cache_control":{"type":"ephemeral"}"#;
    Ok([&blocks[..at], control, &blocks[at..]].concat())
}
