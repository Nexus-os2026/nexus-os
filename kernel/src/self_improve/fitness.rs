//! Layer 8: System-Wide Fitness Score — the entire OS gets a fitness score
//! that should trend upward over time.

use super::performance::Trend;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────────

/// Complete OS fitness snapshot at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSFitness {
    pub timestamp: u64,
    pub overall_score: f64,
    pub agent_quality: f64,
    pub routing_accuracy: f64,
    pub response_latency: f64,
    pub security_accuracy: f64,
    pub user_satisfaction: f64,
    pub knowledge_depth: f64,
    pub uptime_stability: f64,
    pub evolution_success_rate: f64,
}

impl OSFitness {
    /// Create a new fitness snapshot with component scores.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_quality: f64,
        routing_accuracy: f64,
        response_latency: f64,
        security_accuracy: f64,
        user_satisfaction: f64,
        knowledge_depth: f64,
        uptime_stability: f64,
        evolution_success_rate: f64,
    ) -> Self {
        let mut fitness = Self {
            timestamp: epoch_secs(),
            overall_score: 0.0,
            agent_quality: agent_quality.clamp(0.0, 100.0),
            routing_accuracy: routing_accuracy.clamp(0.0, 100.0),
            response_latency: response_latency.clamp(0.0, 100.0),
            security_accuracy: security_accuracy.clamp(0.0, 100.0),
            user_satisfaction: user_satisfaction.clamp(0.0, 100.0),
            knowledge_depth: knowledge_depth.clamp(0.0, 100.0),
            uptime_stability: uptime_stability.clamp(0.0, 100.0),
            evolution_success_rate: evolution_success_rate.clamp(0.0, 100.0),
        };
        fitness.overall_score = fitness.calculate();
        fitness
    }

    /// Weighted overall score.
    pub fn calculate(&self) -> f64 {
        self.agent_quality * 0.25
            + self.routing_accuracy * 0.10
            + (100.0 - self.response_latency.min(100.0)) * 0.10
            + self.security_accuracy * 0.15
            + self.user_satisfaction * 0.20
            + self.knowledge_depth * 0.10
            + self.uptime_stability * 0.05
            + self.evolution_success_rate * 0.05
    }

    /// Create a baseline fitness for a fresh system.
    pub fn baseline() -> Self {
        Self::new(50.0, 50.0, 50.0, 100.0, 50.0, 0.0, 100.0, 0.0)
    }
}

// ── FitnessHistory ──────────────────────────────────────────────────────────

/// Tracks OS fitness over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessHistory {
    daily_scores: Vec<OSFitness>,
    trend: Trend,
    days_tracked: u32,
    max_entries: usize,
}

impl FitnessHistory {
    pub fn new() -> Self {
        Self {
            daily_scores: Vec::new(),
            trend: Trend::Stable,
            days_tracked: 0,
            max_entries: 365,
        }
    }

    /// Record a daily fitness snapshot.
    pub fn record(&mut self, fitness: OSFitness) {
        self.daily_scores.push(fitness);
        self.days_tracked += 1;

        if self.daily_scores.len() > self.max_entries {
            self.daily_scores.remove(0);
        }

        self.update_trend();
    }

    /// Update the overall trend based on recent vs. baseline scores.
    fn update_trend(&mut self) {
        if self.daily_scores.len() < 3 {
            self.trend = Trend::Stable;
            return;
        }

        let window = 3.min(self.daily_scores.len() / 2);
        let recent_avg: f64 = self.daily_scores[self.daily_scores.len() - window..]
            .iter()
            .map(|f| f.overall_score)
            .sum::<f64>()
            / window as f64;
        let baseline_avg: f64 = self.daily_scores[..window]
            .iter()
            .map(|f| f.overall_score)
            .sum::<f64>()
            / window as f64;

        if baseline_avg == 0.0 {
            self.trend = Trend::Stable;
            return;
        }

        let diff = recent_avg - baseline_avg;
        if diff > 2.0 {
            self.trend = Trend::Improving;
        } else if diff < -2.0 {
            self.trend = Trend::Degrading;
        } else {
            self.trend = Trend::Stable;
        }
    }

    /// Get the current trend.
    pub fn trend(&self) -> &Trend {
        &self.trend
    }

    /// Get all recorded fitness scores.
    pub fn scores(&self) -> &[OSFitness] {
        &self.daily_scores
    }

    /// Get the latest fitness score.
    pub fn latest(&self) -> Option<&OSFitness> {
        self.daily_scores.last()
    }

    /// Days tracked.
    pub fn days_tracked(&self) -> u32 {
        self.days_tracked
    }

    /// Week-over-week change in overall score.
    pub fn weekly_change(&self) -> f64 {
        if self.daily_scores.len() < 7 {
            return 0.0;
        }
        let current = self
            .daily_scores
            .last()
            .map(|f| f.overall_score)
            .unwrap_or(0.0);
        let week_ago = self.daily_scores[self.daily_scores.len() - 7].overall_score;
        current - week_ago
    }

    /// Get scores for a specific number of days.
    pub fn last_n_days(&self, n: usize) -> Vec<&OSFitness> {
        self.daily_scores
            .iter()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

impl Default for FitnessHistory {
    fn default() -> Self {
        Self::new()
    }
}

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_fitness_reasonable() {
        let baseline = OSFitness::baseline();
        assert!(baseline.overall_score > 30.0);
        assert!(baseline.overall_score < 80.0);
    }

    #[test]
    fn fitness_calculation_weights_correct() {
        // All 100s should give 100
        let perfect = OSFitness::new(100.0, 100.0, 0.0, 100.0, 100.0, 100.0, 100.0, 100.0);
        assert!((perfect.calculate() - 100.0).abs() < 1e-9);

        // All 0s (except latency which inverts) should give ~10 (from latency component)
        let worst = OSFitness::new(0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((worst.calculate() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn fitness_scores_clamped() {
        let fitness = OSFitness::new(150.0, -10.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0);
        assert!((fitness.agent_quality - 100.0).abs() < 1e-9);
        assert!((fitness.routing_accuracy - 0.0).abs() < 1e-9);
    }

    #[test]
    fn history_tracks_trend_improving() {
        let mut history = FitnessHistory::new();
        // Record improving scores
        for i in 0..10 {
            history.record(OSFitness::new(
                50.0 + i as f64 * 5.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
            ));
        }
        assert_eq!(*history.trend(), Trend::Improving);
    }

    #[test]
    fn history_tracks_trend_degrading() {
        let mut history = FitnessHistory::new();
        for i in 0..10 {
            history.record(OSFitness::new(
                90.0 - i as f64 * 5.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
            ));
        }
        assert_eq!(*history.trend(), Trend::Degrading);
    }

    #[test]
    fn history_bounded() {
        let mut history = FitnessHistory::new();
        history.max_entries = 5;
        for _ in 0..10 {
            history.record(OSFitness::baseline());
        }
        assert_eq!(history.scores().len(), 5);
        assert_eq!(history.days_tracked(), 10);
    }

    #[test]
    fn weekly_change_calculated() {
        let mut history = FitnessHistory::new();
        for i in 0..10 {
            history.record(OSFitness::new(
                50.0 + i as f64 * 2.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
                50.0,
            ));
        }
        // Should be positive (improving)
        assert!(history.weekly_change() > 0.0);
    }

    #[test]
    fn latest_returns_most_recent() {
        let mut history = FitnessHistory::new();
        history.record(OSFitness::new(
            30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0,
        ));
        history.record(OSFitness::new(
            80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0,
        ));
        assert!((history.latest().unwrap().agent_quality - 80.0).abs() < 1e-9);
    }
}
