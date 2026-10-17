//! Trees of processes an owner started, and ending them.

use std::collections::HashSet;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{ChildStdin, ChildStdout, Command, ExitStatus};
use std::time::{Duration, Instant};

use crate::cancel::Waiter;

/// The environment variable that marks a tree is this prefix and its mark.
const MARK: &str = "HARNESS_OWNER_";
/// How often an ending tree is looked at again, since a member can start
/// another while it is asked to stop.
const RESCAN: Duration = Duration::from_millis(100);
/// How long a tree killed at its deadline gets to vanish before what is
/// left is reported as surviving.
const KILL_WAIT: Duration = Duration::from_secs(2);

/// A tree of processes one owner started: a command a call runs, or a limb
/// the brain runs.
///
/// **Concept.** The process level of the stop protocol
/// ([`harness_core::cancel`]). Its leader leads a process group of its own,
/// and its environment carries a mark unique on the machine that every
/// process it starts inherits. Its members are every live process in the
/// group and every process carrying the mark, so a process that leaves the
/// group (a daemon's double fork) is still the owner's. It has
/// ended only when it has no members left: [`Group::end`] asks, waits on
/// each member's pidfd, kills at the deadline, and says what would not die.
///
/// Marks nest: a tree started inside another carries both marks, so a
/// limb's tree holds its calls' trees.
///
/// A tree made [`Group::detachable`] lets go of a member that starts a
/// session of its own (`setsid`): the one way to leave on purpose.
pub struct Group {
    leader: libc::pid_t,
    pidfd: OwnedFd,
    mark: String,
    /// The session a member must stay in to be the tree's, once the tree is
    /// detachable.
    session: Option<libc::pid_t>,
    status: Option<ExitStatus>,
    /// The leader's piped stdin and stdout, until its owner takes them.
    pipes: (Option<ChildStdin>, Option<ChildStdout>),
}

/// How a tree ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    /// Every member exited before the deadline.
    Graceful,
    /// The deadline came and what was left was killed. `survivors` outlived
    /// `SIGKILL` (uninterruptible sleep): still running, and said so.
    Killed { survivors: Vec<libc::pid_t> },
}

impl Group {
    /// Start `command` as the leader of a new process group, its environment
    /// marked with `mark`: an identity no other tree on the machine has,
    /// such as the id of the call it runs.
    pub fn spawn(command: &mut Command, mark: &str) -> io::Result<Group> {
        let mark = format!("{MARK}{mark}");
        command.env(&mark, "1").process_group(0);
        let mut child = command.spawn()?;
        let leader = child.id() as libc::pid_t;
        let pipes = (child.stdin.take(), child.stdout.take());
        // The leader is an unreaped child, so its pid is still its own.
        let pidfd = pidfd(leader)?;
        Ok(Group {
            leader,
            pidfd,
            mark,
            session: None,
            status: None,
            pipes,
        })
    }

    /// Let a member go once it starts a session of its own (`setsid`): it
    /// leaves this tree, though not the trees around it, whose marks it still
    /// carries, nor its orphan's subreaper. A command can so start a server
    /// that outlives it, and that its owner's owner still ends.
    pub fn detachable(mut self) -> Group {
        // SAFETY: getsid on the leader, an unreaped child.
        self.session = Some(unsafe { libc::getsid(self.leader) });
        self
    }

    /// The leader's stdin and stdout, where the command piped them: the
    /// owner's line to it. Once.
    pub fn pipes(&mut self) -> (Option<ChildStdin>, Option<ChildStdout>) {
        (self.pipes.0.take(), self.pipes.1.take())
    }

    /// The leader's pid, which is the group's id.
    pub fn id(&self) -> libc::pid_t {
        self.leader
    }

    /// The leader's pidfd, readable once it has exited.
    pub fn exits(&self) -> RawFd {
        self.pidfd.as_raw_fd()
    }

    /// The leader's status once it has exited. Reaps it, and any process of
    /// the group this one adopted as subreaper.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        loop {
            let mut status = 0;
            // SAFETY: waitpid writes only the status.
            let pid = unsafe { libc::waitpid(-self.leader, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            if pid == self.leader {
                self.status = Some(ExitStatus::from_raw(status));
            }
        }
        self.status
    }

