//! Adversarial arena — red-team sessions pitting attacker vs defender agents.
//!
//! The arena runs controlled rounds where an attacker agent attempts various
//! exploits while a defender agent tries to block them. Both agents evolve
//! via genome mutation between rounds, creating an evolutionary arms race.

use rand::Rng;
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

use super::detector::ThreatType;

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

/// Most rounds one arena session may run.
///
/// The count reaches the kernel from the interface (`run_adversarial_session`,
/// a `u32`). Every round is computed and kept and the whole session is
/// returned, so a count outside `1..=MAX_ARENA_ROUNDS` is refused, never
/// clamped, before anything is allocated.
pub const MAX_ARENA_ROUNDS: u32 = 50;

/// Why an arena request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaError {
    /// The round count is outside `1..=MAX_ARENA_ROUNDS`.
    InvalidRounds(u32),
}

impl fmt::Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRounds(rounds) => write!(
                f,
                "arena rounds must be between 1 and {MAX_ARENA_ROUNDS}, got {rounds}"
            ),
        }
    }
}

impl std::error::Error for ArenaError {}

/// Accept a round count within `1..=MAX_ARENA_ROUNDS`.
pub fn check_rounds(rounds: u32) -> Result<u32, ArenaError> {
    if (1..=MAX_ARENA_ROUNDS).contains(&rounds) {
        Ok(rounds)
    } else {
        Err(ArenaError::InvalidRounds(rounds))
    }
}

// ---------------------------------------------------------------------------
// ArenaRun
// ---------------------------------------------------------------------------

/// The outcome of [`AdversarialArena::run_session`]: a completed session, or
/// the refusal of a request outside the arena's bounds.
///
/// It serializes exactly as the session does. A refused request has no
/// session to report, so serializing it fails with the refusal's reason: a
/// caller that returns the serialized session returns the refusal as its
/// error. New callers should use [`AdversarialArena::try_run_session`].
#[derive(Debug, Clone)]
#[must_use = "a refused arena request must be reported"]
pub struct ArenaRun(Result<ArenaSession, ArenaError>);

impl ArenaRun {
    /// The session, or the refusal.
    pub fn into_result(self) -> Result<ArenaSession, ArenaError> {
        self.0
    }

    /// The completed session, if the request was accepted.
    pub fn session(&self) -> Option<&ArenaSession> {
        self.0.as_ref().ok()
    }

    /// The refusal, if the request was refused.
    pub fn refusal(&self) -> Option<&ArenaError> {
        self.0.as_ref().err()
    }

    /// The session's defender win rate; NaN for a refused request, which ran
    /// no rounds.
    pub fn defender_win_rate(&self) -> f64 {
        self.session()
            .map_or(f64::NAN, ArenaSession::defender_win_rate)
    }
}

impl Serialize for ArenaRun {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Ok(session) => session.serialize(serializer),
            Err(refusal) => Err(serde::ser::Error::custom(refusal)),
        }
    }
}

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
    ///
    /// A round count outside `1..=MAX_ARENA_ROUNDS` is refused; the result
    /// then serializes as an error (see [`ArenaRun`]).
    pub fn run_session(&mut self, attacker_id: &str, defender_id: &str, rounds: u32) -> ArenaRun {
        ArenaRun(self.try_run_session(attacker_id, defender_id, rounds))
    }

    /// Run a full arena session between an attacker and defender, or refuse
    /// a round count outside `1..=MAX_ARENA_ROUNDS` before anything is
    /// allocated, computed or recorded.
    pub fn try_run_session(
        &mut self,
        attacker_id: &str,
        defender_id: &str,
        rounds: u32,
    ) -> Result<ArenaSession, ArenaError> {
        let rounds = check_rounds(rounds)?;
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
        Ok(session)
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
        let session = arena
            .try_run_session("attacker-1", "defender-1", 20)
            .unwrap();

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
        let session = arena.try_run_session("a", "d", 5).unwrap();
        for (i, result) in session.results.iter().enumerate() {
            assert_eq!(result.round, (i + 1) as u32);
            // Scores are 0 or 1 and complement each other
            assert!((result.attacker_score + result.defender_score - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_session_stored() {
        let mut arena = AdversarialArena::new();
        assert!(arena.run_session("a", "d", 3).session().is_some());
        assert!(arena.run_session("a2", "d2", 5).session().is_some());
        assert_eq!(arena.session_count(), 2);
    }

    #[test]
    fn p0_fg_k_round_counts_outside_the_bound_are_refused_before_anything_runs() {
        assert_eq!(MAX_ARENA_ROUNDS, 50);
        let mut arena = AdversarialArena::new();
        // u32::MAX would ask for about 137 GB; it is checked here without an
        // arena, so a regression cannot allocate it.
        assert_eq!(
            check_rounds(u32::MAX),
            Err(ArenaError::InvalidRounds(u32::MAX))
        );
        for rounds in [0, MAX_ARENA_ROUNDS + 1, 10_000] {
            assert_eq!(
                arena.try_run_session("a", "d", rounds).unwrap_err(),
                ArenaError::InvalidRounds(rounds)
            );
            assert_eq!(
                arena.run_session("a", "d", rounds).refusal(),
                Some(&ArenaError::InvalidRounds(rounds))
            );
        }
        assert_eq!(
            arena.session_count(),
            0,
            "a refused request is not recorded"
        );
        assert_eq!(
            ArenaError::InvalidRounds(51).to_string(),
            "arena rounds must be between 1 and 50, got 51"
        );
    }

    #[test]
    fn p0_fg_k_round_count_bounds_are_inclusive() {
        let mut arena = AdversarialArena::new();
        for rounds in [1, MAX_ARENA_ROUNDS] {
            let session = arena.try_run_session("a", "d", rounds).unwrap();
            assert_eq!(session.rounds, rounds);
            assert_eq!(session.results.len(), rounds as usize);
        }
        assert_eq!(arena.session_count(), 2);
    }

    #[test]
    fn p0_fg_k_a_run_serializes_as_its_session_and_a_refusal_as_an_error() {
        let mut arena = AdversarialArena::new();
        let run = arena.run_session("a", "d", 3);
        assert_eq!(
            serde_json::to_value(&run).unwrap(),
            serde_json::to_value(run.session().unwrap()).unwrap(),
            "an accepted run serializes exactly as its session"
        );
        assert!(run.defender_win_rate().is_finite());

        let refused = arena.run_session("a", "d", 51);
        let error = serde_json::to_value(&refused).unwrap_err().to_string();
        assert!(
            error.contains("arena rounds must be between 1 and 50, got 51"),
            "{error}"
        );
        assert!(refused.defender_win_rate().is_nan());
        assert!(refused.into_result().is_err());
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
