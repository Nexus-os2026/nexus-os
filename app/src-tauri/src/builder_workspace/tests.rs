use super::*;
use nexus_kernel::workspace_authority::WorkspaceAuthorityError;
use std::sync::Mutex;
use web_builder_agent::model_router::ProviderType;

struct Fixture {
    path: PathBuf,
    authority: BuilderWorkspaceAuthority,
    events: Arc<Mutex<Vec<Value>>>,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nexus-c2-{}", Uuid::new_v4()));
        let registry = Arc::new(WorkspaceAuthorityRegistry::new());
        let authority = BuilderWorkspaceAuthority::provision(registry, &path).unwrap();
        Self {
            path,
            authority,
            events: Arc::new(Mutex::new(vec![])),
        }
    }
    fn audit(&self) -> Audit {
        let events = Arc::clone(&self.events);
        Arc::new(move |event| events.lock().unwrap().push(event))
    }
    fn begin(&self) -> PlanningExecution {
        self.authority.begin(self.audit()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn generated() -> GeneratedPlan {
    GeneratedPlan {
        result: serde_json::from_value(json!({
            "plan": {"product_brief": {"project_name":"../../model-selected", "project_type":"site", "target_audience":"people", "sections":[], "design_direction":"../outside", "tone":"clear", "template_suggestion":"/tmp/escape", "estimated_cost":"0", "estimated_time":"0"},
                "acceptance_criteria":{"must_have":[],"must_not_have":[],"constraints":[]}},
            "input_tokens":1,"output_tokens":2,"cost_usd":0.1,"elapsed_seconds":0.0
        })).unwrap(),
        selection: ModelSelection { provider: ProviderType::Ollama, model_id:"test".into(), display_name:"Test".into(), estimated_cost:0.0, is_local:true }
    }
}
fn unknown() -> WorkspaceGrantId {
    serde_json::from_value(json!(Uuid::new_v4())).unwrap()
}
fn revoked(registry: &WorkspaceAuthorityRegistry, id: WorkspaceGrantId, binding: WorkspaceBinding) {
    assert_eq!(
        registry.resolve(id, binding).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant
    );
}

#[test]
fn p0_002c2_trusted_run_persists_only_fixed_files_and_revokes_before_response() {
    let f = Fixture::new();
    let e = f.begin();
    assert_ne!(e.project_id, e.planner.run_id);
    assert_ne!(e.allocator.agent_id, e.planner.agent_id);
    assert_eq!(e.allocator.run_id, e.planner.run_id);
    assert_eq!(e.project_id.get_version_num(), 4);
    assert_eq!(e.planner.run_id.get_version_num(), 4);
    let (parent, owner, child, planner, run) = (
        e.allocation,
        e.allocator,
        e.project.unwrap(),
        e.planner,
        e.planner.run_id,
    );
    let response = finish_plan(e, "use /outside or ../caller-project", |_| Ok(generated()))
        .unwrap()
        .into_json();
    let id = Uuid::parse_str(response["project_id"].as_str().unwrap()).unwrap();
    let root = PathBuf::from(response["project_dir"].as_str().unwrap());
    assert_eq!(root, f.authority.root.join(id.to_string()));
    let state = web_builder_agent::project::load_project_state(&root).unwrap();
    assert_eq!(state.status, ProjectStatus::Planned);
    assert_eq!(state.project_id, id.to_string());
    let plan = web_builder_agent::plan::load_plan_artefacts(&root).unwrap();
    assert_eq!(plan.product_brief.project_name, "../../model-selected");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    assert_eq!(
        std::fs::read_dir(root.join("artefacts")).unwrap().count(),
        2
    );
    revoked(&f.authority.registry, parent, owner);
    revoked(&f.authority.registry, child, planner);
    for payload in [
        response.to_string(),
        std::fs::read_to_string(root.join("builder_state.json")).unwrap(),
        serde_json::to_string(&*f.events.lock().unwrap()).unwrap(),
    ] {
        for private in [
            run.to_string(),
            owner.agent_id.to_string(),
            planner.agent_id.to_string(),
            serde_json::to_value(parent)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
            serde_json::to_value(child)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
        ] {
            assert!(!payload.contains(&private));
        }
    }
    // Descriptive ID/path and saved data cannot import authority into any registry.
    let forged: WorkspaceGrantId = serde_json::from_value(response["project_id"].clone()).unwrap();
    assert_eq!(
        f.authority.registry.resolve(forged, planner).unwrap_err(),
        WorkspaceAuthorityError::UnknownGrant
    );
    assert!(serde_json::from_value::<WorkspaceGrantId>(response["project_dir"].clone()).is_err());
    assert!(
        serde_json::from_value::<WorkspaceGrantId>(serde_json::to_value(state).unwrap()).is_err()
    );
    assert_eq!(
        WorkspaceAuthorityRegistry::new()
            .resolve(child, planner)
            .unwrap_err(),
        WorkspaceAuthorityError::UnknownGrant
    );
}

#[test]
fn p0_002c2_collision_does_not_reuse_state_and_revokes_issued_parent() {
    let f = Fixture::new();
    let project = Uuid::new_v4();
    let existing = f.path.join(project.to_string());
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(
        existing.join("builder_state.json"),
        "do not load or overwrite",
    )
    .unwrap();
    assert!(matches!(
        f.authority
            .allocate(project, Uuid::new_v4(), None, f.audit()),
        Err(PlanningError::Persistence(_))
    ));
    assert_eq!(
        std::fs::read_to_string(existing.join("builder_state.json")).unwrap(),
        "do not load or overwrite"
    );
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["operation"] == "builder.planning.revoke" && v["outcome"] == "revoked"));
}

#[test]
fn p0_002c2_concurrent_runs_receive_independent_roots_and_runs() {
    let f = Fixture::new();
    let authority = &f.authority;
    let results = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let e = authority.begin(Arc::new(|_| {})).unwrap();
                    let run = e.planner.run_id;
                    let result = finish_plan(e, "same prompt", |_| Ok(generated())).unwrap();
                    (result.project_dir, run)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    let roots: std::collections::HashSet<_> = results.iter().map(|v| &v.0).collect();
    let runs: std::collections::HashSet<_> = results.iter().map(|v| v.1).collect();
    assert_eq!(roots.len(), 8);
    assert_eq!(runs.len(), 8);
}

#[test]
fn p0_002c2_missing_unknown_wrong_owner_wrong_run_and_revocation_deny_mutation() {
    for case in ["missing", "unknown", "owner", "run", "revoked", "ancestor"] {
        let f = Fixture::new();
        let mut e = f.begin();
        match case {
            "missing" => e.project = None,
            "unknown" => e.project = Some(unknown()),
            "owner" => e.planner.agent_id = Uuid::new_v4(),
            "run" => e.planner.run_id = Uuid::new_v4(),
            "revoked" => f
                .authority
                .registry
                .revoke(e.project.unwrap(), e.planner)
                .unwrap(),
            "ancestor" => f
                .authority
                .registry
                .revoke(e.allocation, e.allocator)
                .unwrap(),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                e.write("builder_state.json", b"forbidden"),
                Err(PlanningError::Authority(_))
            ),
            "{case}"
        );
        assert_eq!(std::fs::read_dir(&e.root).unwrap().count(), 0);
        assert_eq!(
            f.events.lock().unwrap().last().unwrap()["outcome"],
            "denied"
        );
    }
}

#[test]
fn p0_002c2_readonly_and_deny_cannot_write_or_create_artefacts() {
    for permission in [FsPermissionLevel::ReadOnly, FsPermissionLevel::Deny] {
        let f = Fixture::new();
        let mut e = f.begin();
        e.project = Some(
            f.authority
                .registry
                .narrow(
                    e.project.unwrap(),
                    e.planner,
                    e.planner.agent_id,
                    &e.root,
                    permission,
                )
                .unwrap(),
        );
        assert!(matches!(
            e.create_artefacts(),
            Err(PlanningError::Authority(_))
        ));
        assert!(matches!(
            e.write("builder_state.json", b"denied"),
            Err(PlanningError::Authority(_))
        ));
        assert_eq!(std::fs::read_dir(&e.root).unwrap().count(), 0);
    }
}

#[test]
fn p0_002c2_expired_authority_prevents_provider_dispatch_and_mutation() {
    let f = Fixture::new();
    let expiry = SystemTime::now() + std::time::Duration::from_secs(1);
    let e = f
        .authority
        .allocate(Uuid::new_v4(), Uuid::new_v4(), Some(expiry), f.audit())
        .unwrap();
    // Synchronize with the actual expiry condition, not an assumed fixed sleep.
    let watchdog = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while SystemTime::now() < expiry {
        assert!(std::time::Instant::now() < watchdog);
        std::thread::yield_now();
    }
    assert!(matches!(
        e.create_artefacts(),
        Err(PlanningError::Authority(_))
    ));
    let mut calls = 0;
    assert!(matches!(
        finish_plan(e, "prompt", |_| {
            calls += 1;
            Ok(generated())
        }),
        Err(PlanningError::Authority(_))
    ));
    assert_eq!(calls, 0);
}

