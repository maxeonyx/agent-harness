//! Drives the workspace limb through a realistic sequence of calls in a
//! scratch directory, playing the brain: two agents' contexts, a stale
//! write refused, a command's write noticed, truncated output, a process
//! tree stopped, and the limb killed mid-command. See `demos/limb-workspace`.
//!
//! The brain here keeps the limb's rows itself, a `BTreeMap` it ships with a
//! [`Publisher`], and takes the limb's records by advancing its standing.
//! The limb runs in this process, but for the instance that is killed: that
//! one is this executable run as `demo crash`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use std::{env, fs, thread};

use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::environment::{Description, LimbDown, LimbKey, LimbRow, LimbUp};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{CallId, EventId, LimbId, TaskId, Time, Timestamp, ToolUseId, ValueId};
use harness_core::live::Live;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::value::Bytes;
use harness_limb_workspace::{Config, Kind, Settings, Workspace, now};
use harness_substrate::{Publisher, ReplicaId, Standing, Up, View, What};
use serde_json::{Value, json};

/// The replica the brain's ids are minted at.
const BRAIN: ReplicaId = ReplicaId(0xb7a1);

/// The limb, as the brain's first event.
const LIMB: LimbId = LimbId(EventId {
    at: Time {
        replica: BRAIN,
        n: 1,
    },
    i: 0,
});

