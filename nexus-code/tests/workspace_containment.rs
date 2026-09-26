//! P0-002B: real tools against disposable inside/outside fixtures.
use nexus_code::error::NxError;
use nexus_code::governance::{AuditAction, Capability, CapabilityScope, GovernanceKernel};
use nexus_code::tools::{create_tool, execute_governed, ToolContext};
use serde_json::{json, Value};
use std::path::PathBuf;

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("work");
        let outside = temp.path().join("work-evil");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("inside.rs"), "pub fn inside_marker() {}\n").unwrap();
        std::fs::write(
            outside.join("secret.rs"),
            "pub fn outside_secret_marker() {}\n",
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            outside,
        }
    }

    fn ctx(&self) -> ToolContext {
        ToolContext {
            working_dir: self.root.clone(),
            blocked_paths: vec![],
            max_file_scope: None,
            non_interactive: true,
        }
    }

    fn unchanged(&self) {
        assert_eq!(
            std::fs::read(self.outside.join("secret.rs")).unwrap(),
            b"pub fn outside_secret_marker() {}\n"
        );
        let names: Vec<_> = std::fs::read_dir(&self.outside)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec!["secret.rs"]);
    }

    async fn denied(&self, tool: &str, input: Value, ctx: &ToolContext) {
        let result = create_tool(tool).unwrap().execute(input, ctx).await;
        self.unchanged();
        assert!(
            !result.success,
            "{tool} unexpectedly succeeded: {}",
            result.output
        );
        assert!(
            result.output.contains("Capability denied"),
            "{tool}: {}",
            result.output
        );
        assert!(
            !result.output.contains(self.outside.to_str().unwrap()),
            "canonical outside path disclosed"
        );
    }
}

#[tokio::test]
async fn p0_002b_inside_read_write_edit_and_absolute_paths_work() {
    let f = Fixture::new();
    let ctx = f.ctx();
    for path in [PathBuf::from("inside.rs"), f.root.join("inside.rs")] {
        let result = create_tool("file_read")
            .unwrap()
            .execute(json!({"path":path}), &ctx)
            .await;
        assert!(result.success && result.output.contains("inside_marker"));
    }
    for path in [
        PathBuf::from("new/nested/file.txt"),
        f.root.join("absolute.txt"),
    ] {
        let result = create_tool("file_write")
            .unwrap()
            .execute(json!({"path":path,"content":"before"}), &ctx)
            .await;
        assert!(result.success, "{}", result.output);
        assert_eq!(
            std::fs::read_to_string(f.root.join(&path)).unwrap(),
            "before"
        );
        let result = create_tool("file_edit")
            .unwrap()
            .execute(
                json!({"path":path,"old_text":"before","new_text":"after"}),
                &ctx,
            )
            .await;
        assert!(result.success, "{}", result.output);
        assert_eq!(std::fs::read_to_string(f.root.join(path)).unwrap(), "after");
    }
    f.unchanged();
}

fn assert_policy_failure_evidence(
    result: &Result<impl std::fmt::Debug, NxError>,
    kernel: &GovernanceKernel,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<nexus_code::agent::AgentEvent>,
    fixture: &Fixture,
    policy: &[u8],
) {
    assert!(
        matches!(result, Err(NxError::CapabilityDenied { capability, .. }) if capability == "path.policy"),
        "{result:?}"
    );
    assert!(result.as_ref().unwrap_err().is_filesystem_denial());
    assert!(!result
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains(fixture.root.to_str().unwrap()));
    let mut started = Vec::new();
    let mut denied = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            nexus_code::agent::AgentEvent::ToolCallStart { name, .. } => started.push(name),
            nexus_code::agent::AgentEvent::ToolCallDenied { name, .. } => denied.push(name),
            nexus_code::agent::AgentEvent::Done { .. } => panic!("denial reported as completion"),
            _ => {}
        }
    }
    assert_eq!(started, ["search"]);
    assert_eq!(denied, ["search"]);
    let entries = kernel.audit.entries();
    assert_eq!(entries.iter().filter(|e| matches!(&e.action, AuditAction::ToolInvocation { tool, .. } if tool == "search")).count(), 1);
    assert_eq!(entries.iter().filter(|e| matches!(&e.action, AuditAction::ToolResult { tool, success:false, summary } if tool == "search" && summary.contains("path.policy"))).count(), 1);
    assert!(kernel.audit.verify_chain().is_ok());
    assert_eq!(kernel.fuel.budget().reserved, 0);
    let tool_costs: Vec<_> = kernel
        .fuel
        .cost_history()
        .iter()
        .filter(|(provider, _, _)| provider == "tool")
        .map(|(_, cost, _)| cost.fuel_units)
        .collect();
    assert_eq!(tool_costs, [5]);
    fixture.unchanged();
    assert_eq!(
        std::fs::read(fixture.root.join("inside.rs")).unwrap(),
        b"pub fn inside_marker() {}\n"
    );
    assert_eq!(
        std::fs::read(fixture.root.join(".gitignore")).unwrap(),
        policy
    );
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 2);
    assert!(!fixture.root.join("should-not-exist").exists());
}

