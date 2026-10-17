//! The brain's policy: every decision the core leaves open, apart from what
//! a request renders and how it is worded ([`crate::render`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use harness_core::context::EntryId;
use harness_core::id::Timestamp;

/// Requests that may fail in a row before the task fails.
pub const GIVE_UP: u32 = 5;

/// How long to wait before resending after `failures` failures in a row,
/// when the provider did not say: 2, 4, 8, 16 seconds.
pub fn backoff_ms(failures: u32) -> i64 {
    1000 << failures.min(5)
}

/// Where a request asks the provider to cache: the end of the root, the end
/// of the context the request extends, the end of a shared brief it appends,
/// and its new tail.
///
/// The middle ones are what make fan-outs cheap. A fork starts on its
/// parent's response, which no parent request ever marked, and fresh
/// siblings append the same brief to the same root, so the first request of
/// the first sibling writes that prefix and the others read it ([`hold`]).
/// The same mark on an ordinary turn re-reads the previous request's prefix.
pub fn breakpoints(root: EntryId, shared: &[EntryId], tail: EntryId) -> Vec<EntryId> {
    let mut at = vec![root];
    for &id in shared.iter().chain([&tail]) {
        if !at.contains(&id) {
            at.push(id);
        }
    }
    at
}

/// Fan-out gating: hold a task's request while another task's request whose
/// context contains this task's head has not had its first byte. Siblings
/// over one cold prefix (forks at one response, fresh tasks on one seed)
/// then pay one cache write between them instead of one each.
pub fn hold<'a>(head: EntryId, cold: impl IntoIterator<Item = &'a HashSet<EntryId>>) -> bool {
    cold.into_iter().any(|context| context.contains(&head))
}

/// A new clone for an agent started from a workspace at `dir`: cloned from
/// what `dir` itself was cloned from (or `dir`, if it has no `origin`), into
/// a sibling directory named for the repository and the branch, the way
/// Max names his (`agent-harness` on `dev-x` is `agent-harness-dev-x`).
pub fn clone_of(dir: &Path, branch: &str) -> (String, PathBuf) {
    let from = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    let repo = from
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("clone")
        .trim_end_matches(".git");
    let name = format!("{repo}-{}", branch.replace('/', "-"));
    let parent = dir.parent().unwrap_or(dir);
    (from, parent.join(name))
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
