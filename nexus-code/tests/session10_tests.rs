//! Session 10 tests — coordinator fuel manager, WebFetchTool, 3 critical validation tests.

use nexus_code::coordinator::fuel_manager::CoordinatorFuelManager;
use nexus_code::coordinator::CoordinatorConfig;
use nexus_code::tools::web_fetch::strip_html_tags;

// ═══════════════════════════════════════════════════════
// Fuel Manager Tests (12)
// ═══════════════════════════════════════════════════════

#[test]
fn test_fuel_manager_creation() {
    let mgr = CoordinatorFuelManager::new(50_000);
    assert_eq!(mgr.session_budget(), 50_000);
    assert_eq!(mgr.available_for_allocation(), 50_000);
    assert_eq!(mgr.total_consumed(), 0);
    assert_eq!(mgr.coordinator_consumed(), 0);
}

#[test]
fn test_fuel_allocate() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 10_000).unwrap();
    assert_eq!(mgr.total_allocated_to_children(), 10_000);
    assert_eq!(mgr.available_for_allocation(), 40_000);
}

#[test]
fn test_fuel_allocate_exceeds_budget() {
    let mut mgr = CoordinatorFuelManager::new(10_000);
    let result = mgr.allocate("child-1", 15_000);
    assert!(result.is_err());
}

#[test]
fn test_fuel_allocate_duplicate() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 5_000).unwrap();
    let result = mgr.allocate("child-1", 5_000);
    assert!(result.is_err());
}

#[test]
fn test_fuel_update_consumption() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 10_000).unwrap();
    mgr.update_child_consumption("child-1", 3_000, 2);
    assert_eq!(mgr.total_consumed(), 3_000);
    assert_eq!(mgr.slices()["child-1"].consumed, 3_000);
    assert_eq!(mgr.slices()["child-1"].successful_tool_count, 2);
}

#[test]
fn test_fuel_reclaim() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 10_000).unwrap();
    mgr.update_child_consumption("child-1", 6_000, 3);
    let reclaimed = mgr.reclaim_fuel("child-1");
    assert_eq!(reclaimed, 4_000);
    assert!(mgr.slices()["child-1"].terminated);
    assert_eq!(mgr.slices()["child-1"].allocated, 6_000); // Shrunk to consumed
}

#[test]
fn test_fuel_detect_runaway() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 10_000).unwrap();
    mgr.update_child_consumption("child-1", 8_500, 0); // >80%, 0 results
    let runaways = mgr.detect_runaways();
    assert_eq!(runaways.len(), 1);
    assert_eq!(runaways[0], "child-1");
}

#[test]
fn test_fuel_not_runaway_with_results() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("child-1", 10_000).unwrap();
    mgr.update_child_consumption("child-1", 9_000, 3); // >80% but 3 results
    assert!(mgr.detect_runaways().is_empty());
}

#[test]
fn test_fuel_invariant() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("c1", 15_000).unwrap();
    mgr.allocate("c2", 15_000).unwrap();
    mgr.record_coordinator_consumption(5_000);
    // 15k + 15k allocated + 5k consumed = 35k used, 15k available
    assert_eq!(mgr.available_for_allocation(), 15_000);
    assert!(mgr.allocate("c3", 15_000).is_ok());
    assert_eq!(mgr.available_for_allocation(), 0);
    assert!(mgr.allocate("c4", 1).is_err());
}

#[test]
fn test_fuel_coordinator_consumption() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.record_coordinator_consumption(2_000);
    assert_eq!(mgr.coordinator_consumed(), 2_000);
    assert_eq!(mgr.total_consumed(), 2_000);
    assert_eq!(mgr.available_for_allocation(), 48_000);
}

#[test]
fn test_fuel_summary_format() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("c1", 10_000).unwrap();
    mgr.record_coordinator_consumption(1_000);
    let summary = mgr.summary();
    assert!(summary.contains("50000"));
    assert!(summary.contains("Available:"));
}

