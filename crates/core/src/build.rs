//! Builds, launches and relaunches: which code the harness runs, and how
//! that changes while sessions run on it.
//!
//! **The model.** A build is the set of executables installed from one
//! [`Source`], each named by the digest of its bytes ([`BuildId`]). A
//! version of the brain is a span of the brain's timeline, opened by a
//! [`Launched`] naming its build, so "which build decided this" is a
//! position in the records, like every other version. There is one brain
//! per domain, deciding with one build at a time, and every session moves
//! to a new build when it takes over: a fix reaches the session that wrote
//! it, and the sessions beside it, at their next step. What a session keeps
//! across builds is its data: its contexts are the bytes that were sent, so
//! the provider's cache still hits, and the tool definitions its root
//! declared are in those bytes.
//!
//! **Opening.** Relaunching is an act of the `builds` tool set, offered by
//! the limb of the brain's machine, whose resource is the brain's own
//! service: an agent whose role holds it calls its `relaunch` tool on a
//! clone, and the limb builds and installs that clone's commit. The new
//! build then proves itself before any real session runs on it, and writes
//! nothing to the domain's database until it has, so a build that fails
//! goes back having changed nothing, and nothing runs twice. It is proven
//! in two places, over the [`Pathway`]s each can reach.
//!
//! First the rehearsal, beside the running brain, which carries on: the
//! builds limb, a process of the running build, has the new build run its
//! gate on a copy of the live database and serve that copy's face
//! read-only, loads the page in headless Chrome, has two scratch brains of
//! the new build make a round trip through a limb of each build it will
//! meet, and compares how both builds read the sign-in. A failure there
//! costs nobody anything. Then the gate, on the live database: the running
//! brain drains and stops, and the new build starts as a
//! [`Start::Candidate`], which checks the gate's pathways on what the brain
//! before left, while its face shows that state read-only. Only once they
//! pass does it point the domain's installed link at itself and write its
//! [`Launched`]: the commit point, after which every session resumes on it.
//! If one fails, it says which and why and exits, and the unit restarts
//! the build before, on the bytes it left.
//!
//! The builds limb learns the outcome from what any build shows, whatever
//! shape the new build's records take: the link moved, the unit's process
//! ended (in the candidate's own words), or a deadline passed (it ends the
//! candidate). So the judge never reads the new build's records, and the
//! `relaunch` call settles once, with the outcome.
//!
//! **The way back.** For a short window after the new build opens, the
//! builds limb watches its process. If it ends, the limb points the link at
//! the build before and starts it the same way it rolled forward, and this
//! is never refused: the older build folds what it can of what the newer
//! one wrote, and skips and logs the rest. Max (31 Oct): "Sending two is
//! better than bricking." Whether a way back is lossless is checked before
//! the deploy: the rehearsal folds what the scratch brains wrote with the
//! running build's code. If it skips nothing, the relaunch is two-way, and
//! a way back loses nothing. If it skips any, the relaunch is one-way: a way
//! back would skip such records and may run a call a second time. The
//! `relaunch` call's result arrives only after the relaunch, too late to
//! warn, so the call refuses a one-way relaunch at rehearsal, naming what
//! the running build could not read, and goes ahead only when called again
//! with `one_way`, as with `back`. Max: "The tool call rejects initially
//! and says, this would be one way, call it again with the one-way flag."
//!
//! The face shows where the latest relaunch stands from the `relaunch`
//! call alone: under way, then the stage it ended at, in the builds limb's
//! words.
//!
//! **Built so far.** All of the above but the page: the rehearsal loads no
//! page in headless Chrome.
//!
//! **Limbs move with the brain.** A [`Launched`] names the executable it
//! would start for each kind of limb, and a build writes it only once its
//! gate has passed, so from then a limb running other bytes is superseded
//! ([`crate::state::State::superseded`]): its view says so, and it exits
//! once nothing it holds would die with it, so the brain's next attach
//! starts it from the new build. On the way back the build before writes
//! its own [`Launched`], and limbs move back the same way. A limb running a
//! long command stays on its build until the command ends; facts are the
//! contract between the two.
//!
//! **Where it goes next.** A version as a place: the candidate folds the
//! database while the old brain serves and takes over its socket with no
//! gap; the old process finishes the requests it had in flight as a
//! replica of its own instead of holding up every session while it drains;
//! a call is served by a limb instance offering the definition its root
//! pinned, so an old instance outlives a relaunch only for those calls; the
//! watcher follows a filtered replica of the brain's records over its link
//! instead of reading the database file; and one executable serves every
//! role, so a build is one digest. None of these changes a type here except
//! to add one; the candidate's gate is the readiness check of the first.
//!
//! **Bx.** Shared: an artifact's identity is the digest of its bytes
//! ([`BuildId`] is Bx's artifact identity for one executable); a build is
//! installed once under its name and never overwritten, so going back is
//! running bytes that still exist, not rebuilding them; anything that pins
//! a build pins it by digest. Not shared: a build is named by its commit on
//! disk, not by its resolved inputs (lockfile, toolchain), so two builds of
//! one commit with another toolchain are told apart only by their digests;
//! nothing is signed, and the domain trusts the limb that installs.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::id::{CallId, ParseError, ValueId};

