//! Just enough HTTP/1.1 for one page, its event stream, its JSON, the words
//! Max sends and the dictation socket: each request answered on its own
//! connection.

use std::io::{self, BufRead, BufReader, Read, Write};

use harness_core::id::Seq;

pub struct Request {
    pub method: String,
    pub path: String,
    query: String,
    /// The last event id the browser saw, sent when it reconnects; or the
    /// `after` query parameter, for a client that is not a browser.
    pub last_event_id: Option<Seq>,
    /// `Sec-WebSocket-Key`, when the request asks to become a WebSocket.
    pub websocket_key: Option<String>,
    pub body: Vec<u8>,
}

impl Request {
    /// A query parameter's value, undecoded.
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query.split('&').find_map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k == name).then_some(v)
        })
    }
}

/// The longest request head accepted.
const HEAD_LIMIT: u64 = 16 * 1024;
/// The longest body accepted: a message with large pastes in it.
const BODY_LIMIT: u64 = 16 * 1024 * 1024;

pub fn read_request<S: Read + Write>(stream: S) -> io::Result<(Request, S)> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let mut reader = BufReader::new(stream.take(HEAD_LIMIT));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (Some(method @ ("GET" | "POST")), Some(target)) = (parts.next(), parts.next()) else {
        return Err(invalid("not a GET or a POST"));
    };
    let method = method.to_string();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut request = Request {
        method,
        path: path.to_string(),
        query: query.to_string(),
        last_event_id: None,
        websocket_key: None,
        body: Vec::new(),
    };
    request.last_event_id = request.query("after").and_then(|s| s.parse().ok()).map(Seq);
    let mut length = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("last-event-id") {
            request.last_event_id = value.parse().ok().map(Seq).or(request.last_event_id);
        } else if name.eq_ignore_ascii_case("sec-websocket-key") {
            request.websocket_key = Some(value.to_string());
        } else if name.eq_ignore_ascii_case("content-length") {
            length = value.parse().map_err(|_| invalid("a bad Content-Length"))?;
        }
    }
    if length > BODY_LIMIT {
        return Err(invalid("the body is too long"));
    }
    let buffered = reader.buffer().len() as u64;
    reader.get_mut().set_limit(length.saturating_sub(buffered));
    request.body = vec![0; length as usize];
    reader.read_exact(&mut request.body)?;
    Ok((request, reader.into_inner().into_inner()))
}

pub fn respond(
    stream: &mut impl Write,
    content_type: &str,
    body: &[u8],
    immutable: bool,
) -> io::Result<()> {
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: {cache}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// A failure, with why as plain text.
pub fn refuse(stream: &mut impl Write, status: &str, why: &str) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{why}",
        why.len()
    )?;
    stream.flush()
}

pub fn not_found(stream: &mut impl Write) -> io::Result<()> {
    refuse(stream, "404 Not Found", "")
}

pub fn start_events(stream: &mut impl Write) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\n\
          Connection: close\r\n\r\nretry: 1000\n\n",
    )?;
    stream.flush()
}

/// Accept a WebSocket upgrade. The stream speaks WebSocket after this.
pub fn accept_websocket(stream: &mut impl Write, key: &str) -> io::Result<()> {
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    )?;
    stream.flush()
}
