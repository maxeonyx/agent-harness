//! The bytes of a value, and how they travel.

use std::fmt;
use std::ops::Deref;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

use crate::id::ValueId;

/// The bytes of one [value](ValueId), as they travel beside the facts that
/// name them ([`crate::fact::Batch`]), or fetched by their id once and
/// cached for good, since they never change.
///
/// **Concept.** A value is bytes named by their digest. Nearly every value
/// is text (a tool's output, a JSON input, a role prompt, Max's words), so in
/// JSON these bytes are a string when they are UTF-8, which is exact and
/// readable on the limb wire and in a face's network log, and
/// `{"base64": "..."}` otherwise (a pasted image, recorded audio). A binary
/// encoding gets the raw bytes. Nothing else in the harness writes bytes by
/// hand.
///
/// No value is larger than [`Bytes::LIMIT`]. That is the contract between
/// whoever produces bytes and everyone who holds them: a limb cuts a
/// command's output to fit before it sends it (head and tail, and how much it
/// left out between them) and keeps none of it, and the brain refuses a
/// larger value ([`crate::fact::Rejected::TooLarge`]), so a face can
/// always fetch any value whole and a command that prints gigabytes costs
/// the limb a bounded buffer, not its memory.
///
/// **Vision.** Large content kept apart from the events that name it
/// (`tech.md`); a remote experience that is "really, really reliable", which
/// a value too big to move is not.
///
/// **Target, not built.** A value sent in pieces: BLAKE3 is a tree hash, so a
/// value can be streamed and each piece verified against the same
/// [`ValueId`] (the `bao` construction). That needs a piecewise send in the
/// limb protocol and the store, and changes no fact, so the path stays open;
/// the limit stands until a value that matters is larger than it.
///
/// **Existing harnesses.** OpenCode saves a large command output to a file
/// and returns a truncated preview
/// (`ref-opencode/packages/core/src/tool/plugin/shell.ts:42`); Oh My Pi
/// records a truncated result with the id of an artifact holding the rest
/// (`ref-oh-my-pi/packages/coding-agent/src/tools/output-meta.ts:59`). Both
/// keep the rest on the machine that ran the command. Here the full output
/// is a value in the one store, cut to the limit by its producer, so it is
/// readable wherever the limb ran.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    /// The largest value anyone sends or the brain stores: 32 MiB.
    pub const LIMIT: usize = 32 << 20;

    /// The value these bytes are.
    pub fn id(&self) -> ValueId {
        ValueId::of(&self.0)
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Bytes {
        Bytes(bytes)
    }
}

impl From<&[u8]> for Bytes {
    fn from(bytes: &[u8]) -> Bytes {
        Bytes(bytes.to_vec())
    }
}

impl From<String> for Bytes {
    fn from(text: String) -> Bytes {
        Bytes(text.into_bytes())
    }
}

impl From<&str> for Bytes {
    fn from(text: &str) -> Bytes {
        Bytes(text.as_bytes().to_vec())
    }
}

/// Short and readable: the length, and the text when it is short text.
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(text) if text.len() <= 80 => write!(f, "Bytes({text:?})"),
            _ => write!(f, "Bytes({} bytes, {})", self.0.len(), self.id()),
        }
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !serializer.is_human_readable() {
            return serializer.serialize_bytes(&self.0);
        }
        match std::str::from_utf8(&self.0) {
            Ok(text) => serializer.serialize_str(text),
            Err(_) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("base64", &encode(&self.0))?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Bytes, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(BytesVisitor)
        } else {
            deserializer.deserialize_byte_buf(BytesVisitor)
        }
    }
}

struct BytesVisitor;

impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Bytes;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string, {\"base64\": ...}, or bytes")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Bytes, E> {
        Ok(Bytes::from(text))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<Bytes, E> {
        Ok(Bytes::from(text))
    }

    fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Bytes, E> {
        Ok(Bytes(bytes.to_vec()))
    }

    fn visit_byte_buf<E: de::Error>(self, bytes: Vec<u8>) -> Result<Bytes, E> {
        Ok(Bytes(bytes))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Bytes, A::Error> {
        let mut bytes = None;
        while let Some(key) = map.next_key::<String>()? {
            if key != "base64" || bytes.is_some() {
                return Err(de::Error::custom("expected exactly one key, `base64`"));
            }
            let text: String = map.next_value()?;
            bytes = Some(decode(&text).ok_or_else(|| de::Error::custom("invalid base64"))?);
        }
        bytes
            .map(Bytes)
            .ok_or_else(|| de::Error::missing_field("base64"))
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, padded.
fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Standard base64, padded; `None` for anything else.
fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let digit = |c: u8| -> Option<u32> {
        let d = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        Some(u32::from(d))
    };
    let quads = text.len() / 4;
    let mut out = Vec::with_capacity(quads * 3);
    for (i, quad) in text.chunks(4).enumerate() {
        let pad = quad.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && i + 1 != quads) {
            return None;
        }
        let mut n = 0u32;
        for c in &quad[..4 - pad] {
            n = (n << 6) | digit(*c)?;
        }
        n <<= 6 * pad;
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Some(out)
}
