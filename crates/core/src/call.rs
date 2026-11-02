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
/// [`CallId`] at most once, so shipping a call again is always safe, and
/// that is all crash recovery needs: a limb that has reconnected (after it
/// restarted, or the brain did) is shipped its whole view, which holds a
/// row for each of its unsettled calls ([`crate::state::State::limb_rows`]),
/// and answers each from its own durable record, keeps running it, or,
/// having no record, runs it again if its tool may be rerun or settles it
/// as [`Outcome::Unknown`]. There is no separate recovery message, and the
/// brain does not need to know which tools may be rerun: the limb that owns
/// a tool does.
///
/// Every harness action an agent takes is a call too, to the brain, which
/// is a limb ([`crate::limb::Host::Brain`]): a spawn opens a
/// scope and settles when its last task is done, so a parent waiting on its
/// children is a task with an outstanding call and nothing more; a
/// compaction call compacts its caller's context once its round is over. Their
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
    ///
    /// What the caller's own calls did is not held against it: a call also
    /// expects what the earlier calls of its response observed, since its
    /// caller wrote them to run in order; and a resource last changed while
    /// the caller's own command ran counts as seen.
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
    /// it runs, and its settlement wakes them. A call of a round becomes
    /// background work without changing its caller
    /// ([`CallEvent::Backgrounded`]).
    Background {
        task: TaskId,
        from: CallId,
        /// What the task called it, unique among its owner's running
        /// background work: how the agent, whoever it passes it to, and Max
        /// refer to it.
        name: String,
    },
    /// Max, in a chat with `chat` (his settlement rides into its next
    /// request), or from the tree, outside any context.
    Max { limb: LimbId, chat: Option<TaskId> },
    /// The harness, while running `call`: the attachments of a spawn's shared
    /// brief or of a compaction. Owned by that call, and shown to each
    /// receiving task as its own.
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
    /// Sent to its limb. A plugin's call becomes a row of the limb's view
    /// ([`crate::state::State::limb_rows`]), which a reconnection ships
    /// again, so it is dispatched once; the provider's reading is
    /// dispatched again when an earlier brain was running it.
    Dispatched,
    /// Its limb was asked to stop it within `within` milliseconds of this
    /// fact ([`crate::cancel`]), because every task reading it is stopping
    /// (why is in their [`crate::task::Drain`] facts). Its row in the limb's
    /// view carries the deadline across a reconnection, and a later fact asks
    /// again with less time when a reader's deadline comes sooner; a deadline
    /// is only ever brought forward.
    Stopping {
        within: u64,
    },
    /// Its owner hands it one level up the tree ([`Owner`]), so it can run
    /// on after the owner finishes: a dev server, a browser tab, a long
    /// test run someone will still use. Only a running background call is
    /// passed, by the task that owns it, to the task that owns that one, or
    /// to Max from a task he started. `why` is the owner's reason, shown to
    /// whoever receives it. Passed to Max, it has no time limit from then
    /// on: a limit is for someone waiting on its result, and he keeps it for
    /// its lifetime, until he stops it. Passed to a task, it keeps its limit.
    Passed {
        to: Owner,
        why: String,
    },
    /// Its owner no longer wants it: stop it now ([`crate::cancel`]), with
    /// the brain's grace for a call. Stated for an agent that stops its own
    /// background work, or by Max, for any call.
    Abandon,
    /// Its round no longer waits on it: it runs on as its owner's background
    /// work ([`crate::task::Task::background`]), and its tool use is
    /// answered now with the call's label, so the agent can stop or pass
    /// it. Its whole result comes later, as background work's does: a
    /// message that wakes the agent.
    ///
    /// Stated by the brain for every call of a task's round still
    /// outstanding a grace after Max's words, or a turn one of his acts asks
    /// for, reach it ([`crate::state::Todo::Background`]), so he is heard
    /// while the work goes on; and for a spawn of a round he interrupts, so
    /// his interrupt never stops a scope. Or by Max, for one call he moves
    /// to the background himself. Never for a fresh context's attachments.
    ///
    /// **Concept.** A blocking call blocks the agent, never Max. The
    /// provider needs every tool use answered before anything new is said,
    /// so this is the one way his words reach a model that is waiting:
    /// answer what it waits on now, and keep the work.
    ///
    /// **Vision.** "I desperately need the ability to move stuff to the
    /// background"; "if I send a message, I think it should auto background
    /// anything the agent is waiting on" (2 Nov).
    Backgrounded,
    /// What this call, to the `wait` tool, waits on: stated by the brain as
    /// it dispatches the call, so the fold knows which settlements are read
    /// through it ([`Wait`]).
    Waits(Wait),
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
            | CallEvent::Abandon
            | CallEvent::Backgrounded
            | CallEvent::Waits(_) => {}
            CallEvent::Settled(outcome) => outcome.values(out),
        }
    }
}

