//! R03–R10: real setup routes and positive controls before migration. The
//! fixture adapter must move to HostSession; these expectations must not be
//! imposed on ordinary legacy clients.
use super::*;

struct Bare {
    gov: Arc<GovernedControl>,
    primary: Arc<crate::evidence_store::PrimaryEvidence>,
    workspace: std::path::PathBuf,
    session: HostSession,
    request: std::sync::OnceLock<crate::host_session::SessionRequest>,
    _state: TempRoot,
    _selected: TempRoot,
}
impl Bare {
    fn new() -> Self {
        Self::with_confirmer(Arc::new(Yes))
    }
    fn with_confirmer(confirmer: Arc<dyn ControlConfirmer + Send + Sync>) -> Self {
        assert!(crate::files::make_non_dumpable());
        let state = temp_root("session-setup-state");
        let selected = temp_root("session-setup-selected");
        let home = selected.0.path().join("selected");
        let workspace = home.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let seed = SessionSeed::new().unwrap();
        let evidence = crate::evidence_store::PrimaryEvidence::open(
            &state.0.path().join("p3-evidence/evidence.sqlite3"),
            vec![],
        )
        .unwrap();
        let session = HostSession::open(
            seed,
            state.0.path(),
            evidence.clone(),
            Arc::new(ManualClock::default()),
            confirmer,
        )
        .unwrap();
        let gov = session.control_for_test();
        Self {
            gov,
            primary: evidence,
            workspace,
            session,
            request: std::sync::OnceLock::new(),
            _state: state,
            _selected: selected,
        }
    }
    fn register(&self) -> Result<crate::files::WorkspaceInfo, AuthorityError> {
        let info = self
            .session
            .bootstrap_workspace(&self.workspace, &[])
            .map_err(|_| AuthorityError::Halted)?;
        self.session
            .finish_bootstrap()
            .map_err(|_| AuthorityError::Halted)?;
        let request = self
            .session
            .ingress()
            .accept(CommandKind::Run, 1)
            .map_err(|_| AuthorityError::Halted)?;
        self.request
            .set(request)
            .map_err(|_| AuthorityError::Halted)?;
        Ok(info)
    }
    fn grant(&self) -> Result<crate::authority::ids::GrantId, AuthorityError> {
        self.session
            .grant_for_test(self.request.get().unwrap())
            .map_err(|_| AuthorityError::Halted)
    }
    fn door(&self) -> Result<SessionDoor, AuthorityError> {
        self.session
            .activate_for_test(self.request.get().unwrap())
            .map_err(|_| AuthorityError::Halted)
    }
    fn stop(&self, times: usize) {
        for _ in 0..times {
            self.gov.file_session_halt().engage_emergency();
        }
        assert!(!self.gov.stop_snapshot().broker_stopped);
        assert!(!self.gov.stop_snapshot().kernel_stopped);
    }
}

#[test]
fn session_readiness_sources_are_checked_before_hold_and_workspace() {
    use crate::host_session::recovery::SourceFailure;
    for source in ["new", "torn_mother", "poisoned_mother", "kernel"] {
        let b = Bare::new();
        let root = b._state.0.path();
        match source {
            "torn_mother" => {
                std::fs::write(root.join("mother-journal.jsonl"), b"{\"format\":2").unwrap()
            }
            "poisoned_mother" => {
                std::fs::write(root.join("mother-journal.jsonl.poisoned"), b"").unwrap()
            }
            "kernel" => std::fs::create_dir_all(root.join("files/ledger.db")).unwrap(),
            _ => {}
        }
        let report = b.session.startup_report();
        assert_eq!(report.ready(), source == "new");
        match source {
            "torn_mother" | "poisoned_mother" => {
                assert!(matches!(report.mother, Err(SourceFailure::MotherHistory)))
            }
            "kernel" => assert!(matches!(report.kernel, Err(SourceFailure::KernelLedger))),
            _ => assert!(report.mother.as_ref().unwrap().newly_absent),
        }
        assert_eq!(
            b.session.bootstrap_hold(Duration::ZERO).is_ok(),
            source == "new"
        );
        assert_eq!(b.register().is_ok(), source == "new");
        assert_eq!(b.gov.workspace().is_some(), source == "new");
        assert!(b.gov.grants().is_empty());
        println!("READINESS/{source}: before_workspace=true conforms=true");
    }
}

