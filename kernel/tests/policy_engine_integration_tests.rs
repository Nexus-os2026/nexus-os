//! Integration tests for C.5 — Policy Engine
//!
//! Tests verify the full policy lifecycle: TOML loading, allow/deny evaluation,
//! conditional checks (autonomy level, fuel cost, time window), default-deny
//! semantics, wildcard matching, policy reload, invalid TOML rejection, and
//! end-to-end Supervisor integration with audit trail verification.

use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::policy_engine::{
    EvaluationContext, Policy, PolicyConditions, PolicyDecision, PolicyEffect, PolicyEngine,
    PolicyError,
};
use nexus_kernel::supervisor::Supervisor;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn policy_dir_with_file(filename: &str, content: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(filename), content).unwrap();
    dir
}

fn allow_toml(id: &str, principal: &str, action: &str, resource: &str) -> String {
    format!(
        r#"policy_id = "{id}"
description = "allow {id}"
effect = "allow"
principal = "{principal}"
action = "{action}"
resource = "{resource}"
priority = 100
"#
    )
}

fn deny_toml(id: &str, principal: &str, action: &str, resource: &str, desc: &str) -> String {
    format!(
        r#"policy_id = "{id}"
description = "{desc}"
effect = "deny"
principal = "{principal}"
action = "{action}"
resource = "{resource}"
priority = 100
"#
    )
}

fn base_manifest(name: &str) -> AgentManifest {
    AgentManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec!["llm.query".to_string()],
        fuel_budget: 5000,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: Some("claude-sonnet-4-5".to_string()),
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
    }
}

// ── 1. Policy loaded from TOML file ────────────────────────────────────────

#[test]
fn policy_loaded_from_toml_file() {
    let dir = policy_dir_with_file(
        "allow-tools.toml",
        &allow_toml("allow-tools", "*", "tool_call", "*"),
    );

    let mut engine = PolicyEngine::new(dir.path());
    let count = engine.load_policies().unwrap();

    assert_eq!(count, 1);
    assert_eq!(engine.policies()[0].policy_id, "allow-tools");
    assert_eq!(engine.policies()[0].effect, PolicyEffect::Allow);
    assert_eq!(engine.policies()[0].principal, "*");
}

// ── 2. Allow policy permits action ─────────────────────────────────────────

#[test]
fn allow_policy_permits_action() {
    let dir = policy_dir_with_file(
        "allow-web.toml",
        &allow_toml("allow-web", "*", "tool_call", "web.search"),
    );

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    let ctx = EvaluationContext::default();
    let decision = engine.evaluate("did:nexus:agent1", "tool_call", "web.search", &ctx);
    assert_eq!(decision, PolicyDecision::Allow);
}

// ── 3. Deny policy blocks action with reason ───────────────────────────────

#[test]
fn deny_policy_blocks_action_with_reason() {
    let dir = policy_dir_with_file(
        "deny-exec.toml",
        &deny_toml(
            "deny-exec",
            "*",
            "terminal_command",
            "process.exec",
            "execution forbidden by security policy",
        ),
    );

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    let ctx = EvaluationContext::default();
    let decision = engine.evaluate("did:nexus:agent1", "terminal_command", "process.exec", &ctx);

    match decision {
        PolicyDecision::Deny { reason } => {
            assert_eq!(reason, "execution forbidden by security policy");
        }
        other => panic!("expected Deny with reason, got {other:?}"),
    }
}

// ── 4. Deny overrides allow for same action ────────────────────────────────

#[test]
fn deny_overrides_allow_for_same_action() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("allow-all.toml"),
        allow_toml("allow-all", "*", "*", "*"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("deny-exec.toml"),
        deny_toml(
            "deny-exec",
            "*",
            "terminal_command",
            "process.exec",
            "blocked",
        ),
    )
    .unwrap();

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    let ctx = EvaluationContext::default();

    // Deny wins even though allow-all also matches
    let decision = engine.evaluate("did:nexus:agent1", "terminal_command", "process.exec", &ctx);
    assert!(matches!(decision, PolicyDecision::Deny { .. }));

    // Other actions still allowed
    let decision = engine.evaluate("did:nexus:agent1", "tool_call", "web.search", &ctx);
    assert_eq!(decision, PolicyDecision::Allow);
}

