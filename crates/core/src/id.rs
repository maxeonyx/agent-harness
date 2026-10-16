//! Identifiers, positions and time.
//!
//! Three kinds of name. Things that change over time (a task, a call, a
//! request, a delivery, an environment, a limb) get generated ids, unique
//! across brains, so federated brains can hold each other's records without
//! renaming anything. Immutable bytes are named by their digest, a
//! [`ValueId`]. A context entry is named by the digest of its fields, which
//! are value ids ([`crate::context::EntryId`]), so the context tree is a
//! Merkle tree over stored values.
//!
//! All of them are written as lowercase hex: a generated id as 32 digits, a
//! digest as 64. In JSON that is a string, because readers that take every
//! number as a double (a browser's `JSON.parse`, SQLite's `->>`) would round
//! a 128-bit id; hex also sorts as the id does. Every one parses back with
//! [`FromStr`] and is made with `generate`, so no other crate writes its own
//! hex or its own id scheme.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Text that is not the hex an id or a digest is written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// How many lowercase hex digits were expected.
    pub digits: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {} lowercase hex digits", self.digits)
    }
}

impl std::error::Error for ParseError {}

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u128);

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self)
            }
        }

        impl $name {
            /// A new id, unique across brains and processes.
            pub fn generate() -> $name {
                $name(generate())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:032x}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(text: &str) -> Result<$name, ParseError> {
                hex::<16>(text).map(|bytes| $name(u128::from_be_bytes(bytes)))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                parsed(deserializer)
            }
        }
    };
}

id!(
    /// A [`crate::task::Task`]: one agent's mandate over its whole life,
    /// across every context it works in.
    TaskId
);
id!(
    /// A [`crate::call::Call`]. Shared with the limb that runs it, which runs
    /// each id at most once, so sending a call again after a restart is safe.
    CallId
);
id!(
    /// A [`crate::model_call::ModelCall`]: one request to a provider.
    ModelCallId
);
id!(
    /// A [`crate::delivery::Delivery`].
    DeliveryId
);
id!(
    /// An [`crate::environment::Environment`].
    EnvironmentId
);
id!(
    /// A [`crate::environment::Limb`]. Several environments may share one,
    /// for example an engineer's and a reviewer's on the same clone.
    LimbId
);
id!(
    /// One running instance of a limb: a process, a page. A new instance
    /// means whatever the old one held in memory is gone.
    InstanceId
);
id!(
    /// One connected face: a browser tab, a phone. Max is the same person
    /// behind all of them, and two of them can show him different things.
    FaceId
);

/// A position in one brain's journal: dense, starting at 1.
///
/// It is the only cursor anyone needs. A face that lost its connection asks
/// for everything after the last position it saw; a restarted brain folds
/// from the start; an analytics query reads a range. What Max had seen when
/// he acted is the position his face had reached ([`crate::journal::Author`]),
/// and whether a limb must be told a call again is whether the call was last
/// sent to it before its current connection began.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Seq(pub u64);

/// Milliseconds since the Unix epoch, UTC, as the brain recorded it. Faces
/// show it in the viewer's zone (NZT for Max).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

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
/// about are values beside it. So folding the journal never parses a tool's
/// output, a face fetches any value once and caches it forever, and a value
/// whose bytes are purged leaves every id that names it valid.
///
/// Whoever produces bytes sends them with the facts that name them, in one
/// [`crate::journal::Batch`], as [`crate::value::Bytes`]: a limb with its settlement, the brain with
/// its request, Max's face with his words. There is no second store: a limb
/// keeps nothing the journal names on its own disk.
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
    let invalid = ParseError { digits: 2 * N };
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

/// 128 bits from the operating system's per-process randomness, the process,
/// the moment and a counter.
fn generate() -> u128 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let random = RandomState::new().build_hasher().finish();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"harness-id");
    hasher.update(&random.to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    hasher.update(&nanos.to_le_bytes());
    hasher.update(&NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    u128::from_be_bytes(bytes)
}