/// One executable of a build: the digest of its bytes.
///
/// **Concept.** A version of the code, as a value, so a record of what ran
/// names bytes, not a version string two different builds can share. The
/// brain's [`Launched`] names its own; a limb's [`crate::limb::Hello`]
/// names the one it runs, and the two are compared only to the executable
/// of the same kind the brain would start.
///
/// **Vision.** Self-modification without "live bricking": the code that
/// runs cannot be replaced underneath it, and the code that ran before can
/// always be run again. "Content addressed, same artifact in brain, limb,
/// and browser."
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

/// The code a build was made from: a commit, and whether the tree it was
/// built from had changes on top of it.
///
/// **Concept.** What Max and an agent read to know which code runs, and
/// what the builds limb compares to refuse a relaunch that would undo
/// another's: a build whose commit does not descend from the running one
/// goes back, and only a call that says so may do that. A build with
/// changes cannot be relaunched onto (it could not be built again); it is
/// what a test brain runs.
///
/// **Vision.** An agent editing the harness and relaunching it while Max
/// watches: he sees which commit took over, not a digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// The commit's hash, as `git rev-parse --short=12` gives it.
    pub commit: String,
    pub dirty: bool,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.commit)?;
        if self.dirty {
            f.write_str("+dirty")?;
        }
        Ok(())
    }
}

impl FromStr for Source {
    type Err = ParseError;

    /// `abc123def456` or `abc123def456+dirty`, as `harness-brain --version`
    /// prints it.
    fn from_str(text: &str) -> Result<Source, ParseError> {
        let (commit, dirty) = match text.strip_suffix("+dirty") {
            Some(commit) => (commit, true),
            None => (text, false),
        };
        if commit.is_empty() || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseError {
                expected: "a commit's hash, then +dirty if the tree had changes",
            });
        }
        Ok(Source {
            commit: commit.to_owned(),
            dirty,
        })
    }
}

/// The brain began deciding with `build`: the start of a version's span in
/// the brain's timeline.
///
/// **Concept.** Everything the brain decided between this and the next
/// `Launched` was decided by `build`. It is also the whole of restart:
/// what the brain before it had in flight is what
/// [`crate::state::State::todo`] says to redo. `limbs` is the executable it
/// starts for each kind of plugin limb (`workspace`, `browser`, `builds`):
/// a limb connected with other bytes is superseded, and moves to these
/// once nothing it holds would die with it.
///
/// **Vision.** "A version of a program is just subsumed by the versioning
/// that's already implemented in the data model"; a limb still running an
/// older build after a relaunch is a view of this fact rather than a
/// mechanism of its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launched {
    pub build: BuildId,
    /// What it was built from, when it knows.
    pub source: Option<Source>,
    #[serde(default)]
    pub limbs: BTreeMap<String, BuildId>,
    pub how: Launch,
}

/// How a brain process comes to decide on its domain's database: at once,
/// or once it has shown that it works.
///
/// **Concept.** `Installed` is the build the domain's installed link names
/// (`NAME/build` beside the database): the last one to open it. It opens at
/// once: a restart, a reboot, the unit restarting it after a crash, a test
/// brain, a new database. `Candidate` is a relaunch's one start of a new
/// build onto the live database, told by [`Start::FLAG`]. It writes
/// nothing, not even its [`Launched`], until every pathway of the gate
/// ([`Pathway::GATE`]) has passed on that database; then it points the
/// installed link at itself and launches, and the sessions resume. If one
/// fails, it prints which and why and exits, and the unit's restart runs
/// the installed link: the build before, on the bytes it left, so nothing
/// ran on the candidate and nothing runs twice. A flag rather than the
/// environment, so a build too old to know it refuses to start instead of
/// opening ungated.
///
/// A `back` relaunch is a candidate too, so an older build opens a
/// database a newer one wrote only if it folds every record: going back is
/// lossless or it does not happen.
///
/// **Vision.** "While we want live editing, we don't want live bricking";
/// a restart is "just a pause and continue". Max (31 Oct): "the new brain
/// shouldn't start any agent loops until it's proven out the functionality
/// is all working, past its health check basically", and "we just
/// shouldn't resume real sessions until we've at least done a
/// comprehensive, end to end but not-real health check of some kind".
///
/// **Existing harnesses.** Restart into the new code and hope. A
/// blue-green deploy gates traffic on a readiness probe, but both versions
/// share a database the new one has already migrated, so going back is not
/// lossless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Installed,
    Candidate,
}

impl Start {
    /// The word on a brain's command line that makes its start a
    /// candidate's.
    pub const FLAG: &str = "--candidate";
}

