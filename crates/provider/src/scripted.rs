//! The scripted model: a model this client serves itself, with no call to
//! the provider and no cost, for exercising the harness. It is chosen like
//! any model, by its id [`SCRIPTED`], and answers in the provider's own
//! wire format, so everything after the socket runs as it does for a real
//! response: the status, the event stream, the assembly, the ending.
//!
//! Its reply is a function of the request. The script is the text of the
//! latest user message that has any: each line starting with a verb is one
//! response, taken in turn, one for each assistant message since. Other
//! lines are ignored, so a mandate or a brief can carry one.
//!
//! ```text
//! say TEXT              a reply (`\n` is a line break); the turn ends
//! tool NAME [JSON]      a tool use, with that input (`{}` if none); more
//!                       `tool NAME JSON` after it are in the same response
//! long N                a reply of N lines
//! vary TEXT             TEXT and which time this request was asked, so a
//!                       retry gives other words; the turn ends
//! error STATUS [TEXT]   the request is refused with that HTTP status
//! limit SECONDS         the plan's limit, reset in SECONDS
//! fail [TEXT]           the stream starts, then breaks with an error event
//! drop                  the stream starts, then ends early
//! ```
//!
//! A verb may follow `slow MS` (MS before each streamed piece) and
//! `flaky STATUS` (the first time this client sees the request, it is
//! refused with STATUS; a retry gets the step): `flaky 529 slow 200 say hi`.
//! Text without a verb is echoed. Once the script is used up, the reply
//! quotes the tool results it was sent, and the turn ends.

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Cursor, Read};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_runtime::{Cancel, Cancelled, Waiter};
use serde_json::{Value, json};

use crate::Model;
use crate::http::{Body, Response};

/// The scripted model's id: what a role file, a tuning or `--model` names.
pub const SCRIPTED: &str = "scripted";

/// The scripted model, as a face lists it.
pub fn scripted() -> Model {
    Model {
        id: SCRIPTED.into(),
        name: "Scripted (no cost)".into(),
    }
}

/// The verbs a script line may start with.
const VERBS: [&str; 8] = [
    "say", "tool", "long", "vary", "error", "limit", "fail", "drop",
];

/// How much of each tool result a used-up script's reply quotes.
const QUOTED: usize = 4000;

/// The response to the request `body`. `seen` counts the times this
/// client has been asked each request, by its messages without their cache
/// breakpoints, which are placed anew on each send.
pub(crate) fn respond(body: &[u8], cancel: &Cancel, seen: &Mutex<HashMap<u64, u32>>) -> Response {
    let request: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return refused(400, &format!("the request is not JSON: {e}")),
    };
    let messages = request["messages"].as_array().cloned().unwrap_or_default();
    let asked = {
        let mut hasher = DefaultHasher::new();
        let mut bare = Value::Array(messages.clone());
        strip_breakpoints(&mut bare);
        bare.to_string().hash(&mut hasher);
        let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
        let n = seen.entry(hasher.finish()).or_default();
        *n += 1;
        *n
    };
    let step = step(&messages);
    if let Some(status) = step.flaky
        && asked == 1
    {
        return refused(status, "flaky: refused the first time");
    }
    let (verb, rest) = step.act.split_once(' ').unwrap_or((&step.act, ""));
    let rest = rest.trim();
    let mut events = Events::new(body.len());
    match verb {
        "say" => events.text(&rest.replace("\\n", "\n"), " ", "end_turn"),
        "vary" => events.text(&format!("{rest} (try {asked})"), " ", "end_turn"),
        "tool" => match uses(rest) {
            Ok(uses) => events.tools(&uses, messages.len()),
            Err(name) => {
                return refused(
                    400,
                    &format!("scripted: the input of `tool {name}` is not JSON"),
                );
            }
        },
        "long" => {
            let n: usize = rest.parse().unwrap_or(100);
            let lines: Vec<String> = (1..=n)
                .map(|i| format!("Line {i} of {n} of a long scripted reply.\n"))
                .collect();
            events.text(&lines.concat(), "\n", "end_turn");
        }
        "error" => {
            let (status, text) = rest.split_once(' ').unwrap_or((rest, "scripted error"));
            return refused(status.parse().unwrap_or(500), text);
        }
        "limit" => {
            let reset = now_s() + rest.parse::<i64>().unwrap_or(60);
            let mut response = refused(429, "scripted: the plan's limit");
            response.headers.extend([
                (
                    "anthropic-ratelimit-unified-status".into(),
                    "rejected".into(),
                ),
                (
                    "anthropic-ratelimit-unified-reset".into(),
                    reset.to_string(),
                ),
            ]);
            return response;
        }
        "fail" => {
            events.start_text("Starting a scripted reply that will fail. ");
            let text = if rest.is_empty() {
                "scripted failure"
            } else {
                rest
            };
            events.push(
                "error",
                json!({"type": "error", "error": {"type": "api_error", "message": text}}),
            );
        }
        "drop" => events.start_text("Starting a scripted reply that will stop early. "),
        _ => events.text(&step.act, " ", "end_turn"),
    }
    Response {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        body: Body::Local(Box::new(Stream {
            events: events.0,
            current: Cursor::new(Vec::new()),
            slow: step.slow,
            cancel: cancel.clone(),
        })),
    }
}

