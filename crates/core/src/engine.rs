//! Engines: who runs a task's agent loop. The brain runs its own; a machine
//! limb can run Claude Code's, and the harness hosts that session as a task
//! like any other.
//!
//! **Concept.** A Claude Code session is a [`crate::task::Task`]: one node on
//! Max's map, one chat, one place in his queue, the same mandate, scope,
//! mode, pause, drains and result. What differs is who runs its loop. For a
//! task whose role holds the [`CLAUDE_CODE`] set, Claude Code sends the
//! requests and runs its own tools, in the task's working directory on the
//! limb that offers the set. The brain keeps the task's life; the limb keeps
//! the process and is its voice:
//!
//! - A **run** is one `claude -p` process: a [`crate::call::Call`] whose
//!   caller is [`crate::call::Caller::Engine`] and whose input is a
//!   [`Launch`]. The brain makes one when the task has work and no run, and
//!   stops it when the task is parked, done or abandoned, or has rested for
//!   the offer's [`ClaudeCodeOffer::idle`] minutes; the next input resumes
//!   the session. So the call machinery (dispatch, a stop with a deadline,
//!   settling as unknown when the limb is lost, the task's systemd slice)
//!   is the process machinery.
//! - What the brain wants the run to take now (Max's words, the answers to
//!   the harness tools it called, a hold, an interrupt) is one row of the
//!   limb's view, [`Hosted`], which it applies at most once per id, like
//!   every row.
//! - The limb parses the run's stream into the core's own facts, stated as
//!   its own: requests ([`crate::model_call::ModelCall`], with [`Mirrored`]),
//!   tool calls and their settlements, [`crate::task::Move::Compacted`], and
//!   [`crate::live::Live`] text as it streams. It may state them for a task
//!   only while that task's run is dispatched to it and unsettled. So the
//!   map, the chat, the throbber, Max's queue, the usage gauges, search and
//!   restart serve both engines through one path.
//! - Every permission prompt Claude Code would raise is answered at once,
//!   on the limb, from Max's grants, or denied in band
//!   ([`crate::grant`]). Nothing ever waits on him silently.
//!
//! It is second-class in what it cannot do: the harness neither compacts its
//! context nor places its cache breakpoints, its root holds only what the
//! harness hands Claude Code (Claude Code's own system prompt and tools are
//! its own), and its requests are recorded as Claude Code reports them, not
//! as the bytes sent.
//!
//! What Claude Code says, and what it becomes:
//!
//! | Claude Code (`--output-format stream-json`) | The core |
//! |---|---|
//! | `system/init` | the run's live progress (version, model, tools, MCP servers) |
//! | `system/status` requesting, with the uuids of the user messages it takes | [`crate::model_call::ModelCallEvent::Sent`]: its `renders` are the [`Feed`]s with those uuids and the settled tool results |
//! | `stream_event` `message_start`, then deltas | `FirstByte`, then [`crate::live::Live`] |
//! | `assistant` blocks to `message_stop` | `Ended { Responded }` with usage, and a call `Made` for each tool use, in one batch |
//! | a built-in tool's `user` `tool_result` | the call `Dispatched` to this limb and `Settled` |
//! | `permission_denied`, or a `can_use_tool` the limb denied | `Settled` as [`crate::call::Outcome::Denied`] |
//! | `task_started` backgrounded, then `task_notification` | a background call, then its settlement, which wakes the task as any background result does |
//! | lines with a `parent_tool_use_id` (its own subagents) | live progress of the `Task` call: a subagent is that tool's business |
//! | `compact_boundary` | [`crate::task::Move::Compacted`] |
//! | `rate_limit_event`; a usage-limit result | `Quota` on `Ended`; [`crate::model_call::Ending::Limited`] |
//! | `control_request` `mcp_message` to the harness's server | a call `Made` for the harness tool, routed by the task's binding like any other |
//! | `control_request` `can_use_tool` | answered by the limb from its grant rows ([`crate::grant`]) |
//! | `result` | nothing more: the turn ended with the last response's stop |
//! | anything else | kept in the run's stream file, not parsed |
//!
//! What the brain wants, and what the limb does:
//!
//! | The core | Claude Code |
//! |---|---|
//! | a [`Feed`] in [`Hosted::feeds`] | a stream-json user message with the feed's uuid, written once |
//! | a [`Reply`] | the `mcp_message` response for that call |
//! | [`Hosted::cut`] (Max's interrupt, a cut request) | control `interrupt` |
//! | [`Hosted::hold`] ([`crate::task::Pause::Now`]) | tool uses held at the run's own `PreToolUse` hook callback, feeds not written |
//! | Park, Resume, Tidy, Done prompts | feeds like any other |
//! | the run's call stopping | control `end_session`; at the deadline `SIGTERM`, then the run's scope is stopped, which also ends the sandbox's proxies |
//! | [`Hosted::model`] | control `set_model` |
//!
//! **Vision.** Max's work runs on Claude Code under his employer's managed
//! policy; he wants it "live on the map and in a chat, messageable, paused,
//! stopped, killed and resumed cleanly and fast", reachable on the work VM
//! and later on EC2 and on-prem through limbs, with no permission prompt
//! ever blocking an agent. Restart loses nothing (structure S5), Stop that
//! stops (S6), limbs anywhere (S4), and session data that is his.
//!
//! **Existing harnesses.** Claude Code Desktop and the Agent SDK host the
//! same `claude` process over the same stream; Desktop holds each permission
//! prompt until someone answers it. Here no prompt is ever held.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::context::Span;
use crate::id::{CallId, EventId, TaskId, ValueId};

