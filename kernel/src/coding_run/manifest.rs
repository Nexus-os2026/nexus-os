//! Deterministic content manifests and their canonical hash.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::scope::RelPath;

const MANIFEST_DOMAIN: &[u8] = b"nexus.coding_run.manifest.v1";

/// Backend-computed facts about one staged regular file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub size: u64,
    pub sha256: [u8; 32],
}

impl ManifestEntry {
    pub(crate) fn of(content: &[u8]) -> Self {
        Self {
            size: content.len() as u64,
            sha256: Sha256::digest(content).into(),
        }
    }
}

/// A manifest: regular files by normalized relative path, in sorted order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    entries: BTreeMap<RelPath, ManifestEntry>,
}

impl Manifest {
    pub(crate) fn insert(&mut self, path: RelPath, entry: ManifestEntry) {
        self.entries.insert(path, entry);
    }

    pub fn entries(&self) -> &BTreeMap<RelPath, ManifestEntry> {
        &self.entries
    }

    pub fn get(&self, path: &RelPath) -> Option<&ManifestEntry> {
        self.entries.get(path)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_bytes(&self) -> u64 {
        self.entries.values().map(|entry| entry.size).sum()
    }

    /// SHA-256 over the domain tag, the entry count and, in path order, each
    /// entry as (length-prefixed path bytes, size, content hash). Every field
    /// is length-prefixed or fixed-width, so the encoding is unambiguous.
    pub fn hash(&self) -> ManifestHash {
        let mut hasher = Sha256::new();
        put_bytes(&mut hasher, MANIFEST_DOMAIN);
        hasher.update((self.entries.len() as u64).to_be_bytes());
        for (path, entry) in &self.entries {
            put_bytes(&mut hasher, path.as_string().as_bytes());
            hasher.update(entry.size.to_be_bytes());
            hasher.update(entry.sha256);
        }
        ManifestHash(hasher.finalize().into())
    }
}

/// Canonical hash of a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManifestHash([u8; 32]);

impl ManifestHash {
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub(crate) fn put_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}
