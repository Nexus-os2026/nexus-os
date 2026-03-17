//! Civilization Log — immutable hash-chain audit trail for governance actions.
//!
//! Every governance action (proposal, vote, election, dispute, token transfer)
//! is recorded with cryptographic hash-chain integrity, following the same
//! pattern as the kernel's `AuditTrail`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Types of governance events recorded in the civilization log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceEventType {
    ProposalCreated,
    VoteCast,
    RulePassed,
    ElectionHeld,
    DisputeFiled,
    DisputeResolved,
    TokensEarned,
    TokensSpent,
    Bankruptcy,
}

impl std::fmt::Display for GovernanceEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GovernanceEventType::ProposalCreated => write!(f, "ProposalCreated"),
            GovernanceEventType::VoteCast => write!(f, "VoteCast"),
            GovernanceEventType::RulePassed => write!(f, "RulePassed"),
            GovernanceEventType::ElectionHeld => write!(f, "ElectionHeld"),
            GovernanceEventType::DisputeFiled => write!(f, "DisputeFiled"),
            GovernanceEventType::DisputeResolved => write!(f, "DisputeResolved"),
            GovernanceEventType::TokensEarned => write!(f, "TokensEarned"),
            GovernanceEventType::TokensSpent => write!(f, "TokensSpent"),
            GovernanceEventType::Bankruptcy => write!(f, "Bankruptcy"),
        }
    }
}

/// A single governance event in the hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEvent {
    pub id: Uuid,
    pub event_type: GovernanceEventType,
    pub details: String,
    pub timestamp: u64,
    pub hash: String,
    pub prev_hash: String,
}

/// Civilization log error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LogError {
    #[error("hash chain integrity violation at event index {0}")]
    IntegrityViolation(usize),
    #[error("event serialization failed")]
    SerializationFailed,
}

/// Immutable append-only hash-chain log of all governance actions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CivilizationLog {
    events: Vec<GovernanceEvent>,
}

impl CivilizationLog {
    /// Create a new empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute SHA-256 hash for an event's content.
    fn compute_hash(
        prev_hash: &str,
        event_type: &GovernanceEventType,
        details: &str,
        timestamp: u64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(event_type.to_string().as_bytes());
        hasher.update(details.as_bytes());
        hasher.update(timestamp.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Append a governance event to the log. Returns the event.
    pub fn append_event(
        &mut self,
        event_type: GovernanceEventType,
        details: &str,
    ) -> Result<GovernanceEvent, LogError> {
        let prev_hash = self
            .events
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());

        let timestamp = now_secs();
        let hash = Self::compute_hash(&prev_hash, &event_type, details, timestamp);

        let event = GovernanceEvent {
            id: Uuid::new_v4(),
            event_type,
            details: details.to_string(),
            timestamp,
            hash,
            prev_hash,
        };

        self.events.push(event.clone());
        Ok(event)
    }

    /// Get all events in the log.
    pub fn get_events(&self) -> &[GovernanceEvent] {
        &self.events
    }

    /// Get events filtered by type.
    pub fn get_events_by_type(&self, event_type: &GovernanceEventType) -> Vec<&GovernanceEvent> {
        self.events
            .iter()
            .filter(|e| &e.event_type == event_type)
            .collect()
    }

    /// Verify the entire hash chain. Returns Ok(()) if valid, or the index
    /// of the first broken link.
    pub fn verify_chain(&self) -> Result<(), LogError> {
        for (i, event) in self.events.iter().enumerate() {
            let expected_prev = if i == 0 {
                GENESIS_HASH.to_string()
            } else {
                self.events[i - 1].hash.clone()
            };

            if event.prev_hash != expected_prev {
                return Err(LogError::IntegrityViolation(i));
            }

            let recomputed = Self::compute_hash(
                &event.prev_hash,
                &event.event_type,
                &event.details,
                event.timestamp,
            );
            if event.hash != recomputed {
                return Err(LogError::IntegrityViolation(i));
            }
        }

        Ok(())
    }

    /// Number of events in the log.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_verify() {
        let mut log = CivilizationLog::new();

        log.append_event(GovernanceEventType::ProposalCreated, "test proposal")
            .unwrap();
        log.append_event(GovernanceEventType::VoteCast, "vote on proposal")
            .unwrap();
        log.append_event(GovernanceEventType::RulePassed, "rule enforced")
            .unwrap();

        assert_eq!(log.len(), 3);
        assert!(log.verify_chain().is_ok());
    }

    #[test]
    fn genesis_hash_correct() {
        let mut log = CivilizationLog::new();
        let event = log
            .append_event(GovernanceEventType::ElectionHeld, "first election")
            .unwrap();
        assert_eq!(event.prev_hash, GENESIS_HASH);
    }

    #[test]
    fn chain_links_correctly() {
        let mut log = CivilizationLog::new();

        let e1 = log
            .append_event(GovernanceEventType::TokensEarned, "earned 10")
            .unwrap();
        let e2 = log
            .append_event(GovernanceEventType::TokensSpent, "spent 5")
            .unwrap();

        assert_eq!(e2.prev_hash, e1.hash);
    }

    #[test]
    fn tampered_chain_detected() {
        let mut log = CivilizationLog::new();

        log.append_event(GovernanceEventType::DisputeFiled, "dispute 1")
            .unwrap();
        log.append_event(GovernanceEventType::DisputeResolved, "resolved")
            .unwrap();

        // Tamper with the first event's hash.
        log.events[0].hash = "tampered_hash".to_string();

        let err = log.verify_chain().unwrap_err();
        assert!(matches!(err, LogError::IntegrityViolation(_)));
    }

    #[test]
    fn filter_by_type() {
        let mut log = CivilizationLog::new();

        log.append_event(GovernanceEventType::VoteCast, "vote 1")
            .unwrap();
        log.append_event(GovernanceEventType::ElectionHeld, "election")
            .unwrap();
        log.append_event(GovernanceEventType::VoteCast, "vote 2")
            .unwrap();

        let votes = log.get_events_by_type(&GovernanceEventType::VoteCast);
        assert_eq!(votes.len(), 2);
    }

    #[test]
    fn empty_log_verifies() {
        let log = CivilizationLog::new();
        assert!(log.verify_chain().is_ok());
        assert!(log.is_empty());
    }
}
