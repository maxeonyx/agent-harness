//! The harness's web face: the task tree Max watches, live from the journal.
//!
//! A face is a reader of the journal and nothing else. It folds the records
//! it is given with [`State::apply`], the same fold the brain runs, and
//! shows the views the core derives: the tree of tasks and their states,
//! what each waits on ([`State::waiting_on`], the throbber), Max's queue
//! ([`State::queue`]) and a selected task's context. It keeps no state of
//! its own that could disagree with the journal.
//!
//! **The cursor is the journal position.** The page's event stream is one
//! event per fold, with the [`Seq`] it reached as the event id. The browser
//! resends the last id it got when it reconnects, so a dropped connection
//! resumes with every record after it: the facts in between are read from
//! the [`Source`] again, and the views are whole at every position, so
//! nothing is lost and nothing is applied twice.
//!
//! **A context is a value.** It is fetched by its head entry
//! (`/context/<entry>`) and never changes, so the browser caches it forever
//! and refetches only when the task's head moves.
//!
//! Transport is std's TCP, rustls and one static page with no build step.

mod http;
mod view;

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Role};
use harness_core::id::{Seq, Timestamp};
use harness_core::journal::{Author, Record};
use harness_core::state::State;
use serde_json::json;

pub use view::describe;

/// What a face reads: the journal, and the context entries it names.
///
/// The store implements this. A face never writes.
pub trait Source: Send + Sync + 'static {
    /// The records after `after`, in order, waiting up to `wait` for at
    /// least one. Empty if none came.
    fn read(&self, after: Seq, wait: Duration) -> Vec<Record>;

    /// A context entry by its id, if stored.
    fn entry(&self, id: EntryId) -> Option<Entry>;
}

/// A face over one journal: folds it as it grows and serves the page.
pub struct Face<S> {
    source: S,
    folded: Mutex<Folded>,
    grew: Condvar,
}

struct Folded {
    state: State,
    /// The page's view of `state`, rendered once per fold for every viewer.
    view: Arc<String>,
    /// A record the fold refused: this face's build disagrees with the
    /// brain's, and the views stop at the record before it.
    refused: Option<String>,
}

/// How many recent facts a page that has seen nothing is sent.
const RECENT: u64 = 200;
/// How often an idle event stream is written to, so a dead one is noticed.
const HEARTBEAT: Duration = Duration::from_secs(15);

impl<S: Source> Face<S> {
    pub fn new(source: S) -> Arc<Face<S>> {
        let state = State::default();
        let view = Arc::new(render(&state, None));
        Arc::new(Face {
            source,
            folded: Mutex::new(Folded {
                state,
                view,
                refused: None,
            }),
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
            let mut folded = self.lock();
            for record in &records {
                if let Err(rejected) = folded.state.apply(record) {
                    let refused = format!("record {} refused: {rejected:?}", record.seq.0);
                    folded.refused = Some(refused);
                    break;
                }
            }
            folded.view = Arc::new(render(&folded.state, folded.refused.as_deref()));
            last = folded.state.last().unwrap_or(Seq(0));
            let stop = folded.refused.is_some();
            drop(folded);
            self.grew.notify_all();
            if stop {
                return;
            }
        }
    }

    /// Serve one connection: read its request, answer it.
    pub fn handle(&self, stream: impl Read + Write) -> io::Result<()> {
        let (request, mut stream) = http::read_request(stream)?;
        match request.path.as_str() {
            "/" => http::respond(
                &mut stream,
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
                false,
            ),
            "/events" => self.events(&mut stream, request.last_event_id),
            path => match path.strip_prefix("/context/").and_then(view::parse_entry) {
                Some(head) => self.context(&mut stream, head),
                None => http::not_found(&mut stream),
            },
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Folded> {
        self.folded.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The event stream: from the client's cursor, one event per fold.
    fn events(&self, stream: &mut impl Write, cursor: Option<Seq>) -> io::Result<()> {
        http::start_events(stream)?;
        let mut cursor = cursor;
        loop {
            let folded = self.lock();
            let reached = |f: &Folded| f.state.last().unwrap_or(Seq(0));
            let (folded, _) = self
                .grew
                .wait_timeout_while(folded, HEARTBEAT, |f| {
                    cursor.is_some_and(|c| reached(f) <= c)
                })
                .unwrap_or_else(|e| e.into_inner());
            let seq = reached(&folded);
            if cursor.is_some_and(|c| seq <= c) {
                drop(folded);
                stream.write_all(b": heartbeat\n\n")?;
                stream.flush()?;
                continue;
            }
            let from = cursor.unwrap_or(Seq(seq.0.saturating_sub(RECENT)));
            let facts: Vec<String> = self
                .source
                .read(from, Duration::ZERO)
                .iter()
                .take_while(|r| r.seq <= seq)
                .map(|r| fact(&folded.state, r))
                .collect();
            let view = Arc::clone(&folded.view);
            drop(folded);
            let data = format!(r#"{{"view":{view},"facts":[{}]}}"#, facts.join(","));
            write!(stream, "id: {}\ndata: {data}\n\n", seq.0)?;
            stream.flush()?;
            cursor = Some(seq);
        }
    }

    /// The context ending at `head`, root first.
    fn context(&self, stream: &mut impl Write, head: EntryId) -> io::Result<()> {
        let mut chain = Vec::new();
        let mut next = Some(head);
        while let Some(id) = next {
            let Some(entry) = self.source.entry(id) else {
                chain.push(json!({"id": view::entry(id), "missing": true}));
                break;
            };
            chain.push(match &entry {
                Entry::Root(root) => json!({
                    "id": view::entry(id),
                    "role": "root",
                    "model": root.model,
                    "system": root.system.0,
                    "tools": root.tools.0,
                }),
                Entry::Message(m) => json!({
                    "id": view::entry(id),
                    "role": match m.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => "system",
                    },
                    "content": m.content.0,
                }),
            });
            next = entry.parent();
        }
        chain.reverse();
        let body = serde_json::to_vec(&chain).expect("a JSON value serialises");
        http::respond(stream, "application/json", &body, true)
    }
}

/// A record as the page's journal shows it.
fn fact(state: &State, record: &Record) -> String {
    let by = match record.by {
        Author::Max { .. } => "Max",
        Author::Brain => "brain",
        Author::Limb(_) => "limb",
    };
    let line = json!({
        "seq": record.seq.0,
        "at": record.at.0,
        "by": by,
        "text": describe(state, record),
    });
    line.to_string()
}

fn render(state: &State, refused: Option<&str>) -> String {
    let mut view = view::view(state, now());
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
/// fails. Only this host and the peers `allow` admits may connect.
pub fn serve<S: Source>(
    face: Arc<Face<S>>,
    listener: TcpListener,
    tls: Arc<rustls::ServerConfig>,
    allow: impl Fn(IpAddr) -> bool,
) -> io::Result<()> {
    loop {
        let (tcp, peer) = listener.accept()?;
        let local = tcp.local_addr()?.ip();
        if !(peer.ip().is_loopback() || peer.ip() == local || allow(peer.ip())) {
            continue;
        }
        let face = Arc::clone(&face);
        let tls = Arc::clone(&tls);
        thread::spawn(move || {
            tcp.set_read_timeout(Some(Duration::from_secs(10))).ok();
            tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
            let Ok(connection) = rustls::ServerConnection::new(tls) else {
                return;
            };
            // A closed tab or a failed handshake ends only this connection.
            let _ = face.handle(rustls::StreamOwned::new(connection, tcp));
        });
    }
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
