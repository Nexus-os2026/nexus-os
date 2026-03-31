//! # Self-Improvement Pipeline
//!
//! Orchestrates the five stages: Observer → Analyzer → Proposer → Validator → Applier.

use crate::analyzer::{Analyzer, AnalyzerConfig};
use crate::applier::{Applier, ApplierConfig};
use crate::invariants::InvariantCheckState;
use crate::observer::{Observer, ObserverConfig};
use crate::proposer::{Proposer, ProposerConfig};
use crate::types::{CycleResult, SystemState};
use crate::validator::{Validator, ValidatorConfig};

/// Configuration for the full pipeline.
#[derive(Debug, Clone, Default)]
pub struct PipelineConfig {
    pub observer: ObserverConfig,
    pub analyzer: AnalyzerConfig,
    pub proposer: ProposerConfig,
    pub validator: ValidatorConfig,
    pub applier: ApplierConfig,
}

/// The full self-improvement pipeline.
pub struct SelfImprovementPipeline {
    observer: Observer,
    analyzer: Analyzer,
    proposer: Proposer,
    validator: Validator,
    applier: Applier,
}

impl SelfImprovementPipeline {
    pub fn new(config: PipelineConfig, validator: Validator, applier: Applier) -> Self {
        Self {
            observer: Observer::new(config.observer),
            analyzer: Analyzer::new(config.analyzer),
            proposer: Proposer::new(config.proposer),
            validator,
            applier,
        }
    }

    /// Run one improvement cycle.
    pub fn run_cycle(&mut self, state: &SystemState) -> CycleResult {
        // Stage 1: Observe
        let signals = self.observer.observe(&state.metrics);
        if signals.is_empty() {
            return CycleResult::NoSignals;
        }

        // Stage 2: Analyze
        let opportunities = self.analyzer.analyze(&signals);
        if opportunities.is_empty() {
            return CycleResult::NoOpportunities;
        }

        // Take the highest-impact opportunity
        let best = match opportunities.into_iter().max_by(|a, b| {
            a.estimated_impact
                .partial_cmp(&b.estimated_impact)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Some(o) => o,
            None => return CycleResult::NoOpportunities,
        };

        // Stage 3: Propose
        self.proposer.reset_fuel();
        let proposal = match self.proposer.propose(&best, &state.context) {
            Ok(p) => p,
            Err(e) => return CycleResult::ProposalFailed(e.to_string()),
        };

        // Stage 4: Validate (includes invariant checks + HITL consent)
        let invariant_state = InvariantCheckState {
            audit_chain_valid: state.audit_chain_valid,
            test_suite_passing: state.test_suite_passing,
            hitl_approved: true, // will be checked by validator's hitl_gate
            fuel_remaining: 10_000_u64.saturating_sub(proposal.fuel_cost),
            fuel_budget: 10_000,
        };

        let validated = match self.validator.validate(&proposal, &invariant_state) {
            Ok(v) => v,
            Err(e) => return CycleResult::ValidationFailed(e.to_string()),
        };

        // Stage 5: Apply (with checkpoint + canary)
        match self.applier.apply(&validated) {
            Ok(improvement) => CycleResult::Applied(improvement),
            Err(e) => CycleResult::ApplyFailed(e.to_string()),
        }
    }

    /// Access the observer for direct signal injection.
    pub fn observer_mut(&mut self) -> &mut Observer {
        &mut self.observer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CycleResult, SystemContext, SystemMetrics, SystemState};

    fn make_pipeline(hitl_approve: bool) -> SelfImprovementPipeline {
        let config = PipelineConfig::default();
        let validator = Validator::new(
            config.validator.clone(),
            Box::new(|| crate::validator::TestResults {
                passed: 100,
                failed: 0,
                failures: vec![],
            }),
            Box::new(|_| crate::validator::SimulationRiskResult {
                risk_score: 0.1,
                summary: "ok".into(),
            }),
            Box::new(move |_| {
                if hitl_approve {
                    Ok("ed25519:sig".into())
                } else {
                    Err("denied".into())
                }
            }),
        );
        let applier = Applier::new(
            config.applier.clone(),
            Box::new(|_| Ok(uuid::Uuid::new_v4())),
            Box::new(|| Ok(100)),
            Box::new(|_| {}),
        );
        SelfImprovementPipeline::new(config, validator, applier)
    }

    fn make_state() -> SystemState {
        SystemState {
            metrics: SystemMetrics::new(),
            context: SystemContext::default(),
            audit_chain_valid: true,
            test_suite_passing: true,
        }
    }

    #[test]
    fn test_pipeline_no_signals_returns_no_signals() {
        let mut pipeline = make_pipeline(true);
        let result = pipeline.run_cycle(&make_state());
        assert!(matches!(result, CycleResult::NoSignals));
    }

    #[test]
    fn test_pipeline_full_cycle_with_approval() {
        let mut pipeline = make_pipeline(true);

        // Build baseline first
        for _ in 0..25 {
            let mut state = make_state();
            state.metrics.insert("latency_p99", 100.0);
            pipeline.run_cycle(&state);
        }

        // Now inject a spike
        let mut state = make_state();
        state.metrics.insert("latency_p99", 500.0);
        let result = pipeline.run_cycle(&state);

        assert!(
            matches!(result, CycleResult::Applied(_)),
            "expected Applied, got: {result:?}"
        );
    }

    #[test]
    fn test_pipeline_validation_failure_on_hitl_denial() {
        let mut pipeline = make_pipeline(false);

        // Build baseline
        for _ in 0..25 {
            let mut state = make_state();
            state.metrics.insert("latency_p99", 100.0);
            pipeline.run_cycle(&state);
        }

        // Inject spike — should get denied at HITL
        let mut state = make_state();
        state.metrics.insert("latency_p99", 500.0);
        let result = pipeline.run_cycle(&state);

        assert!(
            matches!(result, CycleResult::ValidationFailed(_)),
            "expected ValidationFailed, got: {result:?}"
        );
    }
}
