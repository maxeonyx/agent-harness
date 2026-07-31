//! The exit-condition scenarios from the spike brief, asserted at the
//! public surfaces: the face's CLI output and the provider wire boundary
//! (the fake provider records every request it receives). The harness
//! drives stdin interactively and reads stdout live, so it can observe
//! what the provider saw *between* steps — not just after the fact.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct FakeProvider {
    _child: KillOnDrop,
    addr: String,
    requests_log: std::path::PathBuf,
}

impl FakeProvider {
    fn start(tmp: &std::path::Path, script: serde_json::Value) -> Self {
        let script_path = tmp.join("script.json");
        std::fs::write(&script_path, script.to_string()).unwrap();
        let requests_log = tmp.join("requests.jsonl");
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-provider"))
            .env("FAKE_PROVIDER_PORT", "0")
            .env("FAKE_PROVIDER_SCRIPT", &script_path)
            .env("FAKE_PROVIDER_LOG", &requests_log)
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake provider");
        let mut first_line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut first_line)
            .expect("read fake provider address");
        let addr = first_line
            .trim()
            .strip_prefix("listening on ")
            .expect("fake provider readiness line")
            .to_string();
        FakeProvider {
            _child: KillOnDrop(child),
            addr,
            requests_log,
        }
    }

    fn requests(&self) -> Vec<serde_json::Value> {
        match std::fs::read_to_string(&self.requests_log) {
            Ok(text) => text
                .lines()
                .map(|l| serde_json::from_str(l).expect("request is valid JSON"))
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

/// The skeleton under test, driven through its public CLI surface.
///
/// Cleanup goes through the skeleton's own graceful shutdown, exactly as
/// a user's terminal would deliver it: dropping stdin is Ctrl-D, the face
/// turns EOF into Quit, the brain drains in-flight work, and the limb
/// kills and reaps its process tree. The harness never hunts descendants
/// globally — process ownership lives inside the skeleton, and the tests
/// exercise it rather than reimplement it. Only a wedged skeleton (a bug,
/// reported loudly) gets a last-resort SIGKILL of the skeleton itself.
struct Skeleton {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_rx: mpsc::Receiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
    seen: Vec<String>,
}

impl Drop for Skeleton {
    fn drop(&mut self) {
        drop(self.stdin.take()); // EOF: request graceful shutdown
        use wait_timeout::ChildExt;
        match self.child.wait_timeout(Duration::from_secs(5)) {
            Ok(Some(_)) => {}
            _ => {
                eprintln!(
                    "skeleton did not exit within 5s of stdin EOF; \
                     killing it (its tool process tree may leak — this is a bug)"
                );
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Skeleton {
    fn start(
        workdir: &std::path::Path,
        provider_addr: &str,
        session_log: &std::path::Path,
    ) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_skeleton"))
            .env("SKELETON_BASE_URL", format!("http://{provider_addr}/v1"))
            .env("SKELETON_MODEL", "fake-model")
            .env("SKELETON_RECORD", session_log)
            // /dump opens $EDITOR; cat streams the dump to stdout where the
            // test harness can assert on it.
            .env("EDITOR", "cat")
            .current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn skeleton");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, stdout_rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Skeleton {
            child,
            stdin: Some(stdin),
            stdout_rx,
            reader: Some(reader),
            seen: Vec::new(),
        }
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin already closed");
        writeln!(stdin, "{line}").expect("write to skeleton stdin");
        stdin.flush().unwrap();
    }

    /// Wait until a stdout line containing `needle` arrives; returns all
    /// lines seen so far (the needle line is last). Panics on timeout.
    fn wait_for(&mut self, needle: &str) -> &[String] {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.stdout_rx.recv_timeout(remaining) {
                Ok(line) => {
                    let hit = line.contains(needle);
                    self.seen.push(line);
                    if hit {
                        return &self.seen;
                    }
                }
                Err(_) => panic!(
                    "timed out waiting for {needle:?}; face output so far:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }

    fn drain_seen(&mut self) {
        while let Ok(line) = self.stdout_rx.try_recv() {
            self.seen.push(line);
        }
    }

    fn quit(mut self) {
        self.send("/quit");
        self.wait_exit();
    }

    /// Wait for the process to exit cleanly (without sending /quit) and
    /// join the stdout reader.
    fn wait_exit(&mut self) {
        let status = self.child.wait().expect("skeleton exit");
        assert!(status.success(), "skeleton exited with failure");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("stdout reader thread");
        }
    }
}

/// Read the PIDs a scripted tool command recorded in the test's own
/// fixture file. The test asserts on processes it *owns* by PID — never
/// on global process-table pattern scans.
fn recorded_pids(pid_file: &std::path::Path) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(pid_file) {
            let pids: Vec<u32> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
            if !pids.is_empty() {
                return pids;
            }
        }
        assert!(
            Instant::now() < deadline,
            "tool command never recorded its pids in {pid_file:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Poll until every one of these specific PIDs is dead, or panic.
fn wait_pids_dead(pids: &[u32]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let alive: Vec<u32> = pids
            .iter()
            .copied()
            .filter(|pid| std::path::Path::new(&format!("/proc/{pid}")).exists())
            .collect();
        if alive.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned processes {alive:?} still alive after drain"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn setup(name: &str) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!(
        "walking-skeleton-test-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&tmp).ok();
    std::fs::create_dir_all(tmp.join("workspace")).unwrap();
    tmp
}

fn roles_and_content(request: &serde_json::Value) -> Vec<(String, String, bool)> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["role"].as_str().unwrap_or_default().to_string(),
                m["content"].as_str().unwrap_or_default().to_string(),
                m["tool_calls"].as_array().is_some_and(|c| !c.is_empty()),
            )
        })
        .collect()
}

/// Exit condition (a): passive activity appends without triggering; ending
/// the turn triggers exactly one request carrying the accumulated context;
/// the tool round-trip produces exactly one more.
#[test]
fn append_never_triggers_and_turn_end_triggers_once() {
    let tmp = setup("append");
    let workdir = tmp.join("workspace");
    std::fs::write(workdir.join("notes.txt"), "remember the milk\n").unwrap();

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "list_files", "arguments": { "path": "." } } },
            { "text": "I listed the files for you." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("hello, what files are here?");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/open notes.txt");
    skeleton.wait_for("[face] opened notes.txt");

    // Observe the wire *between* the appends and the trigger: both appends
    // are acknowledged at the face, and the provider has seen nothing.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        provider.requests().is_empty(),
        "appending must not trigger a provider request"
    );

    skeleton.send("/end");
    skeleton.wait_for("I listed the files for you.");
    skeleton.wait_for("[brain] turn complete");
    skeleton.quit();

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "expected exactly two provider requests");

    let first = roles_and_content(&requests[0]);
    assert!(
        first
            .iter()
            .any(|(role, content, _)| role == "user" && content == "hello, what files are here?"),
        "typed user message missing from first request: {first:?}"
    );
    assert!(
        first.iter().any(|(role, content, _)| role == "user"
            && content.starts_with("[user activity]")
            && content.contains("notes.txt")
            && content.contains("remember the milk")),
        "user activity missing or misframed in first request: {first:?}"
    );
    assert!(
        !first
            .iter()
            .any(|(role, _, _)| role == "tool" || role == "assistant"),
        "first request should predate any agent activity: {first:?}"
    );
    assert!(
        requests[0]["tools"]
            .as_array()
            .is_some_and(|t| !t.is_empty()),
        "first request should declare the limb's tools"
    );

    let second = roles_and_content(&requests[1]);
    assert!(
        second
            .iter()
            .any(|(role, _, has_calls)| role == "assistant" && *has_calls),
        "assistant tool call missing from second request: {second:?}"
    );
    assert!(
        second
            .iter()
            .any(|(role, content, _)| role == "tool" && content.contains("notes.txt")),
        "limb tool result missing from second request: {second:?}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Exit condition (b): while a scripted bash sleep tool call runs, user
/// activity arrives, the face remains responsive, and the activity
/// piggybacks on the next request without splitting the tool-call exchange.
#[test]
fn user_activity_during_tool_call_piggybacks() {
    let tmp = setup("piggyback");
    let workdir = tmp.join("workspace");

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "bash", "arguments": { "command": "sleep 1; echo done" } } },
            { "text": "All done." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the slow thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: bash");

    // The tool is now sleeping. The face must stay responsive: the staged
    // acknowledgment has to arrive *before* the tool result does.
    skeleton.send("by the way, another thought");
    let seen = skeleton.wait_for("[face] staged user message");
    assert!(
        !seen.iter().any(|l| l.contains("[limb] tool result")),
        "face acknowledged mid-tool message only after the tool finished:\n{}",
        seen.join("\n")
    );

    skeleton.wait_for("All done.");
    skeleton.wait_for("[brain] turn complete");
    skeleton.quit();

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "expected exactly two provider requests");
    let second = roles_and_content(&requests[1]);

    let calls_at = second
        .iter()
        .position(|(role, _, has_calls)| role == "assistant" && *has_calls)
        .expect("assistant tool_calls message in second request");
    assert_eq!(
        second[calls_at + 1].0,
        "tool",
        "tool result must immediately follow its tool_calls message; \
         nothing may split the exchange: {second:?}"
    );
    let piggy_at = second
        .iter()
        .position(|(role, content, _)| role == "user" && content == "by the way, another thought")
        .expect("piggybacked mid-tool user message in second request");
    assert!(
        piggy_at > calls_at + 1,
        "piggybacked activity must ride after the tool exchange: {second:?}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Cancellation during an in-flight *provider request* (not a tool call):
/// the request resolves cancelled, the turn finalizes cancelled, no
/// follow-on work starts, and the session stays usable.
#[test]
fn cancel_during_provider_request_drains_and_session_continues() {
    let tmp = setup("cancel-request");
    let workdir = tmp.join("workspace");

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            // The fake provider holds the first request open long enough
            // to cancel it mid-flight.
            { "delay_ms": 3000, "text": "too late, you cancelled me" },
            { "text": "Recovered fine." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please think about something slowly");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[brain] request 0 in flight");
    // Cancel only once the provider has demonstrably received the request
    // (it logs before its scripted delay), so the cancel genuinely hits an
    // in-flight request rather than racing the send.
    let deadline = Instant::now() + Duration::from_secs(5);
    while provider.requests().is_empty() {
        assert!(
            Instant::now() < deadline,
            "provider never received the request"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    skeleton.send("/cancel");
    skeleton.wait_for("[brain] request cancelled");
    skeleton.wait_for("[brain] turn cancelled");

    // Finalized: no follow-on work after the drain.
    std::thread::sleep(Duration::from_millis(300));
    skeleton.drain_seen();
    let seen: Vec<String> = skeleton.seen.clone();
    assert!(
        !seen.iter().any(|l| l.contains("request 1 in flight")),
        "a cancelled request must not be followed by new work:\n{}",
        seen.join("\n")
    );

    // The session is still usable after the cancellation. (The fake
    // provider is single-threaded and still sleeping out the abandoned
    // request; the 10s wait_for budget absorbs that.)
    skeleton.send("still with me?");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("Recovered fine.");
    skeleton.wait_for("[brain] turn complete");
    skeleton.quit();

    std::fs::remove_dir_all(&tmp).ok();
}

/// Cancelling a bash tool call kills the whole process tree, not just the
/// shell: a backgrounded descendant must not survive the drain.
#[test]
fn cancel_kills_descendant_processes() {
    let tmp = setup("descendants");
    let workdir = tmp.join("workspace");

    // The scripted command records its own PIDs (the shell's and its
    // backgrounded descendant's) into the test's fixture file, so the
    // assertion targets exactly the processes this test owns.
    let pid_file = workdir.join("pids.txt");
    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "bash", "arguments": {
                "command": "sleep 600 & { echo $!; echo $$; } > pids.txt; wait" } } },
            { "text": "Recovered fine." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the background thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: bash");
    // The pid file appearing proves the descendant has been forked.
    let pids = recorded_pids(&pid_file);
    assert_eq!(pids.len(), 2, "expected sleep + shell pids: {pids:?}");
    skeleton.send("/cancel");
    skeleton.wait_for("[limb] tool cancelled");
    skeleton.wait_for("[brain] turn cancelled");

    // The shell AND its backgrounded descendant are gone.
    wait_pids_dead(&pids);

    skeleton.quit();
    std::fs::remove_dir_all(&tmp).ok();
}

/// A tool call that *completes* must not leak descendants either: the
/// process group was created for the call, so nothing in it outlives the
/// call's resolution — even when the shell backgrounds a child and exits
/// successfully.
#[test]
fn completed_tool_does_not_leak_descendants() {
    let tmp = setup("completed-leak");
    let workdir = tmp.join("workspace");

    let pid_file = workdir.join("pids.txt");
    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            // Redirected so the pipes close and the shell exits at once,
            // leaving the backgrounded sleep as a would-be orphan.
            { "tool_call": { "name": "bash", "arguments": {
                "command": "sleep 600 > /dev/null 2>&1 & echo $! > pids.txt" } } },
            { "text": "All done." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the background thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("All done.");
    skeleton.wait_for("[brain] turn complete");

    // The tool resolved Ok; its whole group must already be dead — before
    // any shutdown, because completion (not quit) ends the operation.
    let pids = recorded_pids(&pid_file);
    wait_pids_dead(&pids);

    skeleton.quit();
    std::fs::remove_dir_all(&tmp).ok();
}

/// Cancelling a non-bash tool blocked on I/O (read_file on a FIFO with no
/// writer) still finalizes the turn and leaves the session usable.
#[test]
fn cancel_unblocks_a_blocked_read_file() {
    let tmp = setup("fifo");
    let workdir = tmp.join("workspace");
    let fifo = workdir.join("pipe.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo")
            .success(),
        "mkfifo failed"
    );

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "read_file", "arguments": { "path": "pipe.fifo" } } },
            { "text": "Recovered fine." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please read the pipe");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: read_file");
    std::thread::sleep(Duration::from_millis(300));
    skeleton.send("/cancel");
    skeleton.wait_for("[limb] tool cancelled");
    skeleton.wait_for("[brain] turn cancelled");

    // Unblock the leaked reader (a spike-accepted limitation: the blocked
    // open lingers on the blocking pool until the FIFO gets a writer) so
    // shutdown isn't held hostage by it.
    std::fs::write(&fifo, b"unblock\n").expect("open fifo write end");

    // The session is still usable after cancelling a wedged tool.
    skeleton.send("still with me?");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("Recovered fine.");
    skeleton.wait_for("[brain] turn complete");
    skeleton.quit();

    std::fs::remove_dir_all(&tmp).ok();
}

/// /quit while a tool call is in flight: the drain cancels and finalizes
/// it (recorded outcomes, dead process tree), and the process exits
/// cleanly.
#[test]
fn quit_during_tool_call_drains_and_exits_cleanly() {
    let tmp = setup("quit-drain");
    let workdir = tmp.join("workspace");

    let pid_file = workdir.join("pids.txt");
    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "bash", "arguments": {
                "command": "echo $$ > pids.txt; exec sleep 600" } } }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the slow thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: bash");
    let pids = recorded_pids(&pid_file);
    skeleton.send("/quit");
    skeleton.wait_for("[limb] tool cancelled");
    skeleton.wait_for("[brain] turn cancelled");
    skeleton.wait_for("[brain] session closed");
    skeleton.wait_exit();

    wait_pids_dead(&pids);
    std::fs::remove_dir_all(&tmp).ok();
}

