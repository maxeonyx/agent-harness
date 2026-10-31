use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::record::{Durability, Folding, Included, Record, Schema, Unread, What};
use crate::storage::Storage;
use crate::time::{ReplicaId, Time, Timestamp};

/// The replica that decides: it folds its own records and its followers'
/// into one state, in one order of its own, and records that order.
///
/// **Concept.** On main, the brain. Every rule that two replicas could
/// otherwise break concurrently (a task ends once, a draft is delivered
/// once, a limb settles only its own calls) is checked here, in the order
/// this replica includes records, so concurrent acts never need merging: the
/// first included wins and the second is refused in band. A decider
/// holds what it includes for as long as its schema's [`Durability`] says,
/// and reopening folds exactly what it holds, following the verdicts it
/// recorded, so a restart, a relaunch onto a new build, and collection
/// leave the state the same.
///
/// **Vision.** One decider per object (the overview's aspect 3), and on
/// main the brain owns every shared object; a brain that "picks up where it
/// left off"; Bailis's invariant confluence: what merges needs no decider,
/// and what does not is decided by the one replica that owns it.
pub struct Decider<S: Schema, St: Storage<S>> {
    here: ReplicaId,
    next: u64,
    state: S::State,
    standing: BTreeMap<ReplicaId, Standing<S::Refusal>>,
    /// The record holding each [`Durability::Latest`] key.
    latest: BTreeMap<S::Key, Holder>,
    /// Each follower's newest [`Included`], which says how far it was
    /// included, so it is kept even once its record is collected.
    last: BTreeMap<ReplicaId, Last>,
    storage: St,
}

/// How far a decider has included one follower, and what it refused: the
/// part of a decider's state every view to that follower carries.
///
/// **Concept.** The follower's half of causality. A follower's record `n`
/// is pending until `upto` reaches it, then included, or refused with the
/// reason the decider recorded. That is how a tab knows its act is in
/// without being told: it holds a view as of a decider time that includes
/// it. Derived from the decider's [`Included`] records, never stated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "R: Serialize", deserialize = "R: Deserialize<'de>"))]
pub struct Standing<R> {
    /// The follower's newest record included.
    pub upto: u64,
    /// Each included record the rules refused, by its `n`.
    pub refused: BTreeMap<u64, R>,
}

/// Where one of a follower's records stands with its decider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status<'a, R> {
    /// Not yet included: in flight, or the decider is away.
    Pending,
    /// Included and folded.
    Included,
    /// Included and refused: it is held as a fact and had no effect.
    Refused(&'a R),
}

/// A record this build does not fold as the build that included it did.
/// Reopening skips it, so the state is this build's reading of the past;
/// the record stays, so the build that wrote it still reads it as before.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence<R> {
    pub at: Time,
    pub why: Diverged<R>,
}

/// Why a record is skipped on reopening.
#[derive(Clone, Debug, PartialEq)]
pub enum Diverged<R> {
    /// The recorded verdict accepted it; this build's fold refuses it.
    Refused(R),
    /// This build cannot read it, or the receipt that included it
    /// ([`crate::Unread`]).
    Unread(String),
}

impl<R: std::fmt::Display> std::fmt::Display for Diverged<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Diverged::Refused(why) => write!(f, "this build refuses it: {why}"),
            Diverged::Unread(why) => write!(f, "this build cannot read it: {why}"),
        }
    }
}

/// Why the decider did not write its own record.
#[derive(Debug)]
pub enum Error<R, E> {
    /// Its own fold refused it: a bug in whatever wrote it.
    Refused(R),
    Storage(E),
}

impl<R: std::fmt::Display, E: std::fmt::Display> std::fmt::Display for Error<R, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Refused(why) => write!(f, "refused: {why}"),
            Error::Storage(e) => write!(f, "storage: {e}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Holder {
    at: Time,
    /// The decider's record that included it, if it is a follower's.
    included: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct Last {
    n: u64,
    /// Whether the record it includes is still held.
    held: bool,
    refused: bool,
}

/// What one record does to what the decider holds, worked out before it is
/// stored and settled after.
struct Plan<K> {
    drop: Vec<Time>,
    latest: Option<(K, Holder)>,
    orphan: Option<ReplicaId>,
    last: Option<(ReplicaId, Last)>,
}

impl<R> Default for Standing<R> {
    fn default() -> Self {
        Standing {
            upto: 0,
            refused: BTreeMap::new(),
        }
    }
}

impl<R> Standing<R> {
    /// Where the follower's record `n` stands.
    pub fn status(&self, n: u64) -> Status<'_, R> {
        if n > self.upto {
            Status::Pending
        } else {
            self.refused
                .get(&n)
                .map_or(Status::Included, Status::Refused)
        }
    }
}

