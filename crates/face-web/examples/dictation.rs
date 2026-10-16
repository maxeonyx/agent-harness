//! The compose box and dictation, on a journal with one task waiting on Max.
//!
//! The journal is in memory. Its one task has ended its turn, so it waits on
//! Max. The words he sends from the page are folded and appended as his
//! delivery, the way the brain will do it, so the page's journal shows them
//! arrive and the task leaves his queue. Dictation is relayed on the
//! claude.ai login, and the page offers `demos/face-dictation/sample.wav` as
//! a microphone for a machine that has none.
//!
//! Run: `cargo run -p harness-face-web --example dictation [port]`.

use std::collections::HashMap;
use std::net::{IpAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Message, Role, Root, Wire};
use harness_core::delivery::{Content, Delivery};
use harness_core::environment::{Binding, Environment, Fork, Host, Limb, LimbKind, Route};
use harness_core::id::{
    DeliveryId, EnvironmentId, FaceId, LimbId, ModelCallId, Seq, TaskId, Timestamp,
};
use harness_core::journal::{Author, Fact, Launched, Record};
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Purpose, Stop};
use harness_core::state::State;
use harness_core::task::{Created, Giver, Mandate, Mode, Origin, Scope, TaskEvent};
use harness_face_web::{Compose, Deliver, Dictation, Face, Source, describe};
use harness_provider::Login;
use serde_json::json;

#[derive(Clone)]
struct Journal(Arc<(Mutex<Folded>, Condvar)>);

#[derive(Default)]
struct Folded {
    state: State,
    records: Vec<Record>,
    entries: HashMap<EntryId, Entry>,
    next: u128,
}

impl Journal {
    fn lock(&self) -> std::sync::MutexGuard<'_, Folded> {
        self.0.0.lock().unwrap()
    }

    /// Fold the fact, and append it only if the fold accepts it.
    fn push(&self, by: Author, fact: Fact) -> Result<(), String> {
        let mut j = self.lock();
        let record = Record {
            seq: Seq(j.records.len() as u64 + 1),
            at: now(),
            by,
            fact,
        };
        j.state
            .apply(&record)
            .map_err(|r| format!("refused: {r:?}"))?;
        println!("{:>4}  {}", record.seq.0, describe(&j.state, &record));
        j.records.push(record);
        drop(j);
        self.0.1.notify_all();
        Ok(())
    }

    fn id(&self) -> u128 {
        let mut j = self.lock();
        j.next += 1;
        j.next
    }

    fn store(&self, entry: Entry) -> EntryId {
        let id = entry.id();
        self.lock().entries.insert(id, entry);
        id
    }
}

impl Source for Journal {
    fn read(&self, after: Seq, wait: Duration) -> Vec<Record> {
        let j = self.lock();
        let (j, _) = self
            .0
            .1
            .wait_timeout_while(j, wait, |j| j.records.len() as u64 <= after.0)
            .unwrap();
        j.records[(after.0 as usize).min(j.records.len())..].to_vec()
    }

    fn entry(&self, id: EntryId) -> Option<Entry> {
        self.lock().entries.get(&id).cloned()
    }
}

/// What the brain will do with Max's words: make them his delivery.
impl Deliver for Journal {
    fn words(&self, to: TaskId, seen: Seq, words: Wire) -> Result<(), String> {
        let shown = words.0.clone();
        let delivery = Delivery {
            id: DeliveryId(self.id()),
            to,
            content: Content::Words(words),
        };
        let by = Author::Max {
            face: FaceId(1),
            seen,
        };
        self.push(by, Fact::Delivery(delivery))?;
        println!("      {shown}");
        Ok(())
    }
}

/// A task that has had its turn and waits on Max.
fn script(j: &Journal) -> Result<(), String> {
    let brain = |fact| j.push(Author::Brain, fact);
    brain(Fact::Launched(Launched {
        build: "face-dictation-demo".into(),
        by: None,
    }))?;
    let (max, env) = (LimbId(j.id()), EnvironmentId(j.id()));
    brain(Fact::Limb(Limb {
        id: max,
        name: "max".into(),
        host: Host::Face,
        kind: LimbKind::Max,
        fork: Fork::Share,
    }))?;
    brain(Fact::Environment(Environment {
        id: env,
        name: "chat".into(),
        limbs: vec![max],
    }))?;
    let root = j.store(Entry::Root(Root {
        model: "claude-sonnet-5".into(),
        system: Wire(json!("You are talking with Max.").to_string()),
        tools: Wire("[]".into()),
    }));
    let task = TaskId(j.id());
    let created = Created {
        name: "chat".into(),
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
            environment: env,
            routes: vec![Route {
                tool: "ask".into(),
                limb: max,
            }],
        },
        observes: Vec::new(),
        awaits: Vec::new(),
    };
    brain(Fact::Task(task, TaskEvent::Created(Box::new(created))))?;
    let mc = ModelCallId(j.id());
    brain(Fact::ModelCall(
        mc,
        ModelCallEvent::Sent(ModelCall {
            task,
            purpose: Purpose::Turn,
            head: root,
            renders: Vec::new(),
            params: Wire(r#"{"max_tokens":32000}"#.into()),
            breakpoints: Vec::new(),
        }),
    ))?;
    brain(Fact::ModelCall(mc, ModelCallEvent::FirstByte))?;
    let said = json!([{"type": "text", "text": "Ready when you are. Type, paste or dictate."}]);
    let entry = j.store(Entry::Message(Message {
        parent: root,
        role: Role::Assistant,
        content: Wire(said.to_string()),
    }));
    brain(Fact::ModelCall(
        mc,
        ModelCallEvent::Ended {
            ending: Ending::Responded {
                entry,
                stop: Stop::EndTurn,
                calls: Vec::new(),
            },
            usage: None,
            quota: None,
        },
    ))
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .map_or(4322, |p| p.parse().expect("a port number"));
    let home = PathBuf::from(std::env::var("HOME").expect("HOME is set"));
    let certs = home.join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .expect("the shared wildcard certificate");
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/face-dictation/sample.wav");
    let sample = read_wav(&sample).expect("the sample");
    let login = Login::claude_code().expect("a home directory");

    let journal = Journal(Arc::default());
    script(&journal).expect("the script folds");
    let compose = Compose {
        deliver: Some(Box::new(journal.clone())),
        dictation: Some(Dictation::new(login).with_sample(sample)),
    };
    let face = Face::with(journal, compose);
    let follower = Arc::clone(&face);
    thread::spawn(move || follower.follow());
    let listener = TcpListener::bind(("0.0.0.0", port)).expect("the port is free");
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    println!("\nface: https://mx-hv-mj.local:{port}\n");
    harness_face_web::serve(face, listener, tls, allow).expect("the listener");
}

fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Timestamp(ms as i64)
}

/// The PCM of a 16 kHz mono 16-bit WAV file.
fn read_wav(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut format_ok = false;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                let field = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
                let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                format_ok = field(0) == 1 && field(2) == 1 && rate == 16_000 && field(14) == 16;
            }
            b"data" if format_ok => return Ok(body.to_vec()),
            b"data" => return Err("the WAV must be 16 kHz mono 16-bit PCM".into()),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    Err("the WAV has no data chunk".into())
}
