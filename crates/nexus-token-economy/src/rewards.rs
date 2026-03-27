use serde::{Deserialize, Serialize};

use crate::coin::NexusCoin;

/// Reward calculation: quality × difficulty × speed multiplier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardEngine {
    /// Base reward per task in micronexus
    pub base_reward: NexusCoin,
    /// Maximum speed multiplier (tasks completed faster earn more)
    pub max_speed_multiplier: f64,
    /// Target completion time in seconds (at this time, speed multiplier = 1.0)
    pub target_completion_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardCalculation {
    pub base: NexusCoin,
    pub quality_multiplier: f64,
    pub difficulty_multiplier: f64,
    pub speed_multiplier: f64,
    pub final_reward: NexusCoin,
}

impl RewardEngine {
    pub fn default_config() -> Self {
        Self {
            base_reward: NexusCoin::from_coins(1), // 1 NXC base
            max_speed_multiplier: 2.0,
            target_completion_secs: 30,
        }
    }

    /// Calculate reward for a completed task
    /// quality_score: 0.0-1.0 (from capability measurement)
    /// difficulty: 0.0-1.0 (from difficulty estimator, maps to Level 1-5)
    /// completion_secs: how long the task took
    pub fn calculate_reward(
        &self,
        quality_score: f64,
        difficulty: f64,
        completion_secs: u64,
    ) -> RewardCalculation {
        // Quality multiplier: 0.0-1.0 maps to 0.0-1.0 (linear)
        let quality_mult = quality_score.clamp(0.0, 1.0);

        // Difficulty multiplier: scales reward by task difficulty
        // Level 1 (0.2) = 0.5x + 0.2*2.5 = 1.0x, Level 5 (1.0) = 3.0x
        let difficulty_mult = 0.5 + (difficulty.clamp(0.0, 1.0) * 2.5);

        // Speed multiplier: faster completion = higher reward, capped
        let speed_mult = if completion_secs == 0 {
            self.max_speed_multiplier
        } else {
            let ratio = self.target_completion_secs as f64 / completion_secs as f64;
            ratio.clamp(0.5, self.max_speed_multiplier)
        };

        let reward_micro =
            (self.base_reward.micro() as f64 * quality_mult * difficulty_mult * speed_mult) as u64;

        RewardCalculation {
            base: self.base_reward,
            quality_multiplier: quality_mult,
            difficulty_multiplier: difficulty_mult,
            speed_multiplier: speed_mult,
            final_reward: NexusCoin::from_micro(reward_micro),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_quality_high_difficulty_high_reward() {
        let engine = RewardEngine::default_config();
        let calc = engine.calculate_reward(1.0, 1.0, 30);
        // 1.0 quality * 3.0 difficulty * 1.0 speed * 1_000_000 base = 3_000_000 micro
        assert_eq!(calc.final_reward, NexusCoin::from_micro(3_000_000));
    }

    #[test]
    fn test_low_quality_low_reward() {
        let engine = RewardEngine::default_config();
        let calc = engine.calculate_reward(0.0, 1.0, 30);
        assert_eq!(calc.final_reward, NexusCoin::ZERO);
    }

    #[test]
    fn test_speed_multiplier_caps() {
        let engine = RewardEngine::default_config();
        // Very fast: 1 second for a 30s target
        let fast = engine.calculate_reward(1.0, 0.0, 1);
        // speed = min(30/1, 2.0) = 2.0, difficulty = 0.5
        assert_eq!(fast.speed_multiplier, 2.0);

        // Very slow: 600 seconds for a 30s target
        let slow = engine.calculate_reward(1.0, 0.0, 600);
        // speed = max(30/600, 0.5) = 0.5
        assert_eq!(slow.speed_multiplier, 0.5);
    }
}
