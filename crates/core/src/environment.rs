//! Environments and limbs: what an agent can do, and what it needs to know to
//! do it.

use serde::{Deserialize, Serialize};

use crate::call::Outcome;
use crate::context::Wire;
use crate::id::{CallId, EnvironmentId, InstanceId, LimbId, TaskId, Timestamp};
use crate::resource::{Change, Observation, ResourceKey};

/// A named set of limbs: everything an agent in it can reach.
///
/// **Concept.** A [`Limb`] is one thing that runs calls and owns resources:
/// a workspace on a machine, a JavaScript process, the harness's own page in
/// Max's browser, the brain's meta actions, Max himself, Max's queue. An
/// environment is a set of them. Each limb [describes](Description) its
/// tools and the instructions for using them, and the brain renders that
/// description into the [root](crate::context::Root) of each context it
/// starts there, so tools and the knowledge to use them arrive together,
/// are fixed together for the context's life, and change together through
/// the notices of [`crate::resource`].
///
/// What an agent cannot do is what its environment does not contain. The
/// attention scheduler's environment holds only the [`LimbKind::Attention`]
/// limb, so it can reorder the queue and cannot spawn, read files or speak
/// to Max: there is no permission check to get wrong. Spawning, adding a
/// sibling and handing over are tools of the [`LimbKind::Meta`] limb, which
/// the brain implements, so they go through the same call machinery as a
/// file edit, and each acts only on its caller. What a meta limb can start
/// fresh is its own list ([`Startable`]): an orchestrator's lists designers
/// and engineers, a designer's does not list designers, so "only the
/// orchestrator launches designers" is the shape of each one's world.
///
/// An environment is defined by a fact, written by Max or by an agent whose
/// environment holds a limb that defines environments. A new version takes
/// effect for a task at its next root (a handover or a reseed), never under
/// a running context; until then the task is told its catalogue changed.
///
/// **Vision.** "An environment is not just a set of available tools, it is
/// also the context needed to use them"; no permission prompts (an
/// environment's reach is its definition, and widening it means changing the
/// definition, possibly by another agent); the attention scheduler; Max
/// using the agent's own tools in its chat ([`Audience`]); limbs that run on
/// other machines and outlive the brain ([`Host`]); restart ([`Reach`]).
///
/// **Existing harnesses.** OpenCode, Codex and Goose gate tool calls with
/// permission prompts (OpenCode suspends the call until Max answers,
/// `ref-opencode/packages/core/src/permission.ts:227`); Codex adds an OS
/// sandbox; Oh My Pi's `tools.approvalMode` defaults to "yolo"
/// (`ref-oh-my-pi/packages/coding-agent/src/tools/approval.ts:81`); Pi has
/// none. In each, the tool set is a filtered list for the session and its
/// instructions live in one system prompt.
///
/// **Different here.** The unit of capability is the environment, a value
/// that can be inspected, versioned and changed by whoever may change it,
/// rather than a run-time question to Max.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub name: String,
    pub limbs: Vec<LimbId>,
}

/// Which limb runs each tool of a task's context.
///
/// Fixed when a root is made and carried to every fork of it, so the same
/// bytes (the root's tool names) mean the same limbs. A limb declared
/// [`Fork::Clone`] is the exception: each fork's binding names its clone.
/// A redefined environment is picked up at the task's next root. A call is
/// accepted only along a route of its task's binding, which is the whole of
/// the harness's access control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// The environment, as it was when the root was made.
    pub environment: EnvironmentId,
    pub routes: Vec<Route>,
}

impl Binding {
    pub fn routes(&self, tool: &str, limb: LimbId) -> bool {
        self.routes.iter().any(|r| r.tool == tool && r.limb == limb)
    }
}

/// One tool name in a root, and the limb that runs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub tool: String,
    pub limb: LimbId,
}

/// One thing that runs calls and owns resources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limb {
    pub id: LimbId,
    pub name: String,
    pub host: Host,
    pub kind: LimbKind,
    pub fork: Fork,
}

/// What a limb's state does when a context using it forks. Declared by the
/// limb and stated in the instructions it contributes, so the agents know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fork {
    /// Forks call the same limb: a workspace siblings coordinate through,
    /// whose races the notices and refused stale writes handle.
    Share,
    /// Each fork gets a clone of the limb as it is at the fork: a REPL's
    /// variables, a page's state.
    Clone,
    /// A context bound to this limb cannot be forked; spawns from it are
    /// fresh.
    Refuse,
}

