//! Layer 7: Dream Cycle OS Optimization — extends the Dream Forge to optimize
//! the OS itself during idle time, not just agents.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────────

/// Types of OS-level dream optimizations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OSDreamType {
    AgentEvolution,
    PerformanceOptimization,
    SecurityHardening,
    KnowledgeConsolidation,
    RoutingOptimization,
    ResourceRebalancing,
    CodeCleanup,
    PredictivePreloading,
}

impl OSDreamType {
    /// Priority order (lower = higher priority).
    pub fn priority(&self) -> u32 {
        match self {
            Self::SecurityHardening => 1,
            Self::PerformanceOptimization => 2,
            Self::AgentEvolution => 3,
            Self::KnowledgeConsolidation => 4,
            Self::RoutingOptimization => 5,
            Self::ResourceRebalancing => 6,
            Self::PredictivePreloading => 7,
            Self::CodeCleanup => 8,
        }
    }

    /// All dream types in priority order.
    pub fn all_by_priority() -> Vec<Self> {
        let mut types = vec![
            Self::SecurityHardening,
            Self::PerformanceOptimization,
            Self::AgentEvolution,
            Self::KnowledgeConsolidation,
            Self::RoutingOptimization,
            Self::ResourceRebalancing,
            Self::PredictivePreloading,
            Self::CodeCleanup,
        ];
        types.sort_by_key(|t| t.priority());
        types
    }
}

/// Result of a single dream optimization task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamOptimizationResult {
    pub dream_type: OSDreamType,
    pub description: String,
    pub improved: bool,
    pub before_metric: Option<f64>,
    pub after_metric: Option<f64>,
    pub timestamp: u64,
}

/// Morning briefing covering all OS improvements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSBriefing {
    pub generated_at: u64,
    pub improvements: Vec<DreamOptimizationResult>,
    pub total_improvements: u32,
    pub total_attempted: u32,
    pub summary: String,
}

// ── OSDreamCycle ────────────────────────────────────────────────────────────

/// Manages OS-level dream cycle optimizations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSDreamCycle {
    history: Vec<DreamOptimizationResult>,
    enabled_types: Vec<OSDreamType>,
    last_cycle: u64,
    max_history: usize,
    token_budget_per_cycle: u32,
    time_budget_secs: u64,
}

impl OSDreamCycle {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            enabled_types: OSDreamType::all_by_priority(),
            last_cycle: 0,
            max_history: 500,
            token_budget_per_cycle: 50_000,
            time_budget_secs: 300, // 5 minutes
        }
    }

    /// Record a dream optimization result.
    pub fn record_result(&mut self, result: DreamOptimizationResult) {
        self.history.push(result);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Mark the start of a dream cycle.
    pub fn start_cycle(&mut self) {
        self.last_cycle = epoch_secs();
    }

    /// Get the dream types that should run this cycle, in priority order.
    pub fn pending_dream_types(&self) -> Vec<OSDreamType> {
        self.enabled_types.clone()
    }

    /// Enable or disable a specific dream type.
    pub fn set_enabled(&mut self, dream_type: OSDreamType, enabled: bool) {
        if enabled {
            if !self.enabled_types.contains(&dream_type) {
                self.enabled_types.push(dream_type);
                self.enabled_types.sort_by_key(|t| t.priority());
            }
        } else {
            self.enabled_types.retain(|t| t != &dream_type);
        }
    }

    /// Generate a morning briefing from the last cycle's results.
    pub fn generate_briefing(&self) -> OSBriefing {
        let last_24h = epoch_secs().saturating_sub(86400);
        let recent: Vec<&DreamOptimizationResult> = self
            .history
            .iter()
            .filter(|r| r.timestamp >= last_24h)
            .collect();

        let improvements: Vec<DreamOptimizationResult> = recent
            .iter()
            .filter(|r| r.improved)
            .cloned()
            .cloned()
            .collect();

        let total_improved = improvements.len() as u32;
        let total_attempted = recent.len() as u32;

        let summary = if improvements.is_empty() {
            "No OS improvements during the last dream cycle.".to_string()
        } else {
            let descriptions: Vec<String> = improvements
                .iter()
                .map(|r| format!("- {}", r.description))
                .collect();
            format!("While you were away:\n{}", descriptions.join("\n"))
        };

        OSBriefing {
            generated_at: epoch_secs(),
            improvements,
            total_improvements: total_improved,
            total_attempted,
            summary,
        }
    }

    /// Get optimization history.
    pub fn history(&self) -> &[DreamOptimizationResult] {
        &self.history
    }

    /// Get recent improvements (last N).
    pub fn recent_improvements(&self, limit: usize) -> Vec<&DreamOptimizationResult> {
        self.history
            .iter()
            .rev()
            .filter(|r| r.improved)
            .take(limit)
            .collect()
    }

    /// Success rate for a specific dream type.
    pub fn success_rate(&self, dream_type: &OSDreamType) -> f64 {
        let relevant: Vec<&DreamOptimizationResult> = self
            .history
            .iter()
            .filter(|r| &r.dream_type == dream_type)
            .collect();
        if relevant.is_empty() {
            return 0.0;
        }
        let successes = relevant.iter().filter(|r| r.improved).count();
        successes as f64 / relevant.len() as f64
    }

    /// Token budget per cycle.
    pub fn token_budget(&self) -> u32 {
        self.token_budget_per_cycle
    }

    /// Set token budget.
    pub fn set_token_budget(&mut self, budget: u32) {
        self.token_budget_per_cycle = budget;
    }
}

