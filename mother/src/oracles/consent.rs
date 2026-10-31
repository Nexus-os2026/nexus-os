//! I1 no effect before consent; I2 consent binds the exact contract; I3
//! consent is not authority (the broker's refusals and approvals decide).

use super::fake::{commitment_id, Call, Reply};
use super::*;
use crate::broker::{Refusal, MOTHER_AGENT};
use crate::contract::EffectCategory;
use crate::coordinator::{CoordinatorError, MissionState, StepOutcome, StepReport};
use nexus_governed_control::authority::commitment::CommitmentState;
use std::collections::BTreeSet;

/// A contract with every category: network, file, model-tool and spend.
fn every_category() -> MissionContract {
    contract(vec![
        step(request("https://example.org/a")),
        step(tool("git")),
        step(browse("https://example.org/b")),
        Step {
            intent: request("https://api.example.org/paid"),
            declared_cost_minor: 25,
        },
    ])
}

#[test]
fn m1_i1_drafting_and_reading_a_mission_reach_no_broker() {
    let s = setup();
    let id = s.mother.draft(every_category()).unwrap();
    assert_eq!(s.mother.state(id), Some(MissionState::Drafted));
    assert!(s.mother.consent_request(id).is_ok());
    assert!(s.mother.digest(id).is_some());
    assert_eq!(s.mother.run_of(id), None);
    let _ = s.mother.journal();
    assert_eq!(s.broker.calls(), vec![]);
}

#[test]
fn m1_i1_no_step_of_any_category_runs_before_consent() {
    let cases: Vec<(&str, Step)> = vec![
        ("network", step(request("https://example.org/a"))),
        ("model_tool and file", step(tool("git"))),
        ("file (browser download)", step(browse("https://example.org/b"))),
        ("network (connector)", step(connector("github.issues.list"))),
        (
            "spend",
            Step {
                intent: request("https://api.example.org/paid"),
                declared_cost_minor: 1,
            },
        ),
    ];
    for (name, step) in cases {
        let s = setup();
        let id = s.mother.draft(contract(vec![step])).unwrap();
        assert_eq!(
            s.mother.step(id),
            Err(CoordinatorError::NotApproved(MissionState::Drafted)),
            "{name}"
        );
        assert_eq!(
            s.mother.refresh(id),
            Err(CoordinatorError::NotAwaiting(MissionState::Drafted)),
            "{name}"
        );
        assert_eq!(s.mother.state(id), Some(MissionState::Drafted), "{name}");
        assert_eq!(s.broker.calls(), vec![], "{name}: the broker was reached");
    }
}

#[test]
fn m1_i1_declined_consent_is_terminal_and_reaches_no_broker() {
    let s = setup();
    let id = s.mother.draft(every_category()).unwrap();
    assert_eq!(
        s.mother.request_consent(id, &No),
        Err(CoordinatorError::Declined)
    );
    assert_eq!(s.mother.state(id), Some(MissionState::Declined));
    assert_eq!(
        s.mother.step(id),
        Err(CoordinatorError::NotApproved(MissionState::Declined))
    );
    // Asking again cannot turn a no into a yes.
    assert_eq!(
        s.mother.request_consent(id, &Yes),
        Err(CoordinatorError::NotDrafted(MissionState::Declined))
    );
    assert_eq!(s.broker.calls(), vec![]);
}

#[test]
fn m1_i1_an_unknown_mission_reaches_no_broker() {
    let s = setup();
    let other = setup();
    // A mission id from another coordinator names nothing here.
    let foreign = other.mother.draft(one_step()).unwrap();
    other.mother.request_consent(foreign, &Yes).unwrap();
    assert_eq!(s.mother.state(foreign), None);
    assert_eq!(s.mother.step(foreign), Err(CoordinatorError::UnknownMission));
    assert_eq!(
        s.mother.request_consent(foreign, &Yes),
        Err(CoordinatorError::UnknownMission)
    );
    assert_eq!(s.mother.stop(foreign), Err(CoordinatorError::UnknownMission));
    assert_eq!(s.broker.calls(), vec![]);
}

