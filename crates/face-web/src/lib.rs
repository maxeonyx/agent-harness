//! The harness's web face: Max's work as a map of tasks, live from the
//! journal, each opening into the log of what it did.
//!
//! A face is a reader of the journal and nothing else. The brain hands it
//! the [`State`] after each commit, a persistent value the face holds while
//! the brain moves on. It sends the views the core derives: every
//! task, the scope that owns it and what it waits on
//! ([`State::waiting_on`], the throbber), Max's queue ([`State::queue`]) and
//! a task's context. It keeps no state of its own that could disagree with
//! the journal.
//!
//! **The page** (`page.html`) is the map. A task is a node; a scope (the
//! spawn call that owns its members) is a junction under its owner; the
//! projects are the map's disconnected components. A node opens into its
//! log: its context, exactly as sent, rendered, with what is streaming now
//! and Max's words it has not read yet. Moving the map closes it.
//!
//! **The cursor is the journal position.** The page's event stream is one
//! event per commit: the whole view, with the [`Seq`] it reached as the
//! event id. The browser resends the last id it got when it reconnects, and
//! is sent the next view once the journal is past it, so a view is never
//! sent twice and never missed. `/view` is the latest view alone, for a
//! client that only looks.
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
//! where Enter is a new line and Max sends now or for the end of the
//! task's turn), his controls over a task
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

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::call::Caller;
use harness_core::context::{Entry, EntryId, Note, Rendered};
use harness_core::delivery::{Content, Delivery, Prompt};
use harness_core::id::{CallId, FaceId, ModelCallId, Seq, TaskId, Timestamp, ValueId};
use harness_core::journal::Batch;
use harness_core::state::{Delivered, State};
use serde_json::{Value, json};

pub use dictation::{Dictation, Edit, Socket, edits};
pub use live::{Feed, Running};
pub use view::describe;

/// Where a face reads the values the journal's facts name. A face never
/// writes.
pub trait Source: Send + Sync + 'static {
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

