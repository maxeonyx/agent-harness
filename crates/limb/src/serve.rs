//! `serve <limb>`: the limb itself.
//!
//! It takes its state's lock (another instance holding it is an error, so
//! two instances never write one state), leaves the process group and
//! session of whoever started it, and becomes the subreaper of what it
//! starts, so a command's orphans stay its to end. Then it listens on its
//! socket. Each connection is told [`Hello`] at once; the newest one to
//! ship a whole view becomes the link, closing the one before, so a link
//! the network dropped silently is replaced the moment the brain dials
//! again. The kind opens on the first view's [`LimbRow::Definition`].
//!
//! A brain that would start another build in its place
//! ([`LimbRow::Superseded`]) has it exit once nothing it holds would die
//! with it, so the brain's next attach starts that build.
//!
//! A brain going away is a link closing: it stops nothing. The limb exits
//! on a signal, or once no brain has been linked for [`ABSENT`], and only
//! once nothing it started is left: it stops every running call (`SIGTERM`,
//! then `SIGKILL` at the deadline), waits for each to settle, and ends any
//! orphan it adopted. A second signal makes the stop harder.
//!
//! A brain that holds more of the limb's records than its state does
//! ([`harness_substrate::Restored`]) means the state was lost or restored
//! from a backup. The limb then starts again as a new replica, closing
//! every link, so the brain dials again and hears the new one. The brain
//! says which instance each call was first sent to, so none runs twice.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::build::BuildId;
use harness_core::environment::{Hello, LimbDown, LimbKey, LimbRow, LimbUp};
use harness_core::id::{LimbId, ReplicaId};
use harness_core::live::Live;
use harness_core::schema::Harness;
use harness_runtime::{Cancel, signals};
use harness_store::Store;
use harness_substrate::{Log, Up};
use serde_json::Value;

use crate::{ABSENT, Instance, Kind, SHUTDOWN, State};

/// One connection, numbered by the limb.
type Conn = u64;

enum Event {
    Accepted(Conn, UnixStream),
    Down(Conn, Box<LimbDown>),
    Ended(Conn),
    Up(Box<LimbUp>),
    Signal(libc::c_int),
    /// Every call has stopped and nothing is left.
    Stopped,
}

pub fn run<K: Kind>(kind: &str, limb: LimbId) -> Result<ExitCode, String> {
    let state = State::enter(limb).map_err(|e| e.to_string())?;
    let Some(_lock) = state.lock().map_err(|e| format!("its lock: {e}"))? else {
        return Err(format!(
            "{} is held by another instance",
            state.dir().display()
        ));
    };
    // SAFETY: setsid takes no arguments. It fails only for a process that
    // already leads a group, which is then on its own anyway.
    unsafe { libc::setsid() };
    harness_runtime::subreaper().map_err(|e| format!("subreaper: {e}"))?;
    let socket = state.socket();
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)
        .map_err(|e| format!("{}: {e}", state.dir().join(socket).display()))?;
    let build = std::env::current_exe()
        .and_then(std::fs::read)
        .ok()
        .map(|bytes| BuildId::of(&bytes));

    let (events, inbox) = mpsc::channel();
    let accepted = events.clone();
    thread::spawn(move || {
        for (conn, stream) in (1..).zip(listener.incoming()) {
            let Ok(stream) = stream else { continue };
            if accepted.send(Event::Accepted(conn, stream)).is_err() {
                return;
            }
        }
    });
    let signalled = events.clone();
    // A server a command started with `setsid` and that has since been
    // stopped is the limb's to reap, at once, so a `kill -0` sees it gone.
    signals::forward(
        &[libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGCHLD],
        move |s| match s {
            libc::SIGCHLD => harness_runtime::reap_detached(),
            _ => {
                let _ = signalled.send(Event::Signal(s));
            }
        },
    )
    .map_err(|e| format!("signals: {e}"))?;

    let mut limb_ = Served::<K> {
        instance: None,
        log: Some(open_log(&state)?),
        state,
        events: events.clone(),
        cancel: Cancel::new(),
    };
    let mut replica = limb_.replica();
    // Every connection open, by number; `link` is the one that is the link.
    let mut conns: HashMap<Conn, UnixStream> = HashMap::new();
    let mut link: Option<Conn> = None;
    let mut alone_since = Instant::now();
    // The build the brain would start instead of this one, once it says so.
    let mut superseded: Option<BuildId> = None;
    loop {
        if let Some(by) = superseded
            && !limb_.stopping()
            && limb_.instance.as_ref().is_some_and(|i| !i.kind().holds())
        {
            eprintln!(
                "harness-limb-{kind}: superseded by build {:.12}: shutting down",
                by.to_string()
            );
            limb_.down(SHUTDOWN);
        }
        let event = match inbox.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => {
                if link.is_none() && alone_since.elapsed() >= ABSENT && !limb_.stopping() {
                    eprintln!(
                        "harness-limb-{kind}: no brain for {}s: shutting down",
                        ABSENT.as_secs()
                    );
                    limb_.down(SHUTDOWN);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => unreachable!("the loop holds a sender"),
        };
        match event {
            Event::Accepted(conn, mut stream) => {
                let hello = Hello {
                    limb,
                    replica,
                    build,
                };
                if write_line(&mut stream, &hello).is_ok()
                    && let Ok(reader) = stream.try_clone()
                {
                    read(conn, reader, events.clone());
                    conns.insert(conn, stream);
                }
            }
            Event::Down(conn, down) => {
                if link != Some(conn) {
                    // A connection becomes the link with a whole view.
                    if !matches!(*down, LimbDown::View(_)) {
                        continue;
                    }
                    if let Some(old) = link.and_then(|l| conns.remove(&l)) {
                        let _ = old.shutdown(Shutdown::Both);
                    }
                    link = Some(conn);
                }
                superseded = superseded_by(&down, superseded);
                match limb_.apply(*down) {
                    Ok(unsent) => {
                        for record in unsent {
                            ship(&mut conns, link, &Up::<Harness, Live>::Record(record));
                        }
                    }
                    Err(Fresh) => {
                        replica = limb_.replica();
                        for (_, stream) in conns.drain() {
                            let _ = stream.shutdown(Shutdown::Both);
                        }
                    }
                }
            }
            Event::Ended(conn) => {
                conns.remove(&conn);
                if link == Some(conn) {
                    link = None;
                    alone_since = Instant::now();
                }
            }
            Event::Up(up) => ship(&mut conns, link, &*up),
            Event::Signal(signal) => {
                eprintln!(
                    "harness-limb-{kind}: {}: shutting down",
                    signals::name(signal)
                );
                limb_.down(if limb_.stopping() {
                    Duration::ZERO
                } else {
                    SHUTDOWN
                });
            }
            Event::Stopped => return Ok(ExitCode::SUCCESS),
        }
    }
}