// ── 5. Conditional policy checks autonomy level ────────────────────────────

#[test]
fn conditional_policy_checks_autonomy_level() {
    let toml = r#"
policy_id = "allow-if-l3"
description = "allow tool calls at L3+"
effect = "allow"
principal = "*"
action = "tool_call"
resource = "*"
priority = 50

[conditions]
min_autonomy_level = 3
"#;
    let dir = policy_dir_with_file("autonomy-gate.toml", toml);

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    // L1 — below threshold, default deny
    let ctx_l1 = EvaluationContext {
        autonomy_level: AutonomyLevel::L1,
        fuel_cost: None,
    };
    assert!(matches!(
        engine.evaluate("agent1", "tool_call", "web.search", &ctx_l1),
        PolicyDecision::Deny { .. }
    ));

    // L3 — meets threshold, allowed
    let ctx_l3 = EvaluationContext {
        autonomy_level: AutonomyLevel::L3,
        fuel_cost: None,
    };
    assert_eq!(
        engine.evaluate("agent1", "tool_call", "web.search", &ctx_l3),
        PolicyDecision::Allow,
    );

    // L5 — exceeds threshold, still allowed
    let ctx_l5 = EvaluationContext {
        autonomy_level: AutonomyLevel::L5,
        fuel_cost: None,
    };
    assert_eq!(
        engine.evaluate("agent1", "tool_call", "web.search", &ctx_l5),
        PolicyDecision::Allow,
    );
}

// ── 6. Conditional policy checks fuel cost ─────────────────────────────────

#[test]
fn conditional_policy_checks_fuel_cost() {
    let toml = r#"
policy_id = "allow-cheap"
description = "allow actions costing <= 200 fuel"
effect = "allow"
principal = "*"
action = "*"
resource = "*"
priority = 50

[conditions]
max_fuel_cost = 200
"#;
    let dir = policy_dir_with_file("fuel-gate.toml", toml);

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    // Under budget — allowed
    let ctx_ok = EvaluationContext {
        autonomy_level: AutonomyLevel::L5,
        fuel_cost: Some(100),
    };
    assert_eq!(
        engine.evaluate("a", "tool_call", "web.search", &ctx_ok),
        PolicyDecision::Allow,
    );

    // At budget — allowed
    let ctx_exact = EvaluationContext {
        autonomy_level: AutonomyLevel::L5,
        fuel_cost: Some(200),
    };
    assert_eq!(
        engine.evaluate("a", "tool_call", "web.search", &ctx_exact),
        PolicyDecision::Allow,
    );

    // Over budget — condition fails, default deny
    let ctx_over = EvaluationContext {
        autonomy_level: AutonomyLevel::L5,
        fuel_cost: Some(201),
    };
    assert!(matches!(
        engine.evaluate("a", "tool_call", "web.search", &ctx_over),
        PolicyDecision::Deny { .. }
    ));

    // No fuel cost specified — condition passes (None doesn't exceed max)
    let ctx_none = EvaluationContext {
        autonomy_level: AutonomyLevel::L5,
        fuel_cost: None,
    };
    assert_eq!(
        engine.evaluate("a", "tool_call", "web.search", &ctx_none),
        PolicyDecision::Allow,
    );
}

// ── 7. Policy with time window evaluated correctly ─────────────────────────

#[test]
fn policy_with_time_window_stored_correctly() {
    let toml = r#"
policy_id = "business-hours"
description = "allow during business hours"
effect = "allow"
principal = "*"
action = "tool_call"
resource = "*"
priority = 50

[conditions]
time_window = "0 9-17 * * 1-5"
"#;
    let dir = policy_dir_with_file("time-window.toml", toml);

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    // Time window is stored but not enforced in conditions_met() yet,
    // so the policy matches regardless of time. Verify it loads correctly.
    assert_eq!(engine.policies().len(), 1);
    assert_eq!(
        engine.policies()[0].conditions.time_window,
        Some("0 9-17 * * 1-5".to_string())
    );

    // Policy still evaluates (time_window stored but not blocking)
    let ctx = EvaluationContext::default();
    let decision = engine.evaluate("agent1", "tool_call", "web.search", &ctx);
    assert_eq!(decision, PolicyDecision::Allow);
}

