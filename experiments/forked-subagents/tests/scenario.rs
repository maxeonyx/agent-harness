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
    /// Held open on purpose: the fake provider exits when its stdin closes,
    /// so it can never outlive the test that started it.
    _stdin: std::process::ChildStdin,
    addr: String,
    log: PathBuf,
}

struct Req {
    received: u128,
    answered: Option<u128>,
    stuck: bool,
    headers: Value,
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake provider");
        let stdin = child.stdin.take().unwrap();
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
            _stdin: stdin,
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
                        headers: entry["headers"].clone(),
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
        "--backend".into(),
        "openrouter".into(),
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

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn interrupt(&self) {
        let done = Command::new("kill")
            .arg("-INT")
            .arg(self.pid().to_string())
            .status()
            .expect("send SIGINT");
        assert!(done.success(), "could not signal forks");
    }

    fn close_stdin(&mut self) {
        drop(self.stdin.take());
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

    /// Wait for forks to exit on its own, with stdin still open: closing it
    /// would hand the process a way out the user never gave it.
    fn exits_by_itself(mut self) -> (String, i32) {
        let deadline = Instant::now() + PATIENCE;
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("poll forks") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(200)) {
                    self.seen.push(line);
                }
                panic!("forks never exited:\n{}", self.seen.join("\n"));
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(200)) {
            self.seen.push(line);
        }
        (self.seen.join("\n"), status.code().unwrap_or(-1))
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
    let mut north = json!({"when": "^You are agent `north`", "text": "north says kowhai"});
    let mut south = json!({"when": "^You are agent `south`", "text": "south says tui"});
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
            {"when": "^You are agent `first`", "text": "first says rimu"},
            {"when": "^You are agent `second`", "text": "second says weka"},
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
            {"when": "^You are agent `middle`", "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "leaf", "task": "do the actual work"}
            ]}}]},
            {"when": "^You are agent `leaf`", "text": "leaf says totara"},
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
        "when": "^You are agent `north`", "status": 401, "text": "No auth credentials found"
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
            {"when": "^You are agent `middle`", "tool_calls": [{"name": "task", "arguments": {"agents": [
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

/// Cancellation, driven the way a user drives it: Ctrl-C in chat, with both
/// children's requests held open at the provider. It waits for them, and
/// then it closes.
#[test]
fn cancel_waits_for_what_is_in_flight_then_closes() {
    let dir = workspace("cancel");
    let fake = Fake::start(&dir, split_rules(json!({ "hold": true })));
    let mut live = Live::start(&dir, &fake, "chat", &[], None);
    live.send("SPLIT the work");
    fake.await_requests(3);
    live.interrupt();
    live.wait_for("cancelling");
    // The responses the provider was holding come back, and are kept.
    fake.release();
    let (transcript, code) = live.exits_by_itself();
    assert_eq!(code, 0, "{transcript}");
    assert!(transcript.contains("run: cancelled"), "{transcript}");

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
    /// Finding 1: the region agent names the siblings in the leaf's task,
    /// and the leaf then reads one of them.
    task_names_siblings: bool,
    /// Finding 2: a leaf says, in prose, that it stayed out of its siblings'
    /// ledgers.
    leaf_protests_innocence: bool,
    /// Finding 4: a region agent reads `ledgers/policy.md`, which does not
    /// exist — the real file is `POLICY.md`.
    region_reads_wrong_case_policy: bool,
    /// Finding 3: how the root dresses its totals block.
    totals_style: Style,
    /// A leaf that splits its work again, and whose assignment names no
    /// ledger path — so its remit cannot be established either.
    leaf_forks_anonymously: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Plain,
    /// Bold, backticked, bulleted, and followed by a blank line.
    Markdown,
    /// A correct block preceded by an earlier, wrong value for one name.
    WrongThenRight,
}

/// Normally `weka`, doing weka's work. When the tree is testing an
/// unidentifiable leaf, an agent with a name that matches no branch and an
/// assignment that names no ledger — which then splits its work again.
fn weka_entry(tree: &Tree) -> Value {
    if tree.leaf_forks_anonymously {
        json!({"name": "extra", "task": "handle whatever is left over"})
    } else {
        json!({"name": "weka", "task": leaf_task("awa", "weka", tree)})
    }
}

fn leaf_task(region: &str, branch: &str, tree: &Tree) -> String {
    let base = format!("total ledgers/{region}/{branch}.txt");
    if tree.task_names_siblings {
        let siblings: Vec<&str> = BRANCHES
            .iter()
            .filter(|(r, b)| *r == region && *b != branch)
            .map(|(_, b)| *b)
            .collect();
        format!(
            "{base} ({} are being handled by other agents, do not touch them)",
            siblings.join(" and ")
        )
    } else {
        base
    }
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
        json!({"when": "^You are agent `maunga`", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "kowhai", "task": leaf_task("maunga", "kowhai", tree)},
            {"name": "rimu", "task": leaf_task("maunga", "rimu", tree)},
            {"name": "totara", "task": leaf_task("maunga", "totara", tree)}
        ]}}]}),
        json!({"when": "^You are agent `awa`", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "tui", "task": leaf_task("awa", "tui", tree)},
            {"name": "kea", "task": leaf_task("awa", "kea", tree)},
            weka_entry(tree)
        ]}}]}),
    ];
    if tree.region_reads_wrong_case_policy {
        rules.insert(
            2,
            json!({"when": "^You are agent `maunga`", "times": 1,
                   "tool_calls": [{"name": "read_file", "arguments": {"path": "ledgers/policy.md"}}]}),
        );
        rules.insert(
            3,
            json!({"when": "Error: ledgers/policy.md", "times": 1,
                   "tool_calls": [{"name": "task", "arguments": {"agents": [
                {"name": "kowhai", "task": leaf_task("maunga", "kowhai", tree)},
                {"name": "rimu", "task": leaf_task("maunga", "rimu", tree)},
                {"name": "totara", "task": leaf_task("maunga", "totara", tree)}
            ]}}]}),
        );
    }
    if tree.re_reads_policy {
        // `maunga` reads the policy itself before splitting; the rule that
        // answers it sits ahead of the plain one, and both are used once.
        rules.insert(
            2,
            json!({"when": "^You are agent `maunga`", "times": 1,
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
        rules.push(json!({ "when": format!("^You are agent `{branch}`"), "tool_calls": reads }));
        let report = if tree.leaf_protests_innocence && branch == "kowhai" {
            format!(
                "kowhai: {:.2} — I did not read rimu or totara, as instructed.",
                tree.totals.get("kowhai").unwrap_or(&1.0)
            )
        } else if tree.over_reach && branch == "tui" {
            // A claimed total for a branch that is not its own.
            "tui: 1.00\nkea: 2.00".to_string()
        } else {
            format!("{branch}: {:.2}", tree.totals.get(branch).unwrap_or(&1.0))
        };
        rules.push(json!({ "when": format!("# ledger: {region}/{branch}"), "text": report }));
    }
    if tree.leaf_forks_anonymously {
        rules.push(json!({"when": "^You are agent `extra`",
                          "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "helper", "task": "look at the ledgers for me"}
        ]}}]}));
        rules.push(json!({"when": "levels below the root", "text": "extra: 0.00"}));
    }
    rules.push(json!({"when": "## `kowhai`", "text": "maunga done"}));
    rules.push(json!({"when": "## `tui`", "text": "awa done"}));
    let names = [
        "kowhai", "rimu", "totara", "tui", "kea", "weka", "maunga", "awa", "grand",
    ];
    let value = |name: &str| *tree.totals.get(name).unwrap_or(&1.0);
    let block = match tree.totals_style {
        Style::Plain => names
            .iter()
            .map(|name| format!("{name}: {:.2}", value(name)))
            .collect::<Vec<_>>()
            .join("\n"),
        Style::Markdown => {
            format!(
                "Here are the totals.\n\n{}\n",
                names
                    .iter()
                    .map(|name| format!("- **{name}: {:.2}**", value(name)))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
        Style::WrongThenRight => format!(
            "kowhai: 0.01\n{}",
            names
                .iter()
                .map(|name| format!("{name}: {:.2}", value(name)))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    };
    rules.push(json!({"when": "## `maunga`", "text": block}));
    Value::Array(rules)
}

fn run_bench(name: &str, tree: &Tree) -> (String, PathBuf) {
    let dir = workspace(name);
    let fake = Fake::start(&dir, bench_rules(tree));
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("bench")
        .args(["--backend", "openrouter"])
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
        task_names_siblings: false,
        leaf_protests_innocence: false,
        region_reads_wrong_case_policy: false,
        totals_style: Style::Plain,
        leaf_forks_anonymously: false,
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
            task_names_siblings: false,
            leaf_protests_innocence: false,
            region_reads_wrong_case_policy: false,
            totals_style: Style::Plain,
            leaf_forks_anonymously: false,
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
            task_names_siblings: false,
            leaf_protests_innocence: false,
            region_reads_wrong_case_policy: false,
            totals_style: Style::Plain,
            leaf_forks_anonymously: false,
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
            task_names_siblings: false,
            leaf_protests_innocence: false,
            region_reads_wrong_case_policy: false,
            totals_style: Style::Plain,
            leaf_forks_anonymously: false,
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
            task_names_siblings: false,
            leaf_protests_innocence: false,
            region_reads_wrong_case_policy: false,
            totals_style: Style::Plain,
            leaf_forks_anonymously: false,
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
            task_names_siblings: false,
            leaf_protests_innocence: false,
            region_reads_wrong_case_policy: false,
            totals_style: Style::Plain,
            leaf_forks_anonymously: false,
        },
    );
    assert!(
        text.contains("leaf over-reach 0/6  region over-reach 0/2  policy re-reads 2/8"),
        "{text}"
    );
}

