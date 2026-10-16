//! The web face watching a scripted night of work.
//!
//! A realistic journal is written as it would be by a brain: every record is
//! folded with `State::apply` first and appended to a `harness-store` file
//! only if the fold accepts it. The face reads the same file on its own
//! connection. An orchestrator spawns two designers that share one attachment, then
//! two engineers and a fork of itself; one engineer asks Max a question and
//! waits on him, the other fails after three overloaded requests. Each
//! agent's facts are written from its own thread, so the tree moves the way
//! a real one does.
//!
//! Run: `cargo run -p harness-face-web --example scripted [port]`.

use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::attention::{Ranked, Ranking};
use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::context::{Entry, EntryId, Message, Rendered, Role, Root, Wire};
use harness_core::delivery::{Content, Delivery};
use harness_core::environment::{
    Binding, Environment, Fork, Host, Limb, LimbKind, Machine, Reach, Route, Startable,
};
use harness_core::id::{
    CallId, DeliveryId, EnvironmentId, FaceId, InstanceId, LimbId, ModelCallId, Seq, TaskId,
    Timestamp, ToolUseId,
};
use harness_core::journal::{Author, Fact, Launched, Record};
use harness_core::model_call::{
    Breakpoint, Ending, ModelCall, ModelCallEvent, Purpose, Stop, Ttl, Usage,
};
use harness_core::state::State;
use harness_core::task::{Created, Drain, Giver, Mandate, Mode, Origin, Scope, TaskEvent};
use harness_face_web::{Face, Source, describe};
use harness_store::Store;
use serde_json::{Value, json};

// The journal: one SQLite store. The brain writes it on its connection; the
// face reads it on its own, woken when the brain appends.

#[derive(Clone)]
struct Reader(Arc<Shared>);

struct Shared {
    store: Mutex<Store>,
    last: Mutex<Seq>,
    grew: Condvar,
}

impl Source for Reader {
    fn read(&self, after: Seq, wait: Duration) -> Vec<Record> {
        let last = self.0.last.lock().unwrap();
        let wake = self.0.grew.wait_timeout_while(last, wait, |l| *l <= after);
        drop(wake.unwrap());
        let store = self.0.store.lock().unwrap();
        store.read(after, u64::MAX).expect("the store reads")
    }

    fn entry(&self, id: EntryId) -> Option<Entry> {
        let store = self.0.store.lock().unwrap();
        store.entry(id).expect("the store reads")
    }
}

// The writer: the one brain this journal has.

struct Writer {
    store: Store,
    reader: Reader,
    state: State,
    next: u128,
}

#[derive(Clone)]
struct Brain(Arc<Mutex<Writer>>);

const MAX: Author = Author::Max {
    face: FaceId(1),
    seen: Seq(0),
};

impl Brain {
    fn id(&self) -> u128 {
        let mut w = self.0.lock().unwrap();
        w.next += 1;
        w.next
    }

    /// Fold the fact, and append it only if the fold accepts it.
    fn push(&self, by: Author, fact: Fact) {
        let mut w = self.0.lock().unwrap();
        let seq = Seq(w.state.last().map_or(1, |s| s.0 + 1));
        let by = match by {
            // Max acts on what his face had shown him: everything so far.
            Author::Max { face, .. } => Author::Max {
                face,
                seen: Seq(seq.0 - 1),
            },
            by => by,
        };
        let record = Record {
            seq,
            at: now(),
            by,
            fact,
        };
        if let Err(rejected) = w.state.apply(&record) {
            panic!("the script is wrong at {}: {rejected:?}\n{record:?}", seq.0);
        }
        println!("{:>4}  {}", seq.0, describe(&w.state, &record));
        w.store.append(&[record]).expect("the store appends");
        *w.reader.0.last.lock().unwrap() = seq;
        w.reader.0.grew.notify_all();
    }

    fn brain(&self, fact: Fact) {
        self.push(Author::Brain, fact);
    }

    /// Store an entry before any fact names it.
    fn store(&self, entry: Entry) -> EntryId {
        let w = self.0.lock().unwrap();
        w.store
            .put_entry(&entry)
            .expect("the store keeps the entry")
    }

    fn message(&self, parent: EntryId, role: Role, content: Value) -> EntryId {
        self.store(Entry::Message(Message {
            parent,
            role,
            content: Wire(content.to_string()),
        }))
    }

