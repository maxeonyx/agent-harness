//! A scripted chat-completions server, for the scenario tests.
//!
//! Agents run concurrently, so a positional script cannot say which response
//! belongs to which agent. A rule matches on the content of the request's
//! last message instead — the tail that distinguishes one agent from another.
//!
//! A needle beginning with `^` matches only the last message's first line.
//! Agent identity is always on that first line ("You are agent `x`."), while
//! a child declared with `after` carries its dependency's whole report
//! further down the same message — so an unanchored needle meant for one
//! agent could be captured by another's inherited text, and changing the
//! framing's wording would silently re-point a rule.
//!
//! It speaks HTTP/1.1 itself, over a thread per connection. An off-the-shelf
//! server with a connection thread pool stalled here: under CPU contention it
//! stopped reading accepted sockets, requests sat unread in the kernel, and
//! the suite passed in three minutes instead of one second.
//!
//! Nothing in the script measures out wall-clock time. A rule can hold its
//! request at a `barrier` until a given number of requests are in flight
//! together — which a serialized harness can never satisfy — or `hold` it
//! until the test sends `POST /release`.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Bounds on the two waits. Reaching either is a failure the test is told
/// about, not something any assertion waits for.
const BARRIER_LIMIT: Duration = Duration::from_secs(5);
const HOLD_LIMIT: Duration = Duration::from_secs(30);

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

struct Gate {
    waiting: usize,
    generation: u64,
    released: bool,
}

struct Fake {
    rules: Vec<Value>,
    used: Mutex<Vec<u64>>,
    gate: Mutex<Gate>,
    signal: Condvar,
    log: Mutex<std::fs::File>,
    sequence: AtomicU64,
}

impl Fake {
    /// One `write_all` per entry: a test polling this file while it is being
    /// appended to must never see half a line.
    fn record(&self, entry: Value) {
        let line = format!("{entry}\n");
        let mut file = self.log.lock().unwrap();
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }

    /// Hold until `count` requests are waiting here together. Returns true if
    /// that never happened.
    fn barrier(&self, count: usize) -> bool {
        let mut gate = self.gate.lock().unwrap();
        gate.waiting += 1;
        if gate.waiting >= count {
            gate.waiting = 0;
            gate.generation += 1;
            self.signal.notify_all();
            return false;
        }
        let generation = gate.generation;
        loop {
            let (next, outcome) = self.signal.wait_timeout(gate, BARRIER_LIMIT).unwrap();
            gate = next;
            if gate.generation != generation {
                return false;
            }
            if outcome.timed_out() {
                gate.waiting -= 1;
                return true;
            }
        }
    }

    /// Hold until the test sends `POST /release`. Returns true if it never did.
    fn hold(&self) -> bool {
        let mut gate = self.gate.lock().unwrap();
        loop {
            if gate.released {
                return false;
            }
            let (next, outcome) = self.signal.wait_timeout(gate, HOLD_LIMIT).unwrap();
            gate = next;
            if outcome.timed_out() && !gate.released {
                return true;
            }
        }
    }

    fn release(&self) {
        let mut gate = self.gate.lock().unwrap();
        gate.released = true;
        self.signal.notify_all();
    }

    fn pick(&self, last: &str) -> Value {
        let mut used = self.used.lock().unwrap();
        self.rules
            .iter()
            .enumerate()
            .find(|(index, rule)| {
                let matched = match rule["when"].as_str() {
                    Some(needle) => match needle.strip_prefix('^') {
                        Some(needle) => last.lines().next().unwrap_or("").contains(needle),
                        None => last.contains(needle),
                    },
                    None => true,
                };
                matched && used[*index] < rule["times"].as_u64().unwrap_or(u64::MAX)
            })
            .map(|(index, rule)| {
                used[index] += 1;
                rule.clone()
            })
            .unwrap_or_else(|| {
                json!({ "text": format!("FAKE PROVIDER: no rule matched last message: {last}") })
            })
    }
}

