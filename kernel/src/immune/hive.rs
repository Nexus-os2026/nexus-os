//! Hive immunity — propagates successful defenses across all agents.
//!
//! When one agent's antibody neutralizes a threat, [`HiveImmunity`] broadcasts
//! the defense pattern to every other agent in the fleet, creating collective
//! immunity — analogous to herd immunity in biology.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::memory::ThreatSignature;

// ---------------------------------------------------------------------------
// ImmunityUpdate
// ---------------------------------------------------------------------------

/// A single propagation event: one agent sharing its defense with the hive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmunityUpdate {
    /// Agent that discovered and neutralized the threat.
    pub source_agent: String,
    /// The threat signature being shared.
    pub signature: ThreatSignature,
    /// UNIX timestamp when propagation occurred.
    pub propagated_at: u64,
}

// ---------------------------------------------------------------------------
// HiveImmunity
// ---------------------------------------------------------------------------

/// Collective defense network for all agents in the OS.
///
/// Stores shared immunity updates keyed by threat hash, so every agent can
/// query whether the hive already has a defense for a given threat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveImmunity {
    /// Shared immunity database: threat_hash → ImmunityUpdate.
    shared: HashMap<String, ImmunityUpdate>,
    /// Total propagation events.
    total_propagations: u64,
}

impl HiveImmunity {
    pub fn new() -> Self {
        Self {
            shared: HashMap::new(),
            total_propagations: 0,
        }
    }

    /// Propagate a threat signature from a source agent to the hive.
    ///
    /// If the signature is already shared, updates the propagation count but
    /// keeps the original entry.
    pub fn propagate(&mut self, source_agent: &str, signature: ThreatSignature) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.total_propagations += 1;
        self.shared
            .entry(signature.threat_hash.clone())
            .or_insert(ImmunityUpdate {
                source_agent: source_agent.to_string(),
                signature,
                propagated_at: now,
            });
    }

    /// Retrieve the shared defense for a given threat hash, if available.
    pub fn get_defense(&self, threat_hash: &str) -> Option<&ImmunityUpdate> {
        self.shared.get(threat_hash)
    }

    /// Check whether the hive already has immunity against a threat pattern.
    pub fn has_immunity(&self, threat_hash: &str) -> bool {
        self.shared.contains_key(threat_hash)
    }

    /// Get all shared immunity entries.
    pub fn get_shared_immunity(&self) -> Vec<&ImmunityUpdate> {
        self.shared.values().collect()
    }

    /// Total number of unique defenses in the hive.
    pub fn defense_count(&self) -> usize {
        self.shared.len()
    }

    /// Total propagation events (including duplicates).
    pub fn total_propagations(&self) -> u64 {
        self.total_propagations
    }
}

impl Default for HiveImmunity {
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

    fn make_sig(hash: &str, pattern: &str) -> ThreatSignature {
        ThreatSignature {
            threat_hash: hash.to_string(),
            pattern: pattern.to_string(),
            antibody_response: "block".to_string(),
            first_seen: 1000,
            times_blocked: 1,
        }
    }

    #[test]
    fn test_propagate_and_lookup() {
        let mut hive = HiveImmunity::new();
        let sig = make_sig("abc123", "ignore previous");
        hive.propagate("agent-1", sig);

        assert!(hive.has_immunity("abc123"));
        assert!(!hive.has_immunity("unknown"));

        let defense = hive.get_defense("abc123").unwrap();
        assert_eq!(defense.source_agent, "agent-1");
        assert_eq!(defense.signature.pattern, "ignore previous");
    }

    #[test]
    fn test_duplicate_propagation_idempotent() {
        let mut hive = HiveImmunity::new();
        let sig1 = make_sig("abc123", "pattern_a");
        let sig2 = make_sig("abc123", "pattern_a_v2");

        hive.propagate("agent-1", sig1);
        hive.propagate("agent-2", sig2);

        // Still only one unique defense
        assert_eq!(hive.defense_count(), 1);
        // But two propagation events
        assert_eq!(hive.total_propagations(), 2);
        // Original source preserved
        assert_eq!(hive.get_defense("abc123").unwrap().source_agent, "agent-1");
    }

    #[test]
    fn test_get_shared_immunity() {
        let mut hive = HiveImmunity::new();
        hive.propagate("a1", make_sig("h1", "p1"));
        hive.propagate("a2", make_sig("h2", "p2"));
        hive.propagate("a3", make_sig("h3", "p3"));

        let shared = hive.get_shared_immunity();
        assert_eq!(shared.len(), 3);
    }

    #[test]
    fn test_empty_hive() {
        let hive = HiveImmunity::new();
        assert_eq!(hive.defense_count(), 0);
        assert_eq!(hive.total_propagations(), 0);
        assert!(hive.get_shared_immunity().is_empty());
    }
}
