use super::*;
use nexus_kernel::actuators::{ActionReviewDecision, ActionReviewEngine};
use nexus_kernel::cognitive::{
    AgentGoal, AgentMemoryManager, CognitivePlanner, PlannerLlm, RegistryExecutor,
};
use nexus_kernel::secrets::{
    backend_env::EnvBackend, backend_keyring::KeyringBackendAdapter, backend_memory::MemoryBackend,
    SecretsFacade,
};
use std::io::Read;
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// The parent owns the child throughout startup/execution and kills/reaps it on
// deadline. Drain both pipes concurrently so output cannot block the child.
fn child_output(mut command: Command, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (send, receive) = mpsc::channel();
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    for (index, mut pipe) in pipes.into_iter().enumerate() {
        let send = send.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = send.send((index, result));
        });
    }
    // Retain the original sender until return so completed readers cannot make
    // recv_timeout spin on Disconnected while the child is still exiting.
    let _keep_sender = send;
    let mut pipes = [None, None];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("P0-001 child exceeded parent deadline ({timeout:?})");
        }
        let status = child.try_wait().unwrap();
        if let Some(status) = status {
            if pipes.iter().all(Option::is_some) {
                return Output {
                    status,
                    stdout: pipes[0].take().unwrap(),
                    stderr: pipes[1].take().unwrap(),
                };
            }
        }
        // Pipe completion wakes us immediately; the interval only polls process
        // exit. Correctness depends on exit status, the witness and the deadline.
        match receive.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok((index, Ok(bytes))) => pipes[index] = Some(bytes),
            Ok((_, Err(error))) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("P0-001 child output failed: {error}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => unreachable!(),
        }
    }
}

