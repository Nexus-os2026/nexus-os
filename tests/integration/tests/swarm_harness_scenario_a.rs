//! Phase B.1 — Scenario A: plan-only smoke.
//!
//! Drives `Director::plan` against a synthetic planner provider that
//! returns a canned two-node `PlanSchema`. Asserts:
//!   - `plan` returns Ok(PlannedSwarm).
//!   - The DAG contains both planned nodes.
//!   - `ticket.dag_content_hash` is a fresh SHA-256 of the built DAG.
//!   - The harness's `AuditTrail` is empty after the Director path.
//!
//! That last assertion is a deliberate observation, not a regression
//! gate. The Director currently has no audit hook (Phase 1 of ADR 0004
//! shipped audit on `SecretsFacade`; Director auditing is out of
//! scope for the harness track). If audit semantics change, this
//! assertion flips and tells us.

use nexus_integration::swarm_harness::prelude::*;

const CANNED_TWO_NODE_PLAN: &str = r#"{
    "nodes": [
        {
            "id": "n1",
            "capability_id": "herald.post",
            "profile": {
                "reasoning": "Light",
                "tool_use": "Basic",
                "latency": "Interactive",
                "context": "Medium",
                "privacy": "Public",
                "cost": "Low"
            },
            "inputs": {}
        },
        {
            "id": "n2",
            "capability_id": "scout.research",
            "profile": {
                "reasoning": "Light",
                "tool_use": "Basic",
                "latency": "Interactive",
                "context": "Medium",
                "privacy": "Public",
                "cost": "Low"
            },
            "inputs": {}
        }
    ],
    "edges": [{"from": "n1", "to": "n2"}]
}"#;

#[tokio::test]
async fn plan_only_smoke_emits_well_formed_plan() {
    let scenario = ScenarioBuilder::new()
        .with_canned_plan(CANNED_TWO_NODE_PLAN)
        .with_synthetic_capability("herald.post")
        .with_synthetic_capability("scout.research")
        .build();

    let planned = scenario
        .plan("post a tweet about Rust")
        .await
        .expect("Director::plan should accept the canned two-node plan");

    assert_eq!(
        planned.dag.node_count(),
        2,
        "canned plan declares two nodes"
    );

    assert_eq!(
        planned.ticket.dag_content_hash,
        dag_content_hash(&planned.dag),
        "ticket.dag_content_hash must be a fresh SHA-256 of the built DAG"
    );

    let audit_events = scenario.audit.lock().unwrap();
    assert_eq!(
        audit_events.events().len(),
        0,
        "Director-only path emits no audit events today; this assertion \
         is a deliberate observation. If Director gains an audit hook, \
         this flips and tells us."
    );

    assert_eq!(
        scenario.planner_provider.invocation_count(),
        1,
        "Director should contact the planner exactly once for a valid plan"
    );
}
