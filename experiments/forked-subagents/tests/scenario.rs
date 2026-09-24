//! Scenarios from the brief, asserted at the two public surfaces: what
//! `forks` printed, and what the provider actually received.
//!
//! No assertion here waits for wall-clock time to pass. Concurrency is proved
//! by a barrier in the fake provider — requests that must be in flight
//! together, which a serialized harness can never satisfy — and ordering is
//! proved by content: the dependent child's request contains its
//! dependency's report, so it cannot have been built before it. Where a test
//! needs requests held open (cancellation, the spend cap), it holds them
//! explicitly and releases them when it is ready.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PATIENCE: Duration = Duration::from_secs(30);

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fake {
    _child: KillOnDrop,
    addr: String,
    log: PathBuf,
}

struct Req {
    received: u128,
    answered: Option<u128>,
    stuck: bool,
    body: Value,
}

impl Req {
    fn messages(&self) -> &Vec<Value> {
        self.body["messages"].as_array().expect("messages array")
    }

    fn last(&self) -> &str {
        self.messages()
            .last()
            .and_then(|message| message["content"].as_str())
            .unwrap_or("")
    }
}

impl Fake {
    fn start(dir: &Path, rules: Value) -> Fake {
        let script = dir.join("script.json");
        std::fs::write(&script, json!({ "rules": rules }).to_string()).unwrap();
        let log = dir.join("requests.jsonl");
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-provider"))
            .env("FAKE_PROVIDER_PORT", "0")
            .env("FAKE_PROVIDER_SCRIPT", &script)
            .env("FAKE_PROVIDER_LOG", &log)
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake provider");
        let stdout = child.stdout.take().unwrap();
        let child = KillOnDrop(child);
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line);
        });
        let line = rx
            .recv_timeout(PATIENCE)
            .expect("fake provider never printed its readiness line");
        reader.join().unwrap();
        let addr = line
            .trim()
            .strip_prefix("listening on ")
            .expect("readiness line")
            .to_string();
        Fake {
            _child: child,
            addr,
            log,
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// Every request the provider has seen, in arrival order, whether or not
    /// it has been answered yet.
    fn requests(&self) -> Vec<Req> {
        let Ok(text) = std::fs::read_to_string(&self.log) else {
            return Vec::new();
        };
        let mut arrived: BTreeMap<u64, Req> = BTreeMap::new();
        // The log is read while the provider is still appending to it, so the
        // last line may not be there in full yet. Any earlier line must parse.
        let complete = text.rfind('\n').map(|at| &text[..at]).unwrap_or("");
        for line in complete.lines() {
            let entry: Value = serde_json::from_str(line).expect("request log is JSON");
            let seq = entry["seq"].as_u64().unwrap();
            let at = entry["at"].as_u64().unwrap() as u128;
            if entry["kind"] == "received" {
                arrived.insert(
                    seq,
                    Req {
                        received: at,
                        answered: None,
                        stuck: false,
                        body: entry["body"].clone(),
                    },
                );
            } else if let Some(request) = arrived.get_mut(&seq) {
                request.answered = Some(at);
                request.stuck = entry["stuck"].as_bool().unwrap_or(false);
            }
        }
        arrived.into_values().collect()
    }

    fn find(&self, needle: &str) -> Req {
        self.requests()
            .into_iter()
            .find(|request| request.last().contains(needle))
            .unwrap_or_else(|| panic!("no request whose last message contains {needle:?}"))
    }

    /// Wait until `count` requests have arrived. Fails loudly rather than
    /// hanging if they never do.
    fn await_requests(&self, count: usize) {
        let deadline = Instant::now() + PATIENCE;
        while self.requests().len() < count {
            assert!(
                Instant::now() < deadline,
                "only {} of {count} requests ever reached the provider",
                self.requests().len()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Let every held request answer.
    fn release(&self) {
        let mut stream = std::net::TcpStream::connect(&self.addr).expect("connect to fake");
        write!(
            stream,
            "POST /release HTTP/1.1\r\nHost: fake\r\nContent-Length: 0\r\n\r\n"
        )
        .unwrap();
        stream.flush().unwrap();
        stream.set_read_timeout(Some(PATIENCE)).unwrap();
        let mut answer = String::new();
        BufReader::new(stream)
            .read_line(&mut answer)
            .expect("fake acknowledged release");
    }

    /// No request may have given up waiting at a barrier or a hold; that
    /// would mean the harness never put them in flight together.
    fn none_stuck(&self) {
        for request in self.requests() {
            assert!(
                !request.stuck,
                "a request waited out its barrier or hold: {}",
                request.last()
            );
        }
    }
}

/// A scratch directory under `target/`, one per test.
fn workspace(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("files")).unwrap();
    std::fs::write(dir.join("files").join("note.txt"), "a note\n").unwrap();
    dir
}

fn base_args(dir: &Path, fake: &Fake) -> Vec<String> {
    vec![
        "--dir".into(),
        dir.join("files").display().to_string(),
        "--base-url".into(),
        fake.base_url(),
        "--model".into(),
        "fake".into(),
        "--provider".into(),
        "".into(),
        "--runs-dir".into(),
        dir.join("runs").display().to_string(),
    ]
}

struct Forks {
    stdout: String,
    stderr: String,
    code: i32,
}

fn forks(dir: &Path, fake: &Fake, extra: &[&str], task: &str) -> Forks {
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("run")
        .args(base_args(dir, fake))
        .args(extra)
        .arg(task)
        .output()
        .expect("run forks");
    Forks {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

/// `forks` driven live, so the test can act while a turn is running.
struct Live {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
    seen: Vec<String>,
}

impl Live {
    fn start(dir: &Path, fake: &Fake, command: &str, extra: &[&str], task: Option<&str>) -> Live {
        let mut builder = Command::new(env!("CARGO_BIN_EXE_forks"));
        builder.arg(command).args(base_args(dir, fake)).args(extra);
        if let Some(task) = task {
            builder.arg(task);
        }
        let mut child = builder
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn forks");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Live {
            child,
            stdin: Some(stdin),
            lines,
            reader: Some(reader),
            seen: Vec::new(),
        }
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn wait_for(&mut self, needle: &str) {
        loop {
            let line = self
                .lines
                .recv_timeout(PATIENCE)
                .unwrap_or_else(|_| panic!("never saw {needle:?}; saw:\n{}", self.seen.join("\n")));
            let hit = line.contains(needle);
            self.seen.push(line);
            if hit {
                return;
            }
        }
    }

    fn finish(mut self) -> (String, i32) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("forks exited");
        while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(200)) {
            self.seen.push(line);
        }
        self.reader.take().unwrap().join().unwrap();
        (self.seen.join("\n"), status.code().unwrap_or(-1))
    }
}

/// Every agent in a run must see the same system prompt and the same tools,
/// or a forked child's inherited prefix is not the parent's bytes.
fn prefix_is_identical(fake: &Fake) {
    let requests = fake.requests();
    assert!(!requests.is_empty(), "no requests were made");
    let system = requests[0].messages()[0].clone();
    let tools = requests[0].body["tools"].clone();
    for request in &requests {
        assert_eq!(request.messages()[0], system, "system prompt differed");
        assert_eq!(request.body["tools"], tools, "tool list differed");
    }
}

/// Two children, launched together. `hold` says how each child's request is
/// held: at a barrier that only genuine concurrency can clear, or until the
/// test releases it.
fn split_rules(hold: Value) -> Value {
    let mut north = json!({"when": "agent `north`", "text": "north says kowhai"});
    let mut south = json!({"when": "agent `south`", "text": "south says tui"});
    for (key, value) in hold.as_object().into_iter().flatten() {
        north[key] = value.clone();
        south[key] = value.clone();
    }
    json!([
        {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "north", "task": "report the note"},
            {"name": "south", "task": "list the directory"}
        ]}}]},
        north,
        south,
        {"when": "Every agent you launched has finished", "text": "both reported"}
    ])
}