// The facade is a process-wide OnceLock. Run each scenario in its own process so
// installing it and setting fake credentials cannot affect unrelated tests.
// A channel watchdog kills a deadlocked child; no sleeps or scheduling guesses.
fn isolated(name: &str, test: impl FnOnce()) {
    let full_name = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    let witness = format!("P0-001 CHILD COMPLETED: {full_name}");
    if std::env::var("NEXUS_P0_LOCK_CHILD").as_deref() == Ok(full_name.as_str()) {
        let (done, wait) = mpsc::channel();
        thread::spawn(move || {
            if matches!(
                wait.recv_timeout(Duration::from_secs(15)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                eprintln!("P0-001 lock regression exceeded 15-second deadline");
                std::process::exit(124);
            }
        });
        test();
        eprintln!("\n{witness}");
        done.send(()).unwrap();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", &full_name, "--nocapture"])
        .env("NEXUS_P0_LOCK_CHILD", &full_name)
        .env("NEXUS_ORACLE_EPHEMERAL", "1")
        .env("NEXUS_DB_PATH", ":memory:")
        .env(
            "NEXUS_CONFIG_PATH",
            std::env::temp_dir().join(format!("p0-001-{}.toml", Uuid::new_v4())),
        );
    for key in [
        "DEEPSEEK_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "OPENROUTER_API_KEY",
        "NVIDIA_NIM_API_KEY",
    ] {
        child.env(key, "p0-001-fake-secret");
    }
    let output = child_output(child, Duration::from_secs(20));
    assert!(
        output.status.success(),
        "{name}: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| *line == witness)
            .count(),
        1,
        "{name}: child did not complete the intended test body exactly once\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[should_panic(expected = "child did not complete the intended test body exactly once")]
fn p0_001_launcher_rejects_zero_tests() {
    isolated("p0_001_nonexistent_test_name", || unreachable!());
}

#[test]
#[should_panic(expected = "child did not complete the intended test body exactly once")]
fn p0_001_launcher_rejects_early_exit() {
    isolated("p0_001_launcher_rejects_early_exit", || {
        std::process::exit(0)
    });
}

#[test]
#[should_panic(expected = "exceeded parent deadline")]
fn p0_001_launcher_enforces_parent_deadline() {
    let full_name = format!(
        "{}::p0_001_launcher_enforces_parent_deadline",
        module_path!().split_once("::").unwrap().1
    );
    if std::env::var("NEXUS_P0_STALLED_CHILD").as_deref() == Ok(full_name.as_str()) {
        // No child watchdog: only the parent can terminate this blocked child.
        let (_send, receive) = mpsc::channel::<()>();
        receive.recv().unwrap();
        return;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &full_name, "--nocapture"])
        .env("NEXUS_P0_STALLED_CHILD", full_name);
    child_output(command, Duration::from_secs(1));
}

fn install_facade(state: &AppState) {
    let config = nexus_kernel::config::CredentialFacadeConfig {
        env_override_providers: vec!["llm".into()],
    };
    nexus_kernel::secrets::global::install(Arc::new(SecretsFacade::new(
        Arc::new(EnvBackend::new()),
        Arc::new(KeyringBackendAdapter::os_keyring()),
        None,
        Arc::new(MemoryBackend::new()),
        &config,
        state.audit.clone(),
    )));
}

fn agent(state: &AppState, name: &str, model: bool) -> String {
    let mut manifest = parse_manifest(&format!(
        r#"
name = "{name}"
version = "1.0.0"
capabilities = ["llm.query", "fs.read", "fs.write"]
fuel_budget = 10000
autonomy_level = 3
"#
    ))
    .unwrap();
    manifest.llm_model = model.then(|| "p0-test-model".into());
    state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string()
}

#[test]
fn p0_001_permission_commands_release_supervisor() {
    isolated("p0_001_permission_commands_release_supervisor", || {
        use nexus_kernel::audit::{AuditEvent, BatcherConfig, BlockBatchSink};

        struct LockProbe {
            supervisor: Arc<Mutex<Supervisor>>,
            calls: Arc<AtomicUsize>,
        }
        impl BlockBatchSink for LockProbe {
            fn seal_batch(&mut self, _: Vec<AuditEvent>) {
                // Test-only, nonblocking probe at the real shared append boundary.
                assert!(
                    self.supervisor.try_lock().is_ok(),
                    "permission command retained supervisor during shared audit"
                );
                self.calls.fetch_add(1, Ordering::SeqCst);
            }
        }

        let state = AppState::new_in_memory();
        let id = agent(&state, "permission-lock-test", true);
        let calls = Arc::new(AtomicUsize::new(0));
        state.audit.lock().unwrap().enable_distributed_audit(
            BatcherConfig {
                max_events: 1,
                ..BatcherConfig::default()
            },
            Box::new(LockProbe {
                supervisor: state.supervisor.clone(),
                calls: calls.clone(),
            }),
        );
        update_agent_permission(&state, id.clone(), "fs.write".into(), false).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        bulk_update_permissions(
            &state,
            id.clone(),
            vec![PermissionUpdate {
                capability_key: "fs.write".into(),
                enabled: true,
            }],
            Some("P0-001 regression".into()),
        )
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let audit = state.audit.lock().unwrap();
        assert!(audit.verify_integrity());
        assert_eq!(
            audit.events()[0].payload["event"],
            "update_agent_permission"
        );
        assert_eq!(
            audit.events()[1].payload["event"],
            "bulk_update_permissions"
        );
        drop(audit);
        assert!(state
            .supervisor
            .lock()
            .unwrap()
            .get_agent(Uuid::parse_str(&id).unwrap())
            .unwrap()
            .manifest
            .capabilities
            .contains(&"fs.write".to_string()));
        assert_eq!(state.db.get_audit_count().unwrap(), 2);
    });
}

struct Plan {
    response: String,
    access_secret: bool,
}

impl PlannerLlm for Plan {
    fn plan_query(&self, _: &str) -> Result<String, AgentError> {
        if self.access_secret {
            let config = build_provider_config(&NexusConfig::default());
            assert_eq!(
                config.deepseek_api_key.as_deref(),
                Some("p0-001-fake-secret")
            );
        }
        Ok(self.response.clone())
    }
}

fn cycle(
    state: &AppState,
    id: &str,
    plan: Plan,
    executor: &RegistryExecutor,
) -> nexus_kernel::cognitive::CycleResult {
    let mut goal = AgentGoal::new("P0-001 regression".into(), 5);
    goal.model_override = Some("p0-test-model".into());
    state.cognitive_runtime.assign_goal(id, goal).unwrap();
    let memory = AgentMemoryManager::new(Box::new(DbMemoryStore {
        db: state.db.clone(),
    }));
    run_cognitive_cycle(
        state,
        id,
        &CognitivePlanner::new(Box::new(plan)),
        &memory,
        executor,
    )
    .unwrap()
}

#[test]
fn p0_001_cognitive_secret_reentry() {
    isolated("p0_001_cognitive_secret_reentry", || {
        let state = AppState::new_in_memory();
        install_facade(&state);
        let id = agent(&state, "lock-test-agent", true);
        let executor = RegistryExecutor::new(
            std::env::temp_dir(),
            state.audit.clone(),
            state.supervisor.clone(),
            None,
        );
        let result = cycle(
            &state,
            &id,
            Plan {
                response: r#"[{"action":{"type":"Noop"},"description":"finish"}]"#.into(),
                access_secret: true,
            },
            &executor,
        );
        assert!(result.success);
        assert_eq!(result.steps_executed, 1);
        let audit = state.audit.lock().unwrap();
        assert!(audit.verify_integrity());
        let events = audit.events();
        assert!(events
            .iter()
            .any(|e| e.payload["event"] == "cognitive.step_executed"));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.payload["event"] == "secret_accessed" && e.payload["result"] == "ok")
                .count(),
            6
        );
        let secret_index = events
            .iter()
            .position(|e| e.payload["event"] == "secret_accessed")
            .unwrap();
        let step_index = events
            .iter()
            .position(|e| e.payload["event"] == "cognitive.step_executed")
            .unwrap();
        assert!(
            secret_index < step_index,
            "secret audit must be immediate, not buffered until cycle completion"
        );
        assert!(!serde_json::to_string(events)
            .unwrap()
            .contains("p0-001-fake-secret"));
    });
}

struct TestWarden(WardenReviewEngine);

impl ActionReviewEngine for TestWarden {
    fn review(
        &self,
        id: &str,
        name: &str,
        action: &PlannedAction,
    ) -> Result<ActionReviewDecision, String> {
        self.0.review_with(
            id,
            name,
            action,
            true,
            || "p0-test-model".into(),
            |_, _| Ok("YES safe test write".into()),
        )
    }
}

#[test]
fn p0_001_warden_audit_reentry() {
    isolated("p0_001_warden_audit_reentry", || {
        let state = AppState::new_in_memory();
        let id = agent(&state, "lock-test-agent", true);
        agent(&state, "nexus-warden", true);
        let workspace = std::env::temp_dir().join(format!("p0-001-{}", Uuid::new_v4()));
        std::fs::create_dir(&workspace).unwrap();
        let executor = RegistryExecutor::new(
            workspace.clone(),
            state.audit.clone(),
            state.supervisor.clone(),
            Some(Arc::new(TestWarden(WardenReviewEngine {
                state: state.clone(),
            }))),
        );
        let result = cycle(&state, &id, Plan {
            response: r#"[{"action":{"type":"FileWrite","path":"result.txt","content":"lock regression"},"description":"write test fixture"}]"#.into(),
            access_secret: false,
        }, &executor);
        assert!(result.success);
        assert_eq!(result.steps_executed, 1);
        assert_eq!(
            std::fs::read_to_string(workspace.join("result.txt")).unwrap(),
            "lock regression"
        );
        let audit = state.audit.lock().unwrap();
        assert!(audit.verify_integrity());
        assert!(audit
            .events()
            .iter()
            .any(|e| e.payload["action"] == "warden_review"));
        assert!(audit.events().iter().any(
            |e| e.payload["event_kind"] == "warden.review" && e.payload["decision"] == "allow"
        ));
        assert_eq!(state.db.get_audit_count().unwrap(), 1);
        std::fs::remove_dir_all(workspace).unwrap();
    });
}

#[test]
fn p0_001_warden_supervisor_audit_order() {
    isolated("p0_001_warden_supervisor_audit_order", || {
        let state = AppState::new_in_memory();
        install_facade(&state);
        agent(&state, "nexus-warden", false);
        let warden = WardenReviewEngine {
            state: state.clone(),
        };
        let fallback_calls = AtomicUsize::new(0);
        let result = warden
            .review_with(
                &Uuid::nil().to_string(),
                "actor",
                &PlannedAction::Noop,
                true,
                || {
                    // Deterministic: this callback must run after the supervisor snapshot
                    // is released, before secrets access acquires the shared audit lock.
                    assert!(
                        state.supervisor.try_lock().is_ok(),
                        "supervisor held during default-model resolution"
                    );
                    assert!(
                        state.audit.try_lock().is_ok(),
                        "audit held during default-model resolution"
                    );
                    assert!(build_provider_config(&NexusConfig::default())
                        .deepseek_api_key
                        .is_some());
                    fallback_calls.fetch_add(1, Ordering::SeqCst);
                    "p0-test-model".into()
                },
                |_, _| Ok("YES safe".into()),
            )
            .unwrap();
        assert_eq!(
            fallback_calls.load(Ordering::SeqCst),
            1,
            "Warden must execute the lock-checking provider fallback exactly once"
        );
        assert!(matches!(result, ActionReviewDecision::Allow { .. }));
        assert!(state.audit.lock().unwrap().verify_integrity());
    });
}
