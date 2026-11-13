//! Handing the face over to the next build, so it never drops.
//!
//! [`HANDOVER`](crate::brain::HANDOVER) shuts the running brain A down as
//! `SIGTERM` does. Once it has drained it stops writing but serves on,
//! read-only, and starts the candidate B that relaunch named in
//! [`Start::HANDOVER`] as its own child, so B is in the unit's cgroup,
//! passing it A's listener and a pipe. B gates on the database A left,
//! accepting on the same listener, then writes, tells systemd it is the
//! unit's main process (`MAINPID=`, which `NotifyAccess=all` allows), and
//! says so down the pipe. A then ends its links, so each tab reconnects to
//! B, takes no more connections, and exits once its dictations have ended.
//! A connection waits in the listener's queue and is never refused. If B
//! ends first, A exits, and the unit starts the installed link again.

use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use harness_core::build::Start;
use harness_face_web::{Face, Source};
use serde::Deserialize;

/// Tells B the descriptors A passed it: its listener, then the pipe's end.
const ENV: &str = "HARNESS_HANDOVER";

/// How long A waits for its dictations to end once B serves.
const DICTATING: Duration = Duration::from_secs(10 * 60);

/// The candidate relaunch asked for, in [`Start::HANDOVER`].
#[derive(Deserialize)]
struct Asked {
    argv: Vec<String>,
    by: Option<String>,
}

/// What B was passed, read before any thread starts.
pub struct Taking {
    pub listener: TcpListener,
    done: File,
}

/// What A passed this brain, if it was started to take over. Read once,
/// first thing: the descriptors are made close-on-exec again, so nothing
/// this brain starts holds them.
pub fn take() -> Result<Option<Taking>, String> {
    let Ok(fds) = std::env::var(ENV) else {
        return Ok(None);
    };
    // SAFETY: called first in `main`, before any other thread runs.
    unsafe { std::env::remove_var(ENV) };
    let fds: Vec<RawFd> = fds
        .split(' ')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{ENV}={fds}: {e}"))?;
    let [listener, done] = fds[..] else {
        return Err(format!("{ENV} names {} descriptors, not 2", fds.len()));
    };
    for fd in [listener, done] {
        // SAFETY: fcntl on a descriptor; a bad one fails, and says so.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(format!(
                "{ENV}: descriptor {fd}: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    // SAFETY: A passed these two to this process alone, which owns them now.
    Ok(Some(unsafe {
        Taking {
            listener: TcpListener::from_raw_fd(listener),
            done: File::from_raw_fd(done),
        }
    }))
}

impl Taking {
    /// B has opened: make it the unit's main process, and tell A.
    pub fn took_over(mut self) -> Result<(), String> {
        let socket = std::env::var("NOTIFY_SOCKET")
            .map_err(|_| "the unit gives no NOTIFY_SOCKET, so it cannot be told".to_string())?;
        let at = match socket.strip_prefix('@') {
            Some(name) => {
                use std::os::linux::net::SocketAddrExt;
                SocketAddr::from_abstract_name(name)
            }
            None => SocketAddr::from_pathname(&socket),
        }
        .map_err(|e| format!("NOTIFY_SOCKET={socket}: {e}"))?;
        let said = format!("MAINPID={}", std::process::id());
        UnixDatagram::unbound()
            .and_then(|d| d.send_to_addr(said.as_bytes(), &at))
            .map_err(|e| format!("NOTIFY_SOCKET={socket}: {e}"))?;
        self.done
            .write_all(b"1")
            .map_err(|e| format!("telling the brain before: {e}"))
    }
}

/// A, drained and no longer writing: start the candidate named beside
/// `db` on `listener`, serve until it has opened, then close `face` and
/// wait for its dictations. Whether B took over.
pub fn hand_over<S: Source>(db: &Path, listener: &TcpListener, face: &Face<S>) -> bool {
    match start(db, listener) {
        Ok((pid, mut opened)) => {
            let mut said = [0u8; 1];
            if opened.read(&mut said).is_ok_and(|n| n == 1) {
                outln!("harness-brain: handed over to process {pid}; dictations finish here");
                face.close();
                let until = Instant::now() + DICTATING;
                while face.dictating() > 0 && Instant::now() < until {
                    thread::sleep(Duration::from_millis(100));
                }
                return true;
            }
            errln!("harness-brain: process {pid} ended before it took over");
        }
        Err(why) => errln!("harness-brain: cannot hand over: {why}"),
    }
    false
}

/// Start the candidate, as A's child, with the listener and a pipe's
/// writing end: its process, and the pipe's reading end.
fn start(db: &Path, listener: &TcpListener) -> Result<(u32, File), String> {
    let at = db.with_file_name(Start::HANDOVER);
    let asked = std::fs::read(&at).map_err(|e| format!("{}: {e}", at.display()));
    let _ = std::fs::remove_file(&at);
    let asked: Asked =
        serde_json::from_slice(&asked?).map_err(|e| format!("{}: {e}", at.display()))?;
    let (program, args) = asked.argv.split_first().ok_or("it names no command")?;
    let mut fds = [0 as RawFd; 2];
    // SAFETY: pipe2 writes two descriptors.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(format!("a pipe: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: both were just made, and are owned here alone.
    let (opened, done) = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
    let passed = [listener.as_raw_fd(), done.as_raw_fd()];
    let mut command = Command::new(program);
    command
        .args(args)
        .env(ENV, format!("{} {}", passed[0], passed[1]))
        .env_remove(crate::brain::RELAUNCHED_BY);
    if let Some(by) = &asked.by {
        command.env(crate::brain::RELAUNCHED_BY, by);
    }
    // SAFETY: between fork and exec only fcntl runs, which is
    // async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            for fd in passed {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|e| format!("{program}: {e}"))?;
    drop(done);
    Ok((child.id(), opened))
}
