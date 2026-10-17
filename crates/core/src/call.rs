//! Calls: one action in a limb, by an agent, by Max, or by the harness for
//! a call it is running.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, LimbId, TaskId, ToolUseId, ValueId};
use crate::resource::Observation;

/// One action in a limb, as decided. Its life is its [`CallEvent`]s.
///
/// **Concept.** A call is durable from the moment it is decided to the
/// moment its outcome is known: [made](CallEvent::Made) before it is sent,
/// [dispatched](CallEvent::Dispatched) to a limb, and
/// [settled](CallEvent::Settled) by that limb. The limb runs each
/// [`CallId`] at most once, so sending a call again is always safe, and
/// that is all crash recovery needs: a call is sent again whenever its limb
/// has reconnected since it was last sent (after its limb restarted, or the
/// brain did), and the limb answers from its own durable record, keeps
/// running it, or, having no record, runs it again if its tool may be
/// rerun or settles it as [`Outcome::Unknown`]. There is no separate
/// recovery message, and the brain does not need to know which tools may be
/// rerun: the limb that owns a tool does.
///
/// Every harness action an agent takes is a call too, to a limb the brain
/// itself implements ([`crate::environment::LimbKind`]): a spawn opens a
/// scope and settles when its last task is done, so a parent waiting on its
/// children is a task with an outstanding call and nothing more; a handover
/// moves its caller to a new context. Their
/// effects are appended in the same batch as the call's own events, so a
/// crash leaves all of them or none, and a restart never spawns twice.
///
/// The same type serves Max ([`Caller::Max`]): editing a file in a chat
/// is a call to the agent's own edit tool in the same limb. Its settlement
/// rides into the chat's next request as what Max did and what came back,
/// and the versions it touched become what that context has observed.
///
/// **Vision.** The user turn ("contribute to a chat the same way as the
/// agent can"); structured concurrency (spawn is a call, so blocking,
/// cancellation and results reuse call machinery); restart ("killed
/// mid-tool-call"), with the unknown outcome kept distinct from failure
/// (critique §5); the shared-workspace race, where a limb can see it coming
/// ([`Call::expects`]); the throbber's "running a tool, where, since when".
///
/// **Existing harnesses.** After a crash mid-tool, Oh My Pi gives the model
/// a synthetic result saying the outcome is unknown
/// (`ref-oh-my-pi/packages/coding-agent/src/session/exit-diagnostics.ts:287`);
/// Codex injects "aborted" at prompt build without persisting it
/// (`ref-codex/codex-rs/core/src/context_manager/normalize.rs:21`); Pi
/// inserts "No result provided"
/// (`ref-pi/packages/ai/src/api/transform-messages.ts:178`); Goose
/// re-dispatches the unanswered call
/// (`ref-goose/crates/goose/src/agents/state_machine/ops_toolcalling.rs:1015`),
/// which reruns one that must not be. None asks a surviving executor what
/// happened, and none lets a tool's author say whether a rerun is safe. Pi
/// and Codex record a shell command Max runs from the prompt as its own kind
/// of context (`ref-pi/packages/coding-agent/src/core/messages.ts:28`,
/// `ref-codex/codex-rs/core/src/context/user_shell_command.rs:30`); here
/// every tool a limb offers Max works that way.
///
/// **Different here.** Delivery is at-least-once and execution at-most-once
/// per id, owned by the executor; the record of what was asked and what came
/// back is the brain's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub caller: Caller,
    pub tool: String,
    /// The tool's input exactly as written: the `tool_use` block's `input`.
    pub input: ValueId,
    /// The versions the caller's view was based on: its context's latest
    /// observation of each resource the limb owns (for Max, what his face
    /// last showed him). A limb checks those its call names, which covers
    /// the tools whose effects it can know in advance (an edit, a write),
    /// and refuses the call when they have moved. A resource the call names
    /// that is absent here is one the caller has never seen, so a write to
    /// it that would replace something is refused too ("read it first").
    /// A command whose effects the limb cannot know (`bash`, `git`) is not
    /// checked; its writes reach other contexts afterwards, as notices
    /// naming the call.
    pub expects: Vec<Observation>,
}

impl Call {
    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        let Call {
            caller: _,
            tool: _,
            input,
            expects: _,
        } = self;
        out.push(*input);
    }
}

/// Who made a call, which decides who owns it, whose context its result
/// reaches, and how its limb is found.
///
/// Owner and audience are what matter. An agent's call belongs to its task
/// and answers its tool use. Max's call in an engineer's chat belongs to
/// Max and only rides into the engineer's next request: the engineer can
/// finish while Max's file is open, and stopping the engineer never stops
/// Max's edit.
///
/// An agent names only a tool: which limb runs it is its task's binding,
/// the same list its root's tools were made from, so a tool use cannot
/// reach a limb its context was not given. Max and the harness name the limb,
/// because they act from outside any one context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Caller {
    /// A task, answering `tool_use` of the response in flight: the task's
    /// next request waits for it.
    Agent { task: TaskId, tool_use: ToolUseId },
    /// Background work a task started with its own call `from` to a meta
    /// tool, under `name`: whoever owns it ([`Owner`]) cannot finish while
    /// it runs, and its settlement wakes them.
    Background {
        task: TaskId,
        from: CallId,
        /// What the task called it, unique among its owner's running
        /// background work: how the agent, whoever it passes it to, and Max
        /// refer to it.
        #[serde(default)]
        name: String,
    },
    /// Max, in a chat with `chat` (his settlement rides into its next
    /// request), or from the tree, outside any context.
    Max { limb: LimbId, chat: Option<TaskId> },
    /// The harness, while running `call`: the attachments of a spawn's shared
    /// brief or of a handover. Owned by that call, and shown to each
    /// receiving task as a tool call it made.
    Meta { call: CallId, limb: LimbId },
}

