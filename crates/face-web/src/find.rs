//! Finding a chat by what was said in it: `/find?q=<words>`, read off the
//! fold like a context. Each task is searched in the entries it added
//! itself: a fork's inherited prefix is its parent's, so a match there
//! names the chat it was said in, once.
//!
//! What is searched is each message's text blocks, Max's and the agents';
//! tool inputs and results are not what was said. An entry never changes,
//! so its text is read from the store once and kept.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use harness_core::context::{Entry, EntryId};
use harness_core::id::{TaskId, ValueId};
use harness_core::state::State;
use serde_json::{Value, json};

/// The most chats one search names.
const LIMIT: usize = 30;
/// How much of the text around a match the snippet shows, in characters.
const BEFORE: usize = 60;
const AFTER: usize = 100;

/// Each entry's text as said, and lowercased char for char, so a match's
/// position in one is its position in the other.
#[derive(Default)]
pub struct Said(Mutex<HashMap<EntryId, Arc<Words>>>);

struct Words {
    text: Vec<char>,
    lower: String,
}

/// Each task's own message entries, oldest task first.
pub fn own_entries(state: &State) -> Vec<(TaskId, Vec<(EntryId, ValueId)>)> {
    let mut tasks: Vec<_> = state.tasks().collect();
    tasks.sort_by_key(|t| t.created.seq.n);
    let mut claimed = BTreeSet::new();
    tasks
        .into_iter()
        .filter_map(|t| {
            let own: Vec<_> = state
                .context(t.head()?)
                .into_iter()
                .filter(|(id, _)| claimed.insert(*id))
                .filter_map(|(id, entry)| match entry {
                    Entry::Message { message, .. } => Some((id, message.content)),
                    Entry::Root(_) => None,
                })
                .collect();
            Some((t.id, own))
        })
        .collect()
}

impl Said {
    /// The chats whose own words hold `query`, newest first: each with how
    /// many of its entries match and a snippet of the newest.
    pub fn find(
        &self,
        tasks: Vec<(TaskId, Vec<(EntryId, ValueId)>)>,
        query: &str,
        value: impl Fn(ValueId) -> Option<Vec<u8>>,
    ) -> Value {
        let query = lower(query.trim());
        if query.is_empty() {
            return json!([]);
        }
        let mut found = Vec::new();
        for (task, entries) in tasks.into_iter().rev() {
            let mut hits = entries.iter().filter_map(|&(id, content)| {
                let words = self.words(id, content, &value)?;
                let at = words.lower.find(&query)?;
                let at = words.lower[..at].chars().count();
                Some((id, words, at))
            });
            let Some(last) = hits.next_back() else {
                continue;
            };
            let (entry, words, at) = last;
            found.push(json!({
                "task": task.to_string(),
                "entry": entry.to_string(),
                "snippet": snippet(&words.text, at, query.chars().count()),
                "hits": hits.count() + 1,
            }));
            if found.len() == LIMIT {
                break;
            }
        }
        Value::Array(found)
    }

    fn words(
        &self,
        id: EntryId,
        content: ValueId,
        value: &impl Fn(ValueId) -> Option<Vec<u8>>,
    ) -> Option<Arc<Words>> {
        if let Some(w) = self.0.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            return Some(w.clone());
        }
        let text = said(&value(content)?);
        let words = Arc::new(Words {
            lower: lower(&text),
            text: text.chars().collect(),
        });
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(id, words.clone());
        Some(words)
    }
}

/// A message's text blocks, joined.
fn said(content: &[u8]) -> String {
    serde_json::from_slice::<Vec<Value>>(content)
        .unwrap_or_default()
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Lowercased one char for one char, so char positions carry over.
fn lower(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(one), None) => one,
                _ => c,
            }
        })
        .collect()
}

/// The text around a match at char `at`, `len` chars long, on one line,
/// with where the match sits in it.
fn snippet(text: &[char], at: usize, len: usize) -> Value {
    let start = at.saturating_sub(BEFORE);
    let end = (at + len + AFTER).min(text.len());
    let flat = |r: &[char]| -> String {
        r.iter()
            .map(|&c| if c.is_whitespace() { ' ' } else { c })
            .collect()
    };
    json!({
        "before": format!("{}{}", if start > 0 { "…" } else { "" }, flat(&text[start..at])),
        "match": flat(&text[at..at + len]),
        "after": format!("{}{}", flat(&text[at + len..end]), if end < text.len() { "…" } else { "" }),
    })
}

/// A query parameter as typed: `+` and `%XX` decoded.
pub fn decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        match b {
            b'+' => out.push(b' '),
            b'%' => {
                let hex: Vec<u8> = bytes.by_ref().take(2).collect();
                match std::str::from_utf8(&hex)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    Some(v) => out.push(v),
                    None => out.extend(b"%".iter().chain(&hex)),
                }
            }
            b => out.push(b),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
