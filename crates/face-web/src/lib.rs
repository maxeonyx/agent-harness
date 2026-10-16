//! The harness's web face: Max's work as a map of tasks, live from the
//! journal, each opening into the log of what it did.
//!
//! A face is a reader of the journal and nothing else. It folds the records
//! it is given with [`State::apply`], the same fold the brain runs, and
//! sends the views the core derives: every task, the scope that owns it and
//! what it waits on ([`State::waiting_on`], the throbber), Max's queue
//! ([`State::queue`]) and a task's context. It keeps no state of its own
//! that could disagree with the journal.
//!
//! **The page** (`page.html`) is the map. A task is a node; a scope (the
//! spawn call that owns its members) is a junction under its owner; the
//! projects are the map's disconnected components. A node opens into its
//! log: its context, exactly as sent, rendered, with what is streaming now
//! and Max's words it has not read yet. Moving the map closes it.
//!
//! **The cursor is the journal position.** The page's event stream is one
//! event per fold: the whole view, with the [`Seq`] it reached as the event
//! id. The browser resends the last id it got when it reconnects, and is
//! sent the next view once the journal is past it, so a view is never sent
//! twice and never missed. `/view` is the latest view alone, for a client
//! that only looks.
//!
//! **A context is a value.** It is fetched by its head entry
//! (`/context/<entry>`), read off the fold ([`State::context`]) with the
//! bytes each entry names from the [`Source`], and when each response was
//! sent and ended and the context it was sent with, which never change; so
//! the browser caches it forever and refetches only when the task's head
//! moves.
//!
//! **Max's facts.** Everything Max does from the page is a batch of his
//! facts in the core's own encoding, posted to `/act` and passed to [`Act`],
//! which the brain implements: his words (a delivery from the compose box,
//! where Enter is a new line and Ctrl+Enter sends), his controls over a task
//! (interrupt, stop, hold), a new order for his queue, and the task he is
//! focused on. The bytes his facts name (his words) travel in the batch, and
//! the page names them with `/digest`, the core's [`ValueId::of`]. The face
//! writes nothing and judges nothing: the brain commits the batch as his,
//! and the core's fold decides what Max may author, so a refusal comes back
//! with the core's reason. Each page is its own face, with an id it keeps,
//! and mints the ids of what it creates, so a post retried after a dropped
//! connection cannot deliver twice.
//!
//! **Live progress.** A running request's streamed events and a running
//! command's output are not facts; the brain puts them in a [`Feed`], and
//! `/live?model=<id>` or `/live?call=<id>` streams them with their count as
//! the event id, so a reconnect resumes where it left off.
//!
//! **Usage and trouble.** The subscription's 5-hour and 7-day use is the
//! latest response's report, and why a task's last turn did not respond is
//! that turn's ending: both are read off the fold, never kept.
//!
//! The box takes dictation too ([`Dictation`]): the page's microphone,
//! relayed to Anthropic's speech-to-text endpoint, the transcript arriving in
//! the box as editable text.
//!
//! Transport is std's TCP, rustls and one static page with no build step;
//! the dictation socket is a WebSocket (tungstenite, without its TLS).

mod dictation;
mod http;
mod live;
mod view;

use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId};
use harness_core::id::{CallId, FaceId, ModelCallId, Seq, Timestamp, ValueId};
use harness_core::journal::{Batch, Record};
use harness_core::state::State;
use serde_json::{Value, json};

pub use dictation::{Dictation, Edit, Socket, edits};
pub use live::{Feed, Running};
pub use view::describe;

/// What a face reads: the journal, and the values its facts name.
///
/// The store implements this. A face never writes.
pub trait Source: Send + Sync + 'static {
    /// The records after `after`, in order, waiting up to `wait` for at
    /// least one. Empty if none came.
    fn read(&self, after: Seq, wait: Duration) -> Vec<Record>;

    /// A value's bytes, if stored.
    fn value(&self, id: ValueId) -> Option<Vec<u8>>;
}

/// Where Max's facts go: the brain, the journal's only writer.
pub trait Act: Send + Sync + 'static {
    /// Commit `batch` authored [`Author::Max`] by `face`, which had shown
    /// him the journal up to `seen`. An `Err` is why it was not committed
    /// (the core's [`harness_core::journal::Rejected`], by name), shown to
    /// Max.
    fn act(&self, face: FaceId, seen: Seq, batch: Batch) -> Result<(), String>;
}

