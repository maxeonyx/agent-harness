//! Roles: what a kind of agent is, in the abstract.

use serde::{Deserialize, Serialize};

use crate::id::{LimbId, ValueId};
use crate::limb::ToolSet;
use crate::resource::Observation;

pub use crate::environment::{Duplicate, Effort, Tuning, UnknownEffort};

/// A role's identity, the same for every version of it.
pub type RoleId = crate::id::EnvironmentId;

/// A role: the abstract kind of agent. Its system prompt, its model and
/// request parameters, the tool sets its agents hold, and the roles they
/// can start.
///
/// **Concept.** Max (29 Oct): "A role is the abstract thing, and then an
/// agent is the concrete thing. An agent has a context and a working
/// directory and a role. The role includes the agent instructions, the
/// skill set, the tool set." So a role names no limb and no directory. Its
/// tool sets are names ([`Role::tools`]). Each of its agents finds each set
/// on a limb when it is created: the brain's own sets on the brain, every
/// other on the limb of the agent's working directory, or of the brain's
/// own machine for an agent with none. Its instructions and skills come
/// with the sets that carry them: Max's own with `user`, a directory's
/// `AGENTS.md` with the working directory. "Orchestrator", "designer",
/// "engineer" and "chat" are roles; a [`crate::task::Task`] is in exactly
/// one, for life.
///
/// A role that holds only the brain's sets is a meta role: its agents have
/// no working directory and need no process on any machine. Max: "Now that
/// I say meta lim, I think meta role makes more sense. A role can be hosted
/// in the brain."
///
/// What an agent cannot do is what its role holds no set for. The attention
/// scheduler's role holds only `attention`, so it can reorder the queue and
/// cannot spawn, read files or speak to Max: there is no permission check
/// to get wrong. What a task can start fresh is its own role's `starts`,
/// checked by the brain's fold against the task whose meta call creates it,
/// so "only the orchestrator launches designers" is the shape of each
/// one's world, and a member adding a sibling to its scope is held to its
/// own list, not its parent's.
///
/// **Defined and changed.** A version is a fact, stated by the brain's
/// `roles` set when a role file in a repository changes, edited and
/// reviewed like code; the version records the file it came from
/// ([`Role::source`]). A new version takes effect for a task at its next
/// request for `params`, and at its next root for everything else, never
/// under a running context; a task whose root observed the old file is told
/// it changed, at its next request. Max overrides the model and effort for
/// one task ([`Tuning`]).
///
/// **Vision.** "An environment is not just a set of available tools, it is
/// also the context needed to use them"; no permission prompts ("an
/// environment is defined with its permissions; extending them means
/// changing that definition, possibly by another agent"); the attention
/// scheduler that "has available the tools, and only those"; effort and
/// model per role.
///
/// **Existing harnesses.** Claude Code's agent files (this repository's
/// `.claude/agents/designer.md`) give a kind a prompt, a model and an
/// effort, and Oh My Pi's add which kinds it may spawn and a thinking level
/// (`ref-oh-my-pi/packages/coding-agent/src/task/agents.ts:20`); both filter
/// one session's tool list per kind, and check the spawn list in the task
/// tool's own code. In each, a changed definition is either ignored by
/// running agents or silently re-rendered under them.
///
/// **Different here.** The definition is a versioned fact, checked by the
/// fold against the task that creates another, not by a filter in a tool;
/// and a change reaches running agents as a dated notice and takes effect
/// at their next root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: RoleId,
    /// What Max and the agents call it: `orchestrator`, `designer`. Unique
    /// among roles.
    pub name: String,
    /// One line on what an agent in it does, for those who can start one.
    #[serde(default)]
    pub description: String,
    /// The role prompt: who an agent in it is and how it works. Rendered
    /// into every root, after the provider identity and before its tool
    /// sets' instructions.
    #[serde(alias = "role")]
    pub prompt: ValueId,
    /// The model every root names, unless the task is tuned to another.
    pub model: String,
    /// The request parameters every turn sends, as the exact JSON object:
    /// effort, thinking, the output limit. Not part of the context, so a new
    /// version applies from each task's next request.
    pub params: ValueId,
    /// The tool sets its agents hold, by name, in the order their tools are
    /// declared: `meta`, `web`, `user`, `workspace`, `browser`, `builds`,
    /// currently. One that holds [`crate::limb::WORKSPACE`] gives each agent
    /// a working directory when it is created.
    #[serde(default)]
    pub tools: Vec<String>,
    /// The roles a task in it can start fresh tasks in. A fork of itself can
    /// always be started: it cannot reach anything its source cannot.
    pub starts: Vec<RoleId>,
    /// The definition this version was read from: a role file at a version.
    /// `None` for a version Max stated directly.
    pub source: Option<Observation>,
}

impl Role {
    /// Whether its agents have a working directory.
    pub fn works_in_a_directory(&self) -> bool {
        self.tools.iter().any(|t| t == crate::limb::WORKSPACE)
    }
}

/// Which limb runs each tool of a task's context.
///
/// Made with the root's `tools` array by [`Binding::new`], from the same
/// tool sets, so a root cannot declare a tool its binding does not route or
/// route one it does not declare. Carried to every fork of the root, so the
/// same bytes (the root's tool names) mean the same limbs. Every route goes
/// to the limb that hosts one of the task's role's sets for this task,
/// which the brain's fold checks. An agent's call names only its tool and
/// is routed here, which is the whole of the harness's access control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub routes: Vec<Route>,
}

impl Binding {
    /// The binding of a new root and the root's exact `tools` array, from
    /// the tool sets its task holds, each with the limb that hosts it for
    /// the task, in the role's order. Tools only Max may call are routed
    /// nowhere and declared nowhere. Refused, with the name, when two sets
    /// offer the same tool name.
    pub fn new(sets: &[(LimbId, &ToolSet)]) -> Result<(Binding, String), Duplicate> {
        let mut routes: Vec<Route> = Vec::new();
        let mut tools = String::from("[");
        for (limb, set) in sets {
            for tool in &set.tools {
                if tool.audience == crate::limb::Audience::Max {
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
    /// A limb from an older record is resolved by the state
    /// (`State::resolve`) before anything is sent to it.
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