// ------------------------------------------- the reviewer's reproductions

fn tree(correct: BTreeMap<String, f64>) -> Tree {
    Tree {
        over_reach: false,
        re_reads_policy: false,
        totals: correct,
        task_names_siblings: false,
        leaf_protests_innocence: false,
        region_reads_wrong_case_policy: false,
        totals_style: Style::Plain,
        leaf_forks_anonymously: false,
    }
}

/// Finding 1. The set of branches a leaf may touch used to be every branch
/// name appearing anywhere in its assignment, which handed the parent control
/// of the scorer: name the siblings in the prose and reading them became free.
#[test]
fn a_leaf_that_reads_a_siblings_ledger_is_over_reach_even_when_its_task_names_the_sibling() {
    let (correct, _) = learn_fixture("bench-learn-f1");
    let mut plan = tree(correct);
    plan.task_names_siblings = true;
    plan.over_reach = true;
    let (text, _) = run_bench("bench-f1", &plan);
    assert!(
        text.contains("leaf over-reach 2/6"),
        "a leaf read a sibling's ledger and was scored clean:\n{text}"
    );
}

/// Finding 2. Naming a sibling is not doing its work. The `explained` framing
/// hands every child its siblings' names, so a mention-counting metric
/// punished the treatment under test for being explicit.
#[test]
fn a_leaf_that_only_says_it_stayed_out_is_not_over_reach() {
    let (correct, _) = learn_fixture("bench-learn-f2");
    let mut plan = tree(correct);
    plan.leaf_protests_innocence = true;
    let (text, _) = run_bench("bench-f2", &plan);
    assert!(
        text.contains("leaf over-reach 0/6"),
        "perfect discipline was scored as a violation:\n{text}"
    );
}

