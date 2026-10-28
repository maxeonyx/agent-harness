//! The claude.ai login: one owner of the subscription credential, for every
//! Anthropic endpoint the harness uses (the Messages API, the usage meter,
//! dictation).
//!
//! **The harness's own login.** It signs in as Claude Code does, with Claude
//! Code's public OAuth client, PKCE and scopes (all read from Claude Code
//! 2.1.310's `rn()` config and `wVr`, `Tqo` and its refresh function), so
//! every endpoint that accepts Claude Code's token accepts this one. It is a
//! second login beside Claude Code's: refresh tokens rotate, so sharing one
//! would sign the other out. Claude Code's `~/.claude/.credentials.json` is
//! never touched.
//!
//! **Kept fresh.** The tokens live in `~/.local/share/agent-harness/login.json`
//! (mode 0600, replaced atomically). An access token lasts about 8 hours; it
//! is refreshed from a random point 1 to 2 hours before it expires, so the
//! brains that share the file do not all refresh at once, and again when a
//! request is refused with 401. A refresh holds `login.lock`, and reads the
//! file again under it: the brain that loses the race uses the winner's
//! token instead of spending the rotated refresh token a second time.
//!
//! **Signing in once.** [`Login::begin`] gives the URL Max opens; claude.ai
//! then shows him a code, which [`Login::finish`] exchanges for the tokens.
//! What `begin` made (the PKCE verifier and state) waits in
//! `login.pending.json`, so the two halves can run in different processes.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_runtime::Cancel;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};

use crate::http;

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE: &str = "https://claude.com/cai/oauth/authorize";
const TOKEN_HOST: &str = "platform.claude.com";
const TOKEN_PATH: &str = "/v1/oauth/token";
const REDIRECT: &str = "https://platform.claude.com/oauth/code/callback";
const SCOPES: &str = "org:create_api_key user:profile user:inference \
    user:sessions:claude_code user:mcp_servers user:file_upload";

/// How long before expiry a refresh starts: at least the first, plus up to
/// the second at random.
const EARLY: Duration = Duration::from_secs(3600);
const JITTER: Duration = Duration::from_secs(3600);

/// The harness's own claude.ai login, in its file.
#[derive(Clone, Debug)]
pub struct Login {
    path: PathBuf,
}

/// An access token, valid when it was read. It has no `Debug` or `Display`,
/// so it cannot be printed by accident.
pub struct Token(String);

/// Why there is no usable token. It never contains a token.
#[derive(Debug)]
pub struct LoginError(pub String);

/// The tokens as the file holds them.
struct Tokens {
    access: String,
    refresh: String,
    /// Unix milliseconds.
    expires_at: u64,
    refresh_at: u64,
}

impl Login {
    /// The harness's login, `~/.local/share/agent-harness/login.json`.
    pub fn own() -> Result<Login, LoginError> {
        let home = std::env::var_os("HOME").ok_or(LoginError("HOME is not set".into()))?;
        Ok(Login::at(
            PathBuf::from(home).join(".local/share/agent-harness/login.json"),
        ))
    }

    /// A login kept in `path`.
    pub fn at(path: PathBuf) -> Login {
        Login { path }
    }

    /// The current access token, refreshed first if it is due, or why there
    /// is none.
    pub fn token(&self) -> Result<Token, LoginError> {
        let tokens = self.read()?;
        if now_ms() < tokens.refresh_at {
            return Ok(Token(tokens.access));
        }
        match self.refresh(None) {
            Ok(token) => Ok(token),
            // A failed early refresh leaves the token it was replacing.
            Err(_) if now_ms() < tokens.expires_at => Ok(Token(tokens.access)),
            Err(e) => Err(e),
        }
    }

    /// A fresh token after `stale` was refused with 401: the file's, if
    /// another request has refreshed it since, else a new one.
    pub fn refused(&self, stale: &Token) -> Result<Token, LoginError> {
        self.refresh(Some(stale))
    }

