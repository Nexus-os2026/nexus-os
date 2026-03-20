//! Parliament — agents propose and vote on governance rules.
//!
//! Voting weight is proportional to reputation score. Rules auto-enforce
//! after majority approval.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::log::{CivilizationLog, GovernanceEventType};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Lifecycle status of a governance proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Expired,
}

/// A governance rule proposal submitted by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: Uuid,
    pub proposer_id: String,
    pub rule_text: String,
    pub votes_for: u32,
    pub votes_against: u32,
    pub status: ProposalStatus,
    pub created_at: u64,
    pub expires_at: u64,
}

/// A single vote on a proposal, weighted by reputation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter_id: String,
    pub proposal_id: Uuid,
    pub in_favor: bool,
    pub weight: f64,
    pub cast_at: u64,
}

/// Parliament error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParliamentError {
    #[error("proposal {0} not found")]
    ProposalNotFound(Uuid),
    #[error("proposal {0} is not active")]
    ProposalNotActive(Uuid),
    #[error("agent {0} has already voted on proposal {1}")]
    AlreadyVoted(String, Uuid),
    #[error("invalid voting weight: {0}")]
    InvalidWeight(String),
}

/// The parliament where agents propose and vote on governance rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parliament {
    proposals: Vec<Proposal>,
    votes: Vec<Vote>,
    /// Default proposal duration in seconds (24 hours).
    pub proposal_duration_secs: u64,
}

impl Default for Parliament {
    fn default() -> Self {
        Self {
            proposals: Vec::new(),
            votes: Vec::new(),
            proposal_duration_secs: 86_400,
        }
    }
}

impl Parliament {
    /// Create a new empty parliament.
    pub fn new() -> Self {
        Self::default()
    }

    /// Submit a new governance rule proposal.
    pub fn propose_rule(
        &mut self,
        proposer_id: &str,
        rule_text: &str,
        log: &mut CivilizationLog,
    ) -> Proposal {
        let now = now_secs();
        let proposal = Proposal {
            id: Uuid::new_v4(),
            proposer_id: proposer_id.to_string(),
            rule_text: rule_text.to_string(),
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Active,
            created_at: now,
            expires_at: now + self.proposal_duration_secs,
        };
        let _ = log.append_event(
            GovernanceEventType::ProposalCreated,
            &format!("Proposal {} by {}: {}", proposal.id, proposer_id, rule_text),
        );
        self.proposals.push(proposal.clone());
        proposal
    }

    /// Cast a weighted vote on an active proposal.
    pub fn cast_vote(
        &mut self,
        voter_id: &str,
        proposal_id: Uuid,
        in_favor: bool,
        reputation_score: f64,
        log: &mut CivilizationLog,
    ) -> Result<Vote, ParliamentError> {
        if !(0.0..=1.0).contains(&reputation_score) {
            return Err(ParliamentError::InvalidWeight(format!(
                "reputation_score must be in [0.0, 1.0], got {reputation_score}"
            )));
        }

        // Expire proposals first.
        self.expire_stale_proposals();

        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or(ParliamentError::ProposalNotFound(proposal_id))?;

        if proposal.status != ProposalStatus::Active {
            return Err(ParliamentError::ProposalNotActive(proposal_id));
        }

        // Check duplicate vote.
        if self
            .votes
            .iter()
            .any(|v| v.voter_id == voter_id && v.proposal_id == proposal_id)
        {
            return Err(ParliamentError::AlreadyVoted(
                voter_id.to_string(),
                proposal_id,
            ));
        }

        let weight = 0.1 + reputation_score * 0.9; // min weight 0.1, max 1.0
        let vote = Vote {
            voter_id: voter_id.to_string(),
            proposal_id,
            in_favor,
            weight,
            cast_at: now_secs(),
        };

        if in_favor {
            proposal.votes_for += 1;
        } else {
            proposal.votes_against += 1;
        }

        let _ = log.append_event(
            GovernanceEventType::VoteCast,
            &format!(
                "Vote by {} on {}: favor={}, weight={:.2}",
                voter_id, proposal_id, in_favor, weight
            ),
        );

        self.votes.push(vote.clone());
        Ok(vote)
    }

