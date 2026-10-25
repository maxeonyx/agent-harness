//! `attach <limb>`: how a brain reaches a limb on this machine, starting it
//! if it is not running. A limb on another machine, through `ssh`, is not
//! built yet: the brain refuses one.
//!
//! It connects to the limb's socket. If nothing is listening, it starts
//! `serve` and waits for it to listen. Then it carries the link: its stdin
//! to the socket and the socket to its stdout, until either closes. A
//! brain closing its end ends only the link; the limb runs on.
//!
//! It fails, printing why on stderr, if the limb cannot start or ends while
//! linked: what the limb printed last, from its log. A link the limb closed
//! while it runs on (a newer link replaced it) ends quietly.
//!
//! Where there is a systemd user manager, the limb runs in a scope of its
//! own (`harness-limb-<limb>.scope`), so it is no part of the brain's
//! service: restarting the service leaves it running, and
//! `systemctl --user stop` stops it like any unit.

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::id::LimbId;

use crate::State;

/// How long a starting limb gets to listen.
const START: Duration = Duration::from_secs(30);
/// How often a starting limb's socket is tried.
const POLL: Duration = Duration::from_millis(50);
/// How long a limb that closed the link gets to release its lock, if it is
/// ending.
const ENDING: Duration = Duration::from_secs(2);

pub fn run(kind: &str, limb: LimbId) -> Result<ExitCode, String> {
    let state = State::enter(limb).map_err(|e| e.to_string())?;
    let from = std::fs::metadata(state.log()).map(|m| m.len()).unwrap_or(0);
    let stream = match UnixStream::connect(state.socket()) {
        Ok(stream) => stream,
        Err(_) => start(kind, limb, &state, from)?,
    };
    relay(stream).map_err(|e| format!("the link: {e}"))?;
    // The limb closed the link. If it is ending, say why.
    let until = Instant::now() + ENDING;
    while Instant::now() < until {
        if !state.running() {
            return Err(state.last_words(from));
        }
        thread::sleep(POLL);
    }
    Ok(ExitCode::SUCCESS)
}

/// Start the limb and connect once it listens.
fn start(kind: &str, limb: LimbId, state: &State, from: u64) -> Result<UnixStream, String> {
    let exe = std::env::current_exe().map_err(|e| format!("its own executable: {e}"))?;
    let log = std::fs::File::options()
        .create(true)
        .append(true)
        .open(state.log())
        .map_err(|e| format!("{}: {e}", state.log().display()))?;
    let mut command = if user_manager() {
        let mut c = Command::new("systemd-run");
        c.args(["--user", "--scope", "--quiet", "--collect"])
            .arg(format!("--unit=harness-limb-{limb}"))
            .arg("--")
            .arg(&exe);
        c
    } else {
        Command::new(&exe)
    };
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    let mut child = command
        .arg("serve")
        .arg(limb.to_string())
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log2)
        .spawn()
        .map_err(|e| format!("harness-limb-{kind} serve: {e}"))?;
    let until = Instant::now() + START;
    loop {
        if let Ok(stream) = UnixStream::connect(state.socket()) {
            return Ok(stream);
        }
        // Ended, and no other instance holds the state: it could not start.
        if matches!(child.try_wait(), Ok(Some(_))) && !state.running() {
            return Err(state.last_words(from));
        }
        if Instant::now() >= until {
            return Err(format!(
                "it did not listen within {}s; see {}",
                START.as_secs(),
                state.log().display()
            ));
        }
        thread::sleep(POLL);
    }
}

/// Whether a systemd user manager runs this user's units.
fn user_manager() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .is_some_and(|dir| Path::new(&dir).join("systemd/private").exists())
}

/// Carry the link until either side closes it.
fn relay(stream: UnixStream) -> io::Result<()> {
    let mut up = stream.try_clone()?;
    let closer = stream.try_clone()?;
    // The brain's end closing ends the link, whichever side the limb is on.
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut up);
        let _ = closer.shutdown(Shutdown::Both);
        std::process::exit(0);
    });
    let mut down = stream;
    let mut out = io::stdout().lock();
    let mut buf = [0; 64 * 1024];
    loop {
        let n = down.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        out.write_all(&buf[..n])?;
        out.flush()?;
    }
}
