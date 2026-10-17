use std::fmt;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

use crate::time::{Time, Timestamp};

/// One thing that happened at one replica: a time, the replica's clock, and
/// what happened.
///
/// **Concept.** The only thing that crosses between replicas, and the atom:
/// what one replica says together is one record, taken whole or refused
/// whole (a response and the calls of its tool uses, a spawn's dispatch
/// and its members, a settlement and the changes it made). A record is
/// never changed. It keeps its time wherever it is held, so a resend is
/// known as a resend, and a record that is no longer needed is collected,
/// never rewritten.
///
/// **Vision.** "a time and a place, and a fact" (Max's place is a replica); filtered sub-histories
/// that keep event identity (CloneX).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Record<S: Schema> {
    pub at: Time,
    pub clock: Timestamp,
    pub what: What<S>,
}

/// What a record says.
///
/// **Concept.** A record is an envelope and a payload. The envelope (the
/// record's time and clock, whether it is said or a receipt, and what a
/// receipt includes) is the substrate's, and every build reads it. The
/// payload (what was said, or why a receipt refused) is the schema's, and a
/// newer build may say something an older one has no word for. Such a
/// record is [`What::Unread`] in the build that cannot read it: held as it
/// was written, never rewritten, never folded whole. So no build fails to open a
/// replica's records because of what another build wrote; only a change
/// to the envelope, or to how a storage lays it out, is a boundary between
/// builds.
///
/// `Unread` is for a build reading what a later one wrote. A build must read
/// everything earlier builds wrote, so a schema changes its vocabulary
/// only by adding to it, never by changing what an existing word means.
///
/// Records are JSON wherever they are stored or shipped: an unread record
/// is written back out exactly as it was read.
///
/// **Vision.** "While we want live editing, we don't want live bricking":
/// going back to the build before a bad one opens what the bad one wrote,
/// and going forward again loses nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum What<S: Schema> {
    /// The replica's own facts, in its schema's vocabulary.
    Said(S::Said),
    /// That this replica included another replica's record.
    Included(Included<S::Refusal>),
    /// A payload this build cannot read.
    Unread(Unread),
}

/// A record whose payload this build's schema does not parse.
///
/// **Concept.** Kept and skipped. A decider keeps it whatever durability
/// its writer declared (this build cannot tell what supersedes it), counts
/// its time, and never folds it, though its schema may glimpse the part it
/// can read ([`Schema::glimpse`]); a receipt it cannot read still includes
/// the record it names, so the follower's standing moves on, and the
/// record it names is not folded. A decider includes a follower's unread
/// record without folding it. A build that reads it folds it in its place
/// the next time it opens the records.
///
/// **Existing harnesses.** A migration runs forward once, and an older
/// build then refuses the database (Rails, Django); or a reader skips the
/// fields it does not know and drops them when it writes the row back
/// (protobuf).
#[derive(Clone, Debug)]
pub struct Unread {
    /// For a receipt: the record it includes.
    pub of: Option<Time>,
    /// The record's `what`, exactly as it was written.
    pub what: Box<RawValue>,
    /// Why this build's schema does not parse it.
    pub why: String,
}

impl Unread {
    /// What this build's schema can read of a record whose payload it cannot
    /// parse ([`Schema::Glimpse`]), if anything. A receipt's refusal is not
    /// glimpsed: the verdict it holds is not this build's to know.
    pub fn glimpse<S: Schema>(&self) -> Option<S::Glimpse> {
        match serde_json::from_str(self.what.get()).ok()? {
            Envelope::Said(said) => serde_json::from_str(said.get()).ok(),
            Envelope::Included { .. } => None,
        }
    }
}

impl PartialEq for Unread {
    fn eq(&self, other: &Unread) -> bool {
        self.of == other.of && self.what.get() == other.what.get()
    }
}

/// The envelope of a `what`: the substrate's, read by every build.
#[derive(Deserialize)]
enum Envelope<'a> {
    #[serde(borrow)]
    Said(&'a RawValue),
    Included {
        of: Time,
        #[serde(borrow)]
        refused: Option<&'a RawValue>,
    },
}

