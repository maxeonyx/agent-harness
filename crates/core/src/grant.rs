//! Grants: Max's standing answers to the permission asks of a hosted
//! engine, which take the place of permission prompts.
//!
//! **Concept.** Claude Code asks before some tool uses: its own defaults,
//! the rules of its settings, and the asks of a managed policy that nothing
//! below it can remove. A prompt held for a person is a denial in all but
//! name that also blocks the agent, often for hours, so the harness never
//! holds one. The limb hosting the run answers each `can_use_tool` at once,
//! from its grant rows ([`crate::limb::LimbRow::Grant`]), even while the
//! brain is down:
//!
//! 1. A grant whose rule matches answers it: allow, or deny with its
//!    message. A once-grant is then used up.
//! 2. Otherwise it is denied in band. The tool use settles as
//!    [`crate::call::Outcome::Denied`], carrying the [`Ask`], and the
//!    model reads that Max has been shown it and will say if he allows
//!    it: it carries on with the rest, or ends its turn saying what it
//!    needs.
//!
//! A denied call that no grant answers is a proposal on Max's map and in
//! his queue (a view, never a fact). He answers it with a grant, the enduring
//! fix first: allow or refuse this kind of use from now on, for the scope he
//! picks; or once; or "I'll do it", a once-deny whose message says so. A
//! grant that answers a live task's call reaches it as
//! [`crate::delivery::Prompt::Granted`], so it can try again. Grants that
//! allow, and are not for policy rules, also go to each new run as
//! `--allowedTools`, so they are never asked about at all.
//!
//! **Vision.** Max (12 Nov): "A permission prompt should only ever be
//! brought up in combination with an enduring fix." The repo's settled
//! rule: an environment is defined with its permissions, and extending them
//! is changing that definition. A grant is that change, made in one tap
//! from the thing that needed it.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, RoleId, TaskId};

pub use crate::id::GrantId;

/// Max's standing answer to a kind of tool use by hosted engines: a fact he
/// states ([`crate::fact::Fact::Grant`]), named by its event
/// ([`GrantId`]), until he revokes it ([`crate::fact::Fact::Revoke`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The uses it answers, in Claude Code's rule syntax:
    /// `Bash(gh pr merge:*)`, `WebFetch(domain:docs.rs)`, `Edit(/etc/**)`.
    pub rule: String,
    pub scope: GrantScope,
    pub answer: Answer,
    pub uses: Uses,
    /// It answers asks that work's managed policy raised, not only those of
    /// Claude Code's defaults and other settings. Refused unless the
    /// brain's domain lets Max's grants answer them
    /// ([`crate::domain::Domain::policy`]), and never as a standing allow
    /// when it lets him answer each one only.
    #[serde(default)]
    pub policy: bool,
    /// The denied call that led to it, so that proposal is closed and its
    /// task is told.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<CallId>,
}

/// Whose asks a grant answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantScope {
    /// Every hosted task of this brain.
    Domain,
    /// The tasks of one role.
    Role(RoleId),
    /// One task and everything under it.
    Tree(TaskId),
}

/// What a grant says to the uses it matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Answer {
    Allow,
    /// Refused, and the model reads `message`: what to do instead ("leave
    /// merges to me: end with the PR ready"), or "Max will do it".
    Deny {
        message: String,
    },
}

/// How long a grant answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Uses {
    /// The next matching use only.
    Once,
    /// Every matching use until Max revokes it.
    Standing,
}

/// Why a hosted engine would have prompted, kept with the denied call
/// ([`crate::call::Outcome::Denied`]): what Max's proposal shows him, and
/// what the grant he makes from it starts from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    /// The rule that asked, when one did: `Bash(rm -rf *)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    pub source: RuleSource,
    /// Claude Code's own words for why it would ask.
    pub reason: String,
    /// The rules Claude Code suggests would allow it: the enduring fixes
    /// the proposal offers first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

/// Where the ask came from, which decides who may answer it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSource {
    /// An ask rule of work's managed policy (`policySettings`): only a
    /// grant with [`Grant::policy`] answers it.
    Policy,
    /// An ask rule of Claude Code's other settings: user, project, local.
    Settings,
    /// No rule allowed it, and the permission mode asks.
    Mode,
    /// Claude Code's classifier, in [`crate::engine::PermissionMode::Auto`].
    Classifier,
}
