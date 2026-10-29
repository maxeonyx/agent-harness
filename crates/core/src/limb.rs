//! Limbs: the processes the harness gets onto machines so that its agents
//! can do things there, and the brain, which is a limb too.
//!
//! A limb is infrastructure, not part of the agent model. The abstract
//! side of an agent is its [role](crate::role::Role); the concrete side is
//! the [task](crate::task::Task), with its context and its working
//! directory ([`Workdir`]). What crosses between the brain and a limb is
//! the substrate's link: the limb's own records up ([`LimbUp`]), and the
//! rows the brain derives for it down ([`LimbDown`]).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::build::BuildId;
use crate::call::{Call, Owner};
use crate::id::{LimbId, ParseError, ReplicaId, TaskId, Timestamp};
use crate::legacy;
use crate::role::{Role, RoleId};
use crate::value::Bytes;

pub use crate::environment::{Audience, Hello, Instruction, Reach, Tool};

/// The tool set of a shell and file tools in the agent's working
/// directory. A role that holds it gives each of its agents a [`Workdir`].
pub const WORKSPACE: &str = "workspace";

/// A limb: a process the harness runs on one machine as one user, so that
/// agents can do things there; or the brain, which hosts the harness's own
/// tool sets.
///
/// **Concept.** Max (29 Oct): "I think a lim is an extension of the harness
/// infrastructure. It's not part of the agent model. We get a process onto
/// a remote machine so that we can do stuff on the remote machine. What
/// stuff? It varies." What a limb does is the tool sets it offers
/// ([`Limb::offers`]): a shell and files in any directory ([`WORKSPACE`]),
/// Max's own instructions and skills, a browser, the brain's service to
/// relaunch. A limb on his desktop might offer only a browser over CDP, a
/// locked-down one only web fetches from its network location.
///
/// There is one per brain, machine and user. Every agent of a brain that
/// works on a machine works through that machine's one limb, each in its
/// own working directory ([`Workdir`]), so the limb stays a few MiB however
/// many directories and agents it serves. Max: "The lim shouldn't have huge
/// memory usage itself, right?" What agents run is not the limb's: each
/// command runs in a systemd scope of its own, inside a slice for its agent
/// ("the lim could create a scope for an agent who has bash callability"),
/// so an agent's build shows as that agent's and stops with it. Another
/// brain on the same machine (another domain's, or a test brain trying a
/// change to limb code) defines a limb of its own there: each runs its own
/// brain's build, and one domain's calls never reach another's process.
///
/// The brain is a limb ([`Host::Brain`]): "A role can be hosted in the
/// brain. Given a tool set, instruction set, skill set, system prompt. The
/// brain is a lim." It offers the harness's own tool sets (`meta`, `web`,
/// `attention`, `roles`) from its own process, so a role that holds only
/// those, a meta role, needs no process on any machine.
///
/// **Process.** A limb on a machine is `harness-limb serve <limb>`, started
/// by `harness-limb attach <limb>` when the brain dials it and finds it not
/// running, in `harness-limb-<limb>.scope` where there is a systemd user
/// manager. Each command an agent runs is `harness-call-<call>.scope` in
/// the slice `harness-<task>.slice`, and ends, with everything it started
/// whatever its session or environment, when its call ends. Its state is
/// one directory on its host, `$XDG_STATE_HOME/harness/limbs/<limb>`: its
/// lock, its socket, its records, and its log. It keeps, for all its
/// directories at once, the versions of the files live contexts have seen
/// (named by absolute path, so two directories that nest see each other's
/// writes), the rescan of those after each command, the order each agent's
/// calls run in, and what each agent's own calls did. Per directory it
/// keeps only what belongs to the directory: making it, or cloning it, once,
/// and its instructions ([`Prepared`]).
///
/// **Errors.** Max: "I'm not interested in the crash. I think that that's
/// bad code if we crash. ... Rust gives us the tools to gracefully handle
/// almost all error cases, so we should." A limb's process ends only when
/// it cannot exist (another instance holds its state, or its socket or
/// records cannot be opened), on a signal, with no brain for 15 minutes,
/// or superseded with nothing running. A directory that cannot be made, a
/// tool set that cannot open, a row it cannot read and a tool that panics
/// each fail only themselves, in band: a [`Prepared::Failed`], a call
/// settled as failed with what went wrong.
///
/// **Identity.** Three things, each named once: the limb, a [`LimbId`]
/// whose definition is a fact in one brain's records; its instance, a
/// [`ReplicaId`], the timeline of records kept in its state, new only if
/// that state was lost ([`Reach::Connected`]); and its build, a
/// [`BuildId`], which may be older than the brain's, since a limb outlives
/// a relaunch.
///
/// **Defined and changed.** The brain defines its own limb and its
/// machine's when it starts, from its options, and states a new version
/// when an offer's settings change: the limb hears it in its view
/// ([`LimbRow::Definition`]). A new version may change its name and its
/// offers, never its host.
///
/// **Existing harnesses.** Codex's exec-server is also a server the harness
/// connects to, one per machine, but it kills a connection's processes when
/// the connection closes (`ref-codex/codex-rs/exec-server/README.md:56`).
/// OpenCode, Pi and Goose run tools in the agent's own process, so a remote
/// tool means a remote agent.
///
/// **Different here.** The work outlives the connection and the brain: a
/// call's outcome is a record in the limb's own timeline, shipped to
/// whichever connection comes next. Reaching another machine is the SSH Max
/// already trusts, with no relay and no tokens of the harness's own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limb {
    pub id: LimbId,
    /// Unique among limbs: `brain` for the brain, a machine's limb by the
    /// machine's name, as Max and the agents call it.
    pub name: String,
    pub host: Host,
    /// The tool sets it offers, each with its settings: what this limb is
    /// for.
    pub offers: Vec<Offer>,
}

