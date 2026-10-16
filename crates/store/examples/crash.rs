//! Kill a brain mid-append and show the store survived it.
//!
//! `crash <dir>` starts a writer (this program again, as `crash write
//! <file>`), lets it commit a few batches, kills it with SIGKILL while it is
//! appending the next, then reopens the store, folds it, and queries it.
//!
//! The writer behaves like a brain: each batch is one task's conversation
//! with Max (his words, the request, the response), every record applied to
//! its own [`State`] first, every entry stored before the fact that names it.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use harness_core::context::{Entry, EntryId, Message, Rendered, Role, Root, Wire};
use harness_core::delivery::{Content, Delivery};
use harness_core::environment::{Binding, Environment};
use harness_core::id::{
    DeliveryId, Digest, EnvironmentId, FaceId, ModelCallId, Seq, TaskId, Timestamp,
};
use harness_core::journal::{Author, Fact, Launched, Record};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Purpose, Stop, Usage};
use harness_core::state::State;
use harness_core::task::{Created, Giver, Mandate, Mode, Origin, Scope, TaskEvent};
use harness_store::Store;

/// Turns per task, so records per batch is `1 + 4 * TURNS`.
const TURNS: u64 = 500;
/// Batches the writer commits before the one it is killed in.
const COMMITTED: u64 = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["write", file] => write(file),
        [dir] => watch(dir),
        _ => Err("usage: crash <dir>".into()),
    }
}

/// The demo: run a writer, kill it mid-batch, reopen, fold, query.
fn watch(dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let file = format!("{dir}/journal.sqlite");
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{file}{suffix}"));
    }

    println!(
        "== A writer appends batches of {} records to {file}",
        1 + 4 * TURNS
    );
    let mut writer = Command::new(std::env::current_exe()?)
        .args(["write", &file])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut lines = BufReader::new(writer.stdout.take().ok_or("no stdout")?).lines();
    let mut digests = vec![String::new()];
    let mut ends = vec![0];
    let mut append_took = Duration::ZERO;
    let mut began = Instant::now();
    let killed_in = loop {
        let line = lines.next().ok_or("writer exited early")??;
        let words: Vec<&str> = line.split(' ').collect();
        match words.as_slice() {
            ["appending", batch, end, digest] => {
                let batch: u64 = batch.parse()?;
                ends.push(end.parse()?);
                digests.push(digest.to_string());
                began = Instant::now();
                println!(
                    "writer: appending batch {batch}, seq {}..={end}",
                    ends[batch as usize - 1] + 1
                );
                if batch > COMMITTED {
                    // Land the kill halfway through a typical append.
                    std::thread::sleep(append_took / 2);
                    writer.kill()?;
                    writer.wait()?;
                    println!(
                        "** kill -9 the writer {:?} into appending batch {batch}",
                        append_took / 2
                    );
                    break batch;
                }
            }
            ["committed", batch] => {
                append_took = began.elapsed();
                println!("writer: committed batch {batch} in {append_took:?}");
            }
            _ => return Err(format!("writer said: {line}").into()),
        }
    };

    println!("\n== Reopen the store and fold it");
    let store = Store::open(&file)?;
    let state = store.fold()?;
    let last = state.last().map_or(0, |s| s.0);
    let digest = digest(&state);
    let before = ends[killed_in as usize - 1];
    let after = ends[killed_in as usize];
    println!(
        "folded {last} records, {} tasks, state digest {digest}",
        state.tasks().count()
    );
    if last == before && digest == digests[killed_in as usize - 1] {
        println!(
            "OK: batch {killed_in} is wholly absent; the state is the writer's after batch {}",
            killed_in - 1
        );
    } else if last == after && digest == digests[killed_in as usize] {
        println!(
            "OK: batch {killed_in} had committed before the kill landed, and is wholly present"
        );
    } else {
        return Err(format!(
            "FAIL: the store ends at {last}, which is not a batch boundary ({before} or {after}) with the writer's state"
        )
        .into());
    }

    println!("\n== A face that had seen seq 2 catches up, one page of 3");
    for record in store.read(Seq(2), 3)? {
        let fact = match &record.fact {
            Fact::Task(id, _) => format!("Task {}", id.0),
            Fact::Delivery(delivery) => format!("Delivery to task {}", delivery.to.0),
            Fact::ModelCall(id, _) => format!("ModelCall {}", id.0),
            other => format!("{other:?}"),
        };
        println!("seq {}: {fact}", record.seq.0);
    }

    println!("\n== Tokens per task, in SQL over the journal");
    let sql = "\
SELECT sent.fact -> '$.ModelCall[1].Sent.task' AS task,
       count(*) AS requests,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.input') AS input,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.cache_read') AS cache_read,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.output') AS output
FROM record AS ended
JOIN record AS sent ON sent.subject = ended.subject
  AND sent.fact -> '$.ModelCall[1].Sent' IS NOT NULL
WHERE ended.kind = 'ModelCall'
  AND ended.fact -> '$.ModelCall[1].Ended' IS NOT NULL