#[test]
fn p0_002c2_missing_storage_is_not_recreated_and_prevents_provider_dispatch() {
    let f = Fixture::new();
    std::fs::remove_dir(&f.path).unwrap();
    let mut calls = 0;
    assert!(matches!(
        run_plan(&f.authority, f.audit(), "prompt", |_| {
            calls += 1;
            Ok(generated())
        }),
        Err(PlanningError::Authority(_))
    ));
    assert_eq!(calls, 0);
    assert!(!f.path.exists());
    assert!(BuilderWorkspaceAuthority::provision(
        Arc::clone(&f.authority.registry),
        Path::new(".")
    )
    .is_err());
}

#[test]
fn p0_002c2_removed_project_root_is_never_recreated_even_for_planfailed() {
    for provider_success in [true, false] {
        let f = Fixture::new();
        let e = f.begin();
        let root = e.root.clone();
        let mut calls = 0;
        let result = finish_plan(e, "prompt", |_| {
            calls += 1;
            if provider_success || calls == 2 {
                std::fs::remove_dir(&root).unwrap();
            }
            if provider_success {
                Ok(generated())
            } else {
                Err("provider error".into())
            }
        });
        assert!(matches!(result, Err(PlanningError::Authority(_))));
        assert!(!root.exists());
        assert_eq!(calls, if provider_success { 1 } else { 2 });
    }
}

#[test]
fn p0_002c2_relative_targets_reject_absolute_parent_and_prefix_escape() {
    let f = Fixture::new();
    let e = f.begin();
    let sibling = f.path.join(format!("{}-evil", e.project_id));
    std::fs::create_dir(&sibling).unwrap();
    let outside = sibling.join("bad.json");
    for target in [
        outside.to_str().unwrap(),
        "../bad.json",
        "artefacts/../../bad.json",
        "",
        ".",
    ] {
        assert!(e.write(target, b"denied").is_err());
    }
    assert!(!outside.exists());
    assert_eq!(std::fs::read_dir(&e.root).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn p0_002c2_symlink_escape_and_replaced_canonical_root_are_denied() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let e = f.begin();
    let outside = f.path.join("outside");
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, e.root.join("artefacts")).unwrap();
    assert!(matches!(
        e.write("artefacts/product_brief.json", b"denied"),
        Err(PlanningError::Authority(_))
    ));
    std::fs::remove_file(e.root.join("artefacts")).unwrap();
    std::fs::remove_dir(&e.root).unwrap();
    symlink(&outside, &e.root).unwrap();
    assert!(matches!(
        e.write("builder_state.json", b"denied"),
        Err(PlanningError::Authority(_))
    ));
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
}

#[test]
fn p0_002c2_each_project_mutation_resolves_current_authority() {
    for boundary in ["mkdir", "brief", "criteria", "planned", "failed"] {
        let f = Fixture::new();
        let e = f.begin();
        if boundary != "mkdir" {
            e.create_artefacts().unwrap();
        }
        if matches!(boundary, "criteria" | "planned") {
            e.write("artefacts/product_brief.json", b"brief").unwrap();
        }
        if boundary == "planned" {
            e.write("artefacts/acceptance_criteria.json", b"criteria")
                .unwrap();
        }
        f.authority
            .registry
            .revoke(e.project.unwrap(), e.planner)
            .unwrap();
        let result = match boundary {
            "mkdir" => e.create_artefacts(),
            "brief" => e.write("artefacts/product_brief.json", b"denied"),
            "criteria" => e.write("artefacts/acceptance_criteria.json", b"denied"),
            _ => {
                let mut state = create_project(&e.project_id.to_string(), "prompt");
                transition(
                    &mut state,
                    if boundary == "failed" {
                        ProjectStatus::PlanFailed
                    } else {
                        ProjectStatus::Planned
                    },
                )
                .unwrap();
                e.save_state(&state)
            }
        };
        assert!(
            matches!(result, Err(PlanningError::Authority(_))),
            "{boundary}"
        );
        let missing = match boundary {
            "mkdir" => "artefacts",
            "brief" => "artefacts/product_brief.json",
            "criteria" => "artefacts/acceptance_criteria.json",
            _ => "builder_state.json",
        };
        assert!(!e.root.join(missing).exists());
    }
}

#[test]
fn p0_002c2_authority_denial_never_retries_or_writes_raw_planfailed() {
    for initial_denial in [true, false] {
        let f = Fixture::new();
        let e = f.begin();
        let (parent, owner, root) = (e.allocation, e.allocator, e.root.clone());
        if initial_denial {
            f.authority.registry.revoke(parent, owner).unwrap();
        }
        let mut calls = 0;
        let result = finish_plan(e, "prompt", |_| {
            calls += 1;
            f.authority.registry.revoke(parent, owner).unwrap();
            Err("provider failure after revocation".into())
        });
        assert!(matches!(result, Err(PlanningError::Authority(_))));
        assert_eq!(calls, usize::from(!initial_denial));
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
}

#[test]
fn p0_002c2_persistence_failure_is_terminal_and_revokes_without_retry() {
    let f = Fixture::new();
    let e = f.begin();
    let (parent, owner, root) = (e.allocation, e.allocator, e.root.clone());
    let mut calls = 0;
    let result = finish_plan(e, "prompt", |_| {
        calls += 1;
        // A deterministic filesystem failure, without permission/OS assumptions.
        std::fs::create_dir(root.join("builder_state.json")).unwrap();
        Ok(generated())
    });
    assert!(matches!(result, Err(PlanningError::Persistence(_))));
    assert_eq!(calls, 1);
    assert!(root.join("builder_state.json").is_dir());
    revoked(&f.authority.registry, parent, owner);
}

#[test]
fn p0_002c2_provider_failure_writes_authorized_planfailed_and_revokes() {
    let f = Fixture::new();
    let e = f.begin();
    let (parent, owner, root) = (e.allocation, e.allocator, e.root.clone());
    let mut attempts = vec![];
    let result = finish_plan(e, "prompt", |fallback| {
        attempts.push(fallback);
        Err("generation failed".into())
    });
    assert!(matches!(result, Err(PlanningError::Provider(_))));
    assert_eq!(attempts, [false, true]);
    assert_eq!(
        web_builder_agent::project::load_project_state(&root)
            .unwrap()
            .status,
        ProjectStatus::PlanFailed
    );
    revoked(&f.authority.registry, parent, owner);
}

#[test]
fn p0_002c2_fallback_success_remains_governed() {
    let f = Fixture::new();
    let mut attempts = vec![];
    assert!(run_plan(&f.authority, f.audit(), "prompt", |fallback| {
        attempts.push(fallback);
        if fallback {
            Ok(generated())
        } else {
            Err("provider unavailable".into())
        }
    })
    .is_ok());
    assert_eq!(attempts, [false, true]);
}

#[test]
fn p0_002c2_drop_early_return_unwind_and_missing_receiver_revoke() {
    for panic in [false, true] {
        let f = Fixture::new();
        let e = f.begin();
        let (parent, owner) = (e.allocation, e.allocator);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if panic {
                let _ = finish_plan(e, "prompt", |_| panic!("provider panic"));
            } else {
                let _owned = e;
            }
        }));
        assert_eq!(outcome.is_err(), panic);
        revoked(&f.authority.registry, parent, owner);
    }
    let f = Fixture::new();
    let e = f.begin();
    let (parent, owner) = (e.allocation, e.allocator);
    let (tx, rx) = std::sync::mpsc::channel();
    drop(rx);
    assert!(tx
        .send(finish_plan(e, "prompt", |_| Ok(generated())))
        .is_err());
    revoked(&f.authority.registry, parent, owner);
}

#[test]
fn p0_002c2_cleanup_error_cannot_return_success() {
    let f = Fixture::new();
    let mut e = f.begin();
    let real = e.allocation;
    let owner = e.allocator;
    e.allocation = unknown();
    assert!(matches!(
        finish_plan(e, "prompt", |_| Ok(generated())),
        Err(PlanningError::Authority(_))
    ));
    f.authority.registry.revoke(real, owner).unwrap();
}

#[test]
fn p0_002c2_audit_can_resolve_registry_and_never_exposes_authority() {
    let f = Fixture::new();
    let registry = Arc::clone(&f.authority.registry);
    let witness = f.begin();
    let id = witness.project.unwrap();
    let binding = witness.planner;
    let audit = Arc::new(move |event: Value| {
        assert!(registry.resolve(id, binding).is_ok());
        assert_eq!(event.as_object().unwrap().len(), 3);
    });
    assert!(run_plan(&f.authority, audit, "prompt", |_| Ok(generated())).is_ok());
}

