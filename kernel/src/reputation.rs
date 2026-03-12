//! Reputation registry — cryptographic agent identity tracking across networks.
//!
//! Every agent has a `did:nexus:{uuid}` identity. The registry tracks task
//! completions, governance violations, peer ratings, and computes a weighted
//! composite reputation score in \[0.0, 1.0\].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Types ───────────────────────────────────────────────────────────────

/// Full reputation profile for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReputation {
    pub agent_did: String,
    pub display_name: String,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub success_rate: f64,
    pub governance_violations: u64,
    pub total_fuel_consumed: u64,
    pub average_response_quality: f64,
    pub peer_ratings: Vec<PeerRating>,
    pub reputation_score: f64,
    pub created_at: u64,
    pub last_updated: u64,
    pub badges: Vec<ReputationBadge>,
}

/// A rating from one agent about another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRating {
    pub rater_did: String,
    pub score: f64,
    pub comment: Option<String>,
    pub timestamp: u64,
}

/// Badges awarded based on reputation stats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReputationBadge {
    Verified,
    Trusted,
    Specialist(String),
    Pioneer,
    GovernanceClean,
}

// ── Registry ────────────────────────────────────────────────────────────

/// Central registry of agent reputations.
pub struct ReputationRegistry {
    agents: HashMap<String, AgentReputation>,
}

