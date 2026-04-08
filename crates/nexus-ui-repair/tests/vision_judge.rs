//! Phase 1.4 Deliverable 4 — vision_judge tests.
//!
//! Codex CLI calls are exercised via mock bash scripts in
//! `tests/fixtures/mock_codex*.sh`. The Anthropic escalation path is
//! exercised via a `MockAnthropicClient` injected into `VisionJudge`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nexus_ui_repair::governance::audit::AuditLog;
use nexus_ui_repair::governance::cost_ceiling::CostCeiling;
use nexus_ui_repair::specialists::vision_judge::{
    AnthropicClient, AnthropicResponse, VisionJudge, VisionJudgeError, VisionVerdictKind,
};
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn build_judge(
    codex_script: &str,
    anthropic_client: Option<Arc<dyn AnthropicClient>>,
) -> (
    VisionJudge,
    tempfile::TempDir,
    Arc<Mutex<CostCeiling>>,
    Arc<Mutex<AuditLog>>,
) {
    let dir = tempdir().expect("tempdir");
    let schema_path = dir.path().join("schema.json");
    let spend_path = dir.path().join("spend.json");
    let audit_path = dir.path().join("audit.jsonl");
    let cost = Arc::new(Mutex::new(
        CostCeiling::load_from_disk(spend_path, 10.0).expect("load"),
    ));
    let audit = Arc::new(Mutex::new(AuditLog::new(audit_path)));
    let codex_path = fixture_path(codex_script);
    let judge = if let Some(client) = anthropic_client {
        VisionJudge::with_anthropic_client(
            codex_path,
            schema_path,
            Some("test-key".into()),
            Arc::clone(&cost),
            Arc::clone(&audit),
            client,
        )
    } else {
        VisionJudge::new(
            codex_path,
            schema_path,
            Some("test-key".into()),
            Arc::clone(&cost),
            Arc::clone(&audit),
        )
    };
    (judge, dir, cost, audit)
}

fn dummy_screenshot(dir: &std::path::Path) -> PathBuf {
    let p = dir.join("shot.png");
    // Minimal PNG header bytes — vision_judge only reads them for the
    // anthropic path; for the codex path the path is just passed to
    // the mock script.
    std::fs::write(&p, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).unwrap();
    p
}

#[tokio::test]
async fn codex_path_parses_verdict_successfully() {
    let (judge, dir, _cost, _audit) = build_judge("mock_codex.sh", None);
    let shot = dummy_screenshot(dir.path());
    let verdict = judge
        .judge(&shot, "did anything change?")
        .await
        .expect("judge");
    assert_eq!(verdict.verdict, VisionVerdictKind::Changed);
    assert!((verdict.confidence - 0.92).abs() < 1e-6);
}

#[tokio::test]
async fn codex_non_zero_exit_returns_codex_exited_non_zero() {
    let (judge, dir, _c, _a) = build_judge("mock_codex_fail.sh", None);
    let shot = dummy_screenshot(dir.path());
    let err = judge.judge(&shot, "x").await.unwrap_err();
    match err {
        VisionJudgeError::CodexExitedNonZero { code, stderr } => {
            assert_eq!(code, 7);
            assert!(stderr.contains("simulated codex failure"));
        }
        other => panic!("expected CodexExitedNonZero, got {:?}", other),
    }
}

#[tokio::test]
async fn codex_missing_output_returns_output_file_missing() {
    let (judge, dir, _c, _a) = build_judge("mock_codex_no_output.sh", None);
    let shot = dummy_screenshot(dir.path());
    let err = judge.judge(&shot, "x").await.unwrap_err();
    assert!(matches!(err, VisionJudgeError::OutputFileMissing));
}

#[tokio::test]
async fn codex_garbage_output_returns_parse_failed() {
    let (judge, dir, _c, _a) = build_judge("mock_codex_garbage.sh", None);
    let shot = dummy_screenshot(dir.path());
    let err = judge.judge(&shot, "x").await.unwrap_err();
    assert!(matches!(err, VisionJudgeError::OutputParseFailed(_)));
}

#[tokio::test]
async fn codex_path_records_zero_cost_specialist_call() {
    let (judge, dir, cost, audit) = build_judge("mock_codex.sh", None);
    let shot = dummy_screenshot(dir.path());
    judge.judge(&shot, "x").await.expect("judge");
    assert_eq!(cost.lock().unwrap().spent_usd(), 0.0);
    assert!(audit.lock().unwrap().last_hash() != "0".repeat(64));
}

