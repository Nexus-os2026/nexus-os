//! Integration tests for Phase 7.1 — A2A + MCP Protocol Integration
//!
//! Tests verify that the protocol layer (A2A Agent Cards, A2A tasks, MCP tool
//! discovery, MCP tool invocation) is fully governed: capability checks, fuel
//! accounting, speculative execution, audit trail integrity, and sender auth
//! all enforced — no governance bypass possible through external protocols.

use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::protocols::a2a::{
    AgentCard, MessagePart, MessageRole, TaskMessage, TaskPayload, TaskStatus, A2A_PROTOCOL_VERSION,
};
use nexus_kernel::protocols::bridge::{A2ATaskRequest, GovernanceBridge, McpInvokeRequest};
use nexus_kernel::protocols::mcp::McpServer;
use serde_json::json;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn full_manifest(name: &str) -> AgentManifest {
    AgentManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec![
            "web.search".to_string(),
            "web.read".to_string(),
            "llm.query".to_string(),
            "fs.read".to_string(),
            "fs.write".to_string(),
            "process.exec".to_string(),
            "social.post".to_string(),
            "social.x.post".to_string(),
            "social.x.read".to_string(),
            "messaging.send".to_string(),
            "audit.read".to_string(),
        ],
        fuel_budget: 100_000,
        autonomy_level: Some(3),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: Some("claude-sonnet-4-5".to_string()),
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
    }
}

fn limited_manifest(name: &str, caps: Vec<&str>, fuel: u64) -> AgentManifest {
    AgentManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: caps.into_iter().map(String::from).collect(),
        fuel_budget: fuel,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
    }
}

fn text_payload(text: &str) -> TaskPayload {
    TaskPayload {
        message: TaskMessage {
            role: MessageRole::User,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            metadata: None,
        },
        metadata: None,
    }
}

// ── Test 1: A2A Agent Card generated from manifest with all capabilities mapped ──

#[test]
fn a2a_agent_card_maps_all_capabilities_to_skills() {
    let manifest = full_manifest("protocol-agent");
    let card = AgentCard::from_manifest(&manifest, "https://nexus.local:3000");

    // Card identity
    assert_eq!(card.name, "protocol-agent");
    assert_eq!(card.version, A2A_PROTOCOL_VERSION);
    assert_eq!(card.url, "https://nexus.local:3000/a2a/protocol-agent");

    // All 11 capabilities must map to skills
    assert_eq!(
        card.skills.len(),
        11,
        "all 11 capabilities must become A2A skills"
    );

    let skill_ids: Vec<&str> = card.skills.iter().map(|s| s.id.as_str()).collect();
    let expected = [
        "web-search",
        "web-read",
        "llm-query",
        "fs-read",
        "fs-write",
        "process-exec",
        "social-post",
        "social-x-post",
        "social-x-read",
        "messaging-send",
        "audit-read",
    ];
    for id in &expected {
        assert!(skill_ids.contains(id), "missing skill: {id}");
    }

    // Every skill must have description, tags, input/output modes
    for skill in &card.skills {
        assert!(
            skill.description.is_some(),
            "skill '{}' needs description",
            skill.id
        );
        assert!(!skill.tags.is_empty(), "skill '{}' needs tags", skill.id);
        assert!(
            !skill.input_modes.is_empty(),
            "skill '{}' needs input_modes",
            skill.id
        );
        assert!(
            !skill.output_modes.is_empty(),
            "skill '{}' needs output_modes",
            skill.id
        );
    }

    // Auth: L3 autonomy → bearer + mTLS
    assert_eq!(card.authentication.len(), 2);
    let auth_types: Vec<&str> = card
        .authentication
        .iter()
        .map(|a| a.scheme_type.as_str())
        .collect();
    assert!(auth_types.contains(&"bearer"));
    assert!(auth_types.contains(&"mtls"));

    // Rate limit from fuel budget: 100_000 / 100 = 1000 RPM
    assert_eq!(card.rate_limit_rpm, Some(1000));

    // LLM capability → state_transition_history enabled
    assert!(card.capabilities.state_transition_history);

    // Card round-trips through JSON
    let json_str = serde_json::to_string_pretty(&card).unwrap();
    let parsed: AgentCard = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.skills.len(), 11);
    assert_eq!(parsed.name, "protocol-agent");
}

