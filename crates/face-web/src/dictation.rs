//! Dictation: the page's microphone, relayed to Anthropic's speech-to-text
//! endpoint on the claude.ai login, and the transcript sent back as edits.
//!
//! **Why a relay.** A browser WebSocket cannot set `Authorization`, and the
//! token must not reach the page anyway. So the page streams raw 16-bit PCM
//! to the face, and the face streams it on with the [`Login`]'s token: the
//! same credential owner as the Messages API, read again for every session
//! and never refreshed.
//!
//! **Snapshots into edits.** The endpoint sends the whole session's text in
//! every frame, and a later frame may rewrite earlier words. The page is sent
//! only what changed: a word-level diff against the text it already has, as
//! [`Edit`]s in UTF-16 offsets, the unit of a JavaScript string. That keeps
//! the traffic linear in the length of a dictation, and lets the page keep
//! Max's own edits to the box while the words keep arriving.
//!
//! **One transcript per press.** A dropped endpoint socket is a fresh session
//! on reconnect, so the text so far is kept and the new session's text is
//! appended after it. The audio sent while reconnecting is lost.
//!
//! The protocol, as the dictation spike found it: binary frames of
//! little-endian 16-bit mono PCM; `{"type":"KeepAlive"}` every few seconds;
//! `{"type":"CloseStream"}` to finish; `TranscriptText` (or
//! `TranscriptInterim`) frames carrying the full text, and one
//! `TranscriptEndpoint` once the text is final, after `CloseStream`.

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_provider::Login;
use serde_json::{Value, json};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

/// Dictation on a claude.ai login.
pub struct Dictation {
    login: Login,
    tls: Arc<rustls::ClientConfig>,
    sample: Option<Vec<u8>>,
}

/// A stream whose reads can be made to give up quickly, so one thread can
/// serve both ends of a relay.
pub trait Socket: Read + Write {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

impl<T: Socket + ?Sized> Socket for &mut T {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        (**self).set_read_timeout(timeout)
    }
}

impl Socket for TcpStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }
}