#[test]
fn m1_i2_the_owner_is_shown_the_exact_contract() {
    let s = setup();
    let contract = every_category();
    let id = s.mother.draft(contract.clone()).unwrap();
    let owner = Recording::default();
    s.mother.request_consent(id, &owner).unwrap();
    let shown = owner.0.lock().unwrap().clone();
    assert_eq!(shown.len(), 1);
    let shown = &shown[0];
    assert_eq!(shown.mission, id);
    assert_eq!(shown.digest, contract.digest().unwrap());
    assert_eq!(Some(shown.digest), s.mother.digest(id));
    assert_eq!(shown.goal, contract.goal);
    assert_eq!(shown.consent_ttl_ms, contract.consent_ttl_ms);
    assert_eq!(shown.total_declared_cost_minor, 25);
    assert_eq!(
        shown.categories,
        BTreeSet::from([
            EffectCategory::File,
            EffectCategory::Network,
            EffectCategory::ModelTool,
            EffectCategory::Spend,
        ])
    );
    assert_eq!(shown.steps.len(), contract.steps.len());
    for (index, (summary, step)) in shown.steps.iter().zip(&contract.steps).enumerate() {
        assert_eq!(summary.index, index);
        assert_eq!(summary.detail, serde_json::to_string(&step.intent).unwrap());
        assert_eq!(summary.categories, step.categories().unwrap());
        assert_eq!(summary.declared_cost_minor, step.declared_cost_minor);
    }
    assert_eq!(
        shown.steps.iter().map(|s| s.domain).collect::<Vec<_>>(),
        vec!["request", "tool", "browse", "request"]
    );
    // What is shown before asking is what is asked.
    let mut again = s.mother.consent_request(id).unwrap();
    again.mission = shown.mission;
    assert_eq!(&again, shown);
}

#[test]
fn m1_i2_consented_steps_run_through_the_broker_exactly_in_order() {
    let s = setup();
    let contract = two_steps();
    let id = s.approved(contract.clone());
    assert_eq!(s.mother.state(id), Some(MissionState::Approved { next_step: 0 }));
    assert_eq!(s.broker.calls(), vec![], "consent alone reaches no broker");

    s.broker.reply(Reply::Done(Some("first".into())));
    assert_eq!(
        s.mother.step(id),
        Ok(StepReport {
            step: 0,
            outcome: StepOutcome::Done {
                text: Some("first".into())
            }
        })
    );
    assert_eq!(s.mother.state(id), Some(MissionState::Approved { next_step: 1 }));
    assert_eq!(s.mother.step(id).unwrap().step, 1);
    assert_eq!(s.mother.state(id), Some(MissionState::Completed));

    let run = s.mother.run_of(id).expect("a run was opened");
    let intent = |i: usize| serde_json::to_string(&contract.steps[i].intent).unwrap();
    assert_eq!(
        s.broker.calls(),
        vec![
            Call::OpenRun {
                agent: MOTHER_AGENT.into()
            },
            Call::Act {
                run,
                intent: intent(0)
            },
            Call::Act {
                run,
                intent: intent(1)
            },
            Call::Finish(run),
        ]
    );
    // Each step once: nothing more runs.
    assert_eq!(
        s.mother.step(id),
        Err(CoordinatorError::NotApproved(MissionState::Completed))
    );
    assert_eq!(s.broker.acts(), 2);
}

#[test]
fn m1_i2_consent_is_asked_once() {
    let s = setup();
    let id = s.approved(one_step());
    assert_eq!(
        s.mother.request_consent(id, &Yes),
        Err(CoordinatorError::NotDrafted(MissionState::Approved {
            next_step: 0
        }))
    );
}

