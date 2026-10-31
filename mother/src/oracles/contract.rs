//! I9 fail closed on what M1 cannot route; I2 the digest binds every field.

use super::*;
use crate::contract::{
    categories_of, domain_of, ContractError, EffectCategory, MAX_CONSENT_TTL_MS, MAX_GOAL_CHARS,
    MAX_STEPS,
};
use crate::coordinator::{CoordinatorError, MAX_MISSIONS};
use nexus_governed_control::display::{InputIntent, PerceptionIntent};
use nexus_governed_control::synthetic::SyntheticIntent;
use std::collections::BTreeSet;

use EffectCategory::{File, ModelTool, Network, Spend};

fn observe() -> Intent {
    Intent::Observe(PerceptionIntent::Screen { region: None })
}

fn input() -> Intent {
    Intent::Input(InputIntent::Move { x: 1, y: 1 })
}

fn synthetic() -> Intent {
    Intent::Synthetic(SyntheticIntent {})
}

#[test]
fn m1_i9_categories_come_from_the_intent_s_domain() {
    let set = |c: &[EffectCategory]| c.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(categories_of(&request("https://a.example")), Ok(set(&[Network])));
    assert_eq!(categories_of(&connector("x.y")), Ok(set(&[Network])));
    assert_eq!(
        categories_of(&browse("https://a.example")),
        Ok(set(&[Network, File]))
    );
    assert_eq!(categories_of(&tool("git")), Ok(set(&[ModelTool, File])));
    assert_eq!(categories_of(&observe()), Err("observe"));
    assert_eq!(categories_of(&input()), Err("input"));
    assert_eq!(categories_of(&synthetic()), Err("synthetic"));
    let paid = Step {
        intent: request("https://a.example"),
        declared_cost_minor: 5,
    };
    assert_eq!(paid.categories(), Ok(set(&[Network, Spend])));
    assert_eq!(step(tool("git")).categories(), Ok(set(&[ModelTool, File])));
    // The domain names are the broker's own serialisation tags.
    for intent in [
        request("https://a.example"),
        tool("git"),
        connector("x.y"),
        browse("https://a.example"),
        observe(),
        input(),
        synthetic(),
    ] {
        let json = serde_json::to_value(&intent).unwrap();
        assert_eq!(json["domain"], domain_of(&intent));
    }
}

#[test]
fn m1_i9_unroutable_domains_are_refused_when_drafted() {
    for (intent, domain) in [
        (observe(), "observe"),
        (input(), "input"),
        (synthetic(), "synthetic"),
    ] {
        let s = setup();
        let refused = s.mother.draft(contract(vec![step(tool("git")), step(intent)]));
        assert_eq!(
            refused,
            Err(CoordinatorError::Contract(ContractError::UnsupportedDomain {
                step: 1,
                domain
            }))
        );
        assert_eq!(s.mother.journal(), vec![]);
        assert_eq!(s.broker.calls(), vec![]);
    }
}

#[test]
fn m1_i9_every_contract_bound_is_checked() {
    let with_goal = |goal: &str| MissionContract {
        goal: goal.into(),
        ..one_step()
    };
    let refused = |c: MissionContract| {
        let s = setup();
        let result = s.mother.draft(c.clone());
        assert_eq!(s.broker.calls(), vec![]);
        assert_eq!(c.validate().err(), result.clone().err().map(|e| match e {
            CoordinatorError::Contract(e) => e,
            other => panic!("{other:?}"),
        }));
        result.err()
    };
    let contract_error = |e: ContractError| Some(CoordinatorError::Contract(e));

    assert_eq!(refused(with_goal("")), contract_error(ContractError::EmptyGoal));
    assert_eq!(refused(with_goal("   ")), contract_error(ContractError::EmptyGoal));
    assert_eq!(
        refused(with_goal(&"g".repeat(MAX_GOAL_CHARS + 1))),
        contract_error(ContractError::GoalTooLong)
    );
    assert_eq!(refused(with_goal(&"é".repeat(MAX_GOAL_CHARS))), None);
    for hidden in [
        "line\nbreak",
        "tab\there",
        "nul\0",
        "bidi \u{202E}evil",
        "isolate \u{2066}x",
        "zero\u{200B}width",
        "joiner\u{200D}",
        "bom\u{FEFF}",
        "nbsp\u{00A0}space",
        "ideographic\u{3000}space",
        "thin\u{2009}space",
        "del\u{7F}",
    ] {
        assert_eq!(
            refused(with_goal(hidden)),
            contract_error(ContractError::GoalNotPlainText),
            "{hidden:?}"
        );
    }
    assert_eq!(
        refused(contract(vec![])),
        contract_error(ContractError::NoSteps)
    );
    let many = |n: usize| contract((0..n).map(|_| step(tool("git"))).collect());
    assert_eq!(refused(many(MAX_STEPS)), None);
    assert_eq!(
        refused(many(MAX_STEPS + 1)),
        contract_error(ContractError::TooManySteps)
    );
    let ttl = |ms: u64| MissionContract {
        consent_ttl_ms: ms,
        ..one_step()
    };
    assert_eq!(refused(ttl(0)), contract_error(ContractError::ConsentTtlZero));
    assert_eq!(refused(ttl(MAX_CONSENT_TTL_MS)), None);
    assert_eq!(
        refused(ttl(MAX_CONSENT_TTL_MS + 1)),
        contract_error(ContractError::ConsentTtlTooLong)
    );
}

#[test]
fn m1_i2_the_digest_binds_every_field() {
    let base = two_steps();
    let digest = |c: &MissionContract| c.digest().unwrap();
    assert_eq!(digest(&base), digest(&base.clone()));
    let text = digest(&base).to_string();
    assert!(text.starts_with("sha256:"), "{text}");
    assert_eq!(text.len(), "sha256:".len() + 64);

    let mut variants = Vec::new();
    let mut c = base.clone();
    c.goal.push('!');
    variants.push(("goal", c));
    let mut c = base.clone();
    c.steps[1].intent = request("https://example.org/c");
    variants.push(("intent", c));
    let mut c = base.clone();
    c.steps[0].declared_cost_minor = 1;
    variants.push(("cost", c));
    let mut c = base.clone();
    c.consent_ttl_ms += 1;
    variants.push(("ttl", c));
    let mut c = base.clone();
    c.steps.swap(0, 1);
    variants.push(("order", c));
    let mut c = base.clone();
    c.steps.pop();
    variants.push(("length", c));
    let mut seen = BTreeSet::from([digest(&base).to_string()]);
    for (name, variant) in variants {
        assert!(
            seen.insert(digest(&variant).to_string()),
            "{name} did not change the digest"
        );
    }
}

#[test]
fn m1_i9_the_total_declared_cost_saturates() {
    let c = contract(vec![
        Step {
            intent: tool("git"),
            declared_cost_minor: u64::MAX,
        },
        Step {
            intent: tool("git"),
            declared_cost_minor: 3,
        },
    ]);
    assert_eq!(c.total_declared_cost_minor(), u64::MAX);
}

#[test]
fn m1_i9_the_coordinator_keeps_a_bounded_number_of_missions() {
    let s = setup();
    for _ in 0..MAX_MISSIONS {
        s.mother.draft(one_step()).unwrap();
    }
    assert_eq!(s.mother.draft(one_step()), Err(CoordinatorError::Capacity));
    assert_eq!(s.broker.calls(), vec![]);
}
