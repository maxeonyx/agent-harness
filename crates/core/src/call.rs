//! Calls: one action in a limb, by an agent, by Max, or by the harness for
//! a call it is running.

use serde::{Deserialize, Serialize};

use crate::context::Wire;
use crate::id::{CallId, ContentId, InstanceId, LimbId, TaskId, ToolUseId};
use crate::journal::Rejected;
use crate::resource::Observation;

/// One action in a limb, as decided. Its life is its [`CallEvent`]s.
///
/// **Concept.** A call is durable from the moment it is decided to the
/// moment its outcome is known: [made](CallEvent::Made) before it is sent,
/// [dispatched](CallEvent::Dispatched) to a limb instance, and
/// [settled](CallEvent::Settled). That is all crash recovery needs. A made
/// call is sent; a dispatched call is asked about (the limb answers from its
/// own durable record, or says it has none, and then the call is sent again
/// if its tool allows a rerun, or settles as [`Outcome::Unknown`]); a
/// settled call is history. Every attempt is a recorded dispatch, so a rerun
/// never hides the first.
///
/// Every harness action an agent takes is a call too, to a limb the brain
/// itself implements ([`crate::environment::LimbKind`]): a spawn opens a
/// scope and settles when its last task is done, so a parent waiting on its
/// children is a task with an outstanding call and nothing more; a handover
/// moves its caller to a new context; an ask is a call Max answers. Their
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
/// happened, and none lets a tool's author say whether a rerun is safe
/// ([`crate::environment::Tool::rerun`]). Pi and Codex record a shell
/// command Max runs from the prompt as its own kind of context
/// (`ref-pi/packages/coding-agent/src/core/messages.ts:28`,
/// `ref-codex/codex-rs/core/src/context/user_shell_command.rs:30`); here
/// every tool a limb offers Max works that way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub caller: Caller,
    pub limb: LimbId,
    pub tool: String,
    pub input: Wire,
    /// The versions the caller's view was based on: its context's latest
    /// observation of each resource this limb owns (for Max, what his face
    /// last showed him). A limb checks those its call names, which covers
    /// the tools whose effects it can know in advance (an edit, a write),
    /// and refuses the call when they have moved. A command whose effects it
    /// cannot know (`bash`, `git`) is not checked; its writes reach other
    /// contexts afterwards, as notices naming the writer.
    pub expects: Vec<Observation>,
}

/// Who made a call, which decides who owns it and whose context its result
/// reaches.
///
/// Owner and audience are what matter. An agent's call belongs to its task
/// and answers its tool use. Max's call in an engineer's chat belongs to
/// Max and only rides into the engineer's next request: the engineer can
/// finish while Max's file is open, and stopping the engineer never stops
/// Max's edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Caller {
    /// A task. With `tool_use`, the call answers that tool use and the task's
    /// next request waits for it. Without, it is background work the task
    /// started through a meta tool: the task cannot finish while it runs,
    /// and its settlement wakes the task.
    Agent {
        task: TaskId,
        tool_use: Option<ToolUseId>,
    },
    /// Max, in a chat with `chat` (his settlement rides into its next
    /// request), or from the tree, outside any context.
    Max { chat: Option<TaskId> },
    /// The harness, while running `call`: the attachments of a spawn's shared
    /// brief or of a handover. Owned by that call, and shown to each
    /// receiving task as a tool call it made.
    Meta { call: CallId },
}

/// A fact in a call's life.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallEvent {
    Made(Call),
    /// Sent to a limb instance. A second dispatch is a rerun.
    Dispatched {
        instance: InstanceId,
    },
    Settled(Outcome),
    /// The real outcome of a call that settled as [`Outcome::Unknown`]: a
    /// remote command that finished while the brain was down. The earlier
    /// settlement stands; the caller is told.
    Late(Outcome),
}

/// Where a call is in its life: the fold of its events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Made, not yet sent.
    Made,
    Dispatched {
        instance: InstanceId,
        attempt: u32,
    },
    Settled {
        outcome: Outcome,
        late: Option<Outcome>,
    },
}

impl Status {
    /// Fold one event, or refuse it: a settled call never moves again, and
    /// only an unknown outcome can be learned late.
    pub fn apply(&self, event: &CallEvent) -> Result<Status, Rejected> {
        Ok(match (self, event) {
            (Status::Made, CallEvent::Dispatched { instance }) => Status::Dispatched {
                instance: *instance,
                attempt: 1,
            },
            (Status::Dispatched { attempt, .. }, CallEvent::Dispatched { instance }) => {
                Status::Dispatched {
                    instance: *instance,
                    attempt: attempt + 1,
                }
            }
            (Status::Made | Status::Dispatched { .. }, CallEvent::Settled(outcome)) => {
                Status::Settled {
                    outcome: outcome.clone(),
                    late: None,
                }
            }
            (
                Status::Settled {
                    outcome: Outcome::Unknown,
                    late: None,
                },
                CallEvent::Late(late),
            ) => Status::Settled {
                outcome: Outcome::Unknown,
                late: Some(late.clone()),
            },
            (Status::Settled { .. }, _) => return Err(Rejected::Finished),
            _ => return Err(Rejected::Busy),
        })
    }

    pub fn is_settled(&self) -> bool {
        matches!(self, Status::Settled { .. })
    }
}

/// How a call ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Completed(Output),
    /// The tool ran and reported failure (a failing command).
    Failed(Output),
    /// The limb did nothing, because what the caller expected has moved.
    /// Guaranteed to have had no effect, so trying again after a fresh look
    /// is always safe.
    Refused {
        current: Vec<Observation>,
    },
    /// Stopped while running; it may have had partial effects.
    Interrupted {
        by: Interruption,
        partial: Option<Output>,
    },
    /// Nobody knows whether it ran, and its tool does not allow a rerun. The
    /// agent is told so and decides what to check.
    Unknown,
}

/// What a call returned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    /// The content the model sees, already shaped for its context: a tool
    /// result's content, or for Max's actions the transcript of what he did.
    /// For a spawn, rendered from its children's outcomes when it settles.
    pub model: Wire,
    /// The full output when `model` is a cut-down view of it, so Max and
    /// later reads can see everything.
    pub full: Option<ContentId>,
    /// The versions this result embodies: the file version a read returned,
    /// the version a write produced, the skill version a load returned. A
    /// later change to any of them reaches the context as a notice.
    pub observes: Vec<Observation>,
}

/// Why a call was stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interruption {
    /// Max interrupted the turn.
    Max,
    /// Its owner is stopping.
    Stop,
    /// The limb ran out of the time it allows a call.
    Timeout,
    /// The limb lost the brain for longer than it waits, and stopped its
    /// calls rather than leave them running unowned.
    Disconnected,
}
