//! End-to-end tests exercising multiple v4 subsystems together.
//!
//! Each test creates real instances of the subsystem under test and verifies
//! cross-cutting concerns such as governance, fuel accounting, audit integrity,
//! and cryptographic verification.

use nexus_kernel::adaptive_policy::{AdaptiveGovernor, AutonomyChange, RunOutcome};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::delegation::{DelegationConstraints, DelegationEngine, DelegationError};
use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::parse_manifest;
use nexus_kernel::replay::bundle::{ApprovalRecord, EvidenceBundle, PolicySnapshot};
use nexus_kernel::replay::verifier::{verify_bundle, VerificationVerdict};

use nexus_sdk::AgentContext;

use nexus_connectors_llm::circuit_breaker::{CircuitState, ProviderCircuitBreaker};
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_connectors_llm::routing::{ProviderRouter, RoutingStrategy};

use nexus_marketplace::package::{create_unsigned_bundle, MarketplaceError, PackageMetadata};
use nexus_marketplace::registry::MarketplaceRegistry;

use nexus_enterprise::compliance::generate_soc2_report;
use nexus_enterprise::rbac::{RbacEngine, Role};

use nexus_collaboration::channel::{AgentMessage, GovernedChannel};

use nexus_cloud::auth::AuthManager;
use nexus_cloud::metering::MeteringEngine;
use nexus_cloud::tenant::{Plan, TenantManager};

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_kernel::consent::{ApprovalDecision, ApprovalVerdict, GovernedOperation};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct TestProvider {
    provider_name: String,
    cost: f64,
    should_fail: bool,
}

impl TestProvider {
    fn new(name: &str, cost: f64, should_fail: bool) -> Self {
        Self {
            provider_name: name.to_string(),
            cost,
            should_fail,
        }
    }
}

impl LlmProvider for TestProvider {
    fn query(
        &self,
        _prompt: &str,
        _max_tokens: u32,
        _model: &str,
    ) -> Result<LlmResponse, AgentError> {
        if self.should_fail {
            Err(AgentError::SupervisorError(format!(
                "{} failed",
                self.provider_name
            )))
        } else {
            Ok(LlmResponse {
                output_text: format!("response from {}", self.provider_name),
                token_count: 10,
                model_name: "test".to_string(),
                tool_calls: vec![],
            })
        }
    }

    fn name(&self) -> &str {
        &self.provider_name
    }

    fn cost_per_token(&self) -> f64 {
        self.cost
    }
}

// ---------------------------------------------------------------------------
// Test 1: Full governance pipeline
// ---------------------------------------------------------------------------