#[test]
fn p0_002c2_cwd_independence() {
    const FLAG: &str = "NEXUS_C2_CWD_TEST";
    const NAME: &str = "builder_workspace::tests::p0_002c2_cwd_independence";
    if std::env::var(FLAG).as_deref() == Ok(NAME) {
        let f = Fixture::new();
        let e = f.begin();
        let root = e.root.clone();
        let other = Fixture::new();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&other.path).unwrap();
        let result = finish_plan(e, "prompt", |_| Ok(generated()));
        std::env::set_current_dir(original).unwrap();
        assert_eq!(PathBuf::from(result.unwrap().project_dir), root);
        eprintln!("C2 cwd witness");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env(FLAG, NAME)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("C2 cwd witness"));
}

#[test]
fn p0_002c2_narrowing_failure_revokes_and_never_dispatches_provider() {
    let mut f = Fixture::new();
    f.authority.planner = Uuid::nil(); // C1 must reject this invalid backend binding.
    let mut calls = 0;
    assert!(matches!(
        run_plan(&f.authority, f.audit(), "prompt", |_| {
            calls += 1;
            Ok(generated())
        }),
        Err(PlanningError::Authority(_))
    ));
    assert_eq!(calls, 0);
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["operation"] == "builder.planning.revoke" && v["outcome"] == "revoked"));
}

#[test]
fn p0_002c2_nonmatching_or_nondirectory_root_is_denied() {
    let f = Fixture::new();
    let mut e = f.begin();
    let real = e.root.clone();
    e.root = f.authority.root.clone();
    assert!(matches!(
        e.write("builder_state.json", b"denied"),
        Err(PlanningError::Authority(_))
    ));
    e.root = real;
    // The retained Windows directory handle permits rename but a deleted entry
    // can remain delete-pending until close. Replace the pathname portably.
    std::fs::rename(&e.root, e.root.with_extension("original")).unwrap();
    std::fs::write(&e.root, "replacement evidence").unwrap();
    assert!(matches!(
        e.create_artefacts(),
        Err(PlanningError::Authority(_))
    ));
    assert_eq!(
        std::fs::read_to_string(&e.root).unwrap(),
        "replacement evidence"
    );
}

#[test]
fn p0_002c2_revocation_after_provider_response_prevents_success_and_planfailed() {
    for success in [true, false] {
        let f = Fixture::new();
        let e = f.begin();
        let (root, parent, owner) = (e.root.clone(), e.allocation, e.allocator);
        let mut calls = 0;
        let result = finish_plan(e, "prompt", |fallback| {
            calls += 1;
            if success || fallback {
                f.authority.registry.revoke(parent, owner).unwrap();
            }
            if success {
                Ok(generated())
            } else {
                Err("provider failure".into())
            }
        });
        assert!(matches!(result, Err(PlanningError::Authority(_))));
        assert_eq!(calls, if success { 1 } else { 2 });
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
        revoked(&f.authority.registry, parent, owner);
    }
}

#[test]
fn p0_002c2_appstate_requires_explicit_storage_and_shares_registry_with_adapter() {
    let f = Fixture::new();
    let mut state = crate::AppState::new_in_memory();
    assert!(generate_plan(&state, "prompt")
        .unwrap_err()
        .contains("storage"));
    state.builder_workspace = Ok(Arc::new(
        BuilderWorkspaceAuthority::provision(Arc::clone(&state.workspace_authority), &f.path)
            .unwrap(),
    ));
    let cloned = state.clone();
    let adapter = cloned.builder_workspace.as_ref().unwrap();
    assert!(Arc::ptr_eq(&state.workspace_authority, &adapter.registry));
    let e = adapter.begin(f.audit()).unwrap();
    state
        .workspace_authority
        .revoke(e.allocation, e.allocator)
        .unwrap();
    assert!(matches!(
        e.create_artefacts(),
        Err(PlanningError::Authority(_))
    ));
    state.shutdown_oracle_runtime();
}

#[test]
fn p0_002c2_real_success_pipeline_rechecks_between_every_mutation() {
    for (previous, denied_target) in [
        ("mkdir.artefacts", "artefacts/product_brief.json"),
        (
            "artefacts/product_brief.json",
            "artefacts/acceptance_criteria.json",
        ),
        ("artefacts/acceptance_criteria.json", "builder_state.json"),
    ] {
        let f = Fixture::new();
        let mut e = f.begin();
        let (parent, owner, root) = (e.allocation, e.allocator, e.root.clone());
        let registry = Arc::clone(&e.registry);
        e.audit = Arc::new(move |event| {
            // Revoke after the preceding authorization snapshot. That operation
            // may finish; the next independently authorized mutation must deny.
            if event["operation"] == format!("builder.planning.{previous}") {
                registry.revoke(parent, owner).unwrap();
            }
        });
        let mut calls = 0;
        let result = finish_plan(e, "prompt", |_| {
            calls += 1;
            Ok(generated())
        });
        assert!(
            matches!(result, Err(PlanningError::Authority(_))),
            "{denied_target}"
        );
        assert_eq!(calls, 1);
        assert!(!root.join(denied_target).exists());
        revoked(&f.authority.registry, parent, owner);
    }
}

// C3 fixtures register through the real C2 pipeline. Only the test fixture
// creates React output: the C3 command must never create it or its parents.
fn registered(f: &Fixture) -> (String, PathBuf) {
    let result = run_plan(&f.authority, f.audit(), "site", |_| Ok(generated())).unwrap();
    let root = PathBuf::from(result.project_dir);
    std::fs::create_dir(root.join("react")).unwrap();
    (result.project_id, root)
}

#[test]
fn p0_002c3_registration_follows_persistence_and_revocation() {
    let f = Fixture::new();
    let mut e = f.begin();
    let (id, parent, owner, root) = (e.project_id, e.allocation, e.allocator, e.root.clone());
    assert!(f.authority.catalog.lookup(id).is_err());
    let registry = Arc::clone(&f.authority.registry);
    let catalog = Arc::clone(&f.authority.catalog);
    e.audit = Arc::new(move |event| {
        if event["operation"] == "builder.registration" && event["outcome"] == "registered" {
            revoked(&registry, parent, owner);
            assert!(root.join("builder_state.json").is_file());
            assert!(root.join("artefacts/acceptance_criteria.json").is_file());
            assert!(catalog.lookup(id).is_ok());
        } else {
            assert!(catalog.lookup(id).is_err());
        }
    });
    // Completion is after publication too.
    let old = Arc::clone(&e.audit);
    e.audit = Arc::new(move |event| {
        if event["operation"] != "builder.planning.complete" {
            old(event);
        }
    });
    finish_plan(e, "site", |_| Ok(generated())).unwrap();
    assert!(f.authority.catalog.lookup(id).is_ok());
}

#[test]
fn p0_002c3_failed_planning_never_registers() {
    for case in [
        "provider",
        "persistence",
        "authority",
        "revoke",
        "registration",
    ] {
        let f = Fixture::new();
        let mut e = f.begin();
        let (id, root, parent, owner) = (e.project_id, e.root.clone(), e.allocation, e.allocator);
        if case == "authority" {
            f.authority.registry.revoke(parent, owner).unwrap();
        }
        if case == "revoke" {
            e.allocation = unknown();
        }
        if case == "registration" {
            let root = root.clone();
            e.audit = Arc::new(move |event| {
                if event["operation"] == "builder.planning.revoke" {
                    std::fs::rename(&root, root.with_extension("persisted")).unwrap();
                    std::fs::create_dir(&root).unwrap();
                }
            });
        }
        let result = finish_plan(e, "site", |_| {
            if case == "provider" {
                return Err("provider failure".into());
            }
            if case == "persistence" {
                std::fs::create_dir(root.join("builder_state.json")).unwrap();
            }
            Ok(generated())
        });
        assert!(result.is_err(), "{case}");
        assert!(f.authority.catalog.lookup(id).is_err(), "{case}");
        if case == "revoke" {
            f.authority.registry.revoke(parent, owner).unwrap();
        }
        revoked(&f.authority.registry, parent, owner);
        if case == "registration" {
            assert!(root
                .with_extension("persisted")
                .join("builder_state.json")
                .is_file());
        }
    }
}

