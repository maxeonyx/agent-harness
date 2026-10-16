//! Drives the workspace limb through a realistic sequence of calls in a
//! scratch directory, playing the brain: two agents' contexts, a stale
//! write refused, a command's write noticed, truncated output, a process
//! tree cancelled, and a crash answered by re-sending calls. See
//! `demos/limb-workspace`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use std::{env, fs, thread};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::environment::{FromLimb, ToLimb};
use harness_core::id::{CallId, LimbId, TaskId, ValueId};
use harness_core::journal::{Batch, Fact};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::value::Bytes;
use harness_limb_workspace::{Config, Settings, Workspace};
use serde_json::{Value, json};

const LIMB: LimbId = LimbId(1);

fn main() {
    let scratch = env::current_dir().unwrap().join("demo-scratch.ignore");
    if env::args().nth(1).as_deref() == Some("crash") {
        return crash(&scratch);
    }
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(scratch.join("work")).unwrap();
    println!("scratch directory: {}\n", scratch.display());

    let mut brain = Brain::open(&scratch);

    title("The tools, as a context's root declares them");
    brain.ws.handle(ToLimb::Describe);
    let Some(FromLimb::Described(description)) = brain.next() else {
        panic!("expected a description");
    };
    for tool in &description.tools {
        let definition: Value = serde_json::from_str(&tool.definition).unwrap();
        let required = &definition["input_schema"]["required"];
        println!("  {:<6} required={required}", tool.name);
    }
    println!(
        "\n  bash, exactly as rendered into the root:\n  {}",
        description.tools[0].definition
    );
    let text = &description.instructions[0].text;
    println!(
        "\n  the instructions beside them begin: {:?} ...",
        &text[..text.find(". ").unwrap() + 1]
    );

    let mut a = Context::new("A", 1);
    let mut b = Context::new("B", 2);

    title("A writes a file, B reads it and edits it");
    brain.call(&mut a, "write", json!({ "path": "src/lib.rs", "content": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n" }));
    brain.call(&mut b, "read", json!({ "path": "src/lib.rs" }));
    brain.call(
        &mut b,
        "edit",
        json!({ "path": "src/lib.rs", "old": "a: i32, b: i32", "new": "a: i64, b: i64" }),
    );
    brain.call(&mut b, "grep", json!({ "pattern": "fn \\w+" }));

    title("A edits on its old view: refused, untouched; it reads again and retries");
    brain.call(
        &mut a,
        "edit",
        json!({ "path": "src/lib.rs", "old": "-> i32", "new": "-> i64" }),
    );
    brain.call(&mut a, "read", json!({ "path": "src/lib.rs" }));
    brain.call(
        &mut a,
        "edit",
        json!({ "path": "src/lib.rs", "old": "-> i32", "new": "-> i64" }),
    );

    title(
        "A command changes a watched file: reported after the call, and B's next edit is refused",
    );
    let key = ResourceKey {
        limb: LIMB,
        kind: "file".into(),
        name: "src/lib.rs".into(),
    };
    println!(
        "\n  → Watch src/lib.rs, giving the versions A and B have seen (the limb reports only a version that is neither)"
    );
    let known = [&a, &b]
        .iter()
        .filter_map(|c| c.knows.get(&key))
        .map(|version| Observation {
            resource: key.clone(),
            version: *version,
        })
        .collect();
    brain.ws.handle(ToLimb::Watch { known });
    brain.drain();
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "sed -i 's/a + b/b + a/' src/lib.rs && cat src/lib.rs", "grep": "" }),
    );
    brain.call(
        &mut b,
        "edit",
        json!({ "path": "src/lib.rs", "old": "b + a", "new": "a + b" }),
    );

    title("Long output, cut to head + tail + the lines matching the agent's grep");
    let tests = "for i in $(seq 1 2000); do if [ $i = 1234 ]; then echo \"test parser::nested_quotes ... FAILED\"; else echo \"test case_$i ... ok\"; fi; done; exit 101";
    let full = brain.call(
        &mut a,
        "bash",
        json!({ "command": tests, "grep": "FAILED" }),
    );
    if let Some(id) = full.and_then(|o| o.full) {
        println!(
            "  the full output came with the settlement, as value {id} ({} bytes)",
            brain.values[&id].len()
        );
    }
    println!("\n  with an empty grep, only the head and tail:");
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "seq 1 100", "grep": "" }),
    );

    title("A long-running command is interrupted and its whole process tree reaped");
    let pids = scratch.join("work/tree.pids");
    let tree = "echo $$ > tree.pids; sleep 300 & echo $! >> tree.pids; (sleep 300 & echo $! >> tree.pids; wait) & sleep 0.2; echo started a tree; wait";
    let call = brain.dispatch(&mut a, "bash", json!({ "command": tree, "grep": "" }));
    loop {
        match brain.next() {
            Some(FromLimb::Live(Live::Call { chunk, .. })) => {
                println!("  live: {chunk:?}");
                if chunk.contains("started") {
                    break;
                }
            }
            other => panic!("expected live output, got {other:?}"),
        }
    }
    let pids = read_pids(&pids);
    println!("  running: {}", alive(&pids));
    println!("  → Stop");
    brain.ws.handle(ToLimb::Stop { call });
    brain.settle(&mut a, call);
    println!("  afterwards: {}", after(&pids));

    title(
        "The limb crashes mid-command; a new instance reaps the orphan and answers the calls sent again",
    );
    let first = brain.sent[&CallId(1)].clone();
    drop(brain);
    let status = std::process::Command::new(env::current_exe().unwrap())
        .arg("crash")
        .status()
        .unwrap();
    println!("  the crashed limb process exited with {status}");
    let orphan = read_pids(&scratch.join("work/crash.pid"));
    println!("  left behind: {}", alive(&orphan));
    let mut brain = Brain::open(&scratch);
    println!("  after the new instance opened: {}", after(&orphan));
    let crashed = Brain::run(&mut Context::new("A", 1), "bash", crash_command(), 900).1;
    for (why, message) in [
        ("again: A's first write, settled before the crash", first),
        ("again: the command the crash cut off", crashed),
        (
            "for a call this limb never received",
            ToLimb::Stop { call: CallId(901) },
        ),
    ] {
        let call = match &message {
            ToLimb::Run { id, .. } => *id,
            ToLimb::Stop { call } => *call,
            _ => unreachable!(),
        };
        println!("\n  → {} call {} {why}", verb(&message), call.0);
        brain.ws.handle(message);
        let Some(FromLimb::Facts(batch)) = brain.next() else {
            panic!("expected facts");
        };
        let outcome = brain.facts(batch).remove(&call).expect("a settlement");
        println!("    {}", kind(&outcome));
    }
}