impl<S: Schema> Serialize for What<S> {
    fn serialize<Se: Serializer>(&self, serializer: Se) -> Result<Se::Ok, Se::Error> {
        match self {
            What::Said(said) => serializer.serialize_newtype_variant("What", 0, "Said", said),
            What::Included(included) => {
                serializer.serialize_newtype_variant("What", 1, "Included", included)
            }
            What::Unread(unread) => unread.what.serialize(serializer),
        }
    }
}

impl<'de, S: Schema> Deserialize<'de> for What<S> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let what = Box::<RawValue>::deserialize(deserializer)?;
        let unread = |of, why: serde_json::Error| Unread {
            of,
            what: what.clone(),
            why: why.to_string(),
        };
        Ok(
            match serde_json::from_str(what.get()).map_err(D::Error::custom)? {
                Envelope::Said(said) => match serde_json::from_str(said.get()) {
                    Ok(said) => What::Said(said),
                    Err(why) => What::Unread(unread(None, why)),
                },
                Envelope::Included { of, refused: None } => {
                    What::Included(Included { of, refused: None })
                }
                Envelope::Included {
                    of,
                    refused: Some(refused),
                } => match serde_json::from_str(refused.get()) {
                    Ok(refused) => What::Included(Included {
                        of,
                        refused: Some(refused),
                    }),
                    Err(why) => What::Unread(unread(Some(of), why)),
                },
            },
        )
    }
}

/// "This record of mine includes that record of yours": a decider's
/// receipt, written in its own timeline.
///
/// **Concept.** Which times include which, in band. A decider takes each
/// follower record into its own timeline exactly once, at the time of its
/// `Included`, and folds it there; so the decider's order of concurrent
/// acts is its own sequence, recorded, the same for every replica and every
/// later build. A record its rules refuse is still held and still included,
/// with `refused` saying why: the act stays a fact, folded as refused
/// ([`Schema::refused`]), and the follower reads both in its
/// [`crate::Standing`]. Including a follower's record `m` includes every
/// earlier record of that replica too, since a replica's records arrive in its
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

/// How long a record is kept by the replicas that hold it.
///
/// **Concept.** Declared per record by its schema, so it is in band: every
/// replica that holds the record agrees, and a store can keep the
/// declaration beside the bytes. A replica keeps its own records only until
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
/// on the link that carries the replica's records, in order, and is gone when
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

/// The vocabulary and rules of one kind of data flow: what replicas say, how
/// it folds, and how long it lasts. `harness-core` is one.
///
/// **Concept.** The substrate's only dependency on what the records mean.
/// [`Schema::fold`] decides: it folds one record into the state or refuses
/// it, deterministically from the state and the record, and is the only
/// replica rules live. It runs where the decider includes a record, and
/// again only when a decider reopens, following the verdicts it recorded,
/// so the past is never judged again by a newer build.
pub trait Schema: Clone + fmt::Debug + PartialEq + 'static {
    /// What a replica says in one record.
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

    /// The part of a payload a build can read even where its [`Self::Said`]
    /// does not parse: what a newer build wrote that this one must still
    /// judge, decoded so that a word this build does not know inside it is
    /// passed over.
    type Glimpse: DeserializeOwned;

    /// Fold what [`Self::Glimpse`] read of a record this build cannot read,
    /// at the decider's time `here`; `at` is the record's own. Never a
    /// refusal: the record is not this build's to judge. It runs where
    /// [`Schema::fold`] would.
    fn glimpse(state: &mut Self::State, here: Time, at: Time, glimpse: &Self::Glimpse);

    /// Fold a record the rules refused, as refused: an invalid act is still
    /// a fact, and the state may say so (a draft edit that raced another
    /// device's becomes a draft of its own). It runs instead of
    /// [`Schema::fold`] wherever the decider's recorded verdict is a
    /// refusal, so a refusal's effect never depends on history that
    /// collection has since dropped. By default a refusal has no effect.
    fn refused(state: &mut Self::State, folding: Folding<'_, Self>, why: &Self::Refusal) {
        let _ = (state, folding, why);
    }
}

/// One record as a decider folds it.
#[derive(Debug)]
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

impl<S: Schema> Clone for Folding<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: Schema> Copy for Folding<'_, S> {}