#[test]
fn scope_suspends_the_parent_while_its_children_run_at_the_same_time() {
    let dir = workspace("scope");
    // Neither child is answered until both are in flight: a harness that ran
    // them one after another would never get past the first.
    let fake = Fake::start(&dir, split_rules(json!({ "barrier": 2 })));
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    fake.none_stuck();

    let requests = fake.requests();
    assert_eq!(requests.len(), 4, "root, two children, root again");

    let north = fake.find("agent `north`");
    let south = fake.find("agent `south`");
    let resumed = fake.find("Every agent you launched has finished");
    assert!(
        resumed.received >= north.answered.unwrap() && resumed.received >= south.answered.unwrap(),
        "the parent was resumed before its children finished"
    );
    let tool_results: Vec<&Value> = resumed
        .messages()
        .iter()
        .filter(|message| message["role"] == "tool")
        .collect();
    assert_eq!(tool_results.len(), 1, "the scope returned one tool result");
    let content = tool_results[0]["content"].as_str().unwrap();
    let north_at = content.find("north says kowhai").expect("north's report");
    let south_at = content.find("south says tui").expect("south's report");
    assert!(north_at < south_at, "reports were not in declared order");

    assert!(out.stdout.contains("scope opened: north, south"));
    assert!(out.stdout.contains("scope returned"));
    prefix_is_identical(&fake);
}