fn verb(message: &ToLimb) -> &'static str {
    match message {
        ToLimb::Run { .. } => "Run",
        _ => "Stop",
    }
}

fn crash_command() -> Value {
    json!({ "command": "echo $$ > crash.pid; echo running; sleep 300", "grep": "" })
}

/// The child process for the crash scene: start a command and die without
/// any cleanup.
fn crash(scratch: &Path) {
    let mut brain = Brain::open(scratch);
    let mut c = Context::new("A", 1);
    brain.next_call = 900;
    brain.dispatch(&mut c, "bash", crash_command());
    let pid = scratch.join("work/crash.pid");
    while !pid.exists() {
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(100));
    std::process::exit(9);
}

struct Brain {
    ws: Workspace,
    rx: Receiver<FromLimb>,
    next_call: u128,
    /// Every value the limb has sent, as the brain's store would hold them.
    values: HashMap<ValueId, Vec<u8>>,
    /// Every call sent, to send again after the crash.
    sent: HashMap<CallId, ToLimb>,
}

impl Brain {
    fn open(scratch: &Path) -> Brain {
        let (tx, rx) = mpsc::channel();
        let config = Config {
            limb: LIMB,
            user: false,
            settings: Settings {
                dir: scratch.join("work"),
                clone: None,
            },
            state: scratch.join("state"),
        };
        let ws = Workspace::open(config, tx).unwrap();
        let Ok(FromLimb::Hello { instance }) = rx.recv() else {
            panic!("expected Hello first");
        };
        println!("  [limb instance {:08x}]", instance.0 as u32);
        Brain {
            ws,
            rx,
            next_call: 1,
            values: HashMap::new(),
            sent: HashMap::new(),
        }
    }

    /// The Run message for one call by `context`.
    fn run(context: &mut Context, tool: &str, input: Value, id: u128) -> (CallId, ToLimb) {
        let id = CallId(id);
        let input = Bytes::from(input.to_string());
        let expects = context
            .knows
            .iter()
            .map(|(resource, version)| Observation {
                resource: resource.clone(),
                version: *version,
            })
            .collect();
        let call = Call {
            caller: Caller::Agent {
                task: context.task,
                tool_use: None,
            },
            tool: tool.into(),
            input: input.id(),
            expects,
        };
        (id, ToLimb::Run { id, call, input })
    }

    /// Keep a batch's values, print its changes, and return its settlements.
    fn facts(&mut self, batch: Batch) -> HashMap<CallId, Outcome> {
        for value in batch.values {
            self.values.insert(value.id(), value.0);
        }
        let mut settled = HashMap::new();
        for fact in batch.facts {
            match fact {
                Fact::Call(call, CallEvent::Settled(outcome)) => {
                    settled.insert(call, outcome);
                }
                Fact::Change(change) => print_change(&change),
                other => println!("    {other:?}"),
            }
        }
        settled
    }

