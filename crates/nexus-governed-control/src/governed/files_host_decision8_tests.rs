//! Decisions 7–8 baseline and fresh route sweep; no Stop worker is ever scheduled.
//! The baseline has only the legacy issue entry point; the exact recorded hook
//! and backend observations below reproduce the previously reported schedule.
use super::*;
use crate::authority::commitment::CommitmentState;
use crate::authority::file_session::{FileAttachment, HaltCell};

fn selected() -> (Fx, FileAttachment) {
    let mut f = Fx::new();
    let port = f.door.file_port().unwrap();
    let attached = port.attach().unwrap();
    f.door.claim_file_session(&attached).unwrap();
    f.run = f.door.open_file_run(&attached).unwrap();
    f.door.admit_files(f.run, HASH, LABEL).unwrap();
    (f, attached)
}
fn no_worker(f: &Fx) {
    assert!(f.gov.file_session_halt().is_engaged());
    assert!(!f.gov.stop_snapshot().broker_stopped, "no Stop worker ran");
    assert!(
        !f.gov.stop_snapshot().kernel_stopped,
        "no kernel worker ran"
    );
}
fn phase(f: &Fx, p: EvidencePhase) -> usize {
    f.evidence.records().iter().filter(|r| r.phase == p).count()
}
struct CountOwner {
    doors: AtomicUsize,
    grants: AtomicUsize,
    after_grant_answer: Option<HaltCell>,
}
impl CountOwner {
    fn new(cell: Option<HaltCell>) -> Arc<Self> {
        Arc::new(Self {
            doors: AtomicUsize::new(0),
            grants: AtomicUsize::new(0),
            after_grant_answer: cell,
        })
    }
}
impl ControlConfirmer for CountOwner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.grants.fetch_add(1, Ordering::SeqCst);
        if let Some(cell) = &self.after_grant_answer {
            cell.engage_emergency();
        }
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        self.doors.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        true
    }
    fn confirm_undo(&self, _: &UndoConfirmation) -> bool {
        true
    }
}
#[test]
fn decision7_door_exact_issue_recorded_counter() {
    let f = Fx::new();
    let ingress = f.gov.file_session_halt();
    assert_eq!(f.gov.status().doors_issued, 1);
    f.gov
        .hooks()
        .arm(at::ISSUE_RECORDED, move || ingress.engage_emergency());
    let issued = f.gov.issue_agent_door_with_halt(
        mother(),
        f.owner.clone(),
        Some(f.gov.file_session_halt()),
    );
    no_worker(&f);
    assert_eq!(
        f.gov.status().doors_issued,
        1,
        "ISSUE_RECORDED: no new backend door after ingress"
    );
    assert!(matches!(issued, Err(AuthorityError::Halted)));
}
#[test]
fn decision7_door_exact_issue_recorded_evidence() {
    let f = Fx::new();
    let ingress = f.gov.file_session_halt();
    let opened = phase(&f, EvidencePhase::AgentDoorOpened);
    let declined = phase(&f, EvidencePhase::AgentDoorDeclined);
    f.gov
        .hooks()
        .arm(at::ISSUE_RECORDED, move || ingress.engage_emergency());
    let _issued = f.gov.issue_agent_door_with_halt(
        mother(),
        f.owner.clone(),
        Some(f.gov.file_session_halt()),
    );
    no_worker(&f);
    assert_eq!(
        phase(&f, EvidencePhase::AgentDoorOpened),
        opened + 1,
        "Decision 8: preserve the already written answer, never rewrite history"
    );
    assert_refusal(&f, "door_activation_refused_halted");
    assert_eq!(phase(&f, EvidencePhase::AgentDoorDeclined), declined + 1);
}
#[test]
fn decision7_door_halted_before_dialogue() {
    let f = Fx::new();
    let owner = CountOwner::new(None);
    f.gov.file_session_halt().engage_emergency();
    let issued =
        f.gov
            .issue_agent_door_with_halt(mother(), owner.clone(), Some(f.gov.file_session_halt()));
    no_worker(&f);
    assert_eq!(
        owner.doors.load(Ordering::SeqCst),
        0,
        "engaged before issue: no native dialogue"
    );
    assert!(matches!(issued, Err(AuthorityError::Halted)));
}
#[test]
fn decision7_legacy_resume_control() {
    let f = Fx::new();
    f.gov.emergency_stop();
    f.gov.resume(&*f.owner).unwrap();
    assert!(f.gov.file_session_halt().is_engaged());
    f.gov
        .issue_agent_door(mother(), f.owner.clone())
        .unwrap()
        .open_run()
        .unwrap();
}
#[test]
fn decision7_workspace_registration_after_ingress() {
    let root = temp_root("d7-registration");
    let evidence = Arc::new(MemoryEvidence::new(100_000));
    let clock = Arc::new(ManualClock::default());
    let stop = StopAuthority::new();
    let gov = broker(&root, &evidence, &clock, &stop);
    let dirs = Dirs::new();
    assert!(gov.workspace().is_none());
    gov.file_session_halt().engage_emergency();
    let result = gov.register_workspace_in_with_halt(
        &dirs.0.join("home/ws"),
        &dirs.exclusions(),
        Some(&gov.file_session_halt()),
    );
    assert!(!gov.stop_snapshot().broker_stopped);
    assert!(!gov.stop_snapshot().kernel_stopped);
    assert!(
        gov.workspace().is_none(),
        "workspace admission after ingress must install no backend workspace"
    );
    assert!(result.is_err());
}
fn grant_after_ingress(after_answer: bool) {
    let f = Fx::new();
    let cell = f.gov.file_session_halt();
    let owner = CountOwner::new(after_answer.then_some(cell.clone()));
    let before = f.gov.grants().len();
    if !after_answer {
        cell.engage_emergency();
    }
    let result = f.gov.request_grant_with_halt(
        &GrantRequest::Files {
            workspace: f.gov.workspace().unwrap().id,
        },
        LONG,
        &*owner,
        Some(&cell),
    );
    no_worker(&f);
    assert_eq!(
        f.gov.grants().len(),
        before,
        "Files grant issuance after ingress must add no backend grant"
    );
    assert_eq!(
        owner.grants.load(Ordering::SeqCst),
        usize::from(after_answer)
    );
    assert!(result.is_err());
}
#[test]
fn decision7_files_grant_already_halted() {
    grant_after_ingress(false);
}
#[test]
fn decision7_files_grant_after_native_answer() {
    grant_after_ingress(true);
}
#[test]
fn decision7_deny_only_topology_handles_do_not_select() {
    let f = Fx::new();
    f.gov.file_session_halt().engage_emergency();
    let port = f.door.file_port().unwrap();
    let attached = port.attach().unwrap();
    assert!(f.door.claim_file_session(&attached).is_err());
    assert!(f.door.open_file_run(&attached).is_err());
    no_worker(&f);
}
#[test]
fn decision7_selected_routes_refuse_at_entry() {
    let (f, attached) = selected();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let before = f.gov.authority().runs().review2_counts();
    f.gov.file_session_halt().engage_emergency();
    assert!(f.door.claim_file_session(&attached).is_err());
    assert!(f.door.open_run().is_err());
    assert!(f.door.open_file_run(&attached).is_err());
    assert!(f.door.adopt(f.run, pending.id).is_err());
    assert!(f
        .door
        .confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: "msn-000002/1/1".into(),
            summary: vec!["Create one approved text file".into()],
            page_facts: None
        })
        .is_err());
    assert!(f.door.admit_files(f.run, HASH, LABEL).is_err());
    assert!(f
        .door
        .admit_inference(f.run, HASH, LABEL, [42; 32])
        .is_err());
    assert!(f.propose("later.txt", CONTENT).is_err());
    assert!(f.authorize(&pending).is_err());
    assert!(f.execute(&pending).is_err());
    assert_eq!(f.gov.authority().runs().review2_counts(), before);
    assert!(!f.ws.join(NAME).exists());
    no_worker(&f);
}
#[test]
fn decision7_consent_after_validation_before_fill() {
    let (f, _) = selected();
    let before = f.door.consents_cached();
    let cell = f.gov.file_session_halt();
    f.gov
        .hooks()
        .arm(at::CONSENT_BEFORE_FILL, move || cell.engage_emergency());
    let result = f.door.confirm_mission(&MissionConfirmation {
        contract_hash: HASH.into(),
        request: "msn-000002/1/1".into(),
        summary: vec!["Create one approved text file".into()],
        page_facts: None,
    });
    no_worker(&f);
    assert_eq!(f.door.consents_cached(), before);
    assert!(result.is_err());
}
#[test]
fn decision7_files_prepare_after_member_check() {
    let (f, _) = selected();
    let staged = Arc::new(AtomicUsize::new(0));
    let seen = staged.clone();
    f.gov.files().hooks().every(at::FILE_STAGED, move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    let cell = f.gov.file_session_halt();
    f.gov
        .hooks()
        .arm(at::DOOR_MEMBER_CHECKED, move || cell.engage_emergency());
    let result = f.propose(NAME, CONTENT);
    no_worker(&f);
    assert_eq!(
        staged.load(Ordering::SeqCst),
        0,
        "File staging dispatched after ingress at DOOR_MEMBER_CHECKED"
    );
    assert!(result.is_err());
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
}
#[test]
fn decision7_prepare_refuses_commitment_after_record() {
    let (f, _) = selected();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::PREPARE_BEFORE_INSERT, move || cell.engage_emergency());
    assert!(f.propose(NAME, CONTENT).is_err());
    no_worker(&f);
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
}
#[test]
fn decision7_authorize_after_record_control() {
    let (f, _) = selected();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let cell = f.gov.file_session_halt();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::AUTHORIZE_RECORDED, move || cell.engage_emergency());
    assert!(f.authorize(&pending).is_err());
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
}
#[test]
fn decision7_authorize_during_final_grant_validation() {
    let (f, _) = selected();
    let pending = f.propose(NAME, CONTENT).unwrap();
    let cell = f.gov.file_session_halt();
    let registry = f.gov.authority().commitments().clone();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::AUTHORIZE_RECORDED, move || {
            registry.hooks().arm("decision7.grant_validation", move || {
                cell.engage_emergency()
            });
        });
    let result = f.authorize(&pending);
    no_worker(&f);
    assert_ne!(
        f.gov
            .authority()
            .commitments()
            .view(pending.id)
            .unwrap()
            .state,
        CommitmentState::Authorized,
        "authorization admitted after ingress during final grant validation"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_refusal(&f, "authorization_refused_halted");
}
#[test]
fn decision7_execute_during_final_grant_validation() {
    let (f, _) = selected();
    let pending = f.ready(NAME, CONTENT);
    let cell = f.gov.file_session_halt();
    let registry = f.gov.authority().commitments().clone();
    f.gov
        .authority()
        .commitments()
        .hooks()
        .arm(at::COMPLETE_BEFORE_CHECK, move || {
            registry.hooks().arm("decision7.grant_validation", move || {
                cell.engage_emergency()
            });
        });
    let dispatched = Arc::new(AtomicUsize::new(0));
    let seen = dispatched.clone();
    f.gov
        .files()
        .hooks()
        .every(at::FILE_EXECUTE_START, move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
    let result = f.execute(&pending);
    no_worker(&f);
    assert!(
        !f.ws.join(NAME).exists(),
        "publication must still be stopped"
    );
    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        0,
        "Files execution dispatched after ingress during final grant validation"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert_refusal(&f, "execution_dispatch_refused_halted");
}
#[test]
fn decision7_selected_positive_controls() {
    let (f, _) = selected();
    let before = f.gov.grants().len();
    f.gov
        .request_grant(
            &GrantRequest::Files {
                workspace: f.gov.workspace().unwrap().id,
            },
            LONG,
            &*f.owner,
        )
        .unwrap();
    assert_eq!(f.gov.grants().len(), before + 1);
    f.create(NAME, CONTENT).unwrap();
    assert_eq!(std::fs::read_to_string(f.ws.join(NAME)).unwrap(), CONTENT);
}

#[test]
fn decision7_held_consent_after_validation_before_fill() {
    let (f, _) = selected();
    let request = MissionResumeConfirmation {
        mission: "msn-000001".into(),
        contract_hash: HASH.into(),
        request: "msn-000001/1/2".into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["Create one approved text file".into()],
    };
    f.door.confirm_held_mission(&request).unwrap();
    let before = f.door.consents_cached();
    let cell = f.gov.file_session_halt();
    f.gov
        .hooks()
        .arm(at::CONSENT_BEFORE_FILL, move || cell.engage_emergency());
    assert!(f.door.confirm_held_mission(&request).is_err());
    no_worker(&f);
    assert_eq!(f.door.consents_cached(), before);
}

#[test]
fn decision7_grant_after_evidence_before_install() {
    use crate::authority::evidence::{EvidenceSink, EvidenceUnavailable};
    struct StopOnGrant {
        memory: MemoryEvidence,
        cell: std::sync::OnceLock<HaltCell>,
        reached: AtomicBool,
    }
    impl EvidenceSink for StopOnGrant {
        fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
            let seq = self.memory.record(record)?;
            if record.phase == EvidencePhase::GrantIssued {
                self.cell.get().unwrap().engage_emergency();
                self.reached.store(true, Ordering::SeqCst);
            }
            Ok(seq)
        }
    }
    let root = temp_root("d7-grant-record");
    let evidence = Arc::new(StopOnGrant {
        memory: MemoryEvidence::new(1000),
        cell: std::sync::OnceLock::new(),
        reached: AtomicBool::new(false),
    });
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            evidence.clone(),
            Arc::new(ManualClock::default()),
        )
        .unwrap(),
    );
    evidence.cell.set(gov.file_session_halt()).unwrap();
    let dirs = Dirs::new();
    let workspace = gov
        .register_workspace_in(&dirs.0.join("home/ws"), &dirs.exclusions())
        .unwrap();
    let owner = CountOwner::new(None);
    let result = gov.request_grant_with_halt(
        &GrantRequest::Files {
            workspace: workspace.id,
        },
        LONG,
        &*owner,
        Some(&gov.file_session_halt()),
    );
    assert_eq!(owner.grants.load(Ordering::SeqCst), 1);
    assert!(evidence.reached.load(Ordering::SeqCst));
    assert!(gov.file_session_halt().is_engaged());
    assert!(!gov.stop_snapshot().broker_stopped);
    assert!(!gov.stop_snapshot().kernel_stopped);
    assert_eq!(
        gov.grants().len(),
        0,
        "grant added after ingress in GrantIssued evidence, after native answer"
    );
    assert!(evidence
        .memory
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::GrantDeclined
            && r.failure == Some("halted")
            && r.detail
                .iter()
                .any(|(k, v)| k == "reason" && v == "grant_issue_refused_halted")));
    assert!(result.is_err());
}

