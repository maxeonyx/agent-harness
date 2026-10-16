//! The Anthropic Messages API on a Claude subscription token, streamed.
//!
//! Three things make a subscription token work, as measured in Max's plugin:
//! the `oauth-2025-04-20` beta, the identity string as `system[0]`, and a
//! system prompt free of one fingerprinted sentence. The caller owns the
//! request body, so the third is the caller's to keep; the identity is
//! prepended here because without it Opus refuses with a fake 429.

use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

pub const IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";
const URL: &str = "https://api.anthropic.com/v1/messages";
const CLAIM: &str = "anthropic-ratelimit-unified-representative-claim";

/// The OAuth access token. Deliberately not `Debug` or `Display`.
pub struct Token(String);

impl Token {
    /// Reads Claude Code's access token. Never refreshes: refreshing rotates
    /// the token and logs out the Claude Code session that owns it.
    pub fn from_claude_code(path: &Path) -> Result<Token, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Auth(format!("cannot read {}: {e}", path.display())))?;
        let creds: Value = serde_json::from_str(&text)
            .map_err(|_| Error::Auth(format!("{} is not JSON", path.display())))?;
        let oauth = &creds["claudeAiOauth"];
        let (Some(token), Some(expires_ms)) =
            (oauth["accessToken"].as_str(), oauth["expiresAt"].as_u64())
        else {
            return Err(Error::Auth(format!(
                "{} has no claudeAiOauth.accessToken/expiresAt",
                path.display()
            )));
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        if expires_ms <= now_ms {
            return Err(Error::Auth(
                "the Claude Code access token has expired; use Claude Code once so it refreshes, then rerun".into(),
            ));
        }
        Ok(Token(token.to_owned()))
    }
}

/// One server-sent event: its `event:` name and its parsed `data:`.
pub struct Event {
    pub name: String,
    pub data: Value,
}

/// What a finished stream tells the caller beyond its events.
pub struct Outcome {
    /// `five_hour` (or another window) is the plan; `overage` is third-party billing.
    pub claim: Option<String>,
    /// `message_start` usage, updated by `message_delta` usage.
    pub usage: Value,
}

#[derive(Debug)]
pub enum Error {
    Auth(String),
    Transport(String),
    Status(u16, String),
    Stream(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Auth(m) => write!(f, "auth: {m}"),
            Error::Transport(m) => write!(f, "transport: {m}"),
            Error::Status(s, body) => write!(f, "HTTP {s}: {body}"),
            Error::Stream(m) => write!(f, "stream: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Client {
    agent: ureq::Agent,
    token: Token,
}

impl Client {
    pub fn new(token: Token) -> Client {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        Client { agent, token }
    }

    /// Sends `body` with `stream: true` and calls `on_event` for each event as
    /// it arrives. `system[0]` becomes the identity string.
    pub fn stream(
        &self,
        mut body: Value,
        mut on_event: impl FnMut(&Event),
    ) -> Result<Outcome, Error> {
        body["stream"] = Value::Bool(true);
        with_identity(&mut body);

        let response = self
            .agent
            .post(URL)
            .header("authorization", &format!("Bearer {}", self.token.0))
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .send(body.to_string().as_bytes())
            .map_err(|e| Error::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let claim = header(CLAIM);

        let mut reader = response.into_body().into_reader();
        if status != 200 {
            let mut text = String::new();
            let _ = std::io::Read::read_to_string(&mut reader, &mut text);
            return Err(Error::Status(status, text));
        }

        let mut usage = Value::Null;
        for event in Events::new(BufReader::new(reader)) {
            let event = event?;
            match event.name.as_str() {
                "message_start" => usage = event.data["message"]["usage"].clone(),
                "message_delta" => merge(&mut usage, &event.data["usage"]),
                "error" => return Err(Error::Stream(event.data["error"].to_string())),
                _ => {}
            }
            on_event(&event);
        }
        Ok(Outcome { claim, usage })
    }
}

fn with_identity(body: &mut Value) {
    let identity = serde_json::json!({ "type": "text", "text": IDENTITY });
    let system = match body["system"].take() {
        Value::Array(blocks) => blocks,
        Value::String(text) => vec![serde_json::json!({ "type": "text", "text": text })],
        _ => vec![],
    };
    let has = system.first().is_some_and(|b| b["text"] == IDENTITY);
    body["system"] = Value::Array(if has {
        system
    } else {
        std::iter::once(identity).chain(system).collect()
    });
}

fn merge(into: &mut Value, from: &Value) {
    if let (Value::Object(into), Value::Object(from)) = (into, from) {
        for (k, v) in from {
            if !v.is_null() {
                into.insert(k.clone(), v.clone());
            }
        }
    }
}

/// Server-sent events parsed line by line from a byte stream.
struct Events<R> {
    lines: std::io::Lines<R>,
}

impl<R: BufRead> Events<R> {
    fn new(reader: R) -> Self {
        Events {
            lines: reader.lines(),
        }
    }
}

impl<R: BufRead> Iterator for Events<R> {
    type Item = Result<Event, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (mut name, mut data) = (String::new(), String::new());
        loop {
            let line = match self.lines.next() {
                None if data.is_empty() => return None,
                None => String::new(),
                Some(Err(e)) => return Some(Err(Error::Transport(e.to_string()))),
                Some(Ok(line)) => line,
            };
            if line.is_empty() {
                if data.is_empty() {
                    continue;
                }
                return Some(
                    serde_json::from_str(&data)
                        .map(|data| Event { name, data })
                        .map_err(|e| Error::Stream(format!("bad event data: {e}"))),
                );
            }
            if let Some(v) = line.strip_prefix("event:") {
                name = v.trim().to_owned();
            } else if let Some(v) = line.strip_prefix("data:") {
                data.push_str(v.trim_start());
            }
        }
    }
}
