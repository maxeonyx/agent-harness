//! The exit-condition scenario from the spike brief, asserted at the
//! provider wire boundary: the fake provider records every request it
//! receives, and the assertions read that log — not harness internals.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn walking_skeleton_scenario() {
    let tmp = std::env::temp_dir().join(format!("walking-skeleton-test-{}", std::process::id()));
    let workdir = tmp.join("workspace");
    std::fs::create_dir_all(&workdir).unwrap();
    std::fs::write(workdir.join("notes.txt"), "remember the milk\n").unwrap();

    let script_path = tmp.join("script.json");
    std::fs::write(
        &script_path,
        serde_json::json!([
            { "tool_call": { "name": "list_files", "arguments": { "path": "." } } },
            { "text": "I listed the files for you." }
        ])
        .to_string(),
    )
    .unwrap();
    let requests_log = tmp.join("requests.jsonl");
    let session_log = tmp.join("session.jsonl");

    let mut provider = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_fake-provider"))
            .env("FAKE_PROVIDER_PORT", "0")
            .env("FAKE_PROVIDER_SCRIPT", &script_path)
            .env("FAKE_PROVIDER_LOG", &requests_log)
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake provider"),
    );
    let mut first_line = String::new();
    BufReader::new(provider.0.stdout.take().unwrap())
        .read_line(&mut first_line)
        .expect("read fake provider address");
    let addr = first_line
        .trim()
        .strip_prefix("listening on ")
        .expect("fake provider readiness line");

    let mut skeleton = Command::new(env!("CARGO_BIN_EXE_skeleton"))
        .env("SKELETON_BASE_URL", format!("http://{addr}/v1"))
        .env("SKELETON_MODEL", "fake-model")
        .env("SKELETON_RECORD", &session_log)
        .current_dir(&workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn skeleton");
    skeleton
        .stdin
        .take()
        .unwrap()
        .write_all(b"hello, what files are here?\n/open notes.txt\n/end\n/quit\n")
        .unwrap();
    let output = skeleton.wait_with_output().expect("skeleton run");
    assert!(output.status.success(), "skeleton exited with failure");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("I listed the files for you."),
        "final agent text missing from face output:\n{stdout}"
    );

    // Provider wire boundary: exactly the requests the design says should
    // exist. Two appends produced zero requests; one turn end produced one
    // request; the tool round-trip produced exactly one more.
    let requests: Vec<serde_json::Value> = std::fs::read_to_string(&requests_log)
        .expect("requests log written")
        .lines()
        .map(|l| serde_json::from_str(l).expect("request is valid JSON"))
        .collect();
    assert_eq!(requests.len(), 2, "expected exactly two provider requests");

    let messages_of = |request: &serde_json::Value| -> Vec<(String, String)> {
        request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m["role"].as_str().unwrap_or_default().to_string(),
                    m["content"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    };

    // First request carries the accumulated context: the typed message and
    // the piggybacked user activity, framed as user activity.
    let first = messages_of(&requests[0]);
    assert!(
        first
            .iter()
            .any(|(role, content)| role == "user" && content == "hello, what files are here?"),
        "typed user message missing from first request: {first:?}"
    );
    assert!(
        first.iter().any(|(role, content)| role == "user"
            && content.starts_with("[user activity]")
            && content.contains("notes.txt")
            && content.contains("remember the milk")),
        "user activity missing or misframed in first request: {first:?}"
    );
    assert!(
        !first
            .iter()
            .any(|(role, _)| role == "tool" || role == "assistant"),
        "first request should predate any agent activity: {first:?}"
    );
    assert!(
        requests[0]["tools"]
            .as_array()
            .is_some_and(|t| !t.is_empty()),
        "first request should declare the limb's tools"
    );

    // Second request carries the tool round-trip: the assistant tool call
    // and the limb's result.
    let second_messages = requests[1]["messages"].as_array().unwrap();
    assert!(
        second_messages.iter().any(|m| m["role"] == "assistant"
            && m["tool_calls"].as_array().is_some_and(|c| !c.is_empty())),
        "assistant tool call missing from second request"
    );
    assert!(
        second_messages.iter().any(|m| m["role"] == "tool"
            && m["content"]
                .as_str()
                .unwrap_or_default()
                .contains("notes.txt")),
        "limb tool result missing from second request"
    );

    // Recorder: both appends happened before inference was triggered, and
    // appending alone sent nothing.
    let events: Vec<String> = std::fs::read_to_string(&session_log)
        .expect("session log written")
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).expect("event is valid JSON")["event"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let trigger_at = events
        .iter()
        .position(|e| e == "trigger_inference")
        .expect("trigger_inference recorded");
    assert!(
        events[..trigger_at].contains(&"append_user_message".to_string())
            && events[..trigger_at].contains(&"append_user_activity".to_string()),
        "appends should precede the trigger: {events:?}"
    );
    assert!(
        !events[..trigger_at].contains(&"request_sent".to_string()),
        "no request may be sent before the turn ends: {events:?}"
    );
    assert_eq!(
        events.iter().filter(|e| *e == "request_sent").count(),
        2,
        "recorder should agree with the wire log on request count"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
