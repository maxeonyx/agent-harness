//! Environments and limbs: what a kind of agent is, what it can do, and what
//! it needs to know to do it.

use serde::{Deserialize, Serialize};

use crate::call::Call;
use crate::id::{CallId, EnvironmentId, InstanceId, LimbId, ValueId};
use crate::journal::Batch;
use crate::live::Live;
use crate::resource::Observation;
use crate::value::Bytes;

/// A role: what one kind of agent is, everything it can reach, and how it
/// thinks.
///
/// **Concept.** A [`Limb`] is one thing that runs calls and owns resources:
/// a workspace on a machine, a JavaScript process, the harness's own page in
/// Max's browser, the brain's meta actions, Max himself, Max's queue. An
/// environment is the definition of a kind of agent in terms of them: its
/// role prompt, its model and request parameters, the limbs every agent of
/// the kind shares ([`Environment::limbs`]), the limbs each agent of the
/// kind is given its own of ([`Environment::places`]), and the kinds it can start fresh
/// ([`Environment::starts`]). "Orchestrator", "designer", "engineer" and
/// "attention scheduler" are environments; a [`crate::task::Task`] is in
/// exactly one, for life.
///
/// Each limb [describes](Description) its tools and the instructions for
/// using them, as a fact in the journal, and the brain renders the role and
/// those descriptions into
/// the [root](crate::context::Root) of each context it starts here. So tools
/// and the knowledge to use them arrive together, are fixed together for
/// the context's life, and change together through the notices of
/// [`crate::resource`]. An engineer's root says which directory and branch
/// are its own because its workspace place describes itself that way, not
/// because a brief remembered to.
///
/// What an agent cannot do is what its environment does not contain. The
/// attention scheduler's environment holds only the [`LimbKind::Attention`]
/// limb, so it can reorder the queue and cannot spawn, read files or speak
/// to Max: there is no permission check to get wrong. What a task can start
/// fresh is its own environment's `starts`, checked by the journal against
/// the task whose meta call creates it: an orchestrator's lists designers
/// and engineers, a designer's and an engineer's list none, so "only the
/// orchestrator launches designers" is the shape of each one's world, and a
/// member adding a sibling to its scope is held to its own list, not its
/// parent's.
///
/// **Defined and changed.** A version is a fact, stated by Max or by the
/// brain. The brain states one when a definition it reads changes: a role
/// file in a repository, edited and reviewed like code, so "another agent
/// changing the definition" is an engineer's pull request, and the version
/// records the file it came from ([`Environment::source`]). A new version
/// takes effect for a task at its next request for `params` (they are not
/// part of the cached prefix), and at its next root for everything else,
/// never under a running context; a task whose root observed the old
/// source is told it changed, at its next request, like any file it read.
///
/// **Vision.** "An environment is not just a set of available tools, it is
/// also the context needed to use them", which can be load-bearing; no
/// permission prompts ("an environment is defined with its permissions;
/// extending them means changing that definition, possibly by another
/// agent"); environments that prevent things by construction (the attention
/// scheduler "has available the tools, and only those"); effort and model
/// per role; a clone of one's own; Max using the agent's own tools in its
/// chat ([`Audience`]); limbs that outlive the brain
/// ([`LimbKind::Plugin`]); restart ([`Reach`]).
///
/// **Existing harnesses.** Claude Code's agent files (this repository's
/// `.claude/agents/designer.md`) give a kind a prompt, a model and an
/// effort, and Oh My Pi's add which kinds it may spawn and a thinking level
/// (`ref-oh-my-pi/packages/coding-agent/src/task/agents.ts:20`); both filter
/// one session's tool list per kind, and the spawn list is checked in the
/// task tool's own code. Oh My Pi isolates a spawn by copying the repository
/// and merging back (`ref-oh-my-pi/packages/coding-agent/src/task/isolation-runner.ts:140`).
/// OpenCode, Codex and Goose gate tool calls with permission prompts
/// (OpenCode suspends the call until Max answers,
/// `ref-opencode/packages/core/src/permission.ts:227`). In each, a changed
/// definition is either ignored by running agents or silently re-rendered
/// under them.
///
/// **Different here.** The definition is a versioned fact, so what an agent
/// could reach at any moment is in the journal; its reach is checked by the
/// fold against the task that creates another, not by a filter in a tool; a
/// clone of its own is a limb its environment names as a place, so the
/// clone's instructions and file versions are its context; and a change
/// reaches running agents as a dated notice and takes effect at their next
/// root.
///
/// **Bx.** Shared: a definition that exists before anything is built from
/// it, and a build record that pins the resolved inputs. A root is that
/// record here: its [`crate::task::Start::observes`] are the versions of the
/// role file, the instructions and the tool schemas it was made from, and
/// the context grown from it is the artifact. Not shared: a changed input
/// does not rebuild anything, because a context cannot be rebuilt without
/// losing what the model did in it; it becomes a notice, and the next root
/// picks it up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: EnvironmentId,
    /// What Max and the agents call it: `orchestrator`, `designer`. Unique
    /// among environments.
    pub name: String,
    /// The role prompt: who an agent here is and how it works. Rendered into
    /// every root, after the provider identity and before the limbs'
    /// instructions.
    pub role: ValueId,
    /// The model every root here names.
    pub model: String,
    /// The request parameters every turn here sends, as the exact JSON
    /// object: effort, thinking, the output limit. Not part of the cached
    /// prefix, so a new version applies from each task's next request.
    pub params: ValueId,
    /// The limbs every task here holds: the same ones for all of them, such
    /// as the meta limb, Max, or a shared read-only checkout.
    pub limbs: Vec<LimbId>,
    /// The limbs each task here is given its own of, by plugin kind: an
    /// engineer's `workspace`, its clone. Filled when the task is created
    /// ([`crate::task::Origin::Fresh`]) with a limb of that kind, and fixed
    /// for the task's life; its forks share it. Usually a limb created in
    /// the same batch, and sometimes an existing one, such as the clone a
    /// designer pushed that its engineer now builds on. The kind is checked,
    /// so a parent filling a place cannot hand its child a limb the child's
    /// definition does not name, such as Max or its own meta limb. Two
    /// places of one kind wait for an environment that needs them; they
    /// would each need a name.
    pub places: Vec<String>,
    /// The environments a task here can start fresh tasks in. A fork of
    /// itself can always be started: it cannot reach anything its source
    /// cannot.
    pub starts: Vec<EnvironmentId>,
    /// The definition this version was read from: a role file at a version.
    /// Roots made here observe it, so an edit to it is a notice to every task
    /// working under the old one. `None` for a version Max stated directly.
    pub source: Option<Observation>,
}