/// Finding 3, the false-negative half: a model that writes its totals as a
/// bulleted, bolded list with a trailing blank line has still answered.
#[test]
fn totals_survive_the_markdown_a_model_actually_writes() {
    let (correct, _) = learn_fixture("bench-learn-f3a");
    let mut plan = tree(correct);
    plan.totals_style = Style::Markdown;
    let (text, _) = run_bench("bench-f3a", &plan);
    assert!(text.contains("totals ok"), "{text}");
}

/// Finding 3, the false-positive half: the old check asked whether *some*
/// line gave the right value, so a wrong value earlier in the block passed.
#[test]
fn a_name_given_the_wrong_value_anywhere_in_the_block_fails() {
    let (correct, _) = learn_fixture("bench-learn-f3b");
    let mut plan = tree(correct);
    plan.totals_style = Style::WrongThenRight;
    let (text, _) = run_bench("bench-f3b", &plan);
    assert!(text.contains("totals WRONG"), "{text}");
}

/// Finding 4. A read that returned an error is not a read. `ledgers/policy.md`
/// is not the policy — the real file is `POLICY.md` — so the attempt is
/// neither over-reach nor a policy re-read.
#[test]
fn a_failed_read_is_not_a_read() {
    let (correct, _) = learn_fixture("bench-learn-f4");
    let mut plan = tree(correct);
    plan.region_reads_wrong_case_policy = true;
    let (text, _) = run_bench("bench-f4", &plan);
    assert!(
        text.contains("region over-reach 0/2  policy re-reads 0/8"),
        "a failed read was counted:\n{text}"
    );
}

