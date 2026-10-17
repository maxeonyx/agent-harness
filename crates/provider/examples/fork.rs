//! A root, a two-turn conversation, then two forks of its last response,
//! the second sent at the first's FirstByte. Prints each request's facts,
//! its text as it streams, its cache reads and writes and the plan's usage.

use std::collections::HashMap;
use std::io::Write;
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Message, Role, Root};
use harness_core::id::{ReplicaId, TaskId, ValueId};
use harness_core::model_call::{Ending, ModelCall};
use harness_provider::{Client, Finished, IDENTITY, Login, Progress, resolve};
use harness_runtime::Cancel;
use serde_json::json;

/// What the journal and the value store hold, in miniature: the entries the
/// fold has learnt and the bytes they name.
/// The replica this example's ids are of: it records nothing.
const EXAMPLE: ReplicaId = ReplicaId(1);

#[derive(Default)]
struct Store {
    entries: HashMap<EntryId, Entry>,
    values: HashMap<ValueId, Vec<u8>>,
}

impl Store {
    fn value(&mut self, bytes: Vec<u8>) -> ValueId {
        let id = ValueId::of(&bytes);
        self.values.insert(id, bytes);
        id
    }

    /// The context ending at `head`, root first, as `State::context` gives it.
    fn context(&self, head: EntryId) -> Vec<(EntryId, &Entry)> {
        let mut chain = Vec::new();
        let mut next = Some(head);
        while let Some(id) = next {
            let entry = &self.entries[&id];
            chain.push((id, entry));
            next = entry.parent();
        }
        chain.reverse();
        chain
    }

    /// Records what a fold learns from a request that responded: the
    /// entries it appended, and its response.
    fn learn(&mut self, call: &ModelCall, finished: Finished) -> EntryId {
        let Ending::Responded { content, .. } = finished.ending else {
            unreachable!("only responses are learnt")
        };
        self.values.extend(finished.value.map(|v| (content, v)));
        self.entries.extend(call.entries());
        let response = call.response(content);
        let id = response.id();
        self.entries.insert(id, response);
        id
    }
}

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "haiku".into());
    let client = Client::new(Login::claude_code().unwrap_or_else(|e| fail(&e.to_string())));
    let models = client
        .models(&Cancel::new())
        .unwrap_or_else(|e| fail(&format!("listing the models: {e}")));
    let model = resolve(&name, models.iter().map(|m| m.id.as_str()))
        .unwrap_or_else(|| fail(&format!("the provider lists no model {name}")))
        .to_string();
    println!("model: {model}");
    let start = Instant::now();
    let mut store = Store::default();
    let params = store.value(br#"{"max_tokens":2048}"#.to_vec());

    let root = root(&mut store, &model);
    let root = {
        let id = root.id();
        store.entries.insert(id, Entry::Root(root));
        id
    };
    let ask = |store: &mut Store, base, text: &str, marks: &[EntryId]| {
        let content = json!([{ "type": "text", "text": text }]).to_string();
        let message = Message {
            role: Role::User,
            content: store.value(content.into_bytes()),
        };
        let head = message.id_after(base);
        let breakpoints = marks.iter().chain([&head]).copied().collect();
        ModelCall {
            model: None,
            task: TaskId(EXAMPLE.origin().event(0)),
            base,
            appends: vec![message],
            renders: vec![],
            params,
            breakpoints,
        }
    };

    let q1 = ask(
        &mut store,
        root,
        "Which three brigs carried the most cargo? One short line each.",
        &[root],
    );
    let a1 = turn(&client, &store, start, "turn 1", &q1, None);
    let a1 = store.learn(&q1, a1);
    let q2 = ask(
        &mut store,
        a1,
        "And which three carried the least? One short line each.",
        &[root],
    );
    let a2 = turn(&client, &store, start, "turn 2", &q2, None);
    let a2 = store.learn(&q2, a2);

    let f1 = ask(
        &mut store,
        a2,
        "Fork 1: which three ports appear most often? One short line each.",
        &[root, a2],
    );
    let f2 = ask(
        &mut store,
        a2,
        "Fork 2: which three cargoes are most common? One short line each.",
        &[root, a2],
    );
    let (first_byte, at_first_byte) = mpsc::channel();
    let forks = std::thread::scope(|s| {
        let (store, client) = (&store, &client);
        let one = s.spawn(|| turn(client, store, start, "fork 1", &f1, Some(first_byte)));
        let _ = at_first_byte.recv();
        let two = s.spawn(|| turn(client, store, start, "fork 2", &f2, None));
        [one.join(), two.join()]
    });
    for (call, fork) in [&f1, &f2].into_iter().zip(forks) {
        store.learn(call, fork.unwrap_or_else(|_| fail("a fork panicked")));
    }
    println!(
        "[{:6.2}s] done: {} entries and {} values stored",
        secs(start),
        store.entries.len(),
        store.values.len()
    );
}

/// Sends `call` and prints its progress; how it ended.
fn turn(
    client: &Client,
    store: &Store,
    start: Instant,
    label: &str,
    call: &ModelCall,
    mut first_byte: Option<mpsc::Sender<()>>,
) -> Finished {
    let mut line = String::new();
    println!("[{:6.2}s] {label}: Sent", secs(start));
    let finished = client
        .send(
            call,
            &store.context(call.base),
            |id| store.values.get(&id).cloned(),
            &Cancel::new(),
            |progress| {
                match progress {
                    Progress::FirstByte => {
                        println!("[{:6.2}s] {label}: FirstByte", secs(start));
                        if let Some(tx) = first_byte.take() {
                            let _ = tx.send(());
                        }
                    }
                    Progress::Text(chunk) => {
                        line.push_str(&chunk);
                        while let Some(n) = line.find('\n') {
                            println!("[{:6.2}s] {label} > {}", secs(start), &line[..n]);
                            line.drain(..=n);
                        }
                    }
                }
                let _ = std::io::stdout().flush();
            },
        )
        .unwrap_or_else(|e| fail(&format!("{label}: {e}")));
    if !line.is_empty() {
        println!("[{:6.2}s] {label} > {line}", secs(start));
    }
    let quota = finished.quota.clone().unwrap_or_else(|| fail("no quota"));
    let usage = finished.usage.unwrap_or_default();
    println!(
        "[{:6.2}s] {label}: Ended  cache_read={} cache_write={} input={} output={}  5h={:?} 7d={:?}",
        secs(start),
        usage.cache_read,
        usage.cache_write_short + usage.cache_write_long,
        usage.input,
        usage.output,
        quota.five_hour.map(|w| w.used),
        quota.seven_day.map(|w| w.used),
    );
    match &finished.ending {
        Ending::Responded { stop, .. } => println!("[{:6.2}s] {label}: stop={stop:?}", secs(start)),
        ending => fail(&format!("{label}: {ending:?}")),
    }
    finished
}

/// A root whose system carries a ledger of about 3k tokens, unique to this
/// run so no earlier run's cache is read.
fn root(store: &mut Store, model: &str) -> Root {
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
        system: store.value(system.to_string().into_bytes()),
        tools: store.value(b"[]".to_vec()),
    }
}

fn secs(start: Instant) -> f64 {
    start.elapsed().as_secs_f64()
}

fn fail(message: &str) -> ! {
    eprintln!("FAILED: {message}");
    std::process::exit(1)
}