/// Where a limb runs, which decides what survives a brain restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Host {
    /// In the brain, transactional with its journal: a meta call's effects
    /// are appended in the same batch as its own events.
    Brain,
    /// A process of its own, so a brain relaunch does not kill its builds.
    /// It keeps a durable record of its calls, so it can say how one ended
    /// after the instance that ran it has gone.
    Process { machine: Machine },
    /// In a face: present only while one is connected.
    Face,
}

/// Where a limb process runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Machine {
    Local,
    Ssh(String),
}

/// What the brain currently knows of a limb. Three lifetimes are distinct:
/// the resources (a workspace's files outlive everything), the instance (a
/// process) and the connection to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reach {
    Connected {
        instance: InstanceId,
        since: Timestamp,
    },
    /// Calls dispatched to it stay dispatched; the throbber says "waiting on
    /// reconnect" rather than "running".
    Lost { last: InstanceId, since: Timestamp },
}

/// What a limb is. Only the limbs the brain implements are named; every
/// other kind is a plugin, so adding one changes no core type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimbKind {
    /// Lifecycle tools that act on their caller: spawn a scope (forks of
    /// itself, or fresh tasks of the kinds listed), add a sibling to its own
    /// scope, hand itself over, run a call in the background. A fork may
    /// always be spawned: it cannot reach anything its parent cannot.
    Meta { starts: Vec<Startable> },
    /// Max. Its agent tool is `ask`: a call he answers, which puts the caller
    /// in his queue until he does. Only agents in an environment holding this
    /// limb can interrupt him mid-turn; any user-facing task gets his
    /// attention by ending its turn. Provisional: whether every environment
    /// holds it by default is Max's call.
    ///
    /// Max's controls over the tree (stop, interrupt, hold, rename, mark
    /// done, rewind, resume, aside) are not tools of this limb. They are facts
    /// he authors directly ([`crate::task::TaskEvent`]), because they act on
    /// tasks no call belongs to, and no agent can call them.
    Max,
    /// Max's queue: read it, rank it, say why. The scheduler's only limb.
    Attention,
    /// A workspace, a JavaScript process, a page, or anything else.
    Plugin { kind: String, config: Wire },
}

/// A fresh task a meta limb can start: an agent kind in an environment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Startable {
    pub agent: String,
    pub environment: EnvironmentId,
}

/// What a limb tells the brain about itself, rendered into each root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// Instructions for using the limb: its `AGENTS.md` hierarchy, its
    /// conventions, its skills index, each with the version it was read at.
    pub instructions: Vec<(Wire, Observation)>,
    pub tools: Vec<Tool>,
}

/// One tool a limb offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    /// The provider's tool definition: description and input schema.
    pub definition: Wire,
    pub audience: Audience,
    /// Whether a call whose outcome is unknown after a crash may simply be
    /// sent again. Only the tool's author knows: a view can be rerun, a merge
    /// cannot. When false the caller is told the outcome is unknown.
    pub rerun: bool,
    /// Declared in the root but surfaced later, so progressive disclosure of
    /// tools stays append-only.
    pub deferred: bool,
}

/// Who may call a tool. Max's face renders his tools as controls; an
/// agent's root declares the agent's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Audience {
    Agent,
    Max,
    Both,
}

/// What the brain sends a limb.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToLimb {
    Describe,
    /// Run a call. A limb that can check `expects` refuses the call when they
    /// have moved, before it has any effect.
    Call {
        call: CallId,
        tool: String,
        input: Wire,
        expects: Vec<Observation>,
    },
    /// Stop a running call as soon as it safely can.
    Interrupt {
        call: CallId,
    },
    /// After a brain restart: how did these calls end?
    Recover {
        calls: Vec<CallId>,
    },
    /// Report changes to these resources.
    Watch {
        resources: Vec<ResourceKey>,
    },
    /// Make a clone of the limb's current state for a fork ([`Fork::Clone`]).
    Fork,
}

/// What a limb sends the brain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FromLimb {
    /// Sent first on every connection. A different instance than a call was
    /// dispatched to means the limb's in-memory state is gone; the brain asks
    /// how that call ended with [`ToLimb::Recover`].
    Instance(InstanceId),
    Described(Description),
    /// Output so far of a running call: [`crate::live::Live`], never a fact.
    Progress {
        call: CallId,
        chunk: Wire,
    },
    Settled {
        call: CallId,
        outcome: Outcome,
    },
    /// The limb has no record of this call: it never arrived, or the instance
    /// that had it is gone.
    Unknown {
        call: CallId,
    },
    Changed(Change),
    /// The clone asked for by [`ToLimb::Fork`].
    Forked {
        limb: LimbId,
    },
    /// Something happened that a task may care about: a test watcher's
    /// result, a page event. Addressed to a task, or to the standing tasks
    /// bound to this limb.
    Event {
        to: Option<TaskId>,
        event: Wire,
    },
}
