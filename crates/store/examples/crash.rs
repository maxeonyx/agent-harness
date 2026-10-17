//! Kill a brain mid-commit and show its storage survived it.
//!
//! `crash <dir>` starts a writer (this program again, as `crash write
//! <file>`), lets it commit a few hundred records, kills it with SIGKILL
//! while it keeps committing, then reopens the brain from the file.
//!
//! The writer is a brain on a fresh [`Store`]: it launches, accepts Max's
//! face and defines an environment, then includes one record of Max's face
//! after another, each creating a task and carrying its mandate's bytes.
//! Every include is one commit of two records: the face's and the brain's
//! receipt for it.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use harness_core::build::{BuildId, Launch, Launched};
use harness_core::environment::Environment;
use harness_core::fact::{Batch, Fact};
use harness_core::id::{EnvironmentId, ReplicaId, TaskId, Time, Timestamp};
use harness_core::schema::{Brain, Record};
use harness_core::task::{Created, Mandate, Mode, Origin, Scope, TaskEvent};
use harness_core::value::Bytes;
use harness_store::Store;
use harness_substrate::{Storage, What};

/// Max's tab: a follower whose records the writer makes up.
const FACE: ReplicaId = ReplicaId(0xface_0000_0000_0001);
/// Tasks the writer reports created before it is killed.
const BEFORE_KILL: u64 = 300;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["write", file] => write(file),
        [dir] => watch(dir),
        _ => Err("usage: crash <dir>".into()),
    }
}

/// The demo: run a writer, kill it mid-run, reopen, fold.
fn watch(dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let file = format!("{dir}/brain.sqlite");
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{file}{suffix}"));
    }

    let mut writer = Command::new(std::env::current_exe()?)
        .args(["write", &file])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut lines = BufReader::new(writer.stdout.take().ok_or("no stdout")?).lines();
    let mut replica = String::new();
    let mut created = 0;
    while created < BEFORE_KILL {
        let line = lines.next().ok_or("writer exited early")??;
        match line.split_once(' ') {
            Some(("replica", id)) => replica = id.to_string(),
            Some(("created", k)) => created = k.parse()?,
            _ => return Err(format!("writer said {line:?}").into()),
        }
    }
    writer.kill()?;
    writer.wait()?;
    println!("writer: brain {replica} on {file}, reported {created} tasks created");
    println!(
        "** kill -9 the writer while it commits task {}",
        created + 1
    );

    let store = Store::open(&file)?;
    let here = store.replica();
    let records = store.load()?.len();
    let (brain, divergences) = Brain::open(here, store)?;
    let state = brain.state();
    let tasks = state.tasks().count() as u64;
    let standing = brain.standing(FACE);
    println!(
        "reopened brain {here}: {records} records, head {}, {tasks} tasks, the face included up to {}",
        brain.head(),
        standing.upto
    );

    let mut ok = true;
    let mut check = |good: bool, what: &str| {
        println!("{}: {what}", if good { "OK" } else { "FAIL" });
        ok &= good;
    };
    check(
        here.to_string() == replica,
        "the replica id is the one the file minted",
    );
    check(
        divergences.is_empty(),
        "every record folds as it did when written",
    );
    check(standing.refused.is_empty(), "no face record was refused");
    check(
        tasks == standing.upto && (tasks == created || tasks == created + 1),
        "every task the writer reported is there, and at most the one it was committing",
    );
    check(
        records as u64 == 2 + 2 * tasks,
        "each commit is whole: a face's record and the brain's receipt, never one alone",
    );

    // Value bytes, read through a second connection to the same file.
    let reader = Store::open(&file)?;
    let newest = state
        .tasks()
        .max_by_key(|t| t.id)
        .ok_or("no tasks survived")?;
    let mandate = reader
        .value(newest.mandate.text)?
        .ok_or("mandate missing")?;
    println!(
        "newest task {:?}, mandate read back: {:?}",
        newest.name,
        String::from_utf8(mandate)?
    );

    // The brain carries on where it left off.
    let (mut brain, _) = Brain::open(here, Store::open(&file)?)?;
    let at = brain
        .say(now(), |_| launched())
        .map_err(|e| format!("{e:?}"))?;
    println!("relaunched: the next record is {at}");

    if !ok {
        return Err("the store did not survive the crash".into());
    }
    Ok(())
}

/// The writer: a brain including Max's face's tasks until it is killed.
fn write(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open(file)?;
    let here = store.replica();
    println!("replica {here}");
    let (mut brain, _) = Brain::open(here, store)?;
    let fail = |e| format!("{e:?}");
    brain
        .say(now(), |_| {
            let mut batch = launched();
            batch.facts.push(Fact::Face(FACE));
            batch
        })
        .map_err(fail)?;
    let role = Bytes::from("You count.");
    let params = Bytes::from("{}");
    let defined = brain
        .say(now(), |at| Batch {
            values: vec![role.clone(), params.clone()],
            facts: vec![Fact::Environment(Environment {
                id: EnvironmentId(at.event(0)),
                name: "counter".into(),
                role: role.id(),
                model: "claude-opus-5-5".into(),
                params: params.id(),
                limbs: vec![],
                places: vec![],
                starts: vec![],
                source: None,
            })],
        })
        .map_err(fail)?;
    let environment = EnvironmentId(defined.event(0));

    for k in 1.. {
        let at = Time {
            replica: FACE,
            n: k,
        };
        let mandate = Bytes::from(format!("Count to {k}."));
        let record = Record {
            at,
            clock: now(),
            what: What::Said(Batch {
                facts: vec![Fact::Task(
                    TaskId(at.event(0)),
                    TaskEvent::Created(Box::new(Created {
                        tuning: Default::default(),
                        name: format!("count {k}"),
                        by: None,
                        scope: Scope::Max,
                        origin: Origin::Fresh {
                            environment,
                            places: BTreeMap::new(),
                        },
                        mode: Mode::UserFacing,
                        mandate: Mandate {
                            text: mandate.id(),
                            brief: None,
                            quotes: vec![],
                        },
                    })),
                )],
                values: vec![mandate],
            }),
        };
        brain.include(now(), record)?;
        println!("created {k}");
    }
    Ok(())
}

fn launched() -> Batch {
    Batch {
        values: vec![],
        facts: vec![Fact::Launched(Launched {
            build: BuildId::of(b"crash-demo"),
            source: None,
            limbs: Default::default(),
            how: Launch::Started,
        })],
    }
}

fn now() -> Timestamp {
    let since = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("the clock is after 1970");
    Timestamp(since.as_millis() as i64)
}
