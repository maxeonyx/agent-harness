//! Stopping work: one protocol at every level.
//!
//! **Concept.** A stop is an owner's request carrying a deadline. Receiving
//! it, the work first makes a hard kill safe (it starts nothing new, and
//! what it holds is recorded), passes the stop to everything it owns with
//! an earlier deadline (earlier by the time it needs for itself
//! afterwards), waits for each to confirm its exit, finishes its own
//! cleanup, and confirms its own exit. At the deadline its owner kills it
//! without its cooperation. A hard kill is a stop whose deadline has come,
//! and asking again can bring a deadline forward, never back, so "press
//! Stop twice", "Ctrl-C twice" and "the deadline passed" are one event.
//!
//! Every deadline is written as `within` milliseconds of the fact that
//! states it ([`deadline`]): the journal's clock is the only one that
//! matters, whoever asked.
//!
//! | Level | Asked by | Cleans up | Confirms | Hard kill |
//! |---|---|---|---|---|
//! | Task | [`Drain::Abandon`] | Its request is cut and its calls and members are stopped, then it takes one tidy-up turn ([`Prompt::Tidy`]) | [`Outcome::Abandoned`], with that turn's report | [`TaskEvent::Expired`]: no more requests, everything it owns stopped now |
//! | Call | [`CallEvent::Stopping`], sent as [`ToLimb::Stop`] | Its limb's: `SIGTERM` to its processes | Settled, interrupted, once its processes are gone | `SIGKILL` |
//! | Request | [`Todo::Cut`] | none: its socket is cut at once | [`ModelCallEvent::Ended`] | (the same) |
//! | Limb process | [`ToLimb::Shutdown`], or its input closing | Stops every call | Exits once nothing it started is left | Its owner kills its whole tree |
//! | Brain process | `SIGINT`, `SIGTERM`, `SIGHUP`: [`Fact::Shutdown`] | Starts nothing; lets requests finish; shuts its limbs down | Exits once every request and limb has | A second signal: the deadline is now |
//!
//! Three ways of stopping differ in whether the work is still wanted. An
//! **abandon** ([`Drain::Abandon`]) is an owner who no longer wants it:
//! Max's Abandon, or an abandoned owner's. The task ends, done but
//! abandoned. A **pause** ([`Pause`]) is Max wanting it later: nothing new
//! starts under the task and nothing running is cut, so it is not a stop
//! of this protocol at all; parking adds one turn in which each agent
//! brings its own work to a safe point. A **shutdown**
//! ([`Fact::Shutdown`]) is a replica going away while its work is still
//! wanted: Ctrl-C on the brain. Tasks are not stopped at all; what was
//! running in the process is cut, recorded as such, and the next launch
//! carries every task on from the journal.
//!
//! Durable work (tasks, calls, requests) is stopped by facts the brain
//! acts on from [`State::todo`]; work that lives only in a process (a
//! request's socket, a command's processes, a limb, the brain) by the same
//! protocol in the `harness-runtime` crate, in band: a stop reaches it
//! through whatever it is blocked on.
//!
//! **Vision.** Structured concurrency: nothing a task started outlives it,
//! every level confirms its exit, and every wait is bounded by a deadline
//! that shrinks on the way down. Cleanup is the agent's own, as a turn,
//! because only the agent knows what its work left half done; a misbehaving
//! one is killed at its deadline all the same.
//!
//! **Bx.** An abandoned task's report is a value in its record like any
//! result, and the abandon itself is a fact, not an exception that unwinds
//! state: what was done, what was cut and why stay readable. Unlike Bx's
//! artifacts, a lifecycle and its deadlines change over time, so they live
//! in the fold, not in the values.
//!
//! [`Drain::Abandon`]: crate::task::Drain::Abandon
//! [`Prompt::Tidy`]: crate::delivery::Prompt::Tidy
//! [`Outcome::Abandoned`]: crate::task::Outcome::Abandoned
//! [`TaskEvent::Expired`]: crate::task::TaskEvent::Expired
//! [`CallEvent::Stopping`]: crate::call::CallEvent::Stopping
//! [`ToLimb::Stop`]: crate::environment::ToLimb::Stop
//! [`ToLimb::Shutdown`]: crate::environment::ToLimb::Shutdown
//! [`Todo::Cut`]: crate::state::Todo::Cut
//! [`Pause`]: crate::task::Pause
//! [`ModelCallEvent::Ended`]: crate::model_call::ModelCallEvent::Ended
//! [`Fact::Shutdown`]: crate::journal::Fact::Shutdown
//! [`State::todo`]: crate::state::State::todo

use crate::id::Timestamp;

/// The deadline a fact stated at `at` gives with `within` milliseconds.
pub fn deadline(at: Timestamp, within: u64) -> Timestamp {
    Timestamp(at.0.saturating_add(i64::try_from(within).unwrap_or(i64::MAX)))
}
