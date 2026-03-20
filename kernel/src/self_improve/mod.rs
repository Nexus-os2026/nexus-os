//! Self-Improving OS — 8-layer system that makes Nexus OS learn, adapt, and
//! improve with every interaction.
//!
//! # Layers
//!
//! 1. **Agent Evolution** — auto-score and mutate agents (see [`genome::auto_evolve`])
//! 2. **Routing Intelligence** — learn which agent handles which request best
//! 3. **Performance Self-Optimization** — detect regressions, queue fixes
//! 4. **Security Evolution** — evolve detection rules from accuracy data
//! 5. **UI Adaptation** — reorder sidebar, default page, quick actions
//! 6. **Knowledge Accumulation** — build user profile across interactions
//! 7. **Dream Cycle OS Optimization** — optimize the OS itself during idle time
//! 8. **System-Wide Fitness Score** — single metric that trends upward

pub mod fitness;
pub mod knowledge;
pub mod os_dreams;
pub mod performance;
pub mod routing;
pub mod security;
pub mod ui_learning;

pub use fitness::{FitnessHistory, OSFitness};
pub use knowledge::{
    BehavioralPattern, InteractionSummary, KnowledgeAccumulator, ProjectContext, UserProfile,
};
pub use os_dreams::{DreamOptimizationResult, OSBriefing, OSDreamCycle, OSDreamType};
pub use performance::{Bottleneck, PerformanceEvolver, PerformanceReport, Trend};
pub use routing::{AgentScore, RoutingLearner, RoutingOutcome, RoutingStats};
pub use security::{RulePerformance, SecurityEvent, SecurityEvolutionReport, SecurityEvolver};
pub use ui_learning::{SessionPattern, UIAdaptation, UILearner};

use serde::{Deserialize, Serialize};

// ── Improvement Event ───────────────────────────────────────────────────────

/// A single self-improvement event logged for auditability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementEvent {
    pub layer: String,
    pub description: String,
    pub before_value: Option<f64>,
    pub after_value: Option<f64>,
    pub timestamp: u64,
}

// ── SelfImprovingOS ─────────────────────────────────────────────────────────

/// Central orchestrator for all 8 self-improvement layers.
///
/// Each layer runs independently and in the background. The orchestrator
/// ties them together and provides a unified API for querying state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfImprovingOS {
    pub routing: RoutingLearner,
    pub performance: PerformanceEvolver,
    pub security: SecurityEvolver,
    pub ui: UILearner,
    pub knowledge: KnowledgeAccumulator,
    pub dreams: OSDreamCycle,
    pub fitness_history: FitnessHistory,
    improvement_log: Vec<ImprovementEvent>,
    max_log_entries: usize,
    enabled: bool,
}

impl SelfImprovingOS {
    pub fn new() -> Self {
        Self {
            routing: RoutingLearner::new(),
            performance: PerformanceEvolver::new(),
            security: SecurityEvolver::new(),
            ui: UILearner::new(),
            knowledge: KnowledgeAccumulator::new(),
            dreams: OSDreamCycle::new(),
            fitness_history: FitnessHistory::new(),
            improvement_log: Vec::new(),
            max_log_entries: 1000,
            enabled: true,
        }
    }

    /// Log an improvement event.
    pub fn log_improvement(&mut self, event: ImprovementEvent) {
        self.improvement_log.push(event);
        if self.improvement_log.len() > self.max_log_entries {
            self.improvement_log.remove(0);
        }
    }

    /// Get the improvement log.
    pub fn improvement_log(&self, limit: u32) -> Vec<&ImprovementEvent> {
        self.improvement_log
            .iter()
            .rev()
            .take(limit as usize)
            .collect()
    }

