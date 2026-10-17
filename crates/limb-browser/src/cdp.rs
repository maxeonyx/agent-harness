//! The Chrome DevTools Protocol over one websocket: commands, each answered
//! by id, and events, handed to whoever opened the connection. Sessions are
//! flattened, so a tab's commands go over the same socket with its
//! `sessionId`.

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use tungstenite::protocol::Role;
use tungstenite::{Message, WebSocket};

/// One event: its method, its params, and the session it came from.
pub struct Event {
    pub method: String,
    pub params: Json,
    pub session: Option<String>,
}

/// The commands sent and not yet answered, by id.
type Waiting = Arc<Mutex<HashMap<u64, Sender<Result<Json, String>>>>>;

pub struct Cdp {
    writer: Mutex<WebSocket<TcpStream>>,
    next: AtomicU64,
    pending: Waiting,
    /// The socket, to close it when this is dropped.
    stream: TcpStream,
}

impl Cdp {
    /// Connect to the browser's websocket `url` (`ws://host:port/path`).
    /// Each event goes to `events`, on the reader's thread, until the
    /// socket closes; then `events` is called once with `Closed`.
    pub fn connect(url: &str, events: impl Fn(Event) + Send + 'static) -> Result<Cdp, String> {
        let rest = url
            .strip_prefix("ws://")
            .ok_or_else(|| format!("{url} is not a ws:// URL"))?;
        let host = rest.split('/').next().unwrap_or(rest);
        let stream = TcpStream::connect(host).map_err(|e| format!("{host}: {e}"))?;
        let (socket, _) = tungstenite::client(url, stream.try_clone().map_err(|e| e.to_string())?)
            .map_err(|e| format!("{url}: {e}"))?;
        let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let reader = WebSocket::from_raw_socket(reader_stream, Role::Client, None);
        let pending: Waiting = Arc::default();
        let waiting = pending.clone();
        thread::spawn(move || read(reader, waiting, events));
        Ok(Cdp {
            writer: Mutex::new(socket),
            next: AtomicU64::new(1),
            pending,
            stream,
        })
    }

    /// Send `method` to the browser, or to a tab's `session`, and wait for
    /// its answer: its result, or the browser's error message.
    pub fn call(
        &self,
        session: Option<&str>,
        method: &str,
        params: Json,
        within: Duration,
    ) -> Result<Json, String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (answer, answered) = mpsc::channel();
        lock(&self.pending).insert(id, answer);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        let sent = lock(&self.writer).send(Message::text(message.to_string()));
        if let Err(e) = sent {
            lock(&self.pending).remove(&id);
            return Err(format!("the browser's connection: {e}"));
        }
        let deadline = Instant::now() + within;
        let got = answered.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        lock(&self.pending).remove(&id);
        match got {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "the browser did not answer {method} within {}s",
                within.as_secs()
            )),
            Err(RecvTimeoutError::Disconnected) => Err("the browser's connection closed".into()),
        }
    }
}

impl Drop for Cdp {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn read(mut socket: WebSocket<TcpStream>, pending: Waiting, events: impl Fn(Event)) {
    loop {
        let text = match socket.read() {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        let Ok(mut message) = serde_json::from_str::<Json>(&text) else {
            continue;
        };
        if let Some(id) = message["id"].as_u64() {
            let answer = match message["error"]["message"].as_str() {
                Some(error) => Err(error.to_string()),
                None => Ok(message["result"].take()),
            };
            if let Some(waiting) = lock(&pending).remove(&id) {
                let _ = waiting.send(answer);
            }
        } else if let Some(method) = message["method"].as_str() {
            events(Event {
                method: method.to_string(),
                params: message["params"].take(),
                session: message["sessionId"].as_str().map(str::to_string),
            });
        }
    }
    // Every command still waiting hears that the connection closed.
    lock(&pending).clear();
    events(Event {
        method: CLOSED.into(),
        params: Json::Null,
        session: None,
    });
}

/// The event that says the connection closed.
pub const CLOSED: &str = "harness.closed";

/// The browser's websocket URL from its HTTP endpoint (`host:port`), as
/// `/json/version` says it: how a browser reached through a tunnel is found.
pub fn discover(endpoint: &str) -> Result<String, String> {
    let host = endpoint
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let fail = |e: &dyn std::fmt::Display| format!("the browser at {host}: {e}");
    let mut stream = TcpStream::connect(&host).map_err(|e| fail(&e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| fail(&e))?;
    // Chrome answers only HTTP/1.1, and may keep the connection open, so
    // the body is read to its length.
    write!(
        stream,
        "GET /json/version HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| fail(&e))?;
    let mut reader = io::BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| fail(&e))? == 0 {
            return Err(fail(&"it closed the connection before answering"));
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().map_err(|e| fail(&e))?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| fail(&e))?;
    let version: Json = serde_json::from_slice(&body).map_err(|e| fail(&e))?;
    let url = version["webSocketDebuggerUrl"]
        .as_str()
        .ok_or_else(|| fail(&"its /json/version names no webSocketDebuggerUrl"))?;
    // A browser behind a tunnel names its own host; the tunnel's is the one
    // that reaches it.
    let path = url
        .strip_prefix("ws://")
        .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
        .unwrap_or_default();
    Ok(format!("ws://{host}{path}"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
