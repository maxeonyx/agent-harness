//! The process half of cancellation: blocking primitives that a request to
//! stop reaches in band, on Linux, with threads and no async runtime.
//!
//! **Concept.** The harness stops work by one protocol at every level
//! ([`harness_core::cancel`]): an owner asks with a deadline; the work makes a
//! hard kill safe, passes the request on with an earlier deadline to what it
//! owns, waits for each to exit, finishes its own cleanup and exits; at the
//! deadline its owner kills it.
//! The core states that protocol for durable work, as facts. This crate is
//! the same protocol for work that lives only in a process: a request's
//! socket, a command's processes, a limb process, the brain itself.
//!
//! A stop reaches work through whatever the work is blocked on, never
//! through a flag polled on a timer:
//! - [`Cancel`] is one piece of work's request to stop. Each [`Waiter`]
//!   made from it is an eventfd that `poll` watches beside the descriptors
//!   the work is blocked on, so a request wakes it at once. Compute loops
//!   call [`Cancel::is_requested`] between steps.
//! - [`Socket`] is a TCP connection whose connect, reads and writes end in
//!   [`Cancelled`] as soon as a stop is requested.
//! - [`Group`] is a tree of processes an owner started, found by its
//!   process group and by a mark in its environment, asked to stop by
//!   signal or in band, killed at the deadline, and confirmed gone through
//!   pidfds ([`end`]).
//! - [`signals`] turns the process's own signals into messages in its
//!   owner's inbox.
//!
//! **Vision.** Structured concurrency all the way down: nothing a task
//! starts outlives it, a stop takes effect while the work is blocked, a
//! second Ctrl-C or a deadline is a hard kill that needs no cooperation, and
//! the harness leaves nothing in the process table.
//!
//! **Not covered.** Name resolution (`getaddrinfo`) blocks a connect and
//! cannot be cancelled. A process that leaves its group and clears its
//! environment is found only once the subreaper adopts it, and a process in
//! uninterruptible sleep can outlive `SIGKILL`; [`Ended::Killed`] names it.
//! A cgroup per limb would close the first escape.

mod cancel;
mod group;
pub mod signals;
mod socket;

pub use cancel::{Cancel, Cancelled, Waiter, Wake};
pub use group::{
    Ended, Group, Process, children, end, marked, reap_children, reap_detached, subreaper,
};
pub use socket::Socket;
