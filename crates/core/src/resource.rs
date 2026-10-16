//! Resources: the changing things a context can embody a version of.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, Digest, LimbId, TaskId};

/// A changing thing owned by a limb, which a context can hold a copy of.
///
/// **Concept.** A file, a skill, the `AGENTS.md` hierarchy, a tool's schema,
/// an agent kind's role prompt, the catalogue of startable kinds, Max's
/// presence, a limb's running instance. An [`Observation`] says "this
/// context embodies `resource` at `version`"; it comes from the root a task
/// was given ([`crate::task::Created::observes`]) and from every call result
/// a request renders ([`crate::call::Output::observes`]). A context knows a
/// version exactly when it was sent something that embodies it.
///
/// At each request the brain compares what the context embodies with the
/// current versions and says once what has moved since it last said
/// ([`crate::context::Note::Stale`]). Nothing is pushed at change time:
/// twenty agents holding a skill Max edits cost nothing until each next
/// sends, and a change reverted before then is never mentioned. The notice
/// carries the boundary ("`design` changed after you loaded it"), not the
/// new content, unless [`Disclosure`] says otherwise.
///
/// Keyed by limb, not environment, so a write through one environment is
/// seen by every context that read the same thing through another.
///
/// **Vision.** Context updates and progressive disclosure, with the
/// testimony in `context-updates.md` that a silently refreshed system prompt
/// is worse than a stale one, because every claim in it loses its date; the
/// shared-workspace race (a sibling's write to a file this context read
/// becomes a notice naming the sibling, and a write based on a stale read is
/// refused); Max's edits during his turn; "your shell state is gone".
///
/// **Existing harnesses.** OpenCode re-reads instructions and skills each
/// step and appends the diff when one changed
/// (`ref-opencode/packages/core/src/instruction-discovery.ts:139`); Codex
/// diffs `AGENTS.md` and the skills catalogue as "world state"
/// (`ref-codex/codex-rs/core/src/context/world_state/agents_md.rs:44`);
/// Goose re-reads `AGENTS.md` into a fresh system prompt each request and
/// says nothing (`ref-goose/crates/goose/src/agents/prompt_manager.rs:90`),
/// the testimony's failure exactly; Oh My Pi caches instruction files until
/// an explicit reset (`ref-oh-my-pi/packages/coding-agent/src/capability/fs.ts:10`),
/// and rejects a stale edit only if the model echoes the hash it was shown
/// (`ref-oh-my-pi/crates/pi-edit/src/modes/hashline/patcher.rs:434`).
///
/// **Different here.** None tracks a file a context read, a skill it loaded,
/// or what a fork inherited. Here the harness records what each context
/// observed, so neither the notice nor the stale-write refusal costs the
/// model anything to carry.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceKey {
    pub limb: LimbId,
    pub kind: ResourceKind,
    /// Within the kind: a path, a skill name, a tool name. Empty for a
    /// singleton such as the instance.
    pub name: String,
}

/// What kind of thing a resource is, which decides how a change to it is
/// disclosed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceKind {
    File,
    Skill,
    /// The index of skills rendered into a root: names and descriptions.
    SkillIndex,
    /// An instruction file rendered into a root, such as an `AGENTS.md`.
    Instructions,
    Tool,
    /// An agent kind's role prompt and settings.
    AgentKind,
    /// The environments and agent kinds a spawn can name.
    Catalogue,
    /// Max's queue as the attention limb holds it.
    Queue,
    /// Max's stated availability ("away until 9"). Observed by user-facing
    /// contexts and the scheduler, so a change reaches each at its next
    /// request without causing one.
    Presence,
    /// The running instance. A new version means in-memory state was lost.
    Instance,
    /// A kind a limb plugin defines.
    Other,
}

/// A version of a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Version {
    /// A content digest where the limb can compute one, otherwise an opaque
    /// token that changes when the content does.
    Is(Digest),
    /// It no longer exists: a deleted file, a removed skill.
    Gone,
}

/// A context embodies `resource` at `version`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub resource: ResourceKey,
    pub version: Version,
}

/// A limb reports that a resource moved to a new version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub resource: ResourceKey,
    pub version: Version,
    /// Who changed it, when that was a call the harness knows of, so a notice
    /// can say "changed by sibling X" or "edited by Max".
    pub by: Option<Writer>,
}

/// Who wrote a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Writer {
    Task(TaskId, CallId),
    Max(CallId),
}

/// How much of a change a context is told.
///
/// The vision's rule is the minimum that invalidates the agent's
/// understanding: it should know that looking again is an option. So
/// [`Disclosure::Boundary`] holds only where looking again *is* an option,
/// when the context's binding reaches the limb that owns the resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Disclosure {
    /// Not mentioned until the context is replaced: a skill description in
    /// the index.
    Silent,
    /// "This changed after you saw it": a loaded skill, a read file,
    /// `AGENTS.md`, the catalogue.
    Boundary,
    /// The new content, because the old one cannot be acted on safely or the
    /// context has no way to look: a tool schema, a lost instance, Max's
    /// presence, anything its binding cannot read.
    Full,
}

impl Disclosure {
    /// The default for a kind of resource, given whether the context can
    /// read it again. A policy the brain may refine; it lives here so every
    /// renderer agrees.
    pub fn of(kind: ResourceKind, can_look: bool) -> Disclosure {
        match kind {
            ResourceKind::SkillIndex => Disclosure::Silent,
            ResourceKind::Tool | ResourceKind::Instance | ResourceKind::Presence => {
                Disclosure::Full
            }
            _ if can_look => Disclosure::Boundary,
            _ => Disclosure::Full,
        }
    }
}