    /// Compute the current OS fitness based on all layers.
    pub fn compute_fitness(&self) -> OSFitness {
        // Agent quality: from routing stats (average best scores across categories)
        let stats = self.routing.get_stats();
        let agent_quality = if stats.categories.is_empty() {
            50.0
        } else {
            let avg: f64 = stats.categories.iter().map(|c| c.best_score).sum::<f64>()
                / stats.categories.len() as f64;
            (avg * 10.0).clamp(0.0, 100.0) // Scale 0-10 to 0-100
        };

        // Routing accuracy: percentage of categories with a clear best agent
        let routing_accuracy = if stats.categories.is_empty() {
            50.0
        } else {
            let with_best = stats
                .categories
                .iter()
                .filter(|c| c.best_agent.is_some())
                .count();
            (with_best as f64 / stats.categories.len() as f64 * 100.0).clamp(0.0, 100.0)
        };

        // Response latency: average across all operations (lower = better)
        let report = self.performance.report();
        let response_latency = if report.operations.is_empty() {
            50.0
        } else {
            let avg: f64 = report.operations.iter().map(|o| o.avg_ms).sum::<f64>()
                / report.operations.len() as f64;
            avg.min(100.0)
        };

        // Security accuracy
        let security_accuracy = self.security.overall_accuracy() * 100.0;

        // User satisfaction: inferred from agent scores and UI patterns
        let user_satisfaction = if stats.total_observations == 0 {
            50.0
        } else {
            agent_quality // Proxy for now
        };

        // Knowledge depth
        let knowledge_depth = self.knowledge.knowledge_depth();

        // Uptime stability: no crashes tracked = 100
        let uptime_stability = 100.0;

        // Evolution success rate from dream cycle
        let evolution_success_rate = self.dreams.success_rate(&OSDreamType::AgentEvolution) * 100.0;

        OSFitness::new(
            agent_quality,
            routing_accuracy,
            response_latency,
            security_accuracy,
            user_satisfaction,
            knowledge_depth,
            uptime_stability,
            evolution_success_rate,
        )
    }

    /// Record a daily fitness snapshot.
    pub fn record_daily_fitness(&mut self) {
        let fitness = self.compute_fitness();
        self.fitness_history.record(fitness);
    }

    /// Get the morning OS briefing.
    pub fn morning_briefing(&self) -> OSBriefing {
        self.dreams.generate_briefing()
    }

    /// Whether the self-improvement system is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable the self-improvement system.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for SelfImprovingOS {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_initializes() {
        let os = SelfImprovingOS::new();
        assert!(os.is_enabled());
        assert!(os.improvement_log(10).is_empty());
    }

    #[test]
    fn computes_baseline_fitness() {
        let os = SelfImprovingOS::new();
        let fitness = os.compute_fitness();
        assert!(fitness.overall_score > 0.0);
        assert!(fitness.overall_score <= 100.0);
    }

    #[test]
    fn logs_improvements() {
        let mut os = SelfImprovingOS::new();
        os.log_improvement(ImprovementEvent {
            layer: "routing".to_string(),
            description: "Forge now handles code requests".to_string(),
            before_value: Some(5.0),
            after_value: Some(9.0),
            timestamp: 0,
        });
        assert_eq!(os.improvement_log(10).len(), 1);
    }

    #[test]
    fn improvement_log_bounded() {
        let mut os = SelfImprovingOS::new();
        os.max_log_entries = 5;
        for i in 0..10 {
            os.log_improvement(ImprovementEvent {
                layer: "test".to_string(),
                description: format!("improvement {i}"),
                before_value: None,
                after_value: None,
                timestamp: 0,
            });
        }
        assert_eq!(os.improvement_log.len(), 5);
    }

    #[test]
    fn fitness_improves_with_data() {
        let mut os = SelfImprovingOS::new();
        let baseline = os.compute_fitness();

        // Add routing data
        for _ in 0..5 {
            os.routing.record(RoutingOutcome {
                request_summary: "test".to_string(),
                request_category: "code".to_string(),
                agent_id: "forge".to_string(),
                score: 9.0,
                timestamp: 0,
            });
        }

        // Add knowledge
        os.knowledge.set_communication_style("concise");
        os.knowledge.record_interaction(InteractionSummary {
            topic: "rust".to_string(),
            languages_mentioned: vec!["rust".to_string()],
            score: 8.0,
            timestamp: 0,
        });

        let improved = os.compute_fitness();
        // Knowledge depth and routing should increase
        assert!(improved.knowledge_depth > baseline.knowledge_depth);
    }

    #[test]
    fn records_daily_fitness() {
        let mut os = SelfImprovingOS::new();
        os.record_daily_fitness();
        os.record_daily_fitness();
        assert_eq!(os.fitness_history.days_tracked(), 2);
    }

    #[test]
    fn morning_briefing_works() {
        let os = SelfImprovingOS::new();
        let briefing = os.morning_briefing();
        assert_eq!(briefing.total_improvements, 0);
    }

    #[test]
    fn enable_disable() {
        let mut os = SelfImprovingOS::new();
        assert!(os.is_enabled());
        os.set_enabled(false);
        assert!(!os.is_enabled());
    }
}
