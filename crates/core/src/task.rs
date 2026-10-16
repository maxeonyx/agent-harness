//! Tasks: anything asynchronous the harness starts and must see end, and
//! the one protocol by which every task ends early.

use serde::{Deserialize, Serialize};

use crate::id::{AgentId, CallId, ModelCallId};

/// Any asynchronous thing the harness runs: an agent, a request to the
/// model, a call to a tool.
///
/// **Concept.** "Task" names what the three share; "agent" names the agent.
/// Each kind keeps its own identity and facts, because what they record
/// differs entirely (a context, a response, a command's output), but every
/// task has an [`Owner`] that must see it end, may own tasks of its own, and
/// ends early by one protocol in four steps:
///
/// 1. **Request cancel.** For an agent, a fact of its own
///    ([`crate::agent::AgentEvent::Cancel`]), decided by its own fold, saying
///    who cancelled it and whether it gets a tidy-up turn. For a request or
///    a call, derived from the agent that reads it, never stated twice.
/// 2. **Cascade.** The cancel reaches everything the task owns
///    ([`crate::state::State::owned`]): an agent's request in flight, its
///    round's calls, its background calls, and through each spawn call the
///    members of its scope, who are cancelled in turn by their owner
///    ([`crate::agent::Canceller::Owner`]).
/// 3. **Clean up**, by kind. A request stops being read, keeping what Max
///    saw ([`crate::model_call::Ending::Abandoned`]). A call's limb is asked
///    to stop it ([`crate::call::CallEvent::Stopping`]) and settles it as
///    interrupted, or the brain gives up on an unreached limb with
///    [`crate::call::Outcome::Unknown`]. A spawn call waits for its members.
///    An agent waits for everything it owns to exit and then, unless the
///    cancel said otherwise, takes a tidy-up turn
///    ([`crate::delivery::Prompt::TidyUp`]): it may use its tools to leave
///    its work where someone can pick it up, may start nothing, and what it
///    says is its report.
/// 4. **Confirm exit.** A task has exited only when its own end is a fact
///    (a request's ending, a call's settlement, an agent's outcome), and an
///    owner only after everything it owns has. Its [`Exit`] says how, in
///    terms every task shares.
///
/// Children exit before their owner tidies up, so an orchestrator's report
/// can say what its engineers managed. No task ends with work it owns still
/// running: an agent cannot finish while its background calls run, unless it
/// hands them to its own owner ([`crate::call::CallEvent::Handed`]), which
/// then cannot finish while they run.
///
/// **Vision.** Structured concurrency at every level ("request cancel,
/// clean up, cascade to subtasks, confirm exit"); the agent's "please tidy
/// up now" turn; background tasks and agents that are "the same kind of
/// thing", with ownership passed to the parent only explicitly; the map
/// Max watches, where background work hangs off the agent that owns it and
/// each node's end reads the same way; the throbber's "stopping".
///
/// **Existing systems.** Trio's nurseries (a scope closes only when every
/// child has exited) and asupersync's regions (cancel is a request; a region
/// closes to quiescence before it finalises; Running, CancelRequested,
/// Cancelling, Finalizing, Completed). Here the finaliser of an agent is a
/// model turn, and a member's failure is information for its owner rather
/// than a reason to cancel its siblings. Of the agent harnesses, Codex and
/// OpenCode abort a turn by a signal that drops the tool calls in flight, and
/// none gives an agent a turn to clean up or lets background work change
/// owner.
///
/// **Bx.** Not shared: a Bx build is pure, so stopping one is discarding
/// it. A task here acts on the world (it spends tokens, runs commands, edits
/// files), which is why ending one is a protocol with a cleanup step rather
/// than a discard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Task {
    Agent(AgentId),
    Request(ModelCallId),
    Call(CallId),
}

/// Who owns a task: who must see it end, and whose cancel reaches it.
///
/// An agent is owned by the agent whose spawn call opened its scope, or by
/// Max in his own scope ([`crate::state::State::owner`]). A request is owned
/// by its agent. A call is owned by its caller (Max's calls, even in an
/// agent's chat, are his), or for the harness's attachments by the call
/// they run for, until its owner hands it on. Ownership changes only by an
/// explicit hand-off, and only up the tree: work an agent hands on becomes
/// its owner's, so nothing becomes nobody's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Owner {
    /// Max: nothing waits on it, and he sees it on his map and can stop it.
    Max,
    Agent(AgentId),
}

/// Where a task is in the protocol every task shares
/// ([`crate::state::State::status`]): what the map draws on each node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// Asked to end early, and not yet ended: cleaning up, or waiting for
    /// what it owns.
    Cancelling,
    Exited(Exit),
}

/// How a task ended, in the terms every task shares. Each kind's own
/// outcome says more ([`crate::agent::Outcome`], [`crate::call::Outcome`],
/// [`crate::model_call::Ending`]); this is what an owner, the map and a
/// day's summary read across kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exit {
    /// It did what it was for.
    Ok,
    /// It could not: a failing command, a provider error, a refused write,
    /// an agent the harness could not carry on.
    Failed,
    /// It was ended early, by Max or by its owner ending.
    Cancelled,
    /// Nobody knows whether it ran: a call whose limb was lost.
    Unknown,
}