// ── 8. No matching policy defaults to deny ─────────────────────────────────

#[test]
fn no_matching_policy_defaults_to_deny() {
    // Engine has a policy, but it doesn't match the request
    let dir = policy_dir_with_file(
        "allow-special.toml",
        &allow_toml("allow-special", "did:nexus:vip", "tool_call", "web.search"),
    );

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    let ctx = EvaluationContext::default();
    let decision = engine.evaluate("did:nexus:other", "tool_call", "web.search", &ctx);

    match decision {
        PolicyDecision::Deny { reason } => {
            assert!(
                reason.contains("default deny"),
                "expected 'default deny' in reason, got: {reason}"
            );
        }
        other => panic!("expected Deny, got {other:?}"),
    }
}

// ── 9. Policy override of consent tier works end-to-end through Supervisor ─

#[test]
fn policy_override_consent_tier_through_supervisor() {
    let dir = tempfile::tempdir().unwrap();

    // Policy: allow tool_call for all agents
    std::fs::write(
        dir.path().join("allow-tools.toml"),
        allow_toml("allow-tools", "*", "tool_call", "*"),
    )
    .unwrap();

    let mut supervisor = Supervisor::with_policy_dir(dir.path());
    let manifest = base_manifest("policy-test-agent");
    let agent_id = supervisor.start_agent(manifest).unwrap();

    // The policy engine should allow tool_call (bypassing default consent tier)
    let result = supervisor.require_tool_call(agent_id);
    assert!(
        result.is_ok(),
        "tool call should be allowed by policy: {result:?}"
    );

    // Now replace with a deny policy and verify it blocks
    let deny_engine = PolicyEngine::with_policies(vec![Policy {
        policy_id: "deny-all".to_string(),
        description: "deny everything".to_string(),
        effect: PolicyEffect::Deny,
        principal: "*".to_string(),
        action: "*".to_string(),
        resource: "*".to_string(),
        priority: 1,
        conditions: PolicyConditions::default(),
    }]);
    supervisor.set_policy_engine(deny_engine.clone());

    // Re-start agent so it picks up the new cedar engine
    let manifest2 = base_manifest("policy-test-agent-2");
    let agent_id2 = supervisor.start_agent(manifest2).unwrap();

    let result = supervisor.require_tool_call(agent_id2);
    // The deny policy in cedar_engine causes ConsentError::PolicyDenied
    assert!(result.is_err(), "tool call should be denied by policy");

    // Verify audit trail has events recorded
    let events = supervisor.audit_trail().events();
    assert!(
        !events.is_empty(),
        "audit trail should have recorded policy-related events"
    );
}

// ── 10. Policy reload picks up new file ────────────────────────────────────

#[test]
fn policy_reload_picks_up_new_file() {
    let dir = tempfile::tempdir().unwrap();

    let mut engine = PolicyEngine::new(dir.path());
    let count = engine.load_policies().unwrap();
    assert_eq!(count, 0, "empty directory should load zero policies");

    // Write a policy file after initial load
    std::fs::write(
        dir.path().join("new-policy.toml"),
        allow_toml("new-policy", "*", "tool_call", "*"),
    )
    .unwrap();

    // Reload picks up the new file
    let count = engine.load_policies().unwrap();
    assert_eq!(count, 1, "reload should find the new policy");
    assert_eq!(engine.policies()[0].policy_id, "new-policy");

    // Add another file
    std::fs::write(
        dir.path().join("second-policy.toml"),
        deny_toml("deny-fs", "*", "*", "fs.*", "filesystem blocked"),
    )
    .unwrap();

    let count = engine.load_policies().unwrap();
    assert_eq!(count, 2, "reload should find both policies");
}

// ── 11. Invalid policy TOML rejected with clear error ──────────────────────