impl Limb {
    /// The offer of the tool set `set`, if this limb has one.
    pub fn offer(&self, set: &str) -> Option<&Offer> {
        self.offers.iter().find(|o| o.set == set)
    }
}

/// One tool set a limb offers, and its settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// The tool set's name, as roles name it: on a machine, `workspace`,
    /// `user` (Max's own instructions and skills), `browser`, `builds`; in
    /// the brain, `meta`, `web`, `attention`, `roles`.
    pub set: String,
    /// Its settings as JSON, read by the limb alone: Max's config
    /// directory, the browser to launch, the unit to relaunch, the roles'
    /// directory. Anything the brain or a face needs from a tool set, the
    /// limb says in its [`ToolSet`].
    pub config: String,
}

/// Where a limb runs, and so how the brain reaches it.
///
/// The brain dials, because it always knows where its limbs are and a limb
/// on another machine often cannot reach it: it runs `harness-limb attach
/// <limb>` on the limb's host, locally as it is and remotely through `ssh
/// <destination>`, which starts the limb if no instance holds its state
/// and then carries the link over its stdin and stdout. Who may connect is
/// whoever can run that command: the user on the machine, and over SSH,
/// whoever SSH lets in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Host {
    /// The brain's own process: the limb the brain is.
    Brain,
    /// A process on the brain's machine, as the brain's user.
    #[default]
    Local,
    /// A process on another machine, as the user `ssh <destination>`
    /// logs in as, by Max's own SSH configuration.
    Ssh { destination: String },
}

/// An agent's working directory: where its [`WORKSPACE`] tools run and its
/// relative paths resolve, on one limb.
///
/// **Concept.** Max (29 Oct): "An agent has a context and a working
/// directory and a role." Given when the agent is created
/// ([`crate::task::Origin::Fresh`]) and fixed for its life; its forks share
/// it. A fresh agent works in its parent's directory, a directory under it,
/// or a clone of its own ([`Branch`]). Several agents may work in one
/// directory, as forks and siblings do: the stale-write refusal and the
/// notices are how they coordinate, and those are the limb's, by absolute
/// path, so they also reach an agent working in a directory that nests in
/// another's.
///
/// The limb makes a directory ready before an agent's first root (makes it,
/// or clones it) and says what it found ([`Prepared`]), when the brain asks
/// for it ([`LimbRow::Workdir`]).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Workdir {
    pub limb: LimbId,
    /// As the brain named it: absolute, or `~/…` in the home of the limb's
    /// user, with no `.` or `..` in it ([`normal`]). The limb resolves it.
    pub dir: PathBuf,
    /// The branch it is a clone for, made if `dir` does not exist yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<Branch>,
}

