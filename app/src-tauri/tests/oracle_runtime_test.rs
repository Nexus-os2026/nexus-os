//! Integration tests for the Phase 1.5a/1.5a.1 GovernanceOracle runtime
//! wiring.
//!
//! These tests exercise `OracleRuntime` directly rather than spinning up a
//! full Tauri app fixture — the runtime is the thing under test and a
//! Tauri fixture costs ~1s per test for zero additional coverage.
//!
//! Phase 1.5a.1: the existing 6 tests were updated to use
//! `try_start_with_mode(..., IdentityMode::Ephemeral)` so they do not read
//! or write `$HOME/.nexus/oracle_identity.key` during test runs. The new
//! identity-persistence tests supply an explicit temporary path.

use nexus_desktop_backend::oracle_runtime::{IdentityMode, OracleRuntime, OracleRuntimeError};
use nexus_governance_engine::{GovernanceRule, GovernanceRuleset, RuleCondition, RuleEffect};
use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision, OracleRequest};
use std::path::{Path, PathBuf};
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
        .enable_all()
        .build()
        .expect("tokio runtime for tests")
}

/// Start an OracleRuntime with an ephemeral identity — no disk I/O.
/// Used by all the pre-1.5a.1 tests that don't care about identity
/// persistence, only about engine behavior.
fn start_ephemeral(ruleset: GovernanceRuleset) -> std::sync::Arc<OracleRuntime> {
    OracleRuntime::try_start_with_mode(ruleset, IdentityMode::Ephemeral)
        .expect("ephemeral start must succeed")
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nexus_oracle_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create tempdir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 1.5a behavior tests (rewritten to use Ephemeral mode)
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn three_requests_all_receive_responses() {
    blocking_runtime().block_on(async move {
        let runtime = start_ephemeral(ruleset_allowing(&["llm.query"]));
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
        let runtime = start_ephemeral(ruleset_allowing(&["llm.query"]));
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

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(runtime.total_processed(), 5);

        runtime.shutdown();
    });
}

#[test]
fn pending_metric_reports_zero_when_drained() {
    blocking_runtime().block_on(async move {
        let runtime = start_ephemeral(ruleset_allowing(&["llm.query"]));
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
        let runtime = start_ephemeral(empty_ruleset());
        assert!(runtime.is_running());

        runtime.shutdown();

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
        let runtime = start_ephemeral(empty_ruleset());

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
        let runtime = start_ephemeral(empty_ruleset());
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

// ───────────────────────────────────────────────────────────────────────────
// 1.5a.1 identity-persistence tests
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn oracle_identity_persists_across_restart() {
    blocking_runtime().block_on(async move {
        let tmp = TempDir::new();
        let identity_path = tmp.path().join("oracle_identity.key");

        let rt1 = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("first start");
        let vk1 = rt1.oracle().verifying_key_bytes().to_vec();
        rt1.shutdown();
        drop(rt1);

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            identity_path.exists(),
            "key file must exist after first start"
        );

        let rt2 = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("second start");
        let vk2 = rt2.oracle().verifying_key_bytes().to_vec();

        assert_eq!(
            vk1, vk2,
            "oracle verifying key must persist byte-for-byte across restarts"
        );
        rt2.shutdown();
    });
}

#[test]
fn oracle_identity_generated_fresh_when_file_absent() {
    blocking_runtime().block_on(async move {
        let tmp = TempDir::new();
        let identity_path = tmp.path().join("nested").join("oracle_identity.key");
        assert!(!identity_path.exists());

        let rt = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("start");

        assert!(
            identity_path.exists(),
            "key file should be created on first start"
        );
        assert!(!rt.oracle().verifying_key_bytes().is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&identity_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "identity file must be 0o600; got 0o{mode:o}");
            let parent_mode = std::fs::metadata(identity_path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                parent_mode, 0o700,
                "identity file parent dir must be 0o700; got 0o{parent_mode:o}"
            );
        }

        rt.shutdown();
    });
}

#[test]
fn oracle_seal_verify_roundtrip() {
    blocking_runtime().block_on(async move {
        let runtime = OracleRuntime::try_start_with_mode(
            ruleset_allowing(&["llm.query"]),
            IdentityMode::Ephemeral,
        )
        .expect("start");
        let oracle = runtime.oracle();

        let request = make_request("llm.query");
        let token = tokio::time::timeout(Duration::from_secs(2), oracle.submit_request(request))
            .await
            .expect("submit_request timely")
            .expect("submit_request ok");

        let payload = oracle.verify_token(&token).expect("verify_token ok");
        assert!(
            matches!(payload.decision, GovernanceDecision::Approved { .. }),
            "expected Approved decision, got {:?}",
            payload.decision
        );

        runtime.shutdown();
    });
}

#[test]
fn start_with_ephemeral_mode_succeeds_without_disk() {
    blocking_runtime().block_on(async move {
        let rt = OracleRuntime::try_start_with_mode(empty_ruleset(), IdentityMode::Ephemeral)
            .expect("ephemeral start");

        assert!(rt.is_running());
        assert!(!rt.oracle().verifying_key_bytes().is_empty());

        rt.shutdown();
    });
}

#[test]
fn identity_mode_from_env_honors_ephemeral_flag() {
    // SAFETY: this test mutates a process-global env var. No other test in
    // this file reads or writes NEXUS_ORACLE_EPHEMERAL — only this one does,
    // so there is no cross-test race. If a future test needs the same flag,
    // group them behind a mutex or serial-test macro.
    std::env::set_var("NEXUS_ORACLE_EPHEMERAL", "1");
    let mode = IdentityMode::from_env().expect("from_env");
    std::env::remove_var("NEXUS_ORACLE_EPHEMERAL");
    assert!(
        matches!(mode, IdentityMode::Ephemeral),
        "NEXUS_ORACLE_EPHEMERAL=1 must resolve to IdentityMode::Ephemeral"
    );
}

#[test]
fn corrupt_identity_file_errors() {
    let tmp = TempDir::new();
    let identity_path = tmp.path().join("oracle_identity.key");

    std::fs::write(&identity_path, b"this-is-not-an-ed25519-keypair").expect("write garbage");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity_path, std::fs::Permissions::from_mode(0o600))
            .expect("chmod garbage file");
    }

    let result = OracleRuntime::try_start_with_mode(
        empty_ruleset(),
        IdentityMode::Persistent(identity_path.clone()),
    );
    let err = match result {
        Ok(_) => panic!("corrupt file must produce typed error, got Ok"),
        Err(e) => e,
    };
    match err {
        OracleRuntimeError::IdentityFormat { path, .. } => {
            assert_eq!(path, identity_path);
        }
        other => panic!("expected IdentityFormat, got {other:?}"),
    }
}
