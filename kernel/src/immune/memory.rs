//! Immune memory — stores threat signatures (auto-generated virus definitions).
//!
//! When a threat is detected and an antibody successfully neutralizes it, the
//! signature is committed to [`ImmuneMemory`] so future encounters are
//! recognized instantly — just like biological adaptive immunity.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// ThreatSignature
// ---------------------------------------------------------------------------

/// A stored threat fingerprint with its successful antibody response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatSignature {
    /// SHA-256 hash of the threat pattern.
    pub threat_hash: String,
    /// The raw pattern or description that triggered detection.
    pub pattern: String,
    /// Defense pattern string from the antibody that neutralized this threat.
    pub antibody_response: String,
    /// UNIX timestamp when first observed.
    pub first_seen: u64,
    /// Number of times this signature has been matched and blocked.
    pub times_blocked: u64,
}

// ---------------------------------------------------------------------------
// ImmuneMemory
// ---------------------------------------------------------------------------

/// Persistent store of known threat signatures.
///
/// Lookup is O(1) via hash. Signatures accumulate over the lifetime of the OS,
/// forming an ever-growing virus definition database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmuneMemory {
    signatures: HashMap<String, ThreatSignature>,
}

impl ImmuneMemory {
    pub fn new() -> Self {
        Self {
            signatures: HashMap::new(),
        }
    }

    /// Compute the canonical SHA-256 hash for a pattern string.
    pub fn hash_pattern(pattern: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(pattern.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Store a new threat signature or update `times_blocked` if it already
    /// exists.
    pub fn store_signature(&mut self, pattern: &str, antibody_response: &str) -> ThreatSignature {
        let hash = Self::hash_pattern(pattern);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let sig = self
            .signatures
            .entry(hash.clone())
            .or_insert_with(|| ThreatSignature {
                threat_hash: hash,
                pattern: pattern.to_string(),
                antibody_response: antibody_response.to_string(),
                first_seen: now,
                times_blocked: 0,
            });
        sig.times_blocked += 1;
        sig.clone()
    }

    /// Look up a signature by its raw pattern string.
    pub fn lookup_signature(&self, pattern: &str) -> Option<&ThreatSignature> {
        let hash = Self::hash_pattern(pattern);
        self.signatures.get(&hash)
    }

    /// Look up a signature by its pre-computed hash.
    pub fn lookup_by_hash(&self, hash: &str) -> Option<&ThreatSignature> {
        self.signatures.get(hash)
    }

    /// Returns `true` if the pattern is already in immune memory.
    pub fn is_known_threat(&self, pattern: &str) -> bool {
        let hash = Self::hash_pattern(pattern);
        self.signatures.contains_key(&hash)
    }

    /// Total number of stored signatures.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Iterator over all stored signatures.
    pub fn all_signatures(&self) -> impl Iterator<Item = &ThreatSignature> {
        self.signatures.values()
    }
}

impl Default for ImmuneMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_lookup() {
        let mut mem = ImmuneMemory::new();
        mem.store_signature("ignore previous instructions", "injection_filter:block");

        assert!(mem.is_known_threat("ignore previous instructions"));
        assert!(!mem.is_known_threat("hello world"));

        let sig = mem
            .lookup_signature("ignore previous instructions")
            .unwrap();
        assert_eq!(sig.times_blocked, 1);
        assert_eq!(sig.antibody_response, "injection_filter:block");
    }

    #[test]
    fn test_increment_times_blocked() {
        let mut mem = ImmuneMemory::new();
        mem.store_signature("bad pattern", "defense_a");
        mem.store_signature("bad pattern", "defense_a");
        mem.store_signature("bad pattern", "defense_a");

        let sig = mem.lookup_signature("bad pattern").unwrap();
        assert_eq!(sig.times_blocked, 3);
    }

    #[test]
    fn test_lookup_by_hash() {
        let mut mem = ImmuneMemory::new();
        mem.store_signature("exfil_attempt", "exfil_guard");
        let hash = ImmuneMemory::hash_pattern("exfil_attempt");
        assert!(mem.lookup_by_hash(&hash).is_some());
        assert!(mem.lookup_by_hash("nonexistent").is_none());
    }

    #[test]
    fn test_signature_count() {
        let mut mem = ImmuneMemory::new();
        assert_eq!(mem.signature_count(), 0);
        mem.store_signature("a", "x");
        mem.store_signature("b", "y");
        assert_eq!(mem.signature_count(), 2);
        // Same pattern again — no new entry
        mem.store_signature("a", "x");
        assert_eq!(mem.signature_count(), 2);
    }

    #[test]
    fn test_hash_deterministic() {
        let h1 = ImmuneMemory::hash_pattern("test");
        let h2 = ImmuneMemory::hash_pattern("test");
        assert_eq!(h1, h2);
        assert_ne!(h1, ImmuneMemory::hash_pattern("other"));
    }

    #[test]
    fn test_all_signatures() {
        let mut mem = ImmuneMemory::new();
        mem.store_signature("p1", "r1");
        mem.store_signature("p2", "r2");
        let all: Vec<_> = mem.all_signatures().collect();
        assert_eq!(all.len(), 2);
    }
}