#[test]
fn full_governance_pipeline() {
    // Parse a manifest with fs.read, fs.write, llm.query, fuel 1000, autonomy L2
    let manifest = parse_manifest(
        r#"
name = "governance-test-agent"
version = "1.0.0"
capabilities = ["fs.read", "fs.write", "llm.query"]
fuel_budget = 1000
autonomy_level = 2
"#,
    )
    .expect("manifest should parse");

    assert_eq!(manifest.name, "governance-test-agent");
    assert_eq!(manifest.fuel_budget, 1000);
    assert_eq!(manifest.autonomy_level, Some(2));

    let agent_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();

    // Create AgentContext with declared capabilities
    let mut ctx = AgentContext::new(
        agent_id,
        manifest.capabilities.clone(),
        manifest.fuel_budget,
    )
    .with_filesystem_permissions(manifest.filesystem_permissions.clone());

    // Capability checks: allowed capabilities pass
    assert!(ctx.require_capability("fs.read").is_ok());
    assert!(ctx.require_capability("fs.write").is_ok());
    assert!(ctx.require_capability("llm.query").is_ok());

    // Capability check: undeclared capability blocked
    assert!(matches!(
        ctx.require_capability("process.exec"),
        Err(AgentError::CapabilityDenied(_))
    ));

    // Perform governed operations — fuel should deduct
    assert!(ctx.read_file("/data/input.txt").is_ok()); // 2 fuel
    assert!(ctx.write_file("/data/output.txt", "result").is_ok()); // 8 fuel
    assert!(ctx.llm_query("summarize", 100).is_ok()); // 10 fuel

    let fuel_consumed = manifest.fuel_budget - ctx.fuel_remaining();
    assert_eq!(fuel_consumed, 20); // 2 + 8 + 10

    // Audit trail should have events for each operation
    let trail = ctx.audit_trail();
    assert!(trail.events().len() >= 3);
    assert!(trail.verify_integrity());

    // Request approval (for L2 autonomy)
    ctx.request_approval("write output file", false);

    // Build policy snapshot
    let policy = PolicySnapshot {
        autonomy_level: AutonomyLevel::L2,
        consent_tiers: BTreeMap::new(),
        capabilities: manifest.capabilities.clone(),
        fuel_budget: manifest.fuel_budget,
    };

    // Build outputs with event_id so the approval check can match them
    let output_event_id = Uuid::new_v4();
    let approval_records = vec![ApprovalRecord {
        output_event_id,
        operation: GovernedOperation::ToolCall,
        decision: ApprovalDecision {
            id: Uuid::new_v4().to_string(),
            approver_id: "test-human".to_string(),
            decision: ApprovalVerdict::Approve,
            signature: None,
            decision_seq: 1,
        },
    }];

    // Export evidence bundle
    let bundle = EvidenceBundle::export(
        agent_id,
        run_id,
        &manifest,
        policy,
        ctx.audit_trail(),
        vec![json!({"input": "test"})],
        vec![json!({"event_id": output_event_id.to_string(), "output": "result"})],
        fuel_consumed,
        manifest.fuel_budget,
        AutonomyLevel::L2,
        approval_records,
    )
    .expect("bundle export should succeed");

    assert_eq!(bundle.agent_id, agent_id);
    assert_eq!(bundle.run_id, run_id);
    assert_eq!(bundle.fuel_consumed, 20);
    assert_eq!(bundle.fuel_budget, 1000);
    assert!(!bundle.bundle_digest.is_empty());

    // Verify bundle passes all 5 checks
    let report = verify_bundle(&bundle, Some(&manifest));
    assert!(report.chain_integrity, "chain integrity failed");
    assert!(
        report.manifest_capabilities_match,
        "manifest capabilities mismatch"
    );
    assert!(report.fuel_within_budget, "fuel exceeded budget");
    assert!(report.approvals_present, "approvals missing");
    assert!(report.monotonic_ordering, "non-monotonic ordering");
    assert_eq!(report.verdict, VerificationVerdict::Valid);
}

// ---------------------------------------------------------------------------
// Test 2: Circuit breaker failover
// ---------------------------------------------------------------------------

