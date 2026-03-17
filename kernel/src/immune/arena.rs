//! Adversarial arena — red-team sessions pitting attacker vs defender agents.
//!
//! The arena runs controlled rounds where an attacker agent attempts various
//! exploits while a defender agent tries to block them. Both agents evolve
//! via genome mutation between rounds, creating an evolutionary arms race.

use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::detector::ThreatType;

// ---------------------------------------------------------------------------
// RoundResult
// ---------------------------------------------------------------------------

/// Outcome of a single attack/defense round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResult {
    pub round: u32,
    pub attacker_score: f64,
    pub defender_score: f64,
    pub attack_type: ThreatType,
    pub defense_successful: bool,
}

// ---------------------------------------------------------------------------
// ArenaSession
// ---------------------------------------------------------------------------

/// A complete adversarial testing session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaSession {
    pub id: Uuid,
    pub attacker_id: String,
    pub defender_id: String,
    pub rounds: u32,
    pub results: Vec<RoundResult>,
}

impl ArenaSession {
    /// Overall attacker win rate (fraction of rounds where defense failed).
    pub fn attacker_win_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let failed = self
            .results
            .iter()
            .filter(|r| !r.defense_successful)
            .count();
        failed as f64 / self.results.len() as f64
    }

    /// Overall defender win rate.
    pub fn defender_win_rate(&self) -> f64 {
        1.0 - self.attacker_win_rate()
    }

    /// Average defender score across all rounds.
    pub fn avg_defender_score(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.results.iter().map(|r| r.defender_score).sum();
        sum / self.results.len() as f64
    }
}

// ---------------------------------------------------------------------------
// AdversarialArena
// ---------------------------------------------------------------------------

/// Manages adversarial red-team sessions between agent pairs.
///
/// Each round simulates an attack attempt of a random [`ThreatType`]. The
/// defender's success probability is based on its base defense score, mutated
/// each round to simulate genome evolution. The attacker's score is the
/// inverse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdversarialArena {
    /// Base defense probability for new sessions (0.0 .. 1.0).
    pub base_defense_rate: f64,
    /// Mutation step applied each round to evolve attacker/defender.
    pub mutation_step: f64,
    /// History of completed sessions.
    pub sessions: Vec<ArenaSession>,
}

impl AdversarialArena {
    pub fn new() -> Self {
        Self {
            base_defense_rate: 0.6,
            mutation_step: 0.05,
            sessions: Vec::new(),
        }
    }

    /// Run a full arena session between an attacker and defender.
    pub fn run_session(
        &mut self,
        attacker_id: &str,
        defender_id: &str,
        rounds: u32,
    ) -> ArenaSession {
        let mut rng = rand::thread_rng();
        let mut results = Vec::with_capacity(rounds as usize);
        let mut defense_rate = self.base_defense_rate;

        let attack_types = [
            ThreatType::PromptInjection,
            ThreatType::DataExfiltration,
            ThreatType::ResourceAbuse,
            ThreatType::UnauthorizedTool,
            ThreatType::AnomalousBehavior,
        ];

        for round_num in 1..=rounds {
            let attack_type = attack_types[rng.gen_range(0..attack_types.len())];

            // Defender succeeds with probability `defense_rate`
            let roll: f64 = rng.gen();
            let defense_successful = roll < defense_rate;

            let defender_score = if defense_successful { 1.0 } else { 0.0 };
            let attacker_score = 1.0 - defender_score;

            results.push(RoundResult {
                round: round_num,
                attacker_score,
                defender_score,
                attack_type,
                defense_successful,
            });

            // Evolve: defender improves when successful, attacker improves when
            // defense fails — bounded to [0.1, 0.95].
            if defense_successful {
                defense_rate = (defense_rate + self.mutation_step).min(0.95);
            } else {
                defense_rate = (defense_rate - self.mutation_step).max(0.1);
            }
        }

        let session = ArenaSession {
            id: Uuid::new_v4(),
            attacker_id: attacker_id.to_string(),
            defender_id: defender_id.to_string(),
            rounds,
            results,
        };

        self.sessions.push(session.clone());
        session
    }

    /// Total sessions run so far.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for AdversarialArena {
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

    #[test]
    fn test_run_session() {
        let mut arena = AdversarialArena::new();
        let session = arena.run_session("attacker-1", "defender-1", 20);

        assert_eq!(session.rounds, 20);
        assert_eq!(session.results.len(), 20);
        assert_eq!(session.attacker_id, "attacker-1");
        assert_eq!(session.defender_id, "defender-1");

        // Win rates must sum to 1.0
        let total = session.attacker_win_rate() + session.defender_win_rate();
        assert!((total - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_round_results_sequential() {
        let mut arena = AdversarialArena::new();
        let session = arena.run_session("a", "d", 5);
        for (i, result) in session.results.iter().enumerate() {
            assert_eq!(result.round, (i + 1) as u32);
            // Scores are 0 or 1 and complement each other
            assert!((result.attacker_score + result.defender_score - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_session_stored() {
        let mut arena = AdversarialArena::new();
        arena.run_session("a", "d", 3);
        arena.run_session("a2", "d2", 5);
        assert_eq!(arena.session_count(), 2);
    }

    #[test]
    fn test_empty_session_rates() {
        let session = ArenaSession {
            id: Uuid::new_v4(),
            attacker_id: "a".into(),
            defender_id: "d".into(),
            rounds: 0,
            results: vec![],
        };
        assert_eq!(session.attacker_win_rate(), 0.0);
        assert_eq!(session.defender_win_rate(), 1.0);
        assert_eq!(session.avg_defender_score(), 0.0);
    }

    #[test]
    fn test_avg_defender_score() {
        let session = ArenaSession {
            id: Uuid::new_v4(),
            attacker_id: "a".into(),
            defender_id: "d".into(),
            rounds: 2,
            results: vec![
                RoundResult {
                    round: 1,
                    attacker_score: 0.0,
                    defender_score: 1.0,
                    attack_type: ThreatType::PromptInjection,
                    defense_successful: true,
                },
                RoundResult {
                    round: 2,
                    attacker_score: 1.0,
                    defender_score: 0.0,
                    attack_type: ThreatType::DataExfiltration,
                    defense_successful: false,
                },
            ],
        };
        assert!((session.avg_defender_score() - 0.5).abs() < f64::EPSILON);
    }
}