/// `forks rescore` must reach the same verdict as the benchmark did, from the
/// trial directory alone.
#[test]
fn rescoring_a_recorded_benchmark_reproduces_its_scores() {
    let (correct, _) = learn_fixture("bench-learn-rescore");
    let mut plan = tree(correct);
    plan.over_reach = true;
    plan.re_reads_policy = true;
    let (bench_text, bench_dir) = run_bench("bench-rescore", &plan);

    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("rescore")
        .arg(&bench_dir)
        .output()
        .expect("run forks rescore");
    let rescored = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "{rescored}");

    for claim in [
        "leaf over-reach 2/6",
        "region over-reach 0/2",
        "policy re-reads 2/8",
    ] {
        assert!(bench_text.contains(claim), "bench line lost {claim}");
    }
    assert!(
        rescored.contains("leaf 2/6 region 0/2 policy 2/8"),
        "rescore disagreed with the run it replayed:\n{rescored}"
    );
    assert!(rescored.contains("[unchanged]"), "{rescored}");
}

// ------------------------------------------------- majors and moderates

/// Ctrl-C at the chat prompt, with nothing running, closes at once.
#[test]
fn cancel_at_an_idle_prompt_closes() {
    let dir = workspace("cancel-idle");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let mut live = Live::start(&dir, &fake, "chat", &[], None);
    live.wait_for("chat:");
    live.interrupt();
    let (transcript, code) = live.exits_by_itself();
    assert_eq!(code, 0, "{transcript}");
    assert_eq!(fake.requests().len(), 0, "{transcript}");
}

/// Finding 5. A retry is new work, and cancellation has to reach it. The
/// provider holds the first attempt, answers it with a 503 once released, and
/// the retry must never be sent.
#[test]
fn cancelling_stops_the_retries() {
    let dir = workspace("cancel-retries");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT", "status": 503, "hold": true, "times": 1, "text": "upstream is busy"},
            {"when": "^SPLIT", "status": 503, "text": "upstream is still busy"}
        ]),
    );
    let mut live = Live::start(&dir, &fake, "run", &[], Some("SPLIT the work"));
    fake.await_requests(1);
    live.interrupt();
    live.wait_for("cancelling");
    fake.release();
    let (transcript, code) = live.finish();

    assert_eq!(
        fake.requests().len(),
        1,
        "a retry was sent after the cancel:\n{transcript}"
    );
    assert_eq!(code, 0, "cancelling is not a failure:\n{transcript}");
}

/// The interrupt stays armed: a second Ctrl-C force-cancels. The responses
/// the provider is still holding are abandoned, the run closes at once, and
/// every agent still ends with a recorded outcome.
#[test]
fn a_second_interrupt_force_cancels() {
    let dir = workspace("second-interrupt");
    let fake = Fake::start(&dir, split_rules(json!({ "hold": true })));
    let mut live = Live::start(&dir, &fake, "run", &[], Some("SPLIT the work"));
    fake.await_requests(3);
    live.interrupt();
    live.wait_for("Ctrl-C again to force-cancel");
    live.interrupt();
    let (transcript, code) = live.exits_by_itself();
    assert_eq!(
        code, 130,
        "the second interrupt was swallowed:\n{transcript}"
    );
    let states = agent_states(&dir);
    assert!(
        states.values().all(|state| state == "cancelled"),
        "every agent must end cancelled: {states:?}\n{transcript}"
    );
    let wire = std::fs::read_to_string(newest(&dir.join("runs")).join("wire.jsonl")).unwrap();
    assert_eq!(
        wire.matches(r#""kind":"abandoned""#).count(),
        2,
        "both held requests should be recorded as abandoned:\n{wire}"
    );
}

/// Finding 6. A panicked agent is still an agent that ended.
#[test]
fn a_panicked_child_ends_with_a_recorded_outcome() {
    let dir = workspace("panic-child");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(
        &dir,
        &fake,
        &["--panic-in", "root › north"],
        "SPLIT the work",
    );
    assert_ne!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("panicked"),
        "the face did not report the panic:\n{}",
        out.stdout
    );
    let states = agent_states(&dir);
    assert_eq!(
        states.get("root › north").map(String::as_str),
        Some("panicked"),
        "the panicked agent has no recorded outcome: {states:?}"
    );
    assert!(
        !states.values().any(|state| state == "running"),
        "an agent was left running after the run ended: {states:?}"
    );
}

/// The root is the one whose evidence you most want: a panic there must still
/// leave a `summary.json` behind.
#[test]
fn a_panicked_root_still_writes_its_summary() {
    let dir = workspace("panic-root");
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks(&dir, &fake, &["--panic-in", "root"], "SPLIT the work");
    assert_ne!(out.code, 0, "{}", out.stdout);
    let states = agent_states(&dir);
    assert_eq!(
        states.get("root").map(String::as_str),
        Some("panicked"),
        "{states:?}"
    );
}