    /// End the tree by `deadline`, or the earlier one the waiter's cancel
    /// is brought forward to: [`end`] over its members.
    pub fn end(
        &mut self,
        waiter: &mut Waiter,
        deadline: Instant,
        ask: Option<libc::c_int>,
        serve: impl FnMut() -> Option<RawFd>,
    ) -> io::Result<Ended> {
        let (group, mark, session) = (self.leader, self.mark.clone(), self.session);
        end(
            waiter,
            deadline,
            ask,
            || {
                self.exited();
                live(|p| {
                    p.group == group
                        || (session.is_none_or(|s| p.session == s) && carries(p.pid, &mark))
                })
            },
            serve,
        )
    }
}

/// One process, as `/proc` shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Process {
    pub pid: libc::pid_t,
    pub parent: libc::pid_t,
    pub group: libc::pid_t,
    pub session: libc::pid_t,
    /// When it started, in clock ticks since boot: with the pid, an
    /// identity no later process has.
    pub start: u64,
}

/// End a tree of processes by a deadline: ask the tree as it stands with
/// `ask` (a signal; `None` where the owner asked in band), wait until none
/// is left, and at the deadline (or the earlier one the waiter's cancel is
/// brought forward to) kill each one left.
///
/// Only the processes that started before it asked are asked, each once,
/// including one a scan missed because it was forked just as the scan ran.
/// What starts after is the cleanup's own work (a trap's `sleep`, a build
/// tool reaping its jobs), its to finish: asking it too would cut the
/// cleanup short. It is killed at the deadline with the rest.
///
/// `members` lists the tree's live processes, reaping what it must first;
/// it is called again on every wake, since members start others and adopted
/// orphans appear. `serve` runs on every wake too, and names a descriptor to
/// wake on: the output a member would otherwise block writing.
///
/// Every signal goes through a pidfd opened and checked against the
/// process's start time, so a pid reused meanwhile is never signalled.
pub fn end(
    waiter: &mut Waiter,
    deadline: Instant,
    ask: Option<libc::c_int>,
    mut members: impl FnMut() -> Vec<Process>,
    mut serve: impl FnMut() -> Option<RawFd>,
) -> io::Result<Ended> {
    let mut asked: HashSet<(libc::pid_t, u64)> = HashSet::new();
    // When it first asked, in the clock `/proc` gives start times in.
    let mut asked_at: Option<u64> = None;
    let mut killed: Option<Instant> = None;
    loop {
        let watch = serve();
        let pinned: Vec<(Process, OwnedFd)> = members()
            .into_iter()
            .filter_map(|p| Some((p, pin(&p)?)))
            .collect();
        if pinned.is_empty() {
            return Ok(match killed {
                None => Ended::Graceful,
                Some(_) => Ended::Killed {
                    survivors: Vec::new(),
                },
            });
        }
        let now = Instant::now();
        let deadline = waiter
            .cancel()
            .deadline()
            .map_or(deadline, |d| d.min(deadline));
        let until = if now >= deadline {
            let since = *killed.get_or_insert(now);
            if now >= since + KILL_WAIT {
                let survivors = pinned.iter().map(|(p, _)| p.pid).collect();
                return Ok(Ended::Killed { survivors });
            }
            for (_, fd) in &pinned {
                signal(fd, libc::SIGKILL);
            }
            (now + RESCAN).min(since + KILL_WAIT)
        } else {
            if let Some(ask) = ask {
                let first = asked_at.is_none();
                let at = *asked_at.get_or_insert_with(ticks);
                for (p, fd) in &pinned {
                    if (first || p.start < at) && asked.insert((p.pid, p.start)) {
                        signal(fd, ask);
                    }
                }
            }
            (now + RESCAN).min(deadline)
        };
        let mut fds: Vec<(RawFd, i16)> = pinned
            .iter()
            .map(|(_, fd)| (fd.as_raw_fd(), libc::POLLIN))
            .collect();
        fds.extend(watch.map(|fd| (fd, libc::POLLIN)));
        waiter.wait(&fds, Some(until))?;
    }
}

