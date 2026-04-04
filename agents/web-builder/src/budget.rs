//! Persistent budget tracking for the Nexus Builder.
//!
//! Stores provider budgets and build history in a JSON file at
//! `~/.nexus/builder_budget.json` so data survives app restarts.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Data Types ─────────────────────────────────────────────────────────────

/// A single build record with cost, token, and timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRecord {
    pub project_name: String,
    pub model_name: String,
    pub provider: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_usd: f64,
    pub elapsed_seconds: f64,
    pub lines_generated: usize,
    pub checkpoint_id: String,
    pub timestamp: String,
}

/// Budget allocation for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBudget {
    pub provider: String,
    pub initial_budget_usd: f64,
    pub spent_usd: f64,
}

/// Root data structure persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetData {
    pub budgets: Vec<ProviderBudget>,
    pub builds: Vec<BuildRecord>,
}

impl Default for BudgetData {
    fn default() -> Self {
        Self {
            budgets: vec![
                ProviderBudget {
                    provider: "anthropic".into(),
                    initial_budget_usd: 0.0,
                    spent_usd: 0.0,
                },
                ProviderBudget {
                    provider: "openai".into(),
                    initial_budget_usd: 0.0,
                    spent_usd: 0.0,
                },
            ],
            builds: Vec::new(),
        }
    }
}

/// Summary returned to the frontend for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub anthropic_initial: f64,
    pub anthropic_spent: f64,
    pub anthropic_remaining: f64,
    pub openai_initial: f64,
    pub openai_spent: f64,
    pub openai_remaining: f64,
    pub total_builds: usize,
    pub avg_cost_per_build: f64,
    pub estimated_builds_remaining: usize,
}

// ── Provider Detection ─────────────────────────────────────────────────────

/// Detect the provider from a model name string.
pub fn detect_provider(model_name: &str) -> &'static str {
    let lower = model_name.to_lowercase();
    if lower.contains("claude")
        || lower.contains("haiku")
        || lower.contains("sonnet")
        || lower.contains("opus")
    {
        "anthropic"
    } else if lower.contains("gpt") {
        "openai"
    } else {
        "other"
    }
}

// ── Budget Tracker ─────────────────────────────────────────────────────────

