//! A task's page as a tab draws it: the newest version Max was shown, its
//! bodies parsed by the one parser the brain checked them with
//! ([`harness_body`]), and the live half drawn as it stands now: the plan's
//! steps and the asks inside them.

use harness_core::ask::Ask;
use harness_core::context::Entry;
use harness_core::id::ValueId;
use harness_core::page::{Ref, Said, Step};
use harness_core::state::State;
use harness_core::task::{Phase, Task};
use harness_program::page::KINDS;
use serde_json::{Value, json};

use crate::view::Text;

/// `t`'s page, or `None` when it has never presented one.
pub fn row(state: &State, value: Text, t: &Task) -> Option<Value> {
    let page = state.page(t.id)?;
    let newest = page.versions.last()?;
    let body = |v: Option<ValueId>| {
        let text = v.and_then(value)?;
        Some(harness_body::body(&String::from_utf8_lossy(&text), KINDS).0)
    };
    let inline = |text: &str| harness_body::inline(text, KINDS).0;
    // Its asks, oldest first, open or settled: its own and its forks'.
    // One that settled is drawn quietly where it was, with how it settled.
    let asks: Vec<(String, Value)> = (page.asks.iter())
        .filter_map(|id| Some((*id, state.call(*id)?)))
        .filter_map(|(id, r)| {
            let ask: Ask = serde_json::from_slice(&value(r.call.input)?).ok()?;
            let settled = r.outcome().map(|o| match o.output() {
                Some(out) => value(out.model).map_or_else(String::new, |b| texts(&b)),
                None => "No longer asked: its agent stopped.".to_owned(),
            });
            let row = json!({
                "call": id.to_string(),
                "label": id.label(),
                "key": ask.key,
                "question": ask.question,
                "end": ask.end,
                "now": inline(&ask.now),
                "why": inline(&ask.why),
                "shows": body(ask.shows),
                "want": ask.want,
                "otherwise": ask.otherwise.map(|o| o.then),
                "settled": settled,
            });
            Some((ask.step.unwrap_or_default(), row))
        })
        .collect();
    let plan = t.description.plan.clone().unwrap_or_default();
    let first_open = plan
        .iter()
        .find(|s| !matches!(s.said, Said::Done))
        .map(|s| s.key.clone());
    let placed = |key: &str| {
        if plan.iter().any(|s| s.key == key) {
            Some(key.to_string())
        } else {
            first_open.clone()
        }
    };
    let mut loose = Vec::new();
    let mut by_step: Vec<(String, Value)> = Vec::new();
    for (step, ask) in asks {
        match placed(&step) {
            Some(key) => by_step.push((key, ask)),
            None => loose.push(ask),
        }
    }
    let steps: Vec<Value> = plan
        .iter()
        .map(|s| {
            let asks: Vec<&Value> = (by_step.iter())
                .filter(|(k, _)| *k == s.key)
                .map(|(_, a)| a)
                .collect();
            let yours = asks.iter().any(|a| a["settled"].is_null());
            json!({
                "key": s.key,
                "title": title(state, s),
                "state": state_of(s, yours),
                "failed": match &s.said { Said::Failed { why } => Some(why), _ => None },
                "body": body(s.body),
                "asks": asks,
            })
        })
        .collect();
    let current = matches!(t.phase(), Some(Phase::AwaitingMax)) && newest.held >= t.since;
    // The quick answer: what the response that last presented said.
    let quick = (page.presented)
        .and_then(|call| match state.entry(state.origin(call)?)? {
            Entry::Message { message, .. } => value(message.content),
            Entry::Root(_) => None,
        })
        .map(|content| texts(&content))
        .filter(|text| !text.trim().is_empty());
    let agreed = page.agreed.as_ref();
    Some(json!({
        "version": newest.n,
        "at": newest.at.0,
        "held": newest.held.0,
        "current": current,
        "lead": newest.sheet.lead,
        "quick": quick,
        "end": {
            "says": newest.sheet.end.says,
            "shows": body(newest.sheet.end.shows),
            // Agreed, it folds to its claim; changed since, it opens, marked.
            "agreed": agreed == Some(&newest.sheet.end),
            "changed": agreed.is_some_and(|a| *a != newest.sheet.end),
        },
        "now": { "says": newest.sheet.now.says, "shows": body(newest.sheet.now.shows) },
        "plan": steps,
        "asks": loose,
        "why": body(newest.sheet.why),
    }))
}

/// A step's title: its own, else the name of the task it is bound to.
fn title(state: &State, step: &Step) -> Option<String> {
    step.title.clone().or_else(|| match &step.of {
        Some(Ref::Task(id)) => state
            .task(*id)
            .map(|t| (t.description.title.clone()).unwrap_or_else(|| t.name.clone())),
        Some(Ref::Item(item)) => Some(item.name.clone()),
        None => None,
    })
}

/// A step's mark: what the agent said, unless an open ask in it makes it
/// Max's move.
fn state_of(step: &Step, yours: bool) -> &'static str {
    match (&step.said, yours) {
        (Said::Done, _) => "done",
        (Said::Failed { .. }, _) => "failed",
        (_, true) => "yours",
        (Said::Doing, false) => "working",
        (Said::Planned, false) => "planned",
    }
}

/// The text of a list of content blocks, its text blocks joined.
fn texts(blocks: &[u8]) -> String {
    let blocks: Vec<Value> = serde_json::from_slice(blocks).unwrap_or_default();
    (blocks.iter())
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}
