//! Running one command as a tree of processes the call owns, in a systemd
//! scope of its own.

use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use harness_core::id::CallId;
use harness_limb::scope::{self, Scope};
use harness_limb::{Limit, SHUTDOWN};
use harness_runtime::{Cancel, Ended as TreeEnded, Group, Member, Wake};

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
    /// How what was left of its tree ended.
    pub tree: TreeEnded,
    /// Why it ran without a scope of its own, if it did.
    pub unscoped: Option<String>,
}

/// How long what a command leaves behind gets to exit after `SIGTERM`,
/// when the command exits or reaches its time limit, before `SIGKILL`.
const GRACE: Duration = Duration::from_secs(2);

/// The mark a call's processes carry in their environment, which the
/// limb's next instance finds them by if this one is killed.
pub fn mark(call: CallId) -> String {
    call.to_string()
}

/// The scope a call's command runs in, without `.scope`.
pub fn unit(call: CallId) -> String {
    format!("harness-call-{call}")
}

/// End what the command of `call` left running when an earlier instance of
/// the limb was killed: its marked processes, then what is left in its
/// scope.
pub fn end_orphan(call: CallId) -> io::Result<()> {
    let mark = mark(call);
    let mut waiter = Cancel::new().waiter()?;
    let deadline = Instant::now() + SHUTDOWN;
    let members = || harness_runtime::marked(&mark);
    harness_runtime::end(&mut waiter, deadline, Some(libc::SIGTERM), members, || None)?;
    scope::kill(&unit(call));
    Ok(())
}

/// Run `command` with `bash -c` in `dir` as the tree of processes `call`
/// owns ([`Group`]), in `scope` (or, if `systemd-run` fails, without one,
/// saying why), its stdout and stderr merged as a terminal shows them,
/// until it exits, reaches `timeout` (unless `limit` is lifted by then), or
/// `cancel` is requested; then end what is left of the tree: `SIGTERM` to
/// each process, and `SIGKILL` at the deadline (the cancel's, or a short
/// grace), and kill what is left in its scope, `setsid` or not. Returns
/// only once nothing of the tree is left, or what is left has outlived
/// `SIGKILL`.
///
/// One thread does it all, waiting in `poll` on the output, the leader's
/// exit and the cancel at once.
#[allow(clippy::too_many_arguments)]
pub fn run(
    call: CallId,
    command: &str,
    dir: &Path,
    scope: &Scope,
    timeout: Duration,
    limit: &Limit,
    cancel: &Cancel,
    truncator: Truncator,
    progress: impl FnMut(&[u8]),
) -> io::Result<Ran> {
    let (reader, writer) = io::pipe()?;
    // SAFETY: fcntl on the pipe's read end, which this function owns.
    unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
    let out = OwnedFd::from(writer);
    let args: [OsString; 2] = ["-c".into(), command.into()];
    let spawn = |scope: Option<&Scope>| -> io::Result<(Group, scope::Spawn)> {
        let mut spawn = scope::command("bash".as_ref(), &args, scope, Some(out.try_clone()?))?;
        spawn.command.current_dir(dir);
        Ok((Group::spawn(&mut spawn.command, &mark(call))?, spawn))
    };
    let (mut group, spawned) = spawn(Some(scope))?;
    let unscoped = match spawned.scoped() {
        scope::Scoped::In => None,
        scope::Scoped::Without(why) => Some(why),
        scope::Scoped::Failed(why) => {
            // `systemd-run` ended without running the command.
            let mut waiter = cancel.waiter()?;
            group.end(&mut waiter, Instant::now() + GRACE, None, || None)?;
            let (plain, spawned) = spawn(None)?;
            let _ = spawned.scoped();
            group = plain;
            Some(why)
        }
    };
    // Held here, the pipe's write end would keep the output from ending.
    drop(out);
    let mut output = Output {
        fd: reader.as_raw_fd(),
        reader,
        open: true,
        truncator,
        progress,
    };
    let mut waiter = cancel.waiter()?;
    let mut until = Some(Instant::now() + timeout);
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
        if waiter.wait(&fds, until)? == Wake::TimedOut {
            if limit.is_lifted() {
                until = None;
                continue;
            }
            break Ended::TimedOut;
        }
    };
    let deadline = match ended {
        Ended::Stopped => cancel.deadline().unwrap_or_else(Instant::now),
        Ended::Exited(_) | Ended::TimedOut => Instant::now() + GRACE,
    };
    let tree = group.end(&mut waiter, deadline, Some(libc::SIGTERM), || {
        output.drain();
        output.watch()
    })?;
    if unscoped.is_none() {
        scope::kill(&scope.unit);
    }
    // A process that left the tree's reach may hold the output open; it
    // does not hold the call.
    output.drain();
    Ok(Ran {
        ended,
        output: output.truncator.finish(),
        tree,
        unscoped,
    })
}

/// A command's output as it arrives.
struct Output<P> {
    reader: io::PipeReader,
    fd: RawFd,
    /// Until every writer has closed it.
    open: bool,
    truncator: Truncator,
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
                    self.truncator.feed(&buffer[..n]);
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
    let tree = &ran.tree;
    let left = match (&ran.ended, tree.asked.is_empty(), tree.killed) {
        (Ended::Exited(_), false, false) => {
            format!(
                " [what it left running was stopped: {}]",
                members(&tree.asked)
            )
        }
        (Ended::Exited(_), false, true) => format!(
            " [what it left running did not exit on SIGTERM and was killed: {}]",
            members(&tree.asked)
        ),
        _ => String::new(),
    };
    let survivors = match tree.survivors.is_empty() {
        true => String::new(),
        false => format!(
            " [still running after SIGKILL: {}]",
            members(&tree.survivors)
        ),
    };
    let unscoped = match &ran.unscoped {
        Some(why) => format!(" [it ran without a systemd scope of its own: {why}]"),
        None => String::new(),
    };
    format!("{ended}{left}{survivors}{unscoped}")
}

/// Processes as PID and command line, each cut to a line.
fn members(members: &[Member]) -> String {
    const WIDTH: usize = 120;
    let member = |m: &Member| {
        let command = match m.command.char_indices().nth(WIDTH) {
            Some((cut, _)) => format!("{}…", &m.command[..cut]),
            None => m.command.clone(),
        };
        format!("PID {} `{command}`", m.pid)
    };
    members.iter().map(member).collect::<Vec<_>>().join(", ")
}