// ── Test 2: A2A task lifecycle — submit → working → complete ──

#[test]
fn a2a_task_lifecycle_submit_to_complete() {
    let mut bridge = GovernanceBridge::new();
    bridge.register_agent(full_manifest("lifecycle-agent"));

    // Submit a task
    let request = A2ATaskRequest {
        sender_id: "external-client".to_string(),
        receiver_agent: "lifecycle-agent".to_string(),
        payload: text_payload("search for Rust best practices"),
        correlation_id: Some("lifecycle-test".to_string()),
    };

    let response = bridge.handle_a2a_task(request).unwrap();
    let mut task = response.task;

    // Bridge moves task to Working
    assert_eq!(task.status, TaskStatus::Working);
    assert_eq!(task.sender, "external-client");
    assert_eq!(task.receiver, "lifecycle-agent");
    assert_eq!(task.correlation_id, Some("lifecycle-test".to_string()));
    assert!(!task.id.is_empty());

    // Governance context is attached
    let gov = task.governance.as_ref().unwrap();
    assert_eq!(gov.fuel_consumed, 1);
    assert!(gov
        .required_capabilities
        .contains(&"web.search".to_string()));
    assert!(gov.audit_hash.is_some());

    // Task can transition Working → Completed
    assert!(task.transition_to(TaskStatus::Completed));
    assert_eq!(task.status, TaskStatus::Completed);
    assert!(task.status.is_terminal());

    // Terminal state cannot transition further
    assert!(!task.transition_to(TaskStatus::Working));
    assert_eq!(task.status, TaskStatus::Completed);
}

// ── Test 3: MCP tool discovery returns governed tools only ──

#[test]
fn mcp_tool_discovery_returns_governed_tools_only() {
    let mut server = McpServer::new();
    let agent_id = uuid::Uuid::new_v4();

    // Agent with only 3 capabilities
    let manifest = limited_manifest(
        "limited-agent",
        vec!["web.search", "fs.read", "audit.read"],
        5000,
    );
    server.register_agent(agent_id, manifest);

    let tools = server.list_tools(agent_id).unwrap();

    // Only the 3 declared capabilities appear as tools
    assert_eq!(tools.len(), 3);
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"web_search"));
    assert!(tool_names.contains(&"fs_read"));
    assert!(tool_names.contains(&"audit_read"));

    // Undeclared capabilities must NOT appear
    assert!(!tool_names.contains(&"fs_write"));
    assert!(!tool_names.contains(&"process_exec"));
    assert!(!tool_names.contains(&"llm_query"));
    assert!(!tool_names.contains(&"social_post"));
    assert!(!tool_names.contains(&"messaging_send"));

    // Every returned tool has governance metadata
    for tool in &tools {
        assert!(!tool.governance.required_capabilities.is_empty());
        assert!(tool.governance.estimated_fuel_cost > 0);
        assert!(tool.description.is_some());
        assert_eq!(tool.input_schema["type"], "object");
    }
}

// ── Test 4: MCP tool invocation goes through capability and fuel checks ──

#[test]
fn mcp_invocation_enforces_capability_and_fuel_checks() {
    let mut bridge = GovernanceBridge::new();
    let manifest = limited_manifest("mcp-agent", vec!["web.search", "fs.read"], 10_000);
    bridge.register_agent(manifest);

    // Authorized tool call succeeds
    let request = McpInvokeRequest {
        caller_id: "mcp-client".to_string(),
        agent_name: "mcp-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "Rust governance"}),
    };
    let response = bridge.handle_mcp_invoke(request).unwrap();
    assert!(!response.result.is_error);
    assert!(response.result.fuel_consumed > 0);
    assert!(response.result.audit_hash.is_some());

    // Tool that agent doesn't have → CapabilityDenied
    let request = McpInvokeRequest {
        caller_id: "mcp-client".to_string(),
        agent_name: "mcp-agent".to_string(),
        tool_name: "fs_write".to_string(),
        params: json!({"path": "/tmp/x", "content": "hack"}),
    };
    let result = bridge.handle_mcp_invoke(request);
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("CapabilityDenied"),
        "expected CapabilityDenied, got: {err_msg}"
    );

    // Fuel exhaustion check: create a fuel-starved agent
    let mut bridge2 = GovernanceBridge::new();
    bridge2.register_agent(limited_manifest("starved-agent", vec!["web.search"], 10));

    let request = McpInvokeRequest {
        caller_id: "client".to_string(),
        agent_name: "starved-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "test"}),
    };
    let result = bridge2.handle_mcp_invoke(request);
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("FuelExhausted"),
        "expected FuelExhausted, got: {err_msg}"
    );
}