/// The live processes carrying `mark`: a tree whose owner is gone, such as
/// the commands of a limb instance that was killed.
pub fn marked(mark: &str) -> Vec<Process> {
    let mark = format!("{MARK}{mark}");
    live(|p| carries(p.pid, &mark))
}

/// This process's live children: once it is a subreaper, every orphan of
/// what it started that nothing else accounts for.
pub fn children() -> Vec<Process> {
    let me = std::process::id() as libc::pid_t;
    live(|p| p.parent == me)
}

/// Reap every child that has exited. Only where no other thread waits on a
/// child of its own.
pub fn reap_children() {
    loop {
        let mut status = 0;
        // SAFETY: waitpid writes only the status.
        if unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } <= 0 {
            break;
        }
    }
}

/// Reap every child that has exited after starting a session of its own:
/// an orphan this process adopted as subreaper, which had left its command
/// ([`Group::detachable`]). Unlike [`reap_children`], safe while other
/// threads wait on children of their own, which stay in this session.
pub fn reap_detached() {
    let me = std::process::id() as libc::pid_t;
    // SAFETY: getsid(0) reads this process's session.
    let session = unsafe { libc::getsid(0) };
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return;
    };
    let exited = dir
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .filter_map(stat)
        .filter(|(p, zombie)| *zombie && p.parent == me && p.session != session);
    for (p, _) in exited {
        let mut status = 0;
        // SAFETY: waitpid writes only the status.
        unsafe { libc::waitpid(p.pid, &mut status, libc::WNOHANG) };
    }
}

/// Become the subreaper of everything this process starts: an orphan below
/// it is adopted by it, not by init, so it stays this process's to end.
pub fn subreaper() -> io::Result<()> {
    // SAFETY: prctl with an integer argument.
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// The live (not zombie) processes `keep` keeps, other than this one.
fn live(keep: impl Fn(&Process) -> bool) -> Vec<Process> {
    let me = std::process::id() as libc::pid_t;
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .filter(|pid| *pid != me)
        .filter_map(stat)
        .filter(|(p, zombie)| !zombie && keep(p))
        .map(|(p, _)| p)
        .collect()
}

/// A process as `/proc/<pid>/stat` describes it, and whether it is a zombie.
fn stat(pid: libc::pid_t) -> Option<(Process, bool)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The fields after the command's closing parenthesis start at field 3.
    let fields: Vec<&str> = stat[stat.rfind(')')? + 1..].split_whitespace().collect();
    let process = Process {
        pid,
        parent: fields.get(1)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        session: fields.get(3)?.parse().ok()?,
        start: fields.get(19)?.parse().ok()?,
    };
    Some((process, *fields.first()? == "Z"))
}

/// Whether the process's environment carries `var`.
fn carries(pid: libc::pid_t, var: &str) -> bool {
    let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ")) else {
        return false;
    };
    let var = var.as_bytes();
    environ.split(|b| *b == 0).any(|kv| {
        kv.strip_prefix(var)
            .is_some_and(|rest| rest.first() == Some(&b'='))
    })
}

/// Now, in clock ticks since boot: the clock of a process's start time in
/// `/proc/<pid>/stat`.
fn ticks() -> u64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime and sysconf only read the clock and a constant.
    let hz = unsafe {
        libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut now);
        libc::sysconf(libc::_SC_CLK_TCK)
    };
    let hz = u64::try_from(hz).unwrap_or(100);
    let secs = u64::try_from(now.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(now.tv_nsec).unwrap_or(0);
    secs * hz + nanos * hz / 1_000_000_000
}

/// A pidfd for the process, if it is still the one `/proc` showed.
fn pin(process: &Process) -> Option<OwnedFd> {
    let fd = pidfd(process.pid).ok()?;
    let same = stat(process.pid).is_some_and(|(p, zombie)| p.start == process.start && !zombie);
    same.then_some(fd)
}

fn pidfd(pid: libc::pid_t) -> io::Result<OwnedFd> {
    // SAFETY: pidfd_open only makes a descriptor.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a new descriptor nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

fn signal(pidfd: &OwnedFd, signal: libc::c_int) {
    // SAFETY: pidfd_send_signal with no siginfo.
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
}