fn assert_refusal(f: &Fx, code: &str) {
    let records = f.evidence.records();
    let record = records
        .iter()
        .find(|r| r.detail.iter().any(|(k, v)| k == "reason" && v == code))
        .expect("closed route refusal must be preserved by evidence sealing");
    assert_eq!(record.failure, Some("halted"));
    assert_ne!(record.outcome, Some("succeeded"));
}

#[test]
fn decision8_workspace_after_validation_before_install() {
    let root = temp_root("d8-register");
    let evidence = Arc::new(MemoryEvidence::new(1000));
    let clock = Arc::new(ManualClock::default());
    let gov = broker(&root, &evidence, &clock, &StopAuthority::new());
    let dirs = Dirs::new();
    let cell = gov.file_session_halt();
    let ingress = cell.clone();
    gov.files()
        .hooks()
        .arm("files.before_register", move || ingress.engage_emergency());
    let result = gov.register_workspace_in_with_halt(
        &dirs.0.join("home/ws"),
        &dirs.exclusions(),
        Some(&cell),
    );
    assert!(cell.is_engaged(), "exact registration slow point reached");
    assert!(!gov.stop_snapshot().broker_stopped);
    assert!(!gov.stop_snapshot().kernel_stopped);
    assert!(
        gov.workspace().is_none(),
        "workspace installed after slow validation ingress"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
}

#[test]
fn decision8_files_after_validation_before_stage() {
    let (f, _) = selected();
    let cell = f.gov.file_session_halt();
    f.gov
        .files()
        .hooks()
        .arm("files.before_stage", move || cell.engage_emergency());
    let staged = Arc::new(AtomicUsize::new(0));
    let seen = staged.clone();
    f.gov.files().hooks().every(at::FILE_STAGED, move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    let result = f.propose(NAME, CONTENT);
    no_worker(&f);
    assert_eq!(
        staged.load(Ordering::SeqCst),
        0,
        "no staging after slow validation"
    );
    assert!(matches!(result, Err(AuthorityError::Halted)));
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
}

#[test]
fn decision8_host_door_carries_cell_before_file_selection() {
    let f = Fx::new();
    let cell = f.gov.file_session_halt();
    let door = f
        .gov
        .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(cell.clone()))
        .unwrap();
    let run = door.open_run().unwrap();
    let token = f.gov.authority().runs().check(run, &mother()).unwrap();
    assert!(!token.is_cancelled());
    let before = f.gov.authority().runs().review2_counts();
    cell.engage_emergency();
    assert!(
        token.is_cancelled(),
        "ordinary host door run must carry the session cell"
    );
    assert!(door.open_run().is_err());
    assert!(door
        .confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: LABEL.into(),
            summary: vec!["one file".into()],
            page_facts: None
        })
        .is_err());
    assert_eq!(f.gov.authority().runs().review2_counts(), before);
    no_worker(&f);
}