#[test]
fn test_fuel_slice_usage_percentage() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("c1", 10_000).unwrap();
    mgr.update_child_consumption("c1", 8_000, 5);
    let slice = &mgr.slices()["c1"];
    assert!((slice.usage_percentage() - 80.0).abs() < 0.1);
    assert_eq!(slice.remaining(), 2_000);
}

// ═══════════════════════════════════════════════════════
// Coordinator Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_coordinator_config_defaults() {
    let config = CoordinatorConfig::default();
    assert_eq!(config.research_workers, 2);
    assert_eq!(config.research_fuel, 8_000);
    assert_eq!(config.implementation_fuel, 10_000);
    assert_eq!(config.verification_fuel, 5_000);
    assert_eq!(config.worker_max_turns, 8);
}

#[test]
fn test_coordinator_total_fuel_calculation() {
    let config = CoordinatorConfig::default();
    let total = nexus_code::coordinator::total_fuel_needed(&config);
    // 2*8000 + 10000 + 5000 = 31000
    assert_eq!(total, 31_000);
}

#[test]
fn test_coordinator_fuel_check_insufficient() {
    let mut mgr = CoordinatorFuelManager::new(20_000);
    // Need 31_000 but only have 20_000
    let result = mgr.allocate("worker", 31_000);
    assert!(result.is_err());
}

#[test]
fn test_coordinator_result_fields() {
    let result = nexus_code::coordinator::CoordinatorResult {
        success: true,
        summary: "Done".to_string(),
        research_findings: vec!["finding 1".to_string()],
        implementation_result: Some("implemented".to_string()),
        verification_result: Some("verified".to_string()),
        total_fuel_consumed: 25_000,
        worker_count: 4,
        fuel_summary: "summary".to_string(),
    };
    assert!(result.success);
    assert_eq!(result.worker_count, 4);
    assert_eq!(result.total_fuel_consumed, 25_000);
}

// ═══════════════════════════════════════════════════════
// WebFetchTool Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_strip_html_basic() {
    assert_eq!(strip_html_tags("<p>hello</p>"), "hello");
    assert_eq!(strip_html_tags("<div><span>world</span></div>"), "world");
}

#[test]
fn test_strip_html_scripts() {
    let html = "<p>before</p><script>alert('xss')</script><p>after</p>";
    let text = strip_html_tags(html);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("alert"));
}

#[test]
fn test_strip_html_nested() {
    let html = "<div class='outer'><p>Some <b>bold</b> text</p></div>";
    let text = strip_html_tags(html);
    assert!(text.contains("Some"));
    assert!(text.contains("bold"));
    assert!(text.contains("text"));
    assert!(!text.contains("class"));
}

#[test]
fn test_web_fetch_invalid_url() {
    let tool = nexus_code::tools::web_fetch::WebFetchTool;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = nexus_code::tools::ToolContext {
        working_dir: std::env::temp_dir(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };
    let result = rt.block_on(nexus_code::tools::NxTool::execute(
        &tool,
        serde_json::json!({"url": "ftp://invalid"}),
        &ctx,
    ));
    assert!(!result.is_success());
    assert!(result.output.contains("http"));
}

// ═══════════════════════════════════════════════════════
// Integration Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_web_fetch_capability_required() {
    let tool = nexus_code::tools::web_fetch::WebFetchTool;
    let cap = nexus_code::tools::NxTool::required_capability(&tool, &serde_json::json!({}));
    assert_eq!(cap, Some(nexus_code::governance::Capability::NetworkAccess));
}

#[test]
fn test_fuel_slice_hard_enforcement() {
    // A child with 100 fuel budget — FuelMeter enforces the cap
    let mut meter = nexus_code::governance::FuelMeter::new(100);
    assert_eq!(meter.remaining(), 100);
    // Reserve 50 OK
    meter.reserve(50).unwrap();
    // Reserve 60 more — only 50 remaining after first reserve
    let result = meter.reserve(60);
    assert!(result.is_err());
}

