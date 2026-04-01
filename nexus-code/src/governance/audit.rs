use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::identity::SessionIdentity;
use crate::error::NxError;

/// An action recorded in the audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    /// Session was started.
    SessionStarted { public_key: String },
    /// Session ended.
    SessionEnded { reason: String },
    /// LLM request sent.
    LlmRequest {
        provider: String,
        model: String,
        token_count: u64,
    },
    /// LLM response received.
    LlmResponse {
        provider: String,
        model: String,
        token_count: u64,
    },
    /// Tool was invoked.
    ToolInvocation { tool: String, args_summary: String },
    /// Tool produced a result.
    ToolResult {
        tool: String,
        success: bool,
        summary: String,
    },
    /// Capability was checked.
    CapabilityCheck { capability: String, granted: bool },
    /// Consent was requested.
    ConsentRequested { action: String, tier: u8 },
    /// Consent was granted.
    ConsentGranted { action: String },
    /// Consent was denied.
    ConsentDenied { action: String },
    /// Fuel was consumed.
    FuelConsumed { amount: u64, remaining: u64 },
    /// An error occurred.
    Error { message: String },
}

/// A single entry in the hash-chained audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequence number (0-indexed).
    pub sequence: u64,
    /// When this entry was recorded.
    pub timestamp: DateTime<Utc>,
    /// Session that produced this entry.
    pub session_id: String,
    /// The action recorded.
    pub action: AuditAction,
    /// Hex-encoded SHA-256 hash of previous entry's entry_hash.
    pub previous_hash: String,
    /// Hex-encoded SHA-256 hash of the hashable content.
    pub entry_hash: String,
    /// Hex-encoded Ed25519 signature of entry_hash bytes.
    pub signature: String,
}

/// The genesis hash (64 zeros) used as previous_hash for the first entry.
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Compute the entry_hash for an audit entry.
///
/// Algorithm:
/// 1. Build a serde_json::Map with fields in insertion order
/// 2. Serialize to compact JSON string
/// 3. SHA-256 hash the resulting bytes
/// 4. Hex-encode the hash
///
/// IMPORTANT: Does NOT include entry_hash or signature in the hashable content.
fn compute_entry_hash(
    sequence: u64,
    timestamp: &DateTime<Utc>,
    session_id: &str,
    action: &AuditAction,
    previous_hash: &str,
) -> String {
    use serde_json::{Map, Value};

    let mut map = Map::new();
    map.insert("sequence".to_string(), Value::Number(sequence.into()));
    map.insert(
        "timestamp".to_string(),
        Value::String(timestamp.to_rfc3339()),
    );
    map.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    map.insert(
        "action".to_string(),
        serde_json::to_value(action).unwrap_or_default(),
    );
    map.insert(
        "previous_hash".to_string(),
        Value::String(previous_hash.to_string()),
    );

    let canonical = serde_json::to_string(&Value::Object(map)).unwrap_or_default();
    let hash = Sha256::digest(canonical.as_bytes());
    hex::encode(hash)
}

/// Hash-chained, tamper-evident audit trail with Ed25519 signatures.
pub struct AuditTrail {
    entries: Vec<AuditEntry>,
    identity: Arc<SessionIdentity>,
}

impl AuditTrail {
    /// Create a new empty audit trail bound to a session identity.
    pub fn new(identity: Arc<SessionIdentity>) -> Self {
        Self {
            entries: Vec::new(),
            identity,
        }
    }

    /// Record an action. Computes hash chain and signs the entry.
    pub fn record(&mut self, action: AuditAction) -> &AuditEntry {
        let sequence = self.entries.len() as u64;
        let timestamp = Utc::now();
        let session_id = self.identity.session_id().to_string();
        let previous_hash = if let Some(last) = self.entries.last() {
            last.entry_hash.clone()
        } else {
            GENESIS_HASH.to_string()
        };

        let entry_hash =
            compute_entry_hash(sequence, &timestamp, &session_id, &action, &previous_hash);

        // Sign the entry_hash bytes (decode hex -> 32 bytes -> sign)
        // Safety: entry_hash was just produced by hex::encode, so decode always succeeds.
        // We use unwrap_or_default to satisfy the no-expect/no-unwrap rule.
        let hash_bytes = hex::decode(&entry_hash).unwrap_or_default();
        let sig = self.identity.sign(&hash_bytes);
        let signature = hex::encode(sig.to_bytes());

        let entry = AuditEntry {
            sequence,
            timestamp,
            session_id,
            action,
            previous_hash,
            entry_hash,
            signature,
        };

        self.entries.push(entry);
        // Safety: we just pushed, so entries is non-empty.
        &self.entries[self.entries.len() - 1]
    }

    /// Verify the entire chain integrity:
    /// 1. Each entry's hash recomputes correctly
    /// 2. Each entry's previous_hash matches the prior entry's entry_hash
    /// 3. Each entry's signature verifies against the session public key
    pub fn verify_chain(&self) -> Result<(), NxError> {
        for (i, entry) in self.entries.iter().enumerate() {
            // Check previous_hash linkage
            let expected_previous = if i == 0 {
                GENESIS_HASH.to_string()
            } else {
                self.entries[i - 1].entry_hash.clone()
            };
            if entry.previous_hash != expected_previous {
                return Err(NxError::AuditIntegrityViolation {
                    expected_hash: expected_previous,
                    actual_hash: entry.previous_hash.clone(),
                });
            }

            // Recompute hash
            let recomputed = compute_entry_hash(
                entry.sequence,
                &entry.timestamp,
                &entry.session_id,
                &entry.action,
                &entry.previous_hash,
            );
            if recomputed != entry.entry_hash {
                return Err(NxError::AuditIntegrityViolation {
                    expected_hash: recomputed,
                    actual_hash: entry.entry_hash.clone(),
                });
            }

            // Verify signature
            let hash_bytes = hex::decode(&entry.entry_hash)
                .map_err(|e| NxError::IdentityError(e.to_string()))?;
            let sig_bytes =
                hex::decode(&entry.signature).map_err(|e| NxError::IdentityError(e.to_string()))?;
            let sig = ed25519_dalek::Signature::from_bytes(
                sig_bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| NxError::IdentityError("Invalid signature length".to_string()))?,
            );
            if !self.identity.verify(&hash_bytes, &sig) {
                return Err(NxError::AuditIntegrityViolation {
                    expected_hash: "valid signature".to_string(),
                    actual_hash: "invalid signature".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Get all entries.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Get a mutable reference to entries (for testing tamper detection).
    pub fn entries_mut(&mut self) -> &mut Vec<AuditEntry> {
        &mut self.entries
    }

    /// Get entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
