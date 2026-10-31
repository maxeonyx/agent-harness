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
//! **Built so far.** All of the above. [`Probation`] is today's trial on
//! real sessions, which nothing judges any more; the face still shows it.
//!
//! **Limbs move with the brain, once it is proven.** A [`Launched`] names
//! the executable it would start for each kind of limb. Once its build is
//! proven, a limb running other bytes is superseded
//! ([`crate::state::State::superseded`]): its view says so, and it exits
//! once nothing it holds would die with it, so the brain's next attach
//! starts it from the new build. Waiting for the proof means a build that
//! goes back never has to move a limb back, and probation shows the new
//! brain working with the limbs it inherited, the mixed case. A limb
//! running a long command stays on its build until the command ends;
//! facts are the contract between the two.
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

use imbl::OrdSet;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::call::CallEvent;
use crate::fact::Fact;
use crate::id::{CallId, ModelCallId, ParseError, ValueId};
use crate::model_call::{Ending, ModelCallEvent};

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
    /// found while it checks the rest, and what he sends waits in his tab.
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
        Pathway::Serve,
        Pathway::Requests,
        Pathway::Response,
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

/// What a build has shown since it took over: whether it works end to end.
/// Today's trial on real sessions, which the gate replaces ([`Start`]):
/// once nothing real runs before a build is proven, there is nothing left
/// for it to judge.
///
/// **Concept.** The end of a build's trial. Before it took over, on its own
/// processes, it showed that it folds a copy of the database, renders
/// every live session's next request in a shape the provider accepts
/// (checked by the provider's token count, which generates nothing),
/// streams a minimal response, serves a page that loads, and makes a round
/// trip through a fresh limb of its own build. What only the live brain can
/// show is that it keeps working with the real sessions and the limbs it
/// inherited: a response to a request of this span, and a settlement by a
/// limb of a call made in this span. A refusal the build is to blame for
/// ([`blames_build`]) before both is its failure; so are its process
/// ending and a request of the span left unanswered past the watcher's
/// deadline, which the watcher sees and this fold does not. Once proven,
/// later refusals are ordinary trouble.
///
/// The span's first request is due at once, because the relaunch call's
/// settlement wakes the session that made it; so a span that stays
/// unanswered is a hang, not an idle brain.
///
/// The running brain folds it into [`crate::state::State::probation`] for
/// the face. Nothing else reads it: the builds limb judges a new build by
/// the installed link and the unit's process, never by its records.
///
/// **Vision.** "A broken build bricks nothing", and the advocate's answer
/// to what proves a build: "its first full tool round trip; before taking
/// over it renders every live session's next request without sending; and
/// a way back that never runs the new build's code."
///
/// **Existing harnesses.** Restart into the new code and hope: nothing
/// tells a build that starts from one that works.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Probation {
    /// The requests this span sent.
    sent: OrdSet<ModelCallId>,
    /// The calls made in this span.
    made: OrdSet<CallId>,
    responded: Option<ModelCallId>,
    round_trip: Option<CallId>,
    refused: Option<Refused>,
}

impl Probation {
    /// Fold one fact of the span, said by a limb if `by_limb`. Facts of
    /// other spans are not shown it.
    pub(crate) fn see(&mut self, by_limb: bool, trial: Trial) {
        if self.refused.is_some() || self.proven().is_some() {
            return;
        }
        match trial {
            Trial::Sent(id) => {
                self.sent.insert(id);
            }
            Trial::Responded(id) if self.sent.contains(&id) => {
                self.responded.get_or_insert(id);
            }
            Trial::Failed(refused)
                if self.sent.contains(&refused.request) && blames_build(refused.status) =>
            {
                self.refused = Some(refused);
            }
            Trial::Made(id) => {
                self.made.insert(id);
            }
            Trial::Settled(id) if by_limb && self.made.contains(&id) => {
                self.round_trip.get_or_insert(id);
            }
            _ => {}
        }
    }

    /// Where the trial stands.
    pub fn verdict(&self) -> Verdict {
        if let Some(refused) = &self.refused {
            return Verdict::Refused(refused.clone());
        }
        match self.proven() {
            Some((response, round_trip)) => Verdict::Proven {
                response,
                round_trip,
            },
            None => Verdict::Proving {
                responded: self.responded,
                round_trip: self.round_trip,
            },
        }
    }

    fn proven(&self) -> Option<(ModelCallId, CallId)> {
        Some((self.responded?, self.round_trip?))
    }
}

/// Where a build's [`Probation`] stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Not yet shown both: what it has shown so far.
    Proving {
        responded: Option<ModelCallId>,
        round_trip: Option<CallId>,
    },
    /// It works end to end.
    Proven {
        response: ModelCallId,
        round_trip: CallId,
    },
    /// The provider refused a request it rendered before it was proven.
    Refused(Refused),
}

