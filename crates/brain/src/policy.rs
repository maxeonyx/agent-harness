//! The brain's policy: every decision the core leaves open, apart from what
//! a request renders and how it is worded ([`crate::render`]).

use std::collections::HashSet;

use harness_core::context::EntryId;
use harness_core::model_call::{Breakpoint, Ttl};

/// The model every root is made for.
pub const MODEL: &str = "claude-sonnet-5";

/// The request parameters besides the context, as sent.
pub const PARAMS: &[u8] = br#"{"max_tokens":16000,"stream":true}"#;

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
pub fn breakpoints(root: EntryId, shared: &[EntryId], tail: EntryId) -> Vec<Breakpoint> {
    let mut at = vec![root];
    for &id in shared.iter().chain([&tail]) {
        if !at.contains(&id) {
            at.push(id);
        }
    }
    at.into_iter()
        .map(|at| Breakpoint {
            at,
            ttl: Ttl::Short,
        })
        .collect()
}

/// Fan-out gating: hold a task's request while another task's request whose
/// context contains this task's head has not had its first byte. Siblings
/// over one cold prefix (forks at one response, fresh tasks on one seed)
/// then pay one cache write between them instead of one each.
pub fn hold<'a>(head: EntryId, cold: impl IntoIterator<Item = &'a HashSet<EntryId>>) -> bool {
    cold.into_iter().any(|context| context.contains(&head))
}

/// An agent kind: the role prompt its roots carry.
pub struct Kind {
    pub name: &'static str,
    pub role: &'static str,
}

/// The agent kinds this brain knows.
pub const KINDS: &[Kind] = &[
    Kind {
        name: "orchestrator",
        role: "You are the orchestrator of one piece of work. Split it into parts that other agents can do in parallel, \
start them with `spawn`, and finish the work with their results: check what they did, fix small things yourself, \
and end your turn with a short report. Give each agent a precise mandate: what to build, where, and how to check it. \
Your last message is your result. \
Your shell and file tools start in the workspace directory: use relative paths, and do not search the machine for it.",
    },
    Kind {
        name: "engineer",
        role: "You are an engineer. Build the one task you are given in the workspace, check that it works by running it, \
and end your turn with a short result: what you built, how you checked it, and anything left undone. \
Do only your task; if something outside it is wrong, say so in your result instead of fixing it. \
Your shell and file tools start in the workspace directory: use relative paths, and do not search the machine for it.",
    },
];

pub fn kind(name: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.name == name)
}