GROUP BY task
ORDER BY task;";
    println!("{sql}\n");
    let db = rusqlite::Connection::open(&file)?;
    let mut query = db.prepare(sql)?;
    let mut rows = query.query([])?;
    println!(
        "{:<40} {:>8} {:>10} {:>10} {:>10}",
        "task", "requests", "input", "cache_read", "output"
    );
    while let Some(row) = rows.next()? {
        println!(
            "{:<40} {:>8} {:>10} {:>10} {:>10}",
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        );
    }
    Ok(())
}

/// The writer: commit batches until killed.
fn write(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut brain = Brain {
        store: Store::open(file)?,
        state: State::default(),
        batch: Vec::new(),
        at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as i64,
    };
    let environment = EnvironmentId(id("environment", 0));
    brain.push(
        Author::Brain,
        Fact::Launched(Launched {
            build: "store-demo".into(),
            by: None,
        }),
    );
    brain.push(
        Author::Brain,
        Fact::Environment(Environment {
            id: environment,
            name: "chat".into(),
            limbs: vec![],
        }),
    );
    // Every task's context starts at the same root, stored once.
    let root = brain.store.put_entry(&Entry::Root(Root {
        model: "claude-opus-5-5".into(),
        system: Wire(r#"[{"type":"text","text":"You are a helpful agent."}]"#.into()),
        tools: Wire("[]".into()),
    }))?;
    let max = Author::Max {
        face: FaceId(id("face", 0)),
        seen: Seq(0),
    };

    for batch in 1.. {
        let task = TaskId(id("task", batch));
        brain.push(
            Author::Brain,
            Fact::Task(
                task,
                TaskEvent::Created(Box::new(Created {
                    name: format!("chat {batch}"),
                    agent: "assistant".into(),
                    scope: Scope::Max,
                    origin: Origin::Fresh,
                    mode: Mode::UserFacing,
                    mandate: Mandate {
                        text: "Talk with Max.".into(),
                        from: Giver::Max,
                    },
                    head: root,
                    binding: Binding {
                        environment,
                        routes: vec![],
                    },
                    observes: vec![],
                    awaits: vec![],
                })),
            ),
        );
        let mut head = root;
        for turn in 0..TURNS {
            let n = batch * TURNS + turn;
            let words = Wire(format!(r#"[{{"type":"text","text":"Max's turn {turn}"}}]"#));
            let delivery = DeliveryId(id("delivery", n));
            brain.push(
                max.clone(),
                Fact::Delivery(Delivery {
                    id: delivery,
                    to: task,
                    content: Content::Words(words.clone()),
                }),
            );
            head = brain.store.put_entry(&message(head, Role::User, words))?;
            let call = ModelCallId(id("model call", n));
            brain.push(
                Author::Brain,
                Fact::ModelCall(
                    call,
                    ModelCallEvent::Sent(ModelCall {
                        task,
                        purpose: Purpose::Turn,
                        head,
                        renders: vec![Rendered::Delivery(delivery)],
                        params: Wire(r#"{"max_tokens":32000,"stream":true}"#.into()),
                        breakpoints: vec![],
                    }),
                ),
            );
            brain.push(
                Author::Brain,
                Fact::ModelCall(call, ModelCallEvent::FirstByte),
            );
            let reply = Wire(format!(
                r#"[{{"type":"text","text":"Reply {turn} in chat {batch}."}}]"#
            ));
            head = brain
                .store
                .put_entry(&message(head, Role::Assistant, reply))?;
            brain.push(
                Author::Brain,
                Fact::ModelCall(
                    call,
                    ModelCallEvent::Ended {
                        ending: Ending::Responded {
                            entry: head,
                            stop: Stop::EndTurn,
                            calls: vec![],
                        },
                        usage: Some(Usage {
                            input: 3 + batch,
                            cache_read: 1000 + 40 * turn,
                            cache_write_short: 40,
                            cache_write_long: 0,
                            output: 20 + (n % 7) * 10,
                        }),
                        quota: None,
                    },
                ),
            );
        }
        let end = brain.state.last().map_or(0, |s| s.0);
        println!("appending {batch} {end} {}", digest(&brain.state));
        brain.store.append(&brain.batch)?;
        brain.batch.clear();
        println!("committed {batch}");
    }
    Ok(())
}

/// Just enough of a brain: fold each fact into its state, then append.
struct Brain {
    store: Store,
    state: State,
    batch: Vec<Record>,
    at: i64,
}

impl Brain {
    fn push(&mut self, by: Author, fact: Fact) {
        self.at += 250;
        let record = Record {
            seq: Seq(self.state.last().map_or(1, |s| s.0 + 1)),
            at: Timestamp(self.at),
            by,
            fact,
        };
        if let Err(rejected) = self.state.apply(&record) {
            panic!("the demo brain wrote a fact the model refuses: {rejected:?} {record:?}");
        }
        self.batch.push(record);
    }
}

fn message(parent: EntryId, role: Role, content: Wire) -> Entry {
    Entry::Message(Message {
        parent,
        role,
        content,
    })
}

/// A full-width id, so the demo exercises ids beyond SQLite's integers.
fn id(kind: &str, n: u64) -> u128 {
    let digest = Digest::of(format!("{kind} {n}").as_bytes());
    u128::from_le_bytes(digest.0[..16].try_into().unwrap())
}

fn digest(state: &State) -> String {
    let digest = Digest::of(format!("{state:?}").as_bytes());
    digest.0[..8].iter().map(|b| format!("{b:02x}")).collect()
}
