//! Layer 6: Knowledge Accumulation — builds a growing understanding of the user
//! and their projects across all interactions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────────

/// User profile built from observed interaction patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub expertise_areas: HashMap<String, f64>,
    pub coding_languages: HashMap<String, f64>,
    pub communication_style: String,
    pub recurring_topics: Vec<String>,
    pub preferences: HashMap<String, String>,
    pub interaction_count: u64,
    pub last_updated: u64,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            expertise_areas: HashMap::new(),
            coding_languages: HashMap::new(),
            communication_style: "unknown".to_string(),
            recurring_topics: Vec::new(),
            preferences: HashMap::new(),
            interaction_count: 0,
            last_updated: 0,
        }
    }
}

/// Context about a project the user is working on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub name: String,
    pub description: String,
    pub technologies: Vec<String>,
    pub current_focus: String,
    pub interaction_count: u64,
    pub last_updated: u64,
}

/// A detected pattern across interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralPattern {
    pub pattern_type: String,
    pub description: String,
    pub confidence: f64,
    pub occurrences: u64,
    pub first_seen: u64,
    pub last_seen: u64,
}

// ── KnowledgeAccumulator ────────────────────────────────────────────────────

/// Accumulates knowledge about the user across all interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeAccumulator {
    user_profile: UserProfile,
    project_memory: HashMap<String, ProjectContext>,
    behavioral_patterns: Vec<BehavioralPattern>,
    interaction_log: Vec<InteractionSummary>,
    max_interactions: usize,
    max_patterns: usize,
}

/// Compressed summary of a single interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionSummary {
    pub topic: String,
    pub languages_mentioned: Vec<String>,
    pub score: f64,
    pub timestamp: u64,
}

impl KnowledgeAccumulator {
    pub fn new() -> Self {
        Self {
            user_profile: UserProfile::default(),
            project_memory: HashMap::new(),
            behavioral_patterns: Vec::new(),
            interaction_log: Vec::new(),
            max_interactions: 500,
            max_patterns: 100,
        }
    }

    /// Record an interaction and update the user profile.
    pub fn record_interaction(&mut self, summary: InteractionSummary) {
        let now = epoch_secs();

        // Update expertise areas based on topic
        {
            let entry = self
                .user_profile
                .expertise_areas
                .entry(summary.topic.clone())
                .or_insert(0.0);
            // Weighted moving average — more interactions = higher confidence
            *entry = (*entry * 0.9) + (summary.score * 0.1);
        }

        // Update coding languages
        for lang in &summary.languages_mentioned {
            let entry = self
                .user_profile
                .coding_languages
                .entry(lang.clone())
                .or_insert(0.0);
            *entry = (*entry + 1.0).min(10.0);
        }

        // Track recurring topics
        if !self.user_profile.recurring_topics.contains(&summary.topic) {
            let topic_count = self
                .interaction_log
                .iter()
                .filter(|i| i.topic == summary.topic)
                .count();
            if topic_count >= 3 {
                self.user_profile
                    .recurring_topics
                    .push(summary.topic.clone());
                if self.user_profile.recurring_topics.len() > 20 {
                    self.user_profile.recurring_topics.remove(0);
                }
            }
        }

        self.user_profile.interaction_count += 1;
        self.user_profile.last_updated = now;

        self.interaction_log.push(summary);
        if self.interaction_log.len() > self.max_interactions {
            self.interaction_log.remove(0);
        }
    }

