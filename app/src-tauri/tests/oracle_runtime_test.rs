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
use nexus_governance_engine::{
    GovernanceRule, GovernanceRuleset, RuleCondition, RuleEffect, RulesetHandle,
};
use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision, OracleRequest};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
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

    // 30 bytes of arbitrary garbage — no NXOK magic, doesn't match V0 (65)
    // or V1 (70) length. Must trip the V1 corrupt branch.
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
        OracleRuntimeError::IdentityFileCorrupt { path, detail } => {
            assert_eq!(path, identity_path);
            assert!(
                detail.contains("NXOK") || detail.contains("length"),
                "detail must surface the corruption reason; got {detail}"
            );
        }
        other => panic!("expected IdentityFileCorrupt, got {other:?}"),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Bug N — V1 file-format tests
// ───────────────────────────────────────────────────────────────────────────

const NXOK: &[u8; 4] = b"NXOK";

fn write_chmod_0600(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("write fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("chmod 0600");
    }
}

#[test]
fn identity_v1_round_trip() {
    blocking_runtime().block_on(async move {
        let tmp = TempDir::new();
        let identity_path = tmp.path().join("oracle_identity.key");

        // First start writes a fresh V1 file.
        let rt1 = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("first start");
        let vk1 = rt1.oracle().verifying_key_bytes().to_vec();
        rt1.shutdown();
        drop(rt1);

        // File must be exactly 70 bytes, magic + version 1.
        let bytes = std::fs::read(&identity_path).expect("read identity");
        assert_eq!(
            bytes.len(),
            70,
            "V1 file must be 70 bytes, got {}",
            bytes.len()
        );
        assert_eq!(&bytes[0..4], NXOK, "magic must be NXOK");
        assert_eq!(bytes[4], 1, "version must be 1");
        assert_eq!(bytes[5], 0x01, "payload[0] must be Ed25519 algo byte");

        // Second start loads the V1 file.
        let rt2 = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("second start");
        assert_eq!(rt2.oracle().verifying_key_bytes().to_vec(), vk1);
        rt2.shutdown();
    });
}

#[test]
fn identity_v0_migrates_to_v1() {
    blocking_runtime().block_on(async move {
        let tmp = TempDir::new();
        let identity_path = tmp.path().join("oracle_identity.key");

        // Step 1: hand-craft a V0 file by generating an identity once
        // through the runtime, reading the V1 file, and stripping the
        // 5-byte header. That ensures the V0 fixture's 65-byte payload
        // is a real, parseable Ed25519 keypair.
        let scratch_path = tmp.path().join("scratch.key");
        let scratch_rt = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(scratch_path.clone()),
        )
        .expect("scratch start");
        let original_vk = scratch_rt.oracle().verifying_key_bytes().to_vec();
        scratch_rt.shutdown();

        let v1_bytes = std::fs::read(&scratch_path).expect("read scratch v1");
        assert_eq!(v1_bytes.len(), 70, "scratch must be V1");
        let v0_payload = &v1_bytes[5..]; // strip magic + version
        assert_eq!(v0_payload.len(), 65);

        // Write the V0 fixture to the real test path.
        write_chmod_0600(&identity_path, v0_payload);
        assert_eq!(
            std::fs::metadata(&identity_path).unwrap().len(),
            65,
            "V0 fixture must be exactly 65 bytes"
        );

        // Step 2: start the runtime against the V0 file. Identity must
        // match (proves V0 parser worked) AND the file must now be V1
        // (proves auto-migration ran during this very startup).
        let rt = OracleRuntime::try_start_with_mode(
            empty_ruleset(),
            IdentityMode::Persistent(identity_path.clone()),
        )
        .expect("start over V0 file");
        let migrated_vk = rt.oracle().verifying_key_bytes().to_vec();
        assert_eq!(
            migrated_vk, original_vk,
            "V0 → V1 migration must preserve the keypair byte-for-byte"
        );
        rt.shutdown();

        let post = std::fs::read(&identity_path).expect("re-read");
        assert_eq!(
            post.len(),
            70,
            "after migration the file must be V1 (70 bytes)"
        );
        assert_eq!(&post[0..4], NXOK);
        assert_eq!(post[4], 1);
        assert_eq!(
            &post[5..],
            v0_payload,
            "V1 payload must equal the original V0 bytes"
        );
    });
}