/// What a face is wired to. With nothing, it only watches.
#[derive(Default)]
pub struct Wiring {
    pub act: Option<Box<dyn Act>>,
    pub dictation: Option<Dictation>,
    /// Live progress, from whoever runs things.
    pub feed: Feed,
}

/// A face over one journal: folds it as it grows and serves the page.
///
/// Only the follower writes `folded`, and holds it only while folding in
/// memory; viewers read it alongside one another. Nothing reads the store
/// while holding either lock.
pub struct Face<S> {
    source: S,
    wiring: Wiring,
    folded: RwLock<Folded>,
    shown: Mutex<Shown>,
    grew: Condvar,
}

struct Folded {
    state: State,
    /// A record the fold refused: this face's build disagrees with the
    /// brain's, and the views stop at the record before it.
    refused: Option<String>,
}

/// What every viewer is sent: the page's view, rendered once per fold, and
/// the last record it includes.
struct Shown {
    view: Arc<String>,
    seq: Seq,
}

/// How often an idle event stream is written to, so a dead one is noticed.
const HEARTBEAT: Duration = Duration::from_secs(15);

impl<S: Source> Face<S> {
    pub fn new(source: S) -> Arc<Face<S>> {
        Face::with(source, Wiring::default())
    }

    /// A face that acts, dictates and shows live progress as `wiring` allows.
    pub fn with(source: S, wiring: Wiring) -> Arc<Face<S>> {
        let state = State::default();
        let view = Arc::new(render(&state, &|_| None, None));
        Arc::new(Face {
            source,
            wiring,
            folded: RwLock::new(Folded {
                state,
                refused: None,
            }),
            shown: Mutex::new(Shown { view, seq: Seq(0) }),
            grew: Condvar::new(),
        })
    }

    /// Fold the journal as it grows. Runs until a record is refused.
    pub fn follow(&self) {
        let mut last = Seq(0);
        loop {
            let records = self.source.read(last, HEARTBEAT);
            if records.is_empty() {
                continue;
            }
            {
                let mut folded = self.folded.write().unwrap_or_else(|e| e.into_inner());
                for record in &records {
                    if let Err(rejected) = folded.state.apply(record) {
                        let refused = format!("record {} refused: {rejected:?}", record.seq.0);
                        folded.refused = Some(refused);
                        break;
                    }
                }
            }
            let folded = self.read();
            let value = |id| self.source.value(id);
            let view = Arc::new(render(&folded.state, &value, folded.refused.as_deref()));
            last = folded.state.last().unwrap_or(Seq(0));
            let stop = folded.refused.is_some();
            drop(folded);
            *self.show() = Shown { view, seq: last };
            self.grew.notify_all();
            if stop {
                return;
            }
        }
    }

