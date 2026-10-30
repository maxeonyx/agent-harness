//! The harness's web face: Max's work as a map of tasks, each opening into
//! the log of what it did, served to every tab as a replica on the substrate.
//!
//! **Each tab is a replica.** The page runs `harness-face-tab` as wasm: the
//! tab's own log, kept in IndexedDB, and its replica of the rows this face
//! derives for it. One WebSocket per tab, `/link?replica=<id>`, carries the
//! substrate's [`Up`] and [`Down`] as JSON. What Max does is a record at his
//! tab: every [`Up::Record`] goes to the brain ([`Wiring::include`]), which
//! answers only with what it folds. So a refusal is in band, in the tab's
//! standing, beside the view that shows what he acted on; there is no reply
//! to a request, because there is no request.
//!
//! **Rows.** After every fold the brain hands the face a [`Folded`]: the
//! state, the brain time it is as of, and every replica's standing. The face
//! derives the rows once ([`view::rows`]: each task, the queue, the
//! harness, Max's drafts), and each link's [`Publisher`] ships its tab
//! what changed: a whole [`View`] on connecting, then steps. Nothing about
//! a tab is kept here while it is away: it reconnects to a whole view and
//! ships what the view's standing does not include.
//!
//! **Live progress.** A running request's streamed text and a running
//! command's output are not facts; the brain puts them in a [`Feed`], and
//! each link forwards what its tab has not had as [`Down::Live`], all of it
//! again after a reconnect.
//!
//! **A context is a value.** It is fetched by its head entry
//! (`/context/<entry>`), read off the fold ([`State::context`]) with the
//! bytes each entry names from the [`Source`], and when each response was
//! sent and ended and the context it was sent with, which never change; so
//! the browser caches it forever and refetches only when the task's head
//! moves.
//!
//! A chat is found by what was said in it (`/find?q=<words>`), read off
//! the fold the same way.
//!
//! **Page errors.** An error the page hits (uncaught, or handled by showing
//! something else) is a [`PageError`]: which tab, what it was trying to do,
//! and the error. The page's first script ships it as `GET /report`, not up
//! the link, since the errors that matter most are those that stop the link
//! or the page's own scripts from starting. The face hands it to
//! [`Wiring::report`]; the brain decides what becomes of it.
//!
//! The box takes dictation too ([`Dictation`]): the page's microphone,
//! relayed to Anthropic's speech-to-text endpoint, the transcript arriving in
//! the box as editable text.
//!
//! Transport is std's TCP, rustls, one static page and the tab's wasm,
//! built from `harness-face-tab` by this crate's build script; the
//! sockets are WebSockets (tungstenite, without its TLS).
//!
//! # Dictation
//!
//! One press of the microphone. The page streams PCM to the face, which relays it with the login's token, since the page must never hold it. Nothing reaches the brain from the relay: the transcript lands in the box as typed text does, and is saved as the draft like it. Pressing send while dictating stops the dictation and stages the text; a second press sends it once the final text is in (`dictation.then`, `finishDictation`). The send itself is drawn in the brain's "A real turn".
//!
//! ```mermaid
//! ---
//! title: "Dictation: one press of the microphone"
//! ---
//! sequenceDiagram
//!   participant page as Max's page
//!   participant face as Face
//!   participant api as Anthropic voice_stream
//!   participant brain as The brain
//!   page->>page: Max presses the microphone: the page makes 16-bit PCM<br/>compose.js dictate: getUserMedia, an AudioWorklet
//!   page->>face: open the dictation socket<br/>wss /dictation?rate=Hz: Face::dictate
//!   face->>api: open the speech stream with the login's token, and keep it alive<br/>Dictation::relay, session, connect: wss, KeepAlive
//!   face-->>page: listening
//!   loop until Max stops
//!     page->>face: a tenth of a second of PCM
//!     face->>api: the same chunk
//!     api-->>face: the whole session's text so far<br/>TranscriptText
//!     face-->>page: what changed, as edits: at, cut, put<br/>Transcript::session, a word-level diff
//!     Note over page: the edits go in where dictation began, at the caret, keeping Max's own edits<br/>applyEdits
//!   end
//!   Note over face,api: a session the endpoint drops is replaced, up to 3 times<br/>Transcript::keep holds the text so far
//!   page->>face: Max stops<br/>stopDictation
//!   Note over page: the microphone is released at once<br/>stopDictation: releaseMic
//!   face->>api: no more audio<br/>CloseStream
//!   api-->>face: the final text<br/>TranscriptEndpoint
//!   face-->>page: final, and the page closes the socket<br/>finishDictation
//!   page->>brain: the text is saved as a draft, 600 ms after the last change<br/>Draft Text record: compose.js edit, save. A chat not yet started keeps it in the tab instead
//! ```