impl Default for OSDreamCycle {
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
    fn dream_types_in_priority_order() {
        let types = OSDreamType::all_by_priority();
        assert_eq!(types[0], OSDreamType::SecurityHardening);
        assert_eq!(types[1], OSDreamType::PerformanceOptimization);
        assert_eq!(types[2], OSDreamType::AgentEvolution);
    }

    #[test]
    fn records_results_and_generates_briefing() {
        let mut cycle = OSDreamCycle::new();
        cycle.record_result(DreamOptimizationResult {
            dream_type: OSDreamType::PerformanceOptimization,
            description: "Optimized list_agents query (200ms → 50ms)".to_string(),
            improved: true,
            before_metric: Some(200.0),
            after_metric: Some(50.0),
            timestamp: epoch_secs(),
        });
        cycle.record_result(DreamOptimizationResult {
            dream_type: OSDreamType::SecurityHardening,
            description: "Evolved 2 detection rules".to_string(),
            improved: true,
            before_metric: None,
            after_metric: None,
            timestamp: epoch_secs(),
        });

        let briefing = cycle.generate_briefing();
        assert_eq!(briefing.total_improvements, 2);
        assert!(briefing.summary.contains("While you were away"));
    }

    #[test]
    fn success_rate_calculation() {
        let mut cycle = OSDreamCycle::new();
        for i in 0..10 {
            cycle.record_result(DreamOptimizationResult {
                dream_type: OSDreamType::AgentEvolution,
                description: format!("evolution {i}"),
                improved: i % 2 == 0, // 50% success
                before_metric: None,
                after_metric: None,
                timestamp: epoch_secs(),
            });
        }
        assert!((cycle.success_rate(&OSDreamType::AgentEvolution) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn enable_disable_dream_types() {
        let mut cycle = OSDreamCycle::new();
        assert!(cycle
            .pending_dream_types()
            .contains(&OSDreamType::CodeCleanup));

        cycle.set_enabled(OSDreamType::CodeCleanup, false);
        assert!(!cycle
            .pending_dream_types()
            .contains(&OSDreamType::CodeCleanup));

        cycle.set_enabled(OSDreamType::CodeCleanup, true);
        assert!(cycle
            .pending_dream_types()
            .contains(&OSDreamType::CodeCleanup));
    }

    #[test]
    fn history_bounded() {
        let mut cycle = OSDreamCycle::new();
        cycle.max_history = 5;
        for i in 0..10 {
            cycle.record_result(DreamOptimizationResult {
                dream_type: OSDreamType::AgentEvolution,
                description: format!("result {i}"),
                improved: true,
                before_metric: None,
                after_metric: None,
                timestamp: epoch_secs(),
            });
        }
        assert_eq!(cycle.history().len(), 5);
    }

    #[test]
    fn empty_briefing() {
        let cycle = OSDreamCycle::new();
        let briefing = cycle.generate_briefing();
        assert_eq!(briefing.total_improvements, 0);
        assert!(briefing.summary.contains("No OS improvements"));
    }
}
