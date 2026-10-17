use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A replica: one timeline of records, and the storage that holds it.
///
/// **Concept.** The brain, a limb with its state directory, a browser tab
/// (its id kept in `sessionStorage`), a phone's tab. A replica is not a
/// process: one process may host several (the brain hosts the roles limb
/// beside its own; a page holds a closed tab's replica while it ships what
/// that tab had not), and a record's time is a time in its replica. A replica
/// writes its records one after another, so its own times are totally
/// ordered; two replicas' times are ordered only by what one has included of
/// the other. Whatever loses its timeline (a tab closed, a limb's state
/// directory wiped, a database restored from a backup) comes back as a new
/// replica: a new id, drawn from the embedding's entropy, never reused.
///
/// **Vision.** Every part of the harness is a replica: the brain, each limb
/// and the face; "a time and a place" (Max's place is a replica).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplicaId(pub u64);

/// A moment at a replica: its `n`th record, counting from 1.
///
/// **Concept.** The time of a record, and the identity of its effects.
/// Within a replica, later is larger. Between replicas there is no clock to
/// compare: a decider's time includes a follower's time when the decider
/// has included that record ([`crate::Included`]), and that is the only
/// cross-replica order there is. Times are never reused, so a record resent
/// after a reconnect is recognised as the one already held.
///
/// Written `{replica}.{n}`, a string in JSON, so a browser's `JSON.parse`
/// never rounds it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    pub replica: ReplicaId,
    pub n: u64,
}

/// The identity of one fact: the time of the record that states it, and its
/// index among that record's facts.
///
/// **Concept.** What a fact creates is named by the fact: a draft is the
/// event that opened it, a task the event that created it, so an id exists
/// only if the record that made it exists, and it says where and when it
/// was made. Nothing generates an id apart from a record, so no id is
/// random, no id is minted by a replica that did not make the thing, and two
/// replicas can never mint the same one.
///
/// **Vision.** "We give it an event ID when I create the message"; data
/// that stays "clear where it came from" across federated brains.
///
/// Written `{replica}.{n}.{i}`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId {
    pub at: Time,
    pub i: u32,
}

/// Milliseconds since the Unix epoch, UTC, on the clock of the replica that
/// wrote a record.
///
/// **Concept.** Every record carries its place's clock, so "now" is in band:
/// a fold that asks whether a deadline has passed reads the clock of the
/// record it is folding, and a deadline with nothing else happening is a
/// record the embedding writes when its timer fires. Clocks of different
/// replicas are not compared for order; [`Time`] is the order. Faces show it
/// in the viewer's zone (NZT for Max).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl ReplicaId {
    /// The first time at this replica: no record has it.
    pub fn origin(self) -> Time {
        Time {
            replica: self,
            n: 0,
        }
    }
}

impl Time {
    /// The `i`th fact of the record at this time.
    pub fn event(self, i: u32) -> EventId {
        EventId { at: self, i }
    }
}

/// Text that is not how an id, a time or a digest is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// What the text should have been.
    pub expected: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {}", self.expected)
    }
}

impl std::error::Error for ParseError {}

impl fmt::Display for ReplicaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for ReplicaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReplicaId({self})")
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.replica, self.n)
    }
}

impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Time({self})")
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.at, self.i)
    }
}

impl fmt::Debug for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventId({self})")
    }
}

impl FromStr for ReplicaId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<ReplicaId, ParseError> {
        let invalid = ParseError {
            expected: "16 lowercase hex digits",
        };
        let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
        if text.len() != 16 || !text.chars().all(hex) {
            return Err(invalid);
        }
        u64::from_str_radix(text, 16)
            .map(ReplicaId)
            .map_err(|_| invalid)
    }
}

impl FromStr for Time {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Time, ParseError> {
        let invalid = ParseError {
            expected: "a time: {replica}.{n}",
        };
        let (replica, n) = text.split_once('.').ok_or(invalid)?;
        Ok(Time {
            replica: replica.parse()?,
            n: number(n).ok_or(invalid)?,
        })
    }
}

impl FromStr for EventId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<EventId, ParseError> {
        let invalid = ParseError {
            expected: "an event: {replica}.{n}.{i}",
        };
        let (at, i) = text.rsplit_once('.').ok_or(invalid)?;
        Ok(EventId {
            at: at.parse()?,
            i: number(i).ok_or(invalid)?,
        })
    }
}

/// A decimal number written without sign or leading zeros, so each value
/// has one spelling.
fn number<N: FromStr>(text: &str) -> Option<N> {
    let canonical = !text.is_empty()
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok()).flatten()
}

macro_rules! as_string {
    ($($name:ident),*) => {$(
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    )*};
}

as_string!(ReplicaId, Time, EventId);