fn main() {
    let scratch = env::current_dir().unwrap().join("demo-scratch.ignore");
    if env::args().nth(1).as_deref() == Some("crash") {
        return crash(&scratch);
    }
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(scratch.join("work")).unwrap();
    println!("scratch directory: {}\n", scratch.display());

    let mut brain = Brain::new(&scratch);
    brain.connect();

    title("The tools, as a context's root declares them");
    let description = brain.description.clone().expect("described");
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

    let mut a = Context::new("A", TaskId(brain.mint()));
    let mut b = Context::new("B", TaskId(brain.mint()));

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
        "A command changes a watched file: reported in its settlement, and B's next edit is refused",
    );
    let key = ResourceKey {
        limb: LIMB,
        kind: "file".into(),
        name: "src/lib.rs".into(),
    };
    println!(
        "\n  → Watch row: src/lib.rs, at the versions A and B have seen (the limb reports only a version that is neither)"
    );
    let known = [&a, &b]
        .iter()
        .filter_map(|c| c.knows.get(&key))
        .map(|version| Observation {
            resource: key.clone(),
            version: *version,
        })
        .collect();
    brain.rows.insert(LimbKey::Watch, LimbRow::Watch { known });
    brain.publish();
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
    println!(
        "\n  A made that change itself, so A's context knows it, and A's own edit goes through:"
    );
    brain.call(
        &mut a,
        "edit",
        json!({ "path": "src/lib.rs", "old": "b + a", "new": "a + b" }),
    );

    title(
        "read: whole numbered lines up to 64 KiB, a long line never cut in its middle, an image as an image",
    );
    let long = format!("short\n{}END\nafter\n", "x".repeat(300));
    fs::write(scratch.join("work/long.txt"), &long).unwrap();
    brain.call(&mut a, "read", json!({ "path": "long.txt" }));
    let many: String = (1..=20_000).map(|n| format!("line {n}\n")).collect();
    fs::write(scratch.join("work/many.txt"), many).unwrap();
    brain.call(
        &mut a,
        "read",
        json!({ "path": "many.txt", "offset": 9000, "limit": 3 }),
    );
    println!(
        "\n  the whole of a 20 000-line file, stopped at the budget (its last two lines shown here):"
    );
    let call = brain.dispatch(&mut a, "read", json!({ "path": "many.txt" }));
    let outcome = loop {
        if let Some(outcome) = brain.settled.remove(&call) {
            break outcome;
        }
        brain.take();
    };
    let output = outcome.output().expect("the read completed");
    let content: Value = serde_json::from_slice(&brain.values[&output.model]).unwrap();
    let text = content[0]["text"].as_str().unwrap();
    text.lines()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .iter()
        .rev()
        .for_each(|l| println!("    │ {l}"));
    let png = [
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R', 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0x0A, b'I', b'D',
        b'A', b'T', 0x78, 0x9C, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0D, 0x0A, 0x2D, 0xB4, 0, 0, 0, 0,
        b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
    ];
    fs::write(scratch.join("work/dot.png"), png).unwrap();
    brain.call(&mut a, "read", json!({ "path": "dot.png" }));

    title("Long output, cut to head + tail + the lines matching the agent's grep");
    let tests = "for i in $(seq 1 2000); do if [ $i = 1234 ]; then echo \"test parser::nested_quotes ... FAILED\"; else echo \"test case_$i ... ok\"; fi; done; exit 101";
    let full = brain.call(
        &mut a,
        "bash",
        json!({ "command": tests, "grep": "FAILED" }),
    );
    if let Some(id) = full.and_then(|o| o.full) {
        println!(
            "  the full output came in the settlement's record, as value {id} ({} bytes)",
            brain.values[&id].len()
        );
    }
    println!("\n  with an empty grep, only the head and tail:");
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "seq 1 100", "grep": "" }),
    );

    title("A long-running command is stopped and its whole process tree ended");
    let pids = scratch.join("work/tree.pids");
    let tree = "echo $$ > tree.pids; sleep 300 & echo $! >> tree.pids; (sleep 300 & echo $! >> tree.pids; wait) & sleep 0.2; echo started a tree; wait";
    let call = brain.dispatch(&mut a, "bash", json!({ "command": tree, "grep": "" }));
    brain.live_until("started");
    let pids = read_pids(&pids);
    println!("  running: {}", alive(&pids));
    println!("  → the call's row now says stopping, by 2 s from now");
    let Some(LimbRow::Call { stopping, .. }) = brain.rows.get_mut(&LimbKey::Call(call)) else {
        unreachable!("the call is unsettled");
    };
    *stopping = Some(Timestamp(now().0 + 2000));
    brain.publish();
    brain.settle(&mut a, call);
    println!("  afterwards: {}", after(&pids));

    title("A server started with setsid outlives its command, until stopped by its pid");
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "setsid sleep 300 > /dev/null 2>&1 & echo $! > server.pid; echo started $(cat server.pid)", "grep": "" }),
    );
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "kill -0 $(cat server.pid) && echo still running in the next call", "grep": "" }),
    );
    brain.call(
        &mut a,
        "bash",
        json!({ "command": "kill $(cat server.pid); sleep 0.2; ps -o stat= -p $(cat server.pid) | grep -qv Z && echo running || echo stopped", "grep": "" }),
    );

    title("The limb is killed mid-command; the next instance ends what it left and settles");
    brain.disconnect();
    println!("  (the in-process instance closed; the next calls go to one in a child process)");
    let once = brain.dispatch(
        &mut a,
        "bash",
        json!({ "command": "echo ran >> once.log", "grep": "" }),
    );
    let cut = brain.dispatch(
        &mut a,
        "bash",
        json!({ "command": "echo $$ > crash.pid; echo running; sleep 300", "grep": "" }),
    );
    let orphan = scratch.join("work/crash.pid");
    fs::write(
        scratch.join("rows.json"),
        serde_json::to_string(&(brain.standing.upto, Vec::from_iter(brain.rows.clone()))).unwrap(),
    )
    .unwrap();
    println!("\n  a limb instance in a child process takes these rows, and is killed mid-command");
    let mut child = Command::new(env::current_exe().unwrap())
        .arg("crash")
        .spawn()
        .unwrap();
    while !orphan.exists() || !scratch.join("work/once.log").exists() {
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(200));
    child.kill().unwrap();
    println!(
        "  the brain kills it before taking what it said: {}",
        child.wait().unwrap()
    );
    let orphan = read_pids(&orphan);
    println!("  left behind: {}", alive(&orphan));
    brain.connect();
    println!("  after the new instance opened: {}", after(&orphan));
    for (call, why) in [
        (once, "settled before the kill"),
        (cut, "the command the kill cut off"),
    ] {
        println!("\n  call {} ({why}):", call.0.at.n);
        brain.settle(&mut a, call);
    }
    let ran = fs::read_to_string(scratch.join("work/once.log")).unwrap();
    println!(
        "\n  once.log has {} line: the settled call did not run again",
        ran.lines().count()
    );
}

/// The limb instance that is killed: it takes the brain's rows and runs
/// their calls until the brain kills it.
fn crash(scratch: &Path) {
    let (upto, rows): (u64, Vec<(LimbKey, LimbRow)>) =
        serde_json::from_slice(&fs::read(scratch.join("rows.json")).unwrap()).unwrap();
    let workspace = Workspace::open(config(scratch), Box::new(|_| {})).unwrap();
    let view = View {
        as_of: Time {
            replica: BRAIN,
            n: u64::MAX,
        },
        standing: Standing {
            upto,
            refused: BTreeMap::new(),
        },
        rows: rows.into_iter().collect(),
    };
    workspace.apply(LimbDown::View(view)).unwrap();
    loop {
        thread::park();
    }
}