#[test]
fn session_setup_commands_refuse_original_cell_emergency_without_native_workers() {
    let mut failures = Vec::new();
    for route in [
        "hold_configure",
        "workspace_install",
        "grant_install",
        "door_activate",
        "port_issue",
        "attachment_claim",
        "file_select",
        "run_create",
        "mission_confirm",
    ] {
        for ingresses in 0..=3 {
            let f = Bare::new();
            if !matches!(route, "hold_configure" | "workspace_install") {
                f.register().unwrap();
            }
            if !matches!(
                route,
                "hold_configure" | "workspace_install" | "grant_install"
            ) {
                f.grant().unwrap();
            }
            let door = (!matches!(
                route,
                "hold_configure" | "workspace_install" | "grant_install" | "door_activate"
            ))
            .then(|| f.door().unwrap());
            let port = matches!(route, "attachment_claim" | "file_select" | "run_create")
                .then(|| door.as_ref().unwrap().file_port().unwrap());
            let attachment = matches!(route, "file_select" | "run_create")
                .then(|| port.as_ref().unwrap().attach().unwrap());
            if route == "run_create" {
                door.as_ref()
                    .unwrap()
                    .claim_file_session(attachment.as_ref().unwrap())
                    .unwrap();
            }
            f.stop(ingresses);
            let success = match route {
                "hold_configure" => f.session.bootstrap_hold(Duration::ZERO).is_ok(),
                "workspace_install" => f.register().is_ok(),
                "grant_install" => f.grant().is_ok(),
                "door_activate" => f.door().is_ok(),
                "port_issue" => door.as_ref().unwrap().file_port().is_ok(),
                "attachment_claim" => port.as_ref().unwrap().attach().is_ok(),
                "file_select" => door
                    .as_ref()
                    .unwrap()
                    .claim_file_session(attachment.as_ref().unwrap())
                    .is_ok(),
                "run_create" => door
                    .as_ref()
                    .unwrap()
                    .open_file_run(attachment.as_ref().unwrap())
                    .is_ok(),
                "mission_confirm" => door
                    .as_ref()
                    .unwrap()
                    .confirm_mission(&MissionConfirmation {
                        contract_hash: HASH.into(),
                        request: CONSENT.into(),
                        summary: vec!["One fixture file".into()],
                        page_facts: None,
                    })
                    .is_ok(),
                _ => unreachable!(),
            };
            let correct = success == (ingresses == 0);
            println!("SETUP/{route}/ingresses={ingresses}: native_workers=0 success={success} conforms={correct}");
            if !correct {
                failures.push((route, ingresses));
            }
            assert!(resources(&f.workspace).is_empty());
        }
    }
    assert!(
        failures.is_empty(),
        "36 executed setup schedules: {failures:?}"
    );
}

#[test]
fn session_workspace_and_door_slow_return_cannot_refresh_lifetime() {
    let mut failures = Vec::new();
    for point in ["files.before_register", "issue.answered", "issue.recorded"] {
        for ingresses in 0..=3 {
            let f = Bare::new();
            if point != "files.before_register" {
                f.register().unwrap();
                f.grant().unwrap();
            }
            let reached = Arc::new(AtomicUsize::new(0));
            let seen = reached.clone();
            let cell = f.gov.file_session_halt();
            let hooks = if point == "files.before_register" {
                f.gov.files().hooks()
            } else {
                f.gov.hooks()
            };
            hooks.arm(point, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingresses {
                    cell.engage_emergency();
                }
            });
            let success = if point == "files.before_register" {
                f.register().is_ok()
            } else {
                f.door().is_ok()
            };
            assert_eq!(reached.load(Ordering::SeqCst), 1);
            let correct = success == (ingresses == 0);
            println!("SETUP/{point}/ingresses={ingresses}: native_workers=0 success={success} conforms={correct}");
            if !correct {
                failures.push((point, ingresses));
            }
            if point == "files.before_register" {
                assert_eq!(f.gov.workspace().is_some(), ingresses == 0);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "12 executed slow setup schedules: {failures:?}"
    );
}

#[test]
fn session_project_selection_itself_is_admitted_before_broker_installation() {
    let mut failures = Vec::new();
    for ingresses in 0..=3 {
        let f = Bare::new();
        let reached = Arc::new(AtomicUsize::new(0));
        let returned = Arc::new(AtomicUsize::new(0));
        let seen = reached.clone();
        let done = returned.clone();
        let cell = f.gov.file_session_halt();
        f.gov
            .files()
            .hooks()
            .arm("session.before.project_select", move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingresses {
                    cell.engage_emergency();
                }
            });
        f.gov
            .files()
            .hooks()
            .arm("session.after.project_select", move || {
                done.fetch_add(1, Ordering::SeqCst);
            });
        let success = f.register().is_ok();
        assert_eq!(reached.load(Ordering::SeqCst), 1);
        let native = returned.load(Ordering::SeqCst);
        let correct = if ingresses == 0 {
            success && native == 1
        } else {
            !success && native == 0 && f.gov.workspace().is_none()
        };
        println!("SETUP/project_select/ingresses={ingresses}: native_workers=0 native_call_returns={native} conforms={correct}");
        if !correct {
            failures.push(ingresses);
        }
    }
    assert!(
        failures.is_empty(),
        "four executed native project selection schedules: {failures:?}"
    );
}