/// One response's worth of the script.
struct Step {
    slow: Duration,
    flaky: Option<u16>,
    /// The verb and what follows it; for a reply with no verb, its text.
    act: String,
}

/// The tool uses of `tool NAME [JSON] [tool NAME JSON]...`, or the name of
/// one whose input is not JSON.
fn uses(mut rest: &str) -> Result<Vec<(String, String)>, String> {
    let mut uses = Vec::new();
    loop {
        let (name, after) = rest.split_once(' ').unwrap_or((rest, ""));
        let after = after.trim_start();
        let (input, tail) = match after {
            "" => ("{}", ""),
            _ => {
                let mut values = serde_json::Deserializer::from_str(after).into_iter::<Value>();
                match values.next() {
                    Some(Ok(_)) => after.split_at(values.byte_offset()),
                    _ => return Err(name.into()),
                }
            }
        };
        uses.push((name.to_string(), input.to_string()));
        match tail.trim() {
            "" => return Ok(uses),
            tail => rest = tail.strip_prefix("tool ").ok_or(name)?.trim_start(),
        }
    }
}

/// The step `messages` are at.
fn step(messages: &[Value]) -> Step {
    let plain = |act: String| Step {
        slow: Duration::ZERO,
        flaky: None,
        act,
    };
    let script = messages.iter().enumerate().rev().find_map(|(i, m)| {
        (m["role"] == "user")
            .then(|| text(m))
            .flatten()
            .map(|t| (i, t))
    });
    let Some((at, script)) = script else {
        return plain("say (scripted) There is no user text to answer.".into());
    };
    let done = messages[at + 1..]
        .iter()
        .filter(|m| m["role"] == "assistant")
        .count();
    let steps: Vec<Step> = script.lines().filter_map(parse).collect();
    if steps.is_empty() {
        return plain(format!(
            "say (scripted) You said: {script}\\n\\nStart a line with one of {} to script a reply.",
            VERBS.join(", ")
        ));
    }
    steps.into_iter().nth(done).unwrap_or_else(|| {
        let results = messages.last().map(tool_results).unwrap_or_default();
        plain(match results.is_empty() {
            true => "say (scripted) The script is used up.".into(),
            false => {
                format!("say (scripted) The script is used up. The tool results:\\n\\n{results}")
            }
        })
    })
}

/// Remove every `cache_control` from a request's JSON.
fn strip_breakpoints(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.remove("cache_control");
            o.values_mut().for_each(strip_breakpoints);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_breakpoints),
        _ => {}
    }
}

/// A script line as a step, if it starts with a verb.
fn parse(line: &str) -> Option<Step> {
    let mut step = Step {
        slow: Duration::ZERO,
        flaky: None,
        act: String::new(),
    };
    let mut rest = line.trim();
    loop {
        let (word, after) = rest.split_once(' ').unwrap_or((rest, ""));
        match word {
            "slow" | "flaky" => {
                let (n, after) = after.trim().split_once(' ').unwrap_or((after.trim(), ""));
                match word {
                    "slow" => step.slow = Duration::from_millis(n.parse().ok()?),
                    _ => step.flaky = Some(n.parse().ok()?),
                }
                rest = after.trim();
            }
            verb if VERBS.contains(&verb) => {
                step.act = rest.to_owned();
                return Some(step);
            }
            _ => return None,
        }
    }
}

/// A message's text blocks, joined; `None` if it has none.
fn text(message: &Value) -> Option<String> {
    let content = &message["content"];
    if let Some(s) = content.as_str() {
        return Some(s.to_owned());
    }
    let texts: Vec<&str> = content
        .as_array()?
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    (!texts.is_empty()).then(|| texts.join("\n"))
}