/// The tool set of a machine limb that runs Claude Code. A role that holds
/// it is hosted: its agents' loops run in Claude Code on that limb, each in
/// its own working directory ([`crate::limb::Workdir`]), with Claude Code's
/// own tools in place of [`crate::limb::WORKSPACE`]'s. Its offer's settings
/// are a [`ClaudeCodeOffer`].
pub const CLAUDE_CODE: &str = "claude-code";

/// Who runs a task's agent loop, from the sets its role holds
/// ([`crate::role::Role::engine`]).
///
/// **Concept.** The loop is something a limb can offer, like a tool set, so
/// choosing it is choosing a set: "tools: claude-code, meta" reads as what
/// the agent uses, and the task's binding routes it to a limb as it routes
/// every set. Kept with the context's root, so a role that changes engine
/// takes effect at the task's next fresh root, never under it.
///
/// **Vision.** One task tree for every agent, whichever program runs it; at
/// work, every model call through Claude Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engine {
    /// The brain renders every request, sends it with its own login, and
    /// routes every tool use: a first-class agent.
    Brain,
    /// Claude Code in print mode, hosted by the limb that offers
    /// [`CLAUDE_CODE`] (module docs).
    ClaudeCode,
}

/// A limb's settings for its [`CLAUDE_CODE`] offer
/// ([`crate::limb::Offer::config`]): the machine's environment for Claude
/// Code sessions, which Max defines and may change.
///
/// **Concept.** The limb starts every run with its own fixed arguments
/// (`-p`, stream-json both ways, partial messages, `--permission-prompt-tool
/// stdio`, its hook callbacks and the harness's SDK MCP server,
/// `--append-system-prompt` with the root's system, `--disallowedTools
/// AskUserQuestion`), never a flag work's policy refuses (`--mcp-config`
/// with a server of another kind, `--agents`, `--plugin-dir`). What varies
/// by machine is here. Claude Code reads its own login, settings, MCP
/// servers, skills and plugins as it always does, so work's managed policy
/// and his work setup apply unchanged.
///
/// **Vision.** "I'm not actually disobeying the rules. I'm still using
/// Claude Code. The enterprise policy still applies."
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeOffer {
    /// The `claude` executable; the one on `PATH` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<PathBuf>,
    /// Added to each run's environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub mode: PermissionMode,
    /// Minutes a session may rest, with nothing running and nothing held,
    /// before its run is ended to free its memory; the next input resumes
    /// it in a second or two. Zero keeps every run until its task is parked
    /// or ends.
    #[serde(default = "idle_minutes")]
    pub idle: u32,
}

fn idle_minutes() -> u32 {
    10
}

/// The permission mode each run starts in (`--permission-mode`). Bypassing
/// permissions is not one: work's policy disables it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Edits in the working directory need no asking; everything else is
    /// answered by grants. The nearest to Max's own `bypassPermissions`
    /// that a managed policy allows.
    #[default]
    AcceptEdits,
    /// Claude Code's classifier decides; what it would ask, and every rule
    /// that asks, still reaches the limb.
    Auto,
    /// Claude Code's default: every use no rule allows asks.
    Default,
}