fn unread<R>(at: Time, why: &str) -> Divergence<R> {
    Divergence {
        at,
        why: Diverged::Unread(why.to_owned()),
    }
}

/// What a follower's record says, or what this build cannot read of it.
fn said<S: Schema>(what: &What<S>) -> Option<Result<&S::Said, &Unread>> {
    match what {
        What::Said(said) => Some(Ok(said)),
        What::Unread(unread) if unread.of.is_none() => Some(Err(unread)),
        What::Included(_) | What::Unread(_) => None,
    }
}

impl<S: Schema, St: Storage<S>> Decider<S, St> {
    /// The decider at `here`, folding everything `storage` holds. A record
    /// this build cannot read, or folds differently from the build that
    /// included it, is skipped and returned, for the embedding to surface.
    #[allow(clippy::type_complexity)]
    pub fn open(
        here: ReplicaId,
        storage: St,
    ) -> Result<(Self, Vec<Divergence<S::Refusal>>), St::Error> {
        let mut held = BTreeMap::new();
        let mut own = BTreeMap::new();
        for record in storage.load()? {
            if record.at.replica == here {
                own.insert(record.at.n, record);
            } else {
                held.insert(record.at, record);
            }
        }
        let mut decider = Decider {
            here,
            next: own.keys().next_back().map_or(1, |n| n + 1),
            state: S::State::default(),
            standing: BTreeMap::new(),
            latest: BTreeMap::new(),
            last: BTreeMap::new(),
            storage,
        };
        let mut divergences = Vec::new();
        for record in own.values() {
            // A receipt this build cannot read still includes its record,
            // as accepted: the verdict it holds is not this build's to know.
            let read;
            let (at, what, included) = match &record.what {
                What::Said(said) => (record.at, Some(Ok(said)), None),
                What::Unread(u) if u.of.is_none() => (record.at, Some(Err(u)), None),
                What::Included(included) => {
                    let what = held.get(&included.of).map(|r| &r.what);
                    (included.of, what.and_then(said), Some(included))
                }
                What::Unread(u) => {
                    divergences.push(unread(record.at, &u.why));
                    let of = u.of.expect("a receipt names what it includes");
                    read = Included { of, refused: None };
                    (of, None, Some(&read))
                }
            };
            let said = match what {
                Some(Ok(said)) => Some(said),
                Some(Err(u)) => {
                    divergences.push(unread(at, &u.why));
                    None
                }
                None => None,
            };
            let mut durability = None;
            if let Some(said) = said {
                let folding = Folding {
                    here: record.at,
                    clock: record.clock,
                    at,
                    said,
                };
                match included.and_then(|i| i.refused.as_ref()) {
                    // The recorded verdict stands.
                    Some(why) => S::refused(&mut decider.state, folding, why),
                    None => {
                        let mut state = decider.state.clone();
                        match S::fold(&mut state, folding) {
                            Ok(()) => {
                                decider.state = state;
                                durability = Some(S::durability(at, said));
                            }
                            Err(why) => {
                                divergences.push(Divergence {
                                    at,
                                    why: Diverged::Refused(why),
                                });
                            }
                        }
                    }
                }
            }
            let holder = Holder {
                at,
                included: included.map(|_| record.at.n),
            };
            let include = included.map(|i| (i.of.replica, i.refused.is_some()));
            let plan = decider.plan(durability, holder, include);
            decider.settle(plan);
            if let Some(included) = included {
                decider.stand(included);
                if !held.contains_key(&included.of)
                    && let Some(last) = decider.last.get_mut(&included.of.replica)
                {
                    last.held = false;
                }
            }
        }
        Ok((decider, divergences))
    }

    pub fn here(&self) -> ReplicaId {
        self.here
    }

    /// This replica's newest time: what a view of it is as of.
    pub fn head(&self) -> Time {
        Time {
            replica: self.here,
            n: self.next - 1,
        }
    }

    /// The time its own next record will have, so what that record
    /// creates can be named before it is said.
    pub fn next(&self) -> Time {
        Time {
            replica: self.here,
            n: self.next,
        }
    }

    /// What every record held amounts to.
    pub fn state(&self) -> &S::State {
        &self.state
    }

    pub fn storage(&self) -> &St {
        &self.storage
    }

    /// How far this decider has included every replica it has included
    /// anything of.
    pub fn standings(&self) -> &BTreeMap<ReplicaId, Standing<S::Refusal>> {
        &self.standing
    }

    /// How far this decider has included `replica`, and what it refused.
    pub fn standing(&self, replica: ReplicaId) -> Standing<S::Refusal> {
        self.standing.get(&replica).cloned().unwrap_or_default()
    }