/// Who a call is for now: who reads its outcome, who may stop it or pass it
/// on, and who cannot finish while it runs.
///
/// **Concept.** Every call has exactly one owner, and ownership follows the
/// task tree: a call is first its caller's (Max's own calls are his), and
/// only a running background call ever moves, one level up at a time, by
/// its owner's own choice ([`CallEvent::Passed`]). So nothing runs that
/// nobody owns, an owner that is abandoned takes everything it owns with
/// it, and work outlives the agent that started it only because an agent
/// said so.
///
/// **Vision.** Background work at a stop: an agent is never forced to kill
/// its background work, and a resource (a dev server, a browser tab)
/// becomes Max's obligation only when an agent he started explicitly hands
/// it to him. Ownership is not tool access: a parent can own a tab without
/// browser tools, and stop it all the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Owner {
    Task(TaskId),
    Max,
}

/// A fact in a call's life.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallEvent {
    Made(Call),
    /// Sent to its limb's current connection. Sent again, as another
    /// dispatch, after every reconnection while it is unsettled.
    Dispatched,
    /// Its limb was asked to stop it within `within` milliseconds of this
    /// fact ([`crate::cancel`]), because every task reading it is stopping
    /// (why is in their [`crate::task::Drain`] facts). Asked again after a
    /// reconnection, like a dispatch, and again with less time when a reader's
    /// deadline comes sooner; a deadline is only ever brought forward.
    Stopping {
        within: u64,
    },
    /// Its owner hands it one level up the tree ([`Owner`]), so it can run
    /// on after the owner finishes: a dev server, a browser tab, a long
    /// test run someone will still use. Only a running background call is
    /// passed, by the task that owns it, to the task that owns that one, or
    /// to Max from a task he started. `why` is the owner's reason, shown to
    /// whoever receives it.
    Passed {
        to: Owner,
        why: String,
    },
    /// Its owner no longer wants it: stop it now ([`crate::cancel`]), with
    /// the brain's grace for a call. Stated for an agent that stops its own
    /// background work, or by Max, for any call.
    Abandon,
    /// How it ended, stated by the limb that ran it (or by the brain, for a
    /// call that never reached one). A second settlement is accepted only
    /// after [`Outcome::Unknown`]: the real outcome of a remote command that
    /// finished while nobody could hear it. The first stands; the caller is
    /// told the second.
    Settled(Outcome),
}

impl CallEvent {
    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        match self {
            CallEvent::Made(call) => call.values(out),
            CallEvent::Dispatched
            | CallEvent::Stopping { .. }
            | CallEvent::Passed { .. }
            | CallEvent::Abandon => {}
            CallEvent::Settled(outcome) => outcome.values(out),
        }
    }
}

/// How a call ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Completed(Output),
    /// The tool ran and reported failure: a failing command, a timeout, a
    /// tool name its context does not have.
    Failed(Output),
    /// The limb did nothing, because what the caller expected has moved.
    /// Guaranteed to have had no effect, so trying again after a fresh look
    /// is always safe.
    Refused {
        current: Vec<Observation>,
    },
    /// Stopped while running, because it was asked or because its limb gave
    /// up on it (the output says which); it may have had partial effects.
    Interrupted {
        partial: Option<Output>,
    },
    /// Nobody knows whether it ran, and its tool does not allow a rerun. The
    /// agent is told so and decides what to check.
    Unknown,
}

impl Outcome {
    /// What it returned, if anything.
    pub fn output(&self) -> Option<&Output> {
        match self {
            Outcome::Completed(output)
            | Outcome::Failed(output)
            | Outcome::Interrupted {
                partial: Some(output),
            } => Some(output),
            Outcome::Refused { .. } | Outcome::Interrupted { partial: None } | Outcome::Unknown => {
                None
            }
        }
    }

    fn values(&self, out: &mut Vec<ValueId>) {
        if let Some(Output {
            model,
            full,
            observes: _,
        }) = self.output()
        {
            out.push(*model);
            out.extend(*full);
        }
    }
}

/// What a call returned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    /// The content the model sees, already shaped for its context: a tool
    /// result's content, or for Max's actions the transcript of what he did.
    /// For a spawn, rendered from its children's outcomes when it settles.
    pub model: ValueId,
    /// The full output when `model` is a cut-down view of it, so Max and
    /// later reads can see everything. Sent by the limb with the settlement,
    /// into the one value store, so it is readable wherever the limb ran.
    pub full: Option<ValueId>,
    /// The versions this result embodies: the file version a read returned,
    /// the version a write produced, the skill version a load returned. A
    /// later change to any of them reaches the context as a notice.
    pub observes: Vec<Observation>,
}