#[test]
fn test_fuel_rollup_to_parent() {
    let mut mgr = CoordinatorFuelManager::new(50_000);
    mgr.allocate("c1", 10_000).unwrap();
    mgr.allocate("c2", 10_000).unwrap();
    mgr.update_child_consumption("c1", 5_000, 3);
    mgr.update_child_consumption("c2", 7_000, 5);
    mgr.record_coordinator_consumption(2_000);
    // Total consumed = 5000 + 7000 + 2000 = 14000
    assert_eq!(mgr.total_consumed(), 14_000);
}

// ═══════════════════════════════════════════════════════
// ═══ THREE CRITICAL 10/10 VALIDATION TESTS ═══
// ═══════════════════════════════════════════════════════

#[test]
fn test_atomic_fuel_allocation_stress() {
    let mut mgr = CoordinatorFuelManager::new(50_000);

    // Workers 1-4 succeed (48,000 allocated)
    assert!(mgr.allocate("child-1", 12_000).is_ok());
    assert!(mgr.allocate("child-2", 12_000).is_ok());
    assert!(mgr.allocate("child-3", 12_000).is_ok());
    assert!(mgr.allocate("child-4", 12_000).is_ok());

    assert_eq!(mgr.total_allocated_to_children(), 48_000);
    assert_eq!(mgr.available_for_allocation(), 2_000);

    // Worker 5 MUST fail — 12,000 > 2,000 remaining
    let result = mgr.allocate("child-5", 12_000);
    assert!(result.is_err());
    match result {
        Err(nexus_code::error::NxError::FuelExhausted {
            remaining,
            required,
        }) => {
            assert_eq!(remaining, 2_000);
            assert_eq!(required, 12_000);
        }
        other => panic!("Expected FuelExhausted, got {:?}", other),
    }

    // CRITICAL: No fuel leaked after failed allocation
    assert_eq!(mgr.total_allocated_to_children(), 48_000);
    assert_eq!(mgr.available_for_allocation(), 2_000);
    assert_eq!(mgr.slices().len(), 4);
    assert!(!mgr.slices().contains_key("child-5"));

    // Verify invariant: consumed + allocated + available == budget
    assert_eq!(
        mgr.coordinator_consumed()
            + mgr.total_allocated_to_children()
            + mgr.available_for_allocation(),
        50_000
    );
}

#[test]
fn test_runaway_detection_and_kill() {
    let mut mgr = CoordinatorFuelManager::new(50_000);

    mgr.allocate("research-1", 10_000).unwrap();

    // 50% — NOT yet runaway
    mgr.update_child_consumption("research-1", 5_000, 0);
    assert!(mgr.detect_runaways().is_empty());

    // 75% — still NOT runaway (threshold >80%)
    mgr.update_child_consumption("research-1", 7_500, 0);
    assert!(mgr.detect_runaways().is_empty());

    // 81% — NOW a runaway
    mgr.update_child_consumption("research-1", 8_100, 0);
    let runaways = mgr.detect_runaways();
    assert_eq!(runaways.len(), 1);
    assert_eq!(runaways[0], "research-1");

    // Kill the runaway
    mgr.terminate_child(
        "research-1",
        "Runaway: >80% fuel consumed with no useful output",
    );

    let slice = mgr.slices().get("research-1").unwrap();
    assert!(slice.terminated);
    assert_eq!(
        slice.termination_reason,
        Some("Runaway: >80% fuel consumed with no useful output".to_string())
    );

    // Reclaim unused fuel
    let reclaimed = mgr.reclaim_fuel("research-1");
    assert_eq!(reclaimed, 1_900); // 10,000 - 8,100 = 1,900

    // After reclaim, fuel is available again
    assert!(mgr.available_for_allocation() >= 1_900);

    // Worker WITH results is NOT a runaway
    mgr.allocate("research-2", 10_000).unwrap();
    mgr.update_child_consumption("research-2", 9_000, 3); // 90% but 3 results
    assert!(mgr.detect_runaways().is_empty());

    // Terminated workers excluded from runaway detection
    assert!(mgr.detect_runaways().is_empty());
}

