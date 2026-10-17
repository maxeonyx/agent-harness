//! The harness as one schema on the substrate: what its replicas say, and how
//! long the brain keeps it.

use harness_substrate::{Decider, Durability, Folding, Schema, Time};

use crate::draft::DraftEvent;
use crate::fact::{Batch, Fact, Refusal};
use crate::id::{DeliveryId, ReplicaId};
use crate::state::State;

/// The harness's [`Schema`]: every replica says a [`Batch`] of [`Fact`]s, the
/// brain folds them into [`State`], and refuses with a [`Refusal`].
///
/// **Concept.** The one vocabulary of the brain, its limbs and Max's faces,
/// on the one data flow. Who said a record is derived from where it came
/// from; what each author may say is checked by the fold; the values a
/// record's facts name travel with it. On main the brain is the only
/// decider, so it owns every object two replicas could act on at once: a
/// task's life, a call's place in it, a draft. A limb decides only what is
/// its own (how a call it ran ended), and its outcome merges with the
/// brain's view by a join: a call the brain settled as unknown takes the
/// limb's real outcome when it arrives.
///
/// **Vision.** One derived data flow shared by the brain, the face and each
/// limb; the brain reads nothing but its own records; sessions as programs
/// that a new build folds without re-judging what an older one decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harness;

/// The brain: the harness's decider.
pub type Brain<St> = Decider<Harness, St>;

/// One record of the harness: a replica's batch, or the brain including one.
pub type Record = harness_substrate::Record<Harness>;

/// What a record kept only as the newest of its kind is the newest of.
///
/// Only facts no rule reads the history of are here, since after
/// collection the brain folds only the newest: which task a face shows, a
/// draft's text (a Send carries the words it sends), a clock moving on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// Which task one face shows.
    Focus(ReplicaId),
    /// One draft's text, whichever face typed it.
    Draft(DeliveryId),
    /// A record that says nothing but its place's clock.
    Clock(ReplicaId),
}

impl Schema for Harness {
    type Said = Batch;
    type Refusal = Refusal;
    type State = State;
    type Key = Key;

    /// Kept, unless every fact but its [`Fact::Seen`] is the newest of one
    /// key. An act carries its `Seen`, so what Max had seen is kept with
    /// what he did.
    fn durability(at: Time, said: &Batch) -> Durability<Key> {
        let mut key = None;
        for fact in &said.facts {
            let this = match fact {
                Fact::Seen(_) => continue,
                Fact::Focus(_) => Key::Focus(at.replica),
                Fact::Draft(id, DraftEvent::Text(_)) => Key::Draft(*id),
                _ => return Durability::Kept,
            };
            if key.is_some_and(|key| key != this) {
                return Durability::Kept;
            }
            key = Some(this);
        }
        Durability::Latest(key.unwrap_or(Key::Clock(at.replica)))
    }

    fn fold(state: &mut State, folding: Folding<'_, Harness>) -> Result<(), Refusal> {
        state.fold(folding)
    }

    fn refused(state: &mut State, folding: Folding<'_, Harness>, why: &Refusal) {
        state.refused(folding, why)
    }
}
