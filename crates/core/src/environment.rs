//! Environments and limbs: what an agent can do, and what it needs to know to
//! do it.

use serde::{Deserialize, Serialize};

use crate::call::Call;
use crate::id::{CallId, EnvironmentId, InstanceId, LimbId};
use crate::journal::Batch;
use crate::live::Live;
use crate::resource::{Observation, ResourceKey};

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
/// fresh is its own list ([`Startable`]), which the journal enforces: an
/// orchestrator's lists designers and engineers, a designer's does not list
/// designers, so "only the orchestrator launches designers" is the shape of
/// each one's world.
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
/// Made with the root's `tools` array by [`Binding::new`], from the same
/// descriptions, so a root cannot declare a tool its binding does not route
/// or route one it does not declare. Carried to every fork of the root, so
/// the same bytes (the root's tool names) mean the same limbs. A redefined
/// environment is picked up at the task's next root. An agent's call names
/// only its tool and is routed here, which is the whole of the harness's
/// access control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// The environment, as it was when the root was made.
    pub environment: EnvironmentId,
    pub routes: Vec<Route>,
}

impl Binding {
    /// The binding of a new root in `environment` and the root's exact
    /// `tools` array, from the descriptions of the limbs it holds, in order.
    /// Tools only Max may call are routed nowhere and declared nowhere.
    /// Refused, with the name, when two limbs offer the same tool name.
    pub fn new(
        environment: EnvironmentId,
        limbs: &[(LimbId, &Description)],
    ) -> Result<(Binding, String), String> {
        let mut routes: Vec<Route> = Vec::new();
        let mut tools = String::from("[");
        for (limb, description) in limbs {
            for tool in &description.tools {
                if tool.audience == Audience::Max {
                    continue;
                }
                if routes.iter().any(|r| r.tool == tool.name) {
                    return Err(tool.name.clone());
                }
                if !routes.is_empty() {
                    tools.push(',');
                }
                tools.push_str(&tool.definition);
                routes.push(Route {
                    tool: tool.name.clone(),
                    limb: *limb,
                });
            }
        }
        tools.push(']');
        Ok((
            Binding {
                environment,
                routes,
            },
            tools,
        ))
    }

    /// The limb that runs `tool` in this context, if the context has it.
    pub fn limb(&self, tool: &str) -> Option<LimbId> {
        self.routes.iter().find(|r| r.tool == tool).map(|r| r.limb)
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
    pub kind: LimbKind,
    pub fork: Fork,
}

/// What a limb's state does when a context using it forks. Declared by the
/// limb and stated in the instructions it contributes, so the agents know.
///
/// The vision's third option, a clone of the limb per fork (a REPL's
/// variables, a page's state), waits for a limb that has such state; it is a
/// new variant here and a clone mapping on [`crate::task::Origin::Fork`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fork {
    /// Forks call the same limb: a workspace siblings coordinate through,
    /// whose races the notices and refused stale writes handle.
    Share,
    /// A context bound to this limb cannot be forked; spawns from it are
    /// fresh.
    Refuse,
}

/// What a limb is, which decides how its calls are carried out. Only the
/// limbs the brain implements are named; every other kind is a plugin, so
/// adding one changes no core type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimbKind {
    /// Lifecycle tools that act on their caller: spawn a scope (forks of
    /// itself, or fresh tasks of the kinds listed), add a sibling to its own
    /// scope, hand itself over, run a call in the background. A fork may
    /// always be spawned: it cannot reach anything its parent cannot.
    ///
    /// The brain runs a meta call by appending its effects with its one
    /// dispatch, so it is never sent twice.
    Meta { starts: Vec<Startable> },
    /// Max. Its agent tool is `ask`: a call he answers, which puts the caller
    /// in his queue until he does. Its calls are never dispatched; his face
    /// shows them, and his answer is the settlement he authors. Only agents
    /// in an environment holding this limb can interrupt him mid-turn; any
    /// user-facing task gets his attention by ending its turn. Provisional:
    /// whether every environment holds it by default is Max's call.
    ///
    /// Max's controls over the tree (stop, interrupt, hold, rename, mark
    /// done, rewind, resume, aside) are not tools of this limb. They are facts
    /// he authors directly ([`crate::task::TaskEvent`]), because they act on
    /// tasks no call belongs to, and no agent can call them.
    Max,
    /// Max's queue: read it, rank it, say why. The scheduler's only limb.
    /// Run by the brain like [`LimbKind::Meta`].
    Attention,
    /// A workspace, a JavaScript process, a page, or anything else. Its
    /// calls are sent over a connection, again after every reconnection
    /// while they are unsettled.
    Plugin {
        kind: String,
        /// The plugin's own settings, as JSON.
        config: String,
        host: Host,
    },
}