// ----- Anthropic escalation: mock client unit tests -----

struct MockAnthropic {
    body: String,
    input_tokens: u64,
    output_tokens: u64,
}

#[async_trait]
impl AnthropicClient for MockAnthropic {
    async fn send_vision_request(
        &self,
        _api_key: &str,
        _model: &str,
        _prompt: &str,
        _png_b64: &str,
    ) -> Result<AnthropicResponse, VisionJudgeError> {
        Ok(AnthropicResponse {
            body_text: self.body.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        })
    }
}

struct MockAnthropicError;

#[async_trait]
impl AnthropicClient for MockAnthropicError {
    async fn send_vision_request(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AnthropicResponse, VisionJudgeError> {
        Err(VisionJudgeError::AnthropicHttp("simulated 500".into()))
    }
}

#[tokio::test]
async fn anthropic_escalation_parses_verdict_and_records_real_cost() {
    let mock = Arc::new(MockAnthropic {
        body: r#"{"verdict":"Changed","confidence":0.8,"reasoning":"r","detected_changes":[]}"#
            .to_string(),
        input_tokens: 1000,
        output_tokens: 500,
    });
    let (judge, dir, cost, audit) = build_judge("mock_codex.sh", Some(mock));
    let shot = dummy_screenshot(dir.path());
    let verdict = judge
        .judge_with_anthropic_escalation(&shot, "x")
        .await
        .expect("escalate");
    assert_eq!(verdict.verdict, VisionVerdictKind::Changed);
    let spent = cost.lock().unwrap().spent_usd();
    // 1000 * 1/M + 500 * 5/M = 0.001 + 0.0025 = 0.0035
    assert!((spent - 0.0035).abs() < 1e-9, "spent={}", spent);
    assert!(audit.lock().unwrap().last_hash() != "0".repeat(64));
}

#[tokio::test]
async fn anthropic_escalation_http_error_propagates() {
    let mock = Arc::new(MockAnthropicError);
    let (judge, dir, _c, _a) = build_judge("mock_codex.sh", Some(mock));
    let shot = dummy_screenshot(dir.path());
    let err = judge
        .judge_with_anthropic_escalation(&shot, "x")
        .await
        .unwrap_err();
    assert!(matches!(err, VisionJudgeError::AnthropicHttp(_)));
}

#[tokio::test]
async fn anthropic_escalation_blocked_when_ceiling_exceeded() {
    let mock = Arc::new(MockAnthropic {
        body: r#"{"verdict":"Changed","confidence":0.8,"reasoning":"r","detected_changes":[]}"#
            .to_string(),
        input_tokens: 100,
        output_tokens: 100,
    });
    let (judge, dir, cost, _a) = build_judge("mock_codex.sh", Some(mock));
    // Burn the ceiling to within $0.001 of the limit.
    cost.lock().unwrap().record_spend(9.999).expect("burn");
    let shot = dummy_screenshot(dir.path());
    let err = judge
        .judge_with_anthropic_escalation(&shot, "x")
        .await
        .unwrap_err();
    assert!(matches!(err, VisionJudgeError::CostCeilingExceeded { .. }));
}

#[tokio::test]
#[should_panic(expected = "CodexCli missing from routing table")]
async fn routing_table_mutation_panics_loud_on_codex_path() {
    let (mut judge, dir, _c, _a) = build_judge("mock_codex.sh", None);
    // Replace the routing table with one that has no providers at all.
    judge.routing_table = nexus_ui_repair::governance::routing::RoutingTable::empty_for_test();
    let shot = dummy_screenshot(dir.path());
    let _ = judge.judge(&shot, "x").await;
}

#[tokio::test]
#[ignore = "requires real Codex CLI + ChatGPT Plus credentials; run manually"]
async fn integration_real_codex_call() {
    let dir = tempdir().expect("tempdir");
    let schema_path = dir.path().join("schema.json");
    let spend_path = dir.path().join("spend.json");
    let audit_path = dir.path().join("audit.jsonl");
    let cost = Arc::new(Mutex::new(
        CostCeiling::load_from_disk(spend_path, 10.0).expect("load"),
    ));
    let audit = Arc::new(Mutex::new(AuditLog::new(audit_path)));
    let judge = VisionJudge::new(
        PathBuf::from("/home/nexus/.npm-global/bin/codex"),
        schema_path,
        None,
        cost,
        audit,
    );
    let shot = dummy_screenshot(dir.path());
    let _ = judge.judge(&shot, "describe what you see").await.unwrap();
}
