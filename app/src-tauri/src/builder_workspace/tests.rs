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
    std::fs::remove_dir(&e.root).unwrap();
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
