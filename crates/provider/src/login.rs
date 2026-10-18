//! The claude.ai login: one owner of the subscription credential, for every
//! Anthropic endpoint the harness uses (the Messages API, dictation).

use std::fmt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Where the claude.ai login lives. Claude Code owns it and refreshes it;
/// the harness only reads it.
///
/// The file is read again for every [`Login::token`], so a token Claude
/// Code has refreshed is picked up at the next request. The refresh token is
/// never read: using it rotates the login and signs Claude Code out.
#[derive(Clone, Debug)]
pub struct Login {
    path: PathBuf,
}

/// An access token, valid when it was read. It has no `Debug` or `Display`,
/// so it cannot be printed by accident.
pub struct Token(String);

/// Why there is no usable token. It never contains the token.
#[derive(Debug)]
pub struct LoginError(pub String);

impl Login {
    /// Claude Code's login, `~/.claude/.credentials.json`.
    pub fn claude_code() -> Result<Login, LoginError> {
        let home = std::env::var_os("HOME").ok_or(LoginError("HOME is not set".into()))?;
        Ok(Login {
            path: PathBuf::from(home).join(".claude/.credentials.json"),
        })
    }

    /// The current access token, or why there is none.
    pub fn token(&self) -> Result<Token, LoginError> {
        let path = self.path.display();
        let text = std::fs::read_to_string(&self.path)
            .map_err(|e| LoginError(format!("cannot read {path}: {e}")))?;
        let file: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| LoginError(format!("{path} is not JSON")))?;
        let oauth = &file["claudeAiOauth"];
        let (Some(token), Some(expires)) =
            (oauth["accessToken"].as_str(), oauth["expiresAt"].as_u64())
        else {
            return Err(LoginError(format!(
                "{path} has no claudeAiOauth.accessToken and expiresAt"
            )));
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        if expires <= now {
            return Err(LoginError(
                "the claude.ai access token has expired: use Claude Code once so it refreshes it"
                    .into(),
            ));
        }
        Ok(Token(token.to_owned()))
    }
}

impl Token {
    /// The `Authorization` header's value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "login: {}", self.0)
    }
}

impl std::error::Error for LoginError {}
