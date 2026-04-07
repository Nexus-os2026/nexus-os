use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agent::AgentAction;
use crate::governance::app_registry::AppCategory;

/// A learned UI pattern — a sequence of actions that achieves a goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIPattern {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: String,
    pub app_context: AppCategory,
    pub actions: Vec<PatternAction>,
    pub success_count: u32,
    pub failure_count: u32,
    pub avg_duration_ms: u64,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub version: u32,
}

impl UIPattern {
    /// Recompute confidence from success/failure counts
    pub fn recompute_confidence(&mut self) {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            self.confidence = 0.0;
        } else {
            self.confidence = self.success_count as f32 / total as f32;
        }
    }

    /// Total number of uses
    pub fn total_uses(&self) -> u32 {
        self.success_count + self.failure_count
    }
}

/// A single action within a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAction {
    pub action_type: String,
    pub parameters: serde_json::Value,
    pub relative_to: Option<String>,
    pub wait_after_ms: u64,
}

impl PatternAction {
    /// Convert from AgentAction to PatternAction
    pub fn from_agent_action(action: &AgentAction) -> Self {
        match action {
            AgentAction::Click { x, y, button } => PatternAction {
                action_type: "click".to_string(),
                parameters: serde_json::json!({ "x": x, "y": y, "button": button }),
                relative_to: None,
                wait_after_ms: 100,
            },
            AgentAction::DoubleClick { x, y } => PatternAction {
                action_type: "double_click".to_string(),
                parameters: serde_json::json!({ "x": x, "y": y }),
                relative_to: None,
                wait_after_ms: 100,
            },
            AgentAction::Type { text } => PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": text }),
                relative_to: None,
                wait_after_ms: 50,
            },
            AgentAction::KeyPress { key } => PatternAction {
                action_type: "key_combo".to_string(),
                parameters: serde_json::json!({ "key": key }),
                relative_to: None,
                wait_after_ms: 100,
            },
            AgentAction::Scroll {
                x,
                y,
                direction,
                amount,
            } => PatternAction {
                action_type: "scroll".to_string(),
                parameters: serde_json::json!({
                    "x": x, "y": y,
                    "direction": direction, "amount": amount
                }),
                relative_to: None,
                wait_after_ms: 200,
            },
            AgentAction::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
            } => PatternAction {
                action_type: "drag".to_string(),
                parameters: serde_json::json!({
                    "start_x": start_x, "start_y": start_y,
                    "end_x": end_x, "end_y": end_y
                }),
                relative_to: None,
                wait_after_ms: 200,
            },
            AgentAction::Wait { ms } => PatternAction {
                action_type: "wait".to_string(),
                parameters: serde_json::json!({ "ms": ms }),
                relative_to: None,
                wait_after_ms: 0,
            },
            AgentAction::Screenshot => PatternAction {
                action_type: "screenshot".to_string(),
                parameters: serde_json::json!({}),
                relative_to: None,
                wait_after_ms: 0,
            },
            AgentAction::Done { summary } => PatternAction {
                action_type: "done".to_string(),
                parameters: serde_json::json!({ "summary": summary }),
                relative_to: None,
                wait_after_ms: 0,
            },
        }
    }
}

/// Result of matching a pattern against a task
#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern: UIPattern,
    pub score: f32,
}

/// Library of learned UI patterns
pub struct PatternLibrary {
    patterns: Vec<UIPattern>,
    file_path: PathBuf,
}