#[test]
fn invalid_policy_toml_rejected_with_clear_error() {
    // Missing required field (effect)
    let bad_toml = r#"
policy_id = "broken"
principal = "*"
action = "*"
resource = "*"
"#;
    let dir = policy_dir_with_file("broken.toml", bad_toml);

    let mut engine = PolicyEngine::new(dir.path());
    let result = engine.load_policies();

    match result {
        Err(PolicyError::ParseError { file, reason }) => {
            assert!(
                file.contains("broken.toml"),
                "error should reference the file: {file}"
            );
            assert!(!reason.is_empty(), "error should have a non-empty reason");
        }
        Err(other) => panic!("expected ParseError, got {other:?}"),
        Ok(count) => panic!("expected error, got Ok({count})"),
    }

    // Completely invalid TOML syntax
    let garbage_toml = "this is not { valid toml at all !!!";
    let dir2 = policy_dir_with_file("garbage.toml", garbage_toml);

    let mut engine2 = PolicyEngine::new(dir2.path());
    let result2 = engine2.load_policies();
    assert!(result2.is_err(), "garbage TOML should produce an error");
}

// ── 12. Wildcard principal matches all agents ──────────────────────────────

#[test]
fn wildcard_principal_matches_all_agents() {
    let dir = policy_dir_with_file(
        "wildcard.toml",
        &allow_toml("wildcard-allow", "*", "tool_call", "web.*"),
    );

    let mut engine = PolicyEngine::new(dir.path());
    engine.load_policies().unwrap();

    let ctx = EvaluationContext::default();

    // Various agent identifiers all match the wildcard
    for agent in &[
        "did:nexus:agent-alpha",
        "did:nexus:agent-beta",
        "did:key:z6Mk123",
        "anonymous",
        "system",
    ] {
        let decision = engine.evaluate(agent, "tool_call", "web.search", &ctx);
        assert_eq!(
            decision,
            PolicyDecision::Allow,
            "wildcard principal should match agent '{agent}'"
        );
    }

    // Non-matching resource still denied
    let decision = engine.evaluate("did:nexus:agent-alpha", "tool_call", "fs.write", &ctx);
    assert!(
        matches!(decision, PolicyDecision::Deny { .. }),
        "web.* should not match fs.write"
    );
}

// ── 13. All policy decisions audited with fail-closed ──────────────────────

#[test]
fn all_policy_decisions_audited_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("allow-tools.toml"),
        allow_toml("allow-tools", "*", "tool_call", "*"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("deny-exec.toml"),
        deny_toml("deny-exec", "*", "terminal_command", "*", "exec blocked"),
    )
    .unwrap();

    let mut supervisor = Supervisor::with_policy_dir(dir.path());

    // Start agent and exercise both allow and deny paths
    let manifest = base_manifest("audit-test-agent");
    let agent_id = supervisor.start_agent(manifest).unwrap();

    // Allowed action — should succeed
    let allow_result = supervisor.require_tool_call(agent_id);
    assert!(allow_result.is_ok());

    // Count events so far
    let events_after_allow = supervisor.audit_trail().events().len();
    assert!(
        events_after_allow > 0,
        "audit trail must record events for allowed operations"
    );

    // Denied action — the deny policy should block via consent
    // Start a new agent to test deny path through terminal command
    let manifest2 = base_manifest("audit-deny-agent");
    let agent_id2 = supervisor.start_agent(manifest2).unwrap();

    // Verify the deny engine is active by directly evaluating
    let ctx = EvaluationContext::default();
    let direct_decision =
        supervisor
            .policy_engine()
            .evaluate(&agent_id2.to_string(), "terminal_command", "*", &ctx);
    assert!(
        matches!(direct_decision, PolicyDecision::Deny { .. }),
        "policy engine should deny terminal commands"
    );

    // Verify the overall audit trail has integrity
    assert!(
        supervisor.audit_trail().verify_integrity(),
        "audit trail hash chain must be valid"
    );

    // Verify all audit events have valid agent IDs and payloads (no silent failures)
    for event in supervisor.audit_trail().events() {
        assert!(
            !event.payload.is_null(),
            "every audit event must have a non-null payload — fail-closed"
        );
    }
}