/// A fact a [`Probation`] reads: what a span's requests and calls did.
///
/// **Concept.** The judge of a build is the build before it, which may not
/// know every word the new build's records hold: a request carries every
/// span it rendered, a response its content. So what the trial reads is
/// only kinds and ids, and how a request ended, read either from a fact
/// this build folds ([`Trial::of`]) or from the [`Glimpsed`] record of one
/// it skips: one it cannot parse, or one its rules refuse because it is
/// about something it skipped (a response to a request it could not read).
/// So a record whose payload names a variant this build does not know
/// still shows a request sent, answered or refused, and a call made or
/// settled; a settlement it skips counts as a limb's when it came from
/// neither the brain nor one of Max's faces. A [`Launched`] it skips still
/// begins a span, read by its name alone, so the trial is the new span's.
/// Not covered: a new kind of fact, a new way for a request to end, a
/// change to an id's form, or a renamed [`crate::fact::Fact::Launched`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trial {
    Sent(ModelCallId),
    Responded(ModelCallId),
    /// The provider failed the request with a status.
    Failed(Refused),
    Made(CallId),
    Settled(CallId),
}

impl Trial {
    pub fn of(fact: &Fact) -> Option<Trial> {
        Some(match fact {
            Fact::ModelCall(id, ModelCallEvent::Sent(_)) => Trial::Sent(*id),
            Fact::ModelCall(id, ModelCallEvent::Ended { ending, .. }) => match ending {
                Ending::Responded { .. } => Trial::Responded(*id),
                Ending::Failed {
                    status: Some(status),
                    message,
                    ..
                } => Trial::Failed(Refused {
                    request: *id,
                    status: *status,
                    message: message.clone(),
                }),
                _ => return None,
            },
            Fact::Call(id, CallEvent::Made(_)) => Trial::Made(*id),
            Fact::Call(id, CallEvent::Settled(_)) => Trial::Settled(*id),
            _ => return None,
        })
    }
}

/// What any build reads of a [`crate::fact::Batch`] it cannot parse: the
/// harness's [`harness_substrate::Schema::Glimpse`]. Each fact is read on
/// its own, so one this build has no word for hides only itself.
#[derive(Debug, Deserialize)]
pub struct Glimpsed {
    facts: Vec<Glance>,
}

impl Glimpsed {
    /// What it read of each fact, by the fact's index in the record.
    pub(crate) fn facts(&self) -> impl Iterator<Item = (u32, Glanced)> + '_ {
        (0..)
            .zip(&self.facts)
            .filter_map(|(i, glance)| match glance {
                Glance::Read(fact) => Some((i, fact.glanced()?)),
                Glance::Other(IgnoredAny) => None,
            })
    }
}

/// What any build reads of one fact of a record it skips.
pub(crate) enum Glanced {
    /// A brain began deciding: a new span, whatever its build.
    Launched,
    Trial(Trial),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Glance {
    Read(GlancedFact),
    Other(IgnoredAny),
}

/// [`Fact`]'s shape, keeping only what a glimpse reads; any other content
/// of the variants it names is passed over unread.
#[derive(Debug, Deserialize)]
enum GlancedFact {
    Launched(IgnoredAny),
    ModelCall(ModelCallId, GlancedModelCall),
    Call(CallId, GlancedCall),
}

#[derive(Debug, Deserialize)]
enum GlancedModelCall {
    Sent(IgnoredAny),
    Ended { ending: GlancedEnding },
}

#[derive(Debug, Deserialize)]
enum GlancedEnding {
    Responded(IgnoredAny),
    Failed {
        status: Option<u16>,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
enum GlancedCall {
    Made(IgnoredAny),
    Settled(IgnoredAny),
}

impl GlancedFact {
    fn glanced(&self) -> Option<Glanced> {
        Some(Glanced::Trial(match self {
            GlancedFact::Launched(_) => return Some(Glanced::Launched),
            GlancedFact::ModelCall(id, GlancedModelCall::Sent(_)) => Trial::Sent(*id),
            GlancedFact::ModelCall(id, GlancedModelCall::Ended { ending }) => match ending {
                GlancedEnding::Responded(_) => Trial::Responded(*id),
                GlancedEnding::Failed {
                    status: Some(status),
                    message,
                } => Trial::Failed(Refused {
                    request: *id,
                    status: *status,
                    message: message.clone(),
                }),
                GlancedEnding::Failed { status: None, .. } => return None,
            },
            GlancedFact::Call(id, GlancedCall::Made(_)) => Trial::Made(*id),
            GlancedFact::Call(id, GlancedCall::Settled(_)) => Trial::Settled(*id),
        }))
    }
}

/// A request of a span on probation that the provider refused, in its
/// words: what the agent and Max read about why the harness went back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refused {
    pub request: ModelCallId,
    pub status: u16,
    pub message: String,
}

/// Whether a provider's refusal with `status` is the fault of the build
/// that rendered the request: a client error, other than a timeout or a
/// limit, which the same request from another build may not get.
pub fn blames_build(status: u16) -> bool {
    (400..500).contains(&status) && !matches!(status, 408 | 429)
}
