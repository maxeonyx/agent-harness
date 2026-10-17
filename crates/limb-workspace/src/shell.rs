//! Running one command as a tree of processes the call owns.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use harness_core::id::CallId;
use harness_core::value::Bytes;
use harness_runtime::{Cancel, Ended as TreeEnded, Group, Wake};

use crate::trunc::{Truncated, Truncator};

/// How a command ended.
pub enum Ended {
    Exited(ExitStatus),
    /// Its call was stopped.
    Stopped,
    TimedOut,
}

pub struct Ran {
    pub ended: Ended,
    pub output: Truncated,
    /// The whole output, when `output` left some of it out.
    pub full: Option<Vec<u8>>,
    /// How what was left of its tree ended.
    pub tree: TreeEnded,
}

/// How long what a command leaves behind gets to exit after `SIGTERM`,
/// when the command exits or reaches its time limit, before `SIGKILL`.
const GRACE: Duration = Duration::from_secs(2);

/// The mark a call's processes carry in their environment, which the
/// limb's next instance finds them by if this one is killed.
pub fn mark(call: CallId) -> String {
    call.to_string()
}

/// Run `command` with `bash -c` in `dir` as the tree of processes `call`
/// owns ([`Group`]), its stdout and stderr merged as a terminal shows them,
/// until it exits, reaches `timeout`, or `cancel` is requested; then end
/// what is left of the tree: `SIGTERM` to each process, and `SIGKILL` at
/// the deadline (the cancel's, or a short grace). Returns only once nothing
/// of the tree is left, or what is left has outlived `SIGKILL`.
///
/// One thread does it all, waiting in `poll` on the output, the leader's
/// exit and the cancel at once.
pub fn run(
    call: CallId,
    command: &str,
    dir: &Path,
    timeout: Duration,
    cancel: &Cancel,
    truncator: Truncator,
    progress: impl FnMut(&[u8]),
) -> io::Result<Ran> {
    let (reader, writer) = io::pipe()?;
    // SAFETY: fcntl on the pipe's read end, which this function owns.
    unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
    let mut builder = Command::new("bash");
    builder
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer);
    let mut group = Group::spawn(&mut builder, &mark(call))?;
    // The builder holds the pipe's write end; the output ends only once
    // every holder has closed it.
    drop(builder);
    let mut output = Output {
        fd: reader.as_raw_fd(),
        reader,
        open: true,
        capture: Capture {
            truncator,
            full: Full::default(),
        },
        progress,
    };
    let mut waiter = cancel.waiter()?;
    let limit = Instant::now() + timeout;
    let ended = loop {
        output.drain();
        if let Some(status) = group.exited() {
            break Ended::Exited(status);
        }
        if cancel.is_requested() {
            break Ended::Stopped;
        }
        let mut fds = vec![(group.exits(), libc::POLLIN)];
        fds.extend(output.watch().map(|fd| (fd, libc::POLLIN)));
        if waiter.wait(&fds, Some(limit))? == Wake::TimedOut {
            break Ended::TimedOut;
        }
    };
    let deadline = match ended {
        Ended::Stopped => cancel.deadline().expect("a stop was requested"),
        Ended::Exited(_) | Ended::TimedOut => Instant::now() + GRACE,
    };
    let tree = group.end(&mut waiter, deadline, Some(libc::SIGTERM), || {
        output.drain();
        output.watch()
    })?;
    // A process that left the tree's reach may hold the output open; it
    // does not hold the call.
    output.drain();
    let capture = output.capture;
    let truncated = capture.truncator.finish();
    let full = truncated.cut.then(|| capture.full.finish());
    Ok(Ran {
        ended,
        output: truncated,
        full,
        tree,
    })
}

/// A command's output as it arrives.
struct Output<P> {
    reader: io::PipeReader,
    fd: RawFd,
    /// Until every writer has closed it.
    open: bool,
    capture: Capture,
    progress: P,
}

impl<P: FnMut(&[u8])> Output<P> {
    /// Take everything the pipe holds now.
    fn drain(&mut self) {
        let mut buffer = [0; 8192];
        while self.open {
            match self.reader.read(&mut buffer) {
                Ok(0) => self.open = false,
                Ok(n) => {
                    self.capture.feed(&buffer[..n]);
                    (self.progress)(&buffer[..n]);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => self.open = false,
            }
        }
    }

    /// The descriptor to wake on for more.
    fn watch(&self) -> Option<RawFd> {
        self.open.then_some(self.fd)
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

/// The line a command's result ends with.
pub fn describe(ran: &Ran, timeout: Duration, shutdown: bool) -> String {
    let ended = match &ran.ended {
        Ended::Exited(status) => match (status.code(), status.signal()) {
            (Some(code), _) => format!("[exit code {code}]"),
            (None, Some(signal)) => format!("[killed by signal {signal}]"),
            (None, None) => "[ended]".into(),
        },
        Ended::TimedOut => format!(
            "[stopped after {}s, the call's time limit]",
            timeout.as_secs()
        ),
        Ended::Stopped if shutdown => "[interrupted: the harness shut down]".into(),
        Ended::Stopped => "[interrupted]".into(),
    };
    let tree = match &ran.tree {
        TreeEnded::Graceful => String::new(),
        TreeEnded::Killed { survivors } if survivors.is_empty() => {
            " [what it left running did not exit when asked, and was killed]".into()
        }
        TreeEnded::Killed { survivors } => {
            format!(" [processes {survivors:?} it started survived SIGKILL and are still running]")
        }
    };
    format!("{ended}{tree}")
}