impl<C, D> Socket for rustls::StreamOwned<C, TcpStream>
where
    C: std::ops::DerefMut + std::ops::Deref<Target = rustls::ConnectionCommon<D>>,
    D: rustls::SideData,
{
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

/// A change to the transcript the page holds: replace `cut` UTF-16 units at
/// `at` with `put`. A frame's edits are in ascending order, each in the
/// offsets of the text before the frame, and never overlap.
#[derive(Debug, PartialEq)]
pub struct Edit {
    pub at: usize,
    pub cut: usize,
    pub put: String,
}

const HOST: &str = "api.anthropic.com";
const PATH: &str = "/api/ws/speech_to_text/voice_stream";
/// Claude Code's parameters; only `encoding` is required.
const QUERY: &str = "encoding=linear16&channels=1&endpointing_ms=300&utterance_end_ms=1000\
    &language=en&use_conversation_engine=true";
const KEEPALIVE: Duration = Duration::from_secs(8);
/// How long a stopped dictation waits for its final text.
const DRAIN: Duration = Duration::from_secs(10);
/// How long each read waits before the relay turns to the other socket.
const POLL: Duration = Duration::from_millis(10);
/// The sample is sent at real time, in chunks this long, like a microphone.
const CHUNK: Duration = Duration::from_millis(100);
const SAMPLE_RATE: u32 = 16_000;
const RECONNECTS: u32 = 3;

impl Dictation {
    pub fn new(login: Login) -> Dictation {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        Dictation {
            login,
            tls: Arc::new(tls),
            sample: None,
        }
    }

    /// Offer the page a recording to dictate instead of its microphone: a
    /// 16 kHz mono 16-bit WAV file, sent at real time through the same
    /// relay. For a machine with no microphone.
    pub fn with_sample(mut self, wav: &Path) -> Result<Dictation, String> {
        self.sample = Some(read_wav(wav)?);
        Ok(self)
    }

    pub fn has_sample(&self) -> bool {
        self.sample.is_some()
    }

    /// Relay one press of the microphone: audio from `page` (or the sample,
    /// if `sample`) to the endpoint at `rate` Hz, edits back to `page`, until
    /// the page sends a text frame (stop) and the final text has arrived, or
    /// either side goes away.
    ///
    /// The page is sent `{"listening": true}` once the endpoint is connected,
    /// `{"edits": [{"at", "cut", "put"}]}` as the text changes, then
    /// `{"final": true}`; or `{"error": why}`.
    pub fn relay<S: Socket>(&self, page: &mut WebSocket<S>, rate: u32, sample: bool) {
        let sample = if sample { self.sample.as_deref() } else { None };
        let rate = if sample.is_some() { SAMPLE_RATE } else { rate };
        page.get_ref().set_read_timeout(Some(POLL)).ok();
        if let Err(e) = self.session(page, rate, sample) {
            let _ = send_page(page, json!({"error": e}));
        }
        let _ = page.close(None);
        let _ = page.flush();
    }

    fn session<S: Socket>(
        &self,
        page: &mut WebSocket<S>,
        rate: u32,
        sample: Option<&[u8]>,
    ) -> Result<(), String> {
        let mut up = self.connect(rate)?;
        send_page(page, json!({"listening": true}))?;
        let mut text = Transcript::default();
        let mut stopped: Option<Instant> = None;
        let mut keepalive = Instant::now();
        let started = Instant::now();
        let chunk = (SAMPLE_RATE as usize * 2 * CHUNK.as_millis() as usize) / 1000;
        let mut chunks = sample.map(|pcm| pcm.chunks(chunk));
        let mut sent = 0u32;
        let mut reconnects = 0;

        loop {
            // From the page: its audio, or the word to stop.
            match page.read() {
                Ok(Message::Binary(pcm)) if sample.is_none() && stopped.is_none() => {
                    let _ = up.send(Message::Binary(pcm));
                }
                Ok(Message::Text(_)) if stopped.is_none() => {
                    close(&mut up);
                    stopped = Some(Instant::now());
                }
                Ok(Message::Close(_)) => return Ok(()),
                Ok(_) => {}
                Err(e) if would_block(&e) => {}
                Err(_) => return Ok(()),
            }

            // The sample, paced like a microphone.
            if let Some(chunks) = chunks.as_mut().filter(|_| stopped.is_none())
                && started.elapsed() >= CHUNK * sent
            {
                match chunks.next() {
                    Some(c) => {
                        let _ = up.send(Message::binary(c.to_vec()));
                    }
                    None => {
                        close(&mut up);
                        stopped = Some(Instant::now());
                    }
                }
                sent += 1;
            }

            // A failed send is noticed by the next read, which reconnects.
            if keepalive.elapsed() >= KEEPALIVE {
                let _ = up.send(Message::text(r#"{"type":"KeepAlive"}"#));
                keepalive = Instant::now();
            }

            // From the endpoint: the text so far, as edits.
            match up.read() {
                Ok(Message::Text(frame)) => {
                    let frame: Value = serde_json::from_str(&frame).unwrap_or_default();
                    match frame["type"].as_str() {
                        Some("TranscriptText" | "TranscriptInterim") => {
                            let data = frame["data"].as_str().unwrap_or_default();
                            let edits = text.session(data);
                            if !edits.is_empty() {
                                send_page(page, json!({"edits": edits_json(&edits)}))?;
                            }
                        }
                        Some("TranscriptEndpoint") if stopped.is_some() => {
                            return send_page(page, json!({"final": true}));
                        }
                        Some("TranscriptError" | "error") => {
                            return Err(format!("dictation endpoint: {frame}"));
                        }
                        _ => {}
                    }
                }
                Err(e) if would_block(&e) => {}
                Ok(Message::Close(_)) | Err(_) if stopped.is_some() => {
                    return send_page(page, json!({"final": true}));
                }
                Ok(Message::Close(_)) | Err(_) => {
                    // A fresh session: keep what was said.
                    reconnects += 1;
                    if reconnects > RECONNECTS {
                        return Err("the dictation endpoint keeps dropping the session".into());
                    }
                    text.keep();
                    up = self.connect(rate)?;
                }
                Ok(_) => {}
            }

            if stopped.is_some_and(|t| t.elapsed() > DRAIN) {
                return send_page(page, json!({"final": true}));
            }
        }
    }

    /// A new session on the endpoint, with the login's current token.
    fn connect(
        &self,
        rate: u32,
    ) -> Result<WebSocket<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>, String> {
        let token = self.login.token().map_err(|e| e.to_string())?;
        let url = format!("wss://{HOST}{PATH}?{QUERY}&sample_rate={rate}");
        let mut request = url.into_client_request().map_err(|e| e.to_string())?;
        let bearer = token
            .bearer()
            .parse()
            .map_err(|_| "the token is not a header")?;
        request.headers_mut().insert("authorization", bearer);

        let tcp = TcpStream::connect((HOST, 443)).map_err(|e| format!("connect: {e}"))?;
        let name = HOST.try_into().expect("a valid host name");
        let tls = rustls::ClientConnection::new(Arc::clone(&self.tls), name)
            .map_err(|e| e.to_string())?;
        let stream = rustls::StreamOwned::new(tls, tcp);
        let (mut up, _) = tungstenite::client(request, stream).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(tungstenite::Error::Http(r)) => {
                let body = r.body().as_deref().unwrap_or_default();
                let body = String::from_utf8_lossy(body);
                format!("dictation endpoint refused: HTTP {} {body}", r.status())
            }
            e => format!("dictation endpoint: {e}"),
        })?;
        up.get_ref().set_read_timeout(Some(POLL)).ok();
        up.send(Message::text(r#"{"type":"KeepAlive"}"#))
            .map_err(upstream)?;
        Ok(up)
    }
}

/// End the audio: the endpoint sends the final text, then its endpoint.
fn close<S: Read + Write>(up: &mut WebSocket<S>) {
    let _ = up.send(Message::text(r#"{"type":"CloseStream"}"#));
}

fn send_page<S: Socket>(page: &mut WebSocket<S>, v: Value) -> Result<(), String> {
    page.send(Message::text(v.to_string()))
        .map_err(|e| format!("page: {e}"))
}

fn upstream(e: tungstenite::Error) -> String {
    format!("dictation endpoint: {e}")
}

fn would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(e)
        if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

fn edits_json(edits: &[Edit]) -> Value {
    edits
        .iter()
        .map(|e| json!({"at": e.at, "cut": e.cut, "put": e.put}))
        .collect()
}

/// One press's text: what earlier sessions said, then the current one's.
#[derive(Default)]
struct Transcript {
    kept: String,
    session: String,
}

impl Transcript {
    fn shown(&self) -> String {
        match (self.kept.is_empty(), self.session.is_empty()) {
            (false, false) => format!("{} {}", self.kept, self.session),
            (false, true) => self.kept.clone(),
            _ => self.session.clone(),
        }
    }

    /// The current session's text is now `text`: the edits that make it so.
    fn session(&mut self, text: &str) -> Vec<Edit> {
        let before = self.shown();
        self.session = text.to_owned();
        edits(&before, &self.shown())
    }

    /// The session ended early: its text is kept, and a new one starts.
    fn keep(&mut self) {
        self.kept = self.shown();
        self.session.clear();
    }
}

/// The edits that turn `old` into `new`, word by word.
///
/// A word carries the whitespace before it, so a word added at the end is
/// one insertion and a rewritten word is one replacement. The common start
/// and end are trimmed first, so the quadratic part is only the region that
/// changed; past a bound, that region is one replacement.
pub fn edits(old: &str, new: &str) -> Vec<Edit> {
    let a = words(old);
    let b = words(new);
    let start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let end = a[start..]
        .iter()
        .rev()
        .zip(b[start..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_mid, b_mid) = (&a[start..a.len() - end], &b[start..b.len() - end]);
    let mut at: usize = a[..start].iter().map(|w| utf16(w)).sum();

    let (n, m) = (a_mid.len(), b_mid.len());
    let mut out = Vec::new();
    if n * m > 1 << 20 {
        out.push(Edit {
            at,
            cut: a_mid.iter().map(|w| utf16(w)).sum(),
            put: b_mid.concat(),
        });
        return out;
    }
    // lcs[i][j]: the longest common subsequence of a_mid[i..] and b_mid[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a_mid[i] == b_mid[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut open: Option<Edit> = None;
    while i < n || j < m {
        if i < n && j < m && a_mid[i] == b_mid[j] {
            out.extend(open.take());
            at += utf16(a_mid[i]);
            i += 1;
            j += 1;
            continue;
        }
        let edit = open.get_or_insert_with(|| Edit {
            at,
            cut: 0,
            put: String::new(),
        });
        if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            edit.put.push_str(b_mid[j]);
            j += 1;
        } else {
            edit.cut += utf16(a_mid[i]);
            at += utf16(a_mid[i]);
            i += 1;
        }
    }
    out.extend(open);
    out
}

/// `text` cut before each run of whitespace that follows a word.
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    let mut prev_word = false;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() && prev_word {
            out.push(&text[from..i]);
            from = i;
        }
        prev_word = !c.is_whitespace();
    }
    if from < text.len() {
        out.push(&text[from..]);
    }
    out
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The PCM of a 16 kHz mono 16-bit WAV file.
fn read_wav(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut format_ok = false;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                let field = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
                let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                format_ok = field(0) == 1 && field(2) == 1 && rate == 16_000 && field(14) == 16;
            }
            b"data" if format_ok => return Ok(body.to_vec()),
            b"data" => return Err("the WAV must be 16 kHz mono 16-bit PCM".into()),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    Err("the WAV has no data chunk".into())
}