// ── Test 5: Governance bridge rejects unauthorized A2A sender ──

#[test]
fn governance_bridge_rejects_unauthorized_a2a_sender() {
    let mut bridge = GovernanceBridge::with_allowed_senders(vec![
        "trusted-partner".to_string(),
        "internal-agent".to_string(),
    ]);
    bridge.register_agent(full_manifest("secure-agent"));

    // Trusted sender succeeds
    let request = A2ATaskRequest {
        sender_id: "trusted-partner".to_string(),
        receiver_agent: "secure-agent".to_string(),
        payload: text_payload("search for something"),
        correlation_id: None,
    };
    assert!(bridge.handle_a2a_task(request).is_ok());

    // Untrusted sender rejected
    let request = A2ATaskRequest {
        sender_id: "malicious-actor".to_string(),
        receiver_agent: "secure-agent".to_string(),
        payload: text_payload("steal data"),
        correlation_id: None,
    };
    let result = bridge.handle_a2a_task(request);
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("malicious-actor"));
    assert!(err_msg.contains("CapabilityDenied"));

    // Empty string sender also rejected
    let request = A2ATaskRequest {
        sender_id: "".to_string(),
        receiver_agent: "secure-agent".to_string(),
        payload: text_payload("anonymous request"),
        correlation_id: None,
    };
    assert!(bridge.handle_a2a_task(request).is_err());

    // Rejection event appears in audit trail
    let rejection_events: Vec<_> = bridge
        .audit_trail()
        .events()
        .iter()
        .filter(|e| {
            e.payload.get("event_kind").and_then(|v| v.as_str()) == Some("bridge.sender_rejected")
        })
        .collect();
    assert!(
        rejection_events.len() >= 2,
        "both rejections must be audited, found {}",
        rejection_events.len()
    );
}

// ── Test 6: Governance bridge rejects MCP call without capability ──

#[test]
fn governance_bridge_rejects_mcp_without_capability() {
    let mut bridge = GovernanceBridge::new();
    // Agent only has web.search
    bridge.register_agent(limited_manifest(
        "restricted-agent",
        vec!["web.search"],
        50_000,
    ));

    // Every tool the agent doesn't have must be rejected
    let unauthorized_tools = [
        "fs_write",
        "fs_read",
        "process_exec",
        "social_post",
        "social_x_post",
        "messaging_send",
        "llm_query",
    ];

    for tool in &unauthorized_tools {
        let request = McpInvokeRequest {
            caller_id: "client".to_string(),
            agent_name: "restricted-agent".to_string(),
            tool_name: tool.to_string(),
            params: json!({}),
        };
        let result = bridge.handle_mcp_invoke(request);
        assert!(
            result.is_err(),
            "tool '{}' should be denied for restricted agent",
            tool
        );
    }

    // Authorized tool still works
    let request = McpInvokeRequest {
        caller_id: "client".to_string(),
        agent_name: "restricted-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "allowed"}),
    };
    assert!(bridge.handle_mcp_invoke(request).is_ok());
}

// ── Test 7: Speculative execution triggers for high-risk protocol requests ──