fn agent_states(dir: &Path) -> BTreeMap<String, String> {
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(newest(&dir.join("runs")).join("summary.json"))
            .expect("a run that panicked still writes summary.json"),
    )
    .unwrap();
    summary["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| {
            (
                agent["path"].as_str().unwrap().to_string(),
                agent["state"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Finding 7. Stdin closing during a turn must not turn into a busy loop on a
/// channel that is ready forever.
#[test]
fn chat_does_not_spin_after_stdin_closes() {
    let dir = workspace("chat-eof");
    let fake = Fake::start(&dir, split_rules(json!({ "hold": true })));
    let mut live = Live::start(&dir, &fake, "chat", &[], None);
    live.send("SPLIT the work");
    fake.await_requests(3);
    live.close_stdin();

    let pid = live.pid();
    std::thread::sleep(Duration::from_millis(300));
    let before = cpu_jiffies(pid);
    std::thread::sleep(Duration::from_millis(500));
    let spent = cpu_jiffies(pid) - before;

    fake.release();
    let (transcript, _) = live.finish();
    // A spinning select burns a whole core: ~50 jiffies in half a second.
    // Waiting properly costs none of them.
    assert!(
        spent < 15,
        "forks chat burned {spent} jiffies in 500ms with nothing to do:\n{transcript}"
    );
}

/// User plus system time, in clock ticks, from `/proc/<pid>/stat`.
fn cpu_jiffies(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("read /proc stat");
    // The process name can contain spaces and brackets; fields are counted
    // from after the closing bracket.
    let tail = stat.rsplit_once(')').expect("stat has a name").1;
    let fields: Vec<&str> = tail.split_whitespace().collect();
    let utime: u64 = fields[11].parse().unwrap_or(0);
    let stime: u64 = fields[12].parse().unwrap_or(0);
    utime + stime
}

/// A leaf splitting its work again is over-reach whatever it was assigned —
/// it is the bottom of the intended tree. Judging its remit first meant an
/// agent whose assignment named no ledger was filed as unscoreable and its
/// `task` call went uncounted.
#[test]
fn a_leaf_that_forks_is_over_reach_even_when_its_remit_is_unclear() {
    let (correct, _) = learn_fixture("bench-learn-f-anon");
    let mut plan = tree(correct);
    plan.leaf_forks_anonymously = true;
    let (text, _) = run_bench("bench-f-anon", &plan);
    assert!(
        text.contains("leaf over-reach 1/6"),
        "a leaf called task and was not counted:\n{text}"
    );
}

// ------------------------------------------------------------ rate limits

/// OpenRouter answers a rate limit with HTTP 200 and an `error` object whose
/// code is 429. Reading only the HTTP status made that permanent, and a whole
/// grid of luna trials died in seconds.
#[test]
fn a_rate_limit_arriving_as_a_200_is_waited_out_not_treated_as_fatal() {
    let dir = workspace("rate-limit");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT", "error_code": 429, "times": 2,
             "text": "temporarily rate-limited upstream. Please retry shortly"},
            {"when": "^SPLIT", "text": "answered once the rate limit passed"}
        ]),
    );
    let out = forks(
        &dir,
        &fake,
        &["--rate-limit-patience", "2"],
        "SPLIT the work",
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("rate limited, waiting"),
        "the rate limit was not waited out:\n{}",
        out.stdout
    );
    assert!(out.stdout.contains("answered once the rate limit passed"));
    assert_eq!(
        fake.requests().len(),
        3,
        "both 429s should have been retried"
    );
}

/// `Retry-After` is what the provider asked for, so it wins over the schedule.
#[test]
fn retry_after_is_honoured() {
    let dir = workspace("retry-after");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT", "status": 429, "retry_after": 1, "times": 1,
             "text": "slow down"},
            {"when": "^SPLIT", "text": "answered after the requested wait"}
        ]),
    );
    // The schedule would wait 100s/24 ≈ 4s. Retry-After says one second, and
    // the run finishes in less than the schedule would have allowed.
    let started = Instant::now();
    let out = forks(
        &dir,
        &fake,
        &["--rate-limit-patience", "100"],
        "SPLIT the work",
    );
    let took = started.elapsed();
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("rate limited, waiting 1.0s"),
        "{}",
        out.stdout
    );
    assert!(
        took < Duration::from_secs(4),
        "waited {took:?}, so Retry-After was ignored in favour of the schedule"
    );
}

