//! One call to the Anthropic API on Max's claude.ai login, for an agent
//! probing it: list the models, try a parameter.
//!
//! ```text
//! harness-api METHOD PATH [BODY_FILE]
//! harness-api GET /v1/models
//! harness-api POST /v1/messages body.json
//! harness-api login            # prints the URL to sign in at
//! harness-api login CODE         # the code claude.ai shows after it
//! harness-api --login DB ...    # a brain's login other than the home brain's
//! ```
//!
//! The status and the response's headers go to stderr, the body to
//! stdout as it arrives. A messages request needs the identity string as
//! `system[0]` (else a fake 429), and its
//! `anthropic-ratelimit-unified-representative-claim` header says whether
//! it was billed to the plan. Every messages call spends Max's usage.

use std::io::{Read, Write};
use std::process::ExitCode;

use harness_provider::{Client, Login};
use harness_runtime::Cancel;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("harness-api: {e}");
            ExitCode::from(2)
        }
    }
}

/// Whether the provider answered 2xx.
fn run() -> Result<bool, String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let login = match args.first().map(String::as_str) {
        Some("--login") if args.len() > 1 => Login::at(args.drain(..2).nth(1).unwrap().into()),
        _ => Login::home().map_err(|e| e.to_string())?,
    };
    let (method, path, file) = match args.as_slice() {
        [l] if l == "login" => {
            println!("{}", login.begin().map_err(|e| e.to_string())?);
            eprintln!("Open the URL, sign in, then: harness-api login CODE");
            return Ok(true);
        }
        [l, code] if l == "login" => {
            login.finish(code).map_err(|e| e.to_string())?;
            eprintln!("Signed in.");
            return Ok(true);
        }
        [method, path] => (method, path, None),
        [method, path, file] => (method, path, Some(file)),
        _ => {
            return Err(
                "usage: harness-api [--login DB] METHOD PATH [BODY_FILE] | login [CODE]".into(),
            );
        }
    };
    let body = match file {
        Some(file) => std::fs::read(file).map_err(|e| format!("{file}: {e}"))?,
        None => Vec::new(),
    };
    let mut response = Client::new(login)
        .map_err(|e| e.0)?
        .exchange(method, path, &body, &Cancel::new())
        .map_err(|e| e.to_string())?;
    eprintln!("{}", response.status);
    for (name, value) in &response.headers {
        eprintln!("{name}: {value}");
    }
    let mut out = std::io::stdout();
    let mut buf = [0; 8192];
    loop {
        let n = response.body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    Ok((200..300).contains(&response.status))
}
