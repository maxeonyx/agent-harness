//! Just enough HTTP/1.1 for one page, its wasm, its contexts and its two
//! sockets (the tab's link and dictation): each request answered on its
//! own connection. Nothing is posted: what a tab says goes up its link,
//! except an error report, which is a GET, as the link may be what failed.

use std::io::{self, BufRead, BufReader, Read, Write};

pub struct Request {
    pub path: String,
    query: String,
    /// `Sec-WebSocket-Key`, when the request asks to become a WebSocket.
    pub websocket_key: Option<String>,
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

pub fn read_request<S: Read + Write>(stream: S) -> io::Result<(Request, S)> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let mut reader = BufReader::new(stream.take(HEAD_LIMIT));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Err(invalid("not a GET"));
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut request = Request {
        path: path.to_string(),
        query: query.to_string(),
        websocket_key: None,
    };
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
        if name.eq_ignore_ascii_case("sec-websocket-key") {
            request.websocket_key = Some(value.to_string());
        }
    }
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
