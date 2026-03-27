//! Synthetic attack technique generators.

use nexus_governance_oracle::CapabilityRequest;
use serde::{Deserialize, Serialize};

use crate::threat_model::ThreatModel;

/// A synthetic attack — a fake request designed to test governance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticAttack {
    pub request: CapabilityRequest,
    pub expected_decision: ExpectedDecision,
    pub technique: AttackTechnique,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedDecision {
    ShouldDeny,
    ShouldAllow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttackTechnique {
    BudgetCreep,
    SalamiSlicing,
    PrivilegeEscalationViaChild,
    CapabilityMasquerading,
    RaceCondition,
    LineageDepthCreep,
    TimingExploit,
    Absorbed(String),
}

/// Trait for attack generators.
pub trait AttackGenerator: Send {
    fn generate(&self, threat_model: &ThreatModel) -> Vec<SyntheticAttack>;
    fn name(&self) -> &str;
}

/// Budget creep: request slightly above budget.
pub struct BudgetCreepGenerator;

impl AttackGenerator for BudgetCreepGenerator {
    fn name(&self) -> &str {
        "BudgetCreep"
    }

    fn generate(&self, _threat_model: &ThreatModel) -> Vec<SyntheticAttack> {
        vec![
            SyntheticAttack {
                request: CapabilityRequest {
                    agent_id: "synthetic-agent-bc".into(),
                    capability: "llm.query".into(),
                    parameters: serde_json::json!({
                        "budget_remaining": {"llm.query": 5},
                        "requested_tokens": 100
                    }),
                    budget_hash: String::new(),
                    request_nonce: uuid::Uuid::new_v4().to_string(),
                },
                expected_decision: ExpectedDecision::ShouldDeny,
                technique: AttackTechnique::BudgetCreep,
                description: "Request far exceeding remaining budget".into(),
            },
            SyntheticAttack {
                request: CapabilityRequest {
                    agent_id: "synthetic-agent-bc-ok".into(),
                    capability: "llm.query".into(),
                    parameters: serde_json::json!({
                        "budget_remaining": {"llm.query": 500},
                        "autonomy_level": 2
                    }),
                    budget_hash: String::new(),
                    request_nonce: uuid::Uuid::new_v4().to_string(),
                },
                expected_decision: ExpectedDecision::ShouldAllow,
                technique: AttackTechnique::BudgetCreep,
                description: "Legitimate request with sufficient budget".into(),
            },
        ]
    }
}

/// Privilege escalation via child: create child with higher autonomy.
pub struct PrivilegeEscalationGenerator;

impl AttackGenerator for PrivilegeEscalationGenerator {
    fn name(&self) -> &str {
        "PrivilegeEscalationViaChild"
    }

    fn generate(&self, _threat_model: &ThreatModel) -> Vec<SyntheticAttack> {
        vec![SyntheticAttack {
            request: CapabilityRequest {
                agent_id: "synthetic-parent-l3".into(),
                capability: "agent.create".into(),
                parameters: serde_json::json!({
                    "autonomy_level": 3,
                    "child_autonomy_level": 5,
                    "lineage_depth": 1
                }),
                budget_hash: String::new(),
                request_nonce: uuid::Uuid::new_v4().to_string(),
            },
            expected_decision: ExpectedDecision::ShouldDeny,
            technique: AttackTechnique::PrivilegeEscalationViaChild,
            description: "L3 parent tries to create L5 child".into(),
        }]
    }
}

/// Lineage depth creep: rapid generational spawning.
pub struct LineageDepthGenerator;

impl AttackGenerator for LineageDepthGenerator {
    fn name(&self) -> &str {
        "LineageDepthCreep"
    }

    fn generate(&self, _threat_model: &ThreatModel) -> Vec<SyntheticAttack> {
        vec![SyntheticAttack {
            request: CapabilityRequest {
                agent_id: "synthetic-deep".into(),
                capability: "agent.create".into(),
                parameters: serde_json::json!({
                    "autonomy_level": 4,
                    "lineage_depth": 8
                }),
                budget_hash: String::new(),
                request_nonce: uuid::Uuid::new_v4().to_string(),
            },
            expected_decision: ExpectedDecision::ShouldDeny,
            technique: AttackTechnique::LineageDepthCreep,
            description: "Agent at lineage depth 8 tries to create child".into(),
        }]
    }
}

/// Return the default set of attack generators.
pub fn default_attack_generators() -> Vec<Box<dyn AttackGenerator + Send>> {
    vec![
        Box::new(BudgetCreepGenerator),
        Box::new(PrivilegeEscalationGenerator),
        Box::new(LineageDepthGenerator),
    ]
}
