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
//! **Relaunching** is an act of the builds limb, a plugin limb of the
//! brain's machine whose resource is the brain's own service. An agent whose
//! environment holds it calls its `relaunch` tool on a clone; the limb
//! builds and installs that clone's commit and proves the new build beside
//! the running brain without touching it: on a copy of the live database
//! it folds every record, renders every live session's next request and
//! has the provider check each one without generating anything, sends one
//! minimal request, and serves the face for a headless page load; the old
//! build then opens the same copy, so the way back is known to read what
//! the new build leaves behind; and a scratch brain of the new build, on
//! the scripted model, makes a full round trip through a limb of the new
//! build. Only then does it stop the running brain (it drains), start the
//! new build on the same database and settle the call. The new build is
//! then on [`Probation`] until it shows what only the live brain can.
//! Until then the limb, a process of the old build, watches its resource
//! by folding the database with its own code, and on any failure starts
//! the old build again from its installed bytes, telling the session why
//! as a change of that resource. So the way back never runs the new
//! build's code, and a broken build bricks nothing.
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
//! to add one.
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
use serde::{Deserialize, Serialize};

use crate::call::CallEvent;
use crate::fact::{Author, Fact, Stated};
use crate::id::{CallId, ModelCallId, ParseError, ValueId};
use crate::model_call::{Ending, ModelCallEvent};

/// One executable of a build: the digest of its bytes.
///
/// **Concept.** A version of the code, as a value, so a record of what ran
/// names bytes, not a version string two different builds can share. The
/// brain's [`Launched`] names its own; a limb's [`crate::environment::Hello`]
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
/// that's already implemented in the data model"; limbs that keep running
/// their old build after a relaunch, round 2's finding, are a view of this
/// fact rather than a mechanism of their own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launched {
    pub build: BuildId,
    /// What it was built from, when it knows.
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub limbs: BTreeMap<String, BuildId>,
    pub how: Launch,
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
    /// the new one failed its [`Probation`]. `None` when Max restarted it
    /// himself.
    Relaunched { from: BuildId, by: Option<CallId> },
}

/// What a build has shown since it took over: whether it works end to end.
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
/// One fold, compiled twice: the running brain folds it into
/// [`crate::state::State::probation`] for the face, and the builds limb,
/// built from the code before, folds the same records with its own code to
/// decide whether to go back. So the judge of a new build is never the new
/// build. The watch outlives the relaunch call, which settles at takeover
/// so the session that asked can make the request that proves it; the
/// limb owns the watch as part of its resource, the brain's service, as a
/// browser limb owns a tab.
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
    /// Fold one record of the span. Records of other spans are not shown it.
    pub fn see(&mut self, record: &Stated) {
        if self.refused.is_some() || self.proven().is_some() {
            return;
        }
        match record.fact {
            Fact::ModelCall(id, ModelCallEvent::Sent(_)) => {
                self.sent.insert(*id);
            }
            Fact::ModelCall(id, ModelCallEvent::Ended { ending, .. }) if self.sent.contains(id) => {
                match ending {
                    Ending::Responded { .. } => {
                        self.responded.get_or_insert(*id);
                    }
                    Ending::Failed {
                        status: Some(status),
                        message,
                        ..
                    } if blames_build(*status) => {
                        self.refused = Some(Refused {
                            request: *id,
                            status: *status,
                            message: message.clone(),
                        });
                    }
                    Ending::Failed { .. }
                    | Ending::Limited { .. }
                    | Ending::Cut { .. }
                    | Ending::Lost => {}
                }
            }
            Fact::Call(id, CallEvent::Made(_)) => {
                self.made.insert(*id);
            }
            Fact::Call(id, CallEvent::Settled(_))
                if matches!(record.by, Author::Limb(_)) && self.made.contains(id) =>
            {
                self.round_trip.get_or_insert(*id);
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
    /// It works end to end: the builds limb moves the brain's installed
    /// link to it, so a restart runs it, and it is what a later failure
    /// goes back to.
    Proven {
        response: ModelCallId,
        round_trip: CallId,
    },
    /// The provider refused a request it rendered before it was proven:
    /// the builds limb goes back, and the build before it sends that
    /// request again ([`crate::state::State::todo`]).
    Refused(Refused),
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