/// Manages budget persistence via a JSON file at `~/.nexus/builder_budget.json`.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    path: PathBuf,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    /// Create a tracker using the default path `~/.nexus/builder_budget.json`.
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Self {
            path: PathBuf::from(home)
                .join(".nexus")
                .join("builder_budget.json"),
        }
    }

    /// Create a tracker with a custom file path (useful for tests).
    #[cfg(test)]
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Load budget data from disk, returning defaults if the file is absent.
    pub fn load(&self) -> BudgetData {
        match std::fs::read_to_string(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => BudgetData::default(),
        }
    }

    /// Persist budget data to disk, creating parent directories as needed.
    fn save(&self, data: &BudgetData) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create budget dir: {e}"))?;
        }
        let json =
            serde_json::to_string_pretty(data).map_err(|e| format!("serialization error: {e}"))?;
        std::fs::write(&self.path, json).map_err(|e| format!("failed to write budget file: {e}"))
    }

    /// Record a completed build, updating the provider's spent amount.
    pub fn record_build(&self, record: BuildRecord) -> Result<(), String> {
        let mut data = self.load();

        // Update spent for the matching provider
        if let Some(budget) = data
            .budgets
            .iter_mut()
            .find(|b| b.provider == record.provider)
        {
            budget.spent_usd += record.cost_usd;
        }

        data.builds.push(record);
        self.save(&data)
    }

    /// Set or update the initial budget for a provider.
    pub fn set_initial_budget(&self, provider: &str, amount: f64) -> Result<(), String> {
        let mut data = self.load();

        if let Some(budget) = data.budgets.iter_mut().find(|b| b.provider == provider) {
            budget.initial_budget_usd = amount;
        } else {
            data.budgets.push(ProviderBudget {
                provider: provider.to_string(),
                initial_budget_usd: amount,
                spent_usd: 0.0,
            });
        }

        self.save(&data)
    }

    /// Set the remaining balance for a provider by adjusting `spent_usd`.
    ///
    /// This lets users manually correct their balance when the tracked
    /// spend diverges from reality (e.g. API console shows different numbers).
    pub fn set_remaining(&self, provider: &str, remaining: f64) -> Result<(), String> {
        let mut data = self.load();

        if let Some(budget) = data.budgets.iter_mut().find(|b| b.provider == provider) {
            budget.spent_usd = (budget.initial_budget_usd - remaining).max(0.0);
        } else {
            // Provider not yet tracked — create with initial = remaining, spent = 0
            data.budgets.push(ProviderBudget {
                provider: provider.to_string(),
                initial_budget_usd: remaining,
                spent_usd: 0.0,
            });
        }

        self.save(&data)
    }

    /// Return the last N build records (most recent last), capped at 50.
    pub fn get_build_history(&self) -> Vec<BuildRecord> {
        let data = self.load();
        let len = data.builds.len();
        let start = len.saturating_sub(50);
        data.builds[start..].to_vec()
    }

    /// Calculate a summary status for the frontend.
    pub fn get_budget_status(&self) -> BudgetStatus {
        let data = self.load();

        let anthropic = data
            .budgets
            .iter()
            .find(|b| b.provider == "anthropic")
            .cloned()
            .unwrap_or(ProviderBudget {
                provider: "anthropic".into(),
                initial_budget_usd: 0.0,
                spent_usd: 0.0,
            });

        let openai = data
            .budgets
            .iter()
            .find(|b| b.provider == "openai")
            .cloned()
            .unwrap_or(ProviderBudget {
                provider: "openai".into(),
                initial_budget_usd: 0.0,
                spent_usd: 0.0,
            });

        let total_builds = data.builds.len();

        // Average cost from the last 5 builds
        let recent: Vec<&BuildRecord> = data.builds.iter().rev().take(5).collect();
        let avg_cost_per_build = if recent.is_empty() {
            0.0
        } else {
            recent.iter().map(|b| b.cost_usd).sum::<f64>() / recent.len() as f64
        };

        let total_remaining = (anthropic.initial_budget_usd - anthropic.spent_usd).max(0.0)
            + (openai.initial_budget_usd - openai.spent_usd).max(0.0);

        let estimated_builds_remaining = if avg_cost_per_build > 0.0 {
            (total_remaining / avg_cost_per_build) as usize
        } else {
            0
        };

        BudgetStatus {
            anthropic_initial: anthropic.initial_budget_usd,
            anthropic_spent: anthropic.spent_usd,
            anthropic_remaining: (anthropic.initial_budget_usd - anthropic.spent_usd).max(0.0),
            openai_initial: openai.initial_budget_usd,
            openai_spent: openai.spent_usd,
            openai_remaining: (openai.initial_budget_usd - openai.spent_usd).max(0.0),
            total_builds,
            avg_cost_per_build,
            estimated_builds_remaining,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_tracker() -> BudgetTracker {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join("nexus_budget_test").join(format!(
            "{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("budget.json");
        // Ensure clean slate
        let _ = fs::remove_file(&path);
        BudgetTracker::with_path(path)
    }

    fn sample_record(name: &str, cost: f64) -> BuildRecord {
        BuildRecord {
            project_name: name.into(),
            model_name: "claude-sonnet-4-20250514".into(),
            provider: "anthropic".into(),
            input_tokens: 500,
            output_tokens: 2000,
            cost_usd: cost,
            elapsed_seconds: 12.3,
            lines_generated: 150,
            checkpoint_id: "ckpt-1".into(),
            timestamp: "2026-04-03T12:00:00Z".into(),
        }
    }

    #[test]
    fn default_loads_empty() {
        let tracker = temp_tracker();
        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 0);
        assert_eq!(status.anthropic_initial, 0.0);
    }

    #[test]
    fn set_budget_persists() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 10.0).unwrap();
        let status = tracker.get_budget_status();
        assert_eq!(status.anthropic_initial, 10.0);
        assert_eq!(status.anthropic_remaining, 10.0);
    }

    #[test]
    fn record_build_updates_spent() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 10.0).unwrap();
        tracker
            .record_build(sample_record("test-site", 0.05))
            .unwrap();

        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 1);
        assert!((status.anthropic_spent - 0.05).abs() < 1e-10);
        assert!((status.anthropic_remaining - 9.95).abs() < 1e-10);
    }

    #[test]
    fn build_history_caps_at_50() {
        let tracker = temp_tracker();
        for i in 0..60 {
            tracker
                .record_build(BuildRecord {
                    project_name: format!("proj-{i}"),
                    checkpoint_id: format!("ckpt-{i}"),
                    ..sample_record("", 0.01)
                })
                .unwrap();
        }
        let history = tracker.get_build_history();
        assert_eq!(history.len(), 50);
        assert_eq!(history[0].project_name, "proj-10");
    }

    #[test]
    fn detect_provider_works() {
        assert_eq!(detect_provider("claude-sonnet-4-20250514"), "anthropic");
        assert_eq!(detect_provider("claude-3-haiku"), "anthropic");
        assert_eq!(detect_provider("gpt-4o"), "openai");
        assert_eq!(detect_provider("llama-3.1-70b"), "other");
    }

    #[test]
    fn estimated_builds_remaining() {
        let tracker = temp_tracker();
        tracker.set_initial_budget("anthropic", 1.0).unwrap();
        for _ in 0..5 {
            tracker.record_build(sample_record("site", 0.10)).unwrap();
        }
        let status = tracker.get_budget_status();
        // Spent 0.50, remaining 0.50, avg 0.10 → 5 builds remaining
        assert_eq!(status.estimated_builds_remaining, 5);
    }

    #[test]
    fn corrupt_file_returns_defaults() {
        let tracker = temp_tracker();
        fs::create_dir_all(tracker.path.parent().unwrap()).unwrap();
        fs::write(&tracker.path, "not valid json!!!").unwrap();
        let status = tracker.get_budget_status();
        assert_eq!(status.total_builds, 0);
    }
}