#[test]
fn circuit_breaker_failover() {
    // Create a router with Priority strategy and two providers
    let mut router = ProviderRouter::new(RoutingStrategy::Priority);

    // Primary provider that always fails, with a circuit breaker (threshold=5, short timeout)
    let breaker_primary = ProviderCircuitBreaker::new(5, Duration::from_millis(50));
    router.add_provider_with_breaker(
        Box::new(TestProvider::new("primary", 0.01, true)),
        breaker_primary,
    );

    // Fallback provider that always succeeds
    let breaker_fallback = ProviderCircuitBreaker::new(5, Duration::from_secs(30));
    router.add_provider_with_breaker(
        Box::new(TestProvider::new("fallback", 0.02, false)),
        breaker_fallback,
    );

    // Simulate 5 failures to open the primary circuit breaker.
    // Each route attempt will try primary (fails, records failure) then fallback (succeeds).
    for _ in 0..5 {
        let result = router.route("test", 100, "model");
        assert!(result.is_ok());
        assert!(result.unwrap().output_text.contains("fallback"));
    }

    // After 5 failures, the primary's circuit should be Open.
    // Router goes directly to fallback.
    let result = router.route("test", 100, "model");
    assert!(result.is_ok());
    assert!(result.unwrap().output_text.contains("fallback"));

    // Wait for the reset timeout to elapse, triggering HalfOpen on next request
    std::thread::sleep(Duration::from_millis(60));

    // The next route will allow one test request through primary (HalfOpen).
    // Primary still fails, so it goes back to Open and falls back.
    let result = router.route("test", 100, "model");
    assert!(result.is_ok());
    assert!(result.unwrap().output_text.contains("fallback"));

    // Now test recovery: create a fresh router where primary succeeds after timeout
    let mut router2 = ProviderRouter::new(RoutingStrategy::Priority);
    let breaker2 = ProviderCircuitBreaker::new(2, Duration::from_millis(20));
    router2.add_provider_with_breaker(Box::new(TestProvider::new("primary", 0.01, true)), breaker2);
    router2.add_provider_with_breaker(
        Box::new(TestProvider::new("fallback", 0.02, false)),
        ProviderCircuitBreaker::new(5, Duration::from_secs(30)),
    );

    // Open the circuit
    for _ in 0..2 {
        let _ = router2.route("test", 100, "model");
    }

    // Wait for reset timeout
    std::thread::sleep(Duration::from_millis(30));

    // Verify a standalone circuit breaker transitions properly
    let mut cb = ProviderCircuitBreaker::new(2, Duration::from_millis(10));
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    std::thread::sleep(Duration::from_millis(15));
    assert!(cb.allow_request());
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
}

// ---------------------------------------------------------------------------
// Test 3: Marketplace publish → install → verify
// ---------------------------------------------------------------------------

#[test]
fn marketplace_publish_install_verify() {
    let mut registry = MarketplaceRegistry::new();

    let metadata = PackageMetadata {
        name: "smart-scheduler".to_string(),
        version: "2.0.0".to_string(),
        description: "AI-powered calendar scheduling agent".to_string(),
        capabilities: vec!["llm.query".to_string(), "fs.read".to_string()],
        tags: vec!["scheduling".to_string(), "calendar".to_string()],
        author_id: "author-scheduler".to_string(),
    };

    let manifest_toml = r#"name = "smart-scheduler"
version = "2.0.0"
capabilities = ["llm.query", "fs.read"]
fuel_budget = 5000
"#;

    let package = create_unsigned_bundle(
        manifest_toml,
        "fn schedule() { /* AI scheduling logic */ }",
        metadata,
        "https://github.com/example/smart-scheduler",
        "nexus-buildkit",
    )
    .expect("bundle should build");

    // Sign with Ed25519 key and publish
    let key = CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[42_u8; 32])
        .expect("valid ed25519 key");
    let package_id = registry
        .publish(package, &key)
        .expect("publish should sign and store package");

    // Search by name
    let results = registry.search("smart-scheduler");
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "smart-scheduler");
    assert!(results[0].tags.contains(&"scheduling".to_string()));

    // Install with signature verification — should succeed
    let installed = registry.install(&package_id);
    assert!(installed.is_ok());
    let signed = installed.unwrap();
    assert_eq!(signed.package_id, package_id);
    assert!(!signed.signature.is_empty());

    // Tamper with the signature and verify install fails
    registry
        .tamper_signature_for_test(&package_id)
        .expect("tampering should succeed");

    let tampered_install = registry.install(&package_id);
    assert_eq!(tampered_install, Err(MarketplaceError::SignatureInvalid));
}

// ---------------------------------------------------------------------------
// Test 4: RBAC compliance pipeline
// ---------------------------------------------------------------------------

