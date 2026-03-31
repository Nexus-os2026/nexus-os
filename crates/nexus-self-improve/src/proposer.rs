//! # Proposer
//!
//! Stage 3 of the self-improvement pipeline. Generates concrete
//! [`ImprovementProposal`]s from opportunities using LLM-based generation
//! or algorithmic optimization.

use crate::types::{
    ImprovementDomain, ImprovementOpportunity, ImprovementProposal, PromptVariant, ProposedChange,
    RollbackPlan, RollbackStep, SystemContext,
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

    /// DSPy-style prompt optimization: generate variants and select best.
    fn propose_prompt_optimization(
        &mut self,
        opportunity: &ImprovementOpportunity,
        _context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        let fuel = self.consume_fuel(100)?;

        // Generate prompt variants (in production, these come from an LLM)
        let variants: Vec<PromptVariant> = (0..self.config.prompt_variant_count)
            .map(|i| PromptVariant {
                variant_id: Uuid::new_v4(),
                prompt_text: format!("optimized_prompt_v{i}"),
                score: 0.0,
            })
            .collect();

        let best_variant = variants.first().cloned().unwrap_or(PromptVariant {
            variant_id: Uuid::new_v4(),
            prompt_text: "default".into(),
            score: 0.0,
        });

        let change = ProposedChange::PromptUpdate {
            agent_id: "target-agent".into(),
            old_prompt_hash: "sha256:old".into(),
            new_prompt: best_variant.prompt_text.clone(),
            optimization_trajectory: variants,
        };

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn propose_config_change(
        &mut self,
        opportunity: &ImprovementOpportunity,
        _context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        let fuel = self.consume_fuel(50)?;

        let change = ProposedChange::ConfigChange {
            key: "agent.response_timeout_ms".into(),
            old_value: serde_json::json!(5000),
            new_value: serde_json::json!(3000),
            justification: format!(
                "opportunity {} suggests reducing timeout to improve latency",
                opportunity.id
            ),
        };

        Ok(self.wrap_proposal(opportunity, change, fuel))
    }

    fn propose_policy_update(
        &mut self,
        opportunity: &ImprovementOpportunity,
        _context: &SystemContext,
    ) -> Result<ImprovementProposal, ProposerError> {
        let fuel = self.consume_fuel(150)?;

        let change = ProposedChange::PolicyUpdate {
            policy_id: "policy-001".into(),
            old_policy_hash: "sha256:old_policy".into(),
            new_policy_cedar: "permit(principal, action, resource) when { context.risk < 0.5 };"
                .into(),
        };

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
