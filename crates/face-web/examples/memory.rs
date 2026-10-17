//! The face over a brain in memory, for trying the page without a model:
//! `cargo run -p harness-face-web --example memory -- <port>`, then open
//! `https://mx-hv-mj.local:<port>`. The brain includes what each tab says
//! and folds it; nothing runs, so a session waits on Max for ever.

use std::collections::HashMap;
use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use harness_core::environment::Environment;
use harness_core::fact::{Batch, Fact, Launched};
use harness_core::id::{EnvironmentId, ReplicaId, Timestamp, ValueId};
use harness_core::schema::{Brain, Record};
use harness_core::value::Bytes;
use harness_face_web::{Face, Folded, Source, Wiring};
use harness_substrate::{Memory, What};

#[derive(Clone, Default)]
struct Values(Arc<Mutex<HashMap<ValueId, Vec<u8>>>>);

impl Values {
    fn keep(&self, batch: &Batch) {
        let mut values = self.0.lock().unwrap();
        for v in &batch.values {
            values.insert(v.id(), v.0.clone());
        }
    }
}

impl Source for Values {
    fn value(&self, id: ValueId) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(&id).cloned()
    }
}

fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    )
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .map_or(4329, |p| p.parse().expect("a port"));
    let values = Values::default();
    let (mut brain, _) = Brain::open(ReplicaId(0xb4a1), Memory::default()).unwrap();
    let launched = Batch {
        values: vec![],
        facts: vec![Fact::Launched(Launched {
            build: "memory".into(),
            by: None,
        })],
    };
    brain.say(now(), |_| launched).unwrap();
    let (role, params) = (Bytes::from("You talk with Max."), Bytes::from("{}"));
    let defined = |at: harness_core::id::Time| Batch {
        facts: vec![Fact::Environment(Environment {
            id: EnvironmentId(at.event(0)),
            name: "chat".into(),
            role: role.id(),
            model: "claude-opus-5-5".into(),
            params: params.id(),
            limbs: vec![],
            places: vec![],
            starts: vec![],
            source: None,
        })],
        values: vec![role.clone(), params.clone()],
    };
    values.keep(&defined(brain.head()));
    brain.say(now(), defined).unwrap();

    let (records, inbox) = mpsc::channel::<Record>();
    let (folds, followed) = mpsc::channel::<Folded>();
    let fold = |brain: &Brain<Memory<_>>| Folded {
        state: brain.state().clone(),
        as_of: brain.head(),
        standing: brain.standings().clone(),
    };
    folds.send(fold(&brain)).unwrap();
    let keeping = values.clone();
    thread::spawn(move || {
        for record in inbox {
            let replica = record.at.replica;
            if !brain.state().is_face(replica) {
                brain
                    .say(now(), |_| Batch {
                        values: vec![],
                        facts: vec![Fact::Face(replica)],
                    })
                    .unwrap();
            }
            if let What::Said(batch) = &record.what {
                keeping.keep(batch);
            }
            match brain.include(now(), record) {
                Ok(Some(_)) => {
                    let _ = folds.send(fold(&brain));
                }
                Ok(None) => {}
                Err(e) => eprintln!("memory brain: {e:?}"),
            }
        }
    });

    let include = Mutex::new(records);
    let face = Face::with(
        values,
        Wiring {
            include: Some(Box::new(move |r| {
                let _ = include.lock().unwrap().send(r);
            })),
            ..Wiring::default()
        },
    );
    let certs =
        PathBuf::from(std::env::var("HOME").unwrap()).join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .unwrap();
    let listener = TcpListener::bind(("0.0.0.0", port)).unwrap();
    println!("face: https://mx-hv-mj.local:{port}");
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    thread::scope(|s| {
        s.spawn(|| face.follow(followed));
        harness_face_web::serve(&face, listener, tls, allow).unwrap();
    });
}