#[tokio::test]
async fn p0_002b_correction_executor_policy_failure_is_terminal() {
    use nexus_code::agent::planner::{Plan, PlanStep};
    // Invalid UTF-8 deterministically exercises an uninterpretable policy,
    // including when tests run as a privileged user who can read mode-000 files.
    for policy in [b"{a,b\n".as_slice(), b"\xff\n".as_slice()] {
        let f = Fixture::new();
        std::fs::write(f.root.join(".gitignore"), policy).unwrap();
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel
            .capabilities
            .grant(Capability::FileWrite, CapabilityScope::Full);
        let plan = Plan {
            summary: "privacy denial must stop the plan".into(),
            steps: vec![
                PlanStep {
                    step: 1,
                    description: "evaluate policy".into(),
                    tool: "search".into(),
                    input: json!({"pattern":"inside_marker"}),
                },
                PlanStep {
                    step: 2,
                    description: "must never execute".into(),
                    tool: "file_write".into(),
                    input: json!({"path":"should-not-exist", "content":"changed"}),
                },
            ],
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let result =
            nexus_code::agent::executor::execute_plan(&plan, &f.ctx(), &mut kernel, &tx, &|_| true)
                .await;
        assert_policy_failure_evidence(&result, &kernel, &mut rx, &f, policy);
        assert_eq!(kernel.fuel.budget().consumed, 5);
    }
}

#[tokio::test]
async fn p0_002b_correction_agent_policy_failure_never_requests_next_model_response() {
    use async_trait::async_trait;
    use nexus_code::llm::{
        provider::{LlmProvider, ProviderRegistry},
        router::{ModelRouter, ModelSlot, SlotConfig},
        types::*,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Provider {
        calls: Arc<AtomicUsize>,
        raw_calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl LlmProvider for Provider {
        fn name(&self) -> &str {
            "fixture"
        }
        fn is_configured(&self) -> bool {
            true
        }
        fn available_models(&self) -> Vec<&str> {
            vec!["fixture"]
        }
        async fn complete(&self, _: &LlmRequest) -> Result<LlmResponse, NxError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let action = match call {
                0 => Some(("search", json!({"pattern":"inside_marker"}))),
                1 => Some((
                    "file_write",
                    json!({"path":"should-not-exist", "content":"changed"}),
                )),
                _ => None,
            };
            Ok(LlmResponse {
                content: String::new(), model: "fixture".into(),
                usage: TokenUsage { total_tokens: 7, ..Default::default() },
                finish_reason: Some(if action.is_some() { "tool_calls" } else { "stop" }.into()),
                content_blocks: None,
                tool_calls: action.map(|(name, input)| vec![json!({"id":format!("call-{call}"), "type":"function", "function":{"name":name,"arguments":input.to_string()}})]),
                stop_reason: Some("tool_calls".into()),
            })
        }
        async fn stream_raw(&self, _: &LlmRequest) -> Result<Option<reqwest::Response>, NxError> {
            self.raw_calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
        async fn stream(
            &self,
            _: &LlmRequest,
            _: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
        ) -> Result<(), NxError> {
            panic!("unexpected streaming path")
        }
    }
    for policy in [b"{a,b\n".as_slice(), b"\xff\n".as_slice()] {
        let f = Fixture::new();
        std::fs::write(f.root.join(".gitignore"), policy).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(Provider {
            calls: calls.clone(),
            raw_calls: raw_calls.clone(),
        }));
        let mut router = ModelRouter::new(Arc::new(registry));
        router.set_slot(
            ModelSlot::Execution,
            SlotConfig {
                provider: "fixture".into(),
                model: "fixture".into(),
            },
        );
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel
            .capabilities
            .grant(Capability::FileWrite, CapabilityScope::Full);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let result = nexus_code::agent::run_agent_loop(
            &mut vec![Message {
                role: Role::User,
                content: "fixture".into(),
            }],
            &router,
            &nexus_code::tools::ToolRegistry::with_defaults(),
            &f.ctx(),
            &mut kernel,
            &nexus_code::agent::AgentConfig {
                max_turns: 3,
                ..Default::default()
            },
            tx,
            Arc::new(|_| true),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert_policy_failure_evidence(&result, &kernel, &mut rx, &f, policy);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(raw_calls.load(Ordering::SeqCst), 1);
        assert_eq!(kernel.fuel.budget().consumed, 12);
    }
}

#[tokio::test]
async fn p0_002b_glob_preserves_inside_absolute_directory_and_dot_patterns() {
    let f = Fixture::new();
    std::fs::create_dir(f.root.join("empty-dir")).unwrap();
    for (pattern, expected) in [
        ("./*.rs".to_string(), "inside.rs"),
        (
            f.root.join("*.rs").to_str().unwrap().to_string(),
            "inside.rs",
        ),
        ("empty-*".into(), "empty-dir"),
        (".".into(), ".\n"),
    ] {
        let result = create_tool("glob")
            .unwrap()
            .execute(json!({"pattern":pattern}), &f.ctx())
            .await;
        assert!(result.success, "{}", result.output);
        assert!(
            format!("{}\n", result.output).contains(expected),
            "{}",
            result.output
        );
        assert!(!result.output.contains(f.root.to_str().unwrap()));
    }
    f.unchanged();
}

#[tokio::test]
async fn p0_002b_outside_absolute_traversal_and_prefix_confusion_denied() {
    let f = Fixture::new();
    for path in [
        f.outside.join("secret.rs"),
        PathBuf::from("../work-evil/secret.rs"),
        PathBuf::from("nested/../../work-evil/secret.rs"),
    ] {
        std::fs::create_dir_all(f.root.join("nested")).unwrap();
        for tool in ["file_read", "file_write", "file_edit"] {
            f.denied(tool, json!({"path":path,"content":"changed","old_text":"outside_secret_marker","new_text":"changed"}), &f.ctx()).await;
        }
    }
}

#[tokio::test]
async fn p0_002b_unconfigured_or_invalid_root_fails_closed() {
    let f = Fixture::new();
    for root in [
        PathBuf::new(),
        PathBuf::from("."),
        f.root.join("missing"),
        f.root.join("inside.rs"),
    ] {
        let mut ctx = f.ctx();
        ctx.working_dir = root;
        for tool in [
            "file_read",
            "file_write",
            "file_edit",
            "search",
            "glob",
            "project_index",
        ] {
            f.denied(tool, json!({"path":f.outside.join("secret.rs"),"pattern":"*","content":"changed","old_text":"outside_secret_marker","new_text":"changed"}), &ctx).await;
        }
        assert!(!f.root.join("missing").exists());
    }
}

#[tokio::test]
async fn p0_002b_search_glob_index_reject_outside_roots_and_globs() {
    let f = Fixture::new();
    for tool in ["search", "glob", "project_index"] {
        for path in [f.outside.clone(), PathBuf::from("../work-evil")] {
            f.denied(
                tool,
                json!({"path":path,"pattern":"*","include_definitions":true}),
                &f.ctx(),
            )
            .await;
        }
    }
    for pattern in [
        f.outside.join("*.rs").to_str().unwrap().to_string(),
        "../work-evil/*.rs".into(),
    ] {
        f.denied("glob", json!({"pattern":pattern}), &f.ctx()).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn p0_002b_symlink_file_directory_and_missing_target_denied() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    symlink(f.outside.join("secret.rs"), f.root.join("escape.rs")).unwrap();
    symlink(&f.outside, f.root.join("escape-dir")).unwrap();
    symlink(f.outside.join("absent.rs"), f.root.join("dangling.rs")).unwrap();
    for path in ["escape.rs", "escape-dir/secret.rs", "dangling.rs"] {
        for tool in ["file_read", "file_write", "file_edit"] {
            f.denied(tool, json!({"path":path,"content":"changed","old_text":"outside_secret_marker","new_text":"changed"}), &f.ctx()).await;
        }
    }
    f.denied(
        "file_write",
        json!({"path":"escape-dir/new/nested/file.txt","content":"changed"}),
        &f.ctx(),
    )
    .await;
    for tool in ["search", "glob", "project_index"] {
        f.denied(
            tool,
            json!({"path":"escape-dir","pattern":"*","include_definitions":true}),
            &f.ctx(),
        )
        .await;
    }
    for pattern in ["escape-dir/*.rs", "escape.rs", "dangling.rs"] {
        f.denied("glob", json!({"pattern":pattern}), &f.ctx()).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn p0_002b_recursive_tools_do_not_follow_descendant_symlinks() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.outside, f.root.join("escape-dir")).unwrap();
    std::os::unix::fs::symlink(f.outside.join("secret.rs"), f.root.join("escape.rs")).unwrap();
    for tool in ["search", "glob", "project_index"] {
        let pattern = if tool == "search" {
            "pub fn"
        } else {
            "**/*.rs"
        };
        let result = create_tool(tool)
            .unwrap()
            .execute(
                json!({"pattern":pattern,"include_definitions":true}),
                &f.ctx(),
            )
            .await;
        assert!(result.success, "{tool}: {}", result.output);
        assert!(
            result.output.contains("inside"),
            "safe traversal skipped: {}",
            result.output
        );
        assert!(
            !result.output.contains("outside_secret_marker")
                && !result.output.contains("secret.rs"),
            "outside content: {}",
            result.output
        );
        assert!(
            !result.output.contains(f.root.to_str().unwrap()),
            "absolute host path: {}",
            result.output
        );
        f.unchanged();
    }
}

#[tokio::test]
async fn p0_002b_pipeline_denial_is_typed_audited_and_releases_fuel() {
    let f = Fixture::new();
    let mut kernel = GovernanceKernel::new(50_000).unwrap();
    let result = execute_governed(
        create_tool("file_read").unwrap().as_ref(),
        json!({"path":f.outside.join("secret.rs")}),
        &f.ctx(),
        &mut kernel,
    )
    .await;
    assert!(
        matches!(result, Err(NxError::CapabilityDenied { .. })),
        "{result:?}"
    );
    assert_eq!(kernel.fuel.budget().reserved, 0);
    assert!(kernel.audit.entries().iter().any(|e| matches!(&e.action, AuditAction::ToolResult { tool, success:false, .. } if tool == "file_read")));
    assert!(kernel.audit.verify_chain().is_ok());
    f.unchanged();
}

#[tokio::test]
async fn p0_002b_executor_stops_after_security_denial_including_consent() {
    use nexus_code::agent::planner::{Plan, PlanStep};
    for tool in ["file_read", "file_write"] {
        let f = Fixture::new();
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel
            .capabilities
            .grant(Capability::FileWrite, CapabilityScope::Full);
        let plan = Plan {
            summary: "containment".into(),
            steps: vec![
                PlanStep {
                    step: 1,
                    description: "deny outside".into(),
                    tool: tool.into(),
                    input: json!({"path":f.outside.join("secret.rs"),"content":"changed"}),
                },
                PlanStep {
                    step: 2,
                    description: "must not execute".into(),
                    tool: "file_write".into(),
                    input: json!({"path":"should-not-exist","content":"changed"}),
                },
            ],
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let result =
            nexus_code::agent::executor::execute_plan(&plan, &f.ctx(), &mut kernel, &tx, &|_| true)
                .await;
        f.unchanged();
        assert!(
            matches!(result, Err(NxError::CapabilityDenied { .. })),
            "{result:?}"
        );
        assert!(!f.root.join("should-not-exist").exists());
        let mut started = 0;
        while let Ok(e) = rx.try_recv() {
            if matches!(e, nexus_code::agent::AgentEvent::ToolCallStart { .. }) {
                started += 1;
            }
        }
        assert_eq!(started, 1);
    }
}

#[tokio::test]
async fn p0_002b_real_agent_loop_does_not_retry_or_run_next_tool() {
    use async_trait::async_trait;
    use nexus_code::llm::{
        provider::{LlmProvider, ProviderRegistry},
        router::{ModelRouter, ModelSlot, SlotConfig},
        types::*,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Provider {
        response: LlmResponse,
        calls: Arc<AtomicUsize>,
        raw_calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl LlmProvider for Provider {
        fn name(&self) -> &str {
            "fixture"
        }
        fn is_configured(&self) -> bool {
            true
        }
        fn available_models(&self) -> Vec<&str> {
            vec!["fixture"]
        }
        async fn complete(&self, _: &LlmRequest) -> Result<LlmResponse, NxError> {
            assert_eq!(
                self.calls.fetch_add(1, Ordering::SeqCst),
                0,
                "provider retried after denial"
            );
            Ok(self.response.clone())
        }
        async fn stream_raw(&self, _: &LlmRequest) -> Result<Option<reqwest::Response>, NxError> {
            assert_eq!(
                self.raw_calls.fetch_add(1, Ordering::SeqCst),
                0,
                "stream retried after denial"
            );
            Ok(None)
        }
        async fn stream(
            &self,
            _: &LlmRequest,
            _: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
        ) -> Result<(), NxError> {
            panic!("unexpected streaming path")
        }
    }
    for tool in ["file_read", "file_write"] {
        let f = Fixture::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let consents = Arc::new(AtomicUsize::new(0));
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(Provider {
            calls:calls.clone(),raw_calls:raw_calls.clone(),
            response:LlmResponse {
                content:String::new(),model:"fixture".into(),usage:TokenUsage::default(),finish_reason:Some("tool_calls".into()),content_blocks:None,
                tool_calls:Some(vec![
                    json!({"id":"denied","type":"function","function":{"name":tool,"arguments":json!({"path":f.outside.join("secret.rs"),"content":"changed"}).to_string()}}),
                    json!({"id":"never","type":"function","function":{"name":"file_write","arguments":json!({"path":"should-not-exist","content":"changed"}).to_string()}}),
                ]),stop_reason:Some("tool_calls".into()),
            },
        }));
        let mut router = ModelRouter::new(Arc::new(registry));
        router.set_slot(
            ModelSlot::Execution,
            SlotConfig {
                provider: "fixture".into(),
                model: "fixture".into(),
            },
        );
        let mut kernel = GovernanceKernel::new(50_000).unwrap();
        kernel
            .capabilities
            .grant(Capability::FileWrite, CapabilityScope::Full);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let witness = consents.clone();
        let result = nexus_code::agent::run_agent_loop(
            &mut vec![Message {
                role: Role::User,
                content: "fixture".into(),
            }],
            &router,
            &nexus_code::tools::ToolRegistry::with_defaults(),
            &f.ctx(),
            &mut kernel,
            &nexus_code::agent::AgentConfig {
                max_turns: 3,
                ..Default::default()
            },
            tx,
            Arc::new(move |_| {
                witness.fetch_add(1, Ordering::SeqCst);
                true
            }),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        f.unchanged();
        assert!(
            matches!(result, Err(NxError::CapabilityDenied { .. })),
            "{result:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(raw_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            consents.load(Ordering::SeqCst),
            usize::from(tool == "file_write")
        );
        assert!(!f.root.join("should-not-exist").exists());
        let mut started = 0;
        while let Ok(e) = rx.try_recv() {
            if matches!(e, nexus_code::agent::AgentEvent::ToolCallStart { .. }) {
                started += 1;
            }
        }
        assert_eq!(started, 1, "risky tool path must execute exactly once");
        assert!(kernel.audit.entries().iter().any(
            |e| matches!(&e.action,AuditAction::ToolResult { tool:t, success:false,.. } if t==tool)
        ));
    }
}

#[tokio::test]
async fn p0_002b_security_category_is_distinct_and_instrumentation_preserves_it() {
    let f = Fixture::new();
    let tool = create_tool("file_read").unwrap();
    let missing = tool.execute(json!({"path":"missing.txt"}), &f.ctx()).await;
    assert!(!missing.success && missing.filesystem_error().is_none());
    let invalid = tool.execute(json!({}), &f.ctx()).await;
    assert!(!invalid.success && invalid.filesystem_error().is_none());
    let denied = tool
        .execute(json!({"path":f.outside.join("secret.rs")}), &f.ctx())
        .await;
    assert!(denied
        .filesystem_error()
        .is_some_and(|e| e.is_filesystem_denial()));
    let mut kernel = GovernanceKernel::new(50_000).unwrap();
    let result = nexus_code::tools::execute_governed_instrumented(
        tool.as_ref(),
        json!({"path":f.outside.join("secret.rs")}),
        &f.ctx(),
        &mut kernel,
    )
    .await;
    assert!(matches!(result, Err(NxError::CapabilityDenied { .. })));
    assert_eq!(kernel.fuel.budget().reserved, 0);
    assert!(kernel
        .audit
        .entries()
        .iter()
        .any(|e| matches!(&e.action, AuditAction::ToolResult { success: false, .. })));
    f.unchanged();
}

#[tokio::test]
async fn p0_002b_scope_only_narrows_root_and_recursive_tools_obey_blocks() {
    let f = Fixture::new();
    let mut ctx = f.ctx();
    ctx.max_file_scope = Some("**".into());
    f.denied(
        "file_write",
        json!({"path":f.outside.join("secret.rs"),"content":"changed"}),
        &ctx,
    )
    .await;
    ctx.blocked_paths = vec![f.root.join("inside.rs").to_str().unwrap().into()];
    for tool in [
        "file_read",
        "file_edit",
        "file_write",
        "search",
        "glob",
        "project_index",
    ] {
        let input = if tool.starts_with("file_") {
            json!({"path":"inside.rs","old_text":"inside_marker","new_text":"changed","content":"changed"})
        } else {
            json!({"pattern":"*","include_definitions":true})
        };
        f.denied(tool, input, &ctx).await;
    }
    assert_eq!(
        std::fs::read(f.root.join("inside.rs")).unwrap(),
        b"pub fn inside_marker() {}\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn p0_002b_inside_symlink_and_symlink_workspace_root_work() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.root.join("inside.rs"), f.root.join("alias.rs")).unwrap();
    let root_alias = f._temp.path().join("root-alias");
    std::os::unix::fs::symlink(&f.root, &root_alias).unwrap();
    let mut ctx = f.ctx();
    ctx.working_dir = root_alias;
    let listed = create_tool("glob")
        .unwrap()
        .execute(json!({"pattern":ctx.working_dir.join("*.rs")}), &ctx)
        .await;
    assert!(
        listed.success && listed.output.contains("inside.rs"),
        "{}",
        listed.output
    );
    let result = create_tool("file_edit")
        .unwrap()
        .execute(
            json!({"path":"alias.rs","old_text":"inside_marker","new_text":"updated"}),
            &ctx,
        )
        .await;
    assert!(result.success, "{}", result.output);
    assert!(std::fs::read_to_string(f.root.join("inside.rs"))
        .unwrap()
        .contains("updated"));
    assert!(std::fs::symlink_metadata(f.root.join("alias.rs"))
        .unwrap()
        .file_type()
        .is_symlink());
    f.unchanged();
}
