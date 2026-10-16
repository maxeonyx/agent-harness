//! A root, a two-turn conversation, then two forks of its last response,
//! the second sent at the first's FirstByte. Prints each request's facts,
//! its text as it streams, its cache reads and writes and its billing claim.

use std::collections::HashMap;
use std::io::Write;
use std::ops::ControlFlow;
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Message, Role, Root, Wire};
use harness_core::id::{ModelCallId, TaskId};
use harness_core::live::Live;
use harness_core::model_call::{Breakpoint, Ending, ModelCall, ModelCallEvent, Purpose, Ttl};
use harness_provider::{Client, IDENTITY, Login, Progress};
use serde_json::json;

type Store = HashMap<EntryId, Entry>;

fn main() {
    let model = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "claude-sonnet-5".into());
    let client = Client::new(Login::claude_code().unwrap_or_else(|e| fail(&e.to_string())));
    let start = Instant::now();
    let mut store = Store::new();

    let root = put(&mut store, Entry::Root(root(&model)));
    let ask = |store: &mut Store, parent, text: &str| {
        let content = json!([{ "type": "text", "text": text }]).to_string();
        put(store, message(parent, Role::User, Wire(content)))
    };
    let q1 = ask(
        &mut store,
        root,
        "Which three brigs carried the most cargo? One short line each.",
    );
    let a1 = turn(&client, &store, start, "turn 1", q1, &[root, q1], None);
    let a1 = put(&mut store, a1);
    let q2 = ask(
        &mut store,
        a1,
        "And which three carried the least? One short line each.",
    );
    let a2 = turn(&client, &store, start, "turn 2", q2, &[root, q2], None);
    let a2 = put(&mut store, a2);

    let f1 = ask(
        &mut store,
        a2,
        "Fork 1: which three ports appear most often? One short line each.",
    );
    let f2 = ask(
        &mut store,
        a2,
        "Fork 2: which three cargoes are most common? One short line each.",
    );
    let (first_byte, at_first_byte) = mpsc::channel();
    let forks = std::thread::scope(|s| {
        let store = &store;
        let client = &client;
        let one = s.spawn(move || {
            turn(
                client,
                store,
                start,
                "fork 1",
                f1,
                &[root, a2, f1],
                Some(first_byte),
            )
        });
        let _ = at_first_byte.recv();
        let two = s.spawn(move || turn(client, store, start, "fork 2", f2, &[root, a2, f2], None));
        [one.join(), two.join()]
    });
    for fork in forks {
        put(&mut store, fork.unwrap_or_else(|_| fail("a fork panicked")));
    }
    println!(
        "[{:6.2}s] done: {} entries stored",
        secs(start),
        store.len()
    );
}

/// One request at `head` with breakpoints at `marks`; its response entry.
fn turn(
    client: &Client,
    store: &Store,
    start: Instant,
    label: &str,
    head: EntryId,
    marks: &[EntryId],
    mut first_byte: Option<mpsc::Sender<()>>,
) -> Entry {
    let call = ModelCall {
        task: TaskId(1),
        purpose: Purpose::Turn,
        head,
        renders: vec![],
        params: Wire(r#"{"max_tokens":2048,"stream":true}"#.into()),
        breakpoints: marks
            .iter()
            .map(|&at| Breakpoint {
                at,
                ttl: Ttl::Short,
            })
            .collect(),
    };
    let mut line = String::new();
    let finished = client
        .send(
            ModelCallId(0),
            &call,
            |id| store.get(&id).cloned(),
            |progress| {
                match progress {
                    Progress::Fact(ModelCallEvent::Sent(_)) => {
                        println!("[{:6.2}s] {label}: Sent", secs(start))
                    }
                    Progress::Fact(ModelCallEvent::FirstByte) => {
                        println!("[{:6.2}s] {label}: FirstByte", secs(start));
                        if let Some(tx) = first_byte.take() {
                            let _ = tx.send(());
                        }
                    }
                    Progress::Fact(_) => {}
                    Progress::Live(Live::Model { event, .. }) => {
                        let event: serde_json::Value = serde_json::from_str(&event.0).unwrap();
                        if let Some(text) = event["delta"]["text"].as_str() {
                            line.push_str(text);
                            while let Some(n) = line.find('\n') {
                                println!("[{:6.2}s] {label} > {}", secs(start), &line[..n]);
                                line.drain(..=n);
                            }
                        }
                    }
                    Progress::Live(_) => {}
                }
                let _ = std::io::stdout().flush();
                ControlFlow::Continue(())
            },
        )
        .unwrap_or_else(|e| fail(&format!("{label}: {e}")));
    if !line.is_empty() {
        println!("[{:6.2}s] {label} > {line}", secs(start));
    }
    let quota = finished.quota.unwrap_or_else(|| fail("no quota"));
    let usage = finished.usage.unwrap_or_default();
    println!(
        "[{:6.2}s] {label}: Ended  cache_read={} cache_write={} input={} output={}  claim={} 5h={:?} 7d={:?}",
        secs(start),
        usage.cache_read,
        usage.cache_write_short + usage.cache_write_long,
        usage.input,
        usage.output,
        quota.claim.as_deref().unwrap_or("none"),
        quota.five_hour,
        quota.seven_day,
    );
    match finished.result {
        Ok(reply) => {
            let Ending::Responded { stop, .. } = reply.ending(vec![]) else {
                unreachable!()
            };
            println!("[{:6.2}s] {label}: stop={stop:?}", secs(start));
            Entry::Message(reply.message)
        }
        Err(ending) => fail(&format!("{label}: {ending:?}")),
    }
}

/// A root whose system carries a ledger of about 3k tokens, unique to this
/// run so no earlier run's cache is read.
fn root(model: &str) -> Root {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ports = ["Lyttelton", "Napier", "Nelson", "Timaru", "Bluff", "Picton"];
    let mut ledger = format!("Shipping ledger {nonce}. Answer questions about it briefly.\n");
    for i in 0..200 {
        ledger.push_str(&format!(
            "1887-{:02}-{:02}: the {} brig No. {i} left {} with {} tons of {}.\n",
            1 + i % 12,
            1 + i * 7 % 28,
            ["red", "grey", "swift", "old"][i % 4],
            ports[i * 5 % 6],
            100 + i * 37 % 900,
            ["wool", "flax", "timber", "coal", "grain"][i % 5],
        ));
    }
    let system = json!([
        { "type": "text", "text": IDENTITY },
        { "type": "text", "text": ledger },
    ]);
    Root {
        model: model.into(),
        system: Wire(system.to_string()),
        tools: Wire("[]".into()),
    }
}

fn message(parent: EntryId, role: Role, content: Wire) -> Entry {
    Entry::Message(Message {
        parent,
        role,
        content,
    })
}

fn put(store: &mut Store, entry: Entry) -> EntryId {
    let id = entry.id();
    store.insert(id, entry);
    id
}

fn secs(start: Instant) -> f64 {
    start.elapsed().as_secs_f64()
}

fn fail(message: &str) -> ! {
    eprintln!("FAILED: {message}");
    std::process::exit(1)
}