#[test]
fn after_makes_a_sibling_wait_and_hands_it_the_report() {
    let dir = workspace("after");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "first", "task": "go first"},
                {"name": "second", "task": "go second", "after": ["first"]}
            ]}}]},
            {"when": "agent `first`", "text": "first says rimu"},
            {"when": "agent `second`", "text": "second says weka"},
            {"when": "Every agent you launched has finished", "text": "both reported"}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let first = fake.find("agent `first`");
    let second = fake.find("agent `second`");
    // The dependent child's request carries its dependency's report, so it
    // cannot have been built before that report existed.
    assert!(
        second.last().contains("first says rimu"),
        "the dependent child did not receive its dependency's report: {}",
        second.last()
    );
    assert!(second.received >= first.answered.unwrap());
    prefix_is_identical(&fake);
}

#[test]
fn nested_scopes_resume_bottom_up() {
    let dir = workspace("nested");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "middle", "task": "split again"}
            ]}}]},
            {"when": "agent `middle`", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "leaf", "task": "do the actual work"}
            ]}}]},
            {"when": "agent `leaf`", "text": "leaf says totara"},
            {"when": "## `leaf`", "text": "middle passes on totara"},
            {"when": "## `middle`", "text": "root passes on totara"}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("root › middle › leaf"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("root passes on totara"));

    // Each resumption carries the report from the level below it.
    assert!(fake.find("## `leaf`").last().contains("leaf says totara"));
    assert!(
        fake.find("## `middle`")
            .last()
            .contains("middle passes on totara")
    );
    prefix_is_identical(&fake);
}

