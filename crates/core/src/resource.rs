//! Resources: the changing things a context can embody a version of.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, Digest, LimbId, ValueId};

/// A changing thing owned by a limb, which a context can hold a copy of.
///
/// **Concept.** A file, a skill, the `AGENTS.md` hierarchy, a tool's schema,
/// an environment's role file ([`crate::environment::Environment::source`]),
/// Max's presence, Max's queue, a limb's running instance, a test watcher's
/// last run, a page's event log. An [`Observation`] says "this context embodies
/// `resource` at `version`"; it comes from the root an agent was given
/// ([`crate::agent::Start::observes`]) and from every call result a request
/// renders ([`crate::call::Output::observes`]). A context knows a version
/// exactly when it was sent something that embodies it.
///
/// At each request the brain compares what the context embodies with the
/// current versions and says once what has moved since it last said
/// ([`crate::context::Note::Stale`]). Nothing is pushed at change time:
/// twenty agents holding a skill Max edits cost nothing until each next
/// sends, and a change reverted before then is never mentioned. The notice
/// carries the boundary ("`design` changed after you loaded it"), or the
/// content the owning limb attached to the change when looking again is not
/// how the context should learn it ([`Change::content`]).
///
/// This is also how an environment speaks between turns. A standing agent
/// listening for its environment is woken by a change to anything its
/// context embodies, and its next request is that notice: the attention
/// scheduler embodies Max's queue, so a change to the queue is its next
/// turn, and changes that arrive while it thinks fold into the one after.
///
/// Keyed by limb, not environment, so a write through one environment is
/// seen by every context that read the same thing through another. Kinds
/// are the owning limb's words, as limb kinds are: the core reads none of
/// them.
///
/// **Vision.** Context updates and progressive disclosure, with the
/// testimony in `context-updates.md` that a silently refreshed system prompt
/// is worse than a stale one, because every claim in it loses its date; the
/// shared-workspace race (a sibling's write to a file this context read
/// becomes a notice naming the sibling, and a write based on a stale read is
/// refused); Max's edits during his turn; "your shell state is gone"; the
/// attention scheduler's input.
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
/// model anything to carry; and one mechanism carries both "your copy is
/// out of date" and "something happened you are listening for".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceKey {
    pub limb: LimbId,
    /// What kind of thing it is, in its limb's words: `file`, `skill`,
    /// `instructions`, `tool`, `instance`, `queue`, `presence`.
    pub kind: String,
    /// Within the kind: a path, a skill name, a tool name. Empty for a
    /// singleton such as the instance.
    pub name: String,
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

/// A resource moved to a new version. Stated by the limb that owns it, or
/// by Max for his own (his presence).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub resource: ResourceKey,
    pub version: Version,
    /// The call that wrote it, when the limb was running one: the notice
    /// says "changed by your sibling X" or "edited by Max", read off the
    /// call's caller. `None` for a change from outside the harness, such as
    /// Max's own editor.
    pub by: Option<CallId>,
    /// What a context should be shown rather than told to look again: a
    /// tool's new schema, the new queue, a watcher's result, "away until
    /// 9". The limb decides, as the owner of what the change means; the
    /// renderer may still only announce it.
    pub content: Option<ValueId>,
}
