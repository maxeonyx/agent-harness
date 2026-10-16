//! Just enough HTTP/1.1 for one page, its event stream and its JSON: `GET`
//! requests, each answered on its own connection.

use std::io::{self, BufRead, BufReader, Read, Write};

use harness_core::id::Seq;

pub struct Request {
    pub path: String,
    /// The last event id the browser saw, sent when it reconnects; or the
    /// `after` query parameter, for a client that is not a browser.
    pub last_event_id: Option<Seq>,
}

/// The longest request head accepted.
const HEAD_LIMIT: u64 = 16 * 1024;

pub fn read_request<S: Read + Write>(stream: S) -> io::Result<(Request, S)> {
    let mut reader = BufReader::new(stream.take(HEAD_LIMIT));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a GET"));
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut last_event_id = query
        .split('&')
        .find_map(|p| p.strip_prefix("after="))
        .and_then(|s| s.parse().ok())
        .map(Seq);
    let path = path.to_string();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("last-event-id")
        {
            last_event_id = value.trim().parse().ok().map(Seq).or(last_event_id);
        }
    }
    let stream = reader.into_inner().into_inner();
    Ok((
        Request {
            path,
            last_event_id,
        },
        stream,
    ))
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

pub fn not_found(stream: &mut impl Write) -> io::Result<()> {
    stream
        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
    stream.flush()
}

pub fn start_events(stream: &mut impl Write) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\n\
          Connection: close\r\n\r\nretry: 1000\n\n",
    )?;
    stream.flush()
}