/// Which limb runs each tool of a task's context.
///
/// Made with the root's `tools` array by [`Binding::new`], from the same
/// descriptions, so a root cannot declare a tool its binding does not route
/// or route one it does not declare. Carried to every fork of the root, so
/// the same bytes (the root's tool names) mean the same limbs. Every route
/// goes to a limb of the task's environment or one of its places, which the
/// journal checks, and only to a tool that limb's journaled description
/// offers agents. An agent's call names only its tool and is routed here,
/// which is the whole of the harness's access control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub routes: Vec<Route>,
}

/// Two limbs of one root offer a tool of the same name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Duplicate {
    pub tool: String,
}

impl std::fmt::Display for Duplicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "two limbs offer the tool `{}`", self.tool)
    }
}

impl std::error::Error for Duplicate {}

impl Binding {
    /// The binding of a new root and the root's exact `tools` array, from
    /// the descriptions of the limbs it holds, in order. Tools only Max may
    /// call are routed nowhere and declared nowhere. Refused, with the name,
    /// when two limbs offer the same tool name.
    pub fn new(limbs: &[(LimbId, &Description)]) -> Result<(Binding, String), Duplicate> {
        let mut routes: Vec<Route> = Vec::new();
        let mut tools = String::from("[");
        for (limb, description) in limbs {
            for tool in &description.tools {
                if tool.audience == Audience::Max {
                    continue;
                }
                if routes.iter().any(|r| r.tool == tool.name) {
                    return Err(Duplicate {
                        tool: tool.name.clone(),
                    });
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
        Ok((Binding { routes }, tools))
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
///
/// A new version may change its name or a plugin's config, never its kind:
/// calls already made were routed by what it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limb {
    pub id: LimbId,
    /// Unique among limbs, so a limb is found by name and nothing reads a
    /// plugin's config to find it: a workspace is named by its directory,
    /// which is what Max and the agents call it.
    pub name: String,
    pub kind: LimbKind,
}

/// What a limb is, which decides how its calls are carried out. Only the
/// limbs the brain implements are named; every other kind is a plugin, so
/// adding one changes no core type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimbKind {
    /// Lifecycle tools that act on their caller: spawn a scope (forks of
    /// itself, fresh tasks in the environments its own environment
    /// [starts](Environment::starts), or resumes of its finished children),
    /// add a sibling to its own scope, hand itself over, run a call in the
    /// background. One meta limb serves every environment: what a call may
    /// create is read off its caller's environment when the task is created,
    /// and the brain describes the limb to each root with that
    /// environment's list.
    ///
    /// The brain runs a meta call by appending its effects with its one
    /// dispatch, so it is never sent twice.
    Meta,
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
    /// A workspace, a JavaScript process, or anything else: a process of
    /// its own, started by the brain from its kind, so a brain relaunch does
    /// not kill its builds. It keeps a durable record of its calls, so it can
    /// say how one ended after the instance that ran it has gone. Its calls
    /// are sent over a connection, again after every reconnection while
    /// they are unsettled. A limb hosted elsewhere (in Max's page, on
    /// another machine) is a new variant when the first one is built.
    Plugin {
        kind: String,
        /// The plugin's own settings, as JSON, read by the plugin alone.
        /// Anything the harness or a face needs from a limb, it says in its
        /// [`Description`] or its name.
        config: String,
    },
}

impl LimbKind {
    /// Whether the brain runs it itself, and so states its descriptions'
    /// equivalents in code and its resources' changes as facts of its own.
    pub fn is_brains(&self) -> bool {
        !matches!(self, LimbKind::Plugin { .. })
    }

    /// Whether `other` is the same kind, which every version of a limb is.
    pub fn same(&self, other: &LimbKind) -> bool {
        match (self, other) {
            (LimbKind::Plugin { kind: a, .. }, LimbKind::Plugin { kind: b, .. }) => a == b,
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }
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

/// What a plugin limb says about itself, rendered into each root.
///
/// A fact ([`crate::journal::Fact::Described`]), stated by the limb on each
/// connection, after its hello. So the fold knows what every limb offers:
/// a root may route only tools its limbs offer agents, Max may call only
/// tools offered him, a starting task is rooted once each of its limbs has
/// described itself on its current connection, and a face shows a limb from
/// the journal without reading its config. The brain's own limbs are
/// described by the brain's code at each root, since the meta limb's
/// description depends on who holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// Instructions for using the limb: its `AGENTS.md` hierarchy, its
    /// conventions, where it is (a workspace's root directory, and the
    /// branch a clone was made for), its skills index.
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
    /// The name in `definition`. Stated beside it so the core reads no JSON;
    /// a limb builds both from one source.
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
        input: Bytes,
    },
    /// Stop this call: ask it to stop gracefully (`SIGTERM` to a command's
    /// processes), and kill what is left `within` milliseconds of receiving
    /// this; it settles as interrupted once nothing it started is left.
    /// Asked again after a reconnection while it is unsettled, and again with
    /// less time to make the stop harder; a call the limb has no record of
    /// settles as unknown.
    Stop { call: CallId, within: u64 },
    /// The brain is going away: stop every running call as [`ToLimb::Stop`]
    /// does, then exit once nothing the limb started is left, all within
    /// `within` milliseconds. Its input closing means the same, with the
    /// limb's own default. Asked again with less time to make it harder.
    Shutdown { within: u64 },
    /// Watch these resources: what live contexts embody. `known` is every
    /// version the journal holds of each: its current version where one is
    /// recorded, otherwise each version a live context embodies. Report, as
    /// a [`crate::resource::Change`], each resource whose version is none of
    /// these, and from then on each change as it happens, so every change a
    /// limb states is one.
    Watch { known: Vec<Observation> },
}

/// What a limb sends the brain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FromLimb {
    /// Sent first on every connection, once the limb is ready to describe
    /// itself (a clone it was configured to make is made). The brain records
    /// it as [`Reach::Connected`], and sends again every unsettled call it
    /// last sent before. The limb's first facts on the connection are its
    /// [`crate::journal::Fact::Described`].
    Hello { instance: InstanceId },
    /// Facts about itself, its own calls and its own resources, with the
    /// bytes they name: its description, a settlement with its output, a
    /// change with the call that made it. The brain commits them as the
    /// limb's, and the journal refuses any other fact.
    Facts(Batch),
    /// Output so far of a running call.
    Live(Live),
}
