//! Model calls: one request to the provider, and what it cost.

use serde::{Deserialize, Serialize};

use crate::context::{Entry, EntryId, Message, Rendered, Role};
use crate::id::{TaskId, Timestamp, ValueId};

/// One request to the provider, as sent.
///
/// **Concept.** The honest account of a nondeterministic step, from which
/// everything about cost and cache is derived rather than tracked: which
/// context it sent (its [`base`](ModelCall::base) and the messages it
/// [appended](ModelCall::appends), whose last is its [head](ModelCall::head)),
/// what those messages were rendered from ([`ModelCall::renders`]), where it
/// asked the provider to cache, when the response began and ended, how it
/// ended, and what the provider reported. Every turn, retry, keep-alive and
/// handover turn is one.
///
/// A request names its base and carries its new messages, so its head is
/// computed, never stated: it cannot claim a context that does not grow
/// from the one its task has, and the fold learns the new entries from it.
///
/// It is Bx's build record. The task's environment and binding are the
/// definition, the head's [`EntryId`] is the resolved input's content
/// identity, the response is the output, and timing, usage and quota are
/// the execution evidence. Several model calls on one head (a retry, a
/// rewind, a resend after a crash) expose the model's nondeterminism instead
/// of hiding it, as one Bx definition with several records does. Unlike Bx,
/// the output can never be re-derived, so it *is* the history; and the
/// provider's cache is a store the harness neither owns nor can address, so
/// warmth is inferred from these records, not looked up.
///
/// **Provider wire** (spike, 2026-10-16, on the subscription):
/// - Only the parent's breakpoints write cache entries a child can read. A
///   child needs no breakpoint at the fork point: a breakpoint on its own
///   tail finds the parent's entry by lookback. So the fork point must be
///   marked on the parent's request, before anyone knows it will be forked;
///   placing breakpoints at the end of the root, at the latest branch point
///   on the head's chain, and at the tail covers it.
/// - An entry becomes readable when the writing response's `message_start`
///   arrives ([`ModelCallEvent::FirstByte`]). Requests sent before that all
///   miss and each pays a full write, so N siblings over a cold prefix are
///   sent one first, then the rest at its first byte.
/// - Prefixes under about 1k tokens are not cached, silently.
/// - Every response reports the plan's 5h and 7d utilisation, and a billing
///   claim; any claim other than a plan window is a failure ([`Quota`]).
///
/// **Vision.** Cache-aware forking; keep-alive vs cache-expiry handover as
/// an expected-value decision (critique §4); analytics and timesheets; the
/// throbber's "sent, no first byte yet" vs "streaming"; restart (a request
/// sent by an earlier brain with no end was lost and is sent again, since
/// nothing it produced was appended); the subscription's usage window.
///
/// **Existing harnesses.** Oh My Pi's cache warmer refreshes a cache
/// shortly before expiry when the continuation probability times the miss
/// cost, minus the refresh cost, clears five cents
/// (`ref-oh-my-pi/packages/coding-agent/src/session/cache-warmer.ts:216`),
/// per session; Pi refreshes at 90% of the TTL
/// (`ref-pi/packages/coding-agent/src/core/cache-warmer.ts:27`); OpenCode
/// and Goose place breakpoints and count usage afterwards. Here the prefix,
/// not the session, is the thing that is warm, so the same decision sees
/// every task sharing it, and has two more options: hand over now, or let a
/// fork's reads keep it warm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCall {
    pub task: TaskId,
    pub purpose: Purpose,
    /// The context this request extends: its task's head for a turn, an
    /// entry on the task's chain for a keep-alive.
    pub base: EntryId,
    /// The messages this request appended to `base`, in order: the
    /// attachments' tool uses the harness writes, tool results, Max's words,
    /// notes. Empty for a retry of the same head and for a keep-alive.
    pub appends: Vec<Message>,
    /// What `appends` was rendered from, in order. A task has seen exactly
    /// what its requests rendered, so a request retried after a rate limit
    /// renders what arrived meanwhile, and nothing depends on journal
    /// positions.
    pub renders: Vec<Rendered>,
    /// The request's other parameters exactly as sent: `max_tokens`,
    /// thinking, effort, `stream`.
    pub params: ValueId,
    pub breakpoints: Vec<Breakpoint>,
}

impl ModelCall {
    /// The last entry sent: the identity of the whole context.
    pub fn head(&self) -> EntryId {
        self.appends
            .iter()
            .fold(self.base, |parent, message| message.id_after(parent))
    }

    /// The entries this request appended, each with its id, in order.
    pub fn entries(&self) -> Vec<(EntryId, Entry)> {
        let mut parent = self.base;
        self.appends
            .iter()
            .map(|message| {
                let id = message.id_after(parent);
                let entry = Entry::Message {
                    parent,
                    message: message.clone(),
                };
                parent = id;
                (id, entry)
            })
            .collect()
    }

