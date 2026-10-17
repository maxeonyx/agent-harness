//! Builds and launches: which code the brain runs.
//!
//! A [`BuildId`] is a version of the code, as a value: the digest of the
//! brain's executable. A version of the brain is a span of the brain's
//! timeline, opened by a [`Launched`] naming its build, so "which build
//! decided this" is a position in the records, like every other version.
//!
//! Relaunching onto another build while sessions run (the supervisor, the
//! handover, a build's probation and rollback) is not built; `Launch` has
//! the two ways a build comes to run today.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::{CallId, ParseError, ValueId};

/// One build of the brain: the digest of its executable's bytes.
///
/// **Concept.** A version of the code, as a value, so a record of what ran
/// names bytes, not a version string two different builds can share.
///
/// **Vision.** Self-modification without "live bricking": "content
/// addressed, same artifact in brain, limb, and browser".
///
/// **Existing harnesses.** Claude Code and its kind name a release by its
/// version number, and a local build by nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BuildId(pub ValueId);

impl BuildId {
    /// The build whose executable is `bytes`.
    pub fn of(bytes: &[u8]) -> BuildId {
        BuildId(ValueId::of(bytes))
    }
}

impl fmt::Display for BuildId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for BuildId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<BuildId, ParseError> {
        text.parse().map(BuildId)
    }
}

/// The brain began deciding with `build`: the start of a version's span
/// in the brain's timeline.
///
/// **Concept.** Everything the brain decided between this and the next
/// `Launched` was decided by `build`. It is also the whole of restart:
/// what the brain before it had in flight is what
/// [`crate::state::State::todo`] says to redo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launched {
    pub build: BuildId,
    pub how: Launch,
}

/// How a build came to run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Launch {
    /// Nothing was deciding: the machine started, or the brain before it
    /// shut down or died. A request that brain left running ended with it.
    Started,
    /// It took over from the running brain of build `from`. `by` is the
    /// call that asked for it, which this settles, unless Max relaunched
    /// by hand.
    Relaunched { from: BuildId, by: Option<CallId> },
}