#[test]
fn p0_002c3_catalog_is_not_reconstructed_from_descriptive_data() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let arbitrary = Uuid::new_v4();
    let forged = f.authority.root.join(arbitrary.to_string());
    std::fs::create_dir_all(forged.join("react")).unwrap();
    std::fs::write(
        forged.join("builder_state.json"),
        format!(
            r#"{{"project_id":"{arbitrary}","project_dir":"{}"}}"#,
            root.display()
        ),
    )
    .unwrap();
    std::fs::write(forged.join("project.json"), "{}").unwrap();
    for selector in [
        arbitrary.to_string(),
        root.to_string_lossy().into_owned(),
        "../escape".into(),
        "not-a-uuid".into(),
    ] {
        let count = f.events.lock().unwrap().len();
        assert!(f
            .authority
            .write_file(&selector, "file", b"denied", f.audit())
            .is_err());
        assert!(!f.events.lock().unwrap()[count..]
            .iter()
            .any(|v| v["operation"] == "builder.write.issue"));
    }
    assert!(!forged.join("react/file").exists());
    let restarted =
        BuilderWorkspaceAuthority::provision(Arc::new(WorkspaceAuthorityRegistry::new()), &f.path)
            .unwrap();
    assert!(restarted.catalog.projects.lock().unwrap().is_empty());
    assert!(restarted
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(root.join("builder_state.json").is_file());
}

#[test]
fn p0_002c3_duplicate_and_conflicting_registration_cannot_overwrite() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let id = Uuid::parse_str(&id).unwrap();
    let original = f.authority.catalog.lookup(id).unwrap();
    assert!(f.authority.catalog.publish(Arc::clone(&original)).is_err());
    let other = f.authority.root.join("different");
    std::fs::create_dir(&other).unwrap();
    let conflicting = Arc::new(RegisteredBuilderProject {
        project_id: id,
        root: other.clone(),
        storage_root: f.authority.root.clone(),
        storage_identity: Arc::clone(&f.authority.storage_identity),
        identity: DirectoryIdentity::capture(&other).unwrap(),
    });
    assert!(f.authority.catalog.publish(conflicting).is_err());
    assert!(Arc::ptr_eq(
        &f.authority.catalog.lookup(id).unwrap(),
        &original
    ));
    f.authority.catalog.invalidate(&original).unwrap();
    assert!(f.authority.catalog.publish(original).is_err());
}

#[test]
fn p0_002c3_project_removal_and_same_path_replacement_permanently_invalidate() {
    for replacement in [false, true] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let project = f
            .authority
            .catalog
            .lookup(Uuid::parse_str(&id).unwrap())
            .unwrap();
        project.validate_identity().unwrap();
        let weak = Arc::downgrade(&project);
        drop(project);
        std::fs::rename(&root, root.with_extension("original")).unwrap();
        if replacement {
            std::fs::create_dir_all(root.join("react")).unwrap();
        }
        assert!(f
            .authority
            .write_file(&id, "file", b"denied", f.audit())
            .is_err());
        assert!(weak.upgrade().is_none()); // retained project witness released
        if replacement {
            std::fs::remove_dir_all(&root).unwrap();
        }
        assert!(!root.exists()); // no authority lookup has recreated it
        std::fs::rename(root.with_extension("original"), &root).unwrap();
        assert!(f
            .authority
            .write_file(&id, "file", b"still denied", f.audit())
            .is_err());
        assert!(!root.join("react/file").exists());
        assert!(f
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|v| v["operation"] == "builder.registration.invalidate"));
    }
}

#[test]
fn p0_002c3_missing_and_replaced_storage_deny_without_recreation() {
    for replacement in [false, true] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let old = f.path.with_extension("old");
        let detached_project = f.path.with_extension("project");
        // Windows permits moving the observed directory itself with share-delete,
        // but not an ancestor containing an open descendant directory. Move the
        // still-observed project first; keep its identity intact for this test.
        std::fs::rename(&root, &detached_project).unwrap();
        std::fs::rename(&f.path, &old).unwrap();
        if replacement {
            std::fs::create_dir(&f.path).unwrap();
            std::fs::rename(&detached_project, &root).unwrap();
            // The project still matches: denial must detect the replaced storage.
            f.authority
                .catalog
                .lookup(Uuid::parse_str(&id).unwrap())
                .unwrap()
                .identity
                .validate(&root)
                .unwrap();
        }
        assert!(f
            .authority
            .write_file(&id, "file", b"denied", f.audit())
            .is_err());
        assert!(f
            .authority
            .catalog
            .lookup(Uuid::parse_str(&id).unwrap())
            .is_err());
        assert!(!root.join("react/file").exists());
        if !replacement {
            assert!(!f.path.exists());
            std::fs::remove_dir_all(&detached_project).unwrap();
        }
        std::fs::remove_dir(old).unwrap();
    }
}

#[test]
fn p0_002c3_write_uses_fresh_react_grant_and_revokes_before_success() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    assert_ne!(f.authority.writer, f.authority.planner);
    assert_ne!(f.authority.writer, f.authority.allocator);
    assert_ne!(f.authority.writer.to_string(), id);
    let mut e = f.authority.begin_write(&id, "App.tsx", f.audit()).unwrap();
    let (grant, binding) = (e.grant, e.binding);
    let snapshot = f.authority.registry.resolve(grant, binding).unwrap();
    assert_eq!(snapshot.root(), root.join("react"));
    assert_eq!(snapshot.parent(), None);
    assert_eq!(
        snapshot.source(),
        WorkspaceAuthoritySource::BackendAllocated
    );
    assert!(snapshot.expires_at().is_some());
    let registry = Arc::clone(&f.authority.registry);
    e.audit = Arc::new(move |event| {
        if event["operation"] == "builder.write.complete" {
            assert_eq!(event["outcome"], "succeeded");
            revoked(&registry, grant, binding);
        }
    });
    e.finish("App.tsx", b"first").unwrap();
    f.authority
        .write_file(&id, "App.tsx", b"replacement", f.audit())
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("react/App.tsx")).unwrap(),
        b"replacement"
    );
}

#[test]
fn p0_002c3_wrong_binding_and_inactive_or_restricted_grants_deny() {
    for case in [
        "owner", "run", "revoked", "expired", "readonly", "deny", "root",
    ] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let mut e = f.authority.begin_write(&id, "file", f.audit()).unwrap();
        let (original, owner) = (e.grant, e.binding);
        match case {
            "owner" => e.binding.agent_id = Uuid::new_v4(),
            "run" => e.binding.run_id = Uuid::new_v4(),
            "revoked" => f.authority.registry.revoke(e.grant, e.binding).unwrap(),
            "expired" | "readonly" | "deny" | "root" => {
                let permission = match case {
                    "readonly" => FsPermissionLevel::ReadOnly,
                    "deny" => FsPermissionLevel::Deny,
                    _ => FsPermissionLevel::ReadWrite,
                };
                let expiry =
                    (case == "expired").then(|| SystemTime::now() + Duration::from_secs(1));
                e.grant = f
                    .authority
                    .registry
                    .issue_trusted_root(
                        if case == "root" { &root } else { &e.root },
                        e.binding,
                        WorkspaceAuthoritySource::BackendAllocated,
                        permission,
                        expiry,
                    )
                    .unwrap();
                if let Some(expiry) = expiry {
                    let deadline = std::time::Instant::now() + Duration::from_secs(5);
                    while SystemTime::now() < expiry {
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::yield_now();
                    }
                }
            }
            _ => unreachable!(),
        }
        assert!(e.mutate("file", b"denied").is_err(), "{case}");
        e.binding = owner;
        e.revoke().unwrap();
        f.authority.registry.revoke(original, owner).unwrap();
        assert!(!root.join("react/file").exists());
    }
}

#[test]
fn p0_002c3_portable_lexical_contract() {
    for path in [
        "",
        "/file",
        "file/",
        "a//b",
        ".",
        "..",
        "a/./b",
        "a/../b",
        "../sibling-prefix/file",
        "C:file",
        "C:/file",
        "\\file",
        "\\\\host\\file",
        "\\\\?\\C:\\file",
        "\\\\.\\device",
        "file:stream",
        "a\\b",
        "a\0b",
        "a\nb",
        "a\u{7f}b",
        "a\u{85}b",
        "a<b",
        "a>b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        "file.",
        "file ",
        "a./b",
        "a /b",
        "CON",
        "con.txt",
        "CON .txt",
        "a/AUX/b",
        "NUL.ts",
        "prn",
        "COM1",
        "LPT9.txt",
        "COM¹",
        "LPT².ext",
        "CONIN$",
        "CONOUT$",
    ] {
        assert!(validate_relative_file(path).is_err(), "{path:?}");
    }
    for path in [
        "App.tsx",
        "src/components/Card.tsx",
        ".gitignore",
        "résumé.txt",
        "COM10",
        "file%2fname",
        "%2e%2e/literal",
    ] {
        validate_relative_file(path).unwrap();
    }
}