fn main() {
    let port: u16 = std::env::var("FAKE_PROVIDER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let script_path =
        std::env::var("FAKE_PROVIDER_SCRIPT").expect("FAKE_PROVIDER_SCRIPT must be set");
    let log_path = std::env::var("FAKE_PROVIDER_LOG")
        .unwrap_or_else(|_| "fake-provider-requests.jsonl".to_string());

    let script: Value = serde_json::from_str(
        &std::fs::read_to_string(&script_path).expect("failed to read FAKE_PROVIDER_SCRIPT"),
    )
    .expect("FAKE_PROVIDER_SCRIPT must be JSON");
    let rules = script["rules"]
        .as_array()
        .expect("script needs a `rules` array")
        .clone();

    let fake = Arc::new(Fake {
        used: Mutex::new(vec![0; rules.len()]),
        rules,
        gate: Mutex::new(Gate {
            waiting: 0,
            generation: 0,
            released: false,
        }),
        signal: Condvar::new(),
        log: Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .expect("open FAKE_PROVIDER_LOG"),
        ),
        sequence: AtomicU64::new(0),
    });

    let listener = TcpListener::bind(("127.0.0.1", port)).expect("failed to bind fake provider");
    println!("listening on {}", listener.local_addr().unwrap());
    std::io::stdout().flush().ok();

    // Stdin closing is the shutdown signal: when the test process that
    // started this one goes away, so does this one. Without it a fake
    // provider outlives its test and sits there holding a port.
    std::thread::spawn(|| {
        let mut sink = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut sink);
        std::process::exit(0);
    });

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let fake = fake.clone();
        std::thread::spawn(move || converse(stream, &fake));
    }
}

/// One connection, one keep-alive conversation.
fn converse(stream: TcpStream, fake: &Fake) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut stream = stream;
    while let Some((target, body)) = read_request(&mut reader) {
        let (status, response) = answer(&target, &body, fake);
        if write_response(&mut stream, status, &response).is_err() {
            return;
        }
    }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<(String, String)> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut words = line.split_whitespace();
    let target = format!("{} {}", words.next()?, words.next()?);
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        if header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some((target, String::from_utf8_lossy(&body).to_string()))
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

fn answer(target: &str, text: &str, fake: &Fake) -> (u16, String) {
    if target == "POST /release" {
        fake.release();
        return (200, "{}".to_string());
    }
    if !target.starts_with("POST ") || !target.ends_with("/chat/completions") {
        return (404, "{}".to_string());
    }

    let received = millis();
    let sequence = fake.sequence.fetch_add(1, Ordering::SeqCst);
    let body: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let last = body["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message["content"].as_str())
        .unwrap_or("")
        .to_string();
    let rule = fake.pick(&last);

    // Logged on arrival, before any waiting, so a test can see what is in
    // flight.
    fake.record(json!({
        "kind": "received", "seq": sequence, "at": received, "body": body,
    }));

    let mut stuck = false;
    if let Some(count) = rule["barrier"].as_u64() {
        stuck |= fake.barrier(count as usize);
    }
    if rule["hold"].as_bool().unwrap_or(false) {
        stuck |= fake.hold();
    }

    let status = rule["status"].as_u64().unwrap_or(200) as u16;
    let response = if status == 200 {
        let tool_calls: Vec<Value> = rule["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .enumerate()
                    .map(|(index, call)| {
                        json!({
                            "id": format!("call_{sequence}_{index}"),
                            "type": "function",
                            "function": {
                                "name": call["name"],
                                "arguments": call["arguments"].to_string(),
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut message = json!({
            "role": "assistant",
            "content": rule["text"].as_str().unwrap_or(""),
        });
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        json!({
            "id": "chatcmpl-fake",
            "object": "chat.completion",
            "model": body["model"],
            "provider": "fake",
            "choices": [{ "index": 0, "message": message, "finish_reason": "stop" }],
            "usage": rule.get("usage").cloned().unwrap_or(json!({
                "prompt_tokens": 0, "completion_tokens": 0, "cost": 0.0,
                "prompt_tokens_details": { "cached_tokens": 0, "cache_write_tokens": 0 }
            })),
        })
    } else {
        json!({ "error": { "message": rule["text"].as_str().unwrap_or("scripted failure"), "code": status } })
    };

    fake.record(json!({
        "kind": "answered", "seq": sequence, "at": millis(), "stuck": stuck,
    }));
    (status, response.to_string())
}
