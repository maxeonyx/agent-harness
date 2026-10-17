//! The limb as a process of its own, as the brain starts every plugin limb:
//! `harness-limb-<kind> <state dir> <socket> <definition>`, the definition
//! as JSON. It connects to the brain's Unix socket, says [`Hello`], then
//! ships [`LimbUp`]s and takes in [`LimbDown`]s, one JSON value per line.
//!
//! The link can drop and come back: a brain restarting, or a new brain on
//! the same socket. The limb keeps running its calls meanwhile, keeps
//! trying to reconnect, and on reconnecting ships what the brain's standing
//! does not cover. Only a link gone for [`ABSENT`] means the brain is not
//! coming back.
//!
//! A limb that cannot start (its directory cannot be made, its clone
//! fails) exits saying why in the file [`FAILED`] in its state directory,
//! which the brain records as why it never connected.
//!
//! It exits only once nothing it started is left: told to shut down
//! ([`LimbRow::Shutdown`]), the link gone for good, or a signal, it stops
//! every running call (`SIGTERM` to each command's processes, `SIGKILL` at
//! the deadline), waits for each to settle, ends any orphan it adopted as
//! subreaper, and only then exits. A second request makes the stop harder.
//! The brain bounds the whole by killing the limb's tree, which the next
//! instance finds by its marks.
//!
//! [`LimbRow::Shutdown`]: harness_core::environment::LimbRow::Shutdown

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::environment::{Hello, Limb, LimbDown, LimbKind, LimbUp};
use harness_runtime::{Cancel, signals};
use harness_substrate::Up;
use serde::Serialize;

use crate::{Config, Kind, SHUTDOWN, Settings, Workspace, instant};

/// How long the link may stay down before the limb takes the brain as gone:
/// long enough for a crashed brain to be restarted by hand, so a restart
/// never costs a limb its running calls. A brain that shuts down cleanly
/// tells its limbs to go, so only a brain that died leaves them waiting.
pub const ABSENT: Duration = Duration::from_secs(15 * 60);

/// How often it tries to reconnect.
const RETRY: Duration = Duration::from_millis(200);

/// The file in a limb's state directory saying why its process ended
/// before it connected.
pub const FAILED: &str = "failed";

/// Run as the limb of `kind` until it has shut down.
pub fn main(kind: &str) -> ExitCode {
    match serve(kind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("harness-limb-{kind}: {e}");
            if let Some(state) = std::env::args().nth(1).map(PathBuf::from) {
                let _ = std::fs::create_dir_all(&state)
                    .and_then(|()| std::fs::write(state.join(FAILED), &e));
            }
            ExitCode::FAILURE
        }
    }
}

/// What reaches the limb's loop.
enum Event {
    /// Connected: what it writes goes here.
    Linked(UnixStream),
    Down(Box<LimbDown>),
    /// The link dropped; the connector is trying again.
    Unlinked,
    /// The link stayed down for [`ABSENT`].
    Gone,
    Up(Box<LimbUp>),
    Signal(libc::c_int),
    /// Every call has stopped and nothing is left.
    Stopped,
}

fn serve(kind: &str) -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(state), Some(socket), Some(definition), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: <state dir> <socket> <definition>".into());
    };
    // Orphans of its commands are adopted by it, so they stay its to end.
    harness_runtime::subreaper().map_err(|e| format!("subreaper: {e}"))?;
    let limb: Limb =
        serde_json::from_str(&definition).map_err(|e| format!("the definition: {e}"))?;
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
    let (events, inbox) = mpsc::channel();
    let shipped = events.clone();
    let out = Box::new(move |up| {
        let _ = shipped.send(Event::Up(Box::new(up)));
    });
    let workspace = Arc::new(Workspace::open(config, out).map_err(|e| e.to_string())?);
    let hello = Hello {
        limb: workspace.limb(),
        replica: workspace.replica(),
    };
    connect(PathBuf::from(socket), hello, events.clone());

    let signalled = events.clone();
    signals::forward(&[libc::SIGINT, libc::SIGTERM, libc::SIGHUP], move |s| {
        let _ = signalled.send(Event::Signal(s));
    })
    .map_err(|e| format!("signals: {e}"))?;

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
                let _ = events.send(Event::Stopped);
            });
        }
        cancel.request(Instant::now() + within);
    };
    let mut link: Option<UnixStream> = None;
    for event in &inbox {
        match event {
            Event::Linked(stream) => link = Some(stream),
            Event::Unlinked => link = None,
            Event::Gone => down(SHUTDOWN),
            Event::Up(up) => ship(&mut link, &*up),
            Event::Down(down_) => match workspace.apply(*down_) {
                Ok(applied) => {
                    for record in applied.ship {
                        ship(
                            &mut link,
                            &Up::<_, harness_core::live::Live>::Record(record),
                        );
                    }
                    if let Some(by) = applied.shutdown {
                        down(instant(by).saturating_duration_since(Instant::now()));
                    }
                }
                Err(restored) => {
                    return Err(format!(
                        "the brain holds this limb's records up to {}, past its log's {}: \
                         its state was restored or lost, so it must start as a new replica",
                        restored.upto, restored.next
                    ));
                }
            },
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
            Event::Stopped => break,
        }
    }
    Ok(())
}

/// Write one value to the link, if there is one. A write that fails drops
/// it; the reader sees the link close and the connector reconnects.
fn ship(link: &mut Option<UnixStream>, value: &impl Serialize) {
    let Some(stream) = link else { return };
    let mut line = serde_json::to_vec(value).expect("a link value serialises");
    line.push(b'\n');
    if stream.write_all(&line).is_err() {
        *link = None;
    }
}

/// Keep the limb connected to the brain's socket: connect, say hello, read
/// what the brain ships until the link drops, and again, until it has been
/// down for [`ABSENT`]. Detached: it holds nothing.
fn connect(socket: PathBuf, hello: Hello, events: Sender<Event>) {
    thread::spawn(move || {
        let mut down_since = Instant::now();
        loop {
            match UnixStream::connect(&socket) {
                Ok(stream) => {
                    if let Err(e) = linked(stream, &hello, &events) {
                        eprintln!("the link to the brain: {e}");
                    }
                    let _ = events.send(Event::Unlinked);
                    down_since = Instant::now();
                }
                Err(_) if down_since.elapsed() >= ABSENT => {
                    let _ = events.send(Event::Gone);
                    return;
                }
                Err(_) => {}
            }
            thread::sleep(RETRY);
        }
    });
}

fn linked(mut stream: UnixStream, hello: &Hello, events: &Sender<Event>) -> io::Result<()> {
    let mut line = serde_json::to_vec(hello)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    let _ = events.send(Event::Linked(stream.try_clone()?));
    for line in BufReader::new(stream).lines() {
        let line = line?;
        match serde_json::from_str(&line) {
            Ok(down) => {
                let _ = events.send(Event::Down(Box::new(down)));
            }
            Err(e) => eprintln!("from the brain: {e}: {line}"),
        }
    }
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
