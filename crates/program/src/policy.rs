//! Policy: every decision the core leaves open, apart from what a request
//! renders and how it is worded ([`crate::render`]).

use std::collections::HashSet;
use std::time::Duration;

use harness_core::context::EntryId;
use harness_core::id::Timestamp;

/// How full a context is, in percent of its window, when each request
/// warns the agent to wrap up and compact itself: before the provider
/// refuses it and the harness compacts it with no summary of the agent's.
pub const WARN_AT: u64 = 80;

/// Requests that may fail in a row before the task fails.
pub const GIVE_UP: u32 = 5;

/// How long to wait before resending after `failures` failures in a row,
/// when the provider did not say: 2, 4, 8, 16 seconds.
pub fn backoff_ms(failures: u32) -> i64 {
    1000 << failures.min(5)
}

/// Where a request asks the provider to cache, at most the provider's four
/// places: the end of the root, the task's latest checkpoint, the end of the
/// context the request extends, the end of a shared brief it appends, and
/// its new tail.
///
/// The middle ones are what make fan-outs and compactions cheap. A fork
/// starts on its parent's response, which no parent request ever marked,
/// and fresh siblings append the same brief to the same root, so the first
/// request of the first sibling writes that prefix and the others read it
/// ([`hold`]). A compaction resumes at the latest checkpoint, which each
/// request since has kept warm by marking it. The same mark on an ordinary
/// turn re-reads the previous request's prefix. A brief is only on a
/// context's first request, which has no checkpoint yet, so the four are
/// enough.
pub fn breakpoints(
    root: EntryId,
    checkpoint: Option<EntryId>,
    shared: &[EntryId],
    tail: EntryId,
) -> Vec<EntryId> {
    let mut at = vec![root];
    for &id in checkpoint.iter().chain(shared).chain([&tail]) {
        if !at.contains(&id) {
            at.push(id);
        }
    }
    if at.len() > 4 {
        at.drain(1..at.len() - 3);
    }
    at
}

/// Fan-out gating: hold a task's request while another task's request on
/// the same model whose context contains this task's head has not had its
/// first byte. Siblings over one cold prefix (forks at one response, fresh
/// tasks on one seed) then pay one cache write between them instead of one
/// each. The provider caches per model, so a request on another model
/// writes nothing this one could read, and is not waited for.
pub fn hold<'a>(
    head: EntryId,
    model: &str,
    cold: impl IntoIterator<Item = (&'a str, &'a HashSet<EntryId>)>,
) -> bool {
    cold.into_iter()
        .any(|(on, context)| on == model && context.contains(&head))
}

/// How long a call has to stop when no task reading it has a deadline (Max
/// interrupted it, or its task is failing): `SIGTERM`, then `SIGKILL` at
/// this.
pub const CALL_GRACE: Duration = Duration::from_secs(10);

/// How long a member has to stop when the task that owns it is failing
/// rather than being stopped, and so has no deadline to share.
pub const MEMBER_STOP: Duration = Duration::from_secs(60);

/// How long Ctrl-C (or `SIGTERM`) gives the requests in flight to finish
/// before they are cut and the brain exits: long enough for nearly any
/// response, since a cut one is paid for and lost. A second signal cuts
/// them now.
pub const DRAIN: Duration = Duration::from_secs(300);

/// How long past a deadline the brain waits for a process it owns to
/// confirm its exit before killing it.
pub const HARD_WAIT: Duration = Duration::from_secs(2);

/// The time to give what an owner whose deadline is `deadline` owns, asked
/// at `now`: three quarters of what it has left, so it keeps the rest to
/// clean up after them; none once less than a second is left.
pub fn share(now: Timestamp, deadline: Timestamp) -> u64 {
    match u64::try_from(deadline.0 - now.0) {
        Ok(left) if left >= 1000 => left / 4 * 3,
        _ => 0,
    }
}

/// A duration as the milliseconds a fact states.
pub fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}