    /// Serve one connection: read its request, answer it.
    pub fn handle(&self, stream: impl Socket) -> io::Result<()> {
        let (request, mut stream) = http::read_request(stream)?;
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => http::respond(
                &mut stream,
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
                false,
            ),
            ("GET", "/events") => self.events(&mut stream, request.last_event_id),
            ("GET", "/view") => {
                let view = Arc::clone(&self.show().view);
                http::respond(&mut stream, "application/json", view.as_bytes(), false)
            }
            ("GET", "/wiring") => {
                let dictation = self.wiring.dictation.as_ref();
                let can = json!({
                    "act": self.wiring.act.is_some(),
                    "dictation": dictation.is_some(),
                    "sample": dictation.is_some_and(Dictation::has_sample),
                });
                http::respond(
                    &mut stream,
                    "application/json",
                    can.to_string().as_bytes(),
                    false,
                )
            }
            ("POST", "/act") => self.act(&mut stream, &request.body),
            ("POST", "/digest") => {
                let id = ValueId::of(&request.body).to_string();
                http::respond(&mut stream, "text/plain", id.as_bytes(), false)
            }
            ("GET", "/dictation") => self.dictate(stream, &request),
            ("GET", "/live") => self.live(&mut stream, &request),
            ("GET", path) => match path.strip_prefix("/context/").and_then(|h| h.parse().ok()) {
                Some(head) => self.context(&mut stream, head),
                None => http::not_found(&mut stream),
            },
            _ => http::not_found(&mut stream),
        }
    }

    /// A batch of Max's facts: `{"face", "seen", "batch"}`, in the journal's
    /// own encoding.
    fn act(&self, stream: &mut impl Write, body: &[u8]) -> io::Result<()> {
        let Some(act) = &self.wiring.act else {
            return http::refuse(stream, "501 Not Implemented", "not wired to a brain");
        };
        let mut said: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
        let face = serde_json::from_value::<FaceId>(said["face"].take());
        let batch = serde_json::from_value::<Batch>(said["batch"].take());
        let (Ok(face), Some(seen), Ok(batch)) = (face, said["seen"].as_u64(), batch) else {
            return http::refuse(stream, "400 Bad Request", "expected {face, seen, batch}");
        };
        match act.act(face, Seq(seen), batch) {
            Ok(()) => http::respond(stream, "text/plain", b"", false),
            Err(why) => http::refuse(stream, "409 Conflict", &why),
        }
    }

    /// A running thing's live progress: `?model=<id>` or `?call=<id>`.
    fn live(&self, stream: &mut impl Write, request: &http::Request) -> io::Result<()> {
        let running = match (request.query("model"), request.query("call")) {
            (Some(id), _) => id.parse::<ModelCallId>().map(Running::Model),
            (_, Some(id)) => id.parse::<CallId>().map(Running::Call),
            _ => return http::refuse(stream, "400 Bad Request", "expected ?model= or ?call="),
        };
        let Ok(running) = running else {
            return http::refuse(stream, "400 Bad Request", "a bad id");
        };
        http::start_events(stream)?;
        let after = request.last_event_id.map_or(0, |s| s.0 as usize);
        self.wiring.feed.stream(stream, running, after, HEARTBEAT)
    }

    /// The dictation socket: `?rate=<Hz>` of the page's PCM, or `?sample`.
    fn dictate(&self, mut stream: impl Socket, request: &http::Request) -> io::Result<()> {
        let (Some(dictation), Some(key)) = (&self.wiring.dictation, &request.websocket_key) else {
            return http::not_found(&mut stream);
        };
        let rate = request.query("rate").and_then(|r| r.parse().ok());
        let Some(rate) = rate.filter(|r| (8_000..=96_000).contains(r)) else {
            return http::refuse(&mut stream, "400 Bad Request", "expected ?rate=<Hz>");
        };
        http::accept_websocket(&mut stream, key)?;
        let mut page = tungstenite::WebSocket::from_raw_socket(
            stream,
            tungstenite::protocol::Role::Server,
            None,
        );
        dictation.relay(&mut page, rate, request.query("sample").is_some());
        Ok(())
    }

    fn read(&self) -> RwLockReadGuard<'_, Folded> {
        self.folded.read().unwrap_or_else(|e| e.into_inner())
    }

    fn show(&self) -> MutexGuard<'_, Shown> {
        self.shown.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The event stream: from the client's cursor, one event per fold.
    fn events(&self, stream: &mut impl Write, cursor: Option<Seq>) -> io::Result<()> {
        http::start_events(stream)?;
        let mut cursor = cursor;
        loop {
            let (shown, _) = self
                .grew
                .wait_timeout_while(self.show(), HEARTBEAT, |s| {
                    cursor.is_some_and(|c| s.seq <= c)
                })
                .unwrap_or_else(|e| e.into_inner());
            let (seq, view) = (shown.seq, Arc::clone(&shown.view));
            drop(shown);
            if cursor.is_some_and(|c| seq <= c) {
                stream.write_all(b": heartbeat\n\n")?;
                stream.flush()?;
                continue;
            }
            write!(stream, "id: {}\ndata: {view}\n\n", seq.0)?;
            stream.flush()?;
            cursor = Some(seq);
        }
    }

    /// The context ending at `head`, root first, with the bytes its entries
    /// name as text.
    fn context(&self, stream: &mut impl Write, head: EntryId) -> io::Result<()> {
        let text = |id: ValueId| {
            self.source
                .value(id)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        // What each response cost and when it came, read with the chain so
        // the store is read with no lock held.
        let chain: Vec<(EntryId, Entry, Value)> = {
            let folded = self.read();
            folded
                .state
                .context(head)
                .into_iter()
                .map(|(id, entry)| (id, entry.clone(), response(&folded.state, id)))
                .collect()
        };
        let chain: Vec<Value> = chain
            .into_iter()
            .map(|(id, entry, response)| match entry {
                Entry::Root(root) => json!({
                    "id": id.to_string(),
                    "role": "root",
                    "model": root.model,
                    "system": text(root.system),
                    "tools": text(root.tools),
                }),
                Entry::Message { message, .. } => json!({
                    "id": id.to_string(),
                    "role": message.role.as_str(),
                    "content": text(message.content),
                    "response": response,
                }),
            })
            .collect();
        if chain.is_empty() {
            return http::not_found(stream);
        }
        let body = serde_json::to_vec(&chain).expect("a JSON value serialises");
        http::respond(stream, "application/json", &body, true)
    }
}

