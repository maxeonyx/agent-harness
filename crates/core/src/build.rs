//! Builds, launches and relaunches: which code the harness runs, and how
//! that changes while sessions run on it.
//!
//! **The model.** A [`BuildId`] is a version of the code, as a value: the
//! digest of one executable that can serve every role. A version of the
//! brain is a span of the brain's timeline, opened by a [`Launched`] fact
//! naming its build, so "which version ran this" is a position in the data
//! model, like every other version. There is one brain, deciding with one
//! build at a time, and every session moves to a new build when it lands:
//! a fix reaches the session that wrote it, and the sessions running
//! beside it, at their next step.
//!
//! What a session keeps across builds is its data, never code: its
//! contexts are the bytes that were sent (so the provider's cache still
//! hits), its environment is a versioned fact, and the tool definitions its
//! live root declared are [`Contract`]s, served by a limb instance that
//! still offers them, of whichever build that is. So a session's program
//! (Max's "each session its own kind of program") is three facts: its
//! environment's version, the contracts its root pins, and the build of
//! the brain span it is in. Nothing more is stored for it.
//!
//! **A broken build bricks nothing.** Two rules make that structural:
//!
//! - Every build opens every database. A record this build has no word for
//!   is [`harness_substrate::Unread`]: held, skipped, reported. So going
//!   back is always possible.
//! - Make before break. A small supervisor that outlives every build (it
//!   owns the listening sockets, the brain processes and the installed
//!   builds) starts the candidate beside the running brain. The candidate
//!   opens the database and folds it while the old brain serves; only then
//!   does the old brain write a [`Handover`] and stop deciding, and the
//!   candidate continue the brain's timeline with [`Launched`]. Until the
//!   candidate is [`Proven`] by a request that responded, the supervisor
//!   keeps the old build ready, and restores it when the candidate dies or
//!   its requests are refused: [`Launch::RolledBack`].
//!
//! The agent that asked for the relaunch made a call ([`Relaunch`]); the
//! new brain's [`Launched`] settles it, and its session carries on, now on
//! the new build. A relaunch that never happened is the call's failure, in
//! band ([`Failure`]). Every session sees a launch as a dated notice, as it
//! sees any resource of its environment change.
//!
//! **Where it goes next.** Programs as modules: the brain becomes a kernel
//! (substrate, contexts, tasks, calls, limbs, the provider) hosting each
//! session's program (its fold of its own facts, its rendering and policy,
//! its tools and its UI) as a WebAssembly module whose digest is its
//! version, bound per session and moved at a turn boundary. Then a program
//! change restarts nothing and sessions can differ in code, not only in
//! data. It waits for the kernel/program boundary to stop moving: until
//! then every module would pin an interface that is still changing. What
//! keeps the path open: a program's identity is a value like a build's, an
//! environment already names what its tasks run (`Environment::role`, a
//! name the running build resolves, becomes a module's digest), and render
//! and policy stay pure functions of a view to the records they say.
//!
//! **Bx.** Shared: an artifact's identity is the digest of its bytes
//! ([`BuildId`] is Bx's A), a build record says what made it ([`Build`]),
//! and anything that pins an artifact pins it by that digest (a launch, a
//! contract). Not shared yet: Bx's signed records, its resolved-input
//! identity (I: the lockfile and toolchain a build used), and its module
//! interfaces. A build here is one executable, and the domain trusts the
//! limb that states it.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::id::{CallId, ModelCallId, ParseError, PlaceId, ValueId};

/// One build of the harness: the digest of its executable's bytes.
///
/// **Concept.** A version of the code, as a value. It is installed once,
/// under its digest, where the supervisor runs it from, and never
/// overwritten; a `cargo build` writing its output again changes nothing
/// that runs. The same identity names it in the brain, in a limb and (when
/// it is a module) in the browser.
///
/// **Vision.** Self-modification "as soon as possible", with no "live
/// bricking": the code that is running cannot be replaced underneath it,
/// and the code that ran before can always be run again. "Content
/// addressed, same artifact in brain, limb, and browser."
///
/// **Existing harnesses.** Claude Code and its kind update a package in
/// place and run whatever is on disk at the next start. Nix and Bx name
/// artifacts by content.
///
/// **Different here.** The running harness, and every record of what ran,
/// name the build by its content, so a rollback is a fact pointing at bytes
/// that still exist, not a reinstall.
///
/// **Bx.** Bx's artifact identity (A), for one artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BuildId(pub ValueId);

impl BuildId {
    /// The build whose executable is `bytes`.
    pub fn of(bytes: &[u8]) -> BuildId {
        BuildId(ValueId::of(bytes))
    }