    /// The entry a response with `content` is: an assistant message after
    /// the head.
    pub fn response(&self, content: ValueId) -> Entry {
        Entry::Message {
            parent: self.head(),
            message: Message {
                role: Role::Assistant,
                content,
            },
        }
    }

    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        let ModelCall {
            task: _,
            purpose: _,
            base: _,
            appends,
            renders: _,
            params,
            breakpoints: _,
        } = self;
        out.extend(appends.iter().map(|m| m.content));
        out.push(*params);
    }
}

/// Why a request was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    /// A model turn for the task. At most one is in flight per task.
    Turn,
    /// A zero-output request that only refreshes the cache's lifetime. Its
    /// base may be behind the task's head: while a tool call is outstanding,
    /// the only valid keep-alive is the previous request again. It appends
    /// nothing and never moves the task.
    KeepAlive,
}

/// A cache breakpoint the request placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    /// The prefix through this entry is cached. It is on the request's chain.
    pub at: EntryId,
    pub ttl: Ttl,
}

/// How long the provider keeps a cache entry after its last use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ttl {
    /// Five minutes; writes cost 1.25x input.
    Short,
    /// One hour; writes cost 2x input.
    Long,
}

/// A fact in a model call's life.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModelCallEvent {
    /// The request was sent. A [`Purpose::Turn`] moves its task's head to
    /// the request's head and clears what it rendered from the task's
    /// pending inputs.
    Sent(ModelCall),
    /// The response's `message_start` arrived: from here a cache entry this
    /// request wrote is readable by others.
    FirstByte,
    Ended {
        ending: Ending,
        /// What the provider reported, when it reported anything. An
        /// abandoned stream still counts what it had used.
        usage: Option<Usage>,
        quota: Option<Quota>,
    },
}

impl ModelCallEvent {
    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        match self {
            ModelCallEvent::Sent(call) => call.values(out),
            ModelCallEvent::FirstByte => {}
            ModelCallEvent::Ended {
                ending,
                usage: _,
                quota: _,
            } => match ending {
                Ending::Responded { content, stop: _ } => out.push(*content),
                Ending::Abandoned { partial } => out.extend(*partial),
                Ending::Failed { .. } | Ending::Limited { .. } | Ending::Lost => {}
            },
        }
    }
}

/// How a request ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Ending {
    /// The response's `content`, which is the assistant entry after the
    /// request's head ([`ModelCall::response`]). Its tool uses are the calls
    /// made for its task while the request was in flight, in the order they
    /// were made (in the same batch as this, so a crash mid-stream leaves no
    /// effect of a response the model never finished).
    Responded { content: ValueId, stop: Stop },
    /// The provider refused or failed this request. `retry` when it said
    /// when; that is about this request, and how long to wait before sending
    /// its task's next one is the brain's policy.
    Failed {
        status: Option<u16>,
        message: String,
        retry: Option<Timestamp>,
    },
    /// The subscription's limit refused it: every task on the account waits
    /// until `until`. Not the task's failure, so it never counts towards
    /// giving up on it.
    Limited { until: Timestamp },
    /// The harness stopped reading: Max interrupted, or the task is
    /// stopping. Nothing was kept as the model's; the visible text Max may
    /// have been reacting to is kept here and quoted to the model next.
    Abandoned { partial: Option<ValueId> },
    /// Sent by an earlier brain, which ended before it did. Nothing from it
    /// was kept.
    Lost,
}

/// Why the model stopped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    EndTurn,
    ToolUse,
    /// The output limit; hidden thinking counts against it.
    MaxTokens,
    PauseTurn,
    Refusal,
    /// The context no longer fits the model's window.
    ContextWindow,
    Other(String),
}

/// The provider's usage report, in tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write_short: u64,
    pub cache_write_long: u64,
    pub output: u64,
}

/// The subscription's state as one response reported it: a free usage gauge
/// on every response, and the billing check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    /// `anthropic-ratelimit-unified-representative-claim`: `five_hour` and
    /// `seven_day*` are the plan; anything else (`overage`) is billing gone
    /// wrong, and the brain stops sending.
    pub claim: Option<String>,
    /// Fraction of the 5-hour window used, 0 to 1.
    pub five_hour: Option<f64>,
    /// Fraction of the 7-day window used, 0 to 1.
    pub seven_day: Option<f64>,
    /// When the 5-hour window resets.
    pub five_hour_reset: Option<Timestamp>,
    /// When the 7-day window resets.
    pub seven_day_reset: Option<Timestamp>,
}