/// When an entry that is a response was sent and ended, and the context it
/// was sent with: what it cost.
fn response(state: &State, entry: EntryId) -> Value {
    let Some(record) = state.response(entry) else {
        return Value::Null;
    };
    let ended = record.ended.as_ref();
    let usage = ended.and_then(|e| e.usage);
    json!({
        "sent": record.sent.at.0,
        "ended": ended.map(|e| e.at.at.0),
        "context": usage.map(|u| u.input + u.cache_read + u.cache_write_short + u.cache_write_long),
        "output": usage.map(|u| u.output),
    })
}

fn render(
    state: &State,
    value: &dyn Fn(ValueId) -> Option<Vec<u8>>,
    refused: Option<&str>,
) -> String {
    let mut view = view::view(state, value, now());
    view["refused"] = json!(refused);
    view.to_string()
}

fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Timestamp(ms as i64)
}

const PAGE: &str = include_str!("page.html");

/// The TLS configuration for a PEM certificate chain and key.
pub fn tls(cert: &Path, key: &Path) -> io::Result<Arc<rustls::ServerConfig>> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let invalid = |e: rustls::pki_types::pem::Error| io::Error::new(io::ErrorKind::InvalidData, e);
    let chain = CertificateDer::pem_file_iter(cert)
        .map_err(invalid)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let key = PrivateKeyDer::from_pem_file(key).map_err(invalid)?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(io::Error::other)?
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .map_err(io::Error::other)?;
    Ok(Arc::new(config))
}

/// Serve the face over HTTPS, a thread per connection, until the listener
/// fails; then return once every open connection has ended. Only this host
/// and the peers `allow` admits may connect.
pub fn serve<S: Source>(
    face: &Face<S>,
    listener: TcpListener,
    tls: Arc<rustls::ServerConfig>,
    allow: impl Fn(IpAddr) -> bool,
) -> io::Result<()> {
    thread::scope(|s| {
        loop {
            let (tcp, peer) = listener.accept()?;
            let local = tcp.local_addr()?.ip();
            if !(peer.ip().is_loopback() || peer.ip() == local || allow(peer.ip())) {
                continue;
            }
            let tls = Arc::clone(&tls);
            s.spawn(move || {
                tcp.set_read_timeout(Some(Duration::from_secs(10))).ok();
                tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
                let Ok(connection) = rustls::ServerConnection::new(tls) else {
                    return;
                };
                // A closed tab or a failed handshake ends only this connection.
                let mut stream = rustls::StreamOwned::new(connection, tcp);
                if face.handle(&mut stream).is_ok() {
                    stream.conn.send_close_notify();
                    let _ = stream.flush();
                }
            });
        }
    })
}

/// The IPv4 gateway of this machine's default route: on a Hyper-V guest,
/// the Windows host. Read from `/proc/net/route`, so Linux only.
pub fn default_gateway() -> Option<Ipv4Addr> {
    let routes = std::fs::read_to_string("/proc/net/route").ok()?;
    routes.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.get(1) != Some(&"00000000") {
            return None;
        }
        let gateway = u32::from_str_radix(fields.get(2)?, 16).ok()?;
        Some(Ipv4Addr::from(gateway.to_le_bytes()))
    })
}
