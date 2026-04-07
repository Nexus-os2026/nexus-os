//! In-memory verification ledger.
//!
//! Phase 1.1 keeps the ledger in a `HashMap`. Phase 1.2 backs it with
//! `nexus-memory`. Phase 4 adds the Ed25519-signed checkpoint described
//! in v1.1 §8 question 7.

use std::collections::HashMap;

/// In-memory ledger of `(fingerprint → status)` pairs.
#[derive(Debug, Default)]
pub struct VerificationLedger {
    entries: HashMap<String, String>,
}

impl VerificationLedger {
    /// Construct an empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a verification result against the given fingerprint.
    pub fn record(&mut self, fingerprint: String, status: String) {
        self.entries.insert(fingerprint, status);
    }

    /// Look up the recorded status for a fingerprint.
    pub fn lookup(&self, fingerprint: &str) -> Option<&String> {
        self.entries.get(fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_then_lookup() {
        let mut ledger = VerificationLedger::new();
        ledger.record("fp_abc".to_string(), "PASS".to_string());
        assert_eq!(ledger.lookup("fp_abc").map(String::as_str), Some("PASS"));
    }
}