#[test]
fn session_run_and_consent_and_file_binding_final_holds_keep_the_original_lifetime() {
    let mut failures = Vec::new();
    for point in [
        "run.before_insert",
        "door.mission_recorded",
        "door.consent_before_fill",
        "door.admit_before_bind",
        "files.before_install",
    ] {
        for ingresses in 0..=3 {
            let f = Bare::new();
            f.register().unwrap();
            f.grant().unwrap();
            let door = f.door().unwrap();
            let attachment = door.file_port().unwrap().attach().unwrap();
            door.claim_file_session(&attachment).unwrap();
            let request = MissionConfirmation {
                contract_hash: HASH.into(),
                request: CONSENT.into(),
                summary: vec!["One fixture file".into()],
                page_facts: None,
            };
            let binding = matches!(point, "door.admit_before_bind" | "files.before_install");
            let run = binding.then(|| {
                door.confirm_mission(&request).unwrap();
                door.open_file_run(&attachment).unwrap()
            });
            let reached = Arc::new(AtomicUsize::new(0));
            let seen = reached.clone();
            let cell = f.gov.file_session_halt();
            let hooks = if matches!(point, "run.before_insert" | "files.before_install") {
                &f.gov.authority().runs().review2_hooks
            } else {
                f.gov.hooks()
            };
            hooks.arm(point, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingresses {
                    cell.engage_emergency();
                }
            });
            let success = if let Some(run) = run {
                door.admit_files(run, HASH, CONSENT).is_ok()
            } else if point == "run.before_insert" {
                door.open_file_run(&attachment).is_ok()
            } else {
                door.confirm_mission(&request).is_ok()
            };
            assert_eq!(reached.load(Ordering::SeqCst), 1, "{point}");
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            let correct = success == (ingresses == 0);
            println!("SETUP-HOLD/{point}/ingresses={ingresses}: native_workers=0 success={success} conforms={correct}");
            if !correct {
                failures.push((point, ingresses));
            }
            assert!(resources(&f.workspace).is_empty());
        }
    }
    assert!(
        failures.is_empty(),
        "20 executed final-hold schedules: {failures:?}"
    );
}

#[derive(Default)]
struct MissionCount(AtomicUsize);
impl ControlConfirmer for MissionCount {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst);
        true
    }
}

