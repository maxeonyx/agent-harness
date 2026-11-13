//! The `history` tool set, which the brain offers: reading the harness's
//! chats, every agent's and Max's words in them. `find` lists where some
//! words were said, newest first, with when; `context` shows one chat as
//! text, from its end. The face's `/find` searches the same [`Index`].
//!
//! The chats searched are the harness's own. A later source of what was
//! said (Max's sessions in other harnesses, say) adds its own [`Found`]
//! hits to [`find`]'s list, which merges them by when they were said.
//!
//! What is searched is each message's text blocks, Max's and the agents';
//! tool inputs and results are not what was said. Each task is searched in
//! the entries it added itself: a fork's inherited prefix is its parent's,
//! so a match there names the chat it was said in, once. Max's words are
//! also searched as he said them, so a hit in them names him, his label
//! and the chat he said them to.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use harness_core::context::{Entry, EntryId};
use harness_core::id::{Digest, LimbId, TaskId, Timestamp, ValueId};
use harness_core::limb::{Audience, Instruction, Tool, ToolSet};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::state::State;
use harness_core::value::Bytes;
use serde_json::{Value, json};

use crate::render::{blocks, label, text};

/// The tool set's name, as roles hold it.
pub const SET: &str = "history";
pub const FIND: &str = "find";
pub const CONTEXT: &str = "context";

/// The most hits one search lists.
const LIMIT: usize = 30;
/// How much of the text around a match a snippet shows, in characters.
const BEFORE: usize = 60;
const AFTER: usize = 100;
/// About how much of a chat one `context` shows, in characters.
const PAGE: usize = 40_000;
/// How much of a tool's input or result `context` shows.
const TOOL: usize = 400;

/// What the set says of itself in an agent's `system`.
const INSTRUCTIONS: &str = "`find` and `context` read the harness's chats: every agent's work, and Max's words in them. \
`find` lists where some words were said, newest first, with when: each chat with the entry they were last said at, and \
Max's own words under their `max-…` labels; `max_only` narrows it to his words. `context` shows one chat as text, its \
latest entries first up to a page: `at` a task for the chat as it is now, an entry for the chat up to it, or one of \
Max's labels for those words of his in full.";

/// A value's bytes, if the store has them.
pub type Read<'a> = &'a dyn Fn(ValueId) -> Option<Vec<u8>>;
/// A time as an agent is told it, in Max's zone.
pub type Local<'a> = &'a dyn Fn(Timestamp) -> String;

/// The set as the brain `limb` offers it.
pub fn set(limb: LimbId) -> ToolSet {
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| Tool {
        name: name.into(),
        definition: json!({
            "name": name,
            "description": description,
            "input_schema": { "type": "object", "properties": properties, "required": required },
        })
        .to_string(),
        audience: Audience::Agent,
    };
    ToolSet {
        name: SET.into(),
        instructions: vec![Instruction {
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: SET.into(),
                },
                version: Version::Is(Digest::of(INSTRUCTIONS.as_bytes())),
            },
            text: INSTRUCTIONS.into(),
        }],
        tools: vec![
            tool(
                FIND,
                "List where some words were said in the harness's chats, newest first: each chat, the entry \
they were last said at and when, and Max's own words under their labels.",
                json!({
                    "words": { "type": "string", "description": "What was said, matched case-insensitively." },
                    "max_only": { "type": "boolean", "description": "Only Max's own words." },
                }),
                &["words"],
            ),
            tool(
                CONTEXT,
                "Show one chat as text, its latest entries first up to a page, with where to read on.",
                json!({
                    "at": {
                        "type": "string",
                        "description": "A task (the chat as it is now), an entry (the chat up to it), or one of \
                Max's `max-…` labels (those words of his in full).",
                    },
                }),
                &["at"],
            ),
        ],
    }
}

/// Each value's text as said, and lowercased char for char, so a match's
/// position in one is its position in the other. A value never changes,
/// so its text is read from the store once and kept.
#[derive(Default)]
pub struct Index(Mutex<HashMap<ValueId, Arc<Words>>>);

struct Words {
    text: Vec<char>,
    lower: String,
}

/// A chat whose own words hold a search.
pub struct Hit {
    pub task: TaskId,
    /// The newest entry that holds them.
    pub entry: EntryId,
    /// How many of its entries do.
    pub hits: usize,
    pub snippet: Snippet,
}