    /// Refresh the token whenever it is due, on a thread of its own, so a
    /// login left idle for days is still fresh when it is next used.
    pub fn keep_fresh(&self) {
        let login = self.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(600));
                if let Ok(tokens) = login.read()
                    && now_ms() >= tokens.refresh_at
                    && let Err(e) = login.refresh(None)
                {
                    eprintln!("harness: {e}");
                }
            }
        });
    }

    /// Start a sign-in: the URL Max opens. Its verifier and state wait in
    /// `login.pending.json` for [`Login::finish`].
    pub fn begin(&self) -> Result<String, LoginError> {
        let verifier = base64url(&random::<32>()?);
        let state = base64url(&random::<32>()?);
        let challenge =
            base64url(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()).as_ref());
        let pending = json!({ "verifier": verifier, "state": state });
        write_private(&self.pending(), pending.to_string().as_bytes())?;
        let query = [
            ("code", "true"),
            ("client_id", CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", REDIRECT),
            ("scope", SCOPES),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
        ];
        Ok(format!("{AUTHORIZE}?{}", form(&query)))
    }

    /// Finish a sign-in with the code claude.ai showed Max (`CODE#STATE`).
    pub fn finish(&self, pasted: &str) -> Result<(), LoginError> {
        let pending_path = self.pending();
        let text = std::fs::read_to_string(&pending_path).map_err(|_| {
            LoginError("no sign-in was started: start one, then paste its code".into())
        })?;
        let pending: Value = serde_json::from_str(&text)
            .map_err(|_| LoginError(format!("{} is not JSON", pending_path.display())))?;
        let Some((code, state)) = pasted.trim().split_once('#') else {
            return Err(LoginError(
                "the code is CODE#STATE, as claude.ai shows it: paste all of it".into(),
            ));
        };
        if Some(state) != pending["state"].as_str() {
            return Err(LoginError(
                "the code is from another sign-in: open the latest URL".into(),
            ));
        }
        let response = post(&json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": REDIRECT,
            "client_id": CLIENT_ID,
            "code_verifier": pending["verifier"],
            "state": state,
        }))?;
        let _lock = self.lock()?;
        self.write(&tokens_from(&response, None)?)?;
        let _ = std::fs::remove_file(pending_path);
        Ok(())
    }

    /// Refresh under the lock, unless the file under it already holds a
    /// token newer than the one that was due (or `stale`).
    fn refresh(&self, stale: Option<&Token>) -> Result<Token, LoginError> {
        let _lock = self.lock()?;
        let tokens = self.read()?;
        let moved_on = match stale {
            Some(stale) => stale.0 != tokens.access,
            None => now_ms() < tokens.refresh_at,
        };
        if moved_on {
            return Ok(Token(tokens.access));
        }
        let response = post(&json!({
            "grant_type": "refresh_token",
            "refresh_token": tokens.refresh,
            "client_id": CLIENT_ID,
            "scope": SCOPES,
        }))?;
        let fresh = tokens_from(&response, Some(&tokens.refresh))?;
        self.write(&fresh)?;
        Ok(Token(fresh.access))
    }

    fn read(&self) -> Result<Tokens, LoginError> {
        let path = self.path.display();
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(LoginError(
                    "the harness is not signed in to claude.ai: sign in once".into(),
                ));
            }
            Err(e) => return Err(LoginError(format!("cannot read {path}: {e}"))),
        };
        let file: Value =
            serde_json::from_str(&text).map_err(|_| LoginError(format!("{path} is not JSON")))?;
        let field = |name: &str| {
            file[name]
                .as_str()
                .map(str::to_owned)
                .ok_or(LoginError(format!("{path} has no {name}")))
        };
        let number = |name: &str| {
            file[name]
                .as_u64()
                .ok_or(LoginError(format!("{path} has no {name}")))
        };
        Ok(Tokens {
            access: field("access_token")?,
            refresh: field("refresh_token")?,
            expires_at: number("expires_at")?,
            refresh_at: number("refresh_at")?,
        })
    }

    fn write(&self, tokens: &Tokens) -> Result<(), LoginError> {
        let file = json!({
            "access_token": tokens.access,
            "refresh_token": tokens.refresh,
            "expires_at": tokens.expires_at,
            "refresh_at": tokens.refresh_at,
        });
        write_private(&self.path, file.to_string().as_bytes())
    }

    fn lock(&self) -> Result<File, LoginError> {
        let path = self.path.with_extension("lock");
        let file = open_private(&path, false)?;
        file.lock()
            .map_err(|e| LoginError(format!("cannot lock {}: {e}", path.display())))?;
        Ok(file)
    }

    fn pending(&self) -> PathBuf {
        self.path.with_extension("pending.json")
    }
}

