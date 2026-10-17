//! One of Max's devices, played by the same `Tab` the page runs as wasm:
//! a place on the face's link, over TLS checked against the dev CA.

// Shared by the examples; each uses part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};

use harness_core::delivery::{Content, Delivery, Prompt, Read};
use harness_core::draft::DraftEvent;
use harness_core::fact::{Batch, Fact};
use harness_core::id::{DeliveryId, ModelCallId, PlaceId, TaskId, Timestamp};
use harness_core::task::{Drain, Stopper, TaskEvent};
use harness_core::value::Bytes;
use harness_face_tab::{Down, FaceKey, FaceRow, Tab, Up};
use harness_substrate::Status;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde_json::json;
use tungstenite::{Message, WebSocket};

const HOST: &str = "mx-hv-mj.local";

pub fn now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    )
}

/// A user message's content blocks: one text block.
pub fn blocks(text: &str) -> Bytes {
    Bytes::from(json!([{ "type": "text", "text": text }]).to_string())
}

pub fn status(device: &Device, n: u64) -> String {
    match device.tab.status(n) {
        Status::Pending => "is pending".into(),
        Status::Included => "is included".into(),
        Status::Refused(why) => format!("is refused, in its own standing: {why}"),
    }
}

type Tls = StreamOwned<ClientConnection, TcpStream>;

/// One of Max's devices: a tab, as a place, on its own link.
pub struct Device {
    pub name: &'static str,
    pub tab: Tab,
    socket: WebSocket<Tls>,
}

impl Device {
    pub fn connect(name: &'static str, port: u16) -> Device {
        let place = PlaceId(u64::from_le_bytes(random()));
        let tab = Tab::open(place, Vec::new());
        let socket = link(place, port).expect("the face links");
        println!("  {name}: tab {place} linked");
        Device { name, tab, socket }
    }

    /// A new link after the brain restarted: ship what it has not included.
    pub fn reconnect(&mut self, port: u16) {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            match link(self.tab.here(), port) {
                Ok(socket) => {
                    self.socket = socket;
                    break;
                }
                Err(_) if Instant::now() < until => thread::sleep(Duration::from_millis(300)),
                Err(e) => panic!("{}: cannot link again: {e}", self.name),
            }
        }
        self.pump_until(|d| d.tab.view().is_some());
        for record in self.tab.unsent().expect("the tab's log is whole") {
            self.ship(Up::Record(record));
        }
        println!("  {}: linked again", self.name);
    }

    pub fn ship(&mut self, up: Up) {
        let text = serde_json::to_string(&up).unwrap();
        self.socket.send(Message::text(text)).unwrap();
    }

    /// Say a batch as Max at this tab, and ship it; its time.
    pub fn say(&mut self, batch: Batch) -> u64 {
        let record = self
            .tab
            .say(now(), &serde_json::to_string(&batch).unwrap())
            .unwrap();
        let n = record.at.n;
        self.ship(Up::Record(record));
        n
    }

    /// A new message to `session`, sent at once.
    pub fn message(&mut self, session: TaskId, text: &str) -> u64 {
        let draft = DeliveryId(self.tab.next().event(0));
        let words = blocks(text);
        println!("  {}: Max sends {text:?}", self.name);
        self.say(Batch {
            values: vec![words.clone()],
            facts: vec![
                Fact::Draft(draft, DraftEvent::Opened { session }),
                Fact::Draft(
                    draft,
                    DraftEvent::Send {
                        text: words.id(),
                        read: Read::Next,
                    },
                ),
            ],
        })
    }

    pub fn done(&mut self, session: TaskId) -> u64 {
        let id = DeliveryId(self.tab.next().event(0));
        println!("  {}: Max presses Done", self.name);
        self.say(Batch {
            values: Vec::new(),
            facts: vec![Fact::Delivery(Delivery {
                id,
                to: session,
                content: Content::Prompt(Prompt::Done),
                read: Read::Next,
            })],
        })
    }

    /// Max stops `task`, giving it `within` to finish.
    pub fn stop(&mut self, task: TaskId, within: Duration) -> u64 {
        println!(
            "  {}: Max presses Stop, with {within:?} to finish",
            self.name
        );
        self.say(Batch {
            values: Vec::new(),
            facts: vec![Fact::Task(
                task,
                TaskEvent::Drain(Drain::Stop {
                    by: Stopper::Max,
                    within: within.as_millis() as u64,
                }),
            )],
        })
    }

    /// Take in what the brain ships until `done` holds.
    pub fn pump_until(&mut self, done: impl Fn(&Device) -> bool) {
        let until = Instant::now() + Duration::from_secs(120);
        while !done(self) {
            assert!(Instant::now() < until, "{}: waited too long", self.name);
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let down: Down = serde_json::from_str(&text).expect("the brain's rows");
                    self.tab.down(down);
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(e) => panic!("{}: the link: {e}", self.name),
            }
        }
    }

    pub fn rows(&self) -> &BTreeMap<FaceKey, FaceRow> {
        &self.tab.view().expect("a view").rows
    }

    /// The chat: the only task in Max's scope.
    pub fn session(&self) -> Option<TaskId> {
        self.tab.view()?;
        self.rows().keys().find_map(|k| match k {
            FaceKey::Task(id) => Some(*id),
            _ => None,
        })
    }

    /// The task Max started: the one no task owns.
    pub fn root(&self) -> Option<TaskId> {
        self.tab.view()?;
        self.rows().iter().find_map(|(k, r)| match (k, r) {
            (FaceKey::Task(id), FaceRow::Task(row)) if row["owner"].is_null() => Some(*id),
            _ => None,
        })
    }

    pub fn turn(&self, task: TaskId) -> Option<ModelCallId> {
        match self.rows().get(&FaceKey::Task(task))? {
            FaceRow::Task(row) => row["turn"].as_str()?.parse().ok(),
            _ => None,
        }
    }

    pub fn draft(&self, id: DeliveryId) -> Option<String> {
        match self.rows().get(&FaceKey::Draft(id))? {
            FaceRow::Draft(row) => Some(row.text.clone().unwrap_or_default()),
            _ => None,
        }
    }
}

fn random() -> [u8; 8] {
    let mut bytes = [0; 8];
    use std::io::Read as _;
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    bytes
}

/// A tab's link to the face at `port`, over TLS checked against the dev CA.
fn link(place: PlaceId, port: u16) -> Result<WebSocket<Tls>, String> {
    let home = env::var("HOME").unwrap();
    let ca = Path::new(&home).join(".config/dev-certs/shared-local/ca/dev-ca.crt");
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_file(&ca).expect("the dev CA"))
        .unwrap();
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    let tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let name = ServerName::try_from(HOST).unwrap();
    let tls = StreamOwned::new(ClientConnection::new(Arc::new(config), name).unwrap(), tcp);
    let url = format!("wss://{HOST}:{port}/link?place={place}");
    let mut attempt = tungstenite::client::client(url.as_str(), tls);
    loop {
        match attempt {
            Ok((socket, _)) => return Ok(socket),
            Err(tungstenite::HandshakeError::Interrupted(mid)) => attempt = mid.handshake(),
            Err(e) => return Err(format!("the link's handshake: {e}")),
        }
    }
}