#[test]
fn p0_002c3_missing_parents_and_react_are_never_created() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    assert!(f
        .authority
        .write_file(&id, "nested/file", b"denied", f.audit())
        .is_err());
    assert!(!root.join("react/nested").exists());
    std::fs::create_dir(root.join("react/nested")).unwrap();
    f.authority
        .write_file(&id, "nested/file", b"ok", f.audit())
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("react/nested/file")).unwrap(),
        b"ok"
    );
    assert!(f
        .authority
        .write_file(&id, "nested", b"denied", f.audit())
        .is_err());
    std::fs::remove_dir_all(root.join("react")).unwrap();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(!root.join("react").exists());
    std::fs::write(root.join("react"), b"not a directory").unwrap();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
}

#[test]
fn p0_002c3_audit_reentry_and_final_checks_close_callback_mutation_window() {
    for case in ["grant", "project", "react", "parent", "target"] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let react = root.join("react");
        std::fs::create_dir(react.join("nested")).unwrap();
        let mut e = f
            .authority
            .begin_write(&id, "nested/file", f.audit())
            .unwrap();
        let (grant, binding) = (e.grant, e.binding);
        let registry = Arc::clone(&f.authority.registry);
        let catalog = Arc::clone(&f.authority.catalog);
        let callback_root = root.clone();
        e.audit = Arc::new(move |event| {
            // Real catalog/registry access at every audit boundary, no locks held.
            let _ = catalog.lookup(Uuid::parse_str(&id).unwrap());
            let _ = registry.resolve(grant, binding);
            if event["operation"] == "builder.write.authorize" && event["outcome"] == "authorized" {
                match case {
                    "grant" => registry.revoke(grant, binding).unwrap(),
                    "project" => {
                        // Keep the same React directory/witness, while replacing
                        // only its project ancestor. Moving the observed child
                        // first also permits this namespace change on Windows.
                        let detached_react = callback_root.with_extension("react");
                        std::fs::rename(&react, &detached_react).unwrap();
                        std::fs::rename(&callback_root, callback_root.with_extension("old"))
                            .unwrap();
                        std::fs::create_dir(&callback_root).unwrap();
                        std::fs::rename(&detached_react, &react).unwrap();
                    }
                    "react" => {
                        std::fs::rename(&react, callback_root.join("old-react")).unwrap();
                        std::fs::create_dir_all(react.join("nested")).unwrap();
                    }
                    "parent" => std::fs::remove_dir(react.join("nested")).unwrap(),
                    "target" => std::fs::create_dir(react.join("nested/file")).unwrap(),
                    _ => unreachable!(),
                }
            }
        });
        assert!(e.finish("nested/file", b"denied").is_err(), "{case}");
        revoked(&f.authority.registry, grant, binding);
        assert!(!root.join("react/nested/file").is_file());
    }
}

#[test]
fn p0_002c3_cleanup_covers_write_failure_validation_failure_and_unwind() {
    for case in ["io", "validation", "panic"] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let e = f.authority.begin_write(&id, "file", f.audit()).unwrap();
        let (grant, binding) = (e.grant, e.binding);
        if case == "io" {
            let file = root.join("react/file");
            std::fs::write(&file, b"preserved").unwrap();
            let original = std::fs::metadata(&file).unwrap().permissions();
            let mut readonly = original.clone();
            readonly.set_readonly(true);
            std::fs::set_permissions(&file, readonly).unwrap();
            let result = e.finish("file", b"denied");
            std::fs::set_permissions(&file, original).unwrap();
            assert_eq!(result.unwrap_err(), "file write failed");
            assert_eq!(std::fs::read(file).unwrap(), b"preserved");
        } else if case == "validation" {
            assert!(e.finish("missing/file", b"denied").is_err());
        } else {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = e;
                panic!("write panic");
            }))
            .is_err());
        }
        revoked(&f.authority.registry, grant, binding);
    }
}

#[test]
fn p0_002c3_revocation_failure_prevents_success_after_mutation() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let mut e = f.authority.begin_write(&id, "file", f.audit()).unwrap();
    let (grant, binding) = (e.grant, e.binding);
    e.mutate("file", b"persisted evidence").unwrap();
    e.grant = unknown(); // injected finalization failure, after real mutation
    assert_eq!(e.finalize(Ok(())).unwrap_err(), "write revocation failed");
    assert_eq!(
        std::fs::read(root.join("react/file")).unwrap(),
        b"persisted evidence"
    );
    f.authority.registry.revoke(grant, binding).unwrap();
    assert!(!f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["operation"] == "builder.write.complete" && v["outcome"] == "succeeded"));
}

#[test]
fn p0_002c3_concurrent_writes_have_independent_authority() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let barrier = std::sync::Barrier::new(4);
    let credentials = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for index in 0..4 {
            let (f, id, barrier, credentials) = (&f, &id, &barrier, &credentials);
            scope.spawn(move || {
                let name = format!("file{index}");
                let mut e = f.authority.begin_write(id, &name, f.audit()).unwrap();
                credentials.lock().unwrap().push((e.grant, e.binding));
                if index == 0 {
                    e.revoke().unwrap();
                }
                barrier.wait();
                let result = e.finish(&name, b"content");
                assert_eq!(result.is_ok(), index != 0);
            });
        }
    });
    let credentials = credentials.lock().unwrap();
    assert_eq!(
        credentials
            .iter()
            .map(|v| v.0)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        credentials
            .iter()
            .map(|v| v.1.run_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    for (grant, binding) in credentials.iter() {
        revoked(&f.authority.registry, *grant, *binding);
    }
    assert!(!root.join("react/file0").exists());
    for index in 1..4 {
        assert!(root.join(format!("react/file{index}")).is_file());
    }
}

#[test]
fn p0_002c3_no_sensitive_material_in_write_events_or_client_errors() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let e = f.authority.begin_write(&id, "file", f.audit()).unwrap();
    let (grant, binding) = (e.grant, e.binding);
    e.finish("file", b"SECRET CONTENT").unwrap();
    let error = f
        .authority
        .write_file(&id, "/private/caller/root", b"SECRET CONTENT", f.audit())
        .unwrap_err();
    let payload = format!(
        "{error}{}",
        serde_json::to_string(&*f.events.lock().unwrap()).unwrap()
    );
    for sensitive in [
        root.to_string_lossy().into_owned(),
        f.authority.root.to_string_lossy().into_owned(),
        serde_json::to_string(&grant)
            .unwrap()
            .trim_matches('"')
            .to_owned(),
        binding.agent_id.to_string(),
        binding.run_id.to_string(),
        "SECRET CONTENT".into(),
        "/private/caller/root".into(),
    ] {
        assert!(
            !payload.contains(&sensitive),
            "sensitive audit/client output"
        );
    }
}

#[test]
fn p0_002c3_appstate_clones_share_catalog_and_handles() {
    let f = Fixture::new();
    let mut state = crate::AppState::new_in_memory();
    assert!(write_file(&state, "id", "file", "content").is_err());
    state.builder_workspace = Ok(Arc::new(
        BuilderWorkspaceAuthority::provision(Arc::clone(&state.workspace_authority), &f.path)
            .unwrap(),
    ));
    let cloned = state.clone();
    let authority = state.builder_workspace.as_ref().unwrap();
    let clone_authority = cloned.builder_workspace.as_ref().unwrap();
    assert!(Arc::ptr_eq(authority, clone_authority));
    let result = run_plan(authority, f.audit(), "site", |_| Ok(generated())).unwrap();
    std::fs::create_dir(Path::new(&result.project_dir).join("react")).unwrap();
    let id = Uuid::parse_str(&result.project_id).unwrap();
    let project = authority.catalog.lookup(id).unwrap();
    assert!(Arc::ptr_eq(
        &project,
        &clone_authority.catalog.lookup(id).unwrap()
    ));
    write_file(&cloned, &result.project_id, "file", "written by clone").unwrap();
    let restarted = crate::AppState::new_in_memory();
    assert!(write_file(&restarted, &result.project_id, "file", "denied").is_err());
    restarted.shutdown_oracle_runtime();
    state.shutdown_oracle_runtime();
}

#[test]
fn p0_002c3_home_and_cwd_changes_do_not_change_authority() {
    const FLAG: &str = "NEXUS_C3_ENV_TEST";
    const NAME: &str =
        "builder_workspace::tests::p0_002c3_home_and_cwd_changes_do_not_change_authority";
    if std::env::var(FLAG).as_deref() == Ok(NAME) {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let other = Fixture::new();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&other.path).unwrap();
        // This test runs in its own process; environment mutation is isolated.
        std::env::set_var("HOME", &other.path);
        f.authority
            .write_file(&id, "file", b"registered root", f.audit())
            .unwrap();
        std::env::set_current_dir(original).unwrap();
        assert_eq!(
            std::fs::read(root.join("react/file")).unwrap(),
            b"registered root"
        );
        assert_eq!(std::fs::read_dir(&other.path).unwrap().count(), 0);
        eprintln!("C3 environment witness");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env(FLAG, NAME)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("C3 environment witness"));
}

