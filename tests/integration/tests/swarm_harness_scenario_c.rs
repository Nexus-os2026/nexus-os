//! Phase B.3 — Scenario C: oracle plan denial.
//!
//! Drives `Director::plan` against a synthetic oracle that returns
//! `GovernanceDecision::Denied`. Asserts:
//!   - plan() returns Err(SwarmError::OraclePolicyDenied { hints }).
//!   - The hints vector is non-empty (synthesized locally by the
//!     bridge per ADR caveat — never claimed to be oracle-authored).
//!   - audit_trail.events() count is 0 (Director path doesn't audit
//!     today; same observation as scenario A).
//!   - SwarmCoordinator::run is NEVER reached. Subscribing to the
//!     events channel and draining for 500ms returns no events
//!     because no one ever calls run().

use nexus_integration::swarm_harness::prelude::*;
use std::time::Duration;

const CANNED_TWO_NODE_PLAN: &str = r#"{
    "nodes": [
        {
            "id": "n1",
            "capability_id": "herald.post",
            "profile": {
                "reasoning": "Light",
                "tool_use": "None",
                "latency": "Batch",
                "context": "Small",
                "privacy": "StrictLocal",
                "cost": "Free"
            },
            "inputs": {}
        },
        {
            "id": "n2",
            "capability_id": "scout.research",
            "profile": {
                "reasoning": "Light",
                "tool_use": "None",
                "latency": "Batch",
                "context": "Small",
                "privacy": "StrictLocal",
                "cost": "Free"
            },
            "inputs": {}
        }
    ],
    "edges": [{"from": "n1", "to": "n2"}]
}"#;

#[tokio::test]
async fn plan_under_oracle_denial_returns_error_without_audit() {
    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_TWO_NODE_PLAN)
        .with_synthetic_capability("herald.post")
        .with_synthetic_capability("scout.research")
        .with_oracle_decision(GovernanceDecision::Denied)
        .build();

    let event_rx = scenario.events_tx.subscribe();

    let plan_result = scenario.plan("post a tweet about Rust").await;

    match plan_result {
        Err(SwarmError::OraclePolicyDenied { ref hints }) => {
            assert!(
                !hints.is_empty(),
                "the bridge synthesizes at least one local hint on plan denial"
            );
        }
        other => panic!("expected SwarmError::OraclePolicyDenied, got {other:?}"),
    }

    // Coordinator was never reached — no events should have fired
    // through the broadcast channel at all.
    let early_events = drain_for_duration(event_rx, Duration::from_millis(500)).await;
    assert!(
        early_events.is_empty(),
        "no events should arrive when plan() denies before run() is called: {early_events:?}"
    );

    // Director-only path emits no audit events today; matches the
    // scenario A observation.
    let audit = scenario.audit.lock().unwrap();
    assert_eq!(
        audit.events().len(),
        0,
        "Director-only path emits no audit events today"
    );

    // Capability call log is empty because the coordinator never
    // executed any node.
    assert!(
        scenario.capability_call_log().is_empty(),
        "no capability should fire on a denied plan"
    );
}
