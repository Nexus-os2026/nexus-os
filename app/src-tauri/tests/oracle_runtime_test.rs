//! Integration tests for the Phase 1.5a GovernanceOracle runtime wiring.
//!
//! These tests exercise `OracleRuntime` directly rather than spinning up a
//! full Tauri app fixture — the runtime is the thing under test and a
//! Tauri fixture costs ~1s per test for zero additional coverage. The
//! OracleRuntime status is the same surface the `oracle_runtime_status`
//! Tauri command exposes (the command delegates to `state.oracle_runtime_status()`
//! which delegates to `OracleRuntime::status()`).

use nexus_desktop_backend::oracle_runtime::OracleRuntime;
use nexus_governance_engine::{GovernanceRule, GovernanceRuleset, RuleCondition, RuleEffect};
use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision, OracleRequest};
use std::time::Duration;
use tokio::sync::oneshot;

fn ruleset_allowing(caps: &[&str]) -> GovernanceRuleset {
    GovernanceRuleset::new(
        "test".into(),
        1,
        vec![GovernanceRule {
            id: "allow".into(),
            description: "Allow listed capabilities".into(),
            effect: RuleEffect::Allow,
            conditions: vec![RuleCondition::CapabilityInSet(
                caps.iter().map(|s| (*s).into()).collect(),
            )],
        }],
    )
}

fn empty_ruleset() -> GovernanceRuleset {
    GovernanceRuleset::new("test-empty".into(), 1, vec![])
}

fn make_request(capability: &str) -> CapabilityRequest {
    CapabilityRequest {
        agent_id: "test-agent".into(),
        capability: capability.into(),
        parameters: serde_json::json!({}),
        budget_hash: String::new(),
        request_nonce: uuid::Uuid::new_v4().to_string(),
    }
}

fn blocking_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime for tests")
}

#[test]
fn three_requests_all_receive_responses() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(ruleset_allowing(&["llm.query"]));
        let sender = runtime.sender();

        let mut receivers = Vec::new();
        for _ in 0..3 {
            let req = make_request("llm.query");
            let (resp_tx, resp_rx) = oneshot::channel();
            sender
                .send(OracleRequest {
                    request: req,
                    response_tx: resp_tx,
                })
                .await
                .expect("send");
            receivers.push(resp_rx);
        }

        let mut decisions = Vec::new();
        for rx in receivers {
            let decision = tokio::time::timeout(Duration::from_secs(2), rx)
                .await
                .expect("decision timely")
                .expect("oneshot delivered");
            decisions.push(decision);
        }
        assert_eq!(decisions.len(), 3);
        for d in &decisions {
            assert!(matches!(d, GovernanceDecision::Approved { .. }));
        }

        runtime.shutdown();
    });
}

#[test]
fn counter_grows_with_processed_requests() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(ruleset_allowing(&["llm.query"]));
        let sender = runtime.sender();

        assert_eq!(runtime.total_processed(), 0);

        for _ in 0..5 {
            let req = make_request("llm.query");
            let (resp_tx, resp_rx) = oneshot::channel();
            sender
                .send(OracleRequest {
                    request: req,
                    response_tx: resp_tx,
                })
                .await
                .expect("send");
            let _ = tokio::time::timeout(Duration::from_secs(2), resp_rx)
                .await
                .expect("decision timely")
                .expect("oneshot delivered");
        }

        // Give the relay's fetch_add a moment to settle after the last oneshot.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(runtime.total_processed(), 5);

        runtime.shutdown();
    });
}

#[test]
fn pending_metric_reports_zero_when_drained() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(ruleset_allowing(&["llm.query"]));
        let sender = runtime.sender();

        for _ in 0..3 {
            let req = make_request("llm.query");
            let (resp_tx, resp_rx) = oneshot::channel();
            sender
                .send(OracleRequest {
                    request: req,
                    response_tx: resp_tx,
                })
                .await
                .expect("send");
            let _ = tokio::time::timeout(Duration::from_secs(2), resp_rx)
                .await
                .expect("decision timely")
                .expect("oneshot delivered");
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
        let status = runtime.status();
        assert_eq!(
            status.pending_requests, 0,
            "channel should be drained after all responses received"
        );
        assert!(status.is_running);
        assert_eq!(status.total_processed, 3);

        runtime.shutdown();
    });
}

#[test]
fn graceful_shutdown_stops_tasks() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(empty_ruleset());
        assert!(runtime.is_running());

        runtime.shutdown();

        // The relay and engine tasks exit on abort; give them a short grace
        // period to flip is_running() to false.
        let mut flipped = false;
        for _ in 0..20 {
            if !runtime.is_running() {
                flipped = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            flipped,
            "OracleRuntime should report !is_running within 200ms of shutdown"
        );
    });
}

#[test]
fn status_reports_is_running_true_after_start() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(empty_ruleset());

        // Give the spawn a tick to schedule the tasks.
        tokio::time::sleep(Duration::from_millis(5)).await;

        let status = runtime.status();
        assert!(
            status.is_running,
            "oracle_runtime_status must report is_running=true after start"
        );
        assert_eq!(status.total_processed, 0);
        assert_eq!(status.pending_requests, 0);

        runtime.shutdown();
    });
}

#[test]
fn deny_by_default_when_no_rule_matches() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::start(empty_ruleset());
        let sender = runtime.sender();

        let req = make_request("process.exec");
        let (resp_tx, resp_rx) = oneshot::channel();
        sender
            .send(OracleRequest {
                request: req,
                response_tx: resp_tx,
            })
            .await
            .expect("send");

        let decision = tokio::time::timeout(Duration::from_secs(2), resp_rx)
            .await
            .expect("decision timely")
            .expect("oneshot delivered");
        assert_eq!(decision, GovernanceDecision::Denied);

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(runtime.total_processed(), 1);

        runtime.shutdown();
    });
}