    /// Write this replica's own next record: `said` is given the record's time,
    /// so what it creates is named by it. Refused records are not written:
    /// the decider's own refusal is a bug in its writer, not a race.
    pub fn say(
        &mut self,
        clock: Timestamp,
        said: impl FnOnce(Time) -> S::Said,
    ) -> Result<Time, Error<S::Refusal, St::Error>> {
        let at = Time {
            replica: self.here,
            n: self.next,
        };
        let said = said(at);
        let mut state = self.state.clone();
        let folding = Folding {
            here: at,
            clock,
            at,
            said: &said,
        };
        S::fold(&mut state, folding).map_err(Error::Refused)?;
        let durability = S::durability(at, &said);
        let plan = self.plan(Some(durability), Holder { at, included: None }, None);
        let record = Record {
            at,
            clock,
            what: What::Said(said),
        };
        self.storage
            .commit(&[record], &plan.drop)
            .map_err(Error::Storage)?;
        self.settle(plan);
        self.state = state;
        self.next += 1;
        Ok(at)
    }

    /// Include a follower's record: fold it, or record why not, in a record
    /// of this replica's own at the returned time. A record already included
    /// (a resend after a reconnect) is skipped and returns `None`. A gap
    /// before it (records the follower collapsed, or a decider restored
    /// from a backup) is not waited on: the follower's own [`crate::Log`]
    /// sees that it was restored.
    pub fn include(
        &mut self,
        clock: Timestamp,
        record: Record<S>,
    ) -> Result<Option<Time>, St::Error> {
        let from = record.at.replica;
        if from == self.here || record.at.n <= self.standing(from).upto {
            return Ok(None);
        }
        let here = Time {
            replica: self.here,
            n: self.next,
        };
        let mut state = self.state.clone();
        let (refused, durability) = match &record.what {
            What::Said(said) => {
                let folding = Folding {
                    here,
                    clock,
                    at: record.at,
                    said,
                };
                match S::fold(&mut state, folding) {
                    Ok(()) => (None, Some(S::durability(record.at, said))),
                    Err(why) => {
                        state = self.state.clone();
                        S::refused(&mut state, folding, &why);
                        (Some(why), None)
                    }
                }
            }
            // What this build cannot read, or another decider's receipt:
            // held as a fact, folded by nobody here.
            What::Unread(_) | What::Included(_) => (None, Some(Durability::Kept)),
        };
        let holder = Holder {
            at: record.at,
            included: Some(here.n),
        };
        let plan = self.plan(durability, holder, Some((from, refused.is_some())));
        let included = Included {
            of: record.at,
            refused,
        };
        let receipt = Record {
            at: here,
            clock,
            what: What::Included(included.clone()),
        };
        self.storage.commit(&[record, receipt], &plan.drop)?;
        self.settle(plan);
        self.stand(&included);
        self.state = state;
        self.next += 1;
        Ok(Some(here))
    }

    fn stand(&mut self, included: &Included<S::Refusal>) {
        let standing = self.standing.entry(included.of.replica).or_default();
        standing.upto = included.of.n;
        if let Some(why) = &included.refused {
            standing.refused.insert(included.of.n, why.clone());
        }
    }

    /// What holding a record newly folded (`durability`, if it folded),
    /// and, for a follower's, its [`Included`], collects.
    fn plan(
        &self,
        durability: Option<Durability<S::Key>>,
        holder: Holder,
        include: Option<(ReplicaId, bool)>,
    ) -> Plan<S::Key> {
        let mut plan = Plan {
            drop: Vec::new(),
            latest: None,
            orphan: None,
            last: None,
        };
        if let Some(Durability::Latest(key)) = durability {
            if let Some(old) = self.latest.get(&key) {
                plan.drop.push(old.at);
                if let Some(m) = old.included {
                    let replica = old.at.replica;
                    if self.last.get(&replica).is_some_and(|l| l.n == m) {
                        plan.orphan = Some(replica);
                    } else {
                        plan.drop.push(self.time(m));
                    }
                }
            }
            plan.latest = Some((key, holder));
        }
        if let (Some((replica, refused)), Some(n)) = (include, holder.included) {
            if let Some(last) = self.last.get(&replica) {
                let orphaned = plan.orphan == Some(replica) || !last.held;
                if orphaned && !last.refused {
                    plan.drop.push(self.time(last.n));
                    if plan.orphan == Some(replica) {
                        plan.orphan = None;
                    }
                }
            }
            let last = Last {
                n,
                held: true,
                refused,
            };
            plan.last = Some((replica, last));
        }
        plan
    }

    fn settle(&mut self, plan: Plan<S::Key>) {
        if let Some(replica) = plan.orphan
            && let Some(last) = self.last.get_mut(&replica)
        {
            last.held = false;
        }
        if let Some((key, holder)) = plan.latest {
            self.latest.insert(key, holder);
        }
        if let Some((replica, last)) = plan.last {
            self.last.insert(replica, last);
        }
    }

    fn time(&self, n: u64) -> Time {
        Time {
            replica: self.here,
            n,
        }
    }
}
