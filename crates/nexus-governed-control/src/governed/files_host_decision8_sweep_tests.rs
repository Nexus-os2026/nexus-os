//! Fresh Decision 8 sweep: approval is a separate admission from authorization;
//! every refusal after a recorded answer preserves a route-specific end record.
use super::*;

#[test]
fn decision8_approval_during_final_grant_validation() {
    let (f, _) = selected();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let cell = f.gov.file_session_halt();
    let registry = f.gov.authority().commitments().clone();
    registry
        .clone()
        .hooks()
        .arm(at::COMPLETE_BEFORE_CHECK, move || {
            registry.hooks().arm("decision7.grant_validation", move || {
                cell.engage_emergency()
            });
        });
    let result =
        f.gov
            .authority()
            .commitments()
            .request_approval(pending.id, &mother(), f.run, &*f.owner);
    no_worker(&f);
    assert!(
        matches!(result, Err(AuthorityError::Halted)),
        "no approval token issued after final validation ingress"
    );
    assert_refusal(&f, "approval_refused_halted");
}

#[test]
fn decision8_mission_consent_refusal_after_record_is_evidenced() {
    let (f, _) = selected();
    let before = f.door.consents_cached();
    let cell = f.gov.file_session_halt();
    f.gov
        .hooks()
        .arm(at::CONSENT_BEFORE_FILL, move || cell.engage_emergency());
    let result = f.door.confirm_mission(&MissionConfirmation {
        contract_hash: HASH.into(),
        request: "msn-000002/1/1".into(),
        summary: vec!["one file".into()],
        page_facts: None,
    });
    no_worker(&f);
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_eq!(f.door.consents_cached(), before);
    assert_refusal(&f, "mission_consent_refused_halted");
}

#[test]
fn decision8_held_consent_refusal_after_record_is_evidenced() {
    let (f, _) = selected();
    let before = f.door.consents_cached();
    let cell = f.gov.file_session_halt();
    f.gov
        .hooks()
        .arm(at::HELD_RECORDED, move || cell.engage_emergency());
    let result = f.door.confirm_held_mission(&MissionResumeConfirmation {
        mission: "msn-000001".into(),
        contract_hash: HASH.into(),
        request: "msn-000001/1/2".into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["one file".into()],
    });
    no_worker(&f);
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_eq!(f.door.consents_cached(), before);
    assert_refusal(&f, "held_consent_refused_halted");
}

#[test]
fn decision8_prepare_refusal_after_record_is_evidenced() {
    let (f, _) = selected();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::PREPARE_BEFORE_INSERT, move || cell.engage_emergency());
    assert!(matches!(
        f.propose(NAME, CONTENT),
        Err(AuthorityError::Halted)
    ));
    no_worker(&f);
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
    assert_refusal(&f, "commitment_prepare_refused_halted");
}

#[test]
fn decision8_authorization_refusal_after_record_is_evidenced() {
    let (f, _) = selected();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::AUTHORIZE_RECORDED, move || cell.engage_emergency());
    assert!(matches!(f.authorize(&pending), Err(AuthorityError::Halted)));
    no_worker(&f);
    assert_ne!(
        f.gov
            .authority()
            .commitments()
            .view(pending.id)
            .unwrap()
            .state,
        CommitmentState::Authorized
    );
    assert_refusal(&f, "authorization_refused_halted");
}

#[test]
fn decision8_adoption_after_view_before_membership() {
    // Replay can reach adoption before File selection. The source run is a
    // legacy run (no cell); only the adopting host door supplies the deny cell.
    let control = Fx::new();
    let pending = control.propose(NAME, CONTENT).unwrap();
    let live = control
        .gov
        .issue_agent_door(mother(), control.owner.clone())
        .unwrap();
    live.adopt(control.run, pending.id).unwrap();
    assert!(control
        .gov
        .authority()
        .runs()
        .try_is_member(control.run, live.group_for_test())
        .unwrap());
    let f = Fx::new();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let door = f
        .gov
        .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(f.gov.file_session_halt()))
        .unwrap();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm("decision8.adopt_view", move || cell.engage_emergency());
    let result = door.adopt(f.run, pending.id);
    no_worker(&f);
    assert!(
        !f.gov
            .authority()
            .runs()
            .try_is_member(f.run, door.group_for_test())
            .unwrap(),
        "no membership added after view-validation ingress"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
}

#[test]
fn decision9_adoption_refuses_instead_of_retrofitting_the_session_cell() {
    let f = Fx::new();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let door = f
        .gov
        .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(f.gov.file_session_halt()))
        .unwrap();
    let token = f.gov.authority().runs().check(f.run, &mother()).unwrap();
    assert!(matches!(
        door.adopt(f.run, pending.id),
        Err(AuthorityError::NotMember)
    ));
    assert!(!f
        .gov
        .authority()
        .runs()
        .try_is_member(f.run, door.group_for_test())
        .unwrap());
    assert!(f
        .gov
        .authority()
        .runs()
        .session_cell(f.run)
        .unwrap()
        .is_none());
    f.gov.file_session_halt().engage_emergency();
    no_worker(&f);
    assert!(
        !token.is_cancelled(),
        "unrelated legacy run was not changed by refusal"
    );
    f.authorize(&pending).unwrap();
    f.execute(&pending).unwrap();
    assert_eq!(std::fs::read_to_string(f.ws.join(NAME)).unwrap(), CONTENT);
}

#[test]
fn decision8_adoption_final_membership_hold() {
    let f = Fx::new();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let door = f
        .gov
        .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(f.gov.file_session_halt()))
        .unwrap();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .runs()
        .review2_hooks
        .arm("run.before_join", move || cell.engage_emergency());
    let result = door.adopt(f.run, pending.id);
    no_worker(&f);
    assert!(
        !f.gov
            .authority()
            .runs()
            .try_is_member(f.run, door.group_for_test())
            .unwrap(),
        "no membership added after final validation ingress"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
}

fn pending_install(point: &'static str) {
    let (f, _) = selected();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .hooks()
        .arm(point, move || cell.engage_emergency());
    let result = f.propose(NAME, CONTENT);
    no_worker(&f);
    assert_eq!(
        f.gov.control.pending_held(f.run),
        0,
        "no pending effect installed after ingress"
    );
    assert_eq!(f.gov.control.pending_places(), 0, "reservation returned");
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .iter()
        .all(|v| v.state == CommitmentState::Revoked));
    assert_refusal(&f, "pending_effect_refused_halted");
}
#[test]
fn decision8_pending_effect_after_prepare() {
    pending_install(at::PLACE_BEFORE_FILL);
}
#[test]
fn decision8_pending_effect_after_validation() {
    pending_install("decision8.pending_validated");
}
