//! Policy: every decision the core leaves open, apart from what a request
//! renders and how it is worded ([`crate::render`]).

use std::collections::HashSet;
use std::time::Duration;

use harness_core::context::{Entry, EntryId};
use harness_core::id::Timestamp;
use harness_core::resource::ResourceKey;
use harness_core::state::State;
use harness_core::task::Task;

/// The soft limit of context in a window of `window`, past which each
/// request tells the agent to compact now: 300k, or 80% of a smaller window,
/// before the provider refuses it and the harness compacts it with no
/// summary of the agent's.
pub fn limit(window: u64) -> u64 {
    (window * 80 / 100).min(300_000)
}

/// How many tokens of context, in a window of `window`, make each request
/// offer the agent a compaction in its own time: the last 50k before the
/// [`limit`].
pub fn offer_at(window: u64) -> u64 {
    limit(window).saturating_sub(50_000)
}

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

/// Why a task's next compaction starts a new root instead of keeping its
/// latest checkpoint.
///
/// A new root costs about what compacting onto the checkpoint costs: the
/// new context is the root, which every agent of the role shares in the
/// cache, then the mandate, Max's words and the carry, written once. So the
/// harness re-roots whenever keeping the checkpoints buys nothing: the root
/// no longer says what the agent is, or what they keep is already long. A
/// busy agent's cache never goes cold, so its compaction is where a change
/// to its role reaches it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reroot {
    /// Its role, its tool sets' or its directory's instructions, or its
    /// model changed since its root was rendered.
    Stale,
    /// What its checkpoints keep is past [`keep_at`].
    Long,
}

/// The most context a compaction keeps: half the [`limit`], so a context
/// compacted onto its checkpoint has room to work before the next one.
pub fn keep_at(window: u64) -> u64 {
    limit(window) / 2
}

/// Whether a resource is one a root embodies, so only a new root takes up
/// its change.
pub fn rooted(resource: &ResourceKey) -> bool {
    resource.kind == "role" || resource.kind == "instructions"
}

/// Why `task`'s next compaction starts a new root, if it does, for a model
/// whose window is `window`.
pub fn reroot(state: &State, task: &Task, window: u64) -> Option<Reroot> {
    let context = task.context()?;
    let entries = state.context(context.head);
    let model = state.role(task.role).map(|role| task.tuning.model(role));
    let remodelled = match entries.first() {
        Some((_, Entry::Root(root))) => model.is_some_and(|m| m != root.model),
        _ => false,
    };
    let moved = context.knows.iter().any(|(resource, known)| {
        rooted(resource)
            && state
                .resource(resource)
                .is_some_and(|now| now.version != known.has)
    });
    if remodelled || moved {
        return Some(Reroot::Stale);
    }
    // The first request whose prompt reached the checkpoint measures what
    // compacting onto it keeps.
    let checkpoint = state.checkpoint(task)?;
    let kept = (entries.iter())
        .skip_while(|(id, _)| *id != checkpoint)
        .find_map(|(id, _)| {
            [state.request(*id), state.response(*id)]
                .into_iter()
                .flatten()
                .find_map(|r| r.ended.as_ref()?.usage.as_ref())
        })?;
    let kept = kept.input + kept.cache_read + kept.cache_write_short + kept.cache_write_long;
    (kept > keep_at(window)).then_some(Reroot::Long)
}