#[test]
fn m1_i3_a_broker_refusal_fails_the_mission_and_ends_its_run() {
    let s = setup();
    let id = s.approved(two_steps());
    s.broker.reply(Reply::Refuse("no_covering_grant"));
    assert_eq!(
        s.mother.step(id),
        Err(CoordinatorError::Refused(Refusal {
            class: "no_covering_grant"
        }))
    );
    assert_eq!(
        s.mother.state(id),
        Some(MissionState::Failed {
            step: 0,
            class: "no_covering_grant"
        })
    );
    let run = s.mother.run_of(id).unwrap();
    assert!(s.broker.calls().contains(&Call::Cancel(run)));
    assert!(matches!(
        s.mother.step(id),
        Err(CoordinatorError::NotApproved(MissionState::Failed { .. }))
    ));
    assert_eq!(s.broker.acts(), 1);
}

#[test]
fn m1_i3_a_refused_run_opening_fails_the_mission() {
    let s = setup();
    let id = s.approved(one_step());
    s.broker.refuse_open("closed");
    assert_eq!(
        s.mother.step(id),
        Err(CoordinatorError::Refused(Refusal { class: "closed" }))
    );
    assert_eq!(
        s.mother.state(id),
        Some(MissionState::Failed {
            step: 0,
            class: "closed"
        })
    );
    assert_eq!(s.mother.run_of(id), None);
    assert_eq!(s.broker.acts(), 0);
}

#[test]
fn m1_i3_an_r2_step_waits_for_the_owner_s_native_approval() {
    let s = setup();
    let id = s.approved(two_steps());
    let c = commitment_id(7);
    s.broker.reply(Reply::Await(c));
    assert_eq!(
        s.mother.step(id),
        Ok(StepReport {
            step: 0,
            outcome: StepOutcome::AwaitingApproval { commitment: c }
        })
    );
    let waiting = MissionState::AwaitingApproval {
        step: 0,
        commitment: c,
    };
    assert_eq!(s.mother.state(id), Some(waiting));
    // Nothing moves on while the Owner has not answered.
    assert_eq!(s.mother.step(id), Err(CoordinatorError::NotApproved(waiting)));
    for state in [
        CommitmentState::Prepared,
        CommitmentState::Authorized,
        CommitmentState::Executing,
    ] {
        s.broker.set_commitment(c, state);
        assert_eq!(s.mother.refresh(id), Ok(waiting), "{state:?}");
    }
    s.broker.set_commitment(c, CommitmentState::Succeeded);
    assert_eq!(
        s.mother.refresh(id),
        Ok(MissionState::Approved { next_step: 1 })
    );
    assert_eq!(s.mother.step(id).unwrap().step, 1);
    assert_eq!(s.mother.state(id), Some(MissionState::Completed));
    assert_eq!(s.broker.acts(), 2);
}

#[test]
fn m1_i3_an_r2_step_that_does_not_succeed_fails_the_mission() {
    for (state, class) in [
        (Some(CommitmentState::Denied), "denied"),
        (Some(CommitmentState::Failed), "failed"),
        (Some(CommitmentState::Cancelled), "cancelled"),
        (Some(CommitmentState::Expired), "expired"),
        (Some(CommitmentState::Revoked), "revoked"),
        (None, "unknown_commitment"),
    ] {
        let s = setup();
        let id = s.approved(two_steps());
        let c = commitment_id(9);
        s.broker.reply(Reply::Await(c));
        s.mother.step(id).unwrap();
        if let Some(state) = state {
            s.broker.set_commitment(c, state);
        }
        let failed = MissionState::Failed { step: 0, class };
        assert_eq!(s.mother.refresh(id), Ok(failed), "{class}");
        assert_eq!(s.mother.state(id), Some(failed), "{class}");
        let run = s.mother.run_of(id).unwrap();
        assert!(s.broker.calls().contains(&Call::Cancel(run)), "{class}");
        assert_eq!(s.broker.acts(), 1, "{class}");
    }
}
