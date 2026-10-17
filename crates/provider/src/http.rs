//! One HTTP/1.1 exchange over TLS, on a socket a stop cuts: all the HTTP
//! the provider needs, so that a request is cancellable wherever it is
//! blocked (connecting, in the handshake, waiting for the first byte,
//! mid-stream) without a client library that hides its socket.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::time::Duration;

use harness_runtime::{Cancel, Socket};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// How long a connect may take.
const CONNECT: Duration = Duration::from_secs(30);
/// How long a response may go with nothing arriving. A long thinking block
/// still sends pings well inside this.
const IDLE: Duration = Duration::from_secs(300);

type Stream = BufReader<StreamOwned<ClientConnection, Socket>>;

/// A response whose head has arrived; its body is read as it streams.
pub struct Response {
    pub status: u16,
    headers: Vec<(String, String)>,
    pub body: Body,
}

impl Response {
    /// A header's value, by its lowercase name.
    pub fn header(&self, name: &str) -> Option<&str> {
        find(&self.headers, name)
    }
}

fn find<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Send `body` to `https://{host}{path}` with `headers`, and read the
/// response's head. Every wait ends in [`harness_runtime::Cancelled`] once
/// `cancel` is requested, and so does every read of the body.
pub fn post(
    tls: &Arc<ClientConfig>,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    cancel: &Cancel,
) -> io::Result<Response> {
    let socket = Socket::connect(host, 443, cancel, CONNECT, IDLE)?;
    let name = ServerName::try_from(host.to_owned()).map_err(io::Error::other)?;
    let connection = ClientConnection::new(Arc::clone(tls), name).map_err(io::Error::other)?;
    let mut stream = StreamOwned::new(connection, socket);
    let mut head = format!(
        "POST {path} HTTP/1.1\r\nhost: {host}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;

    let mut stream = BufReader::new(stream);
    let status_line = line(&mut stream)?;
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid(format!("not a status line: {status_line:?}")))?;
    let mut headers = Vec::new();
    loop {
        let header = line(&mut stream)?;
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let header = |name: &str| find(&headers, name).map(str::to_owned);
    let chunked = header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
    let length = header("content-length").and_then(|v| v.parse::<u64>().ok());
    let body = match (chunked, length) {
        (true, _) => Body::Chunked { stream, left: 0 },
        (false, Some(left)) => Body::Length { stream, left },
        (false, None) => Body::Close(stream),
    };
    Ok(Response {
        status,
        headers,
        body,
    })
}

/// A response body, framed as its head said.
pub enum Body {
    Chunked {
        stream: Stream,
        left: usize,
    },
    Length {
        stream: Stream,
        left: u64,
    },
    /// Until the server closes the connection.
    Close(Stream),
    Done,
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Body::Done => Ok(0),
            Body::Close(stream) => stream.read(buf),
            Body::Length { stream, left } => {
                if *left == 0 {
                    return Ok(0);
                }
                let max = (*left).min(buf.len() as u64) as usize;
                let n = stream.read(&mut buf[..max])?;
                if n == 0 {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                *left -= n as u64;
                Ok(n)
            }
            Body::Chunked { stream, left } => {
                if *left == 0 {
                    let size = line(stream)?;
                    let size = size.split(';').next().unwrap_or_default().trim();
                    let size = usize::from_str_radix(size, 16)
                        .map_err(|_| invalid(format!("not a chunk size: {size:?}")))?;
                    if size == 0 {
                        // Trailers, then the end.
                        while !line(stream)?.is_empty() {}
                        *self = Body::Done;
                        return Ok(0);
                    }
                    *left = size;
                }
                let max = (*left).min(buf.len());
                let n = stream.read(&mut buf[..max])?;
                if n == 0 {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                *left -= n;
                if *left == 0 && !line(stream)?.is_empty() {
                    return Err(invalid("a chunk runs past its size".into()));
                }
                Ok(n)
            }
        }
    }
}

/// One CRLF-terminated line, without its ending.
fn line(stream: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    if stream.read_line(&mut line)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let end = line.trim_end_matches(['\r', '\n']).len();
    line.truncate(end);
    Ok(line)
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