    fn root(&self, role: &str, tools: &[&str]) -> EntryId {
        let tools: Vec<Value> = tools.iter().map(|t| json!({"name": t})).collect();
        self.store(Entry::Root(Root {
            model: "claude-opus-5-5".into(),
            system: Wire(json!(format!("You are the {role}. Read AGENTS.md first.")).to_string()),
            tools: Wire(Value::from(tools).to_string()),
        }))
    }

    /// One request: the brain prepares it, the model thinks until its first
    /// byte, streams, and ends with these tool uses or its turn.
    fn turn(&self, task: TaskId, head: EntryId, renders: Vec<Rendered>, t: Turn) -> Responded {
        pause(0.5);
        let mc = ModelCallId(self.id());
        self.brain(Fact::ModelCall(
            mc,
            ModelCallEvent::Sent(ModelCall {
                task,
                purpose: Purpose::Turn,
                head,
                renders,
                params: Wire(r#"{"max_tokens":32000}"#.into()),
                breakpoints: vec![Breakpoint {
                    at: head,
                    ttl: Ttl::Short,
                }],
            }),
        ));
        pause(t.first_byte);
        self.brain(Fact::ModelCall(mc, ModelCallEvent::FirstByte));
        pause(t.streaming);
        let mut blocks = vec![json!({"type": "text", "text": t.text})];
        let mut uses = Vec::new();
        for (limb, tool, input) in t.tools {
            let call = CallId(self.id());
            let tool_use = format!("toolu_{:x}", call.0);
            self.brain(Fact::Call(
                call,
                CallEvent::Made(Call {
                    caller: Caller::Agent {
                        task,
                        tool_use: Some(ToolUseId(tool_use.clone())),
                    },
                    limb,
                    tool: tool.into(),
                    input: Wire(input.to_string()),
                    expects: Vec::new(),
                }),
            ));
            blocks.push(json!({"type": "tool_use", "id": tool_use, "name": tool, "input": input}));
            uses.push((call, tool_use, limb));
        }
        let entry = self.message(head, Role::Assistant, Value::from(blocks));
        let calls: Vec<CallId> = uses.iter().map(|u| u.0).collect();
        let stop = if calls.is_empty() {
            Stop::EndTurn
        } else {
            Stop::ToolUse
        };
        self.brain(Fact::ModelCall(
            mc,
            ModelCallEvent::Ended {
                ending: Ending::Responded { entry, stop, calls },
                usage: Some(Usage {
                    input: 2_000 + (mc.0 as u64 % 7) * 300,
                    cache_read: 18_000 + (mc.0 as u64 % 5) * 4_000,
                    cache_write_short: 1_200,
                    cache_write_long: 0,
                    output: 600 + (mc.0 as u64 % 3) * 250,
                }),
                quota: None,
            },
        ));
        Responded { entry, uses }
    }

    /// A request the provider refuses.
    fn overloaded(&self, task: TaskId, head: EntryId, renders: Vec<Rendered>, wait: f32) {
        pause(0.5);
        let mc = ModelCallId(self.id());
        self.brain(Fact::ModelCall(
            mc,
            ModelCallEvent::Sent(ModelCall {
                task,
                purpose: Purpose::Turn,
                head,
                renders,
                params: Wire(r#"{"max_tokens":32000}"#.into()),
                breakpoints: Vec::new(),
            }),
        ));
        pause(wait);
        self.brain(Fact::ModelCall(
            mc,
            ModelCallEvent::Ended {
                ending: Ending::Failed {
                    status: Some(529),
                    message: "overloaded_error".into(),
                    retry: None,
                },
                usage: None,
                quota: None,
            },
        ));
    }

    /// Run a call on its limb and settle it with `result`.
    fn run(&self, call: CallId, limb: LimbId, secs: f32, result: &str) {
        self.dispatch(call, limb);
        pause(secs);
        self.settle(call, Author::Limb(limb), result);
    }

    fn dispatch(&self, call: CallId, limb: LimbId) {
        self.brain(Fact::Call(
            call,
            CallEvent::Dispatched {
                instance: InstanceId(limb.0),
            },
        ));
    }

    fn settle(&self, call: CallId, by: Author, result: &str) {
        self.push(
            by,
            Fact::Call(
                call,
                CallEvent::Settled(Outcome::Completed(Output {
                    model: Wire(json!(result).to_string()),
                    full: None,
                    observes: Vec::new(),
                })),
            ),
        );
    }

    /// The user message answering a response's tool uses.
    fn results(&self, parent: EntryId, answers: &[(&str, &str)]) -> EntryId {
        let blocks: Vec<Value> = answers
            .iter()
            .map(|(id, text)| json!({"type": "tool_result", "tool_use_id": id, "content": text}))
            .collect();
        self.message(parent, Role::User, Value::from(blocks))
    }

    fn create(&self, id: TaskId, created: Created) {
        self.brain(Fact::Task(id, TaskEvent::Created(Box::new(created))));
    }
}

struct Turn {
    first_byte: f32,
    streaming: f32,
    text: &'static str,
    tools: Vec<(LimbId, &'static str, Value)>,
}

struct Responded {
    entry: EntryId,
    uses: Vec<(CallId, String, LimbId)>,
}

fn turn(first_byte: f32, streaming: f32, text: &'static str) -> Turn {
    Turn {
        first_byte,
        streaming,
        text,
        tools: Vec::new(),
    }
}

impl Turn {
    fn tool(mut self, limb: LimbId, tool: &'static str, input: Value) -> Turn {
        self.tools.push((limb, tool, input));
        self
    }
}

fn pause(secs: f32) {
    thread::sleep(Duration::from_secs_f32(secs));
}

fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    Timestamp(ms as i64)
}

fn mandate(text: &str, from: Giver) -> Mandate {
    Mandate {
        text: text.into(),
        from,
    }
}

fn binding(environment: EnvironmentId, routes: &[(&str, LimbId)]) -> Binding {
    Binding {
        environment,
        routes: routes
            .iter()
            .map(|(tool, limb)| Route {
                tool: (*tool).into(),
                limb: *limb,
            })
            .collect(),
    }
}

// The night's work.

struct World {
    meta: LimbId,
    max: LimbId,
    ws: LimbId,
    orchestrator: Binding,
    designer: Binding,
    engineer: Binding,
}

fn setup(b: &Brain) -> World {
    b.brain(Fact::Launched(Launched {
        build: "face-web-demo".into(),
        by: None,
    }));
    let (meta, max, ws) = (LimbId(b.id()), LimbId(b.id()), LimbId(b.id()));
    let (orch, design, eng) = (
        EnvironmentId(b.id()),
        EnvironmentId(b.id()),
        EnvironmentId(b.id()),
    );
    let limb = |id, name: &str, host, kind| {
        b.brain(Fact::Limb(Limb {
            id,
            name: name.into(),
            host,
            kind,
            fork: Fork::Share,
        }))
    };
    let starts = [("designer", design), ("engineer", eng)]
        .map(|(agent, environment)| Startable {
            agent: agent.into(),
            environment,
        })
        .to_vec();
    limb(meta, "meta", Host::Brain, LimbKind::Meta { starts });
    limb(max, "max", Host::Face, LimbKind::Max);
    let workspace = LimbKind::Plugin {
        kind: "workspace".into(),
        config: Wire(r#"{"root":"~/agent-harness-workspace/ah-face-web"}"#.into()),
    };
    limb(
        ws,
        "workspace",
        Host::Process {
            machine: Machine::Local,
        },
        workspace,
    );
    b.brain(Fact::Reach {
        limb: ws,
        reach: Reach::Connected {
            instance: InstanceId(ws.0),
            since: now(),
        },
    });
    let env = |id, name: &str, limbs: Vec<LimbId>| {
        b.brain(Fact::Environment(Environment {
            id,
            name: name.into(),
            limbs,
        }))
    };
    env(orch, "orchestrator", vec![meta, max]);
    env(design, "designer", vec![ws]);
    env(eng, "engineer", vec![ws, max]);
    World {
        meta,
        max,
        ws,
        orchestrator: binding(orch, &[("spawn", meta), ("ask", max)]),
        designer: binding(design, &[("read", ws), ("grep", ws)]),
        engineer: binding(
            eng,
            &[("read", ws), ("edit", ws), ("bash", ws), ("ask", max)],
        ),
    }
}

fn fresh(
    name: &str,
    agent: &str,
    scope: Scope,
    m: Mandate,
    head: EntryId,
    binding: &Binding,
    awaits: Vec<CallId>,
) -> Created {
    Created {
        name: name.into(),
        agent: agent.into(),
        scope,
        origin: Origin::Fresh,
        // Max talks to what he starts; what an agent spawns reports back.
        mode: match scope {
            Scope::Max => Mode::UserFacing,
            Scope::Spawn(_) => Mode::Autonomous,
        },
        mandate: m,
        head,
        binding: binding.clone(),
        observes: Vec::new(),
        awaits,
    }
}

fn script(b: Brain) {
    let w = Arc::new(setup(&b));

    let orch = TaskId(b.id());
    let root = b.root("orchestrator", &["spawn", "ask"]);
    let night = "Build the foundations tonight: store, provider, workspace limb, web face.";
    b.create(
        orch,
        fresh(
            "orchestrator",
            "orchestrator",
            Scope::Max,
            mandate(night, Giver::Max),
            root,
            &w.orchestrator,
            Vec::new(),
        ),
    );
    let words = DeliveryId(b.id());
    b.push(
        MAX,
        Fact::Delivery(Delivery {
            id: words,
            to: orch,
            content: Content::Words(Wire(json!(night).to_string())),
        }),
    );
    let asked = b.message(root, Role::User, json!([{"type": "text", "text": night}]));

    // Scope 1: two designers, sharing one read of the vision.
    let r = b.turn(
        orch,
        asked,
        vec![Rendered::Delivery(words)],
        turn(2.0, 3.0, "Two designers first, with different framings.").tool(
            w.meta,
            "spawn",
            json!({"agents": ["designer-a", "designer-b"]}),
        ),
    );
    let (spawn, spawn_use, _) = r.uses[0].clone();
    b.dispatch(spawn, w.meta);
    let vision = CallId(b.id());
    b.brain(Fact::Call(
        vision,
        CallEvent::Made(Call {
            caller: Caller::Meta { call: spawn },
            limb: w.ws,
            tool: "read".into(),
            input: Wire(r#"{"path":"vision.md"}"#.into()),
            expects: Vec::new(),
        }),
    ));
    let designers: Vec<(TaskId, &str)> = vec![
        (TaskId(b.id()), "designer-a"),
        (TaskId(b.id()), "designer-b"),
    ];
    let droot = b.root("designer", &["read", "grep"]);
    for (id, name) in &designers {
        b.create(
            *id,
            fresh(
                name,
                "designer",
                Scope::Spawn(spawn),
                mandate(
                    "Decide the core model, back-chained from the vision.",
                    Giver::Task(orch),
                ),
                droot,
                &w.designer,
                vec![vision],
            ),
        );
    }
    b.run(vision, w.ws, 2.0, "# Vision\nA personal agent harness...");
    let attached = {
        let read = b.message(droot, Role::Assistant, json!([{"type": "tool_use", "id": "toolu_vision", "name": "read", "input": {"path": "vision.md"}}]));
        b.results(
            read,
            &[("toolu_vision", "# Vision\nA personal agent harness...")],
        )
    };
    let handles: Vec<_> = designers
        .iter()
        .enumerate()
        .map(|(i, (id, _))| {
            let (b, w, id) = (b.clone(), Arc::clone(&w), *id);
            thread::spawn(move || {
                let slow = i as f32 * 1.5;
                let r = b.turn(
                    id,
                    attached,
                    vec![Rendered::Call(vision)],
                    turn(1.5 + slow, 3.0, "Reading the reference designs.").tool(
                        w.ws,
                        "grep",
                        json!({"pattern": "fn apply", "path": "ref-salsa"}),
                    ),
                );
                let (call, tu, limb) = r.uses[0].clone();
                b.run(call, limb, 2.0 + slow, "src/runtime.rs:212: fn apply(...)");
                let back = b.results(r.entry, &[(&tu, "src/runtime.rs:212: fn apply(...)")]);
                b.turn(
                    id,
                    back,
                    vec![Rendered::Call(call)],
                    turn(
                        1.0,
                        4.0 + slow,
                        "The journal is the only truth; every view is a fold.",
                    ),
                );
            })
        })
        .collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    pause(1.0);
    b.settle(
        spawn,
        Author::Brain,
        "designer-a: one journal, folds. designer-b: agrees, adds attention.",
    );

    // Scope 2: two engineers and a fork of the orchestrator.
    let back = b.results(
        r.entry,
        &[(
            &spawn_use,
            "designer-a: one journal, folds. designer-b: agrees.",
        )],
    );
    let r = b.turn(
        orch,
        back,
        vec![Rendered::Call(spawn)],
        turn(
            1.5,
            4.0,
            "Designs agree. Starting the engineers, and a fork to check the briefs.",
        )
        .tool(
            w.meta,
            "spawn",
            json!({"agents": ["engineer-store", "engineer-face-web", "fork:review-briefs"]}),
        ),
    );
    let (spawn, spawn_use, _) = r.uses[0].clone();
    b.dispatch(spawn, w.meta);
    let eroot = b.root("engineer", &["read", "edit", "bash", "ask"]);
    let store = TaskId(b.id());
    let face = TaskId(b.id());
    let fork = TaskId(b.id());
    b.create(
        store,
        fresh(
            "engineer-store",
            "engineer",
            Scope::Spawn(spawn),
            mandate(
                "Build crates/store: the journal on disk.",
                Giver::Task(orch),
            ),
            eroot,
            &w.engineer,
            Vec::new(),
        ),
    );
    b.create(
        face,
        fresh(
            "engineer-face-web",
            "engineer",
            Scope::Spawn(spawn),
            mandate("Build crates/face-web: the tree, live.", Giver::Task(orch)),
            eroot,
            &w.engineer,
            Vec::new(),
        ),
    );
    b.create(
        fork,
        Created {
            name: "orchestrator-review-briefs".into(),
            agent: "orchestrator".into(),
            scope: Scope::Spawn(spawn),
            origin: Origin::Fork { from: orch },
            mode: Mode::Autonomous,
            mandate: mandate(
                "Check each engineer brief against Max's words.",
                Giver::Task(orch),
            ),
            head: r.entry,
            binding: w.orchestrator.clone(),
            observes: Vec::new(),
            awaits: Vec::new(),
        },
    );

    let mut handles = Vec::new();
    let (b2, w2) = (b.clone(), Arc::clone(&w));
    handles.push(thread::spawn(move || {
        let (b, w) = (b2, w2);
        let brief = b.message(
            eroot,
            Role::User,
            json!("Build crates/store: the journal on disk."),
        );
        let r = b.turn(
            store,
            brief,
            vec![],
            turn(2.5, 3.0, "Starting the crate.").tool(
                w.ws,
                "bash",
                json!({"command": "cargo new --lib crates/store"}),
            ),
        );
        let (call, tu, limb) = r.uses[0].clone();
        b.run(call, limb, 3.0, "Created library `store` package");
        let back = b.results(r.entry, &[(&tu, "Created library `store` package")]);
        for (i, wait) in [4.0, 5.0, 6.0].into_iter().enumerate() {
            // The first attempt renders the result; it is in the head after.
            let renders = if i == 0 {
                vec![Rendered::Call(call)]
            } else {
                Vec::new()
            };
            b.overloaded(store, back, renders, wait);
            pause(1.5);
        }
        b.brain(Fact::Task(
            store,
            TaskEvent::Drain(Drain::Fail(
                "the provider was overloaded on 3 attempts in a row".into(),
            )),
        ));
    }));
    let (b2, w2) = (b.clone(), Arc::clone(&w));
    handles.push(thread::spawn(move || {
        let (b, w) = (b2, w2);
        let brief = b.message(
            eroot,
            Role::User,
            json!("Build crates/face-web: the tree, live."),
        );
        let r = b.turn(
            face,
            brief,
            vec![],
            turn(1.5, 3.0, "Reading the core model and the style notes.")
                .tool(w.ws, "read", json!({"path": "crates/core/src/state.rs"}))
                .tool(w.ws, "read", json!({"path": "tui-styling.md"})),
        );
        let (c1, t1, _) = r.uses[0].clone();
        let (c2, t2, _) = r.uses[1].clone();
        b.dispatch(c1, w.ws);
        b.dispatch(c2, w.ws);
        pause(1.5);
        b.settle(
            c2,
            Author::Limb(w.ws),
            "beige background, maroon for important text",
        );
        pause(2.5);
        b.settle(
            c1,
            Author::Limb(w.ws),
            "pub fn waiting_on(&self, task: TaskId, now: Timestamp) ...",
        );
        let back = b.results(
            r.entry,
            &[
                (&t1, "pub fn waiting_on(...)"),
                (&t2, "beige background, maroon"),
            ],
        );
        let r = b.turn(
            face,
            back,
            vec![Rendered::Call(c1), Rendered::Call(c2)],
            turn(2.0, 2.0, "One question before I serve it.").tool(
                w.max,
                "ask",
                json!({"question": "Which port should the face serve on?"}),
            ),
        );
        let (ask, tu, _) = r.uses[0].clone();
        b.dispatch(ask, w.max);
        b.brain(Fact::Ranking(Ranking {
            order: vec![Ranked {
                task: face,
                why: "Blocks engineer-face-web; one word answers it.".into(),
            }],
        }));
        pause(15.0);
        b.settle(ask, MAX, "4320 is fine.");
        let back = b.results(r.entry, &[(&tu, "4320 is fine.")]);
        let r = b.turn(
            face,
            back,
            vec![Rendered::Call(ask)],
            turn(1.5, 3.0, "Writing the server.").tool(
                w.ws,
                "edit",
                json!({"path": "crates/face-web/src/lib.rs"}),
            ),
        );
        let (call, tu, limb) = r.uses[0].clone();
        b.run(call, limb, 2.0, "edited");
        let back = b.results(r.entry, &[(&tu, "edited")]);
        b.turn(
            face,
            back,
            vec![Rendered::Call(call)],
            turn(1.0, 5.0, "Served at https://mx-hv-mj.local:4320."),
        );
    }));
    let (b2, fork_use) = (b.clone(), spawn_use.clone());
    handles.push(thread::spawn(move || {
        let (b, spawn_use) = (b2, fork_use);
        let first = b.results(
            r.entry,
            &[(
                &spawn_use,
                "You are the fork: check each engineer brief against Max's words.",
            )],
        );
        b.turn(
            fork,
            first,
            vec![],
            turn(
                3.0,
                8.0,
                "The briefs match. The store brief should say who owns retries.",
            ),
        );
    }));
    handles.into_iter().for_each(|h| h.join().unwrap());
    pause(1.0);
    b.settle(
        spawn,
        Author::Brain,
        "store: failed (overloaded). face-web: done. review: briefs match.",
    );

    let back = b.results(
        r.entry,
        &[(
            &spawn_use,
            "store: failed (overloaded). face-web: done. review: briefs match.",
        )],
    );
    b.turn(
        orch,
        back,
        vec![Rendered::Call(spawn)],
        turn(
            1.5,
            4.0,
            "Scope 2 is in. The store engineer failed on provider overload; relaunch it?",
        ),
    );
    b.brain(Fact::Ranking(Ranking {
        order: vec![Ranked {
            task: orch,
            why: "Scope 2 results are in; the store engineer failed and needs a decision.".into(),
        }],
    }));
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .map_or(4320, |p| p.parse().expect("a port number"));
    let certs =
        PathBuf::from(std::env::var("HOME").unwrap()).join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .expect("the shared wildcard certificate");

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/face-web-demo.sqlite"
    );
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path}{suffix}"));
    }
    let store = Store::open(path).expect("the store opens");
    let reader = Reader(Arc::new(Shared {
        store: Mutex::new(Store::open(path).expect("the store opens")),
        last: Mutex::new(Seq(0)),
        grew: Condvar::new(),
    }));
    let face = Face::new(reader.clone());
    let follower = Arc::clone(&face);
    thread::spawn(move || follower.follow());
    let listener = TcpListener::bind(("0.0.0.0", port)).expect("the port is free");
    // The Windows host, read again for each connection: DHCP can move it.
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    let server = thread::spawn(move || harness_face_web::serve(face, listener, tls, allow));
    println!("face: https://mx-hv-mj.local:{port}\n");

    let brain = Brain(Arc::new(Mutex::new(Writer {
        store,
        reader,
        state: State::default(),
        next: 0,
    })));
    script(brain);
    println!("\nscript finished; serving until Ctrl-C");
    server.join().unwrap().expect("the listener");
}
