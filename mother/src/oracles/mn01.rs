//! MN-01: zero file, network, process, model-tool or spend effects before
//! consent. The recording EffectPort proves the count is 0; a positive
//! control proves the same port sees every kind once consent is given.

use super::*;

/// A restatement whose every lane asks for every kind of effect.
fn asks_everything() -> String {
    let mut markers = String::new();
    for role in [
        "RESEARCHERA",
        "RESEARCHERB",
        "SKEPTIC",
        "BUILDER",
        "VERIFIER",
        "SENTINEL",
    ] {
        for kind in ["FILE", "NETWORK", "PROCESS", "MODELTOOL", "SPEND"] {
            markers.push_str(&format!("ASK-{role}-{kind} "));
        }
    }
    markers
}

/// Lane invocations, as journaled.
fn lane_calls(w: &World) -> usize {
    w.mother
        .journal()
        .unwrap()
        .iter()
        .filter(|e| matches!(e.event, crate::journal::Event::LaneInvoked { .. }))
        .count()
}

#[test]
fn mn01_no_effect_of_any_kind_before_consent() {
    let w = world();
    let c = contract_with(&asks_everything());

    // Draft: every call a caller can make, none of which may reach the port.
    let id = w.mother.draft(c.clone()).unwrap();
    let _ = w.mother.confirmation(id);
    assert!(w.mother.advance(id).is_err(), "no work from Draft");
    let _ = w.mother.revise(id, c.clone()).unwrap();
    assert!(w.mother.advance(id).is_err());

    // AwaitingConsent.
    w.mother.submit(id).unwrap();
    let _ = w.mother.confirmation(id).unwrap();
    for _ in 0..3 {
        assert!(w.mother.advance(id).is_err(), "no work before consent");
    }
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingConsent));
    assert_eq!(w.mother.consented_hash(id), None);

    // Restart before consent: still nothing.
    let w = w.restart();
    assert!(w.mother.advance(id).is_err());

    // A second mission, stopped before consent.
    let stopped = w.mother.draft(c.clone()).unwrap();
    w.mother.submit(stopped).unwrap();
    assert_eq!(w.mother.stop(stopped), Ok(MissionState::Stopped));
    assert!(w.mother.advance(stopped).is_err());

    assert_eq!(
        w.port.calls(),
        vec![],
        "the port was reached before consent"
    );
    assert_eq!(
        lane_calls(&w),
        0,
        "a lane invocation was journaled before consent"
    );
    assert_eq!(w.keyed_calls(), 0, "a lane ran before consent");
}

#[test]
fn mn01_positive_control_every_kind_reaches_the_port_after_consent() {
    // Same lanes, same recorder: once consented (and not forbidden, and not
    // spend, whose money budget is zero) each kind reaches the port.
    let w = world();
    let id = w.approved(contract_with(
        "ASK-BUILDER-FILE ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS ASK-BUILDER-MODELTOOL",
    ));
    for _ in 0..5 {
        w.mother.advance(id).unwrap();
    }
    assert_eq!(w.mother.state(id), Some(MissionState::Done));
    let kinds: BTreeSet<EffectKind> = w.port.effects().into_iter().map(|(_, k)| k).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            EffectKind::File,
            EffectKind::Network,
            EffectKind::Process,
            EffectKind::ModelTool
        ])
    );
}

#[test]
fn mn01_spend_never_reaches_the_port_because_money_is_zero() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-SPEND"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    let advance = w.mother.advance(id).unwrap();
    assert_eq!(advance.to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_money_zero"));
    assert_eq!(w.port.effects(), vec![]);
}

#[test]
fn mn01_a_forbidden_effect_never_reaches_the_port() {
    for kind in EffectKind::ALL {
        let w = world();
        let mut c = contract_with(&format!(
            "ASK-BUILDER-{}",
            kind.as_str().to_uppercase().replace('_', "")
        ));
        c.forbidden_effects.insert(kind);
        let id = w.approved(c);
        w.mother.advance(id).unwrap();
        w.mother.advance(id).unwrap();
        let advance = w.mother.advance(id).unwrap();
        assert_eq!(advance.to, MissionState::Blocked, "{kind:?}");
        assert_eq!(
            w.mother.reason(id),
            Some(format!("forbidden_effect:{}", kind.as_str())),
            "{kind:?}"
        );
        assert_eq!(w.port.effects(), vec![], "{kind:?}");
        // Blocked is terminal: nothing more runs.
        assert!(w.mother.advance(id).is_err());
        assert_eq!(w.port.effects(), vec![], "{kind:?}");
    }
}
