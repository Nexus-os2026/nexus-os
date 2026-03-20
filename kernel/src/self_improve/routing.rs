//! Layer 2: Routing Intelligence — learns which agent handles which request best.
//!
//! After every task, [`RoutingLearner`] records which agent handled the request
//! and how well it scored. Over time, this builds a model that recommends the
//! best agent for any request category.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Types ───────────────────────────────────────────────────────────────────

/// A single routing observation: which agent handled a request and how well.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingOutcome {
    pub request_summary: String,
    pub request_category: String,
    pub agent_id: String,
    pub score: f64,
    pub timestamp: u64,
}

/// Running average score for an agent in a given category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScore {
    pub agent_id: String,
    pub avg_score: f64,
    pub sample_count: u64,
    total_score: f64,
}

impl AgentScore {
    pub fn new(agent_id: impl Into<String>, score: f64) -> Self {
        Self {
            agent_id: agent_id.into(),
            avg_score: score,
            sample_count: 1,
            total_score: score,
        }
    }

    pub fn update(&mut self, score: f64) {
        self.total_score += score;
        self.sample_count += 1;
        self.avg_score = self.total_score / self.sample_count as f64;
    }
}

/// Snapshot of routing model state for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStats {
    pub total_observations: u64,
    pub categories: Vec<CategoryStats>,
}

/// Per-category breakdown of agent performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub category: String,
    pub best_agent: Option<String>,
    pub best_score: f64,
    pub agent_scores: Vec<AgentScore>,
}

// ── RoutingLearner ──────────────────────────────────────────────────────────

/// Learns optimal agent routing from observed task outcomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingLearner {
    routing_history: Vec<RoutingOutcome>,
    routing_model: HashMap<String, Vec<AgentScore>>,
    max_history: usize,
}

impl RoutingLearner {
    pub fn new() -> Self {
        Self {
            routing_history: Vec::new(),
            routing_model: HashMap::new(),
            max_history: 2000,
        }
    }

    /// Record a routing outcome after a task is completed and scored.
    pub fn record(&mut self, outcome: RoutingOutcome) {
        // Update the routing model
        let scores = self
            .routing_model
            .entry(outcome.request_category.clone())
            .or_default();

        if let Some(entry) = scores.iter_mut().find(|s| s.agent_id == outcome.agent_id) {
            entry.update(outcome.score);
        } else {
            scores.push(AgentScore::new(&outcome.agent_id, outcome.score));
        }

        // Keep history bounded
        self.routing_history.push(outcome);
        if self.routing_history.len() > self.max_history {
            self.routing_history.remove(0);
        }
    }

