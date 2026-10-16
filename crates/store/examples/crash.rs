//! Kill a brain mid-append and show the store survived it.
//!
//! `crash <dir>` starts a writer (this program again, as `crash write
//! <file> <start>`), lets it commit a few batches, kills it with SIGKILL
//! while it is appending the next, then reopens the store, folds it, and
//! queries it.
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
use harness_core::task::{Created, Giver, Mandate, Mode, Move, Origin, Scope, TaskEvent};
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
        ["write", file, start] => write(file, start),
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
    let start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis() as i64;
    let mut writer = Command::new(std::env::current_exe()?)
        .args(["write", &file, &start.to_string()])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut lines = BufReader::new(writer.stdout.take().ok_or("no stdout")?).lines();
    let mut ends = vec![0];
    let mut append_took = Duration::ZERO;
    let mut began = Instant::now();
    let killed_in = loop {
        let line = lines.next().ok_or("writer exited early")??;
        let words: Vec<&str> = line.split(' ').collect();
        match words.as_slice() {
            ["appending", batch, end] => {
                let batch: u64 = batch.parse()?;
                ends.push(end.parse()?);
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
    let mut store = Store::open(&file)?;
    let state = store.fold()?;
    let last = state.last().map_or(0, |s| s.0);
    println!("folded {last} records, {} tasks", state.tasks().count());
    // The writer's state at the two boundaries the kill could have left.
    let mut writer = Brain::new(start);
    (1..killed_in).for_each(|n| writer.next(n));
    let before = writer.state.clone();
    writer.next(killed_in);
    if state == before {
        println!(
            "OK: batch {killed_in} is wholly absent; the state equals the writer's after batch {}",
            killed_in - 1
        );
    } else if state == writer.state {
        println!(
            "OK: batch {killed_in} had committed before the kill landed, and is wholly present"
        );
    } else {
        return Err(format!(
            "FAIL: the store ends at {last}, and its state is not the writer's at a batch boundary ({} or {})",
            ends[killed_in as usize - 1],
            ends[killed_in as usize]
        )
        .into());
    }

    println!("\n== A fact naming an entry the store does not hold is refused");
    let unstored = message(writer.root.id(), Role::User, Wire("[]".into()));
    let refused = store.append(&[Record {
        seq: Seq(last + 1),
        at: Timestamp(start),
        by: Author::Brain,
        fact: Fact::Task(
            TaskId(id("task", 1)),
            TaskEvent::Moved(Move::Rewind {
                head: unstored.id(),
            }),
        ),
    }]);
    match refused {
        Err(error) => println!("refused: {error}"),
        Ok(()) => return Err("FAIL: the store appended a fact naming an unstored entry".into()),
    }

    println!("\n== A face that had seen seq 2 catches up, one page of 3");
    for record in store.read(Seq(2), 3)? {
        let fact = match &record.fact {
            Fact::Task(id, _) => format!("Task {id}"),
            Fact::Delivery(delivery) => format!("Delivery to task {}", delivery.to),
            Fact::ModelCall(id, _) => format!("ModelCall {id}"),
            other => format!("{other:?}"),
        };
        println!("seq {}: {fact}", record.seq.0);
    }

    println!("\n== Tokens per task, in SQL over the journal");
    let sql = "\
SELECT sent.fact ->> '$.ModelCall[1].Sent.task' AS task,
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
        "{:<32} {:>8} {:>10} {:>10} {:>10}",
        "task", "requests", "input", "cache_read", "output"
    );
    while let Some(row) = rows.next()? {
        println!(
            "{:<32} {:>8} {:>10} {:>10} {:>10}",
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
fn write(file: &str, start: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open(file)?;
    let mut brain = Brain::new(start.parse()?);
    for n in 1.. {
        brain.next(n);
        for entry in &brain.entries {
            store.put_entry(entry)?;
        }
        let end = brain.state.last().map_or(0, |s| s.0);
        println!("appending {n} {end}");
        store.append(&brain.batch)?;
        println!("committed {n}");
    }
    Ok(())
}

/// Just enough of a brain, and deterministic given its start time, so the
/// watcher can rebuild exactly the state the writer had at each batch.
struct Brain {
    state: State,
    at: i64,
    root: Entry,
    /// The batch being built, and the entries it names.
    batch: Vec<Record>,
    entries: Vec<Entry>,
}

impl Brain {
    fn new(at: i64) -> Brain {
        Brain {
            state: State::default(),
            at,
            // Every task's context starts at the same root.
            root: Entry::Root(Root {
                model: "claude-opus-5-5".into(),
                system: Wire(r#"[{"type":"text","text":"You are a helpful agent."}]"#.into()),
                tools: Wire("[]".into()),
            }),
            batch: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Build batch `n`, one task's whole conversation with Max, folding each
    /// record into the state as it goes.
    fn next(&mut self, n: u64) {
        self.batch.clear();
        self.entries.clear();
        let environment = EnvironmentId(id("environment", 0));
        if n == 1 {
            self.push(
                Author::Brain,
                Fact::Launched(Launched {
                    build: "store-demo".into(),
                    by: None,
                }),
            );
            self.push(
                Author::Brain,
                Fact::Environment(Environment {
                    id: environment,
                    name: "chat".into(),
                    limbs: vec![],
                }),
            );
            self.entries.push(self.root.clone());
        }
        let max = Author::Max {
            face: FaceId(id("face", 0)),
            seen: Seq(0),
        };
        let task = TaskId(id("task", n));
        let root = self.root.id();
        self.push(
            Author::Brain,
            Fact::Task(
                task,
                TaskEvent::Created(Box::new(Created {
                    name: format!("chat {n}"),
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
            let k = n * TURNS + turn;
            let words = Wire(format!(r#"[{{"type":"text","text":"Max's turn {turn}"}}]"#));
            let delivery = DeliveryId(id("delivery", k));
            self.push(
                max.clone(),
                Fact::Delivery(Delivery {
                    id: delivery,
                    to: task,
                    content: Content::Words(words.clone()),
                }),
            );
            head = self.put(message(head, Role::User, words));
            let call = ModelCallId(id("model call", k));
            self.push(
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
            self.push(
                Author::Brain,
                Fact::ModelCall(call, ModelCallEvent::FirstByte),
            );
            let reply = Wire(format!(
                r#"[{{"type":"text","text":"Reply {turn} in chat {n}."}}]"#
            ));
            head = self.put(message(head, Role::Assistant, reply));
            self.push(
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
                            input: 3 + n,
                            cache_read: 1000 + 40 * turn,
                            cache_write_short: 40,
                            cache_write_long: 0,
                            output: 20 + (k % 7) * 10,
                        }),
                        quota: None,
                    },
                ),
            );
        }
    }

    fn put(&mut self, entry: Entry) -> EntryId {
        let id = entry.id();
        self.entries.push(entry);
        id
    }

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
