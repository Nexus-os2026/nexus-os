//! Roles & Elections — agents fill elected governance roles.
//!
//! Four roles: Coordinator (highest reputation), Auditor (monthly rotation),
//! Researcher (curiosity score), Guardian (security score). Elections are
//! scored and the winner serves a fixed term.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::log::{CivilizationLog, GovernanceEventType};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Governance roles that agents can be elected to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Overall coordination — highest reputation wins.
    Coordinator,
    /// Audit compliance — monthly rotation.
    Auditor,
    /// Research direction — curiosity score determines winner.
    Researcher,
    /// Security oversight — security score determines winner.
    Guardian,
}

impl Role {
    /// Default term length in seconds for this role.
    pub fn term_duration_secs(&self) -> u64 {
        match self {
            Role::Coordinator => 7 * 86_400, // 7 days
            Role::Auditor => 30 * 86_400,    // 30 days
            Role::Researcher => 14 * 86_400, // 14 days
            Role::Guardian => 14 * 86_400,   // 14 days
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Coordinator => write!(f, "Coordinator"),
            Role::Auditor => write!(f, "Auditor"),
            Role::Researcher => write!(f, "Researcher"),
            Role::Guardian => write!(f, "Guardian"),
        }
    }
}

/// A current role assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub role: Role,
    pub agent_id: String,
    pub elected_at: u64,
    pub term_expires_at: u64,
    pub election_score: f64,
}

/// A candidate in an election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub agent_id: String,
    pub score: f64,
}

/// A vote in a role election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleVote {
    pub voter_id: String,
    pub candidate_id: String,
    pub weight: f64,
}

/// A completed or in-progress election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Election {
    pub id: Uuid,
    pub role: Role,
    pub candidates: Vec<Candidate>,
    pub votes: Vec<RoleVote>,
    pub winner: Option<String>,
    pub held_at: u64,
}

/// Role manager error.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RoleError {
    #[error("no candidates for role {0}")]
    NoCandidates(String),
    #[error("role {0} is not yet expired")]
    TermNotExpired(String),
}

/// Manages elected roles and elections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleManager {
    assignments: Vec<RoleAssignment>,
    election_history: Vec<Election>,
}

impl RoleManager {
    /// Create a new role manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run an election for a role given a set of candidates with scores.
    /// The candidate with the highest score wins. Scores should reflect
    /// the appropriate metric for the role (reputation, curiosity, security).
    pub fn run_election(
        &mut self,
        role: Role,
        candidates: Vec<Candidate>,
        log: &mut CivilizationLog,
    ) -> Result<Election, RoleError> {
        if candidates.is_empty() {
            return Err(RoleError::NoCandidates(role.to_string()));
        }

        // Winner is the candidate with the highest score.
        let winner = candidates
            .iter()
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| RoleError::NoCandidates(role.to_string()))?;

        let now = now_secs();
        let term_duration = role.term_duration_secs();

        let election = Election {
            id: Uuid::new_v4(),
            role: role.clone(),
            candidates: candidates.clone(),
            votes: Vec::new(), // Score-based election, no separate votes needed.
            winner: Some(winner.agent_id.clone()),
            held_at: now,
        };

        // Remove existing assignment for this role.
        self.assignments.retain(|a| a.role != role);

        // Install new assignment.
        self.assignments.push(RoleAssignment {
            role: role.clone(),
            agent_id: winner.agent_id.clone(),
            elected_at: now,
            term_expires_at: now + term_duration,
            election_score: winner.score,
        });

        // Best-effort: audit election result; governance action succeeds regardless
        let _ = log.append_event(
            GovernanceEventType::ElectionHeld,
            &format!(
                "Election for {}: {} won with score {:.2} (from {} candidates)",
                role,
                winner.agent_id,
                winner.score,
                candidates.len()
            ),
        );

        self.election_history.push(election.clone());
        Ok(election)
    }

    /// Get all current role assignments.
    pub fn get_current_roles(&self) -> &[RoleAssignment] {
        &self.assignments
    }

    /// Check if a role's term has expired.
    pub fn is_term_expired(&self, role: &Role) -> bool {
        let now = now_secs();
        self.assignments
            .iter()
            .find(|a| &a.role == role)
            .map(|a| now >= a.term_expires_at)
            .unwrap_or(true) // No assignment means "expired" (needs election).
    }

    /// Get the agent currently holding a role.
    pub fn get_role_holder(&self, role: &Role) -> Option<&RoleAssignment> {
        self.assignments.iter().find(|a| &a.role == role)
    }

    /// Get election history.
    pub fn get_election_history(&self) -> &[Election] {
        &self.election_history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elect_coordinator() {
        let mut rm = RoleManager::new();
        let mut log = CivilizationLog::new();

        let candidates = vec![
            Candidate {
                agent_id: "agent-1".into(),
                score: 0.7,
            },
            Candidate {
                agent_id: "agent-2".into(),
                score: 0.95,
            },
            Candidate {
                agent_id: "agent-3".into(),
                score: 0.6,
            },
        ];

        let election = rm
            .run_election(Role::Coordinator, candidates, &mut log)
            .unwrap();
        assert_eq!(election.winner, Some("agent-2".to_string()));

        let roles = rm.get_current_roles();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].agent_id, "agent-2");
        assert_eq!(roles[0].role, Role::Coordinator);
    }

    #[test]
    fn no_candidates_error() {
        let mut rm = RoleManager::new();
        let mut log = CivilizationLog::new();

        let err = rm
            .run_election(Role::Auditor, vec![], &mut log)
            .unwrap_err();
        assert!(matches!(err, RoleError::NoCandidates(_)));
    }

    #[test]
    fn new_election_replaces_old() {
        let mut rm = RoleManager::new();
        let mut log = CivilizationLog::new();

        let c1 = vec![Candidate {
            agent_id: "a1".into(),
            score: 0.9,
        }];
        rm.run_election(Role::Guardian, c1, &mut log).unwrap();

        let c2 = vec![Candidate {
            agent_id: "a2".into(),
            score: 0.8,
        }];
        rm.run_election(Role::Guardian, c2, &mut log).unwrap();

        let roles = rm.get_current_roles();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].agent_id, "a2");
        assert_eq!(rm.get_election_history().len(), 2);
    }

    #[test]
    fn term_expired_check() {
        let rm = RoleManager::new();
        // No assignment means expired.
        assert!(rm.is_term_expired(&Role::Researcher));
    }
}