#[cfg(unix)]
#[test]
fn p0_002c3_unix_symlink_containment_and_project_replacement() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join("react");
    let sibling = root.join("react-evil");
    std::fs::create_dir(&sibling).unwrap();
    std::fs::write(sibling.join("file"), b"outside").unwrap();
    symlink(&sibling, react.join("escape")).unwrap();
    symlink(sibling.join("file"), react.join("outside")).unwrap();
    symlink(react.join("missing"), react.join("dangling")).unwrap();
    symlink("/dev/null", react.join("device")).unwrap();
    for path in [
        "escape/file",
        "outside",
        "dangling",
        "device",
        "../react-evil/file",
    ] {
        assert!(
            f.authority
                .write_file(&id, path, b"denied", f.audit())
                .is_err(),
            "{path}"
        );
    }
    assert_eq!(std::fs::read(sibling.join("file")).unwrap(), b"outside");
    std::fs::create_dir(react.join("nested")).unwrap();
    std::fs::write(react.join("nested/file"), b"before").unwrap();
    symlink(react.join("nested/file"), react.join("contained")).unwrap();
    symlink(react.join("nested"), react.join("parent-link")).unwrap();
    f.authority
        .write_file(&id, "contained", b"after", f.audit())
        .unwrap();
    f.authority
        .write_file(&id, "parent-link/new", b"new", f.audit())
        .unwrap();
    assert_eq!(std::fs::read(react.join("nested/file")).unwrap(), b"after");
    assert_eq!(std::fs::read(react.join("nested/new")).unwrap(), b"new");
    std::fs::rename(&root, root.with_extension("old")).unwrap();
    symlink(root.with_extension("old"), &root).unwrap();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_err());
}

#[cfg(unix)]
#[test]
fn p0_002c3_unix_react_redirect_and_special_targets_deny() {
    use std::os::unix::fs::symlink;
    const FLAG: &str = "NEXUS_C3_SOCKET_FIXTURE";
    const NAME: &str =
        "builder_workspace::tests::p0_002c3_unix_react_redirect_and_special_targets_deny";
    if std::env::var(FLAG).as_deref() == Ok(NAME) {
        // Relative binding avoids sockaddr_un pathname limits under macOS's
        // long native temp roots. The child alone has a different cwd.
        let socket = std::os::unix::net::UnixListener::bind("socket").unwrap();
        drop(socket);
        eprintln!("C3 socket fixture");
        return;
    }
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join("react");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env(FLAG, NAME)
        .current_dir(&react)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("C3 socket fixture"));
    assert!(f
        .authority
        .write_file(&id, "socket", b"denied", f.audit())
        .is_err());
    #[cfg(target_os = "macos")]
    type Mode = u16;
    #[cfg(not(target_os = "macos"))]
    type Mode = u32;
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: Mode) -> std::ffi::c_int;
    }
    use std::os::unix::ffi::OsStrExt;
    let fifo = std::ffi::CString::new(react.join("fifo").as_os_str().as_bytes()).unwrap();
    // SAFETY: NUL-terminated owned pathname, valid POSIX mode, no retained pointer.
    assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(f
        .authority
        .write_file(&id, "fifo", b"denied", f.audit())
        .is_err());
    let relocated = root.join("different-react");
    std::fs::rename(&react, &relocated).unwrap();
    symlink(&relocated, &react).unwrap();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(!relocated.join("file").exists());
}

#[cfg(unix)]
#[test]
fn p0_002c3_unix_unavailable_identity_denies_without_fabricated_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let original = std::fs::metadata(&root).unwrap().permissions();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o0)).unwrap();
    let result = f.authority.write_file(&id, "file", b"denied", f.audit());
    std::fs::set_permissions(&root, original).unwrap();
    assert!(result.is_err());
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_ok());
    assert!(!root.join("react/file").exists());
}

#[cfg(unix)]
#[test]
fn p0_002c3_unix_identity_handle_is_close_on_exec() {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...) -> std::ffi::c_int;
    }
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let project = f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .unwrap();
    // F_GETFD and FD_CLOEXEC are both 1 on supported Linux and Darwin targets.
    // SAFETY: a live owned descriptor and the no-argument F_GETFD operation.
    let flags = unsafe { fcntl(project.identity.handle().as_raw_fd(), 1) };
    assert!(flags >= 0);
    assert_ne!(flags & 1, 0);
}

#[cfg(windows)]
#[test]
fn p0_002c3_windows_native_directory_identity_and_noninheritable_handle() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let project = f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .unwrap();
    project.identity.validate(&root).unwrap(); // actual FILE_ID_INFO query
    let mut flags = 0;
    // SAFETY: live retained File handle, valid writable u32 output.
    assert_ne!(
        unsafe { GetHandleInformation(project.identity.handle().as_raw_handle(), &mut flags) },
        0
    );
    assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
    // Share-delete permits rename with the retained handle still alive. Matching
    // canonical path strings must not make a newly allocated directory valid.
    std::fs::rename(&root, root.with_extension("old")).unwrap();
    std::fs::create_dir_all(root.join("react")).unwrap();
    assert_eq!(
        project.identity.validate(&root),
        Err(IdentityError::Changed)
    );
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
}

#[cfg(windows)]
#[test]
fn p0_002c3_windows_native_reparse_project_and_react_redirect_deny() {
    use std::os::windows::fs::symlink_dir;
    for project_replacement in [true, false] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let target = if project_replacement {
            root.clone()
        } else {
            root.join("react")
        };
        let old = target.with_extension("old");
        std::fs::rename(&target, &old).unwrap();
        symlink_dir(&old, &target)
            .expect("native Windows test requires symlink creation privilege");
        assert!(f
            .authority
            .write_file(&id, "file", b"denied", f.audit())
            .is_err());
        assert!(!old.join("file").exists());
        if project_replacement {
            assert!(f
                .authority
                .catalog
                .lookup(Uuid::parse_str(&id).unwrap())
                .is_err());
        }
    }
}

#[test]
fn p0_002c3_panicking_audit_still_revokes_without_double_panic() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let mut e = f.authority.begin_write(&id, "file", f.audit()).unwrap();
    let (grant, binding) = (e.grant, e.binding);
    // Both the normal authorization audit and the Drop revocation audit panic.
    // The second panic must be contained, after the real registry revocation.
    e.audit = Arc::new(|_| panic!("injected audit panic"));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.finish("file", b"denied")))
            .is_err()
    );
    revoked(&f.authority.registry, grant, binding);
    assert!(!root.join("react/file").exists());
}

#[test]
fn p0_002c3_catalog_unavailable_and_issuance_failure_deny() {
    let mut f = Fixture::new();
    let (id, root) = registered(&f);
    f.authority.writer = Uuid::nil();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(f
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["operation"] == "builder.write.issue" && v["outcome"] == "denied"));
    f.authority.writer = Uuid::new_v4();
    let catalog = Arc::clone(&f.authority.catalog);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = catalog.projects.lock().unwrap();
        panic!("catalog unavailable");
    }));
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(!root.join("react/file").exists());
}

#[test]
fn p0_002c3_removed_project_is_not_recreated() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    std::fs::remove_dir_all(&root).unwrap();
    assert!(f
        .authority
        .write_file(&id, "file", b"denied", f.audit())
        .is_err());
    assert!(!root.exists());
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_err());
}

// P0-002C4A: dev-server commands validate C3 provenance and identity, then
// fail closed. No process is launched and nothing is created or mutated.
fn devserver_events(f: &Fixture, from: usize) -> Vec<Value> {
    f.events.lock().unwrap()[from..]
        .iter()
        .filter(|v| {
            v["operation"]
                .as_str()
                .is_some_and(|op| op.starts_with("builder.devserver."))
        })
        .cloned()
        .collect()
}

// Full recursive snapshot (paths, kinds, file bytes) without following links.
fn tree(root: &Path) -> Vec<(PathBuf, String, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if metadata.is_dir() {
                pending.push(path);
                entries.push((relative, "dir".into(), vec![]));
            } else if metadata.is_file() {
                entries.push((relative, "file".into(), std::fs::read(&path).unwrap()));
            } else {
                entries.push((relative, "other".into(), vec![]));
            }
        }
    }
    entries.sort();
    entries
}

fn plant_react_project(react: &Path) {
    std::fs::write(
        react.join("package.json"),
        r#"{"name":"c4a","scripts":{"preinstall":"exit 1","dev":"vite"},"devDependencies":{"vite":"^5.3.4"}}"#,
    )
    .unwrap();
    std::fs::write(react.join("index.html"), "<html></html>").unwrap();
}

