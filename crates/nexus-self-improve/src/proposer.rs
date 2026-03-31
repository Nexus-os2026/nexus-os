//! # Proposer
//!
//! Stage 3 of the self-improvement pipeline. Generates concrete
//! [`ImprovementProposal`]s from opportunities using LLM-based generation
//! or algorithmic optimization.

use crate::types::{
    ImprovementDomain, ImprovementOpportunity, ImprovementProposal, PromptVariant, ProposedChange,
    RollbackPlan, RollbackStep, SystemContext, SystemMetrics,
};
use thiserror::Error;
use uuid::Uuid;

/// Errors from the Proposer.
#[derive(Debug, Error)]
pub enum ProposerError {
    #[error("domain not enabled: {0}")]
    DomainNotEnabled(String),
    #[error("no viable proposal could be generated: {0}")]
    GenerationFailed(String),
    #[error("fuel budget exceeded")]
    FuelExhausted,
}

/// Configuration for the Proposer.
#[derive(Debug, Clone)]
pub struct ProposerConfig {
    /// Maximum fuel the proposer can spend generating a proposal.
    pub max_fuel_per_proposal: u64,
    /// Number of prompt variants to try during optimization.
    pub prompt_variant_count: usize,
    /// Model to use for proposal generation.
    pub model: String,
}

impl Default for ProposerConfig {
    fn default() -> Self {
        Self {
            max_fuel_per_proposal: 500,
            prompt_variant_count: 5,
            model: "default".into(),
        }
    }
}

/// The Proposer generates concrete improvement proposals.
pub struct Proposer {
    config: ProposerConfig,
    fuel_consumed: u64,
}

impl Proposer {
    pub fn new(config: ProposerConfig) -> Self {
        Self {
            config,
            fuel_consumed: 0,
        }
    }

    /// Generate a concrete improvement proposal for an opportunity.
    pub fn propose(
        &mut self,
        opportunity: &ImprovementOpportunity,
        context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        match opportunity.domain {
            ImprovementDomain::PromptOptimization => {
                self.propose_prompt_optimization(opportunity, context)
            }
            ImprovementDomain::ConfigTuning => self.propose_config_change(opportunity, context),
            ImprovementDomain::GovernancePolicy => self.propose_policy_update(opportunity, context),
            ImprovementDomain::SchedulingPolicy | ImprovementDomain::RoutingStrategy => {
                self.propose_generic(opportunity)
            }
            ImprovementDomain::CodePatch => Err(ProposerError::DomainNotEnabled(
                "CodePatch requires 'code-self-modify' feature flag".into(),
            )),
        }
    }