/// A Claude Code session: its durable conversation, named by the uuid Claude
/// Code files its transcript under on the limb's machine
/// (`~/.claude/projects/<cwd>/<uuid>.jsonl`).
///
/// **Concept.** The hosted task's context lives in it
/// ([`crate::task::Context::session`]). One session belongs to one task:
/// two processes on one session write the same transcript with no lock and
/// branch it, so the harness never runs two, and a fork, a resume of a
/// finished task or a move back always copies into a new session.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Session(pub String);

/// A message's uuid in a Claude Code session: the id the limb gives each
/// message it writes, and the id Claude Code gives each one it records,
/// which `--resume-session-at` names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageUuid(pub String);

/// How a run opens its session: the input of its call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Launch {
    /// The task's first run: a new session named `session`.
    New { session: Session },
    /// The task's own session, which already exists (`--resume`): after an
    /// idle rest, a park, or the loss of the run before.
    Resume { session: Session },
    /// A new session `session`, copied from `from` as it stood at `at`, or
    /// at its end (`--resume from --fork-session`, with
    /// `--resume-session-at at`): a fork, a finished task resumed with a
    /// new mandate, a move back to an earlier entry, or a session the
    /// harness did not start, taken over without touching the original.
    Fork {
        session: Session,
        from: Session,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<MessageUuid>,
    },
}

/// A request a hosted task's engine sent, as its run reported it
/// ([`crate::model_call::ModelCall::mirrored`]).
///
/// **Concept.** The brain's own requests are recorded as the exact bytes
/// sent. A mirrored one is recorded as Claude Code reports it: what it
/// appended, as the stream shows it, and the response. Claude Code's own
/// insertions (its reminders, its instructions files) are in its transcript
/// on the limb's machine, not here. The uuids let a move back or a fork
/// open the session at this request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirrored {
    /// The run that sent it: its limb states it only while that run is
    /// dispatched to it and unsettled.
    pub run: CallId,
    /// Claude Code's uuids of the messages it appended, in order.
    pub uuids: Vec<MessageUuid>,
    /// Its uuid of the response, once it ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<MessageUuid>,
}

/// What the brain wants a hosted task's run to take now: one row of the
/// hosting limb's view per hosted task with a run
/// ([`crate::limb::LimbRow::Hosted`]).
///
/// **Concept.** A limb follows its view and has no run message, so nothing
/// is sent twice after a reconnection: it writes each feed and reply once,
/// by its id, and applies each cut once. The row is current state, derived
/// from the brain's facts; the limb's own records say what it already did.
///
/// **Vision.** Max's words reach a working session at once and are taken
/// in at its next request, as with the brain's own agents; a pause or an
/// interrupt takes effect within a second; a brain relaunch changes nothing
/// a session sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hosted {
    pub task: TaskId,
    /// Its run now.
    pub run: CallId,
    /// What is pending for the task, rendered as the brain renders its own
    /// requests, oldest first. A feed leaves the row when a request renders
    /// it.
    pub feeds: Vec<Feed>,
    /// The outcomes of harness tools the session called and has not been
    /// answered: settled, or backgrounded and answered with their label.
    pub replies: Vec<Reply>,
    /// Paused now ([`crate::task::Pause::Now`] over it): no tool use starts,
    /// no feed is written, a response in flight finishes.
    pub hold: bool,
    /// Interrupt the turn in flight, once per id: Max's interrupt, a cut
    /// request, an abandon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut: Option<EventId>,
    /// The model Max tuned the task to, when not its root's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// One pending input, as a user message for the session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feed {
    /// The message's uuid, derived from its first span, so a request that
    /// takes it can be told apart by Claude Code's report of it.
    pub uuid: MessageUuid,
    /// What it was rendered from: what the request that takes it renders.
    pub renders: Vec<Span>,
    /// The message's exact content.
    pub content: ValueId,
}

/// The answer to one harness tool call the session made through the
/// harness's MCP server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub call: CallId,
    /// The MCP tool result, rendered from the call's outcome as the brain
    /// renders a tool result, or the label it runs on under in the
    /// background.
    pub result: ValueId,
}

/// What a tool set's tools act on, which decides whether a hosted task may
/// hold it ([`crate::limb::ToolSet::effects`]).
///
/// **Concept.** A hosted task's root binds only sets that act on the
/// harness, reached through the harness's MCP server: spawning, waiting,
/// messages, its page. A harness shell, file, fetch or browser beside
/// Claude Code's own tools would act outside work's policy, which is
/// evasion, so a hosted task never has one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effects {
    /// It acts only on the harness: its tasks, calls, pages and messages.
    Harness,
    /// It acts on a machine or the network.
    #[default]
    World,
}
