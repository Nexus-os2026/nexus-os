//! Targeted rule mutation for each missed attack technique.

use nexus_governance_engine::rules::{GovernanceRule, RuleCondition, RuleEffect};

use crate::synthetic_attacks::AttackTechnique;

/// Generate a governance rule that closes the gap exposed by a missed attack.
pub fn mutate_rule_for_technique(technique: &AttackTechnique) -> GovernanceRule {
    match technique {
        AttackTechnique::BudgetCreep => GovernanceRule {
            id: format!("evolved-budget-check-{}", uuid::Uuid::new_v4()),
            description: "Deny requests when budget is insufficient".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::MinBudgetRemaining {
                capability: "llm.query".into(),
                minimum: 10,
            }],
        },
        AttackTechnique::PrivilegeEscalationViaChild => GovernanceRule {
            id: format!("evolved-escalation-block-{}", uuid::Uuid::new_v4()),
            description: "Deny agent creation above parent autonomy level".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::MaxAutonomyLevel(4)],
        },
        AttackTechnique::LineageDepthCreep => GovernanceRule {
            id: format!("evolved-depth-limit-{}", uuid::Uuid::new_v4()),
            description: "Deny agent creation beyond depth 5".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::MaxLineageDepth(5)],
        },
        AttackTechnique::CapabilityMasquerading => GovernanceRule {
            id: format!("evolved-cap-mask-{}", uuid::Uuid::new_v4()),
            description: "Deny capabilities not in explicit allowlist".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::CapabilityNotInSet(vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
                "web.search".into(),
            ])],
        },
        _ => GovernanceRule {
            id: format!("evolved-generic-{}", uuid::Uuid::new_v4()),
            description: "Generic deny rule from evolution".into(),
            effect: RuleEffect::Deny,
            conditions: vec![RuleCondition::CapabilityNotInSet(vec![
                "llm.query".into(),
                "fs.read".into(),
            ])],
        },
    }
}
