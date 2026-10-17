//! A request to stop, and waiting on it beside other work.

use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// One piece of work's request to stop, with its deadline: when the work
/// must have exited, after which its owner kills it.
///
/// **Concept.** The in-process form of a stop ([`harness_core::cancel`]).
/// An owner makes one for each piece of work it runs in this process (a
/// request, a call, a limb's whole shutdown) and requests the stop; the work
/// waits through a [`Waiter`], or checks [`Cancel::is_requested`] between
/// steps. A request can only bring the deadline forward, so asking again (a
/// second Ctrl-C, Max's kill) makes the stop harder, never softer, and a
/// deadline that has come ([`Cancel::is_due`]) is the hard stop. Clones
/// share it.
///
/// It is deliberately flat: an owner passes a stop on to what it owns by
/// requesting each of their cancels with an earlier deadline itself, so
/// there is no second tree beside the owner's own record of what it runs.
#[derive(Clone, Default)]
pub struct Cancel(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    deadline: Option<Instant>,
    /// Moved on by each request that brought the deadline forward.
    version: u64,
    waiters: Vec<(u64, RawFd)>,
    next: u64,
}

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Ask the work to have exited by `deadline`, unless it was already
    /// asked for an earlier one. Wakes every waiter.
    pub fn request(&self, deadline: Instant) {
        let mut state = lock(&self.0);
        if state.deadline.is_some_and(|d| d <= deadline) {
            return;
        }
        state.deadline = Some(deadline);
        state.version += 1;
        let one: u64 = 1;
        for (_, fd) in &state.waiters {
            // SAFETY: a live eventfd: a waiter leaves the list under this
            // lock before its descriptor closes. Eight bytes from a u64.
            unsafe { libc::write(*fd, (&one as *const u64).cast(), 8) };
        }
    }

    /// Ask the work to stop now: the hard stop.
    pub fn now(&self) {
        self.request(Instant::now());
    }

    pub fn deadline(&self) -> Option<Instant> {
        lock(&self.0).deadline
    }

    pub fn is_requested(&self) -> bool {
        self.deadline().is_some()
    }

    /// Whether the deadline has come: nothing graceful is left.
    pub fn is_due(&self) -> bool {
        self.deadline().is_some_and(|d| d <= Instant::now())
    }

    /// A way to block on descriptors that this cancel also wakes.
    pub fn waiter(&self) -> io::Result<Waiter> {
        // SAFETY: eventfd only makes a descriptor.
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new descriptor nothing else owns.
        let event = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut state = lock(&self.0);
        let key = state.next;
        state.next += 1;
        state.waiters.push((key, event.as_raw_fd()));
        Ok(Waiter {
            cancel: self.clone(),
            key,
            event,
            seen: 0,
        })
    }
}

/// Blocks one thread on descriptors and its [`Cancel`] at once.
pub struct Waiter {
    cancel: Cancel,
    key: u64,
    event: OwnedFd,
    /// The cancel's version this waiter last reported.
    seen: u64,
}

/// What ended a [`Waiter::wait`].
#[derive(Debug, PartialEq, Eq)]
pub enum Wake {
    /// A descriptor is ready, hung up or in error: try the operation.
    Ready,
    /// The stop was requested, or its deadline brought forward, since this
    /// waiter last said so. A first request is always reported, however
    /// long before the waiter was made.
    Changed,
    TimedOut,
}

impl Waiter {
    pub fn cancel(&self) -> &Cancel {
        &self.cancel
    }

    /// Block until one of `fds` is ready for its events (`libc::POLLIN`,
    /// `libc::POLLOUT`), the cancel changes, or `until`.
    pub fn wait(&mut self, fds: &[(RawFd, i16)], until: Option<Instant>) -> io::Result<Wake> {
        let mut polls: Vec<libc::pollfd> = fds
            .iter()
            .map(|&(fd, events)| (fd, events))
            .chain([(self.event.as_raw_fd(), libc::POLLIN)])
            .map(|(fd, events)| libc::pollfd {
                fd,
                events,
                revents: 0,
            })
            .collect();
        loop {
            let version = lock(&self.cancel.0).version;
            if version != self.seen {
                self.seen = version;
                return Ok(Wake::Changed);
            }
            let timeout = until.map_or(-1, |until| {
                let left = until.saturating_duration_since(Instant::now());
                left.as_nanos().div_ceil(1_000_000).min(i32::MAX as u128) as i32
            });
            // SAFETY: a live array of pollfds of this length.
            let n = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, timeout) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            let (event, fds) = polls.split_last().expect("the eventfd is polled");
            if event.revents != 0 {
                let mut count = 0u64;
                // SAFETY: eight bytes into a u64.
                unsafe { libc::read(event.fd, (&mut count as *mut u64).cast(), 8) };
                continue;
            }
            if fds.iter().any(|p| p.revents != 0) {
                return Ok(Wake::Ready);
            }
            if until.is_some_and(|u| u <= Instant::now()) {
                return Ok(Wake::TimedOut);
            }
        }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        lock(&self.cancel.0).waiters.retain(|(k, _)| *k != self.key);
    }
}

/// The error a blocking operation ends in when its work is asked to stop.
/// Not [`io::ErrorKind::Interrupted`], which std's `read_exact` and
/// `write_all` quietly retry.
#[derive(Debug)]
pub struct Cancelled;

impl Cancelled {
    pub fn error() -> io::Error {
        io::Error::other(Cancelled)
    }

    pub fn is(e: &io::Error) -> bool {
        e.get_ref().is_some_and(|e| e.is::<Cancelled>())
    }
}

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("asked to stop")
    }
}

impl std::error::Error for Cancelled {}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