/// /rebuild recomputes the model view from the log without losing the
/// conversation: the next request still carries everything.
#[test]
fn rebuild_preserves_the_model_view() {
    let tmp = setup("rebuild");
    let workdir = tmp.join("workspace");

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "text": "First answer." },
            { "text": "Second answer." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("first question");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("First answer.");
    skeleton.send("/rebuild");
    skeleton.wait_for("[brain] context rebuilt");
    skeleton.send("second question");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("Second answer.");
    skeleton.quit();

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "expected exactly two provider requests");
    let second = roles_and_content(&requests[1]);
    for expected in [
        ("user", "first question"),
        ("assistant", "First answer."),
        ("user", "second question"),
    ] {
        assert!(
            second
                .iter()
                .any(|(role, content, _)| role == expected.0 && content == expected.1),
            "rebuild lost {expected:?} from the model view: {second:?}"
        );
    }

    std::fs::remove_dir_all(&tmp).ok();
}

/// /dump: the model view as markdown — wire order, verbatim content, with
/// everything the model cannot see in HTML comments (invisible events,
/// piggyback annotations). Asserted through the public surface: $EDITOR
/// (cat) streams the dump to the face's terminal.
#[test]
fn dump_shows_model_view_with_invisible_facts() {
    let tmp = setup("dump");
    let workdir = tmp.join("workspace");

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "bash", "arguments": { "command": "sleep 1; echo done" } } },
            { "text": "All done." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the slow thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: bash");
    skeleton.send("by the way, another thought");
    skeleton.wait_for("[face] staged user message");
    skeleton.wait_for("All done.");
    skeleton.wait_for("[brain] turn complete");

    skeleton.send("/dump");
    let seen = skeleton.wait_for("[face] returned from dump").to_vec();
    skeleton.quit();

    let dump_start = seen
        .iter()
        .position(|l| l.contains("# walking-skeleton context dump"))
        .expect("dump header in face output");
    let dump = seen[dump_start..].join("\n");

    // The model view, verbatim, in wire order.
    assert!(
        dump.contains("## system"),
        "system section missing:\n{dump}"
    );
    assert!(
        dump.contains("please run the slow thing"),
        "user message missing:\n{dump}"
    );
    assert!(
        dump.contains("```json tool_calls"),
        "tool calls not rendered visibly (the model sees them):\n{dump}"
    );
    assert!(
        dump.contains("## tool ("),
        "tool result section missing:\n{dump}"
    );

    // Everything the model sees must be in the dump: the request's tools
    // field (schemas) and the environment contributions composed into the
    // system prompt. The dump renders the same request_parts projection
    // the request builder sends, so a miss here would be a shared-code
    // bug, not a rendering gap.
    assert!(
        dump.contains("## tools") && dump.contains("\"name\": \"bash\""),
        "tool schemas missing from dump (the model sees the tools field):\n{dump}"
    );
    assert!(
        dump.contains("[environment]") && dump.contains("- model: fake-model"),
        "environment contributions missing from the system prompt:\n{dump}"
    );
    assert!(
        dump.contains("All done."),
        "assistant text missing:\n{dump}"
    );

    // What the model can't see is present, but only as HTML comments.
    assert!(
        dump.contains("<!-- seq") && dump.contains("\"type\":\"turn_end\""),
        "invisible events should appear as comments:\n{dump}"
    );

    // The piggybacked message is shown at its wire position (after the
    // tool exchange) with its arrival annotated.
    let piggy_note = dump
        .find("arrived while a tool exchange was open")
        .expect("piggyback annotation missing");
    let tool_section = dump.find("## tool (").unwrap();
    let piggy_text = dump
        .find("by the way, another thought")
        .expect("piggybacked message missing from dump");
    assert!(
        tool_section < piggy_note && piggy_note < piggy_text,
        "piggybacked message should render after the tool exchange, \
         annotated:\n{dump}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

/// Exit condition (c): a cancel during an in-flight tool call drains and
/// finalizes to a recorded cancelled outcome, visible at the face, with
/// the session usable afterwards.
#[test]
fn cancel_during_tool_call_drains_and_session_continues() {
    let tmp = setup("cancel");
    let workdir = tmp.join("workspace");

    let provider = FakeProvider::start(
        &tmp,
        serde_json::json!([
            { "tool_call": { "name": "bash", "arguments": { "command": "sleep 30; echo never" } } },
            { "text": "Recovered fine." }
        ]),
    );
    let mut skeleton = Skeleton::start(&workdir, &provider.addr, &tmp.join("session.jsonl"));

    skeleton.send("please run the very slow thing");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("[limb] tool call: bash");

    let cancel_started = Instant::now();
    skeleton.send("/cancel");
    skeleton.wait_for("[limb] tool cancelled");
    skeleton.wait_for("[brain] turn cancelled");
    assert!(
        cancel_started.elapsed() < Duration::from_secs(5),
        "drain must not wait out the 30s child process"
    );

    // Finalized, not merely stopped: no follow-up request was sent.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        provider.requests().len(),
        1,
        "a cancelled turn must not send a follow-up request"
    );

    // The session is still usable after the cancellation.
    skeleton.send("still with me?");
    skeleton.wait_for("[face] staged user message");
    skeleton.send("/end");
    skeleton.wait_for("Recovered fine.");
    skeleton.wait_for("[brain] turn complete");
    skeleton.quit();

    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "expected exactly two provider requests");
    let second = roles_and_content(&requests[1]);
    let calls_at = second
        .iter()
        .position(|(role, _, has_calls)| role == "assistant" && *has_calls)
        .expect("assistant tool_calls message in second request");
    assert_eq!(
        second[calls_at + 1].0,
        "tool",
        "cancelled tool call still needs its tool result on the wire: {second:?}"
    );
    assert!(
        second[calls_at + 1].1.contains("[tool cancelled]"),
        "tool result must record the cancellation: {second:?}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