mod dictation;
mod find;
mod http;
mod live;
mod view;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io::{self, ErrorKind, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Note, Rendered};
use harness_core::delivery::{Content, Delivery, Prompt};
use harness_core::fact::Refusal;
use harness_core::id::{ReplicaId, TaskId, Time, Timestamp, ValueId};
use harness_core::schema::Record;
use harness_core::state::{Delivered, State};
use harness_face_tab::{Down, FaceKey, FaceRow, Up, View};
use harness_substrate::{Live, Publisher, Standing};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

pub use dictation::Dictation;
use dictation::Socket;
pub use live::{Feed, Running};
pub use view::describe;

/// Where a face reads the values the facts name. A face never writes.
pub trait Source: Send + Sync + 'static {
    /// A value's bytes, if stored.
    fn value(&self, id: ValueId) -> Option<Vec<u8>>;
}

/// What a face is wired to.
pub struct Wiring {
    /// Where each tab's records go: the brain, which states that the tab
    /// is Max's if it is new to it, includes the record, and answers with
    /// a later [`Folded`]. Without it, a face only watches.
    pub include: Option<Box<dyn Fn(Record) + Send + Sync>>,
    pub dictation: Option<Dictation>,
    /// The claude.ai login Max signs in to from the page (`/login`).
    pub login: Option<harness_provider::Login>,
    /// Live progress, from whoever runs things.
    pub feed: Feed,
    /// The models on offer, newest first ([`harness_provider::Client::models`]),
    /// which Max chooses a chat's from.
    pub models: Vec<harness_provider::Model>,
    /// The commit the brain was built from, `+dirty` if the tree had
    /// changes.
    pub source: String,
    /// Where to read the page's files (`crates/face-web/src/page`) from on
    /// each request, so an edit shows on reload; `None` serves the ones
    /// built in.
    pub page: Option<PathBuf>,
    /// Where each error the page reports goes.
    pub report: Box<dyn Fn(PageError) + Send + Sync>,
}

/// An error the page hit, as it reported it: all text, as the page wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageError {
    /// The tab's replica id, or `unknown` before it had one.
    pub tab: String,
    /// What the page was trying to do.
    pub doing: String,
    pub error: String,
}

/// One line, however many lines the page's words run to.
impl fmt::Display for PageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = |s: &str| s.replace(|c: char| c.is_control(), " ");
        write!(
            f,
            "page error in tab {} while {}: {}",
            line(&self.tab),
            line(&self.doing),
            line(&self.error)
        )
    }
}

/// What the brain hands the face after each fold.
pub struct Folded {
    pub state: State,
    /// The brain's time the state is as of: its head.
    pub as_of: Time,
    /// How far the brain has included each replica it holds records of.
    pub standing: BTreeMap<ReplicaId, Standing<Refusal>>,
}

/// A face over one brain: follows its folds and serves the page.
///
/// Only the follower writes `state` and `shown`; links read them alongside
/// one another. Nothing reads the store while holding either lock.
pub struct Face<S> {
    source: S,
    wiring: Wiring,
    state: RwLock<State>,
    shown: Mutex<Option<Arc<Shown>>>,
    said: find::Said,
}

/// What every link publishes from: the rows, derived once per fold.
struct Shown {
    as_of: Time,
    rows: BTreeMap<FaceKey, FaceRow>,
    standing: BTreeMap<ReplicaId, Standing<Refusal>>,
}

/// How long a link waits for its tab before looking for something to
/// ship it. A synchronous TLS socket cannot be read on one thread and
/// written on another, so a link takes turns.
const TURN: Duration = Duration::from_millis(20);
/// How often an idle link is pinged, so a dead one is noticed.
const HEARTBEAT: Duration = Duration::from_secs(15);

