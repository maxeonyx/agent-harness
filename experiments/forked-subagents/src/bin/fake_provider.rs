//! A separate HTTP server speaking the chat-completions API from a script,
//! recording every request body with the times it arrived and was answered.
//!
//! Agents run concurrently, so a positional script cannot say which response
//! belongs to which agent. A rule matches on the content of the request's
//! last message instead — which is exactly the tail that distinguishes one
//! agent from another. Requests are served on several threads, so overlapping
//! requests really do overlap.
//!
//! A request is logged the moment it arrives, before the scripted delay, so a
//! test can wait for requests to be in flight. `answered_ms` is when the delay
//! elapses and the response goes out.

use serde_json::{Value, json};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
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
    let rules: Arc<Vec<Value>> = Arc::new(
        script["rules"]
            .as_array()
            .expect("script needs a `rules` array")
            .clone(),
    );

    let server = Arc::new(
        tiny_http::Server::http(("127.0.0.1", port)).expect("failed to bind fake provider"),
    );
    println!("listening on {}", server.server_addr());
    std::io::stdout().flush().ok();

    let log = Arc::new(Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .expect("open FAKE_PROVIDER_LOG"),
    ));
    let used: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(vec![0; rules.len()]));

    let mut workers = Vec::new();
    for _ in 0..8 {
        let server = server.clone();
        let rules = rules.clone();
        let log = log.clone();
        let used = used.clone();
        workers.push(std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                serve(request, &rules, &log, &used);
            }
        }));
    }
    for worker in workers {
        let _ = worker.join();
    }
}

fn serve(
    mut request: tiny_http::Request,
    rules: &[Value],
    log: &Mutex<std::fs::File>,
    used: &Mutex<Vec<usize>>,
) {
    let received = millis();
    let mut text = String::new();
    request.as_reader().read_to_string(&mut text).ok();
    if request.method() != &tiny_http::Method::Post || !request.url().ends_with("/chat/completions")
    {
        let _ =
            request.respond(tiny_http::Response::from_string("not found").with_status_code(404));
        return;
    }
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let last = body["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message["content"].as_str())
        .unwrap_or("")
        .to_string();

    let rule = {
        let mut used = used.lock().unwrap();
        rules
            .iter()
            .enumerate()
            .find(|(index, rule)| {
                let matched = match rule["when"].as_str() {
                    Some(needle) => last.contains(needle),
                    None => true,
                };
                let times = rule["times"].as_u64().unwrap_or(u64::MAX);
                matched && (used[*index] as u64) < times
            })
            .map(|(index, rule)| {
                used[index] += 1;
                rule.clone()
            })
    };

    let rule = rule.unwrap_or_else(
        || json!({ "text": format!("FAKE PROVIDER: no rule matched last message: {last}") }),
    );

    let delay = rule["delay_ms"].as_u64().unwrap_or(0);
    {
        let entry = json!({
            "received_ms": received,
            "answered_ms": received + delay as u128,
            "body": body,
        });
        let mut file = log.lock().unwrap();
        let _ = writeln!(file, "{entry}");
        let _ = file.flush();
    }
    if delay > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }

    let status = rule["status"].as_u64().unwrap_or(200);
    let response_body = if status == 200 {
        let tool_calls: Vec<Value> = rule["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .enumerate()
                    .map(|(index, call)| {
                        json!({
                            "id": format!("call_{received}_{index}"),
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
        let mut message =
            json!({ "role": "assistant", "content": rule["text"].as_str().unwrap_or("") });
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

    let response = tiny_http::Response::from_string(response_body.to_string())
        .with_status_code(status as u16)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
    let _ = request.respond(response);
}