#[tokio::test]
async fn test_coordinator_persistence_audit_rollup() {
    use sha2::Digest;

    let mut kernel = nexus_code::governance::GovernanceKernel::new(50_000).unwrap();

    // Simulate 4 worker spawns + completions
    let worker_fuel = [8_500u64, 7_200, 9_800, 4_500]; // Total: 30,000

    for (i, fuel) in worker_fuel.iter().enumerate() {
        let child_id = format!("worker-{}", i + 1);

        kernel
            .audit
            .record(nexus_code::governance::AuditAction::ToolInvocation {
                tool: "coordinator".to_string(),
                args_summary: format!("Spawned {} with {}fu", child_id, fuel),
            });

        kernel
            .audit
            .record(nexus_code::governance::AuditAction::ToolResult {
                tool: "coordinator".to_string(),
                success: true,
                summary: format!("{} completed: {}fu consumed", child_id, fuel),
            });
    }

    let total_fuel: u64 = worker_fuel.iter().sum();
    kernel.fuel.consume(
        "coordinator",
        nexus_code::governance::FuelCost {
            input_tokens: 0,
            output_tokens: 0,
            fuel_units: total_fuel,
            estimated_usd: 0.0,
        },
    );

    // Verify audit chain
    assert!(kernel.audit.verify_chain().is_ok());

    // Verify fuel state
    assert_eq!(kernel.fuel.budget().consumed, total_fuel);
    assert_eq!(kernel.fuel.remaining(), 50_000 - total_fuel);

    // 1 SessionStarted + 8 tool entries = 9
    assert_eq!(kernel.audit.len(), 9);

    // Create session data and sign it
    let messages = vec![
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::User,
            content: "coordinate task".to_string(),
        },
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::Assistant,
            content: "Coordinator complete".to_string(),
        },
    ];

    let session_data = serde_json::json!({
        "session_id": kernel.identity.session_id(),
        "saved_at": chrono::Utc::now().to_rfc3339(),
        "messages": messages,
        "fuel_consumed": total_fuel,
        "audit_entry_count": kernel.audit.len(),
    });
    let hash_bytes = sha2::Sha256::digest(serde_json::to_string(&session_data).unwrap().as_bytes());
    let content_hash = hex::encode(hash_bytes);
    let sig = kernel
        .identity
        .sign(hex::decode(&content_hash).unwrap().as_slice());
    let signature = hex::encode(sig.to_bytes());

    // Verify hash and signature
    assert_eq!(content_hash.len(), 64);
    assert!(!signature.is_empty());

    let hash_bytes_verify = hex::decode(&content_hash).unwrap();
    let sig_bytes = hex::decode(&signature).unwrap();
    let sig_verify = ed25519_dalek::Signature::from_bytes(sig_bytes.as_slice().try_into().unwrap());
    assert!(kernel.identity.verify(&hash_bytes_verify, &sig_verify));

    // TAMPER TEST: change fuel_consumed
    let tampered_data = serde_json::json!({
        "session_id": kernel.identity.session_id(),
        "saved_at": chrono::Utc::now().to_rfc3339(),
        "messages": messages,
        "fuel_consumed": 999u64,
        "audit_entry_count": kernel.audit.len(),
    });
    let tampered_hash = hex::encode(sha2::Sha256::digest(
        serde_json::to_string(&tampered_data).unwrap().as_bytes(),
    ));

    // Tampered hash MUST differ
    assert_ne!(tampered_hash, content_hash);

    // Original signature MUST NOT verify against tampered hash
    let tampered_hash_bytes = hex::decode(&tampered_hash).unwrap();
    assert!(!kernel.identity.verify(&tampered_hash_bytes, &sig_verify));
}