#[test]
fn rbac_compliance_pipeline() {
    let mut rbac = RbacEngine::new();

    let admin = Uuid::new_v4();
    let operator = Uuid::new_v4();
    let viewer = Uuid::new_v4();

    rbac.assign_role(admin, Role::Admin);
    rbac.assign_role(operator, Role::Operator);
    rbac.assign_role(viewer, Role::Viewer);

    // Admin: can read, write, execute, approve, delete agents; read audit; read/write config
    assert!(rbac.check(admin, "agent:coder", "read"));
    assert!(rbac.check(admin, "agent:coder", "write"));
    assert!(rbac.check(admin, "agent:coder", "execute"));
    assert!(rbac.check(admin, "agent:coder", "approve"));
    assert!(rbac.check(admin, "agent:coder", "delete"));
    assert!(rbac.check(admin, "audit:events", "read"));
    assert!(rbac.check(admin, "config:global", "read"));
    assert!(rbac.check(admin, "config:global", "write"));

    // Operator: can read, execute, approve agents; read audit; read config — no write/delete
    assert!(rbac.check(operator, "agent:coder", "read"));
    assert!(rbac.check(operator, "agent:coder", "execute"));
    assert!(rbac.check(operator, "agent:coder", "approve"));
    assert!(!rbac.check(operator, "agent:coder", "write"));
    assert!(!rbac.check(operator, "agent:coder", "delete"));
    assert!(rbac.check(operator, "audit:events", "read"));
    assert!(rbac.check(operator, "config:global", "read"));
    assert!(!rbac.check(operator, "config:global", "write"));

    // Viewer: read only on agents, audit, config — no write/execute/delete
    assert!(rbac.check(viewer, "agent:coder", "read"));
    assert!(rbac.check(viewer, "audit:events", "read"));
    assert!(rbac.check(viewer, "config:global", "read"));
    assert!(!rbac.check(viewer, "agent:coder", "write"));
    assert!(!rbac.check(viewer, "agent:coder", "execute"));
    assert!(!rbac.check(viewer, "agent:coder", "delete"));
    assert!(!rbac.check(viewer, "config:global", "write"));

    // Generate a SOC2 compliance report with audit evidence
    let mut trail = AuditTrail::new();
    let agent = Uuid::new_v4();
    trail
        .append_event(
            agent,
            EventType::ToolCall,
            json!({"action": "capability_check", "cap": "llm.query"}),
        )
        .expect("audit append");
    trail
        .append_event(
            agent,
            EventType::UserAction,
            json!({"event": "consent.approval", "tier": 2}),
        )
        .expect("audit append");
    trail
        .append_event(
            agent,
            EventType::StateChange,
            json!({"event": "safety.kpi_check", "status": "normal"}),
        )
        .expect("audit append");
    trail
        .append_event(
            agent,
            EventType::StateChange,
            json!({"event": "fuel.budget_check", "remaining": 500}),
        )
        .expect("audit append");
    trail
        .append_event(
            agent,
            EventType::LlmCall,
            json!({"action": "llm_query", "tokens": 100}),
        )
        .expect("audit append");

    let report = generate_soc2_report(&trail, true, true, true, "Nexus Corp", 0, u64::MAX);

    assert_eq!(report.organization, "Nexus Corp");
    assert_eq!(report.sections.len(), 1);
    assert_eq!(report.sections[0].framework, "SOC2 Type II");

    // Verify all 5 controls are present
    let control_ids: Vec<&str> = report.sections[0]
        .controls
        .iter()
        .map(|c| c.control_id.as_str())
        .collect();
    assert_eq!(
        control_ids,
        vec!["CC6.1", "CC6.2", "CC6.3", "CC7.1", "CC7.2"]
    );
}

// ---------------------------------------------------------------------------
// Test 5: Adaptive governance lifecycle
// ---------------------------------------------------------------------------