impl<S: Source> Face<S> {
    /// A face that includes, dictates and shows live progress as `wiring`
    /// allows.
    pub fn with(source: S, wiring: Wiring) -> Arc<Face<S>> {
        Arc::new(Face {
            source,
            wiring,
            state: RwLock::new(State::default()),
            shown: Mutex::new(None),
            said: find::Said::default(),
        })
    }

    /// Follow the brain's folds until it stops sending them. Folds that
    /// queue while rows are derived are shown as the last of them.
    pub fn follow(&self, folds: Receiver<Folded>) {
        while let Ok(mut folded) = folds.recv() {
            if let Some(last) = folds.try_iter().last() {
                folded = last;
            }
            let value = |id| self.source.value(id);
            let rows = view::rows(
                &folded.state,
                &value,
                &self.wiring.models,
                &self.wiring.source,
                now(),
            );
            *self.state.write().unwrap_or_else(|e| e.into_inner()) = folded.state;
            *self.show() = Some(Arc::new(Shown {
                as_of: folded.as_of,
                rows,
                standing: folded.standing,
            }));
        }
    }

    /// One of the page's files, by its name in `page/`: from disk, read now,
    /// if [`Wiring::page`] says where; otherwise as built in.
    fn page_file(&self, name: &str) -> io::Result<Cow<'static, str>> {
        let missing = || io::Error::new(ErrorKind::NotFound, format!("no page file {name}"));
        match &self.wiring.page {
            // Only a name in the directory, never a path out of it.
            Some(dir) if !name.contains('/') && !name.starts_with('.') => {
                std::fs::read_to_string(dir.join(name))
                    .map(Cow::Owned)
                    .map_err(|e| io::Error::new(e.kind(), format!("page file {name}: {e}")))
            }
            Some(_) => Err(missing()),
            None => PAGE
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, text)| Cow::Borrowed(*text))
                .ok_or_else(missing),
        }
    }

    /// Serve one connection: read its request, answer it.
    fn handle(&self, stream: impl Socket) -> io::Result<()> {
        let (request, mut stream) = http::read_request(stream)?;
        match request.path.as_str() {
            "/" => match DOCUMENT
                .iter()
                .map(|name| self.page_file(name))
                .collect::<io::Result<String>>()
            {
                Ok(document) => http::respond(
                    &mut stream,
                    "text/html; charset=utf-8",
                    document.as_bytes(),
                    false,
                ),
                Err(e) => http::refuse(&mut stream, "500 Internal Server Error", &e.to_string()),
            },
            "/tab.wasm" => http::respond(&mut stream, "application/wasm", TAB, false),
            "/icon.png" => http::respond(&mut stream, "image/png", ICON, false),
            "/wiring" => {
                let dictation = self.wiring.dictation.as_ref();
                let can = json!({
                    "act": self.wiring.include.is_some(),
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
            "/report" => {
                let field = |name| request.query(name).map(find::decode);
                let (Some(tab), Some(doing), Some(error)) =
                    (field("tab"), field("doing"), field("error"))
                else {
                    return http::refuse(
                        &mut stream,
                        "400 Bad Request",
                        "expected ?tab=<replica>&doing=<what>&error=<why>",
                    );
                };
                (self.wiring.report)(PageError { tab, doing, error });
                http::respond(&mut stream, "text/plain", b"", false)
            }
            "/view" => self.view(&mut stream),
            "/link" => self.link(stream, &request),
            "/dictation" => self.dictate(stream, &request),
            // A sign-in: opened in a tab of its own, `/login` begins one and
            // sends the tab on to claude.ai; then `?code=CODE` finishes it
            // with what claude.ai showed. Only Max reaches the face.
            "/login" => {
                let Some(login) = &self.wiring.login else {
                    return http::not_found(&mut stream);
                };
                let done = match request.query("code").map(find::decode) {
                    None => login.begin().map(|url| http::redirect(&mut stream, &url)),
                    Some(code) => login
                        .finish(&code)
                        .map(|()| http::respond(&mut stream, "text/plain", b"Signed in.", false)),
                };
                match done {
                    Ok(sent) => sent,
                    Err(e) => http::refuse(&mut stream, "400 Bad Request", &e.to_string()),
                }
            }
            "/find" => {
                let query = find::decode(request.query("q").unwrap_or(""));
                let tasks = find::own_entries(&self.read());
                let found = self.said.find(tasks, &query, |id| self.source.value(id));
                http::respond(
                    &mut stream,
                    "application/json",
                    found.to_string().as_bytes(),
                    false,
                )
            }
            path => {
                if let Some(name) = path.strip_prefix("/page/") {
                    let kind = match name.rsplit_once('.') {
                        Some((_, "js")) => "text/javascript; charset=utf-8",
                        Some((_, "css")) => "text/css; charset=utf-8",
                        Some((_, "webmanifest")) => "application/manifest+json",
                        _ => "text/html; charset=utf-8",
                    };
                    return match self.page_file(name) {
                        Ok(text) => http::respond(&mut stream, kind, text.as_bytes(), false),
                        Err(_) => http::not_found(&mut stream),
                    };
                }
                if let Some(head) = path.strip_prefix("/context/").and_then(|h| h.parse().ok()) {
                    return self.context(&mut stream, head);
                }
                match path.strip_prefix("/versions/").and_then(|t| t.parse().ok()) {
                    Some(task) => {
                        let body = versions(&self.read(), task).to_string();
                        http::respond(&mut stream, "application/json", body.as_bytes(), false)
                    }
                    None => http::not_found(&mut stream),
                }
            }
        }
    }

    /// A tab's link: `?replica=<id>`. Its records go to the brain; its view,
    /// and live progress, come back.
    fn link(&self, mut stream: impl Socket, request: &http::Request) -> io::Result<()> {
        let replica = request
            .query("replica")
            .and_then(|p| p.parse::<ReplicaId>().ok());
        let (Some(replica), Some(key)) = (replica, &request.websocket_key) else {
            return http::refuse(&mut stream, "400 Bad Request", "expected ?replica=<id>");
        };
        http::accept_websocket(&mut stream, key)?;
        stream.set_read_timeout(Some(TURN))?;
        let mut tab = WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);
        let mut publisher = Publisher::default();
        let mut published: Option<Arc<Shown>> = None;
        let mut live = HashMap::new();
        let mut pinged = Instant::now();
        loop {
            match tab.read() {
                Ok(Message::Text(text)) => match serde_json::from_str::<Up>(&text) {
                    Ok(Up::Record(record)) if record.at.replica == replica => {
                        if let Some(include) = &self.wiring.include {
                            include(record);
                        }
                    }
                    // A link speaks for its own replica, in the schema's words.
                    _ => return Ok(()),
                },
                Ok(Message::Close(_)) => return Ok(()),
                Ok(_) => {}
                Err(e) if dictation::would_block(&e) => {}
                Err(_) => return Ok(()),
            }
            let shown = self.show().clone();
            if let Some(shown) =
                shown.filter(|s| !published.as_ref().is_some_and(|p| Arc::ptr_eq(p, s)))
            {
                let view = View {
                    as_of: shown.as_of,
                    standing: shown.standing.get(&replica).cloned().unwrap_or_default(),
                    rows: shown.rows.clone(),
                };
                if let Some(down) = publisher.publish(view) {
                    send(&mut tab, &down)?;
                }
                published = Some(shown);
            }
            if let Some(shown) = &published {
                for piece in self.wiring.feed.since(&mut live) {
                    let down = Down::Live(Live {
                        after: shown.as_of,
                        live: piece,
                    });
                    send(&mut tab, &down)?;
                }
            }
            if pinged.elapsed() > HEARTBEAT {
                pinged = Instant::now();
                tab.send(Message::Ping(Default::default()))
                    .map_err(io::Error::other)?;
            }
        }
    }

    /// The rows as of the last fold, by key, for a script that only looks.
    fn view(&self, stream: &mut impl Write) -> io::Result<()> {
        let shown = self.show().clone();
        let view = shown.map(|s| json!({ "as_of": s.as_of, "rows": s.rows }));
        let body = serde_json::to_vec(&view).expect("a JSON value serialises");
        http::respond(stream, "application/json", &body, false)
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
        let mut page =
            WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);
        dictation.relay(&mut page, rate, request.query("sample").is_some());
        Ok(())
    }

    fn read(&self) -> RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn show(&self) -> MutexGuard<'_, Option<Arc<Shown>>> {
        self.shown.lock().unwrap_or_else(|e| e.into_inner())
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
            let chain: Vec<(EntryId, Entry, (Value, Option<ValueId>))> = state
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
            .map(|(id, entry, (mut response, params))| match entry {
                Entry::Root(root) => json!({
                    "id": id.to_string(),
                    "role": "root",
                    "model": root.model,
                    "system": text(root.system),
                    "tools": text(root.tools),
                }),
                Entry::Message { message, .. } => {
                    if let Some(params) = params.and_then(|p| self.source.value(p)) {
                        response["effort"] = json!(harness_provider::effort(&params));
                    }
                    json!({
                        "id": id.to_string(),
                        "role": message.role.as_str(),
                        "content": text(message.content),
                        "response": response,
                        "by": by.remove(&id),
                    })
                }
            })
            .collect();
        if chain.is_empty() {
            return http::not_found(stream);
        }
        let body = serde_json::to_vec(&chain).expect("a JSON value serialises");
        http::respond(stream, "application/json", &body, true)
    }
}