impl PatternLibrary {
    /// Create a new pattern library with the given file path
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            patterns: Vec::new(),
            file_path,
        }
    }

    /// Create with default path (~/.nexus/ui_patterns.json)
    pub fn with_default_path() -> Self {
        let path = dirs_path().join("ui_patterns.json");
        Self::new(path)
    }

    /// Load patterns from disk
    pub fn load(&mut self) -> Result<(), String> {
        if !self.file_path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read patterns file: {e}"))?;
        match serde_json::from_str(&data) {
            Ok(patterns) => {
                self.patterns = patterns;
                Ok(())
            }
            Err(e) => {
                warn!("Corrupted patterns file, starting fresh: {e}");
                self.patterns = Vec::new();
                Ok(())
            }
        }
    }

    /// Save patterns to disk
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        let data = serde_json::to_string_pretty(&self.patterns)
            .map_err(|e| format!("Failed to serialize patterns: {e}"))?;
        std::fs::write(&self.file_path, data)
            .map_err(|e| format!("Failed to write patterns file: {e}"))?;
        Ok(())
    }

    /// Find patterns matching a task description, sorted by confidence
    pub fn find_matching(&self, task: &str) -> Vec<PatternMatch> {
        let mut matches: Vec<PatternMatch> = self
            .patterns
            .iter()
            .map(|p| PatternMatch {
                score: match_score(task, &p.trigger),
                pattern: p.clone(),
            })
            .filter(|m| m.score > 0.1)
            .collect();
        matches.sort_by(|a, b| {
            // Sort by score first, then by confidence
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    b.pattern
                        .confidence
                        .partial_cmp(&a.pattern.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        matches
    }

    /// Record a successful use of a pattern
    pub fn record_success(&mut self, pattern_id: &str) {
        if let Some(p) = self.patterns.iter_mut().find(|p| p.id == pattern_id) {
            p.success_count += 1;
            p.recompute_confidence();
            p.last_used = Utc::now();
        }
    }

    /// Record a failed use of a pattern
    pub fn record_failure(&mut self, pattern_id: &str) {
        if let Some(p) = self.patterns.iter_mut().find(|p| p.id == pattern_id) {
            p.failure_count += 1;
            p.recompute_confidence();
            p.last_used = Utc::now();
        }
    }

    /// Add a new pattern to the library
    pub fn add_pattern(&mut self, pattern: UIPattern) {
        self.patterns.push(pattern);
    }

    /// Remove patterns with confidence < 0.2 and total uses > 10
    pub fn prune_low_confidence(&mut self) -> usize {
        let before = self.patterns.len();
        self.patterns.retain(|p| {
            let total = p.total_uses();
            !(p.confidence < 0.2 && total > 10)
        });
        before - self.patterns.len()
    }

    /// Get all patterns
    pub fn patterns(&self) -> &[UIPattern] {
        &self.patterns
    }

    /// Get mutable access to patterns
    pub fn patterns_mut(&mut self) -> &mut Vec<UIPattern> {
        &mut self.patterns
    }

    /// Get pattern count
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// Compute a fuzzy match score between a task and a trigger using word overlap
pub fn match_score(task: &str, trigger: &str) -> f32 {
    let task_words: HashSet<String> = task
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let trigger_words: HashSet<String> = trigger
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let total = trigger_words.len().max(1);
    let overlap = task_words.intersection(&trigger_words).count();
    overlap as f32 / total as f32
}

/// Get the default nexus data directory
fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".nexus")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_pattern(name: &str, trigger: &str) -> UIPattern {
        UIPattern {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: format!("Test pattern: {name}"),
            trigger: trigger.to_string(),
            app_context: AppCategory::Terminal,
            actions: vec![PatternAction {
                action_type: "type".to_string(),
                parameters: serde_json::json!({ "text": "cargo test" }),
                relative_to: None,
                wait_after_ms: 100,
            }],
            success_count: 5,
            failure_count: 1,
            avg_duration_ms: 3000,
            confidence: 5.0 / 6.0,
            created_at: Utc::now(),
            last_used: Utc::now(),
            version: 1,
        }
    }

    #[test]
    fn test_pattern_creation() {
        let p = make_test_pattern("test_runner", "run cargo test");
        assert_eq!(p.name, "test_runner");
        assert_eq!(p.success_count, 5);
        assert_eq!(p.failure_count, 1);
        assert!(p.confidence > 0.8);
    }

    #[test]
    fn test_pattern_serialize_deserialize() {
        let p = make_test_pattern("test_runner", "run cargo test");
        let json = serde_json::to_string(&p).expect("serialize");
        let p2: UIPattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p2.name, p.name);
        assert_eq!(p2.id, p.id);
        assert_eq!(p2.actions.len(), p.actions.len());
    }

    #[test]
    fn test_pattern_confidence_calculation() {
        let mut p = make_test_pattern("test", "test");
        p.success_count = 7;
        p.failure_count = 3;
        p.recompute_confidence();
        assert!((p.confidence - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_pattern_action_from_click() {
        let action = AgentAction::Click {
            x: 100,
            y: 200,
            button: "left".to_string(),
        };
        let pa = PatternAction::from_agent_action(&action);
        assert_eq!(pa.action_type, "click");
        assert_eq!(pa.parameters["x"], 100);
        assert_eq!(pa.parameters["y"], 200);
    }

    #[test]
    fn test_pattern_action_from_type() {
        let action = AgentAction::Type {
            text: "hello world".to_string(),
        };
        let pa = PatternAction::from_agent_action(&action);
        assert_eq!(pa.action_type, "type");
        assert_eq!(pa.parameters["text"], "hello world");
    }

    #[test]
    fn test_pattern_action_from_combo() {
        let action = AgentAction::KeyPress {
            key: "ctrl+c".to_string(),
        };
        let pa = PatternAction::from_agent_action(&action);
        assert_eq!(pa.action_type, "key_combo");
        assert_eq!(pa.parameters["key"], "ctrl+c");
    }

    #[test]
    fn test_match_score_exact() {
        let score = match_score("run cargo test", "run cargo test");
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_match_score_partial() {
        let score = match_score("run tests", "run cargo test");
        assert!(score > 0.3); // "run" matches
    }

    #[test]
    fn test_match_score_no_match() {
        let score = match_score("open browser", "run cargo test");
        assert!(score < 0.2);
    }

    #[test]
    fn test_find_matching_returns_sorted() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("patterns.json");
        let mut lib = PatternLibrary::new(path);

        let mut p1 = make_test_pattern("exact", "run cargo test");
        p1.confidence = 0.9;
        let mut p2 = make_test_pattern("partial", "run build");
        p2.confidence = 0.8;

        lib.add_pattern(p1);
        lib.add_pattern(p2);

        let matches = lib.find_matching("run cargo test");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].pattern.name, "exact");
    }

    #[test]
    fn test_record_success_updates_confidence() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("patterns.json");
        let mut lib = PatternLibrary::new(path);

        let mut p = make_test_pattern("test", "run test");
        p.success_count = 3;
        p.failure_count = 3;
        p.recompute_confidence();
        let id = p.id.clone();
        lib.add_pattern(p);

        lib.record_success(&id);
        let updated = lib.patterns().iter().find(|p| p.id == id).expect("found");
        assert_eq!(updated.success_count, 4);
        assert!(updated.confidence > 0.5);
    }

    #[test]
    fn test_record_failure_updates_confidence() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("patterns.json");
        let mut lib = PatternLibrary::new(path);

        let mut p = make_test_pattern("test", "run test");
        p.success_count = 5;
        p.failure_count = 0;
        p.recompute_confidence();
        let id = p.id.clone();
        lib.add_pattern(p);

        lib.record_failure(&id);
        let updated = lib.patterns().iter().find(|p| p.id == id).expect("found");
        assert_eq!(updated.failure_count, 1);
        assert!(updated.confidence < 1.0);
    }

    #[test]
    fn test_prune_low_confidence() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("patterns.json");
        let mut lib = PatternLibrary::new(path);

        let mut good = make_test_pattern("good", "good pattern");
        good.success_count = 8;
        good.failure_count = 2;
        good.recompute_confidence();

        let mut bad = make_test_pattern("bad", "bad pattern");
        bad.success_count = 1;
        bad.failure_count = 10;
        bad.recompute_confidence();

        lib.add_pattern(good);
        lib.add_pattern(bad);

        let pruned = lib.prune_low_confidence();
        assert_eq!(pruned, 1);
        assert_eq!(lib.len(), 1);
        assert_eq!(lib.patterns()[0].name, "good");
    }

    #[test]
    fn test_pattern_library_save_load() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("patterns.json");

        let mut lib = PatternLibrary::new(path.clone());
        lib.add_pattern(make_test_pattern("test1", "trigger one"));
        lib.add_pattern(make_test_pattern("test2", "trigger two"));
        lib.save().expect("save");

        let mut lib2 = PatternLibrary::new(path);
        lib2.load().expect("load");
        assert_eq!(lib2.len(), 2);
    }
}