/// The tool results in a message, each quoted up to [`QUOTED`] characters,
/// with `\n` escaped for a `say` line.
fn tool_results(message: &Value) -> String {
    let blocks = message["content"].as_array().cloned().unwrap_or_default();
    let quoted: Vec<String> = blocks
        .iter()
        .filter(|b| b["type"] == "tool_result")
        .map(|b| {
            let content = match &b["content"] {
                Value::String(s) => s.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            let cut: String = content.chars().take(QUOTED).collect();
            let error = if b["is_error"] == true {
                " (an error)"
            } else {
                ""
            };
            format!("{}{error}: {cut}", b["tool_use_id"].as_str().unwrap_or("?"))
        })
        .collect();
    quoted.join("\n\n").replace('\n', "\\n")
}

/// A refusal with the provider's error body.
fn refused(status: u16, message: &str) -> Response {
    let body = json!({"type": "error", "error": {"type": "scripted", "message": message}});
    Response {
        status,
        headers: vec![("content-type".into(), "application/json".into())],
        body: Body::Local(Box::new(Cursor::new(body.to_string().into_bytes()))),
    }
}

/// A response's server-sent events, each whole.
struct Events(VecDeque<Vec<u8>>);

impl Events {
    fn new(request: usize) -> Events {
        let mut events = Events(VecDeque::new());
        let usage = json!({
            "input_tokens": request / 4,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
        });
        events.push(
            "message_start",
            json!({"type": "message_start", "message": {
                "id": "msg_scripted", "type": "message", "role": "assistant",
                "model": SCRIPTED, "content": [], "stop_reason": null, "usage": usage,
            }}),
        );
        events
    }

    fn push(&mut self, name: &str, data: Value) {
        self.0
            .push_back(format!("event: {name}\ndata: {data}\n\n").into_bytes());
    }

    /// A text block streamed in pieces ending at each `split`, and no end.
    fn start_text(&mut self, text: &str) {
        self.text_block(text, " ");
    }

    fn text_block(&mut self, text: &str, split: &str) {
        self.push(
            "content_block_start",
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        );
        for piece in text.split_inclusive(split) {
            self.push(
                "content_block_delta",
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": piece}}),
            );
        }
        self.push(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": 0}),
        );
    }

    /// A whole reply of one text block.
    fn text(&mut self, text: &str, split: &str, stop: &str) {
        self.text_block(text, split);
        self.end(stop, text.len() / 4);
    }

    /// A whole reply of tool uses, each id unique in its conversation of
    /// `n` messages.
    fn tools(&mut self, uses: &[(String, String)], n: usize) {
        for (index, (name, input)) in uses.iter().enumerate() {
            self.push(
                "content_block_start",
                json!({"type": "content_block_start", "index": index, "content_block": {
                    "type": "tool_use", "id": format!("toolu_scripted_{n}_{index}"), "name": name, "input": {},
                }}),
            );
            self.push(
                "content_block_delta",
                json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": input}}),
            );
            self.push(
                "content_block_stop",
                json!({"type": "content_block_stop", "index": index}),
            );
        }
        let output = uses.iter().map(|(_, input)| input.len()).sum::<usize>() / 4;
        self.end("tool_use", output);
    }

    fn end(&mut self, stop: &str, output: usize) {
        self.push(
            "message_delta",
            json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": null}, "usage": {"output_tokens": output}}),
        );
        self.push("message_stop", json!({"type": "message_stop"}));
    }
}

/// The events as a body, each after `slow`, cut when `cancel` is due.
struct Stream {
    events: VecDeque<Vec<u8>>,
    current: Cursor<Vec<u8>>,
    slow: Duration,
    cancel: Cancel,
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.current.read(buf)?;
            if n > 0 || buf.is_empty() {
                return Ok(n);
            }
            let Some(next) = self.events.pop_front() else {
                return Ok(0);
            };
            self.pause()?;
            self.current = Cursor::new(next);
        }
    }
}

impl Stream {
    /// Wait `slow`, or until the cancel is due.
    fn pause(&self) -> io::Result<()> {
        let until = Instant::now() + self.slow;
        let mut waiter: Option<Waiter> = None;
        loop {
            if self.cancel.is_due() {
                return Err(Cancelled::error());
            }
            if Instant::now() >= until {
                return Ok(());
            }
            let waiter = match &mut waiter {
                Some(w) => w,
                None => waiter.insert(self.cancel.waiter()?),
            };
            let deadline = self.cancel.deadline().map_or(until, |d| d.min(until));
            waiter.wait(&[], Some(deadline))?;
        }
    }
}

fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