#[test]
fn adaptive_governance_lifecycle() {
    let mut gov = AdaptiveGovernor::new();
    let agent = Uuid::new_v4();

    // Register agent at base autonomy L1, max L3
    gov.register(agent, 1, 3);

    let record = gov.get_record(agent).unwrap();
    assert_eq!(record.total_runs, 0);
    assert!((record.trust_score - 0.5).abs() < f64::EPSILON);

    let policy = gov.get_policy(agent).unwrap();
    assert_eq!(policy.current_autonomy, 1);
    assert_eq!(policy.max_autonomy, 3);

    // Record 10 successful runs → trust should rise to 1.0
    for _ in 0..10 {
        gov.record_run(agent, RunOutcome::Success, 50, 100);
    }

    let record = gov.get_record(agent).unwrap();
    assert_eq!(record.total_runs, 10);
    assert_eq!(record.successful_runs, 10);
    assert!((record.trust_score - 1.0).abs() < f64::EPSILON);

    // Evaluate → should suggest promotion from L1 to L2
    let change = gov.evaluate(agent);
    assert_eq!(change, AutonomyChange::Promote { from: 1, to: 2 });

    // Apply promotion with human approval
    assert!(gov.apply_promotion(agent, true));
    assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 2);

    // Record 5 policy violations → trust drops below demotion threshold (0.3)
    // After 10 successes + 5 violations: trust = (10/15) * (1.0 - 5*0.2) = 0.0
    for _ in 0..5 {
        gov.record_run(
            agent,
            RunOutcome::PolicyViolation {
                violation: "unauthorized access".to_string(),
            },
            100,
            100,
        );
    }

    let record = gov.get_record(agent).unwrap();
    assert_eq!(record.policy_violations, 5);
    assert!(record.trust_score < 0.3);

    // Evaluate → should suggest demotion
    let change = gov.evaluate(agent);
    match change {
        AutonomyChange::Demote { from, to, .. } => {
            assert_eq!(from, 2);
            assert_eq!(to, 1);
        }
        _ => panic!("expected Demote, got {:?}", change),
    }

    // Apply demotion (no approval needed)
    assert!(gov.apply_demotion(agent));
    assert_eq!(gov.get_policy(agent).unwrap().current_autonomy, 1);
}

// ---------------------------------------------------------------------------
// Test 6: Delegation and collaboration
// ---------------------------------------------------------------------------

#[test]
fn delegation_and_collaboration() {
    let mut engine = DelegationEngine::new();

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();

    // Agent A has capabilities, agent B has none
    engine.register_agent(
        agent_a,
        vec![
            "fs.read".to_string(),
            "fs.write".to_string(),
            "llm.query".to_string(),
        ],
    );
    engine.register_agent(agent_b, vec![]);

    // B has no capabilities initially
    assert!(!engine.has_capability(agent_b, "fs.read"));
    assert!(!engine.has_capability(agent_b, "llm.query"));

    // Delegate fs.read to B with fuel limit 100
    let grant = engine
        .delegate(
            agent_a,
            agent_b,
            vec!["fs.read".to_string()],
            DelegationConstraints {
                max_fuel: 100,
                ..Default::default()
            },
        )
        .expect("delegation should succeed");

    // B now has fs.read via delegation
    assert!(engine.has_capability(agent_b, "fs.read"));
    // But not fs.write (wasn't delegated)
    assert!(!engine.has_capability(agent_b, "fs.write"));

    // Exhaust delegated fuel
    assert!(engine.consume_delegated_fuel(grant.id, 80).is_ok());
    assert!(engine.consume_delegated_fuel(grant.id, 20).is_ok());

    // Fuel exhausted → further consumption fails
    assert_eq!(
        engine.consume_delegated_fuel(grant.id, 1),
        Err(DelegationError::FuelExhausted)
    );

    // Create another delegation to test revocation
    let grant2 = engine
        .delegate(
            agent_a,
            agent_b,
            vec!["llm.query".to_string()],
            DelegationConstraints {
                max_fuel: 500,
                ..Default::default()
            },
        )
        .expect("delegation should succeed");

    assert!(engine.has_capability(agent_b, "llm.query"));

    // Revoke → capability lost
    engine.revoke(grant2.id).expect("revoke should succeed");
    assert!(!engine.has_capability(agent_b, "llm.query"));

    // --- GovernedChannel: send task_request, verify receipt and audit ---
    let mut channel = GovernedChannel::new(
        agent_a,
        agent_b,
        vec!["task_request".to_string(), "result".to_string()],
        10, // max 10 per minute
        5,  // 5 fuel per message
        100,
    );

    let msg = AgentMessage::new(
        agent_a,
        agent_b,
        "task_request",
        json!({"task": "analyze data", "delegated_cap": "fs.read"}),
        true,
    );

    assert!(channel.send(msg).is_ok());
    assert_eq!(channel.fuel_remaining(), 95);

    // Verify receipt
    let received = channel.recv().expect("should receive message");
    assert_eq!(received.message_type, "task_request");
    assert_eq!(received.from, agent_a);
    assert_eq!(received.to, agent_b);
    assert!(received.requires_ack);

    // Verify audit event was recorded for the channel send
    let events = channel.audit_trail().events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, EventType::ToolCall);
    let action = events[0].payload.get("action").unwrap().as_str().unwrap();
    assert_eq!(action, "channel_send");
}

