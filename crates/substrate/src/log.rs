use std::collections::BTreeMap;

use crate::decider::Standing;
use crate::record::{Durability, Record, Schema, What};
use crate::storage::Storage;
use crate::time::{PlaceId, Time, Timestamp};

/// A follower's own records, kept until its decider has included them.
///
/// **Concept.** What a tab or a limb says: written here first, durably,
/// with a time of its own, then shipped to the decider for as long as the
/// decider's [`Standing`] has not covered it. Nothing is ever resent by
/// request and nothing is acknowledged by message: on every (re)connection
/// the follower ships what the decider's standing does not include, and the
/// decider skips anything it already has. A record superseded before it is
/// shipped ([`Durability::Latest`]: text typed offline) is collected here,
/// so an hour offline costs one record per draft.
///
/// **Vision.** A draft that has identity from its creation in the browser
/// and survives a closed laptop; a limb whose settlements survive a brain
/// restart; no resend protocol.
pub struct Log<S: Schema, St: Storage<S>> {
    here: PlaceId,
    next: u64,
    records: BTreeMap<u64, Record<S>>,
    storage: St,
}

/// The decider holds records of this place that this place does not: its
/// storage was restored from a backup or lost. It must continue as a new
/// place, or its new records would reuse times the decider already holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Restored {
    /// How far the decider had included this place.
    pub upto: u64,
    /// This place's next time, as its storage says.
    pub next: u64,
}

impl<S: Schema, St: Storage<S>> Log<S, St> {
    /// The log of `here`, from what `storage` holds.
    pub fn open(here: PlaceId, storage: St) -> Result<Self, St::Error> {
        let records: BTreeMap<u64, Record<S>> = storage
            .load()?
            .into_iter()
            .filter(|r| r.at.place == here)
            .map(|r| (r.at.n, r))
            .collect();
        Ok(Log {
            here,
            next: records.keys().next_back().map_or(1, |n| n + 1),
            records,
            storage,
        })
    }

    pub fn here(&self) -> PlaceId {
        self.here
    }

    /// The time its next record will have, so what that record creates
    /// can be named before it is said.
    pub fn next(&self) -> Time {
        Time {
            place: self.here,
            n: self.next,
        }
    }

    /// Write the next record, stored before it is returned for shipping.
    /// `said` is given the record's time, so what it creates is named by it.
    pub fn say(
        &mut self,
        clock: Timestamp,
        said: impl FnOnce(Time) -> S::Said,
    ) -> Result<Record<S>, St::Error> {
        let at = Time {
            place: self.here,
            n: self.next,
        };
        let said = said(at);
        let mut drop = Vec::new();
        if let Durability::Latest(key) = S::durability(at, &said) {
            drop.extend(self.records.values().filter_map(|r| match &r.what {
                What::Said(s) if S::durability(r.at, s) == Durability::Latest(key.clone()) => {
                    Some(r.at)
                }
                _ => None,
            }));
        }
        let record = Record {
            at,
            clock,
            what: What::Said(said),
        };
        self.storage.commit(std::slice::from_ref(&record), &drop)?;
        for time in &drop {
            self.records.remove(&time.n);
        }
        self.records.insert(at.n, record.clone());
        self.next += 1;
        Ok(record)
    }

    /// What the decider standing at `standing` has not included, to ship.
    pub fn unsent(&self, standing: &Standing<S::Refusal>) -> Result<Vec<Record<S>>, Restored> {
        if standing.upto >= self.next {
            return Err(Restored {
                upto: standing.upto,
                next: self.next,
            });
        }
        Ok(self
            .records
            .range(standing.upto + 1..)
            .map(|(_, r)| r.clone())
            .collect())
    }

    /// Collect what the decider has included. The newest record stays,
    /// whatever it is, so this place's count of its own times survives a
    /// restart.
    pub fn cover(&mut self, standing: &Standing<S::Refusal>) -> Result<(), St::Error> {
        let newest = self.next - 1;
        let drop: Vec<Time> = self
            .records
            .range(..=standing.upto.min(newest.saturating_sub(1)))
            .map(|(_, r)| r.at)
            .collect();
        if drop.is_empty() {
            return Ok(());
        }
        self.storage.commit(&[], &drop)?;
        for time in drop {
            self.records.remove(&time.n);
        }
        Ok(())
    }

    /// The records still held: those not yet included, and the newest.
    pub fn records(&self) -> impl Iterator<Item = &Record<S>> {
        self.records.values()
    }
}
