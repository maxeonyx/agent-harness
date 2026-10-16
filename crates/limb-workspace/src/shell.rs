//! Running one command as a process tree the call owns.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::value::Bytes;

use crate::trunc::{Truncated, Truncator};

/// A request to stop a running call.
#[derive(Default)]
pub struct Stop(AtomicBool);

impl Stop {
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// How a command ended.
pub enum Ended {
    Exited(ExitStatus),
    Stopped,
    TimedOut,
}

pub struct Ran {
    pub ended: Ended,
    pub output: Truncated,
    /// The whole output, when `output` left some of it out.
    pub full: Option<Vec<u8>>,
}

/// How long a process tree gets to exit after `SIGTERM` before `SIGKILL`.
const GRACE: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(10);

/// A command running as a process tree the call owns.
pub struct Process {
    child: Child,
    group: libc::pid_t,
    capture: Arc<Mutex<Option<Capture>>>,
    finished: mpsc::Receiver<()>,
}

impl Process {
    /// Start `command` with `bash -c` in `dir`, its stdout and stderr
    /// merged as a terminal shows them.
    ///
    /// The command leads a process group of its own, and the group is the
    /// call's: when the command exits, is stopped or times out, everything
    /// still in the group is sent `SIGTERM`, then `SIGKILL`. A process that
    /// leaves the group (`setsid`) is not the call's any more and is not
    /// followed.
    pub fn start(
        command: &str,
        dir: &Path,
        truncator: Truncator,
        progress: impl Fn(&[u8]) + Send + 'static,
    ) -> io::Result<Process> {
        let (mut reader, writer) = io::pipe()?;
        let mut builder = Command::new("bash");
        builder
            .arg("-c")
            .arg(command)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(writer.try_clone()?)
            .stderr(writer)
            .process_group(0);
        let child = builder.spawn()?;
        // The builder holds the pipe's write end; the output ends only once
        // every holder has closed it.
        drop(builder);
        let group = child.id() as libc::pid_t;

        let capture = Arc::new(Mutex::new(Some(Capture {
            truncator,
            full: Full::default(),
        })));
        let (done, finished) = mpsc::channel();
        let shared = capture.clone();
        thread::spawn(move || {
            let mut buffer = [0; 8192];
            loop {
                let n = match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                let mut capture = shared.lock().unwrap_or_else(|e| e.into_inner());
                let Some(capture) = capture.as_mut() else {
                    break;
                };
                capture.feed(&buffer[..n]);
                progress(&buffer[..n]);
            }
            let _ = done.send(());
        });
        Ok(Process {
            child,
            group,
            capture,
            finished,
        })
    }

    /// The process group, which is the leader's pid.
    pub fn group(&self) -> libc::pid_t {
        self.group
    }

    /// Wait for the command to exit, to be stopped, or to reach `timeout`;
    /// then end whatever is left of its group.
    pub fn wait(mut self, timeout: Duration, stop: &Stop) -> io::Result<Ran> {
        let deadline = Instant::now() + timeout;
        let ended = loop {
            if let Some(status) = self.child.try_wait()? {
                break Ended::Exited(status);
            }
            if stop.get() {
                break Ended::Stopped;
            }
            if Instant::now() >= deadline {
                break Ended::TimedOut;
            }
            thread::sleep(POLL);
        };
        let status = end_group(&mut self.child, self.group)?;
        let ended = match ended {
            Ended::Exited(_) => Ended::Exited(status),
            stopped => stopped,
        };

        // A process that escaped the group may hold the output open; it
        // does not hold the call.
        let _ = self.finished.recv_timeout(GRACE);
        let capture = self
            .capture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .expect("the output is taken once");
        let output = capture.truncator.finish();
        let full = output.cut.then(|| capture.full.finish());
        Ok(Ran {
            ended,
            output,
            full,
        })
    }
}

struct Capture {
    truncator: Truncator,
    full: Full,
}

impl Capture {
    fn feed(&mut self, bytes: &[u8]) {
        self.truncator.feed(bytes);
        self.full.feed(bytes);
    }
}

/// The whole output, cut to fit [`Bytes::LIMIT`]: its head and its tail,
/// and how much was left out between them. Holds at most the limit, however
/// much the command prints.
#[derive(Default)]
struct Full {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted: u64,
}

/// Each of the head and the tail, leaving room for the marker between.
const HALF: usize = Bytes::LIMIT / 2 - 64;

impl Full {
    fn feed(&mut self, mut bytes: &[u8]) {
        let room = HALF - self.head.len();
        let n = room.min(bytes.len());
        self.head.extend_from_slice(&bytes[..n]);
        bytes = &bytes[n..];
        self.tail.extend(bytes);
        let over = self.tail.len().saturating_sub(HALF);
        self.tail.drain(..over);
        self.omitted += over as u64;
    }

    fn finish(self) -> Vec<u8> {
        let mut out = self.head;
        if self.omitted > 0 {
            out.extend_from_slice(
                format!("\n[... {} bytes left out ...]\n", self.omitted).as_bytes(),
            );
        }
        out.extend(self.tail);
        out
    }
}

/// Make sure nothing of the group is left: ask it to stop, give it a
/// moment, then kill what remains, and reap the leader.
fn end_group(child: &mut Child, group: libc::pid_t) -> io::Result<ExitStatus> {
    signal(group, libc::SIGTERM);
    let deadline = Instant::now() + GRACE;
    let mut status = None;
    while Instant::now() < deadline {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if status.is_some() && !signal(group, 0) {
            break;
        }
        thread::sleep(POLL);
    }
    signal(group, libc::SIGKILL);
    match status {
        Some(status) => Ok(status),
        None => child.wait(),
    }
}

/// When a process started, in clock ticks since boot: with its pid, an
/// identity no later process can have. `None` where there is no `/proc`.
pub fn started(pid: libc::pid_t) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Field 22; the fields after the command's closing parenthesis start at 3.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Kill a group left by an instance that died while running it, if its
/// leader is still the process that instance started.
pub fn kill_orphan(group: libc::pid_t, start: u64) {
    if started(group) == Some(start) {
        signal(group, libc::SIGKILL);
    }
}

/// Send `signal` to every process in `group`; whether any received it.
fn signal(group: libc::pid_t, signal: libc::c_int) -> bool {
    // SAFETY: kill has no memory effects; a negative pid names a group.
    unsafe { libc::kill(-group, signal) == 0 }
}

/// The line a command's result ends with.
pub fn describe(ended: &Ended, timeout: Duration) -> String {
    match ended {
        Ended::Exited(status) => match (status.code(), status.signal()) {
            (Some(code), _) => format!("[exit code {code}]"),
            (None, Some(signal)) => format!("[killed by signal {signal}]"),
            (None, None) => "[ended]".into(),
        },
        Ended::TimedOut => format!(
            "[stopped after {}s, the call's time limit; its processes were killed]",
            timeout.as_secs()
        ),
        Ended::Stopped => "[interrupted; its processes were killed]".into(),
    }
}
