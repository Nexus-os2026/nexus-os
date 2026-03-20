//! Creation pattern storage — learn from past agent creations for reuse.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::deployer::GENESIS_MEMORY_DIR;
use super::generator::AgentSpec;

const PATTERNS_FILE: &str = "patterns.json";

/// A reusable creation pattern learned from a successful agent creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationPattern {
    pub trigger_keywords: Vec<String>,
    pub gap_type: String,
    pub agent_spec: AgentSpec,
    pub test_score: f64,
    pub times_reused: u32,
}

/// Store of creation patterns for the Genesis engine.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternStore {
    pub patterns: Vec<CreationPattern>,
}

impl PatternStore {
    /// Load patterns from disk.
    pub fn load(base_dir: &Path) -> Self {
        let path = patterns_path(base_dir);
        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save patterns to disk.
    pub fn save(&self, base_dir: &Path) -> Result<(), String> {
        let dir = base_dir.join(GENESIS_MEMORY_DIR);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create genesis memory directory: {e}"))?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize patterns: {e}"))?;

        std::fs::write(patterns_path(base_dir), json)
            .map_err(|e| format!("Failed to write patterns: {e}"))?;

        Ok(())
    }

    /// Store a successful creation as a reusable pattern.
    pub fn store_pattern(&mut self, pattern: CreationPattern) {
        // Check for duplicate by name
        if let Some(existing) = self
            .patterns
            .iter_mut()
            .find(|p| p.agent_spec.name == pattern.agent_spec.name)
        {
            // Update existing pattern with better score
            if pattern.test_score > existing.test_score {
                *existing = pattern;
            }
        } else {
            self.patterns.push(pattern);
        }
    }

    /// Find a similar pattern that can be adapted instead of creating from scratch.
    ///
    /// Returns the pattern and a similarity score (0.0–1.0) if a match is found
    /// with > 70% capability overlap.
    pub fn find_similar(
        &self,
        required_capabilities: &[String],
        missing_capabilities: &[String],
    ) -> Option<(&CreationPattern, f64)> {
        let mut best: Option<(&CreationPattern, f64)> = None;

        for pattern in &self.patterns {
            let pattern_caps: std::collections::HashSet<&str> = pattern
                .agent_spec
                .capabilities
                .iter()
                .map(|s| s.as_str())
                .collect();

            let required_set: std::collections::HashSet<&str> =
                required_capabilities.iter().map(|s| s.as_str()).collect();

            // Calculate overlap with required capabilities
            let overlap = pattern_caps.intersection(&required_set).count();
            let union = pattern_caps.union(&required_set).count();

            let capability_similarity = if union > 0 {
                overlap as f64 / union as f64
            } else {
                0.0
            };

            // Also check keyword overlap with trigger keywords
            let keyword_overlap = pattern
                .trigger_keywords
                .iter()
                .filter(|kw| {
                    missing_capabilities
                        .iter()
                        .any(|mc| mc.to_lowercase().contains(&kw.to_lowercase()))
                        || required_capabilities
                            .iter()
                            .any(|rc| rc.to_lowercase().contains(&kw.to_lowercase()))
                })
                .count();

            let keyword_similarity = if !pattern.trigger_keywords.is_empty() {
                keyword_overlap as f64 / pattern.trigger_keywords.len() as f64
            } else {
                0.0
            };

            // Combined similarity (weighted)
            let similarity = capability_similarity * 0.6 + keyword_similarity * 0.4;

            // Only consider matches above 70% threshold
            if similarity > 0.7
                && best.as_ref().is_none_or(|(_, best_score)| similarity > *best_score)
            {
                best = Some((pattern, similarity));
            }
        }

        best
    }
}

fn patterns_path(base_dir: &Path) -> PathBuf {
    base_dir.join(GENESIS_MEMORY_DIR).join(PATTERNS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pattern() -> CreationPattern {
        CreationPattern {
            trigger_keywords: vec![
                "database".to_string(),
                "sql".to_string(),
                "query".to_string(),
            ],
            gap_type: "data".to_string(),
            agent_spec: AgentSpec {
                name: "nexus-dbtuner".to_string(),
                display_name: "DB Tuner".to_string(),
                description: "Database optimization specialist".to_string(),
                system_prompt: "You are DB Tuner...".to_string(),
                autonomy_level: 3,
                capabilities: vec![
                    "fs.read".to_string(),
                    "fs.write".to_string(),
                    "llm.query".to_string(),
                ],
                tools: vec!["fs.read".to_string(), "fs.write".to_string()],
                category: "data".to_string(),
                reasoning_strategy: "chain_of_thought".to_string(),
                temperature: 0.7,
                parent_agents: Vec::new(),
            },
            test_score: 8.0,
            times_reused: 0,
        }
    }

    #[test]
    fn store_and_load_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = PatternStore::default();
        store.store_pattern(sample_pattern());
        store.save(tmp.path()).unwrap();

        let loaded = PatternStore::load(tmp.path());
        assert_eq!(loaded.patterns.len(), 1);
        assert_eq!(loaded.patterns[0].agent_spec.name, "nexus-dbtuner");
    }

    #[test]
    fn find_similar_pattern() {
        let mut store = PatternStore::default();
        store.store_pattern(sample_pattern());

        // Same capabilities should match
        let result = store.find_similar(
            &[
                "fs.read".to_string(),
                "fs.write".to_string(),
                "llm.query".to_string(),
            ],
            &["database".to_string(), "sql".to_string()],
        );
        assert!(result.is_some());
    }

    #[test]
    fn no_similar_for_unrelated() {
        let mut store = PatternStore::default();
        store.store_pattern(sample_pattern());

        let result = store.find_similar(
            &["web.search".to_string()],
            &["image_processing".to_string()],
        );
        assert!(result.is_none());
    }

    #[test]
    fn duplicate_pattern_updates() {
        let mut store = PatternStore::default();
        let mut p1 = sample_pattern();
        p1.test_score = 7.0;
        store.store_pattern(p1);

        let mut p2 = sample_pattern();
        p2.test_score = 9.0;
        store.store_pattern(p2);

        assert_eq!(store.patterns.len(), 1);
        assert!((store.patterns[0].test_score - 9.0).abs() < f64::EPSILON);
    }

    #[test]
    fn load_nonexistent_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = PatternStore::load(tmp.path());
        assert!(store.patterns.is_empty());
    }
}