    /// Record a behavioral pattern.
    pub fn record_pattern(&mut self, pattern: BehavioralPattern) {
        // Update existing or add new
        if let Some(existing) = self.behavioral_patterns.iter_mut().find(|p| {
            p.pattern_type == pattern.pattern_type && p.description == pattern.description
        }) {
            existing.occurrences += 1;
            existing.last_seen = pattern.last_seen;
            existing.confidence = (existing.confidence * 0.8) + (pattern.confidence * 0.2);
        } else {
            self.behavioral_patterns.push(pattern);
            if self.behavioral_patterns.len() > self.max_patterns {
                // Remove least confident
                self.behavioral_patterns.sort_by(|a, b| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.behavioral_patterns.truncate(self.max_patterns);
            }
        }
    }

    /// Update project context.
    pub fn update_project(&mut self, name: &str, context: ProjectContext) {
        self.project_memory.insert(name.to_string(), context);
    }

    /// Set a user preference.
    pub fn set_preference(&mut self, key: &str, value: &str) {
        self.user_profile
            .preferences
            .insert(key.to_string(), value.to_string());
    }

    /// Set the user's communication style.
    pub fn set_communication_style(&mut self, style: &str) {
        self.user_profile.communication_style = style.to_string();
    }

    /// Build a context string to inject into agent system prompts.
    pub fn build_user_context(&self) -> String {
        let mut parts = Vec::new();

        // Top expertise areas
        let mut expertise: Vec<(&String, &f64)> =
            self.user_profile.expertise_areas.iter().collect();
        expertise.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        if !expertise.is_empty() {
            let top: Vec<String> = expertise
                .iter()
                .take(5)
                .map(|(k, v)| format!("{k} ({v:.1})"))
                .collect();
            parts.push(format!("User expertise: {}", top.join(", ")));
        }

        // Coding languages
        if !self.user_profile.coding_languages.is_empty() {
            let mut langs: Vec<(&String, &f64)> =
                self.user_profile.coding_languages.iter().collect();
            langs.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top: Vec<&str> = langs.iter().take(5).map(|(k, _)| k.as_str()).collect();
            parts.push(format!("Languages: {}", top.join(", ")));
        }

        // Communication style
        if self.user_profile.communication_style != "unknown" {
            parts.push(format!(
                "Prefers {} responses",
                self.user_profile.communication_style
            ));
        }

        // Current project focus
        if let Some(latest) = self.project_memory.values().max_by_key(|p| p.last_updated) {
            parts.push(format!(
                "Currently working on: {} ({})",
                latest.name, latest.current_focus
            ));
        }

        if parts.is_empty() {
            "New user — no profile data yet.".to_string()
        } else {
            parts.join(". ") + "."
        }
    }

    /// Get the user profile.
    pub fn user_profile(&self) -> &UserProfile {
        &self.user_profile
    }

    /// Get project memory.
    pub fn project_memory(&self) -> &HashMap<String, ProjectContext> {
        &self.project_memory
    }

    /// Get behavioral patterns.
    pub fn patterns(&self) -> &[BehavioralPattern] {
        &self.behavioral_patterns
    }

    /// Knowledge depth score (0-100): how well the OS knows the user.
    pub fn knowledge_depth(&self) -> f64 {
        let expertise_score = (self.user_profile.expertise_areas.len() as f64 * 5.0).min(25.0);
        let language_score = (self.user_profile.coding_languages.len() as f64 * 5.0).min(15.0);
        let style_score = if self.user_profile.communication_style != "unknown" {
            10.0
        } else {
            0.0
        };
        let project_score = (self.project_memory.len() as f64 * 10.0).min(20.0);
        let pattern_score = (self.behavioral_patterns.len() as f64 * 2.0).min(15.0);
        let interaction_score = (self.user_profile.interaction_count as f64 / 10.0).min(15.0);

        (expertise_score
            + language_score
            + style_score
            + project_score
            + pattern_score
            + interaction_score)
            .min(100.0)
    }

    /// Detect languages mentioned in text (simple heuristic).
    pub fn detect_languages(text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let langs = [
            "rust",
            "python",
            "javascript",
            "typescript",
            "go",
            "java",
            "c++",
            "ruby",
            "swift",
            "kotlin",
            "scala",
            "elixir",
            "haskell",
            "lua",
        ];
        langs
            .iter()
            .filter(|lang| lower.contains(*lang))
            .map(|s| s.to_string())
            .collect()
    }
}

impl Default for KnowledgeAccumulator {
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
    fn records_interactions_and_builds_profile() {
        let mut acc = KnowledgeAccumulator::new();
        for _ in 0..5 {
            acc.record_interaction(InteractionSummary {
                topic: "rust".to_string(),
                languages_mentioned: vec!["rust".to_string()],
                score: 8.0,
                timestamp: 0,
            });
        }

        assert!(acc.user_profile().expertise_areas.contains_key("rust"));
        assert!(acc.user_profile().coding_languages.contains_key("rust"));
        assert_eq!(acc.user_profile().interaction_count, 5);
    }

    #[test]
    fn recurring_topics_detected() {
        let mut acc = KnowledgeAccumulator::new();
        // Need 3+ to qualify as recurring, but checked on 4th+
        for _ in 0..5 {
            acc.record_interaction(InteractionSummary {
                topic: "security".to_string(),
                languages_mentioned: vec![],
                score: 7.0,
                timestamp: 0,
            });
        }
        assert!(acc
            .user_profile()
            .recurring_topics
            .contains(&"security".to_string()));
    }

    #[test]
    fn knowledge_depth_increases() {
        let mut acc = KnowledgeAccumulator::new();
        let baseline = acc.knowledge_depth();

        acc.record_interaction(InteractionSummary {
            topic: "rust".to_string(),
            languages_mentioned: vec!["rust".to_string(), "python".to_string()],
            score: 8.0,
            timestamp: 0,
        });
        acc.set_communication_style("concise");

        assert!(acc.knowledge_depth() > baseline);
    }

    #[test]
    fn builds_user_context_string() {
        let mut acc = KnowledgeAccumulator::new();
        acc.record_interaction(InteractionSummary {
            topic: "code".to_string(),
            languages_mentioned: vec!["rust".to_string()],
            score: 9.0,
            timestamp: 0,
        });
        acc.set_communication_style("concise");

        let ctx = acc.build_user_context();
        assert!(ctx.contains("rust") || ctx.contains("Rust"));
        assert!(ctx.contains("concise"));
    }

    #[test]
    fn empty_profile_context() {
        let acc = KnowledgeAccumulator::new();
        assert_eq!(acc.build_user_context(), "New user — no profile data yet.");
    }

    #[test]
    fn detect_languages_works() {
        let langs = KnowledgeAccumulator::detect_languages("I'm writing Rust and Python code");
        assert!(langs.contains(&"rust".to_string()));
        assert!(langs.contains(&"python".to_string()));
    }

    #[test]
    fn patterns_deduplicated() {
        let mut acc = KnowledgeAccumulator::new();
        for _ in 0..3 {
            acc.record_pattern(BehavioralPattern {
                pattern_type: "schedule".to_string(),
                description: "works late".to_string(),
                confidence: 0.8,
                occurrences: 1,
                first_seen: 0,
                last_seen: 0,
            });
        }
        assert_eq!(acc.patterns().len(), 1);
        assert_eq!(acc.patterns()[0].occurrences, 3);
    }

    #[test]
    fn interactions_bounded() {
        let mut acc = KnowledgeAccumulator::new();
        acc.max_interactions = 5;
        for i in 0..10 {
            acc.record_interaction(InteractionSummary {
                topic: format!("topic-{i}"),
                languages_mentioned: vec![],
                score: 7.0,
                timestamp: 0,
            });
        }
        assert_eq!(acc.interaction_log.len(), 5);
    }
}