#[test]
fn session_consent_capacity_is_reserved_before_dialogue_and_keeps_existing_bindings() {
    let owner = Arc::new(MissionCount::default());
    let f = Bare::with_confirmer(owner.clone());
    f.register().unwrap();
    f.grant().unwrap();
    let door = f.door().unwrap();
    let attachment = door.file_port().unwrap().attach().unwrap();
    door.claim_file_session(&attachment).unwrap();
    assert_eq!(owner.0.load(Ordering::SeqCst), 0);
    for number in 1..=64 {
        door.confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: format!("msn-000001/1/{number}"),
            summary: vec!["One bounded consent fixture".into()],
            page_facts: None,
        })
        .unwrap();
        assert_eq!(owner.0.load(Ordering::SeqCst), number);
        if matches!(number, 1 | 63 | 64) {
            println!("CONSENT-BOUNDARY/recorded={number}");
        }
    }
    assert!(door
        .confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: "msn-000001/1/65".into(),
            summary: vec!["No capacity".into()],
            page_facts: None,
        })
        .is_err());
    assert_eq!(
        owner.0.load(Ordering::SeqCst),
        64,
        "no question after capacity is exhausted"
    );
    let run = door.open_file_run(&attachment).unwrap();
    door.admit_files(run, HASH, "msn-000001/1/64").unwrap();
    assert!(matches!(
        door.create_file(
            run,
            &FileCreate {
                name: "consent.txt".into(),
                content: "preserved cached consent\n".into()
            }
        ),
        Ok(AgentOutcome::AwaitingApproval(_))
    ));
    assert!(resources(&f.workspace).is_empty());
    println!("CONSENT-FULL/available=0 dialogs=64 original_binding_usable=true");
}

#[derive(Default)]
struct SetupAnswers {
    grant: AtomicUsize,
    door: AtomicUsize,
    mission: AtomicUsize,
}
impl ControlConfirmer for SetupAnswers {
    fn confirm_action(&self, r: &ActionConfirmation) -> bool {
        Yes.confirm_action(r)
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.grant.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        self.door.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.mission.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        false
    }
    fn confirm_undo(&self, _: &UndoConfirmation) -> bool {
        true
    }
}