/// The text around a match, on one line.
pub struct Snippet {
    pub before: String,
    pub matched: String,
    pub after: String,
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

impl Index {
    /// The chats whose own words hold `query`, newest first.
    pub fn chats(
        &self,
        tasks: Vec<(TaskId, Vec<(EntryId, ValueId)>)>,
        query: &str,
        read: Read,
    ) -> Vec<Hit> {
        let query = lower(query.trim());
        if query.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for (task, entries) in tasks.into_iter().rev() {
            let mut hits = entries.iter().filter_map(|&(id, content)| {
                let (words, at) = self.matches(content, &query, read)?;
                Some((id, words, at))
            });
            let Some((entry, words, at)) = hits.next_back() else {
                continue;
            };
            found.push(Hit {
                task,
                entry,
                snippet: snippet(&words.text, at, query.chars().count()),
                hits: hits.count() + 1,
            });
            if found.len() == LIMIT {
                break;
            }
        }
        found
    }

    /// The text of `value` and where `query` (lowercased) first is in it,
    /// in chars, if it is.
    fn matches(&self, value: ValueId, query: &str, read: Read) -> Option<(Arc<Words>, usize)> {
        let words = self.words(value, read)?;
        let at = words.lower.find(query)?;
        let at = words.lower[..at].chars().count();
        Some((words, at))
    }

    fn words(&self, value: ValueId, read: Read) -> Option<Arc<Words>> {
        if let Some(w) = self.0.lock().unwrap_or_else(|e| e.into_inner()).get(&value) {
            return Some(w.clone());
        }
        let text = said(&read(value)?);
        let words = Arc::new(Words {
            lower: lower(&text),
            text: text.chars().collect(),
        });
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(value, words.clone());
        Some(words)
    }
}

/// One place some words were said, from any source: what `find` lists.
struct Found {
    at: Option<Timestamp>,
    /// Who said them where, as one line.
    said: String,
    snippet: Snippet,
}

/// An agent's `find`: Max's words and the chats that hold `words`, newest
/// first.
pub fn find(
    state: &State,
    index: &Index,
    read: Read,
    local: Local,
    input: &[u8],
) -> Result<Bytes, String> {
    let input: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let words = input["words"].as_str().unwrap_or("").trim();
    if words.is_empty() {
        return Err("`words` must say what to find".into());
    }
    let query = lower(words);
    let mut found: Vec<Found> = state
        .sayings()
        .filter_map(|(value, mark, to)| {
            let (text, at) = index.matches(value, &query, read)?;
            Some(Found {
                at: Some(mark.at),
                said: format!("Max (`{}`) to {}", label(value), chat(state, to)),
                snippet: snippet(&text.text, at, query.chars().count()),
            })
        })
        .collect();
    if input["max_only"] != json!(true) {
        let hits = index.chats(own_entries(state), words, read);
        found.extend(hits.into_iter().map(|hit| {
            let places = match hit.hits {
                1 => String::new(),
                n => format!(", {n} places"),
            };
            Found {
                at: dated(state, hit.task, hit.entry),
                said: format!(
                    "{}{places}, the newest at entry {}",
                    chat(state, hit.task),
                    hit.entry
                ),
                snippet: hit.snippet,
            }
        }));
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.at));
    found.truncate(LIMIT);
    if found.is_empty() {
        return Ok(blocks(&[text(&format!("Nothing said holds \"{words}\"."))]));
    }
    let mut said = format!("Where \"{words}\" was said, newest first:");
    for f in found {
        let when = f.at.map_or_else(|| "undated".to_owned(), local);
        let Snippet {
            before,
            matched,
            after,
        } = f.snippet;
        said.push_str(&format!(
            "\n\n{when} · {}\n  {before}«{matched}»{after}",
            f.said
        ));
    }
    Ok(blocks(&[text(&said)]))
}

/// An agent's `context`: a chat as text from its end, up to a page, or
/// Max's words under a label in full.
pub fn context(state: &State, read: Read, local: Local, input: &[u8]) -> Result<Bytes, String> {
    let input: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let at = input["at"].as_str().unwrap_or("").trim();
    if at.starts_with("max-") {
        let (value, mark, to) = (state.sayings())
            .find(|(v, _, _)| label(*v) == at)
            .ok_or(format!("none of Max's words are labelled `{at}`"))?;
        let words = read(value).map(|b| said(&b)).unwrap_or_default();
        let head = format!(
            "Max wrote this to {} on {} (`{at}`):",
            chat(state, to),
            local(mark.at)
        );
        return Ok(blocks(&[text(&head), text(&words)]));
    }
    let (task, head) = match (at.parse::<TaskId>(), at.parse::<EntryId>()) {
        (Ok(task), _) => {
            let t = state.task(task).ok_or(format!("no task is `{at}`"))?;
            (
                task,
                t.head()
                    .ok_or(format!("the task `{at}` has no context yet"))?,
            )
        }
        (_, Ok(entry)) => {
            let task = (state.tasks())
                .filter_map(|t| Some((t.id, t.head()?)))
                .find(|(_, head)| state.context(*head).iter().any(|(id, _)| *id == entry))
                .map(|(task, _)| task)
                .ok_or(format!("no chat holds the entry `{at}`"))?;
            (task, entry)
        }
        _ => {
            return Err(format!(
                "`at` must be a task, an entry or one of Max's labels, not `{at}`"
            ));
        }
    };
    let chain = state.context(head);
    let dates = dates(state, &chain);
    let mut shown = Vec::new();
    let mut size = 0;
    let mut earlier = None;
    for (i, (id, entry)) in chain.iter().enumerate().rev() {
        let Entry::Message { message, .. } = entry else {
            continue;
        };
        let body = rendered(&read(message.content).unwrap_or_default());
        if size > 0 && size + body.len() > PAGE {
            earlier = Some(*id);
            break;
        }
        size += body.len();
        let when = dates[i].map_or_else(|| "undated".to_owned(), local);
        shown.push(format!("— {} · {when}\n{body}", message.role.as_str()));
    }
    shown.reverse();
    let mut said = format!("{}, up to entry {head}:", chat(state, task));
    if let Some(e) = earlier {
        said.push_str(&format!(
            " its latest {} entries; earlier ones: `context` at `{e}`.",
            shown.len()
        ));
    }
    Ok(blocks(&[text(&said), text(&shown.join("\n\n"))]))
}

