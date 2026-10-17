use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::record::{Durability, Folding, Included, Record, Schema, What};
use crate::storage::Storage;
use crate::time::{PlaceId, Time, Timestamp};

/// The place that decides: it folds its own records and its followers'
/// into one state, in one order of its own, and records that order.
///
/// **Concept.** On main, the brain. Every rule that two places could
/// otherwise break concurrently (a task ends once, a draft is delivered
/// once, a limb settles only its own calls) is checked here, in the order
/// this place includes records, so concurrent acts never need merging: the
/// first included wins and the second is refused in band. A decider
/// holds what it includes for as long as its schema's [`Durability`] says,
/// and reopening folds exactly what it holds, following the verdicts it
/// recorded, so a restart, a relaunch onto a new build, and collection
/// leave the state the same.
///
/// **Vision.** One decider per object (the overview's aspect 3), and on
/// main the brain owns every shared object; a brain that "picks up where it
/// left off"; Bailis's invariant confluence: what merges needs no decider,
/// and what does not is decided by the one place that owns it.
pub struct Decider<S: Schema, St: Storage<S>> {
    here: PlaceId,
    next: u64,
    state: S::State,
    standing: BTreeMap<PlaceId, Standing<S::Refusal>>,
    /// The record holding each [`Durability::Latest`] key.
    latest: BTreeMap<S::Key, Holder>,
    /// Each follower's newest [`Included`], which says how far it was
    /// included, so it is kept even once its record is collected.
    last: BTreeMap<PlaceId, Last>,
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

/// A record the decider's recorded verdict accepted that this build's fold
/// refuses. Reopening skips it, so the state is this build's reading of
/// the past; the record stays, so an older build still reads it as before.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence<R> {
    pub at: Time,
    pub why: R,
}

/// Why the decider did not write its own record.
#[derive(Debug)]
pub enum Error<R, E> {
    /// Its own fold refused it: a bug in whatever wrote it.
    Refused(R),
    Storage(E),
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
    orphan: Option<PlaceId>,
    last: Option<(PlaceId, Last)>,
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

impl<S: Schema, St: Storage<S>> Decider<S, St> {
    /// The decider at `here`, folding everything `storage` holds. A record
    /// this build folds differently from the one that wrote it is skipped
    /// and returned, for the embedding to surface.
    #[allow(clippy::type_complexity)]
    pub fn open(
        here: PlaceId,
        storage: St,
    ) -> Result<(Self, Vec<Divergence<S::Refusal>>), St::Error> {
        let mut held = BTreeMap::new();
        let mut own = BTreeMap::new();
        for record in storage.load()? {
            if record.at.place == here {
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
            let (at, said, included) = match &record.what {
                What::Said(said) => (record.at, Some(said), None),
                What::Included(included) => {
                    let said = match held.get(&included.of).map(|r| &r.what) {
                        Some(What::Said(said)) if included.refused.is_none() => Some(said),
                        _ => None,
                    };
                    (included.of, said, Some(included))
                }
            };
            let mut durability = None;
            if let Some(said) = said {
                let folding = Folding {
                    here: record.at,
                    clock: record.clock,
                    at,
                    said,
                };
                match S::fold(&mut decider.state, folding) {
                    Ok(()) => durability = Some(S::durability(at, said)),
                    Err(why) => divergences.push(Divergence { at, why }),
                }
            }
            let holder = Holder {
                at,
                included: included.map(|_| record.at.n),
            };
            let include = included.map(|i| (i.of.place, i.refused.is_some()));
            let plan = decider.plan(durability, holder, include);
            decider.settle(plan);
            if let Some(included) = included {
                decider.stand(included);
                if !held.contains_key(&included.of)
                    && let Some(last) = decider.last.get_mut(&included.of.place)
                {
                    last.held = false;
                }
            }
        }
        Ok((decider, divergences))
    }

    pub fn here(&self) -> PlaceId {
        self.here
    }

    /// This place's newest time: what a view of it is as of.
    pub fn head(&self) -> Time {
        Time {
            place: self.here,
            n: self.next - 1,
        }
    }

    /// What every record held amounts to.
    pub fn state(&self) -> &S::State {
        &self.state
    }

    pub fn storage(&self) -> &St {
        &self.storage
    }

    /// How far this decider has included `place`, and what it refused.
    pub fn standing(&self, place: PlaceId) -> Standing<S::Refusal> {
        self.standing.get(&place).cloned().unwrap_or_default()
    }

    /// Write this place's own next record: `said` is given the record's time,
    /// so what it creates is named by it. Refused records are not written:
    /// the decider's own refusal is a bug in its writer, not a race.
    pub fn say(
        &mut self,
        clock: Timestamp,
        said: impl FnOnce(Time) -> S::Said,
    ) -> Result<Time, Error<S::Refusal, St::Error>> {
        let at = Time {
            place: self.here,
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
    /// of this place's own at the returned time. A record already included
    /// (a resend after a reconnect) is skipped and returns `None`. A gap
    /// before it (records the follower collapsed, or a decider restored
    /// from a backup) is not waited on: the follower's own [`crate::Log`]
    /// sees that it was restored.
    pub fn include(
        &mut self,
        clock: Timestamp,
        record: Record<S>,
    ) -> Result<Option<Time>, St::Error> {
        let from = record.at.place;
        if from == self.here || record.at.n <= self.standing(from).upto {
            return Ok(None);
        }
        let here = Time {
            place: self.here,
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
                    Err(why) => (Some(why), None),
                }
            }
            // Another decider's receipt: held as a fact, folded by nobody here.
            What::Included(_) => (None, Some(Durability::Kept)),
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
        let standing = self.standing.entry(included.of.place).or_default();
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
        include: Option<(PlaceId, bool)>,
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
                    let place = old.at.place;
                    if self.last.get(&place).is_some_and(|l| l.n == m) {
                        plan.orphan = Some(place);
                    } else {
                        plan.drop.push(self.time(m));
                    }
                }
            }
            plan.latest = Some((key, holder));
        }
        if let (Some((place, refused)), Some(n)) = (include, holder.included) {
            if let Some(last) = self.last.get(&place) {
                let orphaned = plan.orphan == Some(place) || !last.held;
                if orphaned && !last.refused {
                    plan.drop.push(self.time(last.n));
                    if plan.orphan == Some(place) {
                        plan.orphan = None;
                    }
                }
            }
            let last = Last {
                n,
                held: true,
                refused,
            };
            plan.last = Some((place, last));
        }
        plan
    }

    fn settle(&mut self, plan: Plan<S::Key>) {
        if let Some(place) = plan.orphan
            && let Some(last) = self.last.get_mut(&place)
        {
            last.held = false;
        }
        if let Some((key, holder)) = plan.latest {
            self.latest.insert(key, holder);
        }
        if let Some((place, last)) = plan.last {
            self.last.insert(place, last);
        }
    }

    fn time(&self, n: u64) -> Time {
        Time {
            place: self.here,
            n,
        }
    }
}