/// A face over one journal: follows its commits and serves the page.
///
/// Only the follower writes `state`, and holds it only to swap the state
/// in; viewers read it alongside one another. Nothing reads the store while
/// holding either lock.
pub struct Face<S> {
    source: S,
    wiring: Wiring,
    state: RwLock<State>,
    shown: Mutex<Shown>,
    grew: Condvar,
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
        let view = Arc::new(render(&state, &|_| None));
        Arc::new(Face {
            source,
            wiring,
            state: RwLock::new(state),
            shown: Mutex::new(Shown { view, seq: Seq(0) }),
            grew: Condvar::new(),
        })
    }

    /// Follow the brain's commits until it stops sending them. States that
    /// queue while a view renders are shown as the last of them.
    pub fn follow(&self, commits: Receiver<State>) {
        while let Ok(mut state) = commits.recv() {
            if let Some(last) = commits.try_iter().last() {
                state = last;
            }
            let view = Arc::new(render(&state, &|id| self.source.value(id)));
            let last = state.last().unwrap_or(Seq(0));
            *self.state.write().unwrap_or_else(|e| e.into_inner()) = state;
            *self.show() = Shown { view, seq: last };
            self.grew.notify_all();
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

    fn read(&self) -> RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
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
        // What each response cost and when it came, and who wrote each
        // block of what the harness sent, read with the chain so the store
        // is read with no lock held.
        let (chain, sent) = {
            let state = self.read();
            let chain: Vec<(EntryId, Entry, Value)> = state
                .context(head)
                .into_iter()
                .map(|(id, entry)| (id, entry.clone(), response(&state, id)))
                .collect();
            let sent = sent(&state, &chain);
            (chain, sent)
        };
        let mut by = authors(sent, &|id| self.source.value(id));
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
                    "by": by.remove(&id),
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

/// Who wrote a run of blocks the harness sent.
#[derive(Clone, Copy)]
enum Author {
    /// The harness's own words.
    Harness,
    /// A tool's result, or a background call's settlement.
    Call,
    /// The mandate an agent gave the task. A mandate Max gave is his words.
    Mandate,
    /// The brief a spawn's members share.
    Brief,
    /// Max's words, after the harness's label for them: the last of the
    /// run's blocks, as many as his words are.
    Max { words: ValueId, at: Timestamp },
    /// Max's answer to a question asked in the background, in one block
    /// with the harness's note.
    Answer { words: ValueId },
    /// A standard prompt: Max's Done, or the stop an owner or Max made.
    Prompt(Prompt),
}

/// A request that appended some of a context: its messages and the runs of
/// their blocks, with who wrote each run.
struct Sent {
    messages: Vec<(EntryId, ValueId)>,
    runs: Vec<(Author, usize)>,
}

/// The requests that appended the messages of `chain`.
fn sent(state: &State, chain: &[(EntryId, Entry, Value)]) -> Vec<Sent> {
    let wanted: BTreeSet<EntryId> = chain.iter().map(|(id, _, _)| *id).collect();
    state
        .model_calls()
        .filter_map(|(_, record)| {
            let call = &record.call;
            let messages: Vec<(EntryId, ValueId)> = call
                .entries()
                .into_iter()
                .filter_map(|(id, entry)| match entry {
                    Entry::Message { message, .. } => Some((id, message.content)),
                    Entry::Root(_) => None,
                })
                .collect();
            messages
                .iter()
                .any(|(id, _)| wanted.contains(id))
                .then(|| Sent {
                    messages,
                    runs: call
                        .renders
                        .iter()
                        .map(|span| (author(state, call.task, &span.from), span.blocks))
                        .collect(),
                })
        })
        .collect()
}

fn author(state: &State, task: TaskId, from: &Rendered) -> Author {
    match from {
        Rendered::Delivery(id) => match state.delivery(*id) {
            Some(Delivered {
                delivery:
                    Delivery {
                        content: Content::Words(words),
                        ..
                    },
                at,
                ..
            }) => Author::Max {
                words: *words,
                at: at.at,
            },
            Some(Delivered {
                delivery:
                    Delivery {
                        content: Content::Prompt(prompt),
                        ..
                    },
                ..
            }) => Author::Prompt(*prompt),
            None => Author::Harness,
        },
        Rendered::Call(call) => match state.call(*call) {
            Some(record)
                if state.is_ask(*call)
                    && matches!(record.call.caller, Caller::Background { .. }) =>
            {
                match record.outcome().and_then(|o| o.output()) {
                    Some(output) => Author::Answer {
                        words: output.model,
                    },
                    None => Author::Call,
                }
            }
            _ => Author::Call,
        },
        // A task Max started, an aside or a resume, opens with his words.
        Rendered::Note(Note::Mandate) => match state.task(task) {
            Some(t) if t.by.is_none() => Author::Max {
                words: t.mandate.text,
                at: t.created.at,
            },
            _ => Author::Mandate,
        },
        Rendered::Note(Note::Brief) => Author::Brief,
        Rendered::Note(_) => Author::Harness,
    }
}

/// Who wrote each block of each message the harness sent, as the page
/// reads it: one entry per block.
fn authors(
    sent: Vec<Sent>,
    value: &dyn Fn(ValueId) -> Option<Vec<u8>>,
) -> BTreeMap<EntryId, Value> {
    let blocks = |id: ValueId| -> Vec<Value> {
        value(id)
            .and_then(|bytes| serde_json::from_slice::<Vec<Value>>(&bytes).ok())
            .unwrap_or_default()
    };
    let mut out = BTreeMap::new();
    for Sent { messages, runs } in sent {
        // Each run's author, once per block, in block order across the
        // request's messages.
        let mut each = runs.into_iter().flat_map(|(author, n)| {
            let words = match &author {
                Author::Max { words, .. } => blocks(*words).len().max(1),
                _ => 0,
            };
            (0..n).map(move |i| (author, n - i <= words))
        });
        for (id, content) in messages {
            let by: Vec<Value> = (0..blocks(content).len())
                .map(|_| match each.next() {
                    None => Value::Null,
                    Some((author, said)) => match author {
                        Author::Max { at, .. } if said => json!({"by": "max", "at": at.0}),
                        Author::Max { .. } => json!({"by": "label"}),
                        Author::Harness => json!({"by": "harness"}),
                        Author::Call => json!({"by": "call"}),
                        Author::Mandate => json!({"by": "mandate"}),
                        Author::Brief => json!({"by": "brief"}),
                        Author::Prompt(p) => json!({"by": "prompt", "prompt": format!("{p:?}")}),
                        Author::Answer { words } => {
                            let said: Vec<String> = blocks(words)
                                .iter()
                                .filter_map(|b| b["text"].as_str().map(String::from))
                                .collect();
                            json!({"by": "answer", "said": said.join("\n")})
                        }
                    },
                })
                .collect();
            out.insert(id, Value::Array(by));
        }
    }
    out
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

fn render(state: &State, value: &dyn Fn(ValueId) -> Option<Vec<u8>>) -> String {
    view::view(state, value, now()).to_string()
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