fn config(scratch: &Path) -> Config {
    Config {
        limb: LIMB,
        kind: Kind::Workspace,
        settings: Settings {
            dir: scratch.join("work"),
            clone: None,
            skills: None,
        },
        state: scratch.join("state"),
    }
}

/// The brain's side of one limb: the rows it wants of it, its standing, and
/// the link to whichever instance is running.
struct Brain {
    scratch: PathBuf,
    /// The brain's clock: each id minted and each view shipped is a time.
    n: u64,
    rows: BTreeMap<LimbKey, LimbRow>,
    standing: Standing<Refusal>,
    link: Option<Link>,
    /// The instance last connected: its state, and so its replica, outlive
    /// its process.
    instance: Option<ReplicaId>,
    /// Every value the limb has sent, as the brain's store would hold them.
    values: HashMap<ValueId, Vec<u8>>,
    /// Settlements taken and not yet shown.
    settled: HashMap<CallId, Outcome>,
    description: Option<Description>,
}

/// One running instance and what the brain has shipped it.
struct Link {
    workspace: Workspace,
    up: Receiver<LimbUp>,
    publisher: Publisher<LimbKey, LimbRow, Refusal>,
}

impl Brain {
    fn new(scratch: &Path) -> Brain {
        Brain {
            scratch: scratch.into(),
            n: 1,
            rows: BTreeMap::new(),
            standing: Standing {
                upto: 0,
                refused: BTreeMap::new(),
            },
            link: None,
            instance: None,
            values: HashMap::new(),
            settled: HashMap::new(),
            description: None,
        }
    }

    /// The next time of the brain's clock.
    fn tick(&mut self) -> Time {
        self.n += 1;
        Time {
            replica: BRAIN,
            n: self.n,
        }
    }

    /// A new id: the event of the brain's next time.
    fn mint(&mut self) -> EventId {
        self.tick().event(0)
    }

    /// Open a limb instance on the state directory and ship it a whole
    /// view; then wait for it to describe itself.
    fn connect(&mut self) {
        let (tx, up) = mpsc::channel();
        let out = Box::new(move |up| {
            let _ = tx.send(up);
        });
        let workspace = Workspace::open(config(&self.scratch), out).unwrap();
        println!(
            "  [a limb instance opened, at replica {}; the brain holds its records up to {}]",
            workspace.replica(),
            self.standing.upto
        );
        self.instance = Some(workspace.replica());
        self.link = Some(Link {
            workspace,
            up,
            publisher: Publisher::default(),
        });
        self.description = None;
        self.publish();
        while self.description.is_none() {
            self.take();
        }
    }

    /// Close the link: the instance stops its calls and is gone.
    fn disconnect(&mut self) {
        self.drain();
        self.link = None;
    }

    /// Ship the rows, as of now: a whole view to a new link, then steps.
    /// What the limb ships back first, it ships on its link.
    fn publish(&mut self) {
        let view = View {
            as_of: self.tick(),
            standing: self.standing.clone(),
            rows: self.rows.clone(),
        };
        let Some(link) = &mut self.link else {
            return;
        };
        let Some(down) = link.publisher.publish(view) else {
            return;
        };
        let unsent = link.workspace.apply(down).expect("the limb's log is whole");
        for record in unsent {
            self.include(record);
        }
    }

    fn next(&mut self, wait: Duration) -> Option<LimbUp> {
        self.link.as_ref().unwrap().up.recv_timeout(wait).ok()
    }

    /// Take the next record the limb ships.
    fn take(&mut self) {
        loop {
            match self
                .next(Duration::from_secs(10))
                .expect("the limb went quiet")
            {
                Up::Record(record) => return self.include(record),
                Up::Live(_) => {}
            }
        }
    }

    /// Include one of the limb's records: advance the standing, keep its
    /// values and settlements, drop the rows of the calls it settles, and
    /// add each version it reports to what the watch row knows.
    fn include(&mut self, record: harness_core::schema::Record) {
        if record.at.n <= self.standing.upto {
            return;
        }
        self.standing.upto = record.at.n;
        let What::Said(batch) = record.what else {
            return;
        };
        let Batch { values, facts } = batch;
        println!(
            "    ← record {}: {}",
            record.at.n,
            summary(&facts, values.len())
        );
        for value in values {
            self.values.insert(value.id(), value.0);
        }
        for fact in facts {
            match fact {
                Fact::Call(call, CallEvent::Settled(outcome)) => {
                    if let Some(LimbRow::Call { call: c, .. }) =
                        self.rows.remove(&LimbKey::Call(call))
                    {
                        self.rows.remove(&LimbKey::Value(c.input));
                    }
                    self.settled.insert(call, outcome);
                }
                Fact::Change(change) => {
                    print_change(&change);
                    if let Some(LimbRow::Watch { known }) = self.rows.get_mut(&LimbKey::Watch)
                        && known.iter().any(|o| o.resource == change.resource)
                    {
                        known.push(Observation {
                            resource: change.resource,
                            version: change.version,
                        });
                    }
                }
                Fact::Described { description, .. } => self.description = Some(description),
                other => println!("    {other:?}"),
            }
        }
        self.publish();
    }