#[test]
fn cut_full_gives_the_child_the_parents_bytes_then_one_tool_result() {
    let dir = workspace("cut-full");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(&dir, &fake, &["--cut", "full"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let parent = fake.find("SPLIT the work");
    let child = fake.find("agent `north`");
    let parent_messages = parent.messages();
    let child_messages = child.messages();
    assert_eq!(child_messages.len(), parent_messages.len() + 2);
    assert_eq!(
        &child_messages[..parent_messages.len()],
        &parent_messages[..],
        "the child's prefix was not the parent's bytes"
    );
    let turn = &child_messages[parent_messages.len()];
    assert_eq!(turn["role"], "assistant");
    let arguments = turn["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    assert!(arguments.contains("north") && arguments.contains("south"));
    let tail = child_messages.last().unwrap();
    assert_eq!(tail["role"], "tool");
    assert_eq!(tail["tool_call_id"], turn["tool_calls"][0]["id"]);
    assert!(tail["content"].as_str().unwrap().contains("agent `north`"));
    prefix_is_identical(&fake);
}

#[test]
fn cut_own_rewrites_the_childs_copy_of_the_task_call() {
    let dir = workspace("cut-own");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(&dir, &fake, &["--cut", "own"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let parent = fake.find("SPLIT the work");
    let child = fake.find("agent `north`");
    let parent_messages = parent.messages();
    let child_messages = child.messages();
    assert_eq!(child_messages.len(), parent_messages.len() + 2);
    assert_eq!(
        &child_messages[..parent_messages.len()],
        &parent_messages[..]
    );
    let arguments = child_messages[parent_messages.len()]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    assert!(arguments.contains("north"), "{arguments}");
    assert!(
        !arguments.contains("south"),
        "the child saw its sibling's assignment: {arguments}"
    );
    prefix_is_identical(&fake);
}

#[test]
fn cut_before_ends_at_the_message_before_the_task_call() {
    let dir = workspace("cut-before");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(&dir, &fake, &["--cut", "before"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let parent = fake.find("SPLIT the work");
    let child = fake.find("agent `north`");
    let parent_messages = parent.messages();
    let child_messages = child.messages();
    assert_eq!(child_messages.len(), parent_messages.len() + 1);
    assert_eq!(
        &child_messages[..parent_messages.len()],
        &parent_messages[..]
    );
    let tail = child_messages.last().unwrap();
    assert_eq!(tail["role"], "user");
    assert!(tail["content"].as_str().unwrap().contains("agent `north`"));
    assert!(
        child_messages
            .iter()
            .all(|message| message["tool_calls"].is_null()),
        "the child saw the parent's task call"
    );
    prefix_is_identical(&fake);
}

#[test]
fn fresh_children_get_the_system_prompt_and_their_assignment_only() {
    let dir = workspace("fresh");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(&dir, &fake, &["--mode", "fresh"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let child = fake.find("agent `north`");
    assert_eq!(child.messages().len(), 2);
    assert_eq!(child.messages()[1]["role"], "user");
    prefix_is_identical(&fake);
}

#[test]
fn a_rejected_request_faults_the_agent_and_leaves_its_ancestors_suspended() {
    let dir = workspace("fault");
    let mut rules = split_rules(json!({}));
    rules.as_array_mut().unwrap()[1] = json!({
        "when": "agent `north`", "status": 401, "text": "No auth credentials found"
    });
    let fake = Fake::start(&dir, rules);
    let out = forks(&dir, &fake, &[], "SPLIT the work");

    assert_ne!(out.code, 0, "a fault must not exit zero");
    assert!(
        out.stdout.contains("root › north") && out.stdout.contains("FAULT"),
        "the face did not name the faulted agent: {}",
        out.stdout
    );
    assert!(out.stdout.contains("No auth credentials found"));
    assert!(
        out.stdout.contains("this agent stays suspended"),
        "{}",
        out.stdout
    );
    assert!(
        fake.requests().iter().all(|request| !request
            .last()
            .contains("Every agent you launched has finished")),
        "the parent was resumed after a fault"
    );
}

#[test]
fn the_spend_cap_is_a_fault_before_the_request_that_would_break_it() {
    let dir = workspace("cap");
    let mut rules = split_rules(json!({}));
    rules.as_array_mut().unwrap()[0]["usage"] = json!({
        "prompt_tokens": 100, "completion_tokens": 10, "cost": 0.02,
        "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 100}
    });
    let fake = Fake::start(&dir, rules);
    let out = forks(&dir, &fake, &["--max-cost", "0.01"], "SPLIT the work");

    assert_ne!(out.code, 0);
    assert!(out.stdout.contains("spend cap reached"), "{}", out.stdout);
    assert_eq!(
        fake.requests().len(),
        1,
        "a request was sent after the cap was reached"
    );
}

/// A scope puts several requests in the air at once. The cap must count what
/// is already in flight, or the run overshoots it by a whole fan-out.
#[test]
fn the_cap_counts_the_requests_already_in_flight() {
    let dir = workspace("cap-in-flight");
    let mut rules = split_rules(json!({ "hold": true }));
    let usage = json!({
        "prompt_tokens": 100, "completion_tokens": 10, "cost": 0.02,
        "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 100}
    });
    rules.as_array_mut().unwrap()[0]["usage"] = usage.clone();
    rules.as_array_mut().unwrap()[1]["usage"] = usage;
    let fake = Fake::start(&dir, rules);

    let mut live = Live::start(&dir, &fake, "run", &["--max-cost", "0.04"], Some("SPLIT"));
    // The root's request cost $0.02 and one child's is in flight, unanswered.
    // The second child must be refused on what that one might cost.
    fake.await_requests(2);
    live.wait_for("spend cap reached");
    fake.release();
    let (transcript, code) = live.finish();

    assert_ne!(code, 0);
    assert!(
        transcript.contains("in flight at up to"),
        "the cap did not account for work in flight:\n{transcript}"
    );
    assert_eq!(
        fake.requests().len(),
        2,
        "the second child was sent while the first child's cost was still unknown"
    );
}

/// A provider that accepts a request and never answers must not hang the
/// tree. The first attempt is held open and never answered; the request
/// times out, and the retry succeeds.
#[test]
fn a_silent_provider_times_out_and_the_retry_succeeds() {
    let dir = workspace("timeout");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "SPLIT", "hold": true, "times": 1, "text": "never answered"},
            {"when": "SPLIT", "text": "answered on the retry"}
        ]),
    );
    let out = forks(&dir, &fake, &["--request-timeout", "0.2"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("answered on the retry"),
        "{}",
        out.stdout
    );
    assert_eq!(fake.requests().len(), 2, "the request was not retried");
}

#[test]
fn an_over_deep_task_call_is_an_error_result_not_a_missing_tool() {
    let dir = workspace("depth");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "middle", "task": "split again"}
            ]}}]},
            {"when": "agent `middle`", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "leaf", "task": "one level too deep"}
            ]}}]},
            {"when": "levels below the root", "text": "middle did it itself"},
            {"when": "## `middle`", "text": "root passes it on"}
        ]),
    );
    let out = forks(&dir, &fake, &["--max-depth", "1"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("task refused: at the depth limit"));
    assert!(out.stdout.contains("root passes it on"));

    let refused = fake.find("levels below the root");
    assert_eq!(refused.messages().last().unwrap()["role"], "tool");
    assert!(
        !out.stdout.contains("root › middle › leaf"),
        "an over-deep child was launched anyway"
    );
    // The error is in a tool result; the tool list itself never changes.
    prefix_is_identical(&fake);
}

