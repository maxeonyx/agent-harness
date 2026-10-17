//! Identifiers, positions and time.
//!
//! Three kinds of name. Things that change over time (a task, a call, a
//! request, a delivery, an environment, a limb) are named by the event that
//! made them ([`EventId`]): the replica and time of the record that created
//! them, so federated brains hold each other's records without renaming
//! anything and nothing generates an id apart from a fact. Immutable bytes
//! are named by their digest, a [`ValueId`]. A context entry is named by the
//! digest of its fields, which are value ids ([`crate::context::EntryId`]),
//! so the context tree is a Merkle tree over stored values.
//!
//! An event id is written `{replica}.{n}.{i}` and a digest as 64 lowercase hex
//! digits. In JSON each is a string, because readers that take every number
//! as a double (a browser's `JSON.parse`, SQLite's `->>`) would round one.
//! Every one parses back with [`FromStr`], so no other crate writes its own
//! id scheme.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use harness_substrate::{EventId, ParseError, ReplicaId, Time, Timestamp};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub EventId);

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(text: &str) -> Result<$name, ParseError> {
                text.parse().map($name)
            }
        }
    };
}

id!(
    /// A [`crate::task::Task`]: one agent's mandate over its whole life,
    /// across every context it works in. The event that created it.
    TaskId
);
id!(
    /// A [`crate::call::Call`]: the event that made it. Shared with the limb
    /// that runs it, which runs each id at most once.
    CallId
);
id!(
    /// A [`crate::model_call::ModelCall`]: one request to a provider.
    ModelCallId
);
id!(
    /// Max's message: a [`crate::draft::Draft`] from the moment his face
    /// opens it, and the [`crate::delivery::Delivery`] it becomes when sent,
    /// so a message has one identity from his first keystroke.
    DeliveryId
);
id!(
    /// An [`crate::environment::Environment`]: the event of its first
    /// version.
    EnvironmentId
);
id!(
    /// A [`crate::environment::Limb`]: the event of its first version.
    /// Several environments may share one, for example an engineer's and a
    /// reviewer's on the same clone.
    LimbId
);

/// The position of one fact in its decider's timeline: the decider's record
/// it took effect at ([`harness_substrate::Folding::here`]), and its index
/// among that record's facts.
///
/// It orders everything a brain has folded: what came before a launch,
/// whether a call was sent before a limb's current connection began, which
/// version of a resource a context embodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Seq {
    pub n: u64,
    pub i: u32,
}

/// A BLAKE3 hash. Written as hex everywhere, `Debug` included, so a
/// refusal or a log line names a value the way a face and the store do.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Digest {
    /// The digest of `bytes`.
    pub fn of(bytes: &[u8]) -> Digest {
        Digest(*blake3::hash(bytes).as_bytes())
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl FromStr for Digest {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Digest, ParseError> {
        hex::<32>(text).map(Digest)
    }
}

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        parsed(deserializer)
    }
}

/// An immutable value: bytes, named by their digest.
///
/// **Concept.** One kind of thing, stored one way. A message's content, a
/// root's system blocks and tools, a call's input, the model's view of its
/// result and the full output behind it, Max's words, a pasted image, a
/// limb's description of itself, the visible text of an abandoned
/// response: each is a value, kept in one store keyed by this id, and named
/// by the facts that concern it. A fact is small structure; the bytes it is
/// about are values beside it. So folding never parses a tool's
/// output, a face fetches any value once and caches it forever, and a value
/// whose bytes are purged leaves every id that names it valid.
///
/// Whoever produces bytes sends them with the facts that name them, in one
/// [`crate::fact::Batch`], as [`crate::value::Bytes`]: a limb with its
/// settlement, the brain with its request, Max's face with his words. There
/// is no second store: a limb keeps nothing the brain names on its own disk.
///
/// **Vision.** "Message contents / large text / images etc should probably
/// be separate to the event tables themselves" (`tech.md`); compaction that
/// never destroys the transcript; analytics that read small rows.
///
/// **Bx.** The same identity as Bx's artifact (`hash + immutable value`),
/// and plausibly the same store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValueId(pub Digest);

impl ValueId {
    /// The id of `bytes`.
    pub fn of(bytes: &[u8]) -> ValueId {
        ValueId(Digest::of(bytes))
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ValueId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<ValueId, ParseError> {
        text.parse().map(ValueId)
    }
}

/// The provider's id for one `tool_use` block, which its `tool_result` must
/// echo. Generated by the harness for the tool uses it writes itself (the
/// attachments of a fresh context).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ToolUseId(pub String);

/// Read exactly `N` bytes written as `2 * N` lowercase hex digits.
fn hex<const N: usize>(text: &str) -> Result<[u8; N], ParseError> {
    let invalid = ParseError {
        expected: "lowercase hex digits",
    };
    if text.len() != 2 * N {
        return Err(invalid);
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let text = text.as_bytes();
    let mut bytes = [0; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        let (high, low) = digit(text[2 * i])
            .zip(digit(text[2 * i + 1]))
            .ok_or(invalid)?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn parsed<'de, D: Deserializer<'de>, T: FromStr<Err = ParseError>>(
    deserializer: D,
) -> Result<T, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}
