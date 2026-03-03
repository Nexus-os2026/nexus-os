use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BASE_RATING: f64 = 1_200.0;
const K_FACTOR: f64 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillArea {
    Coding,
    SocialMedia,
    WebDesign,
    Research,
    Writing,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskDifficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillLevel {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub agent_id: String,
    pub rating: f64,
    pub level: SkillLevel,
}

#[derive(Debug, Clone, Default)]
pub struct SkillRatingSystem {
    ratings: HashMap<String, HashMap<SkillArea, f64>>,
}

impl SkillRatingSystem {
    pub fn new() -> Self {
        Self {
            ratings: HashMap::new(),
        }
    }

    pub fn rating_for(&self, agent_id: &str, skill: SkillArea) -> f64 {
        self.ratings
            .get(agent_id)
            .and_then(|skills| skills.get(&skill))
            .copied()
            .unwrap_or(BASE_RATING)
    }

    pub fn level_for(&self, agent_id: &str, skill: SkillArea) -> SkillLevel {
        level_for_rating(self.rating_for(agent_id, skill))
    }

    pub fn update_rating(
        &mut self,
        agent_id: &str,
        skill: SkillArea,
        success: bool,
        difficulty: TaskDifficulty,
    ) -> f64 {
        let current = self.rating_for(agent_id, skill);
        let opponent = difficulty_rating(difficulty);
        let expected = 1.0 / (1.0 + 10_f64.powf((opponent - current) / 400.0));
        let outcome = if success { 1.0 } else { 0.0 };
        let next = (current + K_FACTOR * (outcome - expected)).max(1.0);

        self.ratings
            .entry(agent_id.to_string())
            .or_default()
            .insert(skill, next);
        next
    }

    pub fn recommended_difficulty(&self, agent_id: &str, skill: SkillArea) -> TaskDifficulty {
        let rating = self.rating_for(agent_id, skill);
        if rating < 1_100.0 {
            TaskDifficulty::Easy
        } else if rating < 1_450.0 {
            TaskDifficulty::Medium
        } else if rating < 1_800.0 {
            TaskDifficulty::Hard
        } else {
            TaskDifficulty::Expert
        }
    }

    pub fn leaderboard(&self, skill: SkillArea) -> Vec<LeaderboardEntry> {
        let mut entries = self
            .ratings
            .iter()
            .map(|(agent_id, skills)| {
                let rating = skills.get(&skill).copied().unwrap_or(BASE_RATING);
                LeaderboardEntry {
                    agent_id: agent_id.clone(),
                    rating,
                    level: level_for_rating(rating),
                }
            })
            .collect::<Vec<_>>();

        entries.sort_by(|left, right| {
            right
                .rating
                .partial_cmp(&left.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }
}

fn difficulty_rating(difficulty: TaskDifficulty) -> f64 {
    match difficulty {
        TaskDifficulty::Easy => 1_000.0,
        TaskDifficulty::Medium => 1_200.0,
        TaskDifficulty::Hard => 1_500.0,
        TaskDifficulty::Expert => 1_800.0,
    }
}

fn level_for_rating(rating: f64) -> SkillLevel {
    if rating < 1_100.0 {
        SkillLevel::Beginner
    } else if rating < 1_450.0 {
        SkillLevel::Intermediate
    } else if rating < 1_800.0 {
        SkillLevel::Advanced
    } else {
        SkillLevel::Expert
    }
}