/// Patience runs out eventually, and that is a provider fault.
#[test]
fn a_rate_limit_that_never_lifts_ends_as_a_provider_fault() {
    let dir = workspace("rate-limit-forever");
    let fake = Fake::start(
        &dir,
        json!([{"when": "^SPLIT", "error_code": 429, "text": "still rate-limited"}]),
    );
    let out = forks(
        &dir,
        &fake,
        &["--rate-limit-patience", "0.3"],
        "SPLIT the work",
    );
    assert_ne!(out.code, 0);
    assert!(
        out.stdout.contains("rate limited for longer than"),
        "{}",
        out.stdout
    );
}

/// A trial the provider broke is not evidence about the model: it is shown as
/// invalid, counted on its own, and kept out of every rate and mean. A trial
/// that hit its spend cap is the model's own doing and stays in.
#[test]
fn a_provider_fault_invalidates_a_trial_while_a_spend_cap_does_not() {
    let (correct, _) = learn_fixture("bench-learn-invalid");
    let plan = tree(correct);

    // One combo whose root is rate-limited past all patience, and one whose
    // root simply costs more than the cap allows.
    let dir = workspace("bench-invalid");
    let mut rules = bench_rules(&plan).as_array().unwrap().clone();
    rules.insert(
        0,
        json!({"when": "Read `ledgers/POLICY.md` first", "error_code": 429,
               "text": "temporarily rate-limited upstream"}),
    );
    let fake = Fake::start(&dir, Value::Array(rules));
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("bench")
        .args(["--backend", "openrouter"])
        .arg("--base-url")
        .arg(fake.base_url())
        .arg("--grid")
        .arg("fake@")
        .arg("--reps")
        .arg("1")
        .arg("--rate-limit-patience")
        .arg("0.2")
        .arg("--runs-dir")
        .arg(dir.join("runs"))
        .output()
        .expect("run forks bench");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("INVALID (provider fault"),
        "a provider fault was not shown as invalid:\n{text}"
    );
    // Every rate is withheld rather than reported as a row of zeroes.
    assert!(
        text.contains("| 1 | 1 | — | — | — | — | — | — | — | — | — | — |"),
        "an invalid trial was folded into the rates:\n{text}"
    );

    // The same tree, but stopped by its own spending: still a real trial.
    let dir = workspace("bench-capped");
    let fake = Fake::start(&dir, bench_rules(&plan));
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("bench")
        .args(["--backend", "openrouter"])
        .arg("--base-url")
        .arg(fake.base_url())
        .arg("--grid")
        .arg("fake@")
        .arg("--reps")
        .arg("1")
        .arg("--max-cost")
        .arg("0.0")
        .arg("--runs-dir")
        .arg(dir.join("runs"))
        .output()
        .expect("run forks bench");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "{text}");
    assert!(
        !text.contains("INVALID"),
        "a trial stopped by its own spending was discarded:\n{text}"
    );
    assert!(text.contains("| 1 | 0 |"), "{text}");
}

// ------------------------------------------------------------------- face

/// Max: "I want to know *exactly* what's in the model's context window". So
/// every message entering any agent's context is on the screen, whole and
/// exact: the `task` call's arguments, the child's own tail, the tool result
/// its parent is resumed with, and the final replies.
#[test]
fn the_face_shows_every_message_exactly_as_it_enters_a_context() {
    let dir = workspace("face-exact");
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT", "text": "Looking first.",
             "tool_calls": [{"name": "list_dir", "arguments": {"path": "."}},
                            {"name": "task", "arguments": {"agents": [
                {"name": "north", "task": "report the note"}
             ]}}]},
            {"when": "^You are agent `north`", "text": "north line one\nnorth line two"},
            {"when": "Every agent you launched has finished",
             "text": "First paragraph of the answer.\n\nSecond paragraph of the answer."}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    for expected in [
        "root                         [1 user]\n    │ SPLIT the work",
        "root                         [2 assistant]\n    │ Looking first.",
        "root                         [2 assistant tool_use list_dir call_0_0]\n    │ {\"path\":\".\"}",
        "root                         [2 assistant tool_use task call_0_1]\n    │ {\"agents\":[{\"name\":\"north\",\"task\":\"report the note\"}]}",
        "root                         [3 tool list_dir call_0_0]\n    │ note.txt",
        "root › north                 context: root's messages 0–1, then\nroot › north                 [2 user]\n    │ You are agent `north`.\n    │\n    │ Your assignment:\n    │ report the note",
        "root › north                 [3 assistant]\n    │ north line one\n    │ north line two",
        "root                         [4 tool task call_0_1]\n    │ Every agent you launched has finished.",
        "\n    │ ## `north` — completed\n    │ north line one\n    │ north line two",
        "root                         [5 assistant]\n    │ First paragraph of the answer.\n    │\n    │ Second paragraph of the answer.",
    ] {
        assert!(
            out.stdout.contains(expected),
            "missing from the face:\n{expected}\n\nface:\n{}",
            out.stdout
        );
    }
}