/// The limb's state as the loop holds it: its records, and the instance
/// once a brain has defined it.
struct Served<K> {
    instance: Option<Arc<Instance<K>>>,
    /// Its records, until the instance opens on them.
    log: Option<Log<Harness, Store>>,
    state: State,
    events: Sender<Event>,
    /// The shutdown's own stop: requested once it begins.
    cancel: Cancel,
}

/// The state was lost: the limb is a new replica now.
struct Fresh;

impl<K: Kind> Served<K> {
    fn replica(&self) -> ReplicaId {
        match &self.log {
            Some(log) => log.here(),
            None => unreachable!("the replica is read before the kind opens"),
        }
    }

    fn stopping(&self) -> bool {
        self.cancel.is_requested()
    }

    /// Hand a down to the kind, opening it on the first view's definition.
    fn apply(&mut self, down: LimbDown) -> Result<Vec<harness_core::schema::Record>, Fresh> {
        if self.instance.is_none() {
            let LimbDown::View(view) = &down else {
                return Ok(Vec::new());
            };
            let Some(LimbRow::Definition(definition)) = view.rows.get(&LimbKey::Definition) else {
                eprintln!("the brain's view holds no definition of this limb");
                return Ok(Vec::new());
            };
            let events = self.events.clone();
            let out = Box::new(move |up| {
                let _ = events.send(Event::Up(Box::new(up)));
            });
            let log = self.log.take().expect("records until the instance opens");
            match Instance::open(definition, log, self.state.dir(), out) {
                Ok(instance) => self.instance = Some(Arc::new(instance)),
                Err(why) => {
                    // Nothing has started: the limb cannot, and says why.
                    eprintln!("{why}");
                    std::process::exit(1);
                }
            }
        }
        let instance = self.instance.as_ref().expect("opened");
        match instance.apply(down) {
            Ok(unsent) => Ok(unsent),
            Err(restored) => {
                eprintln!(
                    "the brain holds this limb's records up to {}, past its state's {}: \
                     its state was lost or restored, so it starts again as a new replica",
                    restored.upto, restored.next
                );
                instance.kind().stop_all(SHUTDOWN);
                drop(self.instance.take());
                for suffix in ["", "-wal", "-shm"] {
                    let mut path = self.state.records().into_os_string();
                    path.push(suffix);
                    let _ = std::fs::remove_file(path);
                }
                match open_log(&self.state) {
                    Ok(log) => self.log = Some(log),
                    Err(why) => {
                        eprintln!("{why}");
                        std::process::exit(1);
                    }
                }
                Err(Fresh)
            }
        }
    }

