//! The provider limb, `web`. To agents it offers the provider's own web
//! search and web fetch, which run inside a response: the model's use of
//! them and their results are blocks of its response, with citations, so
//! nothing is dispatched and nothing settles. To Max it offers `usage`, a
//! reading of the plan's usage windows, which the brain runs as a call.

use std::collections::BTreeMap;

use harness_core::call::{Outcome, Output};
use harness_core::environment::{Audience, Description, Limb, LimbKind, Tool};
use harness_core::id::{LimbId, Timestamp};
use harness_core::model_call::Window;
use harness_core::value::Bytes;
use serde_json::json;

use crate::render::{blocks, text};

/// Max's tool: read the plan's usage now. Its input is `{}`.
pub const USAGE: &str = "usage";

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
    let usage = Tool {
        name: USAGE.into(),
        definition: json!({
            "name": USAGE,
            "description": "Read the plan's usage windows now: how much of each is used, and when it resets.",
            "input_schema": { "type": "object", "properties": {} },
        })
        .to_string(),
        audience: Audience::Max,
    };
    Description {
        instructions: Vec::new(),
        tools: vec![
            tool("web_search", "web_search_20250305"),
            tool("web_fetch", "web_fetch_20250910"),
            usage,
        ],
    }
}

/// A completed usage call from the provider's `reading` taken `at`, and
/// its outputs' bytes: as text, each window's use and how long until it
/// resets; in full, the reading as JSON, each window by its name.
pub fn usage(reading: &BTreeMap<String, Window>, at: Timestamp) -> (Vec<Bytes>, Outcome) {
    let said: Vec<String> = reading
        .iter()
        .map(|(name, w)| {
            let used = format!("{name} {:.0}% used", w.used * 100.0);
            match w.resets {
                Some(t) => format!("{used}, resets in {}", span(t.0 - at.0)),
                None => used,
            }
        })
        .collect();
    let said = match said.is_empty() {
        true => "The provider reported no usage windows.".to_owned(),
        false => said.join("; "),
    };
    let model = blocks(&[text(&said)]);
    let full = Bytes::from(serde_json::to_string(reading).expect("windows serialise"));
    let output = Output {
        model: model.id(),
        full: Some(full.id()),
        observes: Vec::new(),
    };
    (vec![model, full], Outcome::Completed(output))
}

/// A length of time to the minute: `3h05m`, `2d4h`, `12m`.
fn span(ms: i64) -> String {
    let m = ms.max(0) / 60_000;
    let (d, h, m) = (m / 1440, m / 60 % 24, m % 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h{m:02}m"),
        _ => format!("{d}d{h}h"),
    }
}