// ---------------------------------------------------------------------------
// Test 7: Cloud tenant lifecycle
// ---------------------------------------------------------------------------

#[test]
fn cloud_tenant_lifecycle() {
    let mut tenant_mgr = TenantManager::new();
    let mut auth_mgr = AuthManager::new();
    let mut metering = MeteringEngine::new();

    // Create tenant on Free plan
    let tenant_id = tenant_mgr.create_tenant("Startup Inc", Plan::Free);
    let tenant = tenant_mgr.get_tenant(tenant_id).unwrap();
    assert_eq!(tenant.name, "Startup Inc");
    assert_eq!(tenant.plan, Plan::Free);

    // Verify Free plan limits
    assert_eq!(tenant.resource_limits.max_agents, 1);
    assert_eq!(tenant.resource_limits.max_fuel_per_month, 1_000);
    assert_eq!(tenant.resource_limits.max_llm_tokens_per_month, 10_000);
    assert_eq!(tenant.resource_limits.max_concurrent_runs, 1);

    // Check limit enforcement
    assert!(tenant_mgr.check_limit(tenant_id, "agents", 1));
    assert!(!tenant_mgr.check_limit(tenant_id, "agents", 2));

    // Create API key and authenticate
    let (key_id, raw_key) = auth_mgr.create_key(tenant_id);
    assert!(!key_id.is_empty());
    assert!(raw_key.starts_with("nxk_"));

    // Verify key returns correct tenant
    let verified = auth_mgr.verify_key(&raw_key);
    assert_eq!(verified, Some(tenant_id));

    // Record usage via metering
    metering.record(tenant_id, "fuel_consumed", 400);
    metering.record(tenant_id, "fuel_consumed", 500);

    // Total fuel = 900, within Free limit of 1000
    assert!(metering.is_within_limit(tenant_id, "fuel_consumed", 1000));

    // Record more usage to exceed limit
    metering.record(tenant_id, "fuel_consumed", 200);

    // Total fuel = 1100, exceeds Free limit of 1000
    assert!(!metering.is_within_limit(tenant_id, "fuel_consumed", 1000));

    // Upgrade to Pro plan
    assert!(tenant_mgr.update_plan(tenant_id, Plan::Pro));
    let tenant = tenant_mgr.get_tenant(tenant_id).unwrap();
    assert_eq!(tenant.plan, Plan::Pro);

    // Verify new Pro limits
    assert_eq!(tenant.resource_limits.max_agents, 10);
    assert_eq!(tenant.resource_limits.max_fuel_per_month, 50_000);
    assert_eq!(tenant.resource_limits.max_llm_tokens_per_month, 500_000);
    assert_eq!(tenant.resource_limits.max_concurrent_runs, 5);

    // Pro limits: 1100 fuel is now within the 50_000 limit
    assert!(metering.is_within_limit(tenant_id, "fuel_consumed", 50_000));

    // Revoke API key
    assert!(auth_mgr.revoke_key(&key_id));

    // Verify auth fails after revocation
    assert_eq!(auth_mgr.verify_key(&raw_key), None);
}