#[test]
fn an_invalid_task_call_is_answered_with_an_error_result() {
    let dir = workspace("invalid");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "a", "task": "one", "after": ["b"]},
                {"name": "b", "task": "two", "after": ["a"]}
            ]}}]},
            {"when": "cycle", "text": "root gave up on splitting"}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("task rejected"), "{}", out.stdout);
    let rejected = fake.find("cycle");
    assert_eq!(rejected.messages().last().unwrap()["role"], "tool");
    prefix_is_identical(&fake);
}

/// Cancellation, driven the way a user drives it: `/cancel` in chat, with
/// both children's requests held open at the provider.
#[test]
fn cancel_starts_nothing_new_keeps_what_is_in_flight_and_ends_every_agent() {
    let dir = workspace("cancel");
    let fake = Fake::start(&dir, split_rules(json!({ "hold": true })));
    let mut live = Live::start(&dir, &fake, "chat", &[], None);
    live.send("SPLIT the work");
    fake.await_requests(3);
    live.send("/cancel");
    live.wait_for("cancelling");
    // The responses the provider was holding come back, and are kept.
    fake.release();
    live.wait_for("run: cancelled");
    live.send("/quit");
    let (transcript, code) = live.finish();
    assert_eq!(code, 0, "{transcript}");

    assert_eq!(
        fake.requests().len(),
        3,
        "a request started after the cancel"
    );
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(newest(&dir.join("runs")).join("summary.json")).unwrap(),
    )
    .unwrap();
    let states: Vec<(&str, &str)> = summary["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| {
            (
                agent["path"].as_str().unwrap(),
                agent["state"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        states,
        vec![
            ("root", "cancelled"),
            ("root › north", "completed"),
            ("root › south", "completed")
        ],
        "every agent must end with a recorded outcome:\n{transcript}"
    );
}

fn newest(runs: &Path) -> PathBuf {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(runs)
        .expect("runs directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries.pop().expect("at least one run")
}

// ---------------------------------------------------------------- benchmark

const BRANCHES: [(&str, &str); 6] = [
    ("maunga", "kowhai"),
    ("maunga", "rimu"),
    ("maunga", "totara"),
    ("awa", "tui"),
    ("awa", "kea"),
    ("awa", "weka"),
];

struct Tree {
    /// One leaf reads a sibling's ledger, and one names a sibling in its
    /// report.
    over_reach: bool,
    /// A region agent and a leaf both read `POLICY.md` for themselves —
    /// what a fresh child has to do, and not over-reach.
    re_reads_policy: bool,
    /// The totals block the root ends with.
    totals: BTreeMap<String, f64>,
}

/// A scripted run of the benchmark's intended shape.
fn bench_rules(tree: &Tree) -> Value {
    let mut rules = vec![
        json!({"when": "Read `ledgers/POLICY.md` first",
               "tool_calls": [{"name": "read_file", "arguments": {"path": "ledgers/POLICY.md"}}]}),
        json!({"when": "Ledger and Reconciliation Policy", "times": 1,
               "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "maunga", "task": "total the maunga branches"},
            {"name": "awa", "task": "total the awa branches"}
        ]}}]}),
        json!({"when": "agent `maunga`", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "kowhai", "task": "total ledgers/maunga/kowhai.txt"},
            {"name": "rimu", "task": "total ledgers/maunga/rimu.txt"},
            {"name": "totara", "task": "total ledgers/maunga/totara.txt"}
        ]}}]}),
        json!({"when": "agent `awa`", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "tui", "task": "total ledgers/awa/tui.txt"},
            {"name": "kea", "task": "total ledgers/awa/kea.txt"},
            {"name": "weka", "task": "total ledgers/awa/weka.txt"}
        ]}}]}),
    ];
    if tree.re_reads_policy {
        // `maunga` reads the policy itself before splitting; the rule that
        // answers it sits ahead of the plain one, and both are used once.
        rules.insert(
            2,
            json!({"when": "agent `maunga`", "times": 1,
                   "tool_calls": [{"name": "read_file", "arguments": {"path": "ledgers/POLICY.md"}}]}),
        );
        rules.insert(
            3,
            json!({"when": "Ledger and Reconciliation Policy", "times": 1,
                   "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "kowhai", "task": "total ledgers/maunga/kowhai.txt"},
                {"name": "rimu", "task": "total ledgers/maunga/rimu.txt"},
                {"name": "totara", "task": "total ledgers/maunga/totara.txt"}
            ]}}]}),
        );
    }
    for (region, branch) in BRANCHES {
        let ledger = if tree.over_reach && branch == "kowhai" {
            "ledgers/maunga/rimu.txt".to_string()
        } else {
            format!("ledgers/{region}/{branch}.txt")
        };
        let mut reads = vec![json!({"name": "read_file", "arguments": {"path": ledger}})];
        if tree.re_reads_policy && branch == "weka" {
            reads.insert(
                0,
                json!({"name": "read_file", "arguments": {"path": "ledgers/POLICY.md"}}),
            );
        }
        rules.push(json!({ "when": format!("agent `{branch}`"), "tool_calls": reads }));
        let report = if tree.over_reach && branch == "tui" {
            "tui: 1.00, and kea looks like 2.00 as well".to_string()
        } else {
            format!("{branch}: {:.2}", tree.totals.get(branch).unwrap_or(&1.0))
        };
        rules.push(json!({ "when": format!("# ledger: {region}/{branch}"), "text": report }));
    }
    rules.push(json!({"when": "## `kowhai`", "text": "maunga done"}));
    rules.push(json!({"when": "## `tui`", "text": "awa done"}));
    let block = [
        "kowhai", "rimu", "totara", "tui", "kea", "weka", "maunga", "awa", "grand",
    ]
    .iter()
    .map(|name| format!("{name}: {:.2}", tree.totals.get(*name).unwrap_or(&1.0)))
    .collect::<Vec<_>>()
    .join("\n");
    rules.push(json!({"when": "## `maunga`", "text": block}));
    Value::Array(rules)
}