#[test]
fn speculative_execution_triggers_for_high_risk_requests() {
    let mut bridge = GovernanceBridge::new();
    bridge.register_agent(full_manifest("speculative-agent"));

    // Tier0 (web.search) — no simulation
    let request = A2ATaskRequest {
        sender_id: "client".to_string(),
        receiver_agent: "speculative-agent".to_string(),
        payload: text_payload("search for cats"),
        correlation_id: None,
    };
    let response = bridge.handle_a2a_task(request).unwrap();
    assert!(
        response.simulation.is_none(),
        "Tier0 should NOT trigger simulation"
    );

    // Tier1 (llm.query) — no simulation (only Tier2+ triggers)
    let request = A2ATaskRequest {
        sender_id: "client".to_string(),
        receiver_agent: "speculative-agent".to_string(),
        payload: text_payload("explain quantum computing"),
        correlation_id: None,
    };
    let response = bridge.handle_a2a_task(request).unwrap();
    assert!(
        response.simulation.is_none(),
        "Tier1 should NOT trigger simulation"
    );

    // Tier2 (fs.write) — SHOULD trigger simulation
    let request = A2ATaskRequest {
        sender_id: "client".to_string(),
        receiver_agent: "speculative-agent".to_string(),
        payload: text_payload("write file to /tmp/output.txt"),
        correlation_id: None,
    };
    let response = bridge.handle_a2a_task(request).unwrap();
    assert!(
        response.simulation.is_some(),
        "Tier2 (fs.write) MUST trigger simulation"
    );
    let sim = response.simulation.unwrap();
    assert_eq!(
        sim.agent_id,
        bridge.agent_id_by_name("speculative-agent").unwrap()
    );

    // Tier3 (process.exec) — SHOULD trigger simulation
    let request = A2ATaskRequest {
        sender_id: "client".to_string(),
        receiver_agent: "speculative-agent".to_string(),
        payload: text_payload("execute command ls"),
        correlation_id: None,
    };
    let response = bridge.handle_a2a_task(request).unwrap();
    assert!(
        response.simulation.is_some(),
        "Tier3 (process.exec) MUST trigger simulation"
    );

    // MCP Tier3 tool also triggers simulation
    let request = McpInvokeRequest {
        caller_id: "client".to_string(),
        agent_name: "speculative-agent".to_string(),
        tool_name: "process_exec".to_string(),
        params: json!({"command": "ls"}),
    };
    let response = bridge.handle_mcp_invoke(request).unwrap();
    assert!(
        response.simulation.is_some(),
        "MCP process_exec MUST trigger simulation"
    );

    // MCP Tier0 tool does NOT trigger simulation
    let request = McpInvokeRequest {
        caller_id: "client".to_string(),
        agent_name: "speculative-agent".to_string(),
        tool_name: "fs_read".to_string(),
        params: json!({"path": "/tmp/safe.txt"}),
    };
    let response = bridge.handle_mcp_invoke(request).unwrap();
    assert!(
        response.simulation.is_none(),
        "MCP fs_read should NOT trigger simulation"
    );
}

// ── Test 8: All protocol actions appear in audit trail ──

