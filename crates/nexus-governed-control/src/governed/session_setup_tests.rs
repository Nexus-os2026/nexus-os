//! R03–R10: real setup routes and positive controls before migration. The
//! fixture adapter must move to HostSession; these expectations must not be
//! imposed on ordinary legacy clients.
use super::*;

struct Bare {
    gov: Arc<GovernedControl>,
    workspace: std::path::PathBuf,
    exclusions: Exclusions,
    _state: TempRoot,
    _selected: TempRoot,
}
impl Bare {
    fn new() -> Self {
        assert!(crate::files::make_non_dumpable());
        let state = temp_root("session-setup-state");
        let selected = temp_root("session-setup-selected");
        let home = selected.0.path().join("selected");
        let workspace = home.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let exclusions = Exclusions {
            home,
            identity_home: selected.0.path().join("identity"),
            reserved: vec![selected.0.path().join("reserved")],
            uid: crate::launcher::effective_uid(),
        };
        let gov = Arc::new(
            GovernedControl::with_secrets(
                state.0.path(),
                Vault::Disabled.source(),
                Arc::new(MemoryEvidence::new(1024)),
                Arc::new(ManualClock::default()),
            )
            .unwrap()
            .with_kernel_stop(StopAuthority::new()),
        );
        Self {
            gov,
            workspace,
            exclusions,
            _state: state,
            _selected: selected,
        }
    }
    fn register(&self) -> Result<crate::files::WorkspaceInfo, AuthorityError> {
        self.gov.register_workspace_in_with_halt(
            &self.workspace,
            &self.exclusions,
            Some(&self.gov.file_session_halt()),
        )
    }
    fn grant(&self) -> Result<crate::authority::ids::GrantId, AuthorityError> {
        let workspace = self.gov.workspace().unwrap().id;
        self.gov.request_grant_with_halt(
            &GrantRequest::Files { workspace },
            Duration::from_secs(60),
            &Yes,
            Some(&self.gov.file_session_halt()),
        )
    }
    fn door(&self) -> Result<AgentDoor, AuthorityError> {
        self.gov.issue_agent_door_with_halt(
            AgentId::new("mother").unwrap(),
            Arc::new(Yes),
            Some(self.gov.file_session_halt()),
        )
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
                "hold_configure" => f.gov.hold_files_before_publish(Duration::ZERO).is_ok(),
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
    let f = Bare::new();
    f.register().unwrap();
    f.grant().unwrap();
    let owner = Arc::new(MissionCount::default());
    let door = f
        .gov
        .issue_agent_door_with_halt(
            AgentId::new("mother").unwrap(),
            owner.clone(),
            Some(f.gov.file_session_halt()),
        )
        .unwrap();
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
        door.agent_action(
            run,
            &Intent::File(FileCreate {
                name: "consent.txt".into(),
                content: "preserved cached consent\n".into()
            })
        ),
        Ok(AgentOutcome::AwaitingApproval(_))
    ));
    assert!(resources(&f.workspace).is_empty());
    println!("CONSENT-FULL/available=0 dialogs=64 original_binding_usable=true");
}
