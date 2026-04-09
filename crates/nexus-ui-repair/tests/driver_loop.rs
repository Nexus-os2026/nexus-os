//! Phase 1.4 Group C integration tests — driver loop, heartbeat,
//! SpecialistCall/CostCeiling invariants.
//!
//! These tests validate the §4.1 replay determinism invariant
//! (every LLM call lands in the audit log as a SpecialistCall) and
//! the §4 cost ceiling invariant (every cost-incurring call is
//! pre-checked with `can_afford` and recorded with `record_spend`
//! which persists to disk) at the seam where the driver loop calls
//! `VisionJudge`.

use std::sync::Arc;

use async_trait::async_trait;
use nexus_ui_repair::driver::{Driver, DriverConfig, Heartbeat, PageWorkItem, VisionJudger};
use nexus_ui_repair::governance::audit::AuditLog;
use nexus_ui_repair::governance::cost_ceiling::CostCeiling;
use nexus_ui_repair::specialists::vision_judge::{
    AnthropicClient, AnthropicResponse, VisionJudge, VisionJudgeError, VisionVerdict,
};

/// Ensure $HOME is set so `Acl::default_scout` doesn't panic inside
/// the tempdir-scoped driver tests.
fn ensure_home() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tmpdir");
    std::env::set_var("HOME", tmp.path());
    // Create the default scout roots.
    let base = tmp.path().join(".nexus").join("ui-repair");
    std::fs::create_dir_all(base.join("reports")).unwrap();
    std::fs::create_dir_all(base.join("sessions")).unwrap();
    tmp
}

fn config_in(dir: &std::path::Path, dry_run: bool) -> DriverConfig {
    let base = dir.join(".nexus").join("ui-repair");
    DriverConfig {
        audit_path: base.join("sessions").join("audit.jsonl"),
        cost_ceiling_path: base.join("spend.json"),
        cost_ceiling_usd: 10.0,
        heartbeat_path: base.join("heartbeat.json"),
        heartbeat_interval_ms: 50,
        calibration_path: base.join("calibration.jsonl"),
        dry_run,
        target: nexus_ui_repair::driver::EnumerationSource::default_fixture(),
    }
}

// ---------- Deliverable 7: heartbeat lifecycle ----------

#[tokio::test]
async fn heartbeat_spawn_and_shutdown_no_leak() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("heartbeat.json");
    let hb = Heartbeat::spawn(path.clone(), 25).expect("spawn");
    hb.set_position("/test", "Enumerate");
    // Let a couple of ticks run.
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let snap = hb.snapshot();
    assert_eq!(snap.current_page, "/test");
    assert_eq!(snap.current_state, "Enumerate");
    // Shutdown must return cleanly and join the task.
    hb.shutdown().await;
    // File must exist after shutdown (we wrote at least once).
    assert!(path.exists());
}

#[tokio::test]
async fn heartbeat_immediate_shutdown_does_not_hang() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("heartbeat.json");
    let hb = Heartbeat::spawn(path.clone(), 10_000).expect("spawn");
    // Shut down before the first tick would fire.
    hb.shutdown().await;
    // Initial write happened synchronously in spawn.
    assert!(path.exists());
}

#[tokio::test]
async fn heartbeat_set_position_survives_through_ticks() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("heartbeat.json");
    let hb = Heartbeat::spawn(path.clone(), 20).expect("spawn");
    hb.set_position("/page_a", "Act");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("\"current_page\""));
    assert!(body.contains("/page_a"));
    assert!(body.contains("Act"));
    hb.shutdown().await;
}

// ---------- Deliverable 9: driver loop dry-run walks the state machine ----------

#[tokio::test]
async fn driver_dry_run_walks_state_machine_and_writes_audit() {
    let tmp = ensure_home();
    let cfg = config_in(tmp.path(), true);
    let mut driver = Driver::new(cfg.clone()).expect("driver new");
    driver.start_heartbeat().expect("heartbeat");
    let work = vec![PageWorkItem {
        page: "/builder".into(),
        elements: vec!["btn_edit".into(), "btn_delete".into()],
    }];
    let outcome = driver.run(work).await.expect("run");
    driver.shutdown_heartbeat().await;

    assert_eq!(outcome.pages_visited, 1);
    assert_eq!(outcome.elements_visited, 2);
    // Dry run: no vision calls, no classifications.
    assert_eq!(outcome.vision_calls, 0);
    assert!(outcome.classifications.is_empty());

    // Audit log must exist and contain one PageStart + 6 states per
    // element * 2 elements = 13 entries minimum.
    let audit_body = std::fs::read_to_string(&cfg.audit_path).expect("audit log");
    let lines: Vec<&str> = audit_body.lines().collect();
    assert!(
        lines.len() >= 13,
        "expected at least 13 audit lines, got {}",
        lines.len()
    );
    // Every line must be valid JSON.
    for line in &lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("audit json");
    }
    // The heartbeat file must exist.
    assert!(cfg.heartbeat_path.exists());
}