/// One way a new build is shown to work before any real session runs on
/// it. A build passes when every pathway of both places has: the
/// rehearsal, beside the running brain, then the gate, on the live
/// database.
///
/// **Concept.** The gate's pathways are the candidate's own check, run in
/// both places: on a copy of the live database in the rehearsal
/// (`harness-brain --check`), and on the live database itself before the
/// candidate writes anything ([`Start::Candidate`]). They need nothing but
/// the database, the provider and a port, and they act on nothing. The
/// rehearsal's own need a browser, scratch brains or the running build's
/// code, so they run only there, and the running build judges them by what
/// any build shows: a page's exceptions, a file a round trip leaves, a
/// process's exit. A failure names its pathway, as `{pathway}: {why}`, in
/// the words of the program that failed.
///
/// No pathway runs a call or sends a request of anyone's real work: what
/// only real sessions can show, a logic bug on their own path, is not
/// proven before they run.
///
/// **Vision.** "Why can't we test like many different pathways as part of
/// our opening health check?" (Max, 31 Oct). Each pathway is a way a build
/// has broken, or would brick the harness by breaking what an agent needs
/// to fix it forward: a response, a tool call, a limb, the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pathway {
    /// Every record of the database folds, and none is skipped. The
    /// 31 Oct refusal of 6881 records; on the live database, the records
    /// written since the copy; and on a `back` relaunch, that the older
    /// build reads everything the newer one wrote.
    Fold,
    /// It decides without acting: what it would write first (its
    /// [`Launched`], its limbs' definitions), what it would do first
    /// ([`crate::state::State::todo`]), and the face's view of the whole
    /// state. A panic in what every launch computes from real state, which
    /// once open would loop on every restart.
    Decide,
    /// Each live session's next request, rendered and accepted by the
    /// provider's token count, which generates nothing. A refusal the build
    /// is to blame for ([`blames_build`]) fails it at once; provider
    /// trouble that is not the build's is retried for a minute. Signed
    /// out, each is only rendered.
    Requests,
    /// One short response streamed from the provider, ending in a tool use
    /// it is made to make: the wire, the stream and the parse of a tool
    /// call, which every session needs and an agent needs to relaunch
    /// again. Signed out, it waits: the build's first real request after
    /// the sign-in is its first test of them.
    Response,
    /// Its port bound with its certificate, and its face served read-only
    /// over what it folded, so the page Max has open shows the state it
    /// found while it checks, and what he sends waits in his tab.
    Serve,
    /// The rehearsal's: the page, served over the copy, loads in headless
    /// Chrome and renders its first view with no uncaught exception. A page
    /// that breaks on real state leaves Max unable to see or steer.
    Page,
    /// The rehearsal's: a scratch brain of the new build, on the scripted
    /// model and a fresh database, makes a full round trip through the
    /// brain's own limb and a machine's: its root spawns a child, the
    /// child's `bash` writes a token to a file, and the root's next call
    /// copies it; the running build checks the copy.
    RoundTrip(Through),
    /// The rehearsal's: both builds read the same sign-in from the login.
    /// A build that cannot read it would hold every session as signed out.
    SignIn,
}

/// Which build's machine limb a [`Pathway::RoundTrip`] goes through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Through {
    /// The running build's: what the new brain meets when it opens, since
    /// limbs move to its build only after that.
    Running,
    /// The new build's own: what its limbs move to once it has opened.
    Own,
}

impl Pathway {
    /// The gate's pathways, in the order a candidate checks them.
    pub const GATE: [Pathway; 5] = [
        Pathway::Fold,
        Pathway::Decide,
        Pathway::Requests,
        Pathway::Response,
        Pathway::Serve,
    ];
    /// The rehearsal's own, after the gate on a copy.
    pub const REHEARSAL: [Pathway; 4] = [
        Pathway::Page,
        Pathway::RoundTrip(Through::Running),
        Pathway::RoundTrip(Through::Own),
        Pathway::SignIn,
    ];
}

impl fmt::Display for Pathway {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Pathway::Fold => "fold",
            Pathway::Decide => "decide",
            Pathway::Requests => "requests",
            Pathway::Response => "response",
            Pathway::Serve => "serve",
            Pathway::Page => "page",
            Pathway::RoundTrip(Through::Running) => "round trip through the running build's limb",
            Pathway::RoundTrip(Through::Own) => "round trip through its own limb",
            Pathway::SignIn => "sign-in",
        })
    }
}

/// How a build came to run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Launch {
    /// Nothing was deciding: the machine started, or the brain before it
    /// died without shutting down. A request that brain left running ended
    /// with it.
    Started,
    /// It took over from a brain of build `from` that shut down (which may
    /// be the same build, restarted by hand). `by` is the builds limb's
    /// relaunch call that started it: on the way forward, onto the build the
    /// call asked for, and on the way back, onto the build before it when
    /// the new one's process ended in its window after opening. `None` when
    /// Max restarted it himself.
    Relaunched { from: BuildId, by: Option<CallId> },
}

/// Whether a provider's refusal with `status` is the fault of the build
/// that rendered the request: a client error, other than a timeout or a
/// limit, which the same request from another build may not get.
pub fn blames_build(status: u16) -> bool {
    (400..500).contains(&status) && !matches!(status, 408 | 429)
}
