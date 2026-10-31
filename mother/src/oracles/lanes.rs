//! Lanes: one per role, each call journaled with the lane's identity, an
//! invocation id and its input and output hashes; budgets bound them.

use super::*;
use crate::journal::Event;
use crate::lane::{output_hash, ScriptedLane};

#[test]
fn lanes_exactly_one_lane_per_role() {
    assert!(Lanes::new(keyed_lanes()).is_ok());
    // Missing a role.
    let mut missing = keyed_lanes();
    missing.pop();
    assert!(matches!(
        Lanes::new(missing),
        Err(CoordinatorError::Lanes(_))
    ));
    // A role twice.
    let mut twice = keyed_lanes();
    twice.push(Arc::new(KeyedLane::new(LaneRole::Builder)));
    assert!(matches!(Lanes::new(twice), Err(CoordinatorError::Lanes(_))));
    // A lane in another role's place: six lanes, but two Skeptics.
    let mut wrong: Vec<Arc<dyn Lane>> = keyed_lanes()
        .into_iter()
        .filter(|l| l.identity().role != LaneRole::Verifier)
        .collect();
    wrong.push(Arc::new(KeyedLane::new(LaneRole::Skeptic)));
    assert!(matches!(Lanes::new(wrong), Err(CoordinatorError::Lanes(_))));
}

#[test]
fn lanes_every_call_is_journaled_with_identity_invocation_and_hashes() {
    let w = world();
    let id = w.approved(contract());
    let mut reports = Vec::new();
    for _ in 0..5 {
        reports.extend(w.mother.advance(id).unwrap().lanes);
    }
    let roles: Vec<LaneRole> = reports.iter().map(|r| r.identity.role).collect();
    use LaneRole::*;
    assert_eq!(
        roles,
        vec![
            Sentinel,
            ResearcherA,
            ResearcherB,
            Skeptic,
            Sentinel,
            Builder,
            Sentinel,
            Verifier
        ]
    );
    let journal = w.mother.journal().unwrap();
    let mut invocations = BTreeSet::new();
    for report in &reports {
        assert!(
            invocations.insert(report.invocation),
            "unique invocation ids"
        );
        assert_eq!(report.invocation.mission, id);
        let invoked = journal
            .iter()
            .find(|e| {
                matches!(&e.event, Event::LaneInvoked { invocation, .. } if *invocation == report.invocation)
            })
            .expect("journaled invocation");
        assert_eq!(invoked.seq, report.invocation.seq, "the id names its entry");
        match &invoked.event {
            Event::LaneInvoked {
                identity,
                input_hash,
                ..
            } => {
                assert_eq!(identity, &report.identity);
                assert_eq!(input_hash, &report.input_hash);
                assert_eq!(input_hash.len(), 64);
            }
            _ => unreachable!(),
        }
        let returned = journal
            .iter()
            .find(|e| {
                matches!(&e.event, Event::LaneReturned { invocation, .. } if *invocation == report.invocation)
            })
            .expect("journaled return");
        assert!(returned.seq > invoked.seq);
        match &returned.event {
            Event::LaneReturned { output_hash, .. } => {
                assert_eq!(output_hash, &report.output_hash)
            }
            _ => unreachable!(),
        }
    }
    // Different calls see different inputs (the invocation and the prior
    // outputs are part of the input).
    let inputs: BTreeSet<&String> = reports.iter().map(|r| &r.input_hash).collect();
    assert_eq!(inputs.len(), reports.len());
}

#[test]
fn lanes_the_output_hash_is_the_hash_of_what_the_lane_answered() {
    let builder = ScriptedLane::new(LaneRole::Builder, "built the page", Verdict::Continue);
    let expected = output_hash(&builder.output);
    let mut lanes: Vec<Arc<dyn Lane>> = vec![Arc::new(builder)];
    lanes.extend(
        keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::Builder),
    );
    let w = world_with(lanes);
    let id = w.approved(contract());
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    let built = w.mother.advance(id).unwrap();
    let report = built
        .lanes
        .iter()
        .find(|r| r.identity.role == LaneRole::Builder)
        .unwrap();
    assert_eq!(report.output_hash, expected);
    assert_eq!(report.text, "built the page");
    // A different answer, a different hash.
    let other = ScriptedLane::new(LaneRole::Builder, "built the page!", Verdict::Continue);
    assert_ne!(output_hash(&other.output), expected);
}

#[test]
fn lanes_the_model_call_budget_blocks_the_mission() {
    let w = world();
    let mut c = contract();
    c.budget.max_model_calls = 3;
    let id = w.approved(c);
    w.mother.advance(id).unwrap();
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(a.lanes.len(), 3, "exactly the budget was spent");
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_model_calls"));
    assert_eq!(w.keyed_calls(), 3);
}

#[test]
fn lanes_the_model_call_budget_survives_a_restart() {
    let w = world();
    let mut c = contract();
    c.budget.max_model_calls = 5;
    let id = w.approved(c);
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap(); // four calls
    let w = w.restart();
    let a = w.mother.advance(id).unwrap(); // Sentinel is the fifth, Builder the sixth
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_model_calls"));
    assert_eq!(w.keyed_calls(), 5);
}

#[test]
fn lanes_the_time_budget_blocks_the_mission_before_any_lane() {
    let w = world();
    let id = w.approved(contract());
    w.mother.advance(id).unwrap();
    w.clock.advance(std::time::Duration::from_millis(
        contract().budget.max_duration_ms,
    ));
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(a.lanes, vec![]);
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_time"));
    assert_eq!(w.keyed_calls(), 0);
}

#[test]
fn lanes_the_time_budget_runs_from_consent_not_from_drafting() {
    let w = world();
    let id = w.mother.draft(contract()).unwrap();
    w.clock.advance(std::time::Duration::from_millis(
        contract().budget.max_duration_ms * 2,
    ));
    w.mother.submit(id).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    for _ in 0..5 {
        w.mother.advance(id).unwrap();
    }
    assert_eq!(w.mother.state(id), Some(MissionState::Done));
}

#[test]
fn lanes_verdicts_decide_the_outcome() {
    let w = world();
    let failed = w.mission_in(MissionState::Failed);
    assert_eq!(
        w.mother.reason(failed).as_deref(),
        Some("criterion p1 does not hold")
    );
    let halted = w.mission_in(MissionState::Blocked);
    assert_eq!(
        w.mother.reason(halted).as_deref(),
        Some("sentinel_halt:test halt")
    );
    // A Researcher's halt blocks too, and nothing after it runs.
    let id = w.approved(contract_with("HALT-RESEARCHERA ASK-RESEARCHERB-NETWORK"));
    w.mother.advance(id).unwrap();
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(a.lanes.len(), 2);
    assert_eq!(w.port.effects(), vec![]);
}

#[test]
fn lanes_a_lane_asking_for_too_many_effects_blocks_the_mission() {
    let w = world();
    let markers = "ASK-BUILDER-NETWORK ".repeat(crate::coordinator::MAX_EFFECTS_PER_CALL + 1);
    let id = w.approved(contract_with(&markers));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("too_many_effects"));
    assert_eq!(w.port.effects(), vec![]);
}
