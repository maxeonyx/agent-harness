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
//! **Internal state.** The tokens are a row of the `login` table in a
//! brain's database, beside its records. Every brain on the machine uses
//! the home brain's ([`Login::home`]) unless told another (`--login DB`),
//! so Max signs in once. The table is not one of the store's: a build that
//! does not know it leaves it alone.
//!
//! **Kept fresh.** An access token lasts about 8 hours; it is refreshed
//! from a random point 1 to 2 hours before it expires, so the brains that
//! share a login do not all refresh at once, and again when a request is
//! refused with 401. Refresh tokens rotate, so one must never be spent
//! twice: a refresh first claims the row in a transaction, which also
//! reads it again, and only the claimant sends the old refresh token. The
//! claim is a column, not a transaction held across the request, so the
//! brain's own commits to the file never wait on the network. A process
//! that finds the row claimed uses the token it has while that is still
//! valid, else waits for the claimant's; a claim left by a crash lapses
//! after a minute. A refresh token the endpoint rejects signs the harness
//! out: the tokens are deleted and [`LoginError::SignedOut`] says so.
//!
//! **Signing in once.** [`Login::begin`] gives the URL Max opens; claude.ai
//! then shows him a code, which [`Login::finish`] exchanges for the tokens.
//! What `begin` made (the PKCE verifier and state) waits in the row, so the
//! two halves can run in different processes.

use std::fmt;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_runtime::Cancel;
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
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
/// How long a claim to refresh lasts: far longer than the token endpoint
/// takes to answer.
const CLAIM: Duration = Duration::from_secs(60);

const TABLE: &str = "
CREATE TABLE IF NOT EXISTS login (
    one INTEGER PRIMARY KEY CHECK (one = 1),
    access TEXT,
    refresh TEXT,
    expires_at INTEGER,
    refresh_at INTEGER,
    claimed_until INTEGER NOT NULL DEFAULT 0,
    verifier TEXT,
    state TEXT
) STRICT;
INSERT OR IGNORE INTO login (one) VALUES (1);
";

/// The harness's own claude.ai login, in a brain's database.
#[derive(Clone, Debug)]
pub struct Login {
    db: PathBuf,
}

/// An access token, valid when it was read. It has no `Debug` or `Display`,
/// so it cannot be printed by accident.
pub struct Token(String);

/// Why there is no usable token. It never contains a token.
#[derive(Debug)]
pub enum LoginError {
    /// There are no tokens: never signed in, or the endpoint rejected the
    /// refresh token. Only Max signing in again ends it.
    SignedOut,
    /// The database or the token endpoint failed; it may pass.
    Failed(String),
}

/// The tokens as the row holds them.
struct Tokens {
    access: String,
    refresh: String,
    /// Unix milliseconds.
    expires_at: i64,
    refresh_at: i64,
}

impl Login {
    /// The home brain's login, in `~/.local/share/agent-harness/home/brain.sqlite`.
    pub fn home() -> Result<Login, LoginError> {
        let home = std::env::var_os("HOME").ok_or(LoginError::Failed("HOME is not set".into()))?;
        Ok(Login::at(
            PathBuf::from(home).join(".local/share/agent-harness/home/brain.sqlite"),
        ))
    }

    /// The login kept in the database at `db`.
    pub fn at(db: PathBuf) -> Login {
        Login { db }
    }

    /// Whether there are tokens: the harness is signed in, though the
    /// access token may be due for a refresh.
    pub fn signed_in(&self) -> Result<bool, LoginError> {
        Ok(self.read(&self.open()?)?.is_some())
    }

    /// The current access token, refreshed first if it is due, or why there
    /// is none.
    pub fn token(&self) -> Result<Token, LoginError> {
        let tokens = self.read(&self.open()?)?.ok_or(LoginError::SignedOut)?;
        if now_ms() < tokens.refresh_at {
            return Ok(Token(tokens.access));
        }
        match self.refresh(None) {
            // A failed early refresh leaves the token it was replacing.
            Err(LoginError::Failed(_)) if now_ms() < tokens.expires_at => Ok(Token(tokens.access)),
            result => result,
        }
    }

