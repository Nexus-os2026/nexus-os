//! Dispute Resolution — arbitration when agents disagree.
//!
//! Disputes go through Open -> InReview -> Resolved/Escalated. An arbiter
//! (typically an elected Guardian or Auditor) reviews the issue and renders
//! a decision based on governance rules and precedent.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::log::{CivilizationLog, GovernanceEventType};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Lifecycle status of a dispute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    Open,
    InReview,
    Resolved,
    Escalated,
}

/// A dispute between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: Uuid,
    pub agent_a: String,
    pub agent_b: String,
    pub issue: String,
    pub status: DisputeStatus,
    pub resolution: Option<String>,
    pub arbiter_id: Option<String>,
    pub created_at: u64,
}

/// Dispute resolution error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DisputeError {
    #[error("dispute {0} not found")]
    NotFound(Uuid),
    #[error("dispute {0} is not in the expected status for this operation")]
    InvalidStatus(Uuid),
    #[error("no arbiter assigned to dispute {0}")]
    NoArbiter(Uuid),
}

/// Manages dispute filing, arbitration, and resolution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisputeResolver {
    disputes: Vec<Dispute>,
}

impl DisputeResolver {
    /// Create a new dispute resolver.
    pub fn new() -> Self {
        Self::default()
    }

    /// File a new dispute between two agents.
    pub fn file_dispute(
        &mut self,
        agent_a: &str,
        agent_b: &str,
        issue: &str,
        log: &mut CivilizationLog,
    ) -> Dispute {
        let dispute = Dispute {
            id: Uuid::new_v4(),
            agent_a: agent_a.to_string(),
            agent_b: agent_b.to_string(),
            issue: issue.to_string(),
            status: DisputeStatus::Open,
            resolution: None,
            arbiter_id: None,
            created_at: now_secs(),
        };

        // Best-effort: audit dispute filing; dispute is recorded regardless of log failure
        let _ = log.append_event(
            GovernanceEventType::DisputeFiled,
            &format!(
                "Dispute {} filed: {} vs {} — {}",
                dispute.id, agent_a, agent_b, issue
            ),
        );

        self.disputes.push(dispute.clone());
        dispute
    }

    /// Assign an arbiter to review a dispute.
    pub fn assign_arbiter(
        &mut self,
        dispute_id: Uuid,
        arbiter_id: &str,
        _log: &mut CivilizationLog,
    ) -> Result<Dispute, DisputeError> {
        let dispute = self
            .disputes
            .iter_mut()
            .find(|d| d.id == dispute_id)
            .ok_or(DisputeError::NotFound(dispute_id))?;

        if dispute.status != DisputeStatus::Open {
            return Err(DisputeError::InvalidStatus(dispute_id));
        }

        dispute.arbiter_id = Some(arbiter_id.to_string());
        dispute.status = DisputeStatus::InReview;

        Ok(dispute.clone())
    }

    /// Resolve a dispute with a decision.
    pub fn resolve_dispute(
        &mut self,
        dispute_id: Uuid,
        resolution: &str,
        log: &mut CivilizationLog,
    ) -> Result<Dispute, DisputeError> {
        let dispute = self
            .disputes
            .iter_mut()
            .find(|d| d.id == dispute_id)
            .ok_or(DisputeError::NotFound(dispute_id))?;

        if dispute.status != DisputeStatus::InReview {
            return Err(DisputeError::InvalidStatus(dispute_id));
        }

        if dispute.arbiter_id.is_none() {
            return Err(DisputeError::NoArbiter(dispute_id));
        }

        dispute.resolution = Some(resolution.to_string());
        dispute.status = DisputeStatus::Resolved;

        // Best-effort: audit dispute resolution; resolution state is already committed
        let _ = log.append_event(
            GovernanceEventType::DisputeResolved,
            &format!(
                "Dispute {} resolved by {}: {}",
                dispute_id,
                dispute.arbiter_id.as_deref().unwrap_or("unknown"),
                resolution
            ),
        );

        Ok(dispute.clone())
    }