fn assert_all_deny(f: &Fixture, selector: &str, expected: &str) {
    let from = f.events.lock().unwrap().len();
    assert_eq!(
        f.authority.dev_server_start(selector, f.audit()),
        expected,
        "{selector:?}"
    );
    assert_eq!(
        f.authority
            .dev_server_stop(selector, f.audit())
            .unwrap_err(),
        expected
    );
    assert_eq!(
        f.authority
            .dev_server_status(selector, f.audit())
            .unwrap_err(),
        expected
    );
    let events = devserver_events(f, from);
    assert_eq!(events.len(), 3, "{selector:?}");
    assert!(events.iter().all(|v| v["outcome"] == "denied"
        && v["reason"] != "launch_unavailable"
        && v["reason"] != "no_owned_server"));
}

#[test]
fn p0_002c4a_registered_project_reaches_launch_unavailable_without_mutation() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    plant_react_project(&root.join("react"));
    let before = tree(&f.path);
    for _ in 0..2 {
        let from = f.events.lock().unwrap().len();
        assert_eq!(
            f.authority.dev_server_start(&id, f.audit()),
            "launch unavailable"
        );
        let events = devserver_events(&f, from);
        assert_eq!(
            events,
            vec![
                json!({"operation": "builder.devserver.start", "project_id": id,
                "outcome": "denied", "reason": "launch_unavailable"})
            ]
        );
    }
    assert_eq!(tree(&f.path), before);
    assert!(!root.join("react/node_modules").exists());
    assert!(!root.join("react/package-lock.json").exists());
    // Launch denial never invalidates or alters the registration.
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_ok());
}

#[test]
fn p0_002c4a_selectors_without_current_registration_deny_every_operation() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    // A legacy/forged on-disk project with React, state and package metadata.
    let legacy = Uuid::new_v4();
    let forged = f.authority.root.join(legacy.to_string());
    std::fs::create_dir_all(forged.join("react")).unwrap();
    plant_react_project(&forged.join("react"));
    std::fs::write(
        forged.join("builder_state.json"),
        format!(
            r#"{{"project_id":"{legacy}","project_dir":"{}"}}"#,
            root.display()
        ),
    )
    .unwrap();
    let before = tree(&f.path);
    for selector in [
        legacy.to_string(),
        Uuid::new_v4().to_string(),
        Uuid::nil().to_string(),
        root.to_string_lossy().into_owned(),
        root.join("react").to_string_lossy().into_owned(),
        forged.to_string_lossy().into_owned(),
        format!("{id}/../{legacy}"),
        "../escape".into(),
        "not-a-uuid".into(),
        String::new(),
        "1234".into(),
        "127.0.0.1:5173".into(),
        "npx vite".into(),
    ] {
        assert_all_deny(&f, &selector, "project not registered");
    }
    // Malformed selectors are never echoed into audit.
    let from = f.events.lock().unwrap().len();
    let _ = f.authority.dev_server_start("../escape", f.audit());
    assert_eq!(devserver_events(&f, from)[0]["project_id"], Value::Null);
    assert_eq!(tree(&f.path), before);
    // A restarted authority over the same storage has an empty catalog.
    let restarted =
        BuilderWorkspaceAuthority::provision(Arc::new(WorkspaceAuthorityRegistry::new()), &f.path)
            .unwrap();
    assert_eq!(
        restarted.dev_server_start(&id, f.audit()),
        "project not registered"
    );
    assert!(restarted.dev_server_stop(&id, f.audit()).is_err());
    assert!(restarted.dev_server_status(&id, f.audit()).is_err());
    assert_eq!(tree(&f.path), before);
}

#[test]
fn p0_002c4a_project_and_storage_identity_changes_deny_and_permanently_invalidate() {
    for case in ["removed", "replaced", "storage-missing", "storage-replaced"] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let detached = f.path.with_extension("project");
        let old_storage = f.path.with_extension("old");
        match case {
            "removed" => std::fs::remove_dir_all(&root).unwrap(),
            "replaced" => {
                std::fs::rename(&root, root.with_extension("original")).unwrap();
                std::fs::create_dir_all(root.join("react")).unwrap();
            }
            _ => {
                // Move the observed project first (Windows share-delete rule).
                std::fs::rename(&root, &detached).unwrap();
                std::fs::rename(&f.path, &old_storage).unwrap();
                if case == "storage-replaced" {
                    std::fs::create_dir(&f.path).unwrap();
                    std::fs::rename(&detached, &root).unwrap();
                }
            }
        }
        assert_eq!(
            f.authority.dev_server_start(&id, f.audit()),
            "registration identity denied",
            "{case}"
        );
        // Invalidation is permanent: later calls deny as unregistered.
        assert_all_deny(&f, &id, "project not registered");
        assert!(f
            .authority
            .catalog
            .lookup(Uuid::parse_str(&id).unwrap())
            .is_err());
        match case {
            "removed" => assert!(!root.exists()),
            "replaced" => {
                std::fs::remove_dir_all(&root).unwrap();
                std::fs::rename(root.with_extension("original"), &root).unwrap();
                assert_all_deny(&f, &id, "project not registered");
                assert!(!root.join("react/node_modules").exists());
            }
            "storage-missing" => {
                assert!(!f.path.exists());
                std::fs::remove_dir_all(&detached).unwrap();
                std::fs::remove_dir(&old_storage).unwrap();
            }
            _ => std::fs::remove_dir(&old_storage).unwrap(),
        }
        assert!(f
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|v| v["operation"] == "builder.registration.invalidate"));
    }
}

#[test]
fn p0_002c4a_missing_or_non_directory_react_denies_without_creation() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join("react");
    std::fs::remove_dir(&react).unwrap();
    assert_all_deny(&f, &id, "React identity denied");
    assert!(!react.exists());
    std::fs::write(&react, b"not a directory").unwrap();
    assert_all_deny(&f, &id, "React identity denied");
    assert_eq!(std::fs::read(&react).unwrap(), b"not a directory");
    // React is not part of the registration: its absence never invalidates.
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_ok());
    std::fs::remove_file(&react).unwrap();
    std::fs::create_dir(&react).unwrap();
    assert_eq!(
        f.authority.dev_server_start(&id, f.audit()),
        "launch unavailable"
    );
}

#[cfg(unix)]
#[test]
fn p0_002c4a_unix_react_and_project_symlink_redirects_deny() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join("react");
    let relocated = root.join("elsewhere");
    std::fs::rename(&react, &relocated).unwrap();
    symlink(&relocated, &react).unwrap();
    assert_all_deny(&f, &id, "React identity denied");
    std::fs::remove_file(&react).unwrap();
    std::fs::rename(&relocated, &react).unwrap();
    assert_eq!(
        f.authority.dev_server_start(&id, f.audit()),
        "launch unavailable"
    );
    std::fs::rename(&root, root.with_extension("old")).unwrap();
    symlink(root.with_extension("old"), &root).unwrap();
    assert_eq!(
        f.authority.dev_server_start(&id, f.audit()),
        "registration identity denied"
    );
    assert_all_deny(&f, &id, "project not registered");
}

#[cfg(windows)]
#[test]
fn p0_002c4a_windows_native_reparse_project_and_react_redirect_deny() {
    use std::os::windows::fs::symlink_dir;
    for project_replacement in [true, false] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let target = if project_replacement {
            root.clone()
        } else {
            root.join("react")
        };
        let old = target.with_extension("old");
        std::fs::rename(&target, &old).unwrap();
        symlink_dir(&old, &target)
            .expect("native Windows test requires symlink creation privilege");
        assert_ne!(
            f.authority.dev_server_start(&id, f.audit()),
            "launch unavailable"
        );
        assert!(f.authority.dev_server_stop(&id, f.audit()).is_err());
        assert!(f.authority.dev_server_status(&id, f.audit()).is_err());
        assert_eq!(
            f.authority
                .catalog
                .lookup(Uuid::parse_str(&id).unwrap())
                .is_err(),
            project_replacement
        );
    }
}

#[test]
fn p0_002c4a_stop_and_status_are_truthful_and_expose_no_process_authority() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let before = tree(&f.path);
    let from = f.events.lock().unwrap().len();
    for _ in 0..2 {
        f.authority.dev_server_stop(&id, f.audit()).unwrap();
    }
    let status = f.authority.dev_server_status(&id, f.audit()).unwrap();
    assert_eq!(
        status,
        json!({"status": "stopped", "launch_available": false})
    );
    assert_eq!(tree(&f.path), before);
    let events = devserver_events(&f, from);
    assert_eq!(events.len(), 3);
    for event in &events {
        assert_eq!(event["project_id"], id.as_str());
        assert_eq!(event["outcome"], "succeeded");
        assert_eq!(event["reason"], "no_owned_server");
        assert_eq!(event.as_object().unwrap().len(), 4);
    }
    assert!(!root.join("react/node_modules").exists());
}