fn run_bench(name: &str, tree: &Tree) -> (String, PathBuf) {
    let dir = workspace(name);
    let fake = Fake::start(&dir, bench_rules(tree));
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("bench")
        .arg("--base-url")
        .arg(fake.base_url())
        .arg("--grid")
        .arg("fake@")
        .arg("--reps")
        .arg("1")
        .arg("--runs-dir")
        .arg(dir.join("runs"))
        .output()
        .expect("run forks bench");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bench_dir = text
        .lines()
        .find_map(|line| line.strip_prefix("benchmark in "))
        .map(PathBuf::from)
        .expect("bench directory line");
    (text, bench_dir)
}

/// Re-derive the totals from the generated fixture, applying the rules
/// `POLICY.md` states. `refunds_added` is what an agent that missed the
/// refund rule would report.
fn fixture_totals(bench_dir: &Path, refunds_added: bool) -> BTreeMap<String, f64> {
    let mut totals = BTreeMap::new();
    let mut grand = 0.0;
    for (region, _) in BRANCHES {
        totals.entry(region.to_string()).or_insert(0.0);
    }
    for (region, branch) in BRANCHES {
        let text = std::fs::read_to_string(
            bench_dir
                .join("fixture/ledgers")
                .join(region)
                .join(format!("{branch}.txt")),
        )
        .expect("fixture ledger");
        let mut total = 0.0;
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.is_empty() || words[0].starts_with('#') || words.last() == Some(&"DUP") {
                continue;
            }
            let Ok(amount) = words[1].parse::<f64>() else {
                continue; // VOID
            };
            if words.last() == Some(&"REFUND") && !refunds_added {
                total -= amount;
            } else {
                total += amount;
            }
        }
        let total = (total * 100.0).round() / 100.0;
        totals.insert(branch.to_string(), total);
        *totals.get_mut(region).unwrap() += total;
        grand += total;
    }
    for (_, value) in totals.iter_mut() {
        *value = (*value * 100.0).round() / 100.0;
    }
    totals.insert("grand".to_string(), (grand * 100.0).round() / 100.0);
    totals
}