    /// Stop every call, each to be gone `within` from now, and once nothing
    /// is left, end what it adopted and say so.
    fn down(&mut self, within: Duration) {
        let first = !self.cancel.is_requested();
        self.cancel.request(Instant::now() + within);
        let Some(instance) = self.instance.clone() else {
            let _ = self.events.send(Event::Stopped);
            return;
        };
        instance.kind().stop_all(within);
        if first {
            let events = self.events.clone();
            let cancel = self.cancel.clone();
            thread::spawn(move || {
                instance.kind().join();
                end_adopted(&cancel);
                let _ = events.send(Event::Stopped);
            });
        }
    }
}

fn open_log(state: &State) -> Result<Log<Harness, Store>, String> {
    let records = state.records();
    let store = Store::open(&records).map_err(|e| format!("{}: {e}", records.display()))?;
    Log::open(store.replica(), store).map_err(|e| format!("{}: {e}", records.display()))
}

/// Read what a connection ships until it closes. Detached.
fn read(conn: Conn, stream: UnixStream, events: Sender<Event>) {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            match parse(&line) {
                Ok(down) => {
                    let _ = events.send(Event::Down(conn, Box::new(down)));
                }
                Err(e) => eprintln!("from the brain: {e}: {line}"),
            }
        }
        let _ = events.send(Event::Ended(conn));
    });
}

/// Whether the brain would start another build in this limb's place, as of
/// `down`, if it was `before`.
fn superseded_by(down: &LimbDown, before: Option<BuildId>) -> Option<BuildId> {
    let by = |row: &LimbRow| match row {
        LimbRow::Superseded { by } => Some(*by),
        _ => None,
    };
    match down {
        LimbDown::View(view) => view.rows.get(&LimbKey::Superseded).and_then(by),
        LimbDown::Step(step) => match step.set.iter().find(|(k, _)| *k == LimbKey::Superseded) {
            Some((_, row)) => by(row),
            None if step.gone.contains(&LimbKey::Superseded) => None,
            None => before,
        },
        LimbDown::Live(_) => before,
    }
}

/// A line from the brain, without the rows this build cannot read: a brain
/// of a later build may ship kinds of rows it has no idea of, and the rest
/// of the line still holds.
fn parse(line: &str) -> Result<LimbDown, serde_json::Error> {
    serde_json::from_str(line).or_else(|whole| {
        let mut value: Value = serde_json::from_str(line)?;
        let mut skipped = Vec::new();
        let mut known = |key: &Value, row: Option<&Value>| {
            let ok = serde_json::from_value::<LimbKey>(key.clone()).is_ok()
                && row.is_none_or(|r| serde_json::from_value::<LimbRow>(r.clone()).is_ok());
            if !ok {
                skipped.push(key.to_string());
            }
            ok
        };
        if let Some(rows) = value
            .pointer_mut("/View/rows")
            .and_then(Value::as_object_mut)
        {
            rows.retain(|k, r| known(&Value::String(k.clone()), Some(r)));
        }
        if let Some(set) = value.pointer_mut("/Step/set").and_then(Value::as_array_mut) {
            set.retain(|pair| known(&pair[0], Some(&pair[1])));
        }
        if let Some(gone) = value
            .pointer_mut("/Step/gone")
            .and_then(Value::as_array_mut)
        {
            gone.retain(|k| known(k, None));
        }
        if skipped.is_empty() {
            return Err(whole);
        }
        eprintln!(
            "from the brain: skipping rows this build cannot read: {}",
            skipped.join(", ")
        );
        serde_json::from_value(value)
    })
}

/// Write one value to the link, if there is one. A write that fails drops
/// it: the brain hears the rest when it links again.
fn ship(conns: &mut HashMap<Conn, UnixStream>, link: Option<Conn>, value: &impl serde::Serialize) {
    let Some(stream) = link.and_then(|l| conns.get_mut(&l)) else {
        return;
    };
    if write_line(stream, value).is_err() {
        let _ = stream.shutdown(Shutdown::Both);
    }
}

fn write_line(stream: &mut UnixStream, value: &impl serde::Serialize) -> io::Result<()> {
    let mut line = serde_json::to_vec(value).expect("a link value serialises");
    line.push(b'\n');
    stream.write_all(&line)
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
