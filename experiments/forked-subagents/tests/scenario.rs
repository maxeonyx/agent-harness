//! Scenarios from the brief, asserted at the two public surfaces: what
//! `forks` printed, and what the provider actually received.
//!
//! The fake provider records each request with the times it arrived and was
//! answered, so "the children ran at the same time" and "the parent's next
//! request came after all of them" are observations, not inferences.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

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
    answered: u128,
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
            .recv_timeout(Duration::from_secs(10))
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

    /// Every request the provider saw, in arrival order.
    fn requests(&self) -> Vec<Req> {
        let Ok(text) = std::fs::read_to_string(&self.log) else {
            return Vec::new();
        };
        let mut requests: Vec<Req> = text
            .lines()
            .map(|line| {
                let entry: Value = serde_json::from_str(line).expect("request log is JSON");
                Req {
                    received: entry["received_ms"].as_u64().unwrap() as u128,
                    answered: entry["answered_ms"].as_u64().unwrap() as u128,
                    body: entry["body"].clone(),
                }
            })
            .collect();
        requests.sort_by_key(|request| request.received);
        requests
    }

    fn find(&self, needle: &str) -> Req {
        self.requests()
            .into_iter()
            .find(|request| request.last().contains(needle))
            .unwrap_or_else(|| panic!("no request whose last message contains {needle:?}"))
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

struct Forks {
    stdout: String,
    stderr: String,
    code: i32,
}

fn forks(dir: &Path, fake: &Fake, extra: &[&str], task: &str) -> Forks {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forks"));
    command
        .arg("run")
        .arg("--dir")
        .arg(dir.join("files"))
        .arg("--base-url")
        .arg(fake.base_url())
        .arg("--model")
        .arg("fake")
        .arg("--provider")
        .arg("")
        .arg("--runs-dir")
        .arg(dir.join("runs"));
    command.args(extra).arg(task);
    let output = command.output().expect("run forks");
    Forks {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
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

fn split_rules(delay: u64) -> Value {
    json!([
        {"when": "SPLIT", "tool_calls": [{"name": "task", "arguments": {"agents": [
            {"name": "north", "task": "report the note"},
            {"name": "south", "task": "list the directory"}
        ]}}]},
        {"when": "agent `north`", "delay_ms": delay, "text": "north says kowhai"},
        {"when": "agent `south`", "delay_ms": delay, "text": "south says tui"},
        {"when": "Every agent you launched has finished", "text": "both reported"}
    ])
}

#[test]
fn scope_suspends_the_parent_while_its_children_run_at_the_same_time() {
    let dir = workspace("scope");
    let fake = Fake::start(&dir, split_rules(400));
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let requests = fake.requests();
    assert_eq!(requests.len(), 4, "root, two children, root again");
    let north = fake.find("agent `north`");
    let south = fake.find("agent `south`");
    assert!(
        north.received < south.answered && south.received < north.answered,
        "the children's requests did not overlap in time"
    );

    let resumed = fake.find("Every agent you launched has finished");
    assert!(
        resumed.received >= north.answered && resumed.received >= south.answered,
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
            {"when": "agent `first`", "delay_ms": 300, "text": "first says rimu"},
            {"when": "agent `second`", "text": "second says weka"},
            {"when": "Every agent you launched has finished", "text": "both reported"}
        ]),
    );
    let out = forks(&dir, &fake, &[], "SPLIT the work");
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    let first = fake.find("agent `first`");
    let second = fake.find("agent `second`");
    assert!(
        second.received >= first.answered,
        "the dependent child started before its dependency finished"
    );
    assert!(
        second.last().contains("first says rimu"),
        "the dependent child did not receive its dependency's report: {}",
        second.last()
    );
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

    let middle_resumed = fake.find("## `leaf`");
    let root_resumed = fake.find("## `middle`");
    let leaf = fake.find("agent `leaf`");
    assert!(leaf.answered <= middle_resumed.received);
    assert!(middle_resumed.answered <= root_resumed.received);
    prefix_is_identical(&fake);
}

#[test]
fn cut_full_gives_the_child_the_parents_bytes_then_one_tool_result() {
    let dir = workspace("cut-full");
    let fake = Fake::start(&dir, split_rules(0));
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
    let fake = Fake::start(&dir, split_rules(0));
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
    let fake = Fake::start(&dir, split_rules(0));
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
    let fake = Fake::start(&dir, split_rules(0));
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
    let mut rules = split_rules(0);
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
    let mut rules = split_rules(0);
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

/// Cancellation, driven the way a user drives it: `/cancel` in chat, while a
/// scope is open.
#[test]
fn cancel_starts_nothing_new_keeps_what_is_in_flight_and_ends_every_agent() {
    let dir = workspace("cancel");
    let fake = Fake::start(&dir, split_rules(700));
    let mut child = Command::new(env!("CARGO_BIN_EXE_forks"))
        .arg("chat")
        .arg("--dir")
        .arg(dir.join("files"))
        .arg("--base-url")
        .arg(fake.base_url())
        .arg("--model")
        .arg("fake")
        .arg("--provider")
        .arg("")
        .arg("--runs-dir")
        .arg(dir.join("runs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn forks chat");
    let mut stdin: ChildStdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let child = KillOnDrop(child);
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    writeln!(stdin, "SPLIT the work").unwrap();
    stdin.flush().unwrap();
    let mut seen = Vec::new();
    loop {
        let line = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("waiting for the scope to open");
        let opened = line.contains("scope opened");
        seen.push(line);
        if opened {
            break;
        }
    }
    // Cancel with both children's requests genuinely in flight: the fake
    // logs a request when it arrives, and holds it for 700ms before
    // answering.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while fake.requests().len() < 3 {
        assert!(
            std::time::Instant::now() < deadline,
            "the children's requests never reached the provider"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    writeln!(stdin, "/cancel").unwrap();
    stdin.flush().unwrap();
    let cancelled_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    loop {
        let line = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("waiting for the run report");
        let done = line.contains("run: cancelled");
        seen.push(line);
        if done {
            break;
        }
    }
    writeln!(stdin, "/quit").ok();
    drop(stdin);
    drop(child);
    reader.join().unwrap();
    let transcript = seen.join("\n");

    let requests = fake.requests();
    assert_eq!(requests.len(), 3, "a request started after the cancel");
    assert!(
        requests
            .iter()
            .all(|request| request.received < cancelled_at),
        "a request was sent after the cancel"
    );
    // The in-flight responses were awaited and kept: both children carry the
    // report their request returned.
    assert!(transcript.contains("root › north") && transcript.contains("agent finished"));
    for (agent, state) in [("north", "completed"), ("south", "completed")] {
        assert!(
            transcript.contains(&format!(
                "root › {agent}                 agent finished: {state}"
            )) || transcript.contains(&format!("{agent} ")),
            "{agent} has no recorded outcome:\n{transcript}"
        );
    }
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(newest_run(&dir.join("runs")).join("summary.json")).unwrap(),
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
        "every agent must end with a recorded outcome"
    );
}

fn newest_run(runs: &Path) -> PathBuf {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(runs)
        .expect("runs directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries.pop().expect("at least one run")
}

/// The benchmark's scoring, fed two scripted trees: one that stays inside its
/// assignments and one that does not.
fn bench_rules(over_reach: bool) -> Value {
    let mut rules = vec![
        json!({"when": "Work out the total", "tool_calls": [{"name": "task", "arguments": {"agents": [
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
    for (region, branch) in [
        ("maunga", "kowhai"),
        ("maunga", "rimu"),
        ("maunga", "totara"),
        ("awa", "tui"),
        ("awa", "kea"),
        ("awa", "weka"),
    ] {
        // `kowhai` reads its sibling's file instead of its own, and `tui`
        // names a sibling in its report: two leaves over-reaching.
        let read = if over_reach && branch == "kowhai" {
            "ledgers/maunga/rimu.txt".to_string()
        } else {
            format!("ledgers/{region}/{branch}.txt")
        };
        rules.push(json!({
            "when": format!("agent `{branch}`"),
            "tool_calls": [{"name": "read_file", "arguments": {"path": read}}]
        }));
        let report = if over_reach && branch == "tui" {
            "tui: 1.00, and kea looks like 2.00 as well".to_string()
        } else {
            format!("{branch}: 1.00")
        };
        rules.push(json!({ "when": format!("# ledger: {region}/{branch}"), "text": report }));
    }
    rules.push(json!({"when": "## `kowhai`", "text": "maunga: 3.00"}));
    rules.push(json!({"when": "## `tui`", "text": "awa: 3.00"}));
    rules.push(json!({"when": "## `maunga`", "text": "kowhai: 1.00\nmaunga: 3.00\ngrand: 6.00"}));
    Value::Array(rules)
}

fn run_bench(name: &str, over_reach: bool) -> String {
    let dir = workspace(name);
    let fake = Fake::start(&dir, bench_rules(over_reach));
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
    text
}

#[test]
fn the_benchmark_scores_a_disciplined_tree_clean() {
    let text = run_bench("bench-clean", false);
    assert!(
        text.contains("structure ok  leaf over-reach 0/6  region over-reach 0/2"),
        "{text}"
    );
    assert!(text.contains("totals WRONG"), "{text}");
}

#[test]
fn the_benchmark_counts_leaves_that_stray_outside_their_assignment() {
    let text = run_bench("bench-overreach", true);
    assert!(
        text.contains("structure ok  leaf over-reach 2/6  region over-reach 0/2"),
        "{text}"
    );
}