fn send<S: Socket>(tab: &mut WebSocket<S>, down: &Down) -> io::Result<()> {
    let text = serde_json::to_string(down).expect("a view serialises");
    tab.send(Message::text(text)).map_err(io::Error::other)
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
    /// A standard prompt: Max's Done, or the stop an owner or Max made.
    Prompt(Prompt),
    /// A compaction's framing and summary; it replaced the branch ending at
    /// `from`.
    Compacted { from: EntryId },
}

/// A request that appended some of a context: its messages and the runs of
/// their blocks, with who wrote each run.
struct Sent {
    messages: Vec<(EntryId, ValueId)>,
    runs: Vec<(Author, usize)>,
}

/// The requests and compactions that appended the messages of `chain`.
fn sent<T>(state: &State, chain: &[(EntryId, Entry, T)]) -> Vec<Sent> {
    let wanted: BTreeSet<EntryId> = chain.iter().map(|(id, _, _)| *id).collect();
    let carried = chain.iter().filter_map(|(id, entry, _)| {
        let compacted = state.compacted(*id)?;
        let Entry::Message { message, .. } = entry else {
            return None;
        };
        let runs = compacted.renders.iter().map(|span| {
            let author = match span.from {
                Rendered::Note(Note::Compacted) => Author::Compacted {
                    from: compacted.from,
                },
                _ => author(state, compacted.task, &span.from),
            };
            (author, span.blocks)
        });
        Some(Sent {
            messages: vec![(*id, message.content)],
            runs: runs.collect(),
        })
    });
    let carried: Vec<Sent> = carried.collect();
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
        .chain(carried)
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
        Rendered::Call(_) => Author::Call,
        // A task Max started, an aside or a resume, opens with his words.
        Rendered::Note(Note::Mandate) => match state.task(task) {
            Some(t) if t.by.is_none() => Author::Max {
                words: t.mandate.text,
                at: t.created.at,
            },
            _ => Author::Mandate,
        },
        Rendered::Note(Note::Brief) => Author::Brief,
        Rendered::Note(Note::Kept { words }) => match state.said(*words) {
            Some(mark) => Author::Max {
                words: *words,
                at: mark.at,
            },
            None => Author::Harness,
        },
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
                        Author::Compacted { from } => {
                            json!({"by": "compacted", "from": from.to_string()})
                        }
                    },
                })
                .collect();
            out.insert(id, Value::Array(by));
        }
    }
    out
}