#[test]
fn decision8_host_door_run_final_insertion() {
    let f = Fx::new();
    let cell = f.gov.file_session_halt();
    let door = f
        .gov
        .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(cell.clone()))
        .unwrap();
    let before = f.gov.authority().runs().review2_counts();
    f.gov
        .authority()
        .runs()
        .review2_hooks
        .arm("run.before_insert", move || cell.engage_emergency());
    let result = door.open_run();
    no_worker(&f);
    assert_eq!(f.gov.authority().runs().review2_counts(), before);
    assert!(matches!(result, Err(AuthorityError::Halted)));
}

#[test]
fn decision8_host_cell_then_file_selection_positive() {
    let mut f = Fx::new();
    f.door = Arc::new(
        f.gov
            .issue_agent_door_with_halt(mother(), f.owner.clone(), Some(f.gov.file_session_halt()))
            .unwrap(),
    );
    let attached = f.door.file_port().unwrap().attach().unwrap();
    f.door.claim_file_session(&attached).unwrap();
    f.run = f.door.open_file_run(&attached).unwrap();
    f.door
        .confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: LABEL.into(),
            summary: vec!["one file".into()],
            page_facts: None,
        })
        .unwrap();
    f.door.admit_files(f.run, HASH, LABEL).unwrap();
    f.create(NAME, CONTENT).unwrap();
    assert_eq!(std::fs::read_to_string(f.ws.join(NAME)).unwrap(), CONTENT);
}

#[test]
fn decision8_optional_none_preserves_legacy_registration_and_grant() {
    let root = temp_root("d8-legacy");
    let evidence = Arc::new(MemoryEvidence::new(1000));
    let clock = Arc::new(ManualClock::default());
    let gov = broker(&root, &evidence, &clock, &StopAuthority::new());
    let dirs = Dirs::new();
    gov.file_session_halt().engage_emergency();
    let ws = gov
        .register_workspace_in_with_halt(&dirs.0.join("home/ws"), &dirs.exclusions(), None)
        .unwrap();
    gov.request_grant_with_halt(
        &GrantRequest::Files { workspace: ws.id },
        LONG,
        &*CountOwner::new(None),
        None,
    )
    .unwrap();
    assert_eq!(gov.grants().len(), 1);
}

#[path = "files_host_decision8_sweep_tests.rs"]
mod sweep;