#[test]
fn identity_future_version_errors() {
    let tmp = TempDir::new();
    let identity_path = tmp.path().join("oracle_identity.key");

    // V1 magic + version 99 + 65 bytes of "real-shaped" payload (algo
    // byte + 64 zero bytes). Length is right; only the version is in the
    // future. Must refuse without regenerating.
    let mut fixture = Vec::with_capacity(70);
    fixture.extend_from_slice(NXOK);
    fixture.push(99);
    fixture.push(0x01); // valid algo marker — irrelevant; version check rejects first
    fixture.extend_from_slice(&[0u8; 64]);
    write_chmod_0600(&identity_path, &fixture);

    let result = OracleRuntime::try_start_with_mode(
        empty_ruleset(),
        IdentityMode::Persistent(identity_path.clone()),
    );
    let err = match result {
        Ok(_) => panic!("future version must refuse, got Ok"),
        Err(e) => e,
    };
    match err {
        OracleRuntimeError::IdentityFileFutureVersion { path, version } => {
            assert_eq!(path, identity_path);
            assert_eq!(version, 99);
        }
        other => panic!("expected IdentityFileFutureVersion, got {other:?}"),
    }
    // File must NOT have been silently regenerated — it should still be
    // the future-versioned fixture we wrote.
    let post = std::fs::read(&identity_path).expect("re-read");
    assert_eq!(post, fixture, "fixture must be untouched after refusal");
}

