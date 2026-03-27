//! Hash-chained evaluation audit log.

use sha2::{Digest, Sha256};

use crate::framework::MeasurementSession;

/// An entry in the evaluation audit chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub session_id: String,
    pub agent_id: String,
    pub audit_hash: String,
    pub previous_hash: String,
    pub timestamp: u64,
}

/// Append a measurement session to the audit chain, returning the new entry.
pub fn append_to_chain(session: &MeasurementSession, previous_hash: &str) -> AuditEntry {
    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(session.id.to_string().as_bytes());
    hasher.update(session.audit_hash.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    AuditEntry {
        session_id: session.id.to_string(),
        agent_id: session.agent_id.clone(),
        audit_hash: hash,
        previous_hash: previous_hash.to_string(),
        timestamp: session.started_at,
    }
}

/// Verify a chain of audit entries.
pub fn verify_chain(entries: &[AuditEntry]) -> bool {
    if entries.is_empty() {
        return true;
    }
    for window in entries.windows(2) {
        if window[1].previous_hash != window[0].audit_hash {
            return false;
        }
    }
    true
}