#[test]
fn tool_results_are_shown_whole() {
    let dir = workspace("face-results");
    std::fs::write(
        dir.join("files").join("long.txt"),
        (1..=20)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT",
             "tool_calls": [{"name": "read_file", "arguments": {"path": "long.txt"}},
                            {"name": "read_file", "arguments": {"path": "/etc/passwd"}}]},
            {"when": "line 1", "text": "done"}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let whole = (1..=20)
        .map(|n| format!("\n    │ line {n}"))
        .collect::<String>();
    assert!(
        out.stdout
            .contains(&format!("[3 tool read_file call_0_0]{whole}\n")),
        "a tool result was not shown whole:\n{}",
        out.stdout
    );
    assert!(
        out.stdout.contains(
            "[4 tool read_file call_0_1]\n    │ Error: path must be relative to the run directory; got /etc/passwd"
        ),
        "the limb's error was not shown:\n{}",
        out.stdout
    );
}

/// `/tree` showed `0.0s` against the agent that was still going, which is the
/// one time you want to know how long it has been.
#[test]
fn tree_shows_how_long_a_running_agent_has_been_going() {
    let dir = workspace("face-elapsed");
    let fake = Fake::start(&dir, split_rules(json!({ "hold": true })));
    let mut live = Live::start(&dir, &fake, "chat", &[], None);
    live.send("SPLIT the work");
    fake.await_requests(3);
    // Testing elapsed time needs time to elapse; a quarter of a second is
    // enough to tell 0.0s from a number.
    std::thread::sleep(Duration::from_millis(250));
    live.send("/tree");
    live.wait_for("TOTAL");
    fake.release();
    live.send("/quit");
    let (transcript, _) = live.finish();

    let running: Vec<&str> = transcript
        .lines()
        .filter(|line| line.contains(" running ") || line.contains(" suspended "))
        .collect();
    assert!(
        !running.is_empty(),
        "no agent was shown as still going:\n{transcript}"
    );
    assert!(
        running.iter().all(|line| !line.ends_with("0.0s")),
        "a running agent was shown as having taken no time:\n{}",
        running.join("\n")
    );
}

/// opencode's credential store, as opencode2 lays it out, holding one Claude
/// subscription token that expires `expires_in_ms` from now.
fn opencode_db(dir: &Path, expires_in_ms: i64) -> PathBuf {
    let path = dir.join("opencode.db");
    let _ = std::fs::remove_file(&path);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE credential (id text PRIMARY KEY, integration_id text, label text NOT NULL, value text NOT NULL, connector_id text, method_id text, active integer, time_created integer NOT NULL, time_updated integer NOT NULL);",
    )
    .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let value = json!({
        "type": "oauth", "methodID": "claude-pro-max",
        "refresh": "sk-ant-ort-fixture", "access": "sk-ant-oat-fixture",
        "expires": now + expires_in_ms, "metadata": {"email": "max@example.com"},
    });
    db.execute(
        "INSERT INTO credential VALUES ('cred_1', 'anthropic', 'max@example.com', ?1, NULL, NULL, NULL, ?2, ?2)",
        rusqlite::params![value.to_string(), now],
    )
    .unwrap();
    path
}