    /// Tally weighted votes for a proposal and update its status.
    /// Returns the updated proposal.
    pub fn tally_votes(
        &mut self,
        proposal_id: Uuid,
        log: &mut CivilizationLog,
    ) -> Result<Proposal, ParliamentError> {
        self.expire_stale_proposals();

        let proposal = self
            .proposals
            .iter()
            .find(|p| p.id == proposal_id)
            .ok_or(ParliamentError::ProposalNotFound(proposal_id))?;

        if proposal.status != ProposalStatus::Active {
            return Ok(proposal.clone());
        }

        let relevant_votes: Vec<&Vote> = self
            .votes
            .iter()
            .filter(|v| v.proposal_id == proposal_id)
            .collect();

        if relevant_votes.is_empty() {
            return Ok(proposal.clone());
        }

        let weighted_for: f64 = relevant_votes
            .iter()
            .filter(|v| v.in_favor)
            .map(|v| v.weight)
            .sum();
        let weighted_against: f64 = relevant_votes
            .iter()
            .filter(|v| !v.in_favor)
            .map(|v| v.weight)
            .sum();
        let total = weighted_for + weighted_against;

        let new_status = if total > 0.0 && weighted_for / total > 0.5 {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        // Must re-borrow mutably.
        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or(ParliamentError::ProposalNotFound(proposal_id))?;
        proposal.status = new_status.clone();

        if new_status == ProposalStatus::Passed {
            let _ = log.append_event(
                GovernanceEventType::RulePassed,
                &format!(
                    "Rule passed: {} (proposal {})",
                    proposal.rule_text, proposal_id
                ),
            );
        }

        Ok(proposal.clone())
    }

    /// Get all currently active proposals.
    pub fn get_active_proposals(&mut self) -> Vec<Proposal> {
        self.expire_stale_proposals();
        self.proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Active)
            .cloned()
            .collect()
    }

    /// Get all passed rules.
    pub fn get_passed_rules(&self) -> Vec<Proposal> {
        self.proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Passed)
            .cloned()
            .collect()
    }

    /// Expire proposals past their deadline.
    fn expire_stale_proposals(&mut self) {
        let now = now_secs();
        for p in &mut self.proposals {
            if p.status == ProposalStatus::Active && now >= p.expires_at {
                p.status = ProposalStatus::Expired;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propose_and_vote_passes() {
        let mut parliament = Parliament::new();
        let mut log = CivilizationLog::new();

        let proposal =
            parliament.propose_rule("agent-1", "All agents must log fuel usage", &mut log);
        assert_eq!(proposal.status, ProposalStatus::Active);

        // Three votes in favor, one against.
        parliament
            .cast_vote("agent-2", proposal.id, true, 0.9, &mut log)
            .unwrap();
        parliament
            .cast_vote("agent-3", proposal.id, true, 0.8, &mut log)
            .unwrap();
        parliament
            .cast_vote("agent-4", proposal.id, false, 0.5, &mut log)
            .unwrap();

        let result = parliament.tally_votes(proposal.id, &mut log).unwrap();
        assert_eq!(result.status, ProposalStatus::Passed);
        assert_eq!(result.votes_for, 2);
        assert_eq!(result.votes_against, 1);
    }

    #[test]
    fn duplicate_vote_rejected() {
        let mut parliament = Parliament::new();
        let mut log = CivilizationLog::new();

        let proposal = parliament.propose_rule("agent-1", "Test rule", &mut log);
        parliament
            .cast_vote("agent-2", proposal.id, true, 0.5, &mut log)
            .unwrap();

        let err = parliament
            .cast_vote("agent-2", proposal.id, false, 0.5, &mut log)
            .unwrap_err();
        assert!(matches!(err, ParliamentError::AlreadyVoted(_, _)));
    }

    #[test]
    fn invalid_weight_rejected() {
        let mut parliament = Parliament::new();
        let mut log = CivilizationLog::new();
        let proposal = parliament.propose_rule("agent-1", "rule", &mut log);

        let err = parliament
            .cast_vote("agent-2", proposal.id, true, 1.5, &mut log)
            .unwrap_err();
        assert!(matches!(err, ParliamentError::InvalidWeight(_)));
    }

    #[test]
    fn passed_rules_collected() {
        let mut parliament = Parliament::new();
        let mut log = CivilizationLog::new();

        let p1 = parliament.propose_rule("a1", "rule1", &mut log);
        parliament
            .cast_vote("a2", p1.id, true, 0.9, &mut log)
            .unwrap();
        parliament.tally_votes(p1.id, &mut log).unwrap();

        let p2 = parliament.propose_rule("a1", "rule2", &mut log);
        parliament
            .cast_vote("a2", p2.id, false, 0.9, &mut log)
            .unwrap();
        parliament.tally_votes(p2.id, &mut log).unwrap();

        let passed = parliament.get_passed_rules();
        assert_eq!(passed.len(), 1);
        assert_eq!(passed[0].rule_text, "rule1");
    }
}
