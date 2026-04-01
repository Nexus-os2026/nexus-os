//! Governance pipeline integration tests — tool execution through the full
//! capability + fuel + consent + audit pipeline.

use nexus_code::governance::{
    AuditAction, Capability, CapabilityScope, ConsentTier, GovernanceKernel,
};
use nexus_code::tools::{create_tool, execute_after_consent, execute_governed, ToolContext};
use serde_json::json;

/// Create a GovernanceKernel with enough fuel and default capabilities.
fn test_kernel(fuel: u64) -> GovernanceKernel {
    GovernanceKernel::new(fuel).expect("kernel creation should not fail")
}

/// Create a ToolContext for testing.
fn test_ctx(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    }
}

// ═══════════════════════════════════════════════════════
// Pipeline Integration Tests (15)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_pipeline_tier1_auto_approved() {
    // file_read is Tier1 (auto-approved), FileRead capability is default-granted
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let result = execute_governed(
        tool.as_ref(),
        json!({"path": "test.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .expect("Tier1 should auto-approve");

    assert!(result.is_success());
    assert!(result.output.contains("hello"));
    // duration_ms is a u64, always >= 0 by construction
}

#[tokio::test]
async fn test_pipeline_tier2_returns_consent_required() {
    // file_write is Tier2, needs FileWrite capability (not default-granted)
    // First grant FileWrite so we get to consent phase
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    kernel
        .capabilities
        .grant(Capability::FileWrite, CapabilityScope::Full);

    let tool = create_tool("file_write").unwrap();
    let result = execute_governed(
        tool.as_ref(),
        json!({"path": "test.txt", "content": "data"}),
        &ctx,
        &mut kernel,
    )
    .await;

    match result {
        Err(nexus_code::error::NxError::ConsentRequired { request }) => {
            assert_eq!(request.tier, ConsentTier::Tier2);
        }
        other => panic!("Expected ConsentRequired, got {:?}", other),
    }
}

#[tokio::test]
async fn test_pipeline_tier2_approve_then_execute() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    kernel
        .capabilities
        .grant(Capability::FileWrite, CapabilityScope::Full);

    let tool = create_tool("file_write").unwrap();
    let input = json!({"path": "consent_test.txt", "content": "approved data"});

    // First call returns ConsentRequired
    let result = execute_governed(tool.as_ref(), input.clone(), &ctx, &mut kernel).await;
    let request = match result {
        Err(nexus_code::error::NxError::ConsentRequired { request }) => request,
        other => panic!("Expected ConsentRequired, got {:?}", other),
    };

    // Grant consent and execute
    let result = execute_after_consent(tool.as_ref(), input, &ctx, &mut kernel, &request, true)
        .await
        .expect("Approved consent should succeed");

    assert!(result.is_success());
    assert!(result.output.contains("Created"));
    // Verify file exists on disk
    let content = std::fs::read_to_string(dir.path().join("consent_test.txt")).unwrap();
    assert_eq!(content, "approved data");
}

#[tokio::test]
async fn test_pipeline_tier2_deny_blocks_execution() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    kernel
        .capabilities
        .grant(Capability::FileWrite, CapabilityScope::Full);

    let tool = create_tool("file_write").unwrap();
    let input = json!({"path": "denied.txt", "content": "should not exist"});

    // First call returns ConsentRequired
    let result = execute_governed(tool.as_ref(), input.clone(), &ctx, &mut kernel).await;
    let request = match result {
        Err(nexus_code::error::NxError::ConsentRequired { request }) => request,
        other => panic!("Expected ConsentRequired, got {:?}", other),
    };

    // Deny consent
    let result =
        execute_after_consent(tool.as_ref(), input, &ctx, &mut kernel, &request, false).await;

    assert!(result.is_err());
    match result {
        Err(nexus_code::error::NxError::ConsentDenied { .. }) => {}
        other => panic!("Expected ConsentDenied, got {:?}", other),
    }

    // File should NOT exist
    assert!(!dir.path().join("denied.txt").exists());
}

