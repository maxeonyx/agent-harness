//! Kill a brain mid-commit and show the store survived it.
//!
//! `crash <dir>` starts a writer (this program again, as `crash write
//! <file> <start>`), lets it commit a few batches, kills it with SIGKILL
//! while it is appending the next, then reopens the journal, folds it, and
//! queries it.
//!
//! The writer behaves like a brain: every batch goes through
//! [`Journal::commit`], carrying the bytes its facts name. A batch is
//! [`TASKS`] one-turn tasks, each started, sent, answered and so done.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use harness_core::context::{Message, Note, Rendered, Role, Root};
use harness_core::environment::{Binding, Environment};
use harness_core::id::{Digest, EnvironmentId, ModelCallId, Seq, TaskId, Timestamp, ValueId};
use harness_core::journal::{self, Author, Batch, Fact, Journal, Launched, Rejected};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Purpose, Stop, Usage};
use harness_core::task::{Created, Giver, Mandate, Mode, Origin, Scope, Start, TaskEvent};
use harness_store::Store;

/// Tasks per batch, so records per batch is `4 * TASKS`.
const TASKS: u64 = 500;
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
        ["write", file, start] => write(file, start.parse()?),
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
        "== A writer commits batches of {} records to {file}",
        4 * TASKS
    );
    let start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis() as i64;
    let mut writer = Command::new(std::env::current_exe()?)
        .args(["write", &file, &start.to_string()])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut lines = BufReader::new(writer.stdout.take().ok_or("no stdout")?).lines();
    let mut append_took = Duration::ZERO;
    let mut began = Instant::now();
    let mut committed = 0;
    let killed_in = loop {
        let line = lines.next().ok_or("writer exited early")??;
        let words: Vec<&str> = line.split(' ').collect();
        match words.as_slice() {
            ["appending"] => {
                let batch = committed + 1;
                began = Instant::now();
                println!("writer: folded batch {batch}, appending it");
                if batch > COMMITTED {
                    // Land the kill halfway through a typical commit.
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
            ["committed", batch, first, last] => {
                committed += 1;
                append_took = began.elapsed();
                println!(
                    "writer: committed batch {batch}, seq {first}..={last}, in {append_took:?}"
                );
            }
            _ => return Err(format!("writer said: {line}").into()),
        }
    };

    println!("\n== Reopen the journal and fold it");
    let mut journal = Journal::open(Store::open(&file)?)?;
    let state = journal.state().clone();
    let last = state.last().map_or(0, |s| s.0);
    println!("folded {last} records, {} tasks", state.tasks().count());
    // The writer's state at the two boundaries the kill could have left,
    // rebuilt by the same brain on a store in memory.
    let mut rebuilt = Brain::new(Store::open(":memory:")?, start)?;
    for n in 1..killed_in {
        rebuilt.commit(n)?;
    }
    let before = rebuilt.journal.state().clone();
    rebuilt.commit(killed_in)?;
    if state == before {
        println!(
            "OK: batch {killed_in} is wholly absent; the state equals the writer's after batch {}",
            killed_in - 1
        );
    } else if state == *rebuilt.journal.state() {
        println!(
            "OK: batch {killed_in} had committed before the kill landed, and is wholly present"
        );
    } else {
        return Err(format!(
            "FAIL: the store ends at seq {last}, and its state is not the writer's at a batch boundary"
        )
        .into());
    }

    println!("\n== A fact naming a value that was never stored is refused");
    let unstored = ValueId::of(b"never stored");
    let refused = journal.commit(
        Author::Brain,
        Batch {
            values: vec![],
            facts: vec![rebuilt.created(TaskId(id("task", 0)), unstored)],
        },
        Timestamp(start),
    );
    match refused {
        Err(journal::Error::Refused {
            index,
            why: Rejected::Missing(value),
        }) => println!("refused fact {index}: value {value} is neither stored nor in the batch"),
        Err(other) => return Err(format!("FAIL: {other:?}").into()),
        Ok(_) => return Err("FAIL: the journal committed a fact naming an unstored value".into()),
    }

    println!("\n== A face that had seen seq 2 catches up, one page of 4");
    let store = journal.store();
    for record in store.read(Seq(2), 4)? {
        let fact = match &record.fact {
            Fact::Task(id, _) => format!("Task {id}"),
            Fact::ModelCall(
                id,
                ModelCallEvent::Ended {
                    ending: Ending::Responded { content, .. },
                    ..
                },
            ) => {
                let bytes = store.value(*content)?.ok_or("response not stored")?;
                format!(
                    "ModelCall {id} responded {}",
                    String::from_utf8_lossy(&bytes)
                )
            }
            Fact::ModelCall(id, _) => format!("ModelCall {id}"),
            other => format!("{other:?}"),
        };
        println!("seq {}: {fact}", record.seq.0);
    }

    println!("\n== The five tasks with the most output tokens, in SQL over the journal");
    let sql = "\
SELECT sent.fact ->> '$.ModelCall[1].Sent.task' AS task,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.input') AS input,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.cache_read') AS cache_read,
       sum(ended.fact ->> '$.ModelCall[1].Ended.usage.output') AS output
FROM record AS ended
JOIN record AS sent ON sent.subject = ended.subject
  AND sent.fact -> '$.ModelCall[1].Sent' IS NOT NULL
WHERE ended.kind = 'ModelCall'
  AND ended.fact -> '$.ModelCall[1].Ended' IS NOT NULL
GROUP BY task
ORDER BY output DESC, task
LIMIT 5;";
    println!("{sql}\n");
    let db = rusqlite::Connection::open(&file)?;
    let mut query = db.prepare(sql)?;
    let mut rows = query.query([])?;
    println!(
        "{:<32} {:>8} {:>10} {:>8}",
        "task", "input", "cache_read", "output"
    );
    while let Some(row) = rows.next()? {
        println!(
            "{:<32} {:>8} {:>10} {:>8}",
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        );
    }
    Ok(())
}

/// The writer: commit batches until killed.
fn write(file: &str, start: i64) -> Result<(), Box<dyn std::error::Error>> {
    let mut brain = Brain::new(Announced(Store::open(file)?), start)?;
    for n in 1.. {
        let first = brain.journal.state().next().0;
        brain.commit(n)?;
        let last = brain.journal.state().next().0 - 1;
        println!("committed {n} {first} {last}");
    }
    Ok(())
}

/// The writer's store, saying on stdout when the journal hands it a batch,
/// after the fold, so the watcher can aim its kill at the append itself.
struct Announced(Store);

impl journal::Store for Announced {
    type Error = harness_store::Error;

    fn records(&self, each: &mut dyn FnMut(journal::Record)) -> Result<(), Self::Error> {
        self.0.records(each)
    }

    fn has(&self, value: ValueId) -> Result<bool, Self::Error> {
        self.0.has(value)
    }

    fn append(
        &mut self,
        values: &[Vec<u8>],
        records: &[journal::Record],
    ) -> Result<(), Self::Error> {
        println!("appending");
        self.0.append(values, records)
    }
}

/// Just enough of a brain, and deterministic given its start time, so the
/// watcher can rebuild exactly the state the writer had at each batch.
struct Brain<S> {
    journal: Journal<S>,
    start: i64,
    /// Every task's context starts at this root.
    root: Root,
    /// The bytes the first batch stores and later ones name.
    shared: Vec<Vec<u8>>,
}

/// The one environment, empty: the demo's tasks call no tools.
const ENVIRONMENT: EnvironmentId = EnvironmentId(0xe0);
const SYSTEM: &[u8] = br#"[{"type":"text","text":"You are a helpful agent."}]"#;
const PARAMS: &[u8] = br#"{"max_tokens":32000,"stream":true}"#;

impl<S: journal::Store<Error = harness_store::Error>> Brain<S> {
    fn new(store: S, start: i64) -> Result<Brain<S>, Box<dyn std::error::Error>> {
        let (_, tools) = Binding::new(ENVIRONMENT, &[])?;
        let root = Root {
            model: "claude-opus-5-5".into(),
            system: ValueId::of(SYSTEM),
            tools: ValueId::of(tools.as_bytes()),
        };
        Ok(Brain {
            journal: Journal::open(store)?,
            start,
            root,
            shared: vec![SYSTEM.to_vec(), tools.into_bytes(), PARAMS.to_vec()],
        })
    }

    /// Commit batch `n`: [`TASKS`] tasks, each one turn from start to done.
    fn commit(&mut self, n: u64) -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = Batch::default();
        if n == 1 {
            batch.values.extend(self.shared.iter().cloned());
            batch.facts.push(Fact::Launched(Launched {
                build: "store-demo".into(),
                by: None,
            }));
            batch.facts.push(Fact::Environment(Environment {
                id: ENVIRONMENT,
                name: "chat".into(),
                limbs: vec![],
            }));
        }
        for t in 0..TASKS {
            let k = n * TASKS + t;
            let task = TaskId(id("task", k));
            let ask = format!(r#"[{{"type":"text","text":"Question {k}"}}]"#).into_bytes();
            let reply = format!(r#"[{{"type":"text","text":"Answer {k}."}}]"#).into_bytes();
            let (ask_id, reply_id) = (ValueId::of(&ask), ValueId::of(&reply));
            batch.values.extend([ask, reply]);
            batch.facts.push(self.created(task, ask_id));
            let call = ModelCallId(id("model call", k));
            batch.facts.push(Fact::ModelCall(
                call,
                ModelCallEvent::Sent(ModelCall {
                    task,
                    purpose: Purpose::Turn,
                    base: self.root.id(),
                    appends: vec![Message {
                        role: Role::User,
                        content: ask_id,
                    }],
                    renders: vec![Rendered::Note(Note::Mandate)],
                    params: ValueId::of(PARAMS),
                    breakpoints: vec![],
                }),
            ));
            batch
                .facts
                .push(Fact::ModelCall(call, ModelCallEvent::FirstByte));
            batch.facts.push(Fact::ModelCall(
                call,
                ModelCallEvent::Ended {
                    ending: Ending::Responded {
                        content: reply_id,
                        stop: Stop::EndTurn,
                    },
                    usage: Some(Usage {
                        input: 3 + n,
                        cache_read: 1000 + 40 * t,
                        cache_write_short: 40,
                        cache_write_long: 0,
                        output: 20 + (k % 7) * 10,
                    }),
                    quota: None,
                },
            ));
        }
        let at = Timestamp(self.start + 1000 * n as i64);
        self.journal
            .commit(Author::Brain, batch, at)
            .map_err(|error| format!("the demo brain's batch {n} was not committed: {error:?}"))?;
        Ok(())
    }

    /// A fresh one-turn task started by Max, its mandate the value `ask`.
    fn created(&self, task: TaskId, ask: ValueId) -> Fact {
        Fact::Task(
            task,
            TaskEvent::Created(Box::new(Created {
                name: format!("question {task}"),
                scope: Scope::Max,
                origin: Origin::Fresh {
                    agent: "assistant".into(),
                    start: Box::new(Start {
                        root: self.root.clone(),
                        binding: Binding {
                            environment: ENVIRONMENT,
                            routes: vec![],
                        },
                        observes: vec![],
                        awaits: vec![],
                    }),
                },
                mode: Mode::Autonomous,
                mandate: Mandate {
                    text: ask,
                    from: Giver::Max,
                },
            })),
        )
    }
}

/// A full-width id, so the demo exercises ids beyond SQLite's integers.
fn id(kind: &str, n: u64) -> u128 {
    let digest = Digest::of(format!("{kind} {n}").as_bytes());
    u128::from_le_bytes(digest.0[..16].try_into().unwrap())
}