/// The fixture is deterministic, so one throwaway run is enough to learn it.
fn learn_fixture(name: &str) -> (BTreeMap<String, f64>, BTreeMap<String, f64>) {
    let tree = Tree {
        over_reach: false,
        re_reads_policy: false,
        totals: BTreeMap::new(),
    };
    let (_, bench_dir) = run_bench(name, &tree);
    (
        fixture_totals(&bench_dir, false),
        fixture_totals(&bench_dir, true),
    )
}

#[test]
fn the_benchmark_prints_the_caps_that_bound_a_trial() {
    let (text, _) = run_bench(
        "bench-caps",
        &Tree {
            over_reach: false,
            re_reads_policy: false,
            totals: BTreeMap::new(),
        },
    );
    assert!(
        text.contains(
            "per-trial cap $0.1500 · per-trial depth limit 2 · whole-benchmark budget $5.0000"
        ),
        "{text}"
    );
}

#[test]
fn the_benchmark_scores_a_disciplined_tree_clean_and_correct() {
    let (correct, _) = learn_fixture("bench-learn-clean");
    let (text, _) = run_bench(
        "bench-clean",
        &Tree {
            over_reach: false,
            re_reads_policy: false,
            totals: correct,
        },
    );
    assert!(
        text.contains(
            "structure ok  leaf over-reach 0/6  region over-reach 0/2  policy re-reads 0/8  totals ok"
        ),
        "{text}"
    );
}

#[test]
fn the_benchmark_counts_leaves_that_stray_outside_their_assignment() {
    let (correct, _) = learn_fixture("bench-learn-overreach");
    let (text, _) = run_bench(
        "bench-overreach",
        &Tree {
            over_reach: true,
            re_reads_policy: false,
            totals: correct,
        },
    );
    assert!(
        text.contains("structure ok  leaf over-reach 2/6  region over-reach 0/2"),
        "{text}"
    );
}

/// Ignoring the refund rule — the rule that only `POLICY.md` states — must
/// show up as wrong totals, or the policy document is decorative.
#[test]
fn the_benchmark_rejects_totals_that_ignore_the_refund_rule() {
    let (_, refunds_added) = learn_fixture("bench-learn-refund");
    let (text, _) = run_bench(
        "bench-refund",
        &Tree {
            over_reach: false,
            re_reads_policy: false,
            totals: refunds_added,
        },
    );
    assert!(text.contains("totals WRONG"), "{text}");
}

/// Re-reading the policy is what a fresh child pays instead of inheriting it.
/// It is counted, and it is not over-reach.
#[test]
fn a_policy_re_read_is_counted_and_is_not_over_reach() {
    let (correct, _) = learn_fixture("bench-learn-policy");
    let (text, _) = run_bench(
        "bench-policy",
        &Tree {
            over_reach: false,
            re_reads_policy: true,
            totals: correct,
        },
    );
    assert!(
        text.contains("leaf over-reach 0/6  region over-reach 0/2  policy re-reads 2/8"),
        "{text}"
    );
}