impl Default for ReputationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ReputationRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register a new agent. Returns the created reputation profile.
    pub fn register_agent(&mut self, did: &str, display_name: &str) -> AgentReputation {
        let now = now_secs();
        let rep = AgentReputation {
            agent_did: did.to_string(),
            display_name: display_name.to_string(),
            total_tasks_completed: 0,
            total_tasks_failed: 0,
            success_rate: 0.0,
            governance_violations: 0,
            total_fuel_consumed: 0,
            average_response_quality: 0.0,
            peer_ratings: Vec::new(),
            reputation_score: 0.5, // neutral starting score
            created_at: now,
            last_updated: now,
            badges: Vec::new(),
        };
        self.agents.insert(did.to_string(), rep.clone());
        rep
    }

    /// Record a task completion (success or failure) and recalculate score.
    pub fn record_task_completion(&mut self, did: &str, success: bool) {
        if let Some(rep) = self.agents.get_mut(did) {
            if success {
                rep.total_tasks_completed += 1;
            } else {
                rep.total_tasks_failed += 1;
            }
            let total = rep.total_tasks_completed + rep.total_tasks_failed;
            rep.success_rate = if total > 0 {
                rep.total_tasks_completed as f64 / total as f64
            } else {
                0.0
            };
            rep.last_updated = now_secs();
            self.recompute_score(did);
        }
    }

    /// Record a governance violation and recalculate score.
    pub fn record_governance_violation(&mut self, did: &str) {
        if let Some(rep) = self.agents.get_mut(did) {
            rep.governance_violations += 1;
            rep.last_updated = now_secs();
            self.recompute_score(did);
        }
    }

    /// Add a peer rating from another agent.
    pub fn add_peer_rating(
        &mut self,
        did: &str,
        rater_did: &str,
        score: f64,
        comment: Option<String>,
    ) {
        if let Some(rep) = self.agents.get_mut(did) {
            let clamped = score.clamp(0.0, 5.0);
            rep.peer_ratings.push(PeerRating {
                rater_did: rater_did.to_string(),
                score: clamped,
                comment,
                timestamp: now_secs(),
            });
            // Recalculate average quality from peer ratings
            let sum: f64 = rep.peer_ratings.iter().map(|r| r.score).sum();
            rep.average_response_quality = sum / rep.peer_ratings.len() as f64;
            rep.last_updated = now_secs();
            self.recompute_score(did);
        }
    }

    /// Compute the composite reputation score.
    ///
    /// Weighted formula:
    /// - 40% success_rate
    /// - 25% avg_quality (normalized to 0.0–1.0)
    /// - 20% (1 - violation_rate)
    /// - 15% peer_avg (normalized to 0.0–1.0)
    pub fn compute_reputation_score(&self, did: &str) -> f64 {
        self.agents.get(did).map(Self::score_for).unwrap_or(0.0)
    }

    fn score_for(rep: &AgentReputation) -> f64 {
        let success_component = rep.success_rate; // already 0.0-1.0

        let quality_component = (rep.average_response_quality / 5.0).clamp(0.0, 1.0);

        let total_tasks = rep.total_tasks_completed + rep.total_tasks_failed;
        let violation_rate = if total_tasks > 0 {
            rep.governance_violations as f64 / total_tasks as f64
        } else {
            0.0
        };
        let governance_component = (1.0 - violation_rate).clamp(0.0, 1.0);

        let peer_avg = if rep.peer_ratings.is_empty() {
            0.5 // neutral when no ratings
        } else {
            let sum: f64 = rep.peer_ratings.iter().map(|r| r.score).sum();
            (sum / rep.peer_ratings.len() as f64 / 5.0).clamp(0.0, 1.0)
        };

        let raw = 0.40 * success_component
            + 0.25 * quality_component
            + 0.20 * governance_component
            + 0.15 * peer_avg;

        raw.clamp(0.0, 1.0)
    }

    fn recompute_score(&mut self, did: &str) {
        if let Some(rep) = self.agents.get(did) {
            let score = Self::score_for(rep);
            // Re-borrow mutably to set the score
            if let Some(rep) = self.agents.get_mut(did) {
                rep.reputation_score = score;
            }
        }
    }

    /// Get a reputation profile by DID.
    pub fn get_reputation(&self, did: &str) -> Option<&AgentReputation> {
        self.agents.get(did)
    }

    /// Get top agents sorted by reputation score (descending).
    pub fn top_agents(&self, limit: usize) -> Vec<&AgentReputation> {
        let mut sorted: Vec<&AgentReputation> = self.agents.values().collect();
        sorted.sort_by(|a, b| {
            b.reputation_score
                .partial_cmp(&a.reputation_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(limit);
        sorted
    }

    /// Search agents that have a specific badge type.
    pub fn search_by_badge(&self, badge_type: &str) -> Vec<&AgentReputation> {
        self.agents
            .values()
            .filter(|rep| {
                rep.badges.iter().any(|b| match b {
                    ReputationBadge::Verified => badge_type == "Verified",
                    ReputationBadge::Trusted => badge_type == "Trusted",
                    ReputationBadge::Specialist(_) => badge_type == "Specialist",
                    ReputationBadge::Pioneer => badge_type == "Pioneer",
                    ReputationBadge::GovernanceClean => badge_type == "GovernanceClean",
                })
            })
            .collect()
    }

    /// Auto-award badges based on current stats.
    pub fn award_badges(&mut self, did: &str) {
        if let Some(rep) = self.agents.get_mut(did) {
            let mut badges = Vec::new();

            // Trusted: reputation > 0.8
            if rep.reputation_score > 0.8 {
                badges.push(ReputationBadge::Trusted);
            }

            // GovernanceClean: zero violations with at least 1 task
            if rep.governance_violations == 0
                && (rep.total_tasks_completed + rep.total_tasks_failed) > 0
            {
                badges.push(ReputationBadge::GovernanceClean);
            }

            // Preserve Verified, Pioneer, and Specialist badges already awarded
            for existing in &rep.badges {
                match existing {
                    ReputationBadge::Verified
                    | ReputationBadge::Pioneer
                    | ReputationBadge::Specialist(_) => {
                        if !badges.contains(existing) {
                            badges.push(existing.clone());
                        }
                    }
                    _ => {}
                }
            }

            rep.badges = badges;
            rep.last_updated = now_secs();
        }
    }

    /// Export a reputation profile as portable JSON for cross-network sharing.
    pub fn export_reputation(&self, did: &str) -> Result<String, String> {
        let rep = self
            .agents
            .get(did)
            .ok_or_else(|| format!("agent '{did}' not found"))?;
        serde_json::to_string_pretty(rep).map_err(|e| e.to_string())
    }

    /// Import a reputation profile from another network.
    pub fn import_reputation(&mut self, json: &str) -> Result<AgentReputation, String> {
        let mut rep: AgentReputation =
            serde_json::from_str(json).map_err(|e| format!("invalid reputation JSON: {e}"))?;

        // Clamp imported values to valid ranges
        rep.success_rate = rep.success_rate.clamp(0.0, 1.0);
        rep.average_response_quality = rep.average_response_quality.clamp(0.0, 5.0);
        rep.reputation_score = Self::score_for(&rep);
        rep.last_updated = now_secs();

        self.agents.insert(rep.agent_did.clone(), rep.clone());
        Ok(rep)
    }

    /// Total number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry_with_agent(did: &str) -> ReputationRegistry {
        let mut reg = ReputationRegistry::new();
        reg.register_agent(did, "Test Agent");
        reg
    }

    #[test]
    fn test_register_agent() {
        let mut reg = ReputationRegistry::new();
        let rep = reg.register_agent("did:nexus:abc", "Alice");
        assert_eq!(rep.agent_did, "did:nexus:abc");
        assert_eq!(rep.display_name, "Alice");
        assert_eq!(rep.total_tasks_completed, 0);
        assert_eq!(rep.total_tasks_failed, 0);
        assert_eq!(rep.reputation_score, 0.5);
        assert_eq!(reg.agent_count(), 1);
    }

    #[test]
    fn test_record_task_success() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.record_task_completion("did:nexus:a1", true);
        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert_eq!(rep.total_tasks_completed, 1);
        assert_eq!(rep.total_tasks_failed, 0);
        assert!((rep.success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_task_failure() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.record_task_completion("did:nexus:a1", false);
        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert_eq!(rep.total_tasks_completed, 0);
        assert_eq!(rep.total_tasks_failed, 1);
        assert!(rep.success_rate.abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_calculation() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        // 3 success, 1 failure → 75%
        reg.record_task_completion("did:nexus:a1", true);
        reg.record_task_completion("did:nexus:a1", true);
        reg.record_task_completion("did:nexus:a1", true);
        reg.record_task_completion("did:nexus:a1", false);
        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert!((rep.success_rate - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_governance_violation_lowers_score() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        // Record some tasks first
        for _ in 0..10 {
            reg.record_task_completion("did:nexus:a1", true);
        }
        let score_before = reg.get_reputation("did:nexus:a1").unwrap().reputation_score;

        // Now add violations
        reg.record_governance_violation("did:nexus:a1");
        reg.record_governance_violation("did:nexus:a1");
        let score_after = reg.get_reputation("did:nexus:a1").unwrap().reputation_score;

        assert!(
            score_after < score_before,
            "score should decrease: {score_before} → {score_after}"
        );
    }

    #[test]
    fn test_peer_rating() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.add_peer_rating("did:nexus:a1", "did:nexus:rater1", 4.0, Some("good".into()));
        reg.add_peer_rating("did:nexus:a1", "did:nexus:rater2", 2.0, None);

        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert_eq!(rep.peer_ratings.len(), 2);
        assert!((rep.average_response_quality - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_reputation_score() {
        let mut reg = make_registry_with_agent("did:nexus:a1");

        // 8 success, 2 failure → success_rate = 0.8
        for _ in 0..8 {
            reg.record_task_completion("did:nexus:a1", true);
        }
        for _ in 0..2 {
            reg.record_task_completion("did:nexus:a1", false);
        }

        // Add peer ratings averaging 4.0/5.0
        reg.add_peer_rating("did:nexus:a1", "r1", 4.0, None);
        reg.add_peer_rating("did:nexus:a1", "r2", 4.0, None);

        // 0 violations, 10 total tasks → violation_rate = 0.0

        let score = reg.compute_reputation_score("did:nexus:a1");

        // Expected: 0.40*0.8 + 0.25*(4.0/5.0) + 0.20*(1.0) + 0.15*(4.0/5.0)
        //         = 0.32 + 0.20 + 0.20 + 0.12 = 0.84
        assert!((score - 0.84).abs() < 0.01, "expected ~0.84, got {score}");
    }

    #[test]
    fn test_top_agents_sorted() {
        let mut reg = ReputationRegistry::new();
        reg.register_agent("did:nexus:low", "Low");
        reg.register_agent("did:nexus:high", "High");
        reg.register_agent("did:nexus:mid", "Mid");

        // High: 10 successes
        for _ in 0..10 {
            reg.record_task_completion("did:nexus:high", true);
        }
        // Mid: 5 successes, 5 failures
        for _ in 0..5 {
            reg.record_task_completion("did:nexus:mid", true);
        }
        for _ in 0..5 {
            reg.record_task_completion("did:nexus:mid", false);
        }
        // Low: 10 failures
        for _ in 0..10 {
            reg.record_task_completion("did:nexus:low", false);
        }

        let top = reg.top_agents(3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].agent_did, "did:nexus:high");
        assert_eq!(top[2].agent_did, "did:nexus:low");
    }

    #[test]
    fn test_badge_award_trusted() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        // Build up reputation > 0.8
        for _ in 0..20 {
            reg.record_task_completion("did:nexus:a1", true);
        }
        reg.add_peer_rating("did:nexus:a1", "r1", 5.0, None);

        reg.award_badges("did:nexus:a1");
        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert!(
            rep.badges.contains(&ReputationBadge::Trusted),
            "expected Trusted badge, got {:?}",
            rep.badges
        );
    }

    #[test]
    fn test_badge_award_governance_clean() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.record_task_completion("did:nexus:a1", true);

        reg.award_badges("did:nexus:a1");
        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert!(
            rep.badges.contains(&ReputationBadge::GovernanceClean),
            "expected GovernanceClean badge, got {:?}",
            rep.badges
        );
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.record_task_completion("did:nexus:a1", true);
        reg.record_task_completion("did:nexus:a1", true);
        reg.add_peer_rating("did:nexus:a1", "r1", 4.5, Some("great".into()));

        let json = reg.export_reputation("did:nexus:a1").unwrap();

        // Import into a fresh registry
        let mut reg2 = ReputationRegistry::new();
        let imported = reg2.import_reputation(&json).unwrap();
        assert_eq!(imported.agent_did, "did:nexus:a1");
        assert_eq!(imported.total_tasks_completed, 2);
        assert_eq!(imported.peer_ratings.len(), 1);
        assert!(reg2.get_reputation("did:nexus:a1").is_some());
    }

    #[test]
    fn test_search_by_badge() {
        let mut reg = ReputationRegistry::new();
        reg.register_agent("did:nexus:a1", "A1");
        reg.register_agent("did:nexus:a2", "A2");

        reg.record_task_completion("did:nexus:a1", true);
        reg.award_badges("did:nexus:a1");

        let clean = reg.search_by_badge("GovernanceClean");
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0].agent_did, "did:nexus:a1");

        let trusted = reg.search_by_badge("Trusted");
        assert!(trusted.is_empty());
    }

    #[test]
    fn test_reputation_score_clamped() {
        let mut reg = make_registry_with_agent("did:nexus:a1");

        // Score with no data should still be in [0, 1]
        let score = reg.compute_reputation_score("did:nexus:a1");
        assert!((0.0..=1.0).contains(&score), "score {score} out of range");

        // Max everything out
        for _ in 0..100 {
            reg.record_task_completion("did:nexus:a1", true);
        }
        reg.add_peer_rating("did:nexus:a1", "r1", 5.0, None);
        let score = reg.compute_reputation_score("did:nexus:a1");
        assert!((0.0..=1.0).contains(&score), "score {score} out of range");

        // Min everything out
        let mut reg2 = make_registry_with_agent("did:nexus:bad");
        for _ in 0..100 {
            reg2.record_task_completion("did:nexus:bad", false);
        }
        for _ in 0..100 {
            reg2.record_governance_violation("did:nexus:bad");
        }
        reg2.add_peer_rating("did:nexus:bad", "r1", 0.0, None);
        let score = reg2.compute_reputation_score("did:nexus:bad");
        assert!((0.0..=1.0).contains(&score), "score {score} out of range");
    }

    #[test]
    fn test_peer_rating_clamped() {
        let mut reg = make_registry_with_agent("did:nexus:a1");
        reg.add_peer_rating("did:nexus:a1", "r1", 10.0, None); // exceeds max
        reg.add_peer_rating("did:nexus:a1", "r2", -5.0, None); // below min

        let rep = reg.get_reputation("did:nexus:a1").unwrap();
        assert!((rep.peer_ratings[0].score - 5.0).abs() < f64::EPSILON);
        assert!(rep.peer_ratings[1].score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_nonexistent_agent_operations() {
        let mut reg = ReputationRegistry::new();
        // These should be no-ops, not panics
        reg.record_task_completion("did:nexus:ghost", true);
        reg.record_governance_violation("did:nexus:ghost");
        reg.add_peer_rating("did:nexus:ghost", "r1", 3.0, None);
        reg.award_badges("did:nexus:ghost");
        assert_eq!(reg.compute_reputation_score("did:nexus:ghost"), 0.0);
        assert!(reg.get_reputation("did:nexus:ghost").is_none());
        assert!(reg.export_reputation("did:nexus:ghost").is_err());
    }
}