/// A wait: a call of a round that settles once some of its owner's
/// background work has, as `until` says.
///
/// **Concept.** Waiting on background work is a call, so that ending a
/// turn keeps one meaning: an autonomous agent's turn end is its result,
/// and a chat's is Max's move. An agent with nothing to do until its work
/// lands calls `wait` and stays in its round. The results come whole in
/// the wait's output and are not delivered again; one that settles while
/// nothing waits on it reaches the agent as a message at its next request
/// ([`crate::task::Input::Background`]). A chat's ended turn waits on its
/// work as well as on Max, so a chat need not call it; one that has only
/// answered him waits here instead, so it is not shown as his move. Max's
/// words settle a wait rather than moving it to the background, at once:
/// an ordinary result saying he wrote and what still runs, and the agent
/// waits again once it has answered him. Its task's stopping settles it as
/// interrupted ([`crate::state::Todo::Settle`]). A wait is never moved to
/// the background, since waiting in the background is not waiting.
///
/// Only the caller's own background work is waited on: a call it started
/// with `background` (a spawn among them), one moved out of its round, or
/// one passed to it. A question to Max, from an agent whose role lets it
/// write one, is a call too, so it is one more.
/// Work it didn't start is read, never waited on, so a circle of waits
/// cannot form. A deadline is a background `sleep` among the targets.
///
/// **Vision.** "Join any versus join all. Take the first result as it
/// comes versus wait for all the results. Any background thing. Tool call,
/// an agent, a question to Max", though "it's not strictly needed for an
/// agent that can pause and ask me questions"; "Not all agents would be,
/// but the ones that can, it would be treated as a background task. [...]
/// be able to write a question"; "for background
/// sub-agents, if they [...] end their turn with their final response,
/// how's the harness supposed to know?" (3 Nov). In the records no agent
/// ever ended its turn to wait: 14 of 25 background jobs were waited on
/// with a foreground `sleep`, and every one of those agents worked on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wait {
    /// The calls it waits on, never empty, each running background work of
    /// the caller's when it is stated. The tool's default is all of it. One
    /// passed on since is no longer waited on.
    pub on: Vec<CallId>,
    pub until: Until,
}

/// When a wait settles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Until {
    /// One of its calls has settled: the first result as it comes. The rest
    /// run on. The tool's default, since a server never settles.
    Any,
    /// Every one of its calls has settled.
    All,
}

impl Until {
    /// Whether a wait holds, with `landed` of its calls settled and `running`
    /// still running for its caller.
    pub fn holds(self, landed: usize, running: usize) -> bool {
        match self {
            Until::Any => landed > 0 || running == 0,
            Until::All => running == 0,
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
        /// The fresh look, when the limb gives one: what reading the
        /// current state returns, which the context then embodies like a
        /// read's result, so a retry written after it goes through. Not
        /// held by the later calls of the same response, which were written
        /// before it was seen.
        shown: Option<Output>,
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

    /// What its caller's context sees of it: what it returned, or what a
    /// refusal showed.
    pub fn seen(&self) -> Option<&Output> {
        match self {
            Outcome::Refused { shown, .. } => shown.as_ref(),
            outcome => outcome.output(),
        }
    }

    fn values(&self, out: &mut Vec<ValueId>) {
        if let Some(Output {
            model,
            full,
            observes: _,
        }) = self.seen()
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
