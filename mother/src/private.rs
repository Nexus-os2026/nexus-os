//! P0 (privacy closure): Mother's private store (design §3 I2).
//!
//! Working content, the mission contract and a paused phase's rest, is kept
//! here, content addressed and hash checked, and named in the journal only
//! by reference. Records (the journal, the broker's evidence) keep digests,
//! sizes and codes.
//!
//! The private store is not authority. Nothing in it restores a consent, an
//! approval or a resume: those come only from the verified journal and the
//! broker. A blob no journal entry names is ignored. Content that is
//! missing, or whose content id, size or contract hash does not verify,
//! stops its mission (after its journaled request is withdrawn).
//!
//! The production store is [`crate::adapter::FilePrivate`]: files in
//! `<home>/private/<kind>/<hex>`, directories 0700 and files 0600.
//!
//! Non-claims: no encryption at rest, no retention or purge (blobs stay
//! after their mission ends), no historical scrubbing of v1 journals (they
//! are refused). Someone with write access to Mother's home can delete or
//! replace blobs: that stops the missions that need them, and grants
//! nothing.

use crate::contract::sha256;
use crate::journal::IoKind;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The largest content the store keeps (a contract or a rest is far
/// smaller).
pub const MAX_CONTENT_BYTES: u64 = 1 << 20;

/// What a blob is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentKind {
    /// A mission contract, in its canonical JSON.
    Contract,
    /// A paused phase's rest: the asking lane's remaining effect requests.
    PausedPhase,
}

impl ContentKind {
    pub const ALL: [ContentKind; 2] = [ContentKind::Contract, ContentKind::PausedPhase];

    pub fn as_str(self) -> &'static str {
        match self {
            ContentKind::Contract => "contract",
            ContentKind::PausedPhase => "paused_phase",
        }
    }
}

/// A blob's content id: `SHA-256("nexus.mother.private.v1/<kind>" ‖ len ‖
/// bytes)`, the length as 8 big-endian bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId([u8; 32]);

impl ContentId {
    pub fn of(kind: ContentKind, bytes: &[u8]) -> Self {
        let domain = format!("nexus.mother.private.v1/{}", kind.as_str());
        Self(sha256(domain.as_bytes(), &[bytes]))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Only 64 lowercase hex digits.
    pub fn from_hex(text: &str) -> Option<Self> {
        if text.len() != 64 || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return None;
        }
        let bytes = hex::decode(text).ok()?;
        Some(Self(bytes.try_into().ok()?))
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentId({})", self.to_hex())
    }
}

impl Serialize for ContentId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_hex(&text).ok_or_else(|| de::Error::custom("content id"))
    }
}

/// Why the private store refused: a closed code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateError {
    /// No blob of that kind under that id.
    Missing,
    /// The bytes found do not match the content id (or, for a mission's
    /// content, its size, its parse or its contract hash).
    Mismatch,
    /// Larger than [`MAX_CONTENT_BYTES`].
    TooLarge,
    /// The store or a blob is not a private directory or a regular file
    /// (a link, or open to other users).
    NotPrivate,
    /// The file system refused.
    Io(IoKind),
}

/// Where Mother keeps working content.
pub trait PrivateStore: Send + Sync {
    /// Keep `bytes` durably; their content id.
    fn put(&self, kind: ContentKind, bytes: &[u8]) -> Result<ContentId, PrivateError>;
    /// The bytes kept under `id`, only if their content id is `id`.
    fn get(&self, kind: ContentKind, id: &ContentId) -> Result<Vec<u8>, PrivateError>;
}

type Blobs = BTreeMap<(ContentKind, ContentId), Vec<u8>>;

/// A private store in memory. Clones share the same blobs, so a test can
/// reopen a coordinator over what an earlier one kept, or change a blob.
#[derive(Clone, Default)]
pub struct MemoryPrivate {
    blobs: Arc<Mutex<Blobs>>,
}

impl MemoryPrivate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace (or remove, with `None`) the bytes under `id` (tests).
    pub fn set(&self, kind: ContentKind, id: &ContentId, bytes: Option<Vec<u8>>) {
        let mut blobs = self.blobs.lock().expect("private store");
        match bytes {
            Some(bytes) => blobs.insert((kind, *id), bytes),
            None => blobs.remove(&(kind, *id)),
        };
    }
}

impl PrivateStore for MemoryPrivate {
    fn put(&self, kind: ContentKind, bytes: &[u8]) -> Result<ContentId, PrivateError> {
        if bytes.len() as u64 > MAX_CONTENT_BYTES {
            return Err(PrivateError::TooLarge);
        }
        let id = ContentId::of(kind, bytes);
        self.blobs
            .lock()
            .expect("private store")
            .insert((kind, id), bytes.to_vec());
        Ok(id)
    }

    fn get(&self, kind: ContentKind, id: &ContentId) -> Result<Vec<u8>, PrivateError> {
        let bytes = self
            .blobs
            .lock()
            .expect("private store")
            .get(&(kind, *id))
            .cloned()
            .ok_or(PrivateError::Missing)?;
        if ContentId::of(kind, &bytes) != *id {
            return Err(PrivateError::Mismatch);
        }
        Ok(bytes)
    }
}