#[tokio::test]
async fn test_pipeline_tier3_bash() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    kernel
        .capabilities
        .grant(Capability::ShellExecute, CapabilityScope::Full);

    let tool = create_tool("bash").unwrap();
    let result = execute_governed(
        tool.as_ref(),
        json!({"command": "echo test"}),
        &ctx,
        &mut kernel,
    )
    .await;

    match result {
        Err(nexus_code::error::NxError::ConsentRequired { request }) => {
            assert_eq!(request.tier, ConsentTier::Tier3);
        }
        other => panic!("Expected ConsentRequired (Tier3), got {:?}", other),
    }
}

#[tokio::test]
async fn test_pipeline_capability_denied_before_consent() {
    // FileWrite not granted -> CapabilityDenied before consent is even checked
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    // Do NOT grant FileWrite

    let tool = create_tool("file_write").unwrap();
    let result = execute_governed(
        tool.as_ref(),
        json!({"path": "test.txt", "content": "data"}),
        &ctx,
        &mut kernel,
    )
    .await;

    match result {
        Err(nexus_code::error::NxError::CapabilityDenied { .. }) => {}
        other => panic!("Expected CapabilityDenied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_pipeline_fuel_exhausted_before_consent() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    // Budget of 5, file_read estimated_fuel is 5, but it gets reserved
    // A second call should exhaust
    let mut kernel = test_kernel(3); // Less than file_read's estimated_fuel of 5

    let tool = create_tool("file_read").unwrap();
    let result = execute_governed(
        tool.as_ref(),
        json!({"path": "test.txt"}),
        &ctx,
        &mut kernel,
    )
    .await;

    match result {
        Err(nexus_code::error::NxError::FuelExhausted { .. }) => {}
        other => panic!("Expected FuelExhausted, got {:?}", other),
    }
}

#[tokio::test]
async fn test_pipeline_audit_records_tool_invocation_and_result() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("audit.txt"), "audit me").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let initial_entries = kernel.audit.len();

    let _result = execute_governed(
        tool.as_ref(),
        json!({"path": "audit.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    // Should have added audit entries: CapabilityCheck, ConsentGranted, ToolInvocation, ToolResult
    assert!(kernel.audit.len() > initial_entries);

    // Find ToolInvocation entry
    let has_invocation = kernel.audit.entries().iter().any(|e| {
        matches!(
            &e.action,
            AuditAction::ToolInvocation { tool, .. } if tool == "file_read"
        )
    });
    assert!(has_invocation, "Should have ToolInvocation audit entry");

    // Find ToolResult entry
    let has_result = kernel.audit.entries().iter().any(|e| {
        matches!(
            &e.action,
            AuditAction::ToolResult { tool, success, .. } if tool == "file_read" && *success
        )
    });
    assert!(has_result, "Should have ToolResult audit entry");
}

#[tokio::test]
async fn test_pipeline_audit_chain_intact_after_tool_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("chain.txt"), "verify chain").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let _result = execute_governed(
        tool.as_ref(),
        json!({"path": "chain.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    // The entire audit chain should verify
    kernel
        .audit
        .verify_chain()
        .expect("Audit chain should be intact after tool execution");
}

#[tokio::test]
async fn test_pipeline_fuel_consumed_after_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("fuel.txt"), "fuel test").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let fuel_before = kernel.fuel.budget().consumed;

    let _result = execute_governed(
        tool.as_ref(),
        json!({"path": "fuel.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    let fuel_after = kernel.fuel.budget().consumed;
    assert!(
        fuel_after > fuel_before,
        "Fuel should be consumed after execution"
    );
}

#[tokio::test]
async fn test_pipeline_duration_recorded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("time.txt"), "timing test").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let result = execute_governed(
        tool.as_ref(),
        json!({"path": "time.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    // Duration should be recorded (>= 0 since it's timed)
    // We can't assert > 0 reliably since it might be sub-millisecond
    assert!(result.duration_ms < 10_000, "Shouldn't take 10 seconds");
}

#[tokio::test]
async fn test_pipeline_tool_result_summary_in_audit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("summary.txt"), "summary test").unwrap();

    let tool = create_tool("file_read").unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    let _result = execute_governed(
        tool.as_ref(),
        json!({"path": "summary.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    // The ToolResult audit entry should have a summary
    let result_entry = kernel
        .audit
        .entries()
        .iter()
        .find(|e| matches!(&e.action, AuditAction::ToolResult { .. }));
    assert!(result_entry.is_some());

    if let Some(entry) = result_entry {
        if let AuditAction::ToolResult { summary, .. } = &entry.action {
            assert!(summary.contains("OK"));
            assert!(summary.contains("ms"));
        }
    }
}

#[tokio::test]
async fn test_pipeline_multiple_tools_sequential() {
    let dir = tempfile::tempdir().unwrap();

    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);

    // Tool 1: file_read (Tier1) on a file that we create manually
    std::fs::write(dir.path().join("first.txt"), "first").unwrap();
    let tool1 = create_tool("file_read").unwrap();
    let r1 = execute_governed(
        tool1.as_ref(),
        json!({"path": "first.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();
    assert!(r1.is_success());

    // Tool 2: search (Tier1)
    let tool2 = create_tool("search").unwrap();
    let r2 = execute_governed(
        tool2.as_ref(),
        json!({"pattern": "first"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();
    assert!(r2.is_success());
    assert!(r2.output.contains("first"));

    // Tool 3: glob (Tier1)
    let tool3 = create_tool("glob").unwrap();
    let r3 = execute_governed(
        tool3.as_ref(),
        json!({"pattern": "*.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();
    assert!(r3.is_success());
    assert!(r3.output.contains("first.txt"));

    // Verify audit chain is intact after all 3
    kernel.audit.verify_chain().expect("Chain should be intact");
}

#[tokio::test]
async fn test_pipeline_consent_flow_with_audit_chain() {
    // Full consent flow: authorize -> consent needed -> approve -> execute
    // Verify audit trail captures all phases
    let dir = tempfile::tempdir().unwrap();
    let ctx = test_ctx(dir.path());
    let mut kernel = test_kernel(50_000);
    kernel
        .capabilities
        .grant(Capability::FileWrite, CapabilityScope::Full);

    let tool = create_tool("file_write").unwrap();
    let input = json!({"path": "consent_audit.txt", "content": "governed"});

    // Phase 1: ConsentRequired
    let result = execute_governed(tool.as_ref(), input.clone(), &ctx, &mut kernel).await;
    let request = match result {
        Err(nexus_code::error::NxError::ConsentRequired { request }) => request,
        other => panic!("Expected ConsentRequired, got {:?}", other),
    };

    // Should have ConsentRequested in audit
    let has_consent_requested = kernel.audit.entries().iter().any(|e| {
        matches!(
            &e.action,
            AuditAction::ConsentRequested { action, tier } if action == "file_write" && *tier == 2
        )
    });
    assert!(
        has_consent_requested,
        "Should have ConsentRequested audit entry"
    );

    // Phase 2: Approve and execute
    let result = execute_after_consent(tool.as_ref(), input, &ctx, &mut kernel, &request, true)
        .await
        .unwrap();
    assert!(result.is_success());

    // Should have ConsentGranted in audit
    let has_consent_granted = kernel.audit.entries().iter().any(|e| {
        matches!(
            &e.action,
            AuditAction::ConsentGranted { action } if action == "file_write"
        )
    });
    assert!(
        has_consent_granted,
        "Should have ConsentGranted audit entry"
    );

    // Full chain integrity
    kernel.audit.verify_chain().expect("Chain should be intact");
}

#[tokio::test]
async fn test_pipeline_registry_has_all_tools() {
    let registry = nexus_code::tools::ToolRegistry::with_defaults();
    let tools = registry.list();

    assert!(tools.contains(&"file_read"));
    assert!(tools.contains(&"file_write"));
    assert!(tools.contains(&"file_edit"));
    assert!(tools.contains(&"bash"));
    assert!(tools.contains(&"search"));
    assert!(tools.contains(&"glob"));
    assert!(tools.contains(&"git"));
    assert!(tools.contains(&"test_runner"));
    assert!(tools.contains(&"sub_agent"));
    assert!(tools.contains(&"project_index"));
    assert!(tools.contains(&"web_fetch"));
    assert_eq!(tools.len(), 11);

    // build_tool_prompt should produce non-empty output
    let prompt = registry.build_tool_prompt();
    assert!(prompt.contains("file_read"));
    assert!(prompt.contains("bash"));
    assert!(prompt.len() > 100);
}