/// Where a task's context has more than one version: for each entry with
/// siblings it has been at, which of them it is (`n` of `of`, in the order
/// they were first made), and where Max lands moving to the one before or
/// after: the last response the task had on that path, or failing one, the
/// last request. And for each response the model gave more than once, how
/// many times (`tries`): an entry is its bytes, so a retry that says the
/// same words is the same version, asked again. A compaction's carry is no
/// version of what it replaced: going back past a checkpoint is not a
/// choice Max makes here. Read whole on every change of head and every
/// request that ends, since either can add to it.
fn versions(state: &State, task: TaskId) -> Value {
    let Some(start) = state.task(task).and_then(|t| t.context()).map(|c| c.start) else {
        return json!({});
    };
    let mut calls: Vec<_> = state
        .model_calls()
        .filter(|(_, m)| m.call.task == task && state.descends(m.call.base, start))
        .map(|(_, m)| m)
        .collect();
    calls.sort_by_key(|m| m.sent.seq);
    let mut children: BTreeMap<EntryId, Vec<EntryId>> = BTreeMap::new();
    let mut landing: HashMap<EntryId, (bool, EntryId)> = HashMap::new();
    let mut tries: HashMap<EntryId, usize> = HashMap::new();
    for m in calls {
        if let Some(r) = m.response {
            *tries.entry(r).or_default() += 1;
        }
        let ends = [Some((false, m.call.head())), m.response.map(|r| (true, r))];
        for (responded, end) in ends.into_iter().flatten() {
            let mut child = end;
            while child != start {
                let Some(parent) = state.entry(child).and_then(Entry::parent) else {
                    break;
                };
                let siblings = children.entry(parent).or_default();
                if !siblings.contains(&child) && state.compacted(child).is_none() {
                    siblings.push(child);
                }
                // A response outranks a request; a later one an earlier.
                let best = landing.entry(child).or_insert((responded, end));
                if responded || !best.0 {
                    *best = (responded, end);
                }
                child = parent;
            }
        }
    }
    let mut out = serde_json::Map::new();
    for siblings in children.values().filter(|s| s.len() > 1) {
        let to = |i: Option<usize>| {
            i.and_then(|i| siblings.get(i))
                .and_then(|s| landing.get(s))
                .map(|(_, e)| e.to_string())
        };
        for (i, child) in siblings.iter().enumerate() {
            out.insert(
                child.to_string(),
                json!({
                    "n": i + 1,
                    "of": siblings.len(),
                    "prev": to(i.checked_sub(1)),
                    "next": to(Some(i + 1)),
                }),
            );
        }
    }
    for (entry, n) in tries.into_iter().filter(|(_, n)| *n > 1) {
        let at = out.entry(entry.to_string()).or_insert_with(|| json!({}));
        at["tries"] = n.into();
    }
    Value::Object(out)
}

