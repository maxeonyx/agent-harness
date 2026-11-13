//! Finding a chat by what was said in it: `/find?q=<words>`, read off the
//! fold like a context, through the [`Index`] the brain's `history` set
//! searches too.

use harness_core::context::EntryId;
use harness_core::id::{TaskId, ValueId};
use harness_program::history::Index;
use serde_json::{Value, json};

/// The chats of `tasks` (each with its own entries) whose words hold
/// `query`, newest first: each with how many of its entries match and a
/// snippet of the newest.
pub fn find(
    index: &Index,
    tasks: Vec<(TaskId, Vec<(EntryId, ValueId)>)>,
    query: &str,
    read: &dyn Fn(ValueId) -> Option<Vec<u8>>,
) -> Value {
    let hits = index.chats(tasks, query, read);
    Value::Array(
        hits.into_iter()
            .map(|hit| {
                json!({
                    "task": hit.task.to_string(),
                    "entry": hit.entry.to_string(),
                    "snippet": {
                        "before": hit.snippet.before,
                        "match": hit.snippet.matched,
                        "after": hit.snippet.after,
                    },
                    "hits": hit.hits,
                })
            })
            .collect(),
    )
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