    fn next(&self) -> Option<FromLimb> {
        self.rx.recv_timeout(Duration::from_secs(10)).ok()
    }

    fn drain(&mut self) {
        while let Ok(message) = self.rx.recv_timeout(Duration::from_millis(50)) {
            match message {
                FromLimb::Facts(batch) => {
                    self.facts(batch);
                }
                other => println!("    {other:?}"),
            }
        }
    }

    fn dispatch(&mut self, context: &mut Context, tool: &str, input: Value) -> CallId {
        println!("\n  {} → {tool} {input}", context.name);
        let (call, message) = Brain::run(context, tool, input, self.next_call);
        self.next_call += 1;
        self.sent.insert(call, message.clone());
        self.ws.handle(message);
        call
    }

    fn call(&mut self, context: &mut Context, tool: &str, input: Value) -> Option<Output> {
        let call = self.dispatch(context, tool, input);
        self.settle(context, call)
    }

    fn settle(&mut self, context: &mut Context, call: CallId) -> Option<Output> {
        loop {
            match self.next() {
                Some(FromLimb::Facts(batch)) => {
                    if let Some(outcome) = self.facts(batch).remove(&call) {
                        return self.show(context, outcome);
                    }
                }
                Some(FromLimb::Live(_)) => {}
                Some(other) => println!("    {other:?}"),
                None => panic!("no settlement"),
            }
        }
    }

    fn show(&self, context: &mut Context, outcome: Outcome) -> Option<Output> {
        show(context, outcome, &self.values)
    }
}

fn print_change(change: &Change) {
    let by = match change.by {
        Some(call) => format!("by call {}", call.0),
        None => "from outside the harness".into(),
    };
    println!(
        "    Change: {} {} is now {}, {by}",
        change.resource.kind,
        change.resource.name,
        version(&change.version)
    );
}

fn show(
    context: &mut Context,
    outcome: Outcome,
    values: &HashMap<ValueId, Vec<u8>>,
) -> Option<Output> {
    println!("    {}", kind(&outcome));
    let output = match outcome {
        Outcome::Completed(o) | Outcome::Failed(o) => Some(o),
        Outcome::Interrupted { partial } => partial,
        Outcome::Refused { current } => {
            for o in &current {
                let had = context
                    .knows
                    .get(&o.resource)
                    .map_or("nothing".into(), version);
                println!(
                    "    {} had seen {} at {had}; it is at {}",
                    context.name,
                    o.resource.name,
                    version(&o.version)
                );
            }
            None
        }
        Outcome::Unknown => None,
    };
    if let Some(output) = &output {
        let content: Value = serde_json::from_slice(&values[&output.model]).unwrap();
        for line in content[0]["text"].as_str().unwrap().lines() {
            println!("    │ {line}");
        }
        for o in &output.observes {
            if o.resource.kind == "file" {
                println!(
                    "    {} now knows {} at {}",
                    context.name,
                    o.resource.name,
                    version(&o.version)
                );
            }
            context.knows.insert(o.resource.clone(), o.version);
        }
    }
    output
}

fn kind(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Completed(_) => "Completed".into(),
        Outcome::Failed(_) => "Failed".into(),
        Outcome::Refused { .. } => "Refused (no effect)".into(),
        Outcome::Interrupted { .. } => "Interrupted".into(),
        Outcome::Unknown => "Unknown".into(),
    }
}

/// One agent's context, as far as the brain tracks it here: the versions it
/// has observed, which become every call's `expects`.
struct Context {
    name: &'static str,
    task: TaskId,
    knows: BTreeMap<ResourceKey, Version>,
}

impl Context {
    fn new(name: &'static str, task: u128) -> Context {
        Context {
            name,
            task: TaskId(task),
            knows: BTreeMap::new(),
        }
    }
}

fn version(version: &Version) -> String {
    match version {
        Version::Is(digest) => digest.0[..4].iter().map(|b| format!("{b:02x}")).collect(),
        Version::Gone => "gone".into(),
    }
}

fn title(text: &str) {
    println!("\n━━ {text}");
}

fn read_pids(path: &PathBuf) -> Vec<u32> {
    fs::read_to_string(path)
        .unwrap()
        .split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect()
}

/// Whether each process is still running. A process killed a moment ago
/// may linger as a zombie until its new parent reaps it; it runs nothing.
fn running(pid: &u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|s| !s[s.rfind(')').unwrap() + 1..].trim_start().starts_with('Z'))
}

fn alive(pids: &[u32]) -> String {
    pids.iter()
        .map(|p| format!("{p} {}", if running(p) { "running" } else { "gone" }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// [`alive`], once any process just killed has had a moment to exit.
fn after(pids: &[u32]) -> String {
    for _ in 0..50 {
        if !pids.iter().any(running) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    alive(pids)
}
