//! Layer 5: UI Adaptation — learns how the user interacts with the OS and
//! adapts the interface to match their patterns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Types ───────────────────────────────────────────────────────────────────

/// A single session interaction pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPattern {
    pub pages_visited: Vec<String>,
    pub primary_agent: Option<String>,
    pub session_duration_secs: u64,
    pub timestamp: u64,
}

/// Computed UI adaptations based on usage patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIAdaptation {
    pub sidebar_order: Vec<String>,
    pub default_page: String,
    pub default_agent: Option<String>,
    pub hidden_features: Vec<String>,
    pub quick_actions: Vec<String>,
    pub total_sessions: u64,
}

// ── UILearner ───────────────────────────────────────────────────────────────

/// Tracks user interaction patterns and generates UI adaptations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UILearner {
    page_visits: HashMap<String, u64>,
    sidebar_clicks: HashMap<String, u64>,
    feature_uses: HashMap<String, u64>,
    agent_uses: HashMap<String, u64>,
    session_patterns: Vec<SessionPattern>,
    total_sessions: u64,
    min_sessions_for_adaptation: u64,
    max_sessions: usize,
}

impl UILearner {
    pub fn new() -> Self {
        Self {
            page_visits: HashMap::new(),
            sidebar_clicks: HashMap::new(),
            feature_uses: HashMap::new(),
            agent_uses: HashMap::new(),
            session_patterns: Vec::new(),
            total_sessions: 0,
            min_sessions_for_adaptation: 5,
            max_sessions: 200,
        }
    }

    /// Record a page visit.
    pub fn record_page_visit(&mut self, page: &str) {
        *self.page_visits.entry(page.to_string()).or_default() += 1;
        *self.sidebar_clicks.entry(page.to_string()).or_default() += 1;
    }

    /// Record use of a specific feature.
    pub fn record_feature_use(&mut self, feature: &str) {
        *self.feature_uses.entry(feature.to_string()).or_default() += 1;
    }

    /// Record an agent being used.
    pub fn record_agent_use(&mut self, agent_id: &str) {
        *self.agent_uses.entry(agent_id.to_string()).or_default() += 1;
    }

    /// Record a complete session.
    pub fn record_session(&mut self, pattern: SessionPattern) {
        self.total_sessions += 1;
        self.session_patterns.push(pattern);
        if self.session_patterns.len() > self.max_sessions {
            self.session_patterns.remove(0);
        }
    }

    /// Get the most used page.
    pub fn most_visited_page(&self) -> Option<String> {
        self.page_visits
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(page, _)| page.clone())
    }

    /// Get the most used agent.
    pub fn most_used_agent(&self) -> Option<String> {
        self.agent_uses
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(agent, _)| agent.clone())
    }

    /// Get features that have never been used (after sufficient sessions).
    pub fn unused_features(&self, all_features: &[String]) -> Vec<String> {
        if self.total_sessions < self.min_sessions_for_adaptation {
            return Vec::new();
        }
        all_features
            .iter()
            .filter(|f| !self.feature_uses.contains_key(*f))
            .cloned()
            .collect()
    }

    /// Compute the full UI adaptation based on accumulated data.
    pub fn get_adaptation(&self) -> UIAdaptation {
        // Sort pages by visit frequency (descending)
        let mut sidebar_order: Vec<(String, u64)> = self
            .page_visits
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        sidebar_order.sort_by(|a, b| b.1.cmp(&a.1));
        let sidebar_order: Vec<String> = sidebar_order.into_iter().map(|(k, _)| k).collect();

        let default_page = self
            .most_visited_page()
            .unwrap_or_else(|| "chat".to_string());

        let default_agent = self.most_used_agent();

        // Features with zero uses after threshold sessions
        let hidden_features = if self.total_sessions >= self.min_sessions_for_adaptation {
            self.feature_uses
                .iter()
                .filter(|(_, count)| **count == 0)
                .map(|(f, _)| f.clone())
                .collect()
        } else {
            Vec::new()
        };

        // Most common action sequences
        let mut action_counts: HashMap<String, u64> = HashMap::new();
        for pattern in &self.session_patterns {
            if pattern.pages_visited.len() >= 2 {
                for window in pattern.pages_visited.windows(2) {
                    let seq = format!("{} → {}", window[0], window[1]);
                    *action_counts.entry(seq).or_default() += 1;
                }
            }
        }
        let mut quick_actions: Vec<(String, u64)> = action_counts.into_iter().collect();
        quick_actions.sort_by(|a, b| b.1.cmp(&a.1));
        let quick_actions: Vec<String> = quick_actions
            .into_iter()
            .take(5)
            .map(|(seq, _)| seq)
            .collect();

        UIAdaptation {
            sidebar_order,
            default_page,
            default_agent,
            hidden_features,
            quick_actions,
            total_sessions: self.total_sessions,
        }
    }

    /// Page visit counts (for the UI).
    pub fn page_visit_counts(&self) -> &HashMap<String, u64> {
        &self.page_visits
    }

    /// Feature use counts.
    pub fn feature_use_counts(&self) -> &HashMap<String, u64> {
        &self.feature_uses
    }
}

impl Default for UILearner {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_page_visits() {
        let mut learner = UILearner::new();
        learner.record_page_visit("chat");
        learner.record_page_visit("chat");
        learner.record_page_visit("agents");

        assert_eq!(learner.page_visits["chat"], 2);
        assert_eq!(learner.page_visits["agents"], 1);
        assert_eq!(learner.most_visited_page(), Some("chat".to_string()));
    }

    #[test]
    fn tracks_agent_uses() {
        let mut learner = UILearner::new();
        learner.record_agent_use("forge");
        learner.record_agent_use("forge");
        learner.record_agent_use("aegis");

        assert_eq!(learner.most_used_agent(), Some("forge".to_string()));
    }

    #[test]
    fn adaptation_sidebar_ordered_by_frequency() {
        let mut learner = UILearner::new();
        learner.record_page_visit("terminal");
        for _ in 0..5 {
            learner.record_page_visit("chat");
        }
        for _ in 0..3 {
            learner.record_page_visit("agents");
        }

        let adapt = learner.get_adaptation();
        assert_eq!(adapt.sidebar_order[0], "chat");
        assert_eq!(adapt.sidebar_order[1], "agents");
        assert_eq!(adapt.sidebar_order[2], "terminal");
    }

    #[test]
    fn quick_actions_from_session_patterns() {
        let mut learner = UILearner::new();
        for _ in 0..5 {
            learner.record_session(SessionPattern {
                pages_visited: vec!["chat".to_string(), "agents".to_string(), "code".to_string()],
                primary_agent: Some("forge".to_string()),
                session_duration_secs: 300,
                timestamp: 0,
            });
        }

        let adapt = learner.get_adaptation();
        assert!(adapt.quick_actions.contains(&"chat → agents".to_string()));
    }

    #[test]
    fn default_page_is_most_visited() {
        let mut learner = UILearner::new();
        for _ in 0..10 {
            learner.record_page_visit("terminal");
        }
        learner.record_page_visit("chat");

        assert_eq!(learner.get_adaptation().default_page, "terminal");
    }

    #[test]
    fn sessions_bounded() {
        let mut learner = UILearner::new();
        learner.max_sessions = 5;
        for _ in 0..10 {
            learner.record_session(SessionPattern {
                pages_visited: vec!["chat".to_string()],
                primary_agent: None,
                session_duration_secs: 60,
                timestamp: 0,
            });
        }
        assert_eq!(learner.session_patterns.len(), 5);
        assert_eq!(learner.total_sessions, 10);
    }
}