/// A chat as an agent names it: `name` (role, task id).
fn chat(state: &State, task: TaskId) -> String {
    let Some(t) = state.task(task) else {
        return format!("task {task}");
    };
    let role = state.role(t.role).map_or("", |r| r.name.as_str());
    format!("`{}` ({role}, task {task})", t.name)
}

/// When `entry` of `task`'s chat was said.
fn dated(state: &State, task: TaskId, entry: EntryId) -> Option<Timestamp> {
    let chain = state.context(state.task(task)?.head()?);
    let i = chain.iter().position(|(id, _)| *id == entry)?;
    dates(state, &chain)[i]
}

/// When each entry of a chain was said: a response when it came, anything
/// else when the request that sent it was made, which is that of the
/// first entry after it that has a date.
fn dates(state: &State, chain: &[(EntryId, &Entry)]) -> Vec<Option<Timestamp>> {
    let mut next = None;
    let mut out: Vec<_> = (chain.iter().rev())
        .map(|(id, _)| {
            let own = match state.response(*id) {
                Some(r) => Some(r.ended.as_ref().map_or(r.sent.at, |e| e.at.at)),
                None => state.request(*id).map(|r| r.sent.at),
            };
            next = own.or(next);
            next
        })
        .collect();
    out.reverse();
    out
}

/// A message's content as text: its text blocks whole, each tool use and
/// result cut short, thinking left out.
fn rendered(content: &[u8]) -> String {
    let blocks: Vec<Value> = serde_json::from_slice(content).unwrap_or_default();
    let shown: Vec<String> = blocks
        .iter()
        .filter_map(|b| {
            let kind = b["type"].as_str().unwrap_or("");
            Some(match kind {
                "text" => b["text"].as_str()?.to_owned(),
                "thinking" | "redacted_thinking" => return None,
                "tool_use" | "server_tool_use" => {
                    let name = b["name"].as_str().unwrap_or("");
                    format!("→ {name} {}", cut(&b["input"].to_string(), TOOL))
                }
                "tool_result" => {
                    let said = match &b["content"] {
                        Value::String(s) => s.clone(),
                        Value::Array(parts) => (parts.iter())
                            .filter_map(|p| p["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => String::new(),
                    };
                    format!("← {}", cut(&said, TOOL))
                }
                kind => format!("[{kind}]"),
            })
        })
        .collect();
    shown.join("\n")
}

/// `s` cut to about `n` chars.
fn cut(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_owned(),
    }
}

/// A value's text: a message's text blocks joined, or, for words that are
/// not blocks (a mandate Max gave), the words themselves.
fn said(bytes: &[u8]) -> String {
    match serde_json::from_slice::<Vec<Value>>(bytes) {
        Ok(blocks) => (blocks.iter())
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
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

/// The text around a match at char `at`, `len` chars long, on one line.
fn snippet(text: &[char], at: usize, len: usize) -> Snippet {
    let start = at.saturating_sub(BEFORE);
    let end = (at + len + AFTER).min(text.len());
    let flat = |r: &[char]| -> String {
        r.iter()
            .map(|&c| if c.is_whitespace() { ' ' } else { c })
            .collect()
    };
    Snippet {
        before: format!(
            "{}{}",
            if start > 0 { "…" } else { "" },
            flat(&text[start..at])
        ),
        matched: flat(&text[at..at + len]),
        after: format!(
            "{}{}",
            flat(&text[at + len..end]),
            if end < text.len() { "…" } else { "" }
        ),
    }
}