    /// Take what the limb says for a moment.
    fn drain(&mut self) {
        while self.link.is_some()
            && let Some(up) = self.next(Duration::from_millis(100))
        {
            if let Up::Record(record) = up {
                self.include(record);
            }
        }
    }

    /// Print the call's live output until a chunk holds `text`.
    fn live_until(&mut self, text: &str) {
        loop {
            match self.next(Duration::from_secs(10)).expect("live output") {
                Up::Live(live) => {
                    let Live::Call { chunk, .. } = live.live else {
                        continue;
                    };
                    println!("  live: {chunk:?}");
                    if chunk.contains(text) {
                        return;
                    }
                }
                Up::Record(record) => {
                    self.include(record);
                }
            }
        }
    }

    /// Add the rows for one call by `context`: the call and its input.
    fn dispatch(&mut self, context: &mut Context, tool: &str, input: Value) -> CallId {
        println!("\n  {} → {tool} {input}", context.name);
        let id = CallId(self.mint());
        let bytes = Bytes::from(input.to_string());
        let call = Call {
            caller: Caller::Agent {
                task: context.task,
                tool_use: ToolUseId(format!("toolu_{}", id.0.at.n)),
            },
            tool: tool.into(),
            input: bytes.id(),
            expects: context
                .knows
                .iter()
                .map(|(resource, version)| Observation {
                    resource: resource.clone(),
                    version: *version,
                })
                .collect(),
        };
        self.rows
            .insert(LimbKey::Value(bytes.id()), LimbRow::Value(bytes));
        self.rows.insert(
            LimbKey::Call(id),
            LimbRow::Call {
                call,
                stopping: None,
                first_sent_to: self.instance.expect("an instance has connected"),
            },
        );
        self.publish();
        id
    }

    fn call(&mut self, context: &mut Context, tool: &str, input: Value) -> Option<Output> {
        let call = self.dispatch(context, tool, input);
        self.settle(context, call)
    }

    fn settle(&mut self, context: &mut Context, call: CallId) -> Option<Output> {
        loop {
            if let Some(outcome) = self.settled.remove(&call) {
                return show(context, outcome, &self.values);
            }
            self.take();
        }
    }
}

fn summary(facts: &[Fact], values: usize) -> String {
    let mut parts: Vec<String> = facts
        .iter()
        .map(|fact| match fact {
            Fact::Call(call, CallEvent::Settled(outcome)) => {
                format!("call {} settled {}", call.0.at.n, kind(outcome))
            }
            Fact::Change(_) => "a Change".into(),
            Fact::Described { .. } => "its description".into(),
            _ => "a fact".into(),
        })
        .collect();
    if values > 0 {
        parts.push(format!("{values} value(s) of bytes"));
    }
    parts.join(", ")
}

fn print_change(change: &Change) {
    let by = match change.by {
        Some(call) => format!("by call {}", call.0.at.n),
        None => "from outside the harness".into(),
    };
    println!(
        "      Change: {} {} is now {}, {by}",
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
        match content[0]["type"].as_str() {
            Some("image") => println!(
                "    │ [an image block: {}, {} bytes of base64]",
                content[0]["source"]["media_type"],
                content[0]["source"]["data"].as_str().unwrap().len()
            ),
            _ => content[0]["text"]
                .as_str()
                .unwrap()
                .lines()
                .for_each(|line| println!("    │ {line}")),
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

fn kind(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Completed(_) => "Completed",
        Outcome::Failed(_) => "Failed",
        Outcome::Refused { .. } => "Refused (no effect)",
        Outcome::Interrupted { .. } => "Interrupted",
        Outcome::Unknown => "Unknown",
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
    fn new(name: &'static str, task: TaskId) -> Context {
        Context {
            name,
            task,
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

fn read_pids(path: &Path) -> Vec<u32> {
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