    /// A fresh token after `stale` was refused with 401: the row's, if
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
                if let Err(LoginError::Failed(e)) = login.token() {
                    eprintln!("harness: login: {e}");
                }
            }
        });
    }

    /// Start a sign-in: the URL Max opens. Its verifier and state wait in
    /// the row for [`Login::finish`].
    pub fn begin(&self) -> Result<String, LoginError> {
        let verifier = base64url(&random::<32>()?);
        let state = base64url(&random::<32>()?);
        let challenge =
            base64url(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()).as_ref());
        let db = self.open()?;
        db.execute_batch(TABLE).map_err(failed)?;
        db.execute(
            "UPDATE login SET verifier = ?, state = ?",
            params![verifier, state],
        )
        .map_err(failed)?;
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
        let db = self.open()?;
        let pending: Option<(String, String)> = db
            .query_row(
                "SELECT verifier, state FROM login WHERE verifier IS NOT NULL",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(failed)?;
        let Some((verifier, expected)) = pending else {
            return Err(LoginError::Failed(
                "no sign-in was started: start one, then paste its code".into(),
            ));
        };
        let Some((code, state)) = pasted.trim().split_once('#') else {
            return Err(LoginError::Failed(
                "the code is CODE#STATE, as claude.ai shows it: paste all of it".into(),
            ));
        };
        if state != expected {
            return Err(LoginError::Failed(
                "the code is from another sign-in: open the latest URL".into(),
            ));
        }
        let response = post(&json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": REDIRECT,
            "client_id": CLIENT_ID,
            "code_verifier": verifier,
            "state": state,
        }))
        .map_err(|e| LoginError::Failed(e.why))?;
        let tokens = tokens_from(&response, None)?;
        self.private()?;
        db.execute(
            "UPDATE login SET verifier = NULL, state = NULL, claimed_until = 0",
            [],
        )
        .map_err(failed)?;
        write(&db, Some(&tokens))
    }

    /// Refresh, unless the row, read again, already holds a token newer
    /// than the one that was due (or `stale`).
    fn refresh(&self, stale: Option<&Token>) -> Result<Token, LoginError> {
        let mut db = self.open()?;
        let tokens = loop {
            let tx = immediate(&mut db)?;
            let tokens = self.read(&tx)?.ok_or(LoginError::SignedOut)?;
            let moved_on = match stale {
                Some(stale) => stale.0 != tokens.access,
                None => now_ms() < tokens.refresh_at,
            };
            if moved_on {
                return Ok(Token(tokens.access));
            }
            let claimed: i64 = tx
                .query_row("SELECT claimed_until FROM login", [], |row| row.get(0))
                .map_err(failed)?;
            if claimed <= now_ms() {
                tx.execute(
                    "UPDATE login SET claimed_until = ?",
                    [now_ms() + CLAIM.as_millis() as i64],
                )
                .map_err(failed)?;
                tx.commit().map_err(failed)?;
                break tokens;
            }
            drop(tx);
            // Another process is refreshing: its token, once it has one.
            if stale.is_none() && now_ms() < tokens.expires_at {
                return Ok(Token(tokens.access));
            }
            std::thread::sleep(Duration::from_millis(500));
        };
        let response = post(&json!({
            "grant_type": "refresh_token",
            "refresh_token": tokens.refresh,
            "client_id": CLIENT_ID,
            "scope": SCOPES,
        }));
        let unclaim = |db: &Connection| {
            db.execute("UPDATE login SET claimed_until = 0", [])
                .map_err(failed)
        };
        match response {
            Ok(response) => {
                let fresh = tokens_from(&response, Some(&tokens.refresh))?;
                write(&db, Some(&fresh))?;
                unclaim(&db)?;
                Ok(Token(fresh.access))
            }
            Err(e) if e.rejected => {
                eprintln!("harness: login: {}: signed out", e.why);
                write(&db, None)?;
                unclaim(&db)?;
                Err(LoginError::SignedOut)
            }
            Err(e) => {
                unclaim(&db)?;
                Err(LoginError::Failed(e.why))
            }
        }
    }

    /// Make the database, and the journal beside it, readable by its owner
    /// alone, before it holds tokens.
    fn private(&self) -> Result<(), LoginError> {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.db.clone().into_os_string();
            path.push(suffix);
            match std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(LoginError::Failed(format!(
                        "cannot make {} private: {e}",
                        self.db.display()
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn open(&self) -> Result<Connection, LoginError> {
        let db = Connection::open(&self.db).map_err(failed)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(failed)?;
        Ok(db)
    }

    /// The tokens, if there are any. Only reads, so it is cheap to ask
    /// often: a database never signed in has no table.
    fn read(&self, db: &Connection) -> Result<Option<Tokens>, LoginError> {
        let table: bool = db
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'login')",
                [],
                |row| row.get(0),
            )
            .map_err(failed)?;
        if !table {
            return Ok(None);
        }
        db.query_row(
            "SELECT access, refresh, expires_at, refresh_at FROM login WHERE access IS NOT NULL",
            [],
            |row| {
                Ok(Tokens {
                    access: row.get(0)?,
                    refresh: row.get(1)?,
                    expires_at: row.get(2)?,
                    refresh_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(failed)
    }
}

/// Put `tokens` in the row, or clear it: signed out.
fn write(db: &Connection, tokens: Option<&Tokens>) -> Result<(), LoginError> {
    db.execute(
        "UPDATE login SET access = ?, refresh = ?, expires_at = ?, refresh_at = ?",
        params![
            tokens.map(|t| &t.access),
            tokens.map(|t| &t.refresh),
            tokens.map(|t| t.expires_at),
            tokens.map(|t| t.refresh_at),
        ],
    )
    .map_err(failed)?;
    Ok(())
}

fn immediate(db: &mut Connection) -> Result<Transaction<'_>, LoginError> {
    db.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failed)
}

fn failed(e: rusqlite::Error) -> LoginError {
    LoginError::Failed(format!("the login's database: {e}"))
}

impl Token {
    /// The `Authorization` header's value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

/// Why the token endpoint gave no tokens. `rejected` when it refused the
/// grant itself, rather than failing to answer.
struct Refused {
    why: String,
    rejected: bool,
}

/// The token endpoint's answer to `body`, as JSON.
fn post(body: &Value) -> Result<Value, Refused> {
    let fail = |why: String| Refused {
        why: format!("the claude.ai token endpoint: {why}"),
        rejected: false,
    };
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
        return Err(Refused {
            rejected: matches!(response.status, 400 | 401),
            ..fail(format!("{} {why:.200}", response.status))
        });
    }
    Ok(parsed)
}

/// The tokens in the endpoint's `response`. A refresh that returns no new
/// refresh token keeps `refresh`.
fn tokens_from(response: &Value, refresh: Option<&str>) -> Result<Tokens, LoginError> {
    let missing = |name| LoginError::Failed(format!("the claude.ai token endpoint sent no {name}"));
    let access = response["access_token"]
        .as_str()
        .ok_or(missing("access_token"))?;
    let refresh = response["refresh_token"]
        .as_str()
        .or(refresh)
        .ok_or(missing("refresh_token"))?;
    let lifetime = response["expires_in"]
        .as_i64()
        .ok_or(missing("expires_in"))?
        * 1000;
    let jitter = (u64::from_le_bytes(random::<8>()?) % JITTER.as_millis() as u64) as i64;
    let early = (EARLY.as_millis() as i64 + jitter).min(lifetime / 2);
    let now = now_ms();
    Ok(Tokens {
        access: access.to_owned(),
        refresh: refresh.to_owned(),
        expires_at: now + lifetime,
        refresh_at: now + lifetime - early,
    })
}

fn random<const N: usize>() -> Result<[u8; N], LoginError> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| LoginError::Failed("the system has no randomness to give".into()))?;
    Ok(bytes)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
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
        match self {
            LoginError::SignedOut => write!(
                f,
                "login: the harness is not signed in to claude.ai: sign in once"
            ),
            LoginError::Failed(why) => write!(f, "login: {why}"),
        }
    }
}

impl std::error::Error for LoginError {}
