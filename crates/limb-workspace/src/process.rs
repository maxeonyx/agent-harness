//! The limb as a process of its own, as the brain starts every plugin limb:
//! `harness-limb-<kind> <state dir>`, with the limb's definition as the
//! first line on stdin and then one [`ToLimb`] per line. It answers one
//! [`FromLimb`](harness_core::environment::FromLimb) per line on stdout,
//! `Hello` first.
//!
//! It exits only once nothing it started is left: told to shut down
//! ([`ToLimb::Shutdown`]), its input closing (the brain that started it
//! has gone), or a signal, it stops every running call (`SIGTERM` to each
//! command's processes, `SIGKILL` at the deadline), waits for each to
//! settle, ends any orphan it adopted as subreaper, and only then exits. A
//! second request makes the stop harder. The brain bounds the whole by
//! killing the limb's tree, which the next instance finds by its record.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use harness_core::environment::{Limb, LimbKind, ToLimb};
use harness_runtime::{Cancel, signals};

use crate::{Config, Kind, SHUTDOWN, Settings, Workspace};

/// Run as the limb of `kind` until it has shut down.
pub fn main(kind: &str) -> ExitCode {
    match serve(kind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("harness-limb-{kind}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// What reaches the limb's loop.
enum Event {
    Message(ToLimb),
    /// Stdin closed: the brain has gone.
    Closed,
    Signal(libc::c_int),
    /// Every call has stopped and nothing is left.
    Down,
}

fn serve(kind: &str) -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(state), None) = (args.next(), args.next()) else {
        return Err("usage: <state dir>, with the limb's definition on stdin".into());
    };
    // Orphans of its commands are adopted by it, so they stay its to end.
    harness_runtime::subreaper().map_err(|e| format!("subreaper: {e}"))?;
    let mut first = String::new();
    io::stdin()
        .read_line(&mut first)
        .map_err(|e| e.to_string())?;
    let limb: Limb = serde_json::from_str(&first).map_err(|e| format!("the definition: {e}"))?;
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
    let workspace = Arc::new(Workspace::open(config, out).map_err(|e| e.to_string())?);
    let writer = thread::spawn(move || {
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

    let (events, inbox) = mpsc::channel();
    let signalled = events.clone();
    signals::forward(&[libc::SIGINT, libc::SIGTERM, libc::SIGHUP], move |s| {
        let _ = signalled.send(Event::Signal(s));
    })
    .map_err(|e| format!("signals: {e}"))?;
    let read = events.clone();
    // Detached: it blocks on stdin, and holds nothing.
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            match serde_json::from_str(&line) {
                Ok(message) => {
                    let _ = read.send(Event::Message(message));
                }
                Err(e) => eprintln!("from the brain: {e}: {line}"),
            }
        }
        let _ = read.send(Event::Closed);
    });

    // The shutdown's own stop: requested once it begins.
    let cancel = Cancel::new();
    let down = |within: Duration| {
        workspace.stop_all(within);
        if !cancel.is_requested() {
            let workspace = workspace.clone();
            let events = events.clone();
            let cancel = cancel.clone();
            thread::spawn(move || {
                workspace.join();
                end_adopted(&cancel);
                let _ = events.send(Event::Down);
            });
        }
        cancel.request(Instant::now() + within);
    };
    for event in &inbox {
        match event {
            Event::Message(ToLimb::Shutdown { within }) => down(Duration::from_millis(within)),
            Event::Message(message) => workspace.handle(message),
            Event::Closed => down(SHUTDOWN),
            Event::Signal(signal) => {
                eprintln!(
                    "harness-limb-{kind}: {}: shutting down",
                    signals::name(signal)
                );
                down(if cancel.is_requested() {
                    Duration::ZERO
                } else {
                    SHUTDOWN
                });
            }
            Event::Down => break,
        }
    }
    // Every sender of what it says goes with the workspace, so the writer
    // finishes once the last settlement is out.
    drop(workspace);
    let _ = writer.join();
    Ok(())
}

/// End every process the limb adopted as subreaper and still has: what its
/// commands left that no mark accounts for. It has no calls by now, so
/// every child is such an orphan.
fn end_adopted(cancel: &Cancel) {
    let Ok(mut waiter) = cancel.waiter() else {
        return;
    };
    let deadline = cancel.deadline().unwrap_or_else(Instant::now);
    let members = || {
        harness_runtime::reap_children();
        harness_runtime::children()
    };
    match harness_runtime::end(&mut waiter, deadline, Some(libc::SIGTERM), members, || None) {
        Ok(harness_runtime::Ended::Killed { survivors }) if !survivors.is_empty() => {
            eprintln!("processes {survivors:?} survived SIGKILL");
        }
        Ok(_) => {}
        Err(e) => eprintln!("ending what was left: {e}"),
    }
    harness_runtime::reap_children();
}