#[test]
fn identity_truncated_v1_errors() {
    let tmp = TempDir::new();
    let identity_path = tmp.path().join("oracle_identity.key");

    // V1 magic + version + only 50 of the 65 payload bytes. Must trip the
    // V1 corrupt-length branch.
    let mut fixture = Vec::with_capacity(55);
    fixture.extend_from_slice(NXOK);
    fixture.push(1);
    fixture.extend_from_slice(&[0u8; 50]);
    write_chmod_0600(&identity_path, &fixture);

    let result = OracleRuntime::try_start_with_mode(
        empty_ruleset(),
        IdentityMode::Persistent(identity_path.clone()),
    );
    let err = match result {
        Ok(_) => panic!("truncated V1 must refuse, got Ok"),
        Err(e) => e,
    };
    match err {
        OracleRuntimeError::IdentityFileCorrupt { path, detail } => {
            assert_eq!(path, identity_path);
            assert!(detail.contains("V1") && detail.contains("length"));
        }
        other => panic!("expected IdentityFileCorrupt, got {other:?}"),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Bug M — ruleset hot-swap propagation tests
// ───────────────────────────────────────────────────────────────────────────

fn ruleset_allow(caps: &[&str]) -> GovernanceRuleset {
    GovernanceRuleset::new(
        format!("test-{}", uuid::Uuid::new_v4()),
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

async fn submit_and_wait(runtime: &Arc<OracleRuntime>, capability: &str) -> GovernanceDecision {
    let (tx, rx) = oneshot::channel();
    runtime
        .sender()
        .send(OracleRequest {
            request: make_request(capability),
            response_tx: tx,
        })
        .await
        .expect("send");
    tokio::time::timeout(Duration::from_secs(2), rx)
        .await
        .expect("decision timely")
        .expect("oneshot delivered")
}

#[test]
fn ruleset_hotswap_propagates_to_engine() {
    blocking_runtime().block_on(async move {
        let handle: RulesetHandle = Arc::new(RwLock::new(ruleset_allow(&["llm.query"])));
        let runtime =
            OracleRuntime::try_start_with_handle_and_mode(handle.clone(), IdentityMode::Ephemeral)
                .expect("start");

        // Pre-swap: llm.query allowed.
        let pre = submit_and_wait(&runtime, "llm.query").await;
        assert!(
            matches!(pre, GovernanceDecision::Approved { .. }),
            "pre-swap llm.query should be Approved; got {pre:?}"
        );

        // Hot-swap: replace ruleset with one that denies-by-default
        // (empty rules → fall through to default deny).
        {
            let mut guard = handle.write().expect("write-lock");
            *guard = empty_ruleset();
        }

        // Post-swap: same request now denied. Engine must read from the
        // shared handle on this request, not from a stale clone.
        let post = submit_and_wait(&runtime, "llm.query").await;
        assert_eq!(
            post,
            GovernanceDecision::Denied,
            "post-swap llm.query should be Denied (empty ruleset → deny by default); got {post:?}"
        );

        // Sibling Arc: runtime's handle is the same Arc we wrote through.
        assert!(
            Arc::ptr_eq(&handle, &runtime.governance_ruleset_handle()),
            "governance_ruleset_handle() must return the same Arc the engine reads from"
        );

        runtime.shutdown();
    });
}

#[test]
fn ruleset_hotswap_under_concurrent_load() {
    blocking_runtime().block_on(async move {
        let handle: RulesetHandle = Arc::new(RwLock::new(ruleset_allow(&["llm.query"])));
        let runtime =
            OracleRuntime::try_start_with_handle_and_mode(handle.clone(), IdentityMode::Ephemeral)
                .expect("start");

        // Spawn 50 concurrent submitters racing the swap. Half will land
        // pre-swap (Approved), half post-swap (Denied) — the exact split
        // is a runtime race, but every submission must complete with
        // SOME decision (no deadlock, no panic).
        let mut handles = Vec::with_capacity(50);
        for i in 0..50 {
            let runtime = Arc::clone(&runtime);
            handles.push(tokio::spawn(async move {
                // Stagger half the submissions so they straddle the swap.
                if i >= 25 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                submit_and_wait(&runtime, "llm.query").await
            }));
        }

        // Mid-flight swap.
        tokio::time::sleep(Duration::from_millis(2)).await;
        {
            let mut guard = handle.write().expect("write-lock");
            *guard = empty_ruleset();
        }

        let mut decisions = Vec::with_capacity(50);
        for h in handles {
            decisions.push(h.await.expect("task did not panic"));
        }
        assert_eq!(decisions.len(), 50, "all 50 submissions must complete");

        // The post-swap engine state must always deny llm.query — submit
        // one more after the dust settles to confirm the swap is sticky.
        let final_decision = submit_and_wait(&runtime, "llm.query").await;
        assert_eq!(
            final_decision,
            GovernanceDecision::Denied,
            "after swap and load drain, engine must consistently deny"
        );

        runtime.shutdown();
    });
}

#[test]
fn ruleset_handle_is_shared_arc_with_engine() {
    // Structural invariant Bug M depends on: the handle the runtime
    // exposes via `governance_ruleset_handle()` is the same Arc instance
    // the DecisionEngine task is reading from. If this ever drifts (e.g.
    // someone wraps the handle in another Arc layer), the propagation
    // tests above stop reflecting reality.
    blocking_runtime().block_on(async move {
        let handle: RulesetHandle = Arc::new(RwLock::new(empty_ruleset()));
        let runtime =
            OracleRuntime::try_start_with_handle_and_mode(handle.clone(), IdentityMode::Ephemeral)
                .expect("start");
        assert!(
            Arc::ptr_eq(&handle, &runtime.governance_ruleset_handle()),
            "runtime must hold the SAME Arc the caller passed in — no wrap layer"
        );
        // Strong count: runtime + caller's clone = 2 minimum. Spawned
        // engine task is moved into tokio so adds 1; 3 is the expected
        // minimum without the test holding extras.
        assert!(
            Arc::strong_count(&handle) >= 2,
            "runtime + test must both own a strong ref to the handle; count={}",
            Arc::strong_count(&handle)
        );
        runtime.shutdown();
    });
}
