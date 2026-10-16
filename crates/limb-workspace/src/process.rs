//! The limb as a process of its own, as the brain starts every plugin limb:
//! `harness-limb-<kind> <state dir>`, with the limb's definition as the
//! first line on stdin and then one [`ToLimb`] per line. It answers one
//! [`FromLimb`](harness_core::environment::FromLimb) per line on stdout, `Hello` first, and exits
//! when stdin closes: the brain that started it has gone, and the next
//! brain starts another instance, which answers from the record of calls.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use harness_core::environment::{Limb, LimbKind, ToLimb};

use crate::{Config, Kind, Settings, Workspace};

/// Run as the limb of `kind` until stdin closes.
pub fn main(kind: &str) -> ExitCode {
    match serve(kind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("harness-limb-{kind}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn serve(kind: &str) -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(state), None) = (args.next(), args.next()) else {
        return Err("usage: <state dir>, with the limb's definition on stdin".into());
    };
    let mut lines = io::stdin().lock().lines();
    let first = lines.next().ok_or("no definition on stdin")?;
    let limb: Limb = serde_json::from_str(&first.map_err(|e| e.to_string())?)
        .map_err(|e| format!("the definition: {e}"))?;
    let of_kind = match &limb.kind {
        LimbKind::Plugin { kind: k, .. } if k == kind => Kind::of(k),
        _ => None,
    };
    let Some(of_kind) = of_kind else {
        return Err(format!("the definition is not of a `{kind}` limb"));
    };
    let settings = Settings::of(&limb).ok_or("the definition's settings do not parse")?;
    let config = Config {
        limb: limb.id,
        kind: of_kind,
        settings,
        state: PathBuf::from(state),
    };
    let (out, said) = mpsc::channel();
    let workspace = Workspace::open(config, out).map_err(|e| e.to_string())?;
    thread::scope(|s| {
        s.spawn(move || {
            let mut stdout = io::stdout().lock();
            for message in said {
                let line = serde_json::to_string(&message).expect("a message serialises");
                if writeln!(stdout, "{line}")
                    .and_then(|()| stdout.flush())
                    .is_err()
                {
                    return;
                }
            }
        });
        for line in lines {
            let line = line.map_err(|e| e.to_string())?;
            let message: ToLimb =
                serde_json::from_str(&line).map_err(|e| format!("from the brain: {e}"))?;
            workspace.handle(message);
        }
        // The brain has gone. Running commands are left to the next
        // instance, which finds them in the record and kills them.
        std::process::exit(0)
    })
}
