//! The process's own signals, as messages.
//!
//! A signal that asks a process to stop (Ctrl-C's `SIGINT`, `SIGTERM`,
//! `SIGHUP`) is a request from its owner like any other, so it arrives the
//! same way: as a message in the inbox of whoever owns the process's
//! shutdown, handled between its other events. The handler only writes the
//! signal's number to a pipe, and a thread reads it out.
//!
//! Nothing is blocked with the signal mask: children inherit the mask, and a
//! command started with these signals blocked would ignore the `SIGTERM`
//! its owner sends to stop it.

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

/// The pipe's write end, for the handler.
static PIPE: AtomicI32 = AtomicI32::new(-1);

/// Call `deliver` with each of `signals` as it arrives, in order, on a
/// thread of its own. Once per process.
pub fn forward(
    signals: &[libc::c_int],
    deliver: impl Fn(libc::c_int) + Send + 'static,
) -> io::Result<()> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: pipe2 writes two descriptors.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let [read, write] = fds;
    // A full pipe drops a signal rather than blocking the handler.
    // SAFETY: fcntl on a descriptor just made.
    unsafe { libc::fcntl(write, libc::F_SETFL, libc::O_NONBLOCK) };
    if PIPE
        .compare_exchange(-1, write, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(io::Error::other("signals are already forwarded"));
    }
    for &signal in signals {
        // SAFETY: all-zero is a valid sigaction to fill in.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = handler as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        // SAFETY: installs a handler that only makes async-signal-safe calls.
        let r = unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut())
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: the read end, owned from here on.
    let mut pipe = unsafe { File::from_raw_fd(read) };
    thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut byte = [0u8; 1];
            loop {
                match pipe.read(&mut byte) {
                    Ok(1) => deliver(libc::c_int::from(byte[0])),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    _ => return,
                }
            }
        })?;
    Ok(())
}

extern "C" fn handler(signal: libc::c_int) {
    // SAFETY: write is async-signal-safe; errno is kept for the code the
    // signal interrupted.
    unsafe {
        let errno = *libc::__errno_location();
        let byte = signal as u8;
        libc::write(PIPE.load(Ordering::Relaxed), (&raw const byte).cast(), 1);
        *libc::__errno_location() = errno;
    }
}

/// The signal's usual name.
pub fn name(signal: libc::c_int) -> &'static str {
    match signal {
        libc::SIGINT => "SIGINT",
        libc::SIGTERM => "SIGTERM",
        libc::SIGHUP => "SIGHUP",
        _ => "a signal",
    }
}
