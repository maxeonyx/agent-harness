//! Stream a WAV file to Anthropic's dictation endpoint, the one Claude Code's
//! voice mode uses, and print what comes back.
//!
//! The file is sent the way a microphone would send it: 16 kHz mono 16-bit PCM,
//! in chunks paced at real time (or as fast as possible with `--fast`). Every
//! frame from the server goes to stderr with its arrival time; the final
//! transcript goes to stdout.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const URL: &str = "wss://api.anthropic.com/api/ws/speech_to_text/voice_stream";
// Claude Code 2.1.283 sends these parameters. Only `encoding` is required, and
// the bearer token is the only header the server checks.
const QUERY: &str = "encoding=linear16&sample_rate=16000&channels=1&endpointing_ms=300\
    &utterance_end_ms=1000&language=en&use_conversation_engine=true";
const RATE: usize = 16_000;
const CHUNK_MS: usize = 100;
const KEEPALIVE: Duration = Duration::from_secs(8);
const DRAIN: Duration = Duration::from_secs(10);

const USAGE: &str = "usage: dictation [--fast] [--interims] FILE.wav
  FILE.wav    16 kHz mono 16-bit PCM WAV
  --fast      send the audio as fast as possible instead of at real time
  --interims  ask for typed interim frames (forward_interims=typed)";

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut fast = false;
    let mut interims = false;
    let mut file = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--fast" => fast = true,
            "--interims" => interims = true,
            _ if file.is_none() && !arg.starts_with('-') => file = Some(arg),
            _ => return Err(USAGE.into()),
        }
    }
    let pcm = read_wav(Path::new(&file.ok_or(USAGE)?))?;
    let token = access_token()?;

    let mut url = format!("{URL}?{QUERY}");
    if interims {
        url.push_str("&forward_interims=typed");
    }
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    let headers = request.headers_mut();
    let bearer = format!("Bearer {token}");
    headers.insert("authorization", bearer.parse().map_err(|_| "bad token")?);

    let start = Instant::now();
    let (mut ws, response) = tungstenite::connect(request).map_err(|e| match e {
        tungstenite::Error::Http(r) => format!(
            "upgrade rejected: HTTP {} {}",
            r.status(),
            String::from_utf8_lossy(r.body().as_deref().unwrap_or_default())
        ),
        e => e.to_string(),
    })?;
    log(start, &format!("connected: HTTP {}", response.status()));
    set_read_timeout(&ws, Duration::from_millis(10))?;

    let chunk = RATE * 2 * CHUNK_MS / 1000;
    let chunks: Vec<&[u8]> = pcm.chunks(chunk).collect();
    let audio_secs = pcm.len() as f64 / (RATE * 2) as f64;
    log(
        start,
        &format!(
            "audio: {audio_secs:.1}s in {} chunks of {CHUNK_MS} ms",
            chunks.len()
        ),
    );

    send(&mut ws, Message::text(r#"{"type":"KeepAlive"}"#))?;
    let streaming = Instant::now();
    let mut last_keepalive = Instant::now();
    let mut sent = 0;
    let mut closed_at = None;
    let mut finals = Vec::new();
    let mut pending = String::new();

    loop {
        let audio_due = streaming + Duration::from_millis((sent * CHUNK_MS) as u64);
        if sent < chunks.len() && (fast || Instant::now() >= audio_due) {
            send(&mut ws, Message::binary(chunks[sent].to_vec()))?;
            sent += 1;
            if sent == chunks.len() {
                send(&mut ws, Message::text(r#"{"type":"CloseStream"}"#))?;
                log(start, "sent CloseStream");
                closed_at = Some(Instant::now());
            }
        }
        if last_keepalive.elapsed() >= KEEPALIVE {
            send(&mut ws, Message::text(r#"{"type":"KeepAlive"}"#))?;
            last_keepalive = Instant::now();
        }
        if closed_at.is_some_and(|t| t.elapsed() > DRAIN) {
            log(start, "no endpoint within 10s of CloseStream; giving up");
            break;
        }

        match ws.read() {
            Ok(Message::Text(text)) => {
                log(start, &format!("frame: {text}"));
                let frame: Value = serde_json::from_str(&text).unwrap_or_default();
                let data = frame["data"].as_str().unwrap_or_default();
                match frame["type"].as_str() {
                    Some("TranscriptText" | "TranscriptInterim") if !data.is_empty() => {
                        pending = data.to_string();
                    }
                    Some("TranscriptEndpoint") => {
                        if !pending.is_empty() {
                            finals.push(std::mem::take(&mut pending));
                        }
                        if let Some(t) = closed_at {
                            let ms = t.elapsed().as_millis();
                            log(start, &format!("final endpoint {ms} ms after CloseStream"));
                            break;
                        }
                    }
                    Some("TranscriptError" | "error") => return Err(format!("server: {text}")),
                    _ => {}
                }
            }
            Ok(Message::Close(frame)) => {
                log(start, &format!("server closed: {frame:?}"));
                break;
            }
            Ok(other) => log(start, &format!("frame: {other:?}")),
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    if !pending.is_empty() {
        finals.push(pending);
    }
    let _ = ws.close(None);
    println!("{}", finals.join(" "));
    Ok(())
}

fn send(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>, message: Message) -> Result<(), String> {
    ws.send(message).map_err(|e| format!("send: {e}"))
}

fn set_read_timeout(
    ws: &WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) -> Result<(), String> {
    let tcp = match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return Err("unexpected stream type".into()),
    };
    tcp.set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())
}

fn log(start: Instant, line: &str) {
    eprintln!("[{:6.2}s] {line}", start.elapsed().as_secs_f64());
}

/// The claude.ai OAuth access token Claude Code keeps. Never refreshed here:
/// refreshing rotates the token and can sign Claude Code out.
fn access_token() -> Result<String, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is unset")?;
    let path = Path::new(&home).join(".claude/.credentials.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: Value = serde_json::from_str(&text).map_err(|_| "credentials are not JSON")?;
    let oauth = &json["claudeAiOauth"];
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    if oauth["expiresAt"].as_u64().is_some_and(|t| t <= now_ms) {
        return Err("access token has expired; run `claude` once to refresh it".into());
    }
    oauth["accessToken"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "no claudeAiOauth.accessToken; log in with `claude` first".into())
}

/// The PCM samples of a 16 kHz mono 16-bit WAV file.
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
            b"data" => {
                return Err("WAV must be 16 kHz mono 16-bit PCM; convert with \
                `ffmpeg -i IN -ar 16000 -ac 1 -sample_fmt s16 OUT.wav`"
                    .into());
            }
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    Err("WAV has no data chunk".into())
}