// ---------- §4 / §4.1 invariants at the VisionJudge seam ----------

struct MockAnthropic;

#[async_trait]
impl AnthropicClient for MockAnthropic {
    async fn send_vision_request(
        &self,
        _api_key: &str,
        _model: &str,
        _prompt: &str,
        _screenshot_png_base64: &str,
    ) -> Result<AnthropicResponse, VisionJudgeError> {
        Ok(AnthropicResponse {
            body_text: r#"{"verdict":"Changed","confidence":0.9,"reasoning":"mock","detected_changes":["x"]}"#
                .to_string(),
            input_tokens: 1000,
            output_tokens: 500,
        })
    }
}

#[tokio::test]
async fn vision_judge_escalation_records_specialist_call_and_spend() {
    let tmp = tempfile::tempdir().unwrap();
    let audit_path = tmp.path().join("audit.jsonl");
    let spend_path = tmp.path().join("spend.json");
    let schema_path = tmp.path().join("schema.json");
    // Fake screenshot.
    let shot_path = tmp.path().join("shot.png");
    std::fs::write(&shot_path, b"\x89PNG\r\n\x1a\nfake").unwrap();

    let audit = Arc::new(std::sync::Mutex::new(AuditLog::new(audit_path.clone())));
    let ceiling = Arc::new(std::sync::Mutex::new(
        CostCeiling::load_from_disk(spend_path.clone(), 10.0).unwrap(),
    ));

    let judge = VisionJudge::with_anthropic_client(
        std::path::PathBuf::from("/nonexistent-codex"),
        schema_path,
        Some("test-key".to_string()),
        ceiling.clone(),
        audit.clone(),
        Arc::new(MockAnthropic),
    );

    let verdict = judge
        .judge_with_anthropic_escalation(&shot_path, "Did something change?")
        .await
        .expect("escalation");
    assert!(matches!(
        verdict.verdict,
        nexus_ui_repair::specialists::vision_judge::VisionVerdictKind::Changed
    ));

    // §4.1 invariant: audit log contains a SpecialistCall entry for the
    // LLM call.
    let audit_body = std::fs::read_to_string(&audit_path).expect("audit log file");
    let lines: Vec<&str> = audit_body.lines().collect();
    assert!(!lines.is_empty(), "audit log empty");
    let mut saw_specialist_call = false;
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["state"] == "specialist_call" && v["action"] == "vision_judge.anthropic_haiku45" {
            saw_specialist_call = true;
            assert_eq!(v["specialist"], "vision_judge.anthropic_haiku45");
            // Inputs must include the provider, model, token counts.
            assert_eq!(v["inputs"]["provider"], "AnthropicApi");
            assert!(v["inputs"]["input_tokens"].as_u64().unwrap() > 0);
        }
    }
    assert!(
        saw_specialist_call,
        "no SpecialistCall entry for escalation call"
    );

    // §4 invariant: cost ceiling persistence file updated with real
    // USD spend (> 0 for the escalation path).
    assert!(spend_path.exists(), "spend.json must persist");
    let spend_body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&spend_path).unwrap()).unwrap();
    let spent = spend_body["spent_usd"].as_f64().unwrap();
    assert!(spent > 0.0, "expected non-zero spend, got {}", spent);
    // 1000 in @ $1/M + 500 out @ $5/M = $0.001 + $0.0025 = $0.0035
    assert!(
        (spent - 0.0035).abs() < 1e-9,
        "expected $0.0035, got {}",
        spent
    );
}

