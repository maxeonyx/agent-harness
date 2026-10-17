use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::time::{Time, Timestamp};

/// One thing that happened at one place: a time, the place's clock, and
/// what happened.
///
/// **Concept.** The only thing that crosses between places, and the atom:
/// what one place says together is one record, taken whole or refused
/// whole (a response and the calls of its tool uses, a spawn's dispatch
/// and its members, a settlement and the changes it made). A record is
/// never changed. It keeps its time wherever it is held, so a resend is
/// known as a resend, and a record that is no longer needed is collected,
/// never rewritten.
///
/// **Vision.** "A time and a place, and a fact"; filtered sub-histories
/// that keep event identity (CloneX).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Record<S: Schema> {
    pub at: Time,
    pub clock: Timestamp,
    pub what: What<S>,
}

/// What a record says.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub enum What<S: Schema> {
    /// The place's own facts, in its schema's vocabulary.
    Said(S::Said),
    /// That this place included another place's record.
    Included(Included<S::Refusal>),
}

/// "This record of mine includes that record of yours": a decider's
/// receipt, written in its own timeline.
///
/// **Concept.** Which times include which, in band. A decider takes each
/// follower record into its own timeline exactly once, at the time of its
/// `Included`, and folds it there; so the decider's order of concurrent
/// acts is its own sequence, recorded, the same for every replica and every
/// later build. A record its rules refuse is still held and still included,
/// with `refused` saying why: the act stays a fact, its having had no
/// effect is a fact beside it, and the follower reads both in its
/// [`crate::Standing`]. Including a follower's record `m` includes every
/// earlier record of that place too, since a place's records arrive in its
/// own order.
///
/// Only deciders write these. A follower's knowledge of its decider is the
/// view it holds, and what it had seen when it acted is a fact of its own
/// schema, so receipts never echo back and forth.
///
/// **Vision.** "Which timestamps include which is in band"; "invalid
/// actions stay facts, in band"; a sent status derived from causality.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Included<R> {
    pub of: Time,
    pub refused: Option<R>,
}

/// How long a record is kept by the places that hold it.
///
/// **Concept.** Declared per record by its schema, so it is in band: every
/// place that holds the record agrees, and a store can keep the
/// declaration beside the bytes. A place keeps its own records only until
/// its decider has included them (that is "stored until processed", for
/// every record); this says what the decider keeps after that.
///
/// What is never stored is not a durability: it is [`Live`], which has no
/// time. "Kept until processed" at the decider needs snapshots of its state
/// and is not built: until then, a folded record is the decider's only
/// memory of its effect, so it is kept.
///
/// **Vision.** "Never stored, stored until processed, current state
/// persisted only", declared in band; the database that grows with what
/// Max did, not with how often he clicked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Durability<K> {
    /// Only the newest record with this key is kept, by the decider's order:
    /// a draft's text, which task a tab shows. Only a record no rule reads
    /// the history of may say this, since a fold after collection sees only
    /// the newest.
    Latest(K),
    /// Kept for good: a task's life, a call's settlement, a delivery.
    Kept,
}

/// Something shown as it happens and never stored: streamed tokens,
/// command output, text mid-dictation.
///
/// **Concept.** Takes no time of its own: it follows the record at `after`,
/// on the link that carries the place's records, in order, and is gone when
/// the connection is. The record that ends it (the response, the
/// settlement) supersedes it, so a follower that missed some never needs
/// them resent.
///
/// **Vision.** The never-stored durability kind; a face that shows the
/// agent working as it works.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live<L> {
    pub after: Time,
    pub live: L,
}

/// The vocabulary and rules of one kind of data flow: what places say, how
/// it folds, and how long it lasts. `harness-core` is one.
///
/// **Concept.** The substrate's only dependency on what the records mean.
/// [`Schema::fold`] decides: it folds one record into the state or refuses
/// it, deterministically from the state and the record, and is the only
/// place rules live. It runs where the decider includes a record, and
/// again only when a decider reopens, following the verdicts it recorded,
/// so the past is never judged again by a newer build.
pub trait Schema: Clone + fmt::Debug + PartialEq + 'static {
    /// What a place says in one record.
    type Said: Clone + fmt::Debug + PartialEq + Serialize + DeserializeOwned;
    /// Why the fold refused a record.
    type Refusal: Clone + fmt::Debug + PartialEq + Serialize + DeserializeOwned;
    /// What the facts amount to, at a decider. Cheap to clone: a record is
    /// folded into a clone, and the clone kept only once the record is
    /// stored.
    type State: Clone + Default;
    /// What a [`Durability::Latest`] record supersedes by.
    type Key: Clone + fmt::Debug + Ord;

    /// How long the record saying `said` at `at` is kept.
    fn durability(at: Time, said: &Self::Said) -> Durability<Self::Key>;

    /// Fold one record into `state`, or refuse it. On a refusal `state` may
    /// be part way through the record; the decider discards it.
    fn fold(state: &mut Self::State, folding: Folding<'_, Self>) -> Result<(), Self::Refusal>;
}

/// One record as a decider folds it.
#[derive(Clone, Copy, Debug)]
pub struct Folding<'a, S: Schema> {
    /// The decider's time this record takes effect at: the record itself,
    /// if it is the decider's own, or the [`Included`] that took it in.
    pub here: Time,
    /// The decider's clock at `here`.
    pub clock: Timestamp,
    /// The record's own time: who said it, and the identity of what it
    /// creates.
    pub at: Time,
    pub said: &'a S::Said,
}