    /// Get the best agent for a given request category.
    pub fn recommend_agent(&self, category: &str) -> Option<String> {
        self.routing_model
            .get(category)
            .and_then(|scores| {
                scores
                    .iter()
                    .filter(|s| s.sample_count >= 3) // Need minimum samples
                    .max_by(|a, b| {
                        a.avg_score
                            .partial_cmp(&b.avg_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .map(|s| s.agent_id.clone())
    }

    /// Get the top N agents for a category, ranked by score.
    pub fn top_agents(&self, category: &str, n: usize) -> Vec<AgentScore> {
        let mut scores = self
            .routing_model
            .get(category)
            .cloned()
            .unwrap_or_default();
        scores.sort_by(|a, b| {
            b.avg_score
                .partial_cmp(&a.avg_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores.truncate(n);
        scores
    }

    /// Get full routing stats for the UI.
    pub fn get_stats(&self) -> RoutingStats {
        let categories = self
            .routing_model
            .iter()
            .map(|(cat, scores)| {
                let best = scores.iter().max_by(|a, b| {
                    a.avg_score
                        .partial_cmp(&b.avg_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                CategoryStats {
                    category: cat.clone(),
                    best_agent: best.map(|s| s.agent_id.clone()),
                    best_score: best.map(|s| s.avg_score).unwrap_or(0.0),
                    agent_scores: scores.clone(),
                }
            })
            .collect();

        RoutingStats {
            total_observations: self.routing_history.len() as u64,
            categories,
        }
    }

    /// Get known categories.
    pub fn categories(&self) -> Vec<String> {
        self.routing_model.keys().cloned().collect()
    }

    /// Categorize a request into a standard category based on keywords.
    /// For LLM-based categorization, the caller should use a cheap model.
    pub fn categorize_heuristic(request: &str) -> String {
        let lower = request.to_lowercase();
        let categories = [
            (
                "code",
                &[
                    "code",
                    "function",
                    "bug",
                    "compile",
                    "syntax",
                    "refactor",
                    "implement",
                    "class",
                    "method",
                ][..],
            ),
            (
                "security",
                &[
                    "security",
                    "vulnerab",
                    "exploit",
                    "attack",
                    "firewall",
                    "threat",
                    "injection",
                ],
            ),
            (
                "research",
                &[
                    "research",
                    "find",
                    "search",
                    "analyze",
                    "investigate",
                    "compare",
                    "review",
                ],
            ),
            (
                "design",
                &[
                    "design",
                    "ui",
                    "ux",
                    "layout",
                    "color",
                    "style",
                    "component",
                    "visual",
                ],
            ),
            (
                "devops",
                &[
                    "deploy",
                    "ci",
                    "cd",
                    "docker",
                    "kubernetes",
                    "pipeline",
                    "infrastructure",
                ],
            ),
            (
                "data",
                &[
                    "data",
                    "database",
                    "sql",
                    "query",
                    "schema",
                    "migration",
                    "analytics",
                ],
            ),
            (
                "writing",
                &[
                    "write", "document", "blog", "article", "readme", "describe", "explain",
                ],
            ),
            (
                "planning",
                &[
                    "plan",
                    "roadmap",
                    "architecture",
                    "design",
                    "strategy",
                    "milestone",
                ],
            ),
            (
                "testing",
                &[
                    "test",
                    "coverage",
                    "assertion",
                    "mock",
                    "fixture",
                    "benchmark",
                ],
            ),
        ];

        for (category, keywords) in &categories {
            if keywords.iter().any(|kw| lower.contains(kw)) {
                return category.to_string();
            }
        }
        "general".to_string()
    }
}

impl Default for RoutingLearner {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_outcome(category: &str, agent: &str, score: f64) -> RoutingOutcome {
        RoutingOutcome {
            request_summary: "test request".to_string(),
            request_category: category.to_string(),
            agent_id: agent.to_string(),
            score,
            timestamp: 0,
        }
    }

    #[test]
    fn routing_learns_best_agent() {
        let mut learner = RoutingLearner::new();

        // Forge handles code well
        for _ in 0..5 {
            learner.record(make_outcome("code", "forge", 9.0));
        }
        // Aegis handles code poorly
        for _ in 0..5 {
            learner.record(make_outcome("code", "aegis", 5.0));
        }

        assert_eq!(learner.recommend_agent("code"), Some("forge".to_string()));
    }

    #[test]
    fn routing_requires_minimum_samples() {
        let mut learner = RoutingLearner::new();
        learner.record(make_outcome("code", "forge", 10.0));
        // Only 1 sample — below minimum of 3
        assert_eq!(learner.recommend_agent("code"), None);
    }

    #[test]
    fn routing_returns_none_for_unknown_category() {
        let learner = RoutingLearner::new();
        assert_eq!(learner.recommend_agent("unknown"), None);
    }

    #[test]
    fn top_agents_sorted_by_score() {
        let mut learner = RoutingLearner::new();
        for _ in 0..5 {
            learner.record(make_outcome("security", "aegis", 9.0));
            learner.record(make_outcome("security", "forge", 6.0));
            learner.record(make_outcome("security", "scholar", 7.5));
        }

        let top = learner.top_agents("security", 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].agent_id, "aegis");
        assert_eq!(top[1].agent_id, "scholar");
    }

    #[test]
    fn categorize_heuristic_matches_keywords() {
        assert_eq!(
            RoutingLearner::categorize_heuristic("fix the bug in login"),
            "code"
        );
        assert_eq!(
            RoutingLearner::categorize_heuristic("check for vulnerabilities"),
            "security"
        );
        assert_eq!(
            RoutingLearner::categorize_heuristic("deploy to production"),
            "devops"
        );
        assert_eq!(
            RoutingLearner::categorize_heuristic("hello world"),
            "general"
        );
    }

    #[test]
    fn stats_include_all_categories() {
        let mut learner = RoutingLearner::new();
        for _ in 0..3 {
            learner.record(make_outcome("code", "forge", 8.0));
            learner.record(make_outcome("security", "aegis", 9.0));
        }

        let stats = learner.get_stats();
        assert_eq!(stats.total_observations, 6);
        assert_eq!(stats.categories.len(), 2);
    }

    #[test]
    fn history_bounded_at_max() {
        let mut learner = RoutingLearner::new();
        learner.max_history = 10;
        for i in 0..20 {
            learner.record(make_outcome("code", "forge", i as f64));
        }
        assert_eq!(learner.routing_history.len(), 10);
    }
}