/// A branch of the agent's own, in a clone made for it: its instructions
/// say the clone is its own, and it commits and pushes there.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Branch {
    /// What `git clone` is given: a URL or a path.
    pub from: String,
    /// Checked out from `origin/<name>` if it exists, otherwise made from
    /// `origin/<base>`.
    pub name: String,
    pub base: String,
}

/// What a limb found making a working directory ready: stated as
/// `Fact::Workdir { limb, dir, prepared }` when it first makes the directory
/// ready, on each new link, and whenever its instructions change, so a new
/// root always has them as they are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prepared {
    /// It exists. Its instructions say where it is, which branch is the
    /// agent's own, and what its `AGENTS.md` says, each observing the
    /// version it was read at, so a root embodies them and a change is a
    /// notice.
    Ready { instructions: Vec<Instruction> },
    /// It cannot be made, in the words of what failed (git's, for a clone).
    /// The agents starting there wait, showing why, while every other
    /// agent on the limb carries on; the limb tries again on its next link.
    Failed { why: String },
}

/// `dir` with every `.` removed and every `..` taken back with the part
/// before it, without looking at the disk, so one directory has one name
/// however it was reached. A `..` with nothing before it to take back is
/// kept.
pub fn normal(dir: &Path) -> PathBuf {
    let mut parts: Vec<Component> = Vec::new();
    for part in dir.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir) => {}
                _ => parts.push(part),
            },
            part => parts.push(part),
        }
    }
    parts.iter().collect()
}

/// What a limb says it offers, as a fact on each new link
/// (`Fact::Described`): a [`ToolSet`] for each of its offers. The brain's
/// own limb is described by the brain's code at each root instead, since
/// what `meta` offers depends on the role holding it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    pub sets: Vec<ToolSet>,
}

impl Description {
    pub fn set(&self, name: &str) -> Option<&ToolSet> {
        self.sets.iter().find(|s| s.name == name)
    }
}

/// One tool set as its limb describes it: its tools, and the instructions
/// for using them, which every root of an agent that holds the set
/// renders. So tools and the knowledge to use them arrive together, and
/// change together through notices.
///
/// "Tool set" is Max's word for what a role holds: "The role includes the
/// agent instructions, the skill set, the tool set." The `user` set carries
/// his own instructions and the index of his skills, so a role holding it
/// has them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSet {
    /// As roles and offers name it.
    pub name: String,
    /// Instructions for using it: its conventions, the machine and user it
    /// runs as, Max's instructions and the skills index. A working
    /// directory's own instructions are its [`Prepared::Ready`].
    pub instructions: Vec<Instruction>,
    pub tools: Vec<Tool>,
}

/// Which row of a limb's view of the brain.
///
/// Written as a string (`call:{id}`, `workdir:{dir}`), since a JSON
/// object's keys are.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LimbKey {
    Call(crate::id::CallId),
    /// The bytes of a value a call row names.
    Value(crate::id::ValueId),
    Watch,
    Definition,
    Role(RoleId),
    Superseded,
    Workdir(PathBuf),
}

