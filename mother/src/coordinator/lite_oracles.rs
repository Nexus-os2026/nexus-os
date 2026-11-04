//! Projection and replay semantics, with independent positive controls.
use super::*;
use crate::evidence::{PublicationCause, SettlementEnd};
use nexus_governed_control::authority::commitment::PublicationClass;

#[test]
fn lite_publication_projection_preserves_both_facts() {
    let got = lite_settle::outcome(PublicationClass::Published { recovery_required: true, interrupted: vec!["cancelled", "emergency_stop"] });
    assert_eq!(got, SettlementOutcome::Published { recovery_required: true, interrupted: vec![PublicationCause::Cancelled, PublicationCause::EmergencyStop] });
    assert_eq!(got.ordinary_end(), Some(SettlementEnd::Block(BlockReason::FilePublicationRecoveryRequired)));
    assert_eq!(lite_settle::outcome(PublicationClass::Pending), SettlementOutcome::Uncertain);
    assert_eq!(lite_settle::outcome(PublicationClass::Uncertain), SettlementOutcome::Uncertain);
}

#[test]
fn lite_replay_rejects_duplicate_action_and_conflicting_cause() {
    let contract = crate::oracles::contract();
    let reference = crate::oracles::contract_ref_in(&crate::oracles::test_private(), &contract);
    let id = MissionId(1);
    let action = PendingAction { run: "r".into(), commitment: "c".into(), deadline_mono_ms: 2 };
    let setup = || {
        let mut m = Mission::new(reference.clone(), Some(contract.clone()));
        m.file_mode = true;
        m.state = MissionState::AwaitingApproval;
        m.unresolved_file = Some(0);
        m.open_action = Some((0, action.clone()));
        BTreeMap::from([(id, m)])
    };
    let apply = |missions: &mut BTreeMap<_, _>, event| replay(missions, &mut 2, &mut Session::Open, &Entry::chained(0, crate::journal::GENESIS, id, event));
    let started = Event::SettlementStarted { effect: 0, cause: SettleCause::Stop(StopReason::Owner) };
    let mut missions = setup();
    apply(&mut missions, started.clone()).unwrap();
    assert!(apply(&mut missions, started.clone()).is_err(), "duplicate start");
    assert!(apply(&mut missions, Event::SettlementStarted { effect: 0, cause: SettleCause::TimeBudget }).is_err(), "changed first cause");
    let terminal = |action, end| Event::Transition { trigger: Trigger::Settled { effect: 0, action, outcome: SettlementOutcome::NotPublished, end } };
    assert!(apply(&mut missions, terminal(None, SettlementEnd::Stop(StopReason::Owner))).is_err(), "missing exact action");
    assert!(apply(&mut missions, terminal(Some(action.clone()), SettlementEnd::Block(BlockReason::BudgetTime))).is_err(), "terminal cannot replace first cause");
    apply(&mut missions, terminal(Some(action.clone()), SettlementEnd::Stop(StopReason::Owner))).unwrap();
    assert!(apply(&mut missions, terminal(Some(action), SettlementEnd::Stop(StopReason::Owner))).is_err(), "duplicate terminal");
    assert_eq!(missions[&id].acks.len(), 1);
}

#[test]
fn lite_file_input_checks_precede_port() {
    use nexus_governed_control::files::FileCreate;
    let mut contract = crate::oracles::contract();
    contract.requested_folder = Some("Workspace".into());
    let request = |name: &str, content: String| EffectRequest::CreateFile(FileCreate { workspace: "Workspace".into(), name: name.into(), content });
    for (effect, refusal) in [
        (request("../x.txt", "x".into()), MotherRefusal::FileRequestNotRelative),
        (request(".hidden", "x".into()), MotherRefusal::FileNameInvalid),
        (request("x.txt", "a\0b".into()), MotherRefusal::FileNotText),
        (request("x.txt", "x".repeat(8193)), MotherRefusal::FileTooLarge),
        (request("x.txt", "x\n".repeat(151)), MotherRefusal::FileTooManyLines),
    ] { assert_eq!(mother_refuses(&contract, &effect, Some("Workspace")), Some(refusal)); }
    assert_eq!(mother_refuses(&contract, &request("x.txt", "x".into()), Some("Workspace")), None);
    assert_eq!(mother_refuses(&contract, &request("x.txt", "x".into()), Some("Other")), Some(MotherRefusal::FileFolderNotWorkspace));
}