#[test]
fn p0_002c4a_audit_reentry_and_no_sensitive_material() {
    let path = std::env::temp_dir().join(format!("nexus-c4a-{}", Uuid::new_v4()));
    let authority = Arc::new(
        BuilderWorkspaceAuthority::provision(Arc::new(WorkspaceAuthorityRegistry::new()), &path)
            .unwrap(),
    );
    let events = Arc::new(Mutex::new(Vec::<Value>::new()));
    let result = run_plan(&authority, Arc::new(|_| {}), "site", |_| Ok(generated())).unwrap();
    let root = PathBuf::from(&result.project_dir);
    std::fs::create_dir(root.join("react")).unwrap();
    let id = result.project_id;
    let reentered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let audit: Audit = {
        let (weak, events, id, reentered) = (
            Arc::downgrade(&authority),
            Arc::clone(&events),
            id.clone(),
            Arc::clone(&reentered),
        );
        Arc::new(move |event| {
            events.lock().unwrap().push(event);
            // Re-enter every safe Builder authority path with no lock held.
            if reentered.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3 {
                let authority = weak.upgrade().unwrap();
                let quiet: Audit = Arc::new(|_| {});
                let _ = authority.catalog.lookup(Uuid::parse_str(&id).unwrap());
                assert!(authority.dev_server_status(&id, Arc::clone(&quiet)).is_ok());
                assert!(authority.dev_server_stop(&id, Arc::clone(&quiet)).is_ok());
                let _ = authority.dev_server_start(&id, quiet);
            }
        })
    };
    let mut output = String::new();
    output += authority.dev_server_start(&id, Arc::clone(&audit));
    authority.dev_server_stop(&id, Arc::clone(&audit)).unwrap();
    output += &authority
        .dev_server_status(&id, Arc::clone(&audit))
        .unwrap()
        .to_string();
    output += authority.dev_server_start("../private/caller/root", Arc::clone(&audit));
    output += authority.dev_server_start(&Uuid::new_v4().to_string(), audit);
    assert!(reentered.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    output += &serde_json::to_string(&*events.lock().unwrap()).unwrap();
    for sensitive in [
        path.to_string_lossy().into_owned(),
        authority.root.to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
        root.join("react").to_string_lossy().into_owned(),
        authority.allocator.to_string(),
        authority.planner.to_string(),
        authority.writer.to_string(),
        "private/caller/root".into(),
        "pid".into(),
        "port".into(),
        "url".into(),
        "npm".into(),
        "npx".into(),
    ] {
        assert!(!output.contains(&sensitive), "sensitive: {sensitive}");
    }
    drop(authority);
    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn p0_002c4a_appstate_commands_fail_closed_and_share_registration() {
    let f = Fixture::new();
    let mut state = crate::AppState::new_in_memory();
    let unavailable = "Builder dev server: authority unavailable";
    assert_eq!(dev_server_start(&state, "id").unwrap_err(), unavailable);
    assert_eq!(dev_server_stop(&state, "id").unwrap_err(), unavailable);
    assert_eq!(dev_server_status(&state, "id").unwrap_err(), unavailable);
    state.builder_workspace = Ok(Arc::new(
        BuilderWorkspaceAuthority::provision(Arc::clone(&state.workspace_authority), &f.path)
            .unwrap(),
    ));
    let authority = state.builder_workspace.as_ref().unwrap();
    let result = run_plan(authority, f.audit(), "site", |_| Ok(generated())).unwrap();
    std::fs::create_dir(Path::new(&result.project_dir).join("react")).unwrap();
    let id = result.project_id;
    let cloned = state.clone();
    assert_eq!(
        dev_server_start(&cloned, &id).unwrap_err(),
        "Builder dev server: launch unavailable"
    );
    dev_server_stop(&cloned, &id).unwrap();
    assert_eq!(
        dev_server_status(&state, &id).unwrap(),
        json!({"status": "stopped", "launch_available": false})
    );
    assert_eq!(
        dev_server_start(&state, "not-a-uuid").unwrap_err(),
        "Builder dev server: project not registered"
    );
    let restarted = crate::AppState::new_in_memory();
    assert!(dev_server_start(&restarted, &id).is_err());
    assert!(dev_server_stop(&restarted, &id).is_err());
    assert!(dev_server_status(&restarted, &id).is_err());
    restarted.shutdown_oracle_runtime();
    state.shutdown_oracle_runtime();
}

// HOME, cwd and PATH are process-global; this runs in its own test process.
// Every executable a launch path could reach writes a sentinel if run.
#[test]
fn p0_002c4a_home_cwd_and_path_cannot_influence_or_trigger_execution() {
    const FLAG: &str = "NEXUS_C4A_ENV_TEST";
    const NAME: &str =
        "builder_workspace::tests::p0_002c4a_home_cwd_and_path_cannot_influence_or_trigger_execution";
    if std::env::var(FLAG).as_deref() == Ok(NAME) {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let react = root.join("react");
        plant_react_project(&react);
        let tools = Fixture::new();
        let sentinel = tools.path.join("sentinel");
        let bin = tools.path.join("bin");
        std::fs::create_dir_all(react.join("node_modules/.bin")).unwrap();
        std::fs::create_dir(&bin).unwrap();
        let script = format!("#!/bin/sh\necho ran > \"{}\"\n", sentinel.display());
        let batch = format!("@echo ran > \"{}\"\r\n", sentinel.display());
        for dir in [&bin, &react.join("node_modules/.bin")] {
            for name in ["npm", "npx", "vite", "node"] {
                let unix = dir.join(name);
                std::fs::write(&unix, &script).unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&unix, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
                for ext in ["cmd", "bat"] {
                    std::fs::write(dir.join(format!("{name}.{ext}")), &batch).unwrap();
                }
            }
        }
        // Where the removed HOME-derived start path would have looked.
        let home = Fixture::new();
        let legacy = home.path.join(".nexus/builds").join(&id).join("react");
        std::fs::create_dir_all(&legacy).unwrap();
        plant_react_project(&legacy);
        let (storage_before, home_before) = (tree(&f.path), tree(&home.path));
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&react).unwrap();
        std::env::set_var("HOME", &home.path);
        std::env::set_var("USERPROFILE", &home.path);
        std::env::set_var("PATH", &bin);
        assert_eq!(
            f.authority.dev_server_start(&id, f.audit()),
            "launch unavailable"
        );
        f.authority.dev_server_stop(&id, f.audit()).unwrap();
        assert_eq!(
            f.authority.dev_server_status(&id, f.audit()).unwrap(),
            json!({"status": "stopped", "launch_available": false})
        );
        // A selector naming the HOME-derived legacy project is not authority.
        let legacy_id = Uuid::new_v4().to_string();
        std::fs::create_dir_all(
            home.path
                .join(".nexus/builds")
                .join(&legacy_id)
                .join("react"),
        )
        .unwrap();
        assert_eq!(
            f.authority.dev_server_start(&legacy_id, f.audit()),
            "project not registered"
        );
        std::env::set_current_dir(original).unwrap();
        // Give any hypothetical detached child a moment to reveal itself.
        std::thread::sleep(Duration::from_millis(200));
        assert!(!sentinel.exists(), "a process was launched");
        assert_eq!(tree(&f.path), storage_before);
        assert!(!legacy.join("node_modules").exists());
        assert_eq!(
            tree(&home.path).len(),
            home_before.len() + 2 // only this test's legacy_id/react directories
        );
        eprintln!("C4A environment witness");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env(FLAG, NAME)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("C4A environment witness"));
}

// Security invariant guard: no production launch path remains reachable from
// the three dev-server commands or the Builder authority adapter.
#[test]
fn p0_002c4a_production_dev_server_commands_have_no_launch_path() {
    let lib = include_str!("../lib.rs");
    let start = lib.find("    fn builder_dev_server_start(").unwrap();
    let end = lib.find("    fn builder_dev_server_write_file(").unwrap();
    let commands = &lib[start..end];
    for forbidden in [
        "Command",
        "DevServer",
        "dev_server::",
        "install_deps",
        "forget",
        "spawn",
        "ResourceLimit",
        "process.exec",
        "HOME",
        "env::",
        "npm",
        "npx",
        "PathBuf",
        "join(",
    ] {
        assert!(!commands.contains(forbidden), "{forbidden}");
    }
    assert_eq!(
        commands
            .matches("super::builder_workspace::dev_server_")
            .count(),
        3
    );
    let adapter = include_str!("../builder_workspace.rs");
    for forbidden in [
        "Command::",
        "std::process",
        "DevServer::",
        "DevServerRegistry",
        "web_builder_agent::dev_server",
        "install_deps",
        "mem::forget",
        "ResourceLimit",
        ".spawn(",
        "env::var",
        "create_dir_all(&react",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
}