#[tokio::test]
async fn vision_judge_escalation_refuses_when_ceiling_exhausted() {
    let tmp = tempfile::tempdir().unwrap();
    let audit_path = tmp.path().join("audit.jsonl");
    let spend_path = tmp.path().join("spend.json");
    // Pre-seed spend right up against the ceiling so can_afford fails.
    std::fs::write(&spend_path, r#"{"spent_usd": 9.9999}"#).unwrap();
    let shot_path = tmp.path().join("shot.png");
    std::fs::write(&shot_path, b"\x89PNG\r\n\x1a\nfake").unwrap();

    let audit = Arc::new(std::sync::Mutex::new(AuditLog::new(audit_path.clone())));
    let ceiling = Arc::new(std::sync::Mutex::new(
        CostCeiling::load_from_disk(spend_path.clone(), 10.0).unwrap(),
    ));
    let judge = VisionJudge::with_anthropic_client(
        std::path::PathBuf::from("/nonexistent-codex"),
        tmp.path().join("schema.json"),
        Some("test-key".to_string()),
        ceiling.clone(),
        audit.clone(),
        Arc::new(MockAnthropic),
    );

    let err = judge
        .judge_with_anthropic_escalation(&shot_path, "?")
        .await
        .expect_err("must refuse");
    assert!(matches!(err, VisionJudgeError::CostCeilingExceeded { .. }));
    // Audit log must NOT contain a specialist_call — the refusal
    // happened before the LLM call.
    let audit_body = std::fs::read_to_string(&audit_path).unwrap_or_default();
    assert!(
        !audit_body.contains("vision_judge.anthropic_haiku45"),
        "refused call leaked into audit log"
    );
}

// ---------- Phase 1.4 C4: halt-on-fatal-error policy ----------

/// Mock `VisionJudger` that returns `CostCeilingExceeded` on every
/// call. Chosen over constructing a real `VisionJudge` with a
/// zero-ceiling because the real `judge()` path runs the Codex
/// subprocess before touching the cost ceiling, so we can't force a
/// `CostCeilingExceeded` without either a real Codex binary or a
/// trait double. The trait double is cleaner.
struct CostCeilingMockJudge;

#[async_trait]
impl VisionJudger for CostCeilingMockJudge {
    async fn judge(
        &self,
        _screenshot: &std::path::Path,
        _prompt: &str,
    ) -> Result<VisionVerdict, VisionJudgeError> {
        Err(VisionJudgeError::CostCeilingExceeded {
            ceiling: 0.01,
            attempted: 0.10,
        })
    }
}

#[tokio::test]
async fn driver_halts_cleanly_when_vision_judge_returns_cost_ceiling_exceeded() {
    let tmp = ensure_home();
    let cfg = config_in(tmp.path(), false); // NOT dry-run: we want the judge to be called.
    let mut driver = Driver::new(cfg.clone())
        .expect("driver new")
        .with_vision_judger(Arc::new(CostCeilingMockJudge));

    let work = vec![
        PageWorkItem {
            page: "/page_one".into(),
            elements: vec!["btn_a".into()],
        },
        PageWorkItem {
            page: "/page_two".into(),
            elements: vec!["btn_b".into()],
        },
        PageWorkItem {
            page: "/page_three".into(),
            elements: vec!["btn_c".into()],
        },
    ];

    let outcome = driver.run(work).await.expect("run returns Ok on halt");

    // Halt is a controlled exit, not a panic.
    assert!(outcome.halt.is_some(), "expected halt to be set");
    let halt = outcome.halt.as_ref().unwrap();
    assert_eq!(halt.error_kind, "CostCeilingExceeded");
    assert_eq!(halt.page, "/page_one");
    assert_eq!(halt.element, "btn_a");

    // Halted on first page, never touched pages 2 or 3.
    assert_eq!(outcome.pages_visited, 1);
    // The Err path does not count as a successful vision call.
    assert_eq!(outcome.vision_calls, 0);

    // Audit log must contain exactly one entry with action == "halt"
    // and payload referencing page, element, and CostCeilingExceeded.
    let audit_body = std::fs::read_to_string(&cfg.audit_path).expect("audit log");
    let lines: Vec<&str> = audit_body.lines().collect();
    let mut halt_entries = 0;
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["action"] == "halt" {
            halt_entries += 1;
            assert_eq!(v["inputs"]["page"], "/page_one");
            assert_eq!(v["inputs"]["element"], "btn_a");
            assert_eq!(v["inputs"]["error_kind"], "CostCeilingExceeded");
        }
    }
    assert_eq!(halt_entries, 1, "expected exactly one halt audit entry");
}