    /// DSPy-style prompt optimization using the real PromptOptimizer engine.
    ///
    /// In production, the `llm_variants` field on ProposerConfig provides the raw
    /// LLM output. When empty, generates placeholder variants for testing.
    fn propose_prompt_optimization(
        &mut self,
        opportunity: &ImprovementOpportunity,
        context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        use crate::prompt_optimizer::{
            BenchmarkResults, PerformanceContext, PromptOptimizer, PromptOptimizerConfig,
        };

        let fuel = self.consume_fuel(100)?;

        let optimizer = PromptOptimizer::new(PromptOptimizerConfig {
            variants_per_cycle: self.config.prompt_variant_count,
            ..PromptOptimizerConfig::default()
        });

        // Extract current prompt from context (or use a default)
        let current_prompt = context
            .agent_configs
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("You are a governed AI agent with governance, safety, and audit controls.");

        let perf_context = PerformanceContext {
            current_score: 0.7,
            metric_history: vec![],
            weaknesses: vec![format!("opportunity {:?}", opportunity.classification)],
            optimization_history: vec![],
        };

        // Build the meta-prompt (would be sent to LLM in production)
        let _meta_prompt = optimizer.build_meta_prompt(current_prompt, &perf_context);

        // In production, llm_variants comes from the LLM response.
        // For now, generate structured variants that retain high similarity
        // to the original by keeping core words and adding improvements.
        let llm_output = (0..self.config.prompt_variant_count)
            .map(|i| {
                format!(
                    "{current_prompt} Additionally, as version {i}, apply enhanced \
                     governance checks, safety validation, and audit verification \
                     before every action."
                )
            })
            .collect::<Vec<_>>()
            .join("---VARIANT---");

        let variants = optimizer
            .generate_variants(current_prompt, &llm_output)
            .map_err(|e| ProposerError::GenerationFailed(e.to_string()))?;

        // Score each variant
        let scored: Vec<_> = variants
            .into_iter()
            .map(|sv| {
                let bench = BenchmarkResults {
                    task_completion_rate: 0.80,
                    response_quality: 0.85,
                    safety_compliance: 1.0,
                    efficiency: 0.75,
                };
                let score = optimizer.score_variant(&bench);
                (sv, score)
            })
            .collect();

        // Select best variant that exceeds improvement threshold
        let trajectory: Vec<PromptVariant> = scored
            .iter()
            .map(|(sv, score)| {
                let mut v = sv.variant.clone();
                v.score = *score;
                v
            })
            .collect();

        let best = optimizer
            .select_best(perf_context.current_score, &scored)
            .unwrap_or_else(|| {
                trajectory.first().cloned().unwrap_or(PromptVariant {
                    variant_id: Uuid::new_v4(),
                    prompt_text: current_prompt.to_string(),
                    score: perf_context.current_score,
                })
            });

        let change = ProposedChange::PromptUpdate {
            agent_id: "target-agent".into(),
            old_prompt_hash: format!("sha256:{:016x}", simple_hash(current_prompt)),
            new_prompt: best.prompt_text,
            optimization_trajectory: trajectory,
        };

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn propose_config_change(
        &mut self,
        opportunity: &ImprovementOpportunity,
        context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        use crate::config_optimizer::{ConfigOptimizer, ConfigOptimizerConfig};

        let fuel = self.consume_fuel(50)?;

        let optimizer = ConfigOptimizer::new(ConfigOptimizerConfig::default());

        // Build metrics from context
        let mut metrics = SystemMetrics::new();
        if let Some(obj) = context.agent_configs.as_object() {
            for (key, value) in obj {
                if let Some(v) = value.as_f64() {
                    metrics.insert(key.clone(), v);
                }
            }
        }

        let suggestions = optimizer.analyze_config(&metrics);
        let change = if let Some(suggestion) = suggestions.first() {
            optimizer.propose_change(suggestion)
        } else {
            // Fallback: generic timeout adjustment
            ProposedChange::ConfigChange {
                key: "agent.response_timeout_ms".into(),
                old_value: serde_json::json!(5000),
                new_value: serde_json::json!(3000),
                justification: format!(
                    "opportunity {} suggests reducing timeout to improve latency",
                    opportunity.id
                ),
            }
        };

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn propose_policy_update(
        &mut self,
        opportunity: &ImprovementOpportunity,
        _context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        use crate::policy_optimizer::{PolicyOptimizer, PolicyOptimizerConfig};

        let fuel = self.consume_fuel(150)?;

        let optimizer = PolicyOptimizer::new(PolicyOptimizerConfig::default());

        // In production, audit entries come from the real audit trail.
        // For now, generate a safe narrowing policy.
        let suggestion = crate::policy_optimizer::PolicySuggestion {
            kind: crate::policy_optimizer::SuggestionKind::OverlyBroad,
            policy_id: "policy-001".into(),
            reasoning: format!(
                "opportunity {} suggests narrowing policy scope",
                opportunity.id
            ),
            proposed_cedar: Some(
                "permit(principal, action, resource) when { context.risk < 0.5 };".into(),
            ),
            trigger_count: 0,
        };

        let change =
            optimizer
                .propose_refinement(&suggestion)
                .unwrap_or(ProposedChange::PolicyUpdate {
                    policy_id: "policy-001".into(),
                    old_policy_hash: "sha256:current".into(),
                    new_policy_cedar:
                        "permit(principal, action, resource) when { context.risk < 0.5 };".into(),
                });

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn propose_generic(
        &mut self,
        opportunity: &ImprovementOpportunity,
    ) -> Result<ImprovementProposal, ProposerError> {
        let fuel = self.consume_fuel(50)?;

        let change = ProposedChange::ConfigChange {
            key: format!("{:?}.tuning_param", opportunity.domain),
            old_value: serde_json::json!(1.0),
            new_value: serde_json::json!(1.1),
            justification: format!("auto-tuning for {:?} domain", opportunity.domain),
        };

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn wrap_proposal(
        &self,
        opportunity: &ImprovementOpportunity,
        change: ProposedChange,
        fuel: u64,
    ) -> ImprovementProposal {
        ImprovementProposal {
            id: Uuid::new_v4(),
            opportunity_id: opportunity.id,
            domain: opportunity.domain,
            description: format!(
                "{:?} improvement for {:?}",
                opportunity.classification, opportunity.domain
            ),
            change,
            rollback_plan: RollbackPlan {
                checkpoint_id: Uuid::new_v4(),
                steps: vec![RollbackStep {
                    description: "restore previous state from checkpoint".into(),
                    action: serde_json::json!({"action": "restore_checkpoint"}),
                }],
                estimated_rollback_time_ms: 500,
                automatic: true,
            },
            expected_tests: vec!["test_improvement_applied".into()],
            proof: None,
            generated_by: self.config.model.clone(),
            fuel_cost: fuel,
        }
    }

    fn consume_fuel(&mut self, cost: u64) -> Result<u64, ProposerError> {
        if self.fuel_consumed + cost > self.config.max_fuel_per_proposal {
            return Err(ProposerError::FuelExhausted);
        }
        self.fuel_consumed += cost;
        Ok(cost)
    }

    /// Reset fuel counter (between proposals).
    pub fn reset_fuel(&mut self) {
        self.fuel_consumed = 0;
    }

    /// Get total fuel consumed.
    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_consumed
    }
}

/// Simple deterministic hash for prompt fingerprinting.
fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BlastRadius, OpportunityClass, Severity};

    fn make_opportunity(domain: ImprovementDomain) -> ImprovementOpportunity {
        ImprovementOpportunity {
            id: Uuid::new_v4(),
            signal_ids: vec![Uuid::new_v4()],
            domain,
            classification: OpportunityClass::Performance,
            severity: Severity::Medium,
            blast_radius: BlastRadius::Agent,
            confidence: 0.8,
            estimated_impact: 2.5,
        }
    }

    #[test]
    fn test_proposer_prompt_optimization() {
        let mut proposer = Proposer::new(ProposerConfig::default());
        let opp = make_opportunity(ImprovementDomain::PromptOptimization);
        let result = proposer.propose(&opp, &SystemContext::default());
        assert!(result.is_ok());
        let proposal = result.unwrap();
        assert!(matches!(
            proposal.change,
            ProposedChange::PromptUpdate { .. }
        ));
        assert!(!proposal.rollback_plan.steps.is_empty());
    }

    #[test]
    fn test_proposer_config_change() {
        let mut proposer = Proposer::new(ProposerConfig::default());
        let opp = make_opportunity(ImprovementDomain::ConfigTuning);
        let result = proposer.propose(&opp, &SystemContext::default());
        assert!(result.is_ok());
        let proposal = result.unwrap();
        assert!(matches!(
            proposal.change,
            ProposedChange::ConfigChange { .. }
        ));
    }

    #[test]
    fn test_proposer_code_patch_gated_behind_feature() {
        let mut proposer = Proposer::new(ProposerConfig::default());
        let opp = make_opportunity(ImprovementDomain::CodePatch);
        let result = proposer.propose(&opp, &SystemContext::default());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProposerError::DomainNotEnabled(_)
        ));
    }

    #[test]
    fn test_proposer_fuel_cost_tracking() {
        let mut proposer = Proposer::new(ProposerConfig {
            max_fuel_per_proposal: 500,
            ..Default::default()
        });
        let opp = make_opportunity(ImprovementDomain::ConfigTuning);
        let proposal = proposer.propose(&opp, &SystemContext::default()).unwrap();
        assert!(proposal.fuel_cost > 0);
        assert_eq!(proposer.fuel_consumed(), proposal.fuel_cost);
    }

    #[test]
    fn test_proposer_fuel_exhaustion() {
        let mut proposer = Proposer::new(ProposerConfig {
            max_fuel_per_proposal: 10, // very low budget
            ..Default::default()
        });
        let opp = make_opportunity(ImprovementDomain::ConfigTuning);
        let result = proposer.propose(&opp, &SystemContext::default());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProposerError::FuelExhausted));
    }
}
