//! The adversarial evolution loop — generates synthetic attacks, tests against
//! the governance ruleset, evolves rules on misses.

use serde::{Deserialize, Serialize};

use nexus_governance_engine::engine::DecisionEngine;
use nexus_governance_engine::rules::{GovernanceRule, GovernanceRuleset};
use nexus_governance_oracle::GovernanceDecision;

use crate::rule_mutation::mutate_rule_for_technique;
use crate::synthetic_attacks::{AttackGenerator, ExpectedDecision, SyntheticAttack};
use crate::threat_model::{KnownTechnique, TechniqueSource, ThreatModel};

/// A single evolution cycle record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCycle {
    pub cycle_id: String,
    pub timestamp: u64,
    pub attacks_generated: usize,
    pub attacks_caught: usize,
    pub attacks_missed: usize,
    pub rules_evolved: bool,
    pub new_ruleset_version: Option<String>,
    pub threats_absorbed: Vec<String>,
}

/// The governance evolution engine.
pub struct GovernanceEvolution {
    threat_model: ThreatModel,
    attack_generators: Vec<Box<dyn AttackGenerator + Send>>,
    evolution_history: Vec<EvolutionCycle>,
}

impl GovernanceEvolution {
    pub fn new(
        threat_model: ThreatModel,
        attack_generators: Vec<Box<dyn AttackGenerator + Send>>,
    ) -> Self {
        Self {
            threat_model,
            attack_generators,
            evolution_history: Vec::new(),
        }
    }

    /// Run one evolution cycle: generate attacks, test against ruleset copy, evolve.
    pub fn run_cycle(
        &mut self,
        engine: &DecisionEngine,
        current_ruleset: &GovernanceRuleset,
    ) -> EvolutionCycle {
        // Generate attacks from all generators
        let mut all_attacks: Vec<SyntheticAttack> = Vec::new();
        for gen in &self.attack_generators {
            all_attacks.extend(gen.generate(&self.threat_model));
        }

        let mut caught = 0;
        let mut missed = 0;
        let mut missed_techniques = Vec::new();

        // Test each attack against the current ruleset (via engine.evaluate_request)
        for attack in &all_attacks {
            let decision = engine.evaluate_request(&attack.request, current_ruleset);

            let was_caught = matches!(
                (&attack.expected_decision, &decision),
                (ExpectedDecision::ShouldDeny, GovernanceDecision::Denied)
                    | (
                        ExpectedDecision::ShouldAllow,
                        GovernanceDecision::Approved { .. }
                    )
            );

            let technique_id = format!("{:?}", attack.technique);
            self.threat_model.record_attempt(&technique_id, was_caught);

            if was_caught {
                caught += 1;
            } else {
                missed += 1;
                missed_techniques.push(attack.clone());
            }
        }

        // Evolve rules for missed attacks
        let rules_evolved = !missed_techniques.is_empty();
        let mut new_version = None;
        let mut absorbed = Vec::new();

        if rules_evolved {
            let mut new_rules: Vec<GovernanceRule> = current_ruleset.rules.clone();
            for missed_attack in &missed_techniques {
                let new_rule = mutate_rule_for_technique(&missed_attack.technique);
                new_rules.push(new_rule);

                // Absorb technique into threat model
                let tech_name = format!("{:?}", missed_attack.technique);
                self.threat_model.absorb_technique(KnownTechnique {
                    id: tech_name.clone(),
                    name: missed_attack.description.clone(),
                    description: missed_attack.description.clone(),
                    source: TechniqueSource::AbsorbedFromEvolution,
                    times_attempted: 1,
                    times_caught: 0,
                });
                absorbed.push(tech_name);
            }

            let evolved_ruleset = GovernanceRuleset::new(
                current_ruleset.id.clone(),
                current_ruleset.version + 1,
                new_rules,
            );
            new_version = Some(evolved_ruleset.version_hash());
        }

        let cycle = EvolutionCycle {
            cycle_id: uuid::Uuid::new_v4().to_string(),
            timestamp: epoch_secs(),
            attacks_generated: all_attacks.len(),
            attacks_caught: caught,
            attacks_missed: missed,
            rules_evolved,
            new_ruleset_version: new_version,
            threats_absorbed: absorbed,
        };

        self.evolution_history.push(cycle.clone());
        cycle
    }

    pub fn history(&self) -> &[EvolutionCycle] {
        &self.evolution_history
    }

    pub fn threat_model(&self) -> &ThreatModel {
        &self.threat_model
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic_attacks::default_attack_generators;
    use nexus_governance_engine::rules::{GovernanceRule, RuleCondition, RuleEffect};
    use nexus_governance_oracle::OracleRequest;

    fn test_ruleset() -> GovernanceRuleset {
        GovernanceRuleset::new(
            "test".into(),
            1,
            vec![
                // Allow basic LLM queries
                GovernanceRule {
                    id: "allow-llm".into(),
                    description: "Allow LLM".into(),
                    effect: RuleEffect::Allow,
                    conditions: vec![RuleCondition::CapabilityInSet(vec!["llm.query".into()])],
                },
                // Deny dangerous capabilities
                GovernanceRule {
                    id: "deny-dangerous".into(),
                    description: "Deny dangerous".into(),
                    effect: RuleEffect::Deny,
                    conditions: vec![RuleCondition::CapabilityInSet(vec![
                        "agent.create".into(),
                        "process.exec".into(),
                    ])],
                },
            ],
        )
    }

    #[test]
    fn test_evolution_cycle_catches_known_attack() {
        let (_, rx) = tokio::sync::mpsc::channel::<OracleRequest>(1);
        let engine = DecisionEngine::new(rx, test_ruleset());
        let mut evolution =
            GovernanceEvolution::new(ThreatModel::new(), default_attack_generators());

        let cycle = evolution.run_cycle(&engine, &test_ruleset());
        // The ruleset explicitly denies agent.create, so privilege escalation
        // and lineage depth attacks should be caught
        assert!(cycle.attacks_caught > 0, "Should catch at least one attack");
    }

    #[test]
    fn test_evolution_cycle_evolves_on_miss() {
        let (_, rx) = tokio::sync::mpsc::channel::<OracleRequest>(1);
        // Empty ruleset — everything defaults to deny, but ShouldAllow attacks will be missed
        let empty_ruleset = GovernanceRuleset::new("empty".into(), 1, vec![]);
        let engine = DecisionEngine::new(rx, empty_ruleset.clone());
        let mut evolution =
            GovernanceEvolution::new(ThreatModel::new(), default_attack_generators());

        let cycle = evolution.run_cycle(&engine, &empty_ruleset);
        // With empty ruleset, ShouldAllow attacks will be denied (false positive = miss)
        if cycle.attacks_missed > 0 {
            assert!(cycle.rules_evolved, "Should evolve rules on misses");
        }
    }

    #[test]
    fn test_child_cannot_exceed_parent_budget() {
        use nexus_governance_oracle::CapabilityBudget;
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();

        let mut allocs = std::collections::HashMap::new();
        allocs.insert("llm.query".into(), 1000u64);
        let parent = CapabilityBudget::new("parent".into(), allocs, &sk);

        let child = parent.derive_child("child".into(), 0.5, &sk).unwrap();
        assert!(child.allocations["llm.query"] <= parent.allocations["llm.query"]);
        assert!(child.verify(&vk).is_ok());

        // Full fraction
        let child_full = parent.derive_child("child-full".into(), 1.0, &sk).unwrap();
        assert!(child_full.allocations["llm.query"] <= parent.allocations["llm.query"]);
    }
}