/// When an entry that is a response was sent and ended, the context it was
/// sent with (what it cost), and the model it ran at; with the parameters
/// it was sent with, whose effort the caller reads from the store.
fn response(state: &State, entry: EntryId) -> (Value, Option<ValueId>) {
    let Some(record) = state.response(entry) else {
        return (Value::Null, None);
    };
    let ended = record.ended.as_ref();
    let usage = ended.and_then(|e| e.usage);
    let model =
        record
            .call
            .model
            .clone()
            .or_else(|| match state.context(record.call.base).first() {
                Some((_, Entry::Root(root))) => Some(root.model.clone()),
                _ => None,
            });
    let said = json!({
        "sent": record.sent.at.0,
        "model": model,
        "ended": ended.map(|e| e.at.at.0),
        "context": usage.map(|u| u.input + u.cache_read + u.cache_write_short + u.cache_write_long),
        "output": usage.map(|u| u.output),
    });
    (said, Some(record.call.params))
}

fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Timestamp(ms as i64)
}

/// The page's files, built in: every file in `page/`, by name, as
/// `build.rs` lists them. [`Wiring::page`] reads them from disk instead.
const PAGE: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/page.rs"));
/// The page's document and its style sheet, joined from these files in this
/// order. Its script is the modules `page/*.js`, served at `/page/<name>`:
/// `main.js` starts the page, and each module imports what it uses from the
/// others, so a name lives in one module and says where.
const DOCUMENT: &[&str] = &[
    "head.html",
    "base.css",
    "chrome.css",
    "map.css",
    "overlays.css",
    "card.css",
    "compose.css",
    "phone.css",
    "body.html",
    "tail.html",
];
/// The tab, as the page runs it.
const TAB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tab.wasm"));
/// The face's icon, for its web app manifest (`page/manifest.webmanifest`).
const ICON: &[u8] = include_bytes!("icon.png");

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
