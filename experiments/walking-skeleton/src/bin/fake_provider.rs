//! Fake provider: a separate HTTP server serving the OpenAI-compatible
//! chat-completions API from a response script. It records every request
//! body it receives as JSONL, making the provider wire boundary assertable
//! from outside the harness.

use serde_json::{Value, json};
use std::io::Write;

fn main() {
    let port: u16 = std::env::var("FAKE_PROVIDER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let script_path =
        std::env::var("FAKE_PROVIDER_SCRIPT").expect("FAKE_PROVIDER_SCRIPT must be set");
    let log_path = std::env::var("FAKE_PROVIDER_LOG")
        .unwrap_or_else(|_| "fake-provider-requests.jsonl".to_string());

    let script_text =
        std::fs::read_to_string(&script_path).expect("failed to read FAKE_PROVIDER_SCRIPT");
    let script: Vec<Value> =
        serde_json::from_str(&script_text).expect("FAKE_PROVIDER_SCRIPT must be a JSON array");

    let server =
        tiny_http::Server::http(("127.0.0.1", port)).expect("failed to bind fake provider");
    println!("listening on {}", server.server_addr());
    std::io::stdout().flush().ok();

    let mut step_index = 0usize;
    for mut request in server.incoming_requests() {
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).ok();

        if request.method() != &tiny_http::Method::Post || request.url() != "/v1/chat/completions" {
            let _ = request
                .respond(tiny_http::Response::from_string("not found").with_status_code(404));
            continue;
        }

        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut f| writeln!(f, "{}", body.trim()));
        if let Err(e) = log {
            eprintln!("[fake-provider] failed to log request: {e}");
        }

        let step = script
            .get(step_index)
            .cloned()
            .unwrap_or_else(|| json!({ "text": "fake provider script exhausted" }));
        step_index += 1;

        // Optional per-step delay: lets tests hold a request in flight
        // (e.g. to cancel it, or to interleave user activity).
        if let Some(delay_ms) = step.get("delay_ms").and_then(|v| v.as_u64()) {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

        // A step is either `text`, a single `tool_call`, or `tool_calls`
        // (a list — one response proposing several calls at once).
        let scripted_calls: Vec<serde_json::Value> = if let Some(calls) = step.get("tool_calls") {
            calls.as_array().cloned().unwrap_or_default()
        } else if let Some(call) = step.get("tool_call") {
            vec![call.clone()]
        } else {
            Vec::new()
        };
        let message = if scripted_calls.is_empty() {
            json!({ "role": "assistant", "content": step["text"] })
        } else {
            let tool_calls: Vec<serde_json::Value> = scripted_calls
                .iter()
                .enumerate()
                .map(|(call_index, tool_call)| {
                    json!({
                        "id": format!("call_{step_index}_{call_index}"),
                        "type": "function",
                        "function": {
                            "name": tool_call["name"],
                            "arguments": tool_call["arguments"].to_string()
                        }
                    })
                })
                .collect();
            json!({
                "role": "assistant",
                "content": step.get("text").cloned().unwrap_or(serde_json::Value::Null),
                "tool_calls": tool_calls
            })
        };

        let response_body = json!({
            "id": "chatcmpl-fake",
            "object": "chat.completion",
            "model": "fake-model",
            "choices": [{ "index": 0, "message": message, "finish_reason": "stop" }]
        });
        let response = tiny_http::Response::from_string(response_body.to_string()).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
        let _ = request.respond(response);
    }
}
