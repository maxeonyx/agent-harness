//! Tool sets: what a limb does, each opened from one of its definition's
//! offers ([`harness_core::limb::Offer`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use harness_core::call::{Call, Outcome, Owner};
use harness_core::id::CallId;
use harness_core::limb::{self, Offer};
use harness_runtime::Cancel;

use crate::{Limit, Report, Rows, Voice};

/// One tool set as a limb runs it: its tools, the instructions for using
/// them, and what it does with the brain's rows.
///
/// Each is opened from its offer on its own ([`Open`]), and every call
/// whose tool it has is dispatched to it by name. The limb runs each call
/// on a thread of its own, in the order its agent wrote them
/// ([`crate::Calls`]), and a panic in [`ToolSet::run`] settles the call as
/// failed; a panic anywhere else fails only that step of the set.
pub trait ToolSet: Send + Sync {
    /// What a new link hears of it: its tools and instructions.
    fn describe(&self) -> limb::ToolSet;

    /// Take a new version of its offer's settings, which may still be
    /// those it has.
    fn define(&self, _config: &str) -> Result<(), String> {
        Ok(())
    }

    /// Act on the brain's rows as they now stand, other than the calls:
    /// the resources to watch, the directories to make ready. `connected`:
    /// the rows begin a link. Never blocks on a call.
    fn apply(&self, _rows: &Rows, _connected: bool) {}

    /// Whether running `tool` again, after an instance was cut off while
    /// running it, is the same as running it once.
    fn rerun(&self, _tool: &str) -> bool {
        false
    }

    /// Run one call to its outcome.
    fn run(&self, call: &Running<'_>, report: &mut Report) -> Outcome;

    /// Whether anything it holds besides its calls would die with the
    /// process: a browser it launched and the tabs in it, a relaunch it
    /// still watches.
    fn holds(&self) -> bool {
        false
    }

    /// Once no call runs and the limb is ending: end what it started, and
    /// wait until it has.
    fn close(&self) {}
}

/// How a tool set is opened from its offer: what it returns on failure is
/// why, which the limb says in the set's description.
pub type Open = fn(&Offer, &Opening) -> Result<Arc<dyn ToolSet>, String>;

/// What a tool set opens with.
pub struct Opening {
    pub voice: Arc<Voice>,
    /// Its own directory in the limb's state, made.
    pub state: PathBuf,
}

/// One call as a tool set runs it.
pub struct Running<'a> {
    pub id: CallId,
    pub call: &'a Call,
    pub input: &'a [u8],
    /// Who the call is for now ([`harness_core::limb::LimbRow::Call`]).
    pub owner: Option<Owner>,
    /// The caller's working directory, as the brain named it.
    pub dir: Option<&'a Path>,
    /// Stop it, gracefully, by the cancel's deadline.
    pub cancel: &'a Cancel,
    pub limit: &'a Limit,
    pub(crate) shutting_down: &'a AtomicBool,
}

impl Running<'_> {
    /// Whether the limb is shutting down, which is why a call stopped.
    pub fn shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }
}

/// The offer of the tool set `set` with `settings`, as its `config`.
pub fn offer(set: &str, settings: &impl serde::Serialize) -> Result<Offer, String> {
    Ok(Offer {
        set: set.into(),
        config: serde_json::to_string(settings)
            .map_err(|e| format!("the settings of `{set}`: {e}"))?,
    })
}

/// An offer's settings: its `config` as JSON, the default when it is empty.
pub fn settings<T: serde::de::DeserializeOwned + Default>(config: &str) -> Result<T, String> {
    if config.trim().is_empty() {
        return Ok(T::default());
    }
    serde_json::from_str(config).map_err(|e| format!("its settings: {e}"))
}