/// One row of what a limb holds of the brain: what the brain wants of it,
/// as state, derived from the brain's facts.
///
/// **Concept.** A limb is a follower like a face. It ships its own facts
/// (its description, what it found in each directory, its settlements, the
/// changes it saw) as records of its own replica; the brain ships back the
/// rows it should act on. There is no run message, so nothing is sent
/// again after a reconnection: the limb gets its whole view on connecting
/// and runs each call it holds a row for and has no record of, at most once
/// per id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimbRow {
    /// Run this call: dispatched to this limb and not settled. A limb that
    /// can check `call.expects` refuses it when they have moved, before it
    /// has any effect. `stopping`: stop it gracefully and end what is left
    /// at this deadline; it settles as interrupted once nothing it started
    /// is left. A deadline is only ever brought forward.
    ///
    /// `first_sent_to`: the instance it was first sent to. A limb that is
    /// not that instance and has no record of the call treats it as cut off
    /// mid-run (rerun if its tool may be, otherwise settled as unknown),
    /// never as new.
    ///
    /// `owner`: who the call is for now, which moves when it is passed up
    /// the tree. `None` from a brain that did not say.
    ///
    /// `dir`: the caller's working directory, for a [`WORKSPACE`] tool:
    /// the calling agent's, the chat's for Max's own calls. A command runs
    /// there and relative paths resolve against it.
    Call {
        call: Call,
        stopping: Option<Timestamp>,
        first_sent_to: ReplicaId,
        owner: Option<Owner>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<PathBuf>,
    },
    /// Watch these resources: what live contexts embody. Report, as a
    /// [`crate::resource::Change`], each whose version is none of these,
    /// and from then on each change as it happens.
    Watch {
        known: Vec<crate::resource::Observation>,
    },
    /// A value's bytes.
    Value(Bytes),
    /// The limb's current definition, so a new version reaches the running
    /// limb, and starting one needs nothing but its id. A limb from before
    /// limbs were per machine, linked only to settle the calls it was
    /// running, is shipped its own old definition.
    Definition(Defined),
    /// The current version of a role read from this limb's resources: what
    /// the `roles` set restates only when its files differ.
    Role(Role),
    /// The brain would start the executable `by` for this limb, not the
    /// one it runs: exit, releasing its state, once nothing it runs would
    /// die with it. The brain's next attach then starts `by`.
    Superseded { by: BuildId },
    /// Make this directory ready, and keep its instructions current: a
    /// directory a live agent works in here, or that an unsettled call
    /// runs in. Gone when nothing needs it, and the limb forgets it.
    /// `agents`: those working there, by name, which is what their
    /// commands' scopes are described as.
    Workdir {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch: Option<Branch>,
        #[serde(default)]
        agents: BTreeMap<TaskId, String>,
    },
}

/// A limb's definition as a record holds it: one from before limbs were
/// per machine (each tool set its own limb, a workspace a limb per
/// directory), which a brain reads and never states, or one now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Defined {
    Legacy(legacy::Limb),
    Limb(Limb),
}

impl std::fmt::Display for LimbKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LimbKey::Call(id) => write!(f, "call:{id}"),
            LimbKey::Value(id) => write!(f, "value:{id}"),
            LimbKey::Watch => f.write_str("watch"),
            LimbKey::Definition => f.write_str("definition"),
            LimbKey::Role(id) => write!(f, "role:{id}"),
            LimbKey::Superseded => f.write_str("superseded"),
            LimbKey::Workdir(dir) => write!(f, "workdir:{}", dir.display()),
        }
    }
}

impl std::str::FromStr for LimbKey {
    type Err = ParseError;

    /// Also reads `environment:{id}`, what a role's row was called before.
    fn from_str(text: &str) -> Result<LimbKey, ParseError> {
        let invalid = ParseError {
            expected: "a row: call:{id}, value:{id}, watch, definition, superseded, role:{id} or workdir:{dir}",
        };
        Ok(match text.split_once(':') {
            None if text == "watch" => LimbKey::Watch,
            None if text == "definition" => LimbKey::Definition,
            None if text == "superseded" => LimbKey::Superseded,
            Some(("call", id)) => LimbKey::Call(id.parse()?),
            Some(("value", id)) => LimbKey::Value(id.parse()?),
            Some(("role" | "environment", id)) => LimbKey::Role(id.parse()?),
            Some(("workdir", dir)) if !dir.is_empty() => LimbKey::Workdir(PathBuf::from(dir)),
            _ => return Err(invalid),
        })
    }
}

impl Serialize for LimbKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LimbKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// What the brain ships a limb over its link.
pub type LimbDown = harness_substrate::Down<LimbKey, LimbRow, crate::fact::Refusal, ()>;

/// What a limb ships the brain over its link: its own records, and a
/// running command's output so far.
pub type LimbUp = harness_substrate::Up<crate::schema::Harness, crate::live::Live>;