impl Token {
    /// The `Authorization` header's value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

/// The token endpoint's answer to `body`, as JSON.
fn post(body: &Value) -> Result<Value, LoginError> {
    let fail = |why: String| LoginError(format!("the claude.ai token endpoint: {why}"));
    let tls = http::tls().map_err(|e| fail(e.0))?;
    let body = body.to_string();
    let headers = [
        ("content-type", "application/json"),
        ("user-agent", concat!("harness/", env!("CARGO_PKG_VERSION"))),
    ];
    let mut response = http::exchange(
        &tls,
        "POST",
        TOKEN_HOST,
        TOKEN_PATH,
        &headers,
        body.as_bytes(),
        &Cancel::new(),
    )
    .map_err(|e| fail(e.to_string()))?;
    let mut text = String::new();
    response
        .body
        .read_to_string(&mut text)
        .map_err(|e| fail(e.to_string()))?;
    let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if response.status != 200 {
        // The error body names the error and never echoes a token.
        let why = parsed["error_description"]
            .as_str()
            .or(parsed["error"]["message"].as_str())
            .or(parsed["error"].as_str())
            .unwrap_or("");
        return Err(fail(format!("{} {why:.200}", response.status)));
    }
    Ok(parsed)
}

/// The tokens in the endpoint's `response`. A refresh that returns no new
/// refresh token keeps `refresh`.
fn tokens_from(response: &Value, refresh: Option<&str>) -> Result<Tokens, LoginError> {
    let missing = |name| LoginError(format!("the claude.ai token endpoint sent no {name}"));
    let access = response["access_token"]
        .as_str()
        .ok_or(missing("access_token"))?;
    let refresh = response["refresh_token"]
        .as_str()
        .or(refresh)
        .ok_or(missing("refresh_token"))?;
    let lifetime = response["expires_in"]
        .as_u64()
        .ok_or(missing("expires_in"))?
        * 1000;
    let jitter = u64::from_le_bytes(random::<8>()?) % JITTER.as_millis() as u64;
    let early = (EARLY.as_millis() as u64 + jitter).min(lifetime / 2);
    let now = now_ms();
    Ok(Tokens {
        access: access.to_owned(),
        refresh: refresh.to_owned(),
        expires_at: now + lifetime,
        refresh_at: now + lifetime - early,
    })
}

fn open_private(path: &Path, truncate: bool) -> Result<File, LoginError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| LoginError(format!("cannot make {}: {e}", dir.display())))?;
    }
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(truncate)
        .mode(0o600)
        .open(path)
        .map_err(|e| LoginError(format!("cannot open {}: {e}", path.display())))
}

/// Replace `path` with `bytes` atomically, readable by its owner alone.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), LoginError> {
    let temp = path.with_extension("new");
    let fail = |e: std::io::Error| LoginError(format!("cannot write {}: {e}", path.display()));
    let mut file = open_private(&temp, true)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    std::fs::rename(&temp, path).map_err(fail)
}

fn random<const N: usize>() -> Result<[u8; N], LoginError> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| LoginError("the system has no randomness to give".into()))?;
    Ok(bytes)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Base64url with no padding, as PKCE wants.
fn base64url(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(DIGITS[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// `application/x-www-form-urlencoded`, for the authorize URL's query.
fn form(pairs: &[(&str, &str)]) -> String {
    let encode = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                b' ' => "+".into(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "login: {}", self.0)
    }
}

impl std::error::Error for LoginError {}
