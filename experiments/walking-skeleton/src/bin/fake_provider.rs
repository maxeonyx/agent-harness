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

        let message = if let Some(tool_call) = step.get("tool_call") {
            json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": format!("call_{step_index}"),
                    "type": "function",
                    "function": {
                        "name": tool_call["name"],
                        "arguments": tool_call["arguments"].to_string()
                    }
                }]
            })
        } else {
            json!({ "role": "assistant", "content": step["text"] })
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
