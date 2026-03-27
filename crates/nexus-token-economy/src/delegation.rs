use serde::{Deserialize, Serialize};

use crate::coin::NexusCoin;
use crate::EconomyError;

/// A delegation contract between two agents.
/// Requester locks coins → Provider completes task → Escrow releases on verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    pub id: String,
    pub requester_id: String,
    pub provider_id: String,
    pub task_description: String,
    pub payment: NexusCoin,
    pub status: DelegationStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    /// Quality threshold for automatic release (0.0-1.0)
    pub quality_threshold: f64,
    /// Timeout in seconds — auto-refund if not completed
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegationStatus {
    /// Escrow locked, waiting for provider to accept
    Pending,
    /// Provider accepted, working on task
    InProgress,
    /// Provider completed, awaiting verification
    AwaitingVerification,
    /// Verified, payment released to provider
    Completed,
    /// Failed verification, payment refunded to requester
    Refunded,
    /// Timed out, payment refunded
    TimedOut,
    /// Cancelled by requester before provider accepted
    Cancelled,
}

/// Manages all active delegations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationManager {
    delegations: Vec<Delegation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegationOutcome {
    Released { payment: NexusCoin },
    Refunded { payment: NexusCoin, reason: String },
}

impl DelegationManager {
    pub fn new() -> Self {
        Self {
            delegations: Vec::new(),
        }
    }

    /// Create a new delegation (requester's coins are locked in escrow)
    pub fn create(
        &mut self,
        requester_id: String,
        provider_id: String,
        task_description: String,
        payment: NexusCoin,
        quality_threshold: f64,
        timeout_secs: u64,
    ) -> Delegation {
        let delegation = Delegation {
            id: uuid::Uuid::new_v4().to_string(),
            requester_id,
            provider_id,
            task_description,
            payment,
            status: DelegationStatus::Pending,
            created_at: epoch_now(),
            completed_at: None,
            quality_threshold: quality_threshold.clamp(0.0, 1.0),
            timeout_secs,
        };
        self.delegations.push(delegation.clone());
        delegation
    }

    /// Provider accepts the delegation
    pub fn accept(&mut self, delegation_id: &str) -> Result<(), EconomyError> {
        let d = self.find_mut(delegation_id)?;
        if d.status != DelegationStatus::Pending {
            return Err(EconomyError::DelegationError("Not in Pending state".into()));
        }
        d.status = DelegationStatus::InProgress;
        Ok(())
    }

    /// Provider submits completion
    pub fn submit_completion(&mut self, delegation_id: &str) -> Result<(), EconomyError> {
        let d = self.find_mut(delegation_id)?;
        if d.status != DelegationStatus::InProgress {
            return Err(EconomyError::DelegationError(
                "Not in InProgress state".into(),
            ));
        }
        d.status = DelegationStatus::AwaitingVerification;
        Ok(())
    }

    /// Verify and release payment (quality_score must meet threshold)
    pub fn verify_and_release(
        &mut self,
        delegation_id: &str,
        quality_score: f64,
    ) -> Result<DelegationOutcome, EconomyError> {
        let d = self.find_mut(delegation_id)?;
        if d.status != DelegationStatus::AwaitingVerification {
            return Err(EconomyError::DelegationError(
                "Not awaiting verification".into(),
            ));
        }

        if quality_score >= d.quality_threshold {
            d.status = DelegationStatus::Completed;
            d.completed_at = Some(epoch_now());
            Ok(DelegationOutcome::Released { payment: d.payment })
        } else {
            d.status = DelegationStatus::Refunded;
            d.completed_at = Some(epoch_now());
            Ok(DelegationOutcome::Refunded {
                payment: d.payment,
                reason: format!(
                    "Quality {:.2} below threshold {:.2}",
                    quality_score, d.quality_threshold
                ),
            })
        }
    }

    /// Cancel a pending delegation
    pub fn cancel(&mut self, delegation_id: &str) -> Result<NexusCoin, EconomyError> {
        let d = self.find_mut(delegation_id)?;
        if d.status != DelegationStatus::Pending {
            return Err(EconomyError::DelegationError(
                "Can only cancel Pending delegations".into(),
            ));
        }
        d.status = DelegationStatus::Cancelled;
        d.completed_at = Some(epoch_now());
        Ok(d.payment)
    }

    /// Check for timed-out delegations
    pub fn check_timeouts(&mut self) -> Vec<String> {
        let now = epoch_now();
        let mut timed_out = Vec::new();
        for d in &mut self.delegations {
            if (d.status == DelegationStatus::Pending || d.status == DelegationStatus::InProgress)
                && now.saturating_sub(d.created_at) >= d.timeout_secs
            {
                d.status = DelegationStatus::TimedOut;
                d.completed_at = Some(now);
                timed_out.push(d.id.clone());
            }
        }
        timed_out
    }

    fn find_mut(&mut self, id: &str) -> Result<&mut Delegation, EconomyError> {
        self.delegations
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| EconomyError::DelegationError(format!("Delegation {} not found", id)))
    }

    pub fn delegations(&self) -> &[Delegation] {
        &self.delegations
    }
}

impl Default for DelegationManager {
    fn default() -> Self {
        Self::new()
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_lifecycle() {
        let mut mgr = DelegationManager::new();
        let d = mgr.create(
            "requester".into(),
            "provider".into(),
            "do task".into(),
            NexusCoin::from_coins(5),
            0.7,
            3600,
        );
        let id = d.id.clone();

        mgr.accept(&id).unwrap();
        mgr.submit_completion(&id).unwrap();

        let outcome = mgr.verify_and_release(&id, 0.9).unwrap();
        match outcome {
            DelegationOutcome::Released { payment } => {
                assert_eq!(payment, NexusCoin::from_coins(5));
            }
            _ => panic!("Expected Released"),
        }
    }

    #[test]
    fn test_delegation_refund_on_low_quality() {
        let mut mgr = DelegationManager::new();
        let d = mgr.create(
            "requester".into(),
            "provider".into(),
            "do task".into(),
            NexusCoin::from_coins(5),
            0.8,
            3600,
        );
        let id = d.id.clone();

        mgr.accept(&id).unwrap();
        mgr.submit_completion(&id).unwrap();

        let outcome = mgr.verify_and_release(&id, 0.5).unwrap();
        match outcome {
            DelegationOutcome::Refunded { payment, .. } => {
                assert_eq!(payment, NexusCoin::from_coins(5));
            }
            _ => panic!("Expected Refunded"),
        }
    }

    #[test]
    fn test_delegation_timeout() {
        let mut mgr = DelegationManager::new();
        let d = mgr.create(
            "requester".into(),
            "provider".into(),
            "do task".into(),
            NexusCoin::from_coins(5),
            0.7,
            0, // zero timeout — immediately expired
        );
        let id = d.id.clone();

        // Sleep isn't needed — timeout_secs=0 means any positive elapsed time triggers it
        let timed_out = mgr.check_timeouts();
        assert!(timed_out.contains(&id));

        let status = &mgr
            .delegations()
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .status;
        assert_eq!(*status, DelegationStatus::TimedOut);
    }

    #[test]
    fn test_delegation_cancel() {
        let mut mgr = DelegationManager::new();
        let d = mgr.create(
            "requester".into(),
            "provider".into(),
            "do task".into(),
            NexusCoin::from_coins(5),
            0.7,
            3600,
        );
        let id = d.id.clone();

        let refund = mgr.cancel(&id).unwrap();
        assert_eq!(refund, NexusCoin::from_coins(5));
    }
}
