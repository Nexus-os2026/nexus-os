//! Registered before the session/cell implementation. No eligibility bypass.
use super::*;
use crate::authority::file_session::{FileAttachment, HaltCause, HaltCell};

fn tagged() -> (Fx, FileAttachment, HaltCell) {
    let mut f = Fx::new();
    let port = f.door.file_port().unwrap();
    let attachment = port.attach().unwrap();
    let cell = f.door.claim_file_session(&attachment).unwrap();
    f.run = f.door.open_file_run(&attachment).unwrap();
    f.door.admit_files(f.run, HASH, LABEL).unwrap();
    (f, attachment, cell)
}

#[test]
fn lite_e_fresh_session_really_publishes() {
    let (f, _, cell) = tagged();
    f.create(NAME, CONTENT).unwrap();
    assert_eq!(std::fs::read_to_string(f.ws.join(NAME)).unwrap(), CONTENT);
    assert!(!cell.is_engaged());
}

#[test]
fn lite_e_replacement_port_and_coordinator_never_reset_ownership() {
    for rebuilt in [false, true] {
        let f = Fx::new();
        let port = f.door.file_port().unwrap();
        let attached = port.attach().unwrap();
        let cell = f.door.claim_file_session(&attached).unwrap();
        drop(attached);
        if rebuilt {
            drop(port);
            assert!(f.door.file_port().is_err(), "rebuilt port must refuse");
        } else {
            assert!(port.attach().is_err(), "second coordinator must refuse");
        }
        assert_eq!(cell.cause(), Some(HaltCause::Fence));
    }
}

#[test]
fn lite_e_history_before_selection_and_emergency_lift_refuse() {
    for emergency in [false, true] {
        let f = Fx::new();
        let port = f.door.file_port().unwrap();
        let attached = port.attach().unwrap();
        if emergency {
            f.gov.emergency_stop();
            f.gov.resume(&*f.owner).unwrap();
            assert!(!f.door.emergency_stopped());
        } else {
            let another = port.attach().unwrap();
            drop(another);
        }
        assert!(f.door.claim_file_session(&attached).is_err());
    }
}

#[test]
fn lite_h_captured_token_reads_original_cell_after_registry_poison() {
    let (f, _, cell) = tagged();
    let token = f.gov.authority().runs().check(f.run, &mother()).unwrap();
    assert!(!token.is_cancelled());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.gov.authority().runs().panic_holding_the_runs_lock();
    }))
    .is_err());
    cell.engage_with(HaltCause::TimeBudget);
    assert!(
        token.is_cancelled(),
        "captured token must read the attached cell"
    );
    cell.engage_with(HaltCause::Stop);
    assert_eq!(cell.cause(), Some(HaltCause::TimeBudget));
}

#[test]
fn lite_h_final_check_after_engagement_refuses_real_publication() {
    let (f, _, cell) = tagged();
    let view = f.ready(NAME, CONTENT);
    let observed = cell.clone();
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_BEFORE_FINAL, move || cell.engage());
    let _ = f.execute(&view);
    assert!(
        observed.is_engaged(),
        "the Files final-check hook must have fired"
    );
    assert!(
        !f.ws.join(NAME).exists(),
        "the final check must read the File cell"
    );
    assert!(matches!(
        f.door.publication(view.id).unwrap(),
        super::super::door::Lookup::Mine(
            crate::authority::commitment::PublicationClass::ConfirmedNotPublished
        )
    ));
}

#[test]
fn lite_h_emergency_ingress_engages_without_mother_and_keeps_first_cause() {
    for first in [None, Some(HaltCause::TimeBudget)] {
        let (f, _, cell) = tagged();
        if let Some(cause) = first {
            cell.engage_with(cause);
        }
        let observed = cell.clone();
        *f.gov.stop_gap.lock().unwrap() = Some(Box::new(move |_| {
            assert!(
                observed.is_engaged(),
                "cell engages before broker Stop work"
            );
        }));
        f.gov.emergency_stop();
        f.gov.resume(&*f.owner).unwrap();
        assert_eq!(cell.cause(), Some(first.unwrap_or(HaltCause::Emergency)));
        assert!(f.door.open_run().is_err());
    }
}

#[test]
fn lite_o_tagged_run_cannot_grow_members_or_promote_legacy_run() {
    let (f, attached, _) = tagged();
    let view = f.propose(NAME, CONTENT).unwrap();
    let other = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    assert!(
        other.adopt(f.run, view.id).is_err(),
        "even a Prepared File run cannot grow"
    );
    assert!(
        f.door.open_run().is_err(),
        "selected File door cannot open an untagged run"
    );
    let legacy = other.open_run().unwrap();
    assert!(f.door.admit_files(legacy, HASH, LABEL).is_err());
    assert!(
        other.open_file_run(&attached).is_err(),
        "attachment is not transferable"
    );
}

#[test]
fn lite_b_engaged_cell_refuses_all_shared_admissions_without_attempts() {
    let (f, _, cell) = tagged();
    let runs = f.gov.authority().runs();
    let group = f.door.group_for_test();
    // Positive control: valid cached consent and a real inference binding.
    runs.admit_inference(f.run, &mother(), group, HASH, LABEL, [42; 32])
        .unwrap();
    runs.admit_model_call(f.run, &mother(), group, &[42; 32])
        .unwrap();
    cell.engage();
    assert!(f.propose(NAME, CONTENT).is_err());
    assert!(runs.check(f.run, &mother()).is_err());
    assert!(f.door.admit_files(f.run, HASH, LABEL).is_err());
    assert!(runs
        .admit_inference(f.run, &mother(), group, HASH, LABEL, [42; 32])
        .is_err());
    assert!(runs
        .admit_model_call(f.run, &mother(), group, &[42; 32])
        .is_err());
}

#[test]
fn lite_b_consent_fill_checks_cell_after_dialog() {
    let (f, _, cell) = tagged();
    let before = f.door.consents_cached();
    f.gov
        .hooks()
        .arm(at::CONSENT_BEFORE_FILL, move || cell.engage());
    assert!(f
        .door
        .confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: "msn-000002/1/1".into(),
            summary: vec!["A second yes".into()],
            page_facts: None,
        })
        .is_err());
    assert_eq!(
        f.door.consents_cached(),
        before,
        "engaged cell must not cache a yes"
    );
}

#[test]
fn lite_h_cell_is_attached_before_opened_run_is_exposed() {
    let f = Fx::new();
    let port = f.door.file_port().unwrap();
    let attached = port.attach().unwrap();
    let cell = f.door.claim_file_session(&attached).unwrap();
    f.gov.hooks().arm(at::OPEN_INSERTED, move || cell.engage());
    assert!(f.door.open_file_run(&attached).is_err());
}
