//! The web limb: the provider's own web search and web fetch, which run
//! inside a response. A role that lists `web` gets them; the model's use of
//! them and their results are blocks of its response, with citations, so
//! nothing is dispatched and nothing settles.

use harness_core::environment::{Audience, Description, Limb, LimbKind, Tool};
use harness_core::id::LimbId;
use serde_json::json;

pub fn definition(id: LimbId) -> Limb {
    Limb {
        id,
        name: "web".into(),
        kind: LimbKind::Provider,
    }
}

/// The basic variants: the dynamic-filtering ones (`_20260209`) run the
/// search inside code execution, which the subscription refuses ("no more
/// code_execution tool calls remaining this turn").
pub fn description() -> Description {
    let tool = |name: &str, kind: &str| Tool {
        name: name.into(),
        definition: json!({ "type": kind, "name": name }).to_string(),
        audience: Audience::Agent,
    };
    Description {
        instructions: Vec::new(),
        tools: vec![
            tool("web_search", "web_search_20250305"),
            tool("web_fetch", "web_fetch_20250910"),
        ],
    }
}
