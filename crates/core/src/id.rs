//! Identifiers, positions and time.
//!
//! Two kinds of identity. Things that change over time (a task, a call, a
//! model call, a delivery, an environment, a limb) get generated ids, unique
//! across brains, so federated brains can hold each other's records without
//! renaming anything. Values that never change (context entries, stored
//! output, resource versions) are named by their content: a [`Digest`].
//!
//! Both kinds are written as lowercase hex: a generated id as 32 digits, a
//! digest as 64. In JSON that is a string, because readers that take every
//! number as a double (a browser's `JSON.parse`, SQLite's `->>`) would round
//! a 128-bit id; hex also sorts as the id does.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u128);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:032x}", self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bytes: [u8; 16] = hex(deserializer)?;
                Ok($name(u128::from_be_bytes(bytes)))
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
    /// A [`crate::call::Call`]. Shared with the limb that runs it, so a limb
    /// that outlives a brain restart can say how a call ended.
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
    /// One running instance of a limb: a process, a connection, a page. A new
    /// instance means whatever the old one held in memory is gone.
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
/// he acted is the position his face had reached ([`crate::journal::Author`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Seq(pub u64);

/// Milliseconds since the Unix epoch, UTC, as the brain recorded it. Faces
/// show it in the viewer's zone (NZT for Max).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

/// The BLAKE3 hash of an immutable value's canonical bytes.
///
/// The same identity as Bx's artifact (`hash + immutable value`), and
/// plausibly the same store: context entries, stored output too large to
/// keep inline, and resource versions are all named this way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

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

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        hex(deserializer).map(Digest)
    }
}

/// Read exactly `N` bytes written as `2 * N` lowercase hex digits.
fn hex<'de, D: Deserializer<'de>, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error> {
    struct Hex<const N: usize>;

    impl<const N: usize> serde::de::Visitor<'_> for Hex<N> {
        type Value = [u8; N];

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{} lowercase hex digits", 2 * N)
        }

        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<[u8; N], E> {
            let invalid = || E::invalid_value(serde::de::Unexpected::Str(text), &self);
            if text.len() != 2 * N {
                return Err(invalid());
            }
            let digit = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                _ => None,
            };
            let mut bytes = [0; N];
            for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
                let (high, low) = digit(pair[0]).zip(digit(pair[1])).ok_or_else(invalid)?;
                *byte = high << 4 | low;
            }
            Ok(bytes)
        }
    }

    deserializer.deserialize_str(Hex)
}

/// A stored value by its digest: an image, a tool's full output, a file's
/// content at some version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentId(pub Digest);

/// The provider's id for one `tool_use` block, which its `tool_result` must
/// echo. Generated by the harness for the tool uses it writes itself (the
/// attachments of a fresh context).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ToolUseId(pub String);