    /// The first eight hex digits: enough to tell builds apart in a log.
    pub fn short(&self) -> String {
        self.to_string()[..8].to_owned()
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

/// That a build was made and installed: stated by the limb that made it.
///
/// **Concept.** The build record: which code an artifact came from, and who
/// asked for it. An agent can rebuild any build it can name, and Max can
/// see what each launch ran.
///
/// **Vision.** Builds and history kept in the domain DB; an agent editing
/// the harness and relaunching it while Max watches.
///
/// **Bx.** A build record (B: the definition built; A: the artifact),
/// unsigned and without Bx's resolved inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub id: BuildId,
    pub source: Source,
    /// The call that made it, when an agent did.
    pub by: Option<CallId>,
}

/// The code a build was made from: a commit, and the working tree's changes
/// on top of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub commit: String,
    /// The diff from `commit` to the tree that was built, when it differed:
    /// an agent's uncommitted edit is still reproducible.
    pub diff: Option<ValueId>,
}

/// The brain began deciding with `build`: the start of a version's span in
/// the brain's timeline.
///
/// **Concept.** "Which version" as a position: everything the brain
/// decided between this and the next `Launched` was decided by `build`. It
/// replaces the build name the launch fact carried, and adds how the build
/// came to run.
///
/// **Vision.** "A version of a program is just subsumed by the versioning
/// that's already implemented in the data model"; a relaunch the agent who
/// asked for it sees settle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launched {
    pub build: BuildId,
    pub how: Launch,
}

/// How a build came to run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Launch {
    /// Nothing was deciding: the machine started, or the brain before it
    /// died after it was proven. A request that brain left running ended
    /// with it.
    Started,
    /// It took over from a running brain, which handed over. `by` is the
    /// [`Relaunch`] call this settles, unless Max relaunched by hand.
    Relaunched { from: BuildId, by: Option<CallId> },
    /// The supervisor restored the build `from` replaced, because `from`
    /// failed before it was proven.
    RolledBack { from: BuildId, why: Failure },
}

/// The arguments of a call asking the supervisor to move the brain onto
/// another build.
///
/// **Concept.** The one way the harness's code changes while it runs: a
/// tool call like any other, made by an agent (or Max), settled by the new
/// brain's [`Launched`] or refused with a [`Failure`].
///
/// `replaces` names the build the caller built on. When another relaunch
/// landed first the call fails [`Failure::Stale`] rather than silently
/// reverting the other one's change: two agents improving the harness at
/// once each learn about the other.
///
/// **Vision.** Self-modification: "the agent edits the code, relaunches,
/// and continues the same session".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relaunch {
    pub to: BuildId,
    pub replaces: BuildId,
}

/// The old brain's last decision: the candidate `to` decides from here.
///
/// **Concept.** The break in make-before-break, written only once the
/// candidate has opened the database. Requests the old brain still had in
/// flight are not cut: `runner`, a place the old process keeps for them,
/// settles `runs` when they end, and the new brain includes those endings
/// as it includes a limb's. So a relaunch costs no session a request it
/// had already paid to start.
///
/// **Vision.** "Graceful shutdown waits for API requests", without making
/// every other session wait for the slowest one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handover {
    pub to: BuildId,
    /// The place that settles `runs`; none when nothing was in flight.
    pub runner: Option<PlaceId>,
    pub runs: Vec<ModelCallId>,
}

/// A request sent by `build` responded: the build works end to end, and is
/// what the supervisor goes back to.
///
/// **Concept.** The end of a build's probation. Opening the database proves
/// it can read; a response proves it can render a request the provider
/// accepts and fold what came back. Until this, a failure means rollback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proven {
    pub build: BuildId,
    pub by: ModelCallId,
}

/// Why a build did not take over, or did not keep running.
///
/// **Concept.** What an agent reads when its relaunch failed, and what
/// Max reads under pressure when the harness went back: in band, beside
/// the launch it concerns, never only in a log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    /// Another relaunch landed first: `running` is not what the call
    /// replaces.
    Stale { running: BuildId },
    /// No build with that id is installed.
    Missing,
    /// Its process ended: before it was ready (it could not open the
    /// database) or after it took over, before it was proven. `stderr` is
    /// the end of what it printed.
    Exited {
        code: Option<i32>,
        stderr: Option<ValueId>,
    },
    /// The provider refused a request it sent.
    Refused {
        call: ModelCallId,
        status: Option<u16>,
        message: String,
    },
    /// It was not ready, or not proven, in time.
    TimedOut,
}

/// A tool definition a live root declared, pinned by its bytes.
///
/// **Concept.** The part of a session's program that outlives a build. A
/// root's `tools` are sent with every request under it; while the root
/// lives, a call to `tool` is served only by a limb instance that offers
/// exactly `definition`. When the current build's limb offers a different
/// one, the call goes to an instance of the build that last offered it
/// (installed builds are never overwritten). When none can, it is refused
/// in band as retired, and the task's next request starts a new root (a
/// cache break the task is told about) rather than calling a tool whose
/// meaning changed under it. Today a route names a tool and a limb; it
/// gains `definition`, and a limb's hello lists the contracts it offers.
///
/// **Vision.** "We still want the old tool calls to be usable by the agent
/// until the next handover/compaction or the next cache break."
///
/// **Existing harnesses.** Tools are resolved by name at call time, so a
/// changed tool silently serves calls the model made against its old
/// schema.
///
/// **Bx.** A requirement pinned by content, like a lock's pin of an
/// artifact; the interface is the definition's bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Contract {
    pub tool: String,
    pub definition: ValueId,
}