#[test]
fn all_protocol_actions_appear_in_audit_trail() {
    let mut bridge = GovernanceBridge::with_allowed_senders(vec!["trusted".to_string()]);
    bridge.register_agent(full_manifest("audited-agent"));

    // Record baseline
    let baseline = bridge.audit_trail().events().len();

    // 1. Successful A2A task
    let request = A2ATaskRequest {
        sender_id: "trusted".to_string(),
        receiver_agent: "audited-agent".to_string(),
        payload: text_payload("search for docs"),
        correlation_id: None,
    };
    bridge.handle_a2a_task(request).unwrap();

    // 2. Rejected A2A (unauthorized sender)
    let request = A2ATaskRequest {
        sender_id: "untrusted".to_string(),
        receiver_agent: "audited-agent".to_string(),
        payload: text_payload("hack"),
        correlation_id: None,
    };
    let _ = bridge.handle_a2a_task(request);

    // 3. Successful MCP invocation
    let request = McpInvokeRequest {
        caller_id: "trusted".to_string(),
        agent_name: "audited-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "test"}),
    };
    bridge.handle_mcp_invoke(request).unwrap();

    // 4. Rejected MCP (unauthorized caller)
    let request = McpInvokeRequest {
        caller_id: "untrusted".to_string(),
        agent_name: "audited-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "test"}),
    };
    let _ = bridge.handle_mcp_invoke(request);

    let events = bridge.audit_trail().events();
    let new_events: Vec<_> = events.iter().skip(baseline).collect();

    // At minimum: a2a_task_accepted + sender_rejected + mcp_tool_invoked + sender_rejected
    assert!(
        new_events.len() >= 4,
        "expected at least 4 new audit events, got {}",
        new_events.len()
    );

    // Check specific event kinds present
    let event_kinds: Vec<String> = new_events
        .iter()
        .filter_map(|e| {
            e.payload
                .get("event_kind")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    assert!(event_kinds.contains(&"bridge.a2a_task_accepted".to_string()));
    assert!(event_kinds.contains(&"bridge.sender_rejected".to_string()));
    assert!(event_kinds.contains(&"bridge.mcp_tool_invoked".to_string()));

    // Hash-chain integrity must hold across all events
    assert!(
        bridge.audit_trail().verify_integrity(),
        "audit trail hash chain must be valid after all operations"
    );
}

// ── Test 9: No governance bypass possible through external protocols ──

#[test]
fn no_governance_bypass_through_external_protocols() {
    // This test systematically verifies that every bypass vector is blocked.

    let mut bridge = GovernanceBridge::with_allowed_senders(vec!["legit".to_string()]);
    bridge.register_agent(limited_manifest(
        "locked-agent",
        vec!["web.search", "fs.read"],
        500,
    ));

    // Bypass attempt 1: Unregistered agent name
    let request = A2ATaskRequest {
        sender_id: "legit".to_string(),
        receiver_agent: "nonexistent-agent".to_string(),
        payload: text_payload("search for something"),
        correlation_id: None,
    };
    assert!(
        bridge.handle_a2a_task(request).is_err(),
        "unregistered agent must be rejected"
    );

    // Bypass attempt 2: Empty agent name
    let request = A2ATaskRequest {
        sender_id: "legit".to_string(),
        receiver_agent: "".to_string(),
        payload: text_payload("search for something"),
        correlation_id: None,
    };
    assert!(
        bridge.handle_a2a_task(request).is_err(),
        "empty agent name must be rejected"
    );

    // Bypass attempt 3: Capability escalation via A2A
    let request = A2ATaskRequest {
        sender_id: "legit".to_string(),
        receiver_agent: "locked-agent".to_string(),
        payload: text_payload("execute rm -rf /"),
        correlation_id: None,
    };
    let result = bridge.handle_a2a_task(request);
    assert!(result.is_err(), "process.exec capability must be denied");

    // Bypass attempt 4: Capability escalation via MCP
    let request = McpInvokeRequest {
        caller_id: "legit".to_string(),
        agent_name: "locked-agent".to_string(),
        tool_name: "process_exec".to_string(),
        params: json!({"command": "rm -rf /"}),
    };
    assert!(
        bridge.handle_mcp_invoke(request).is_err(),
        "process_exec must be denied via MCP"
    );

    // Bypass attempt 5: MCP social.post without capability
    let request = McpInvokeRequest {
        caller_id: "legit".to_string(),
        agent_name: "locked-agent".to_string(),
        tool_name: "social_post".to_string(),
        params: json!({"content": "spam"}),
    };
    assert!(
        bridge.handle_mcp_invoke(request).is_err(),
        "social_post must be denied via MCP"
    );

    // Bypass attempt 6: Drain fuel then try again
    // web_search costs 50 fuel, agent has 500 → 10 calls then exhausted
    for i in 0..10 {
        let request = McpInvokeRequest {
            caller_id: "legit".to_string(),
            agent_name: "locked-agent".to_string(),
            tool_name: "web_search".to_string(),
            params: json!({"query": format!("q{i}")}),
        };
        let _ = bridge.handle_mcp_invoke(request);
    }
    // 11th call must fail
    let request = McpInvokeRequest {
        caller_id: "legit".to_string(),
        agent_name: "locked-agent".to_string(),
        tool_name: "web_search".to_string(),
        params: json!({"query": "one more"}),
    };
    assert!(
        bridge.handle_mcp_invoke(request).is_err(),
        "fuel-exhausted agent must reject further calls"
    );

    // Bypass attempt 7: Unauthorized sender after fuel drain
    let request = A2ATaskRequest {
        sender_id: "hacker".to_string(),
        receiver_agent: "locked-agent".to_string(),
        payload: text_payload("search for something"),
        correlation_id: None,
    };
    assert!(
        bridge.handle_a2a_task(request).is_err(),
        "sender auth must still be enforced"
    );

    // Verify bypass attempts were audited — at minimum: sender rejections
    // and capability denials that happen at bridge level. MCP tool-level
    // denials are audited in the MCP server's own trail.
    let error_events: Vec<_> = bridge
        .audit_trail()
        .events()
        .iter()
        .filter(|e| {
            let kind = e
                .payload
                .get("event_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            kind.contains("denied") || kind.contains("rejected") || kind.contains("exhausted")
        })
        .collect();
    assert!(
        !error_events.is_empty(),
        "denial events must appear in audit trail"
    );

    // Full audit chain integrity after all operations
    assert!(bridge.audit_trail().verify_integrity());
}