    /// Escalate a dispute to human review (HITL).
    pub fn escalate_to_human(
        &mut self,
        dispute_id: Uuid,
        log: &mut CivilizationLog,
    ) -> Result<Dispute, DisputeError> {
        let dispute = self
            .disputes
            .iter_mut()
            .find(|d| d.id == dispute_id)
            .ok_or(DisputeError::NotFound(dispute_id))?;

        if dispute.status == DisputeStatus::Resolved {
            return Err(DisputeError::InvalidStatus(dispute_id));
        }

        dispute.status = DisputeStatus::Escalated;

        // Best-effort: audit dispute escalation; HITL escalation state is already committed
        let _ = log.append_event(
            GovernanceEventType::DisputeFiled,
            &format!("Dispute {} escalated to human review", dispute_id),
        );

        Ok(dispute.clone())
    }

    /// Get all disputes.
    pub fn get_disputes(&self) -> &[Dispute] {
        &self.disputes
    }

    /// Get disputes by status.
    pub fn get_disputes_by_status(&self, status: &DisputeStatus) -> Vec<&Dispute> {
        self.disputes
            .iter()
            .filter(|d| &d.status == status)
            .collect()
    }

    /// Get a dispute by ID.
    pub fn get_dispute(&self, dispute_id: Uuid) -> Option<&Dispute> {
        self.disputes.iter().find(|d| d.id == dispute_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_dispute_lifecycle() {
        let mut resolver = DisputeResolver::new();
        let mut log = CivilizationLog::new();

        // File dispute.
        let dispute =
            resolver.file_dispute("agent-1", "agent-2", "Unauthorized data access", &mut log);
        assert_eq!(dispute.status, DisputeStatus::Open);

        // Assign arbiter.
        let dispute = resolver
            .assign_arbiter(dispute.id, "guardian-1", &mut log)
            .unwrap();
        assert_eq!(dispute.status, DisputeStatus::InReview);
        assert_eq!(dispute.arbiter_id, Some("guardian-1".to_string()));

        // Resolve.
        let dispute = resolver
            .resolve_dispute(dispute.id, "agent-1 violated access policy", &mut log)
            .unwrap();
        assert_eq!(dispute.status, DisputeStatus::Resolved);
        assert!(dispute.resolution.is_some());
    }

    #[test]
    fn escalation_works() {
        let mut resolver = DisputeResolver::new();
        let mut log = CivilizationLog::new();

        let dispute = resolver.file_dispute("a", "b", "complex issue", &mut log);
        let dispute = resolver.escalate_to_human(dispute.id, &mut log).unwrap();
        assert_eq!(dispute.status, DisputeStatus::Escalated);
    }

    #[test]
    fn cannot_resolve_open_dispute() {
        let mut resolver = DisputeResolver::new();
        let mut log = CivilizationLog::new();

        let dispute = resolver.file_dispute("a", "b", "issue", &mut log);
        let err = resolver
            .resolve_dispute(dispute.id, "resolution", &mut log)
            .unwrap_err();
        assert!(matches!(err, DisputeError::InvalidStatus(_)));
    }

    #[test]
    fn cannot_assign_arbiter_twice() {
        let mut resolver = DisputeResolver::new();
        let mut log = CivilizationLog::new();

        let dispute = resolver.file_dispute("a", "b", "issue", &mut log);
        resolver
            .assign_arbiter(dispute.id, "arb-1", &mut log)
            .unwrap();

        // Now in InReview, can't assign again.
        let err = resolver
            .assign_arbiter(dispute.id, "arb-2", &mut log)
            .unwrap_err();
        assert!(matches!(err, DisputeError::InvalidStatus(_)));
    }

    #[test]
    fn filter_by_status() {
        let mut resolver = DisputeResolver::new();
        let mut log = CivilizationLog::new();

        resolver.file_dispute("a", "b", "issue1", &mut log);
        resolver.file_dispute("c", "d", "issue2", &mut log);

        let open = resolver.get_disputes_by_status(&DisputeStatus::Open);
        assert_eq!(open.len(), 2);
    }
}
