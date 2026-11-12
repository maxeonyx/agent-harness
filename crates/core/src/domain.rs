//! Domains: what Max calls a brain, what runs there, and which other brains
//! its face takes him to.
//!
//! **Concept.** Each domain is one brain with its own database: home, and
//! work on the work VM. They are never merged. A face shows one brain's map;
//! the brains it lists as [`Peer`]s are one switch away, each the other
//! brain's own page, so no page ever holds two domains' data or runs
//! against a brain built from other code. A peer is listed only by the
//! domain that may see it: work lists home, home lists nothing, so no home
//! device ever learns of work.
//!
//! **Vision.** Domains and federation (milestone F): work sees home, home
//! devices never see work. Max (11 Nov): "I can have my home domain in the
//! same app, maybe a different map."

use serde::{Deserialize, Serialize};

/// A brain's domain, as Max defines it ([`crate::fact::Fact::Domain`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    /// What the switcher calls it: "Work", "Home".
    pub name: String,
    pub engines: Engines,
    /// Whether Max's grants may answer the asks of a managed policy
    /// ([`crate::grant::Grant::policy`]).
    #[serde(default)]
    pub policy: PolicyAsks,
    /// The other brains its face takes him to, in the switcher's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<Peer>,
}

/// Which engines may run a task in this domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engines {
    #[default]
    Any,
    /// Every model call goes through Claude Code: a task whose role is not
    /// hosted ([`crate::engine::Engine::Brain`]) is refused at creation.
    ClaudeCodeOnly,
}

/// How Max may answer the asks of a managed policy: a question for his
/// employer, so the domain says it once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAsks {
    /// Each one, by its own once-grant: he decides every use, later.
    #[default]
    Each,
    /// Standing grants too: his decision, made in advance, answers them.
    Standing,
}

/// Another brain a face served by this one takes Max to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    /// Its face: `https://harness.maxeonyx.com/`.
    pub url: String,
}