/// The interference is after all preceding holds/validation. Compare-gap
/// scheduling also requires an actual same-word CAS before the target sink.
#[test]
fn session_setup_each_final_boundary_has_its_own_unscheduled_ingress_control() {
    let selected = std::env::var("M2DD_SETUP_FINAL").ok();
    let mut failures = Vec::new();
    let mut count = 0;
    for route in [
        "hold_configure",
        "workspace_install",
        "grant_confirm",
        "grant_issue",
        "grant_install",
        "door_confirm",
        "door_activate",
        "port_issue",
        "attachment_claim",
        "file_select",
        "mission_confirm",
        "consent_cache",
        "run_create",
        "file_binding",
    ] {
        for when in ["none", "before", "compare", "repeat"] {
            let key = format!("{route}/{when}");
            if selected.as_ref().is_some_and(|value| value != &key) {
                continue;
            }
            let owner = Arc::new(SetupAnswers::default());
            let f = Bare::with_confirmer(owner.clone());
            let bootstrap = matches!(route, "hold_configure" | "workspace_install");
            let granting = route.starts_with("grant_");
            let issuing = route.starts_with("door_");
            if !bootstrap {
                f.register().unwrap();
            }
            if !bootstrap && !granting {
                f.grant().unwrap();
            }
            let door = (!bootstrap && !granting && !issuing).then(|| f.door().unwrap());
            let attached = matches!(
                route,
                "attachment_claim" | "file_select" | "run_create" | "file_binding"
            );
            let port = attached.then(|| door.as_ref().unwrap().file_port().unwrap());
            let attachment = matches!(route, "file_select" | "run_create" | "file_binding")
                .then(|| port.as_ref().unwrap().attach().unwrap());
            if matches!(route, "run_create" | "file_binding") {
                door.as_ref()
                    .unwrap()
                    .claim_file_session(attachment.as_ref().unwrap())
                    .unwrap();
            }
            let mission = MissionConfirmation {
                contract_hash: HASH.into(),
                request: CONSENT.into(),
                summary: vec!["One fixture file".into()],
                page_facts: None,
            };
            let run = (route == "file_binding").then(|| {
                door.as_ref().unwrap().confirm_mission(&mission).unwrap();
                door.as_ref()
                    .unwrap()
                    .open_file_run(attachment.as_ref().unwrap())
                    .unwrap()
            });
            let history_before = f.gov.file_sessions.counts_for_test();
            let grant_before = owner.grant.load(Ordering::SeqCst);
            let door_before = owner.door.load(Ordering::SeqCst);
            let mission_before = owner.mission.load(Ordering::SeqCst);
            let hit = Arc::new(AtomicUsize::new(0));
            let seen = hit.clone();
            let compares = Arc::new(AtomicUsize::new(0));
            let compared = compares.clone();
            let cell = f.gov.file_session_halt();
            let schedule = cell.clone();
            let point = match route {
                "hold_configure" => "setup.hold_configure",
                "workspace_install" => "setup.workspace_install",
                "grant_confirm" => "setup.grant_confirm",
                "grant_issue" => "setup.grant_issue",
                "grant_install" => "setup.grant_install",
                "door_confirm" => "setup.door_confirm",
                "door_activate" => "setup.door_activate",
                "port_issue" => "setup.port_issue",
                "attachment_claim" => "setup.attachment_claim",
                "file_select" => "setup.file_select",
                "mission_confirm" => "setup.mission_confirm",
                "consent_cache" => "setup.consent_cache",
                "run_create" => "setup.run_create",
                "file_binding" => "setup.file_binding",
                _ => unreachable!(),
            };
            cell.1.arm(point, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                if when == "compare" {
                    let ingress = schedule.clone();
                    schedule.1.arm("session.before_compare", move || {
                        compared.fetch_add(1, Ordering::SeqCst);
                        ingress.engage_emergency();
                    });
                } else if when != "none" {
                    schedule.engage_emergency();
                    if when == "repeat" {
                        schedule.engage_emergency();
                    }
                }
            });
            let success = match route {
                "hold_configure" => f.session.bootstrap_hold(Duration::from_millis(7)).is_ok(),
                "workspace_install" => f.register().is_ok(),
                "grant_confirm" | "grant_issue" | "grant_install" => f.grant().is_ok(),
                "door_confirm" | "door_activate" => f.door().is_ok(),
                "port_issue" => door.as_ref().unwrap().file_port().is_ok(),
                "attachment_claim" => port.as_ref().unwrap().attach().is_ok(),
                "file_select" => door
                    .as_ref()
                    .unwrap()
                    .claim_file_session(attachment.as_ref().unwrap())
                    .is_ok(),
                "mission_confirm" | "consent_cache" => {
                    door.as_ref().unwrap().confirm_mission(&mission).is_ok()
                }
                "run_create" => door
                    .as_ref()
                    .unwrap()
                    .open_file_run(attachment.as_ref().unwrap())
                    .is_ok(),
                "file_binding" => door
                    .as_ref()
                    .unwrap()
                    .admit_files(run.unwrap(), HASH, CONSENT)
                    .is_ok(),
                _ => unreachable!(),
            };
            let admitted = when == "none";
            let history_after = f.gov.file_sessions.counts_for_test();
            let mut conforms = hit.load(Ordering::SeqCst) == 1 && success == admitted;
            if when == "compare" {
                conforms &= compares.load(Ordering::SeqCst) == 1;
            }
            match route {
                "grant_confirm" => {
                    conforms &=
                        owner.grant.load(Ordering::SeqCst) - grant_before == usize::from(admitted)
                }
                "grant_issue" => {
                    conforms &= f
                        .primary
                        .store()
                        .rows()
                        .iter()
                        .filter(|row| row.phase == "grant_issued")
                        .count()
                        == usize::from(admitted)
                }
                "grant_install" => conforms &= f.gov.grants().len() == usize::from(admitted),
                "door_confirm" => {
                    conforms &=
                        owner.door.load(Ordering::SeqCst) - door_before == usize::from(admitted)
                }
                "mission_confirm" => {
                    conforms &= owner.mission.load(Ordering::SeqCst) - mission_before
                        == usize::from(admitted)
                }
                "port_issue" => {
                    conforms &= history_after.0 == history_before.0 + u64::from(admitted)
                }
                "attachment_claim" => {
                    conforms &= history_after.1 == history_before.1 + u64::from(admitted)
                }
                "file_select" => conforms &= history_after.2 == admitted,
                "run_create" => {
                    conforms &= f.gov.authority().runs().views().len() == usize::from(admitted)
                }
                "workspace_install" => conforms &= f.gov.workspace().is_some() == admitted,
                _ => {}
            }
            assert!(resources(&f.workspace).is_empty());
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            println!("SETUP-FINAL/{key}: native_workers=0 success={success} conforms={conforms}");
            count += 1;
            if !conforms {
                failures.push(key);
            }
        }
    }
    assert_eq!(count, if selected.is_some() { 1 } else { 56 });
    assert!(
        failures.is_empty(),
        "{count} executed final setup schedules: {failures:?}"
    );
}