fn forks_on_claude(dir: &Path, fake: &Fake, db: &Path, extra: &[&str], task: &str) -> Forks {
    let output = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("run")
        .arg("--dir")
        .arg(dir.join("files"))
        .args([
            "--backend",
            "claude",
            "--model",
            "anthropic/claude-sonnet-5",
        ])
        .arg("--base-url")
        .arg(format!("http://{}", fake.addr))
        .arg("--credentials")
        .arg(db)
        .arg("--runs-dir")
        .arg(dir.join("runs"))
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

/// Max's Claude subscription, with the two things Anthropic needs to accept
/// its token: the OAuth beta, and Claude Code's identity as `system[0]`. A
/// fork is still its parent's bytes: on this API one turn's tool results
/// travel together in one user message, and the child's copy of that
/// message differs from its parent's only in the `task` slot.
#[test]
fn the_claude_backend_sends_the_subscription_token_and_forks_share_bytes() {
    let dir = workspace("claude-backend");
    let db = opencode_db(&dir, 3_600_000);
    let fake = Fake::start(
        &dir,
        json!([
            {"when": "^SPLIT", "text": "Looking first.", "thinking": "Two files, so two agents.",
             "usage": {"input_tokens": 10, "cache_creation_input_tokens": 200,
                       "cache_read_input_tokens": 3000, "output_tokens": 7},
             "tool_calls": [{"name": "list_dir", "arguments": {"path": "."}},
                            {"name": "task", "arguments": {"agents": [
                {"name": "north", "task": "report the note"}
             ]}}]},
            {"when": "^You are agent `north`", "text": "north says kowhai"},
            {"when": "Every agent you launched has finished", "text": "done"}
        ]),
    );
    let out = forks_on_claude(&dir, &fake, &db, &["--cut", "full"], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains(
            "request returned   in 3210 (cached 3000, written 200)  out 7  $0.0000  via claude subscription, billed to five_hour"
        ),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("[3 assistant thinking]\n    │ Two files, so two agents."),
        "{}",
        out.stdout
    );

    let requests = fake.requests();
    assert_eq!(requests.len(), 3, "{}", out.stdout);
    for request in &requests {
        assert_eq!(
            request.headers["authorization"],
            "Bearer sk-ant-oat-fixture"
        );
        assert_eq!(request.headers["anthropic-beta"], "oauth-2025-04-20");
        assert_eq!(request.headers["anthropic-version"], "2023-06-01");
        let body = &request.body;
        assert_eq!(body["model"], "claude-sonnet-5");
        assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
        assert_eq!(
            body["system"][0]["text"],
            "You are Claude Code, Anthropic's official CLI for Claude."
        );
        assert_eq!(
            body["system"], requests[0].body["system"],
            "system differed"
        );
        assert_eq!(body["tools"], requests[0].body["tools"], "tools differed");
        assert_eq!(body["tools"][2]["name"], "task");
        assert!(body["tools"][2]["input_schema"].is_object());
    }

    let messages = |request: &Req| request.body["messages"].as_array().unwrap().clone();
    let root = messages(&requests[0]);
    let child = messages(&requests[1]);
    let resumed = messages(&requests[2]);
    assert_eq!(
        child[..1],
        root[..],
        "the child did not start from the root's bytes"
    );
    assert_eq!(resumed[..1], root[..]);
    assert_eq!(
        child[1], resumed[1],
        "the child's copy of the `task` turn differs"
    );
    assert_eq!(
        child[1]["content"],
        json!([
            {"type": "thinking", "thinking": "Two files, so two agents.", "signature": "sig-fake"},
            {"type": "text", "text": "Looking first."},
            {"type": "tool_use", "id": "toolu_0_0", "name": "list_dir", "input": {"path": "."}},
            {"type": "tool_use", "id": "toolu_0_1", "name": "task",
             "input": {"agents": [{"name": "north", "task": "report the note"}]}}
        ])
    );
    for (request, task_result) in [
        (&child, "You are agent `north`."),
        (&resumed, "Every agent you launched has finished."),
    ] {
        let results = request[2]["content"].as_array().unwrap();
        assert_eq!(request.len(), 3);
        assert_eq!(request[2]["role"], "user");
        assert_eq!(
            results[0],
            json!({"type": "tool_result", "tool_use_id": "toolu_0_0", "content": "note.txt"})
        );
        assert_eq!(results[1]["tool_use_id"], "toolu_0_1");
        assert!(
            results[1]["content"]
                .as_str()
                .unwrap()
                .starts_with(task_result)
        );
    }
}

#[test]
fn an_expired_subscription_token_stops_the_run_before_any_request() {
    let dir = workspace("claude-expired");
    let db = opencode_db(&dir, -3_600_000);
    let fake = Fake::start(&dir, split_rules(json!({})));
    let out = forks_on_claude(&dir, &fake, &db, &[], "SPLIT the work");
    assert_ne!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stderr.contains("expired") && out.stderr.contains("opencode"),
        "{}",
        out.stderr
    );
    assert_eq!(fake.requests().len(), 0);
}

/// Anthropic refuses a request whose identity it does not accept with a 429
/// whose message is "Error". It is not a rate limit, and waiting it out never
/// works.
#[test]
fn an_identity_rejection_is_a_fault_not_a_rate_limit() {
    let dir = workspace("claude-identity");
    let db = opencode_db(&dir, 3_600_000);
    let fake = Fake::start(
        &dir,
        json!([{"when": "^SPLIT", "status": 429, "text": "Error"}]),
    );
    let out = forks_on_claude(&dir, &fake, &db, &[], "SPLIT the work");
    assert_ne!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("rejected the request identity"),
        "{}",
        out.stdout
    );
    assert_eq!(fake.requests().len(), 1, "the rejection was retried");
}