/// A fresh task a meta limb can start: an agent kind in an environment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Startable {
    pub agent: String,
    pub environment: EnvironmentId,
}

/// Where a plugin limb runs, which decides what survives a brain restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Host {
    /// A process of its own, so a brain relaunch does not kill its builds.
    /// It keeps a durable record of its calls, so it can say how one ended
    /// after the instance that ran it has gone.
    Process(Machine),
    /// In a face: present only while one is connected.
    Face,
}

/// Where a limb process runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Machine {
    Local,
    Ssh(String),
}

/// What the brain knows of its connection to a plugin limb. Three lifetimes
/// are distinct: the resources (a workspace's files outlive everything), the
/// instance (a process) and the connection to it. A connection belongs to
/// the brain that made it, so every limb is unreached from a
/// [`crate::journal::Launched`] until it connects to the new brain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reach {
    /// Connected to `instance`. Every unsettled call of the limb that was
    /// last sent before this is sent again.
    Connected { instance: InstanceId },
    /// The connection dropped. Calls sent to it stay sent; the throbber says
    /// "waiting on reconnect" rather than "running".
    Lost,
}

/// What a limb tells the brain about itself, rendered into each root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// Instructions for using the limb: its `AGENTS.md` hierarchy, its
    /// conventions, where it is (a workspace's root directory), its skills
    /// index.
    pub instructions: Vec<Instruction>,
    pub tools: Vec<Tool>,
}

/// One piece of a limb's instructions, and the version it was read at, which
/// the root then embodies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub text: String,
    pub observes: Observation,
}

/// One tool a limb offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    /// The provider's exact tool definition: name, description, input
    /// schema, and whether it is deferred.
    pub definition: String,
    pub audience: Audience,
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
///
/// The protocol carries the journal's own vocabulary: a call is a
/// [`Call`], and what comes back is facts. So there is nothing to
/// translate, and a limb can say exactly what it knows (which call wrote a
/// file) in the fact that records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToLimb {
    Describe,
    /// Run this call, at most once per id. It arrives again after every
    /// reconnection while it is unsettled: answer from the durable record if
    /// there is one, keep running it if it runs, otherwise run it if its
    /// tool may be rerun, or settle it as unknown. A limb that can check
    /// `call.expects` refuses the call when they have moved, before it has
    /// any effect.
    Run {
        id: CallId,
        call: Call,
        /// The bytes of `call.input`.
        input: Vec<u8>,
    },
    /// Stop this call as soon as it safely can; it settles as interrupted.
    /// Asked again after a reconnection while it is unsettled; a call the
    /// limb has no record of settles as unknown.
    Stop {
        call: CallId,
    },
    /// Report changes to these resources: what live contexts embody.
    Watch {
        resources: Vec<ResourceKey>,
    },
}

/// What a limb sends the brain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FromLimb {
    /// Sent first on every connection. The brain records it as
    /// [`Reach::Connected`], and sends again every unsettled call it last
    /// sent before.
    Hello {
        instance: InstanceId,
    },
    Described(Description),
    /// Facts about its own calls and resources, with the bytes they name:
    /// a settlement with its output, a change with the call that made it.
    /// The brain commits them as the limb's, and refuses any other fact.
    Facts(Batch),
    /// Output so far of a running call.
    Live(Live),
}
