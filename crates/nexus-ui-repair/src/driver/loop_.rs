//! Phase 1.4 Deliverable 9 — the scout driver loop.
//!
//! Integration layer. Owns the session identity, ACL, routing table,
//! audit log, cost ceiling, heartbeat, classifier, and `VisionJudge`
//! specialist, and walks the 5-state machine over a pre-enumerated
//! work queue of pages.
//!
//! # Invariants enforced in this loop (§4.1, §4)
//!
//! - **Replay determinism (I-5, §4.1):** every LLM call is routed
//!   through [`SpecialistCall`] and [`AuditLog::record_specialist_call`].
//!   The driver never calls `VisionJudge::judge*` without recording the
//!   (inputs, output) pair in the audit log — the specialist itself
//!   already records its own call, so the driver just has to ensure it
//!   is the only entry point.
//! - **Cost ceiling (§4):** every cost-incurring call goes through
//!   [`CostCeiling::can_afford`] and [`CostCeiling::record_spend`].
//!   `VisionJudge::judge` records `$0.00` (Codex CLI is free via the
//!   user's ChatGPT Plus subscription).
//!   `VisionJudge::judge_with_anthropic_escalation` pre-checks with
//!   `can_afford` and records the real USD cost from the usage block.
//!
//! # Dry run
//!
//! [`DriverConfig::dry_run`] short-circuits specialist invocation.
//! The state machine still walks end to end and emits audit entries
//! for every state transition, but no `VisionJudge` calls happen and
//! no cost is recorded. Used for:
//!
//! - smoke-testing the audit chain before burning Anthropic credit,
//! - running the driver in a fixture-only context where no real UI is
//!   attached.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::driver::heartbeat::Heartbeat;
use crate::driver::state::DriverState;
use crate::governance::acl::Acl;
use crate::governance::audit::{AuditEntry, AuditLog};
use crate::governance::calibration::CalibrationLog;
use crate::governance::cost_ceiling::CostCeiling;
use crate::governance::identity::SessionIdentity;
use crate::governance::routing::RoutingTable;
use crate::specialists::classifier::{
    Classification, Classifier, ClassifierInput, DomMutation, IpcEvent,
};
use crate::specialists::vision_judge::{VisionJudge, VisionVerdict, VisionVerdictKind};

/// Configuration for a driver run.
#[derive(Debug, Clone)]
pub struct DriverConfig {
    /// Where to write the hash-chained audit log.
    pub audit_path: PathBuf,
    /// Where `CostCeiling` persists its running total.
    pub cost_ceiling_path: PathBuf,
    /// Ceiling in USD (Anthropic escalation budget).
    pub cost_ceiling_usd: f64,
    /// Where to write the heartbeat file.
    pub heartbeat_path: PathBuf,
    /// Heartbeat tick interval in milliseconds.
    pub heartbeat_interval_ms: u64,
    /// Where the calibration log lives (written to by the classifier).
    pub calibration_path: PathBuf,
    /// Dry run — skip all LLM calls but still walk the state machine.
    pub dry_run: bool,
}

impl DriverConfig {
    /// Default configuration rooted at `~/.nexus/ui-repair/`.
    pub fn default_at_home() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let base = PathBuf::from(home).join(".nexus").join("ui-repair");
        Self {
            audit_path: base.join("sessions").join("audit.jsonl"),
            cost_ceiling_path: base.join("spend.json"),
            cost_ceiling_usd: crate::governance::cost_ceiling::DEFAULT_CEILING_USD,
            heartbeat_path: base.join("heartbeat.json"),
            heartbeat_interval_ms: 1000,
            calibration_path: base.join("calibration.jsonl"),
            dry_run: true,
        }
    }
}

/// One page of work. Phase 1.4 ships the fixture path: the driver
/// walks a fixed list of "elements" that the caller pre-enumerated.
#[derive(Debug, Clone)]
pub struct PageWorkItem {
    /// Descriptor route, e.g. `"/builder/edit-team"`.
    pub page: String,
    /// Opaque element identifiers — the driver treats them as strings
    /// and records them in the audit log.
    pub elements: Vec<String>,
}

/// Per-run outcome summary.
#[derive(Debug, Clone)]
pub struct DriverOutcome {
    pub pages_visited: usize,
    pub elements_visited: usize,
    pub classifications: Vec<Classification>,
    pub vision_calls: usize,
}

/// The scout driver.
pub struct Driver {
    identity: SessionIdentity,
    acl: Acl,
    routing: RoutingTable,
    audit: Arc<Mutex<AuditLog>>,
    cost_ceiling: Arc<Mutex<CostCeiling>>,
    classifier: Classifier,
    vision_judge: Option<Arc<VisionJudge>>,
    heartbeat: Option<Heartbeat>,
    state: DriverState,
    config: DriverConfig,
}

impl Driver {
    /// Construct a driver from `config`. Loads the cost ceiling from
    /// disk so prior spend is carried across sessions. Does NOT spawn
    /// the heartbeat yet — call [`Driver::start_heartbeat`] after
    /// entering a tokio runtime.
    pub fn new(config: DriverConfig) -> crate::Result<Self> {
        let cost_ceiling =
            CostCeiling::load_from_disk(config.cost_ceiling_path.clone(), config.cost_ceiling_usd)
                .map_err(|e| {
                    crate::Error::InvariantViolation(format!("cost ceiling load failed: {e}"))
                })?;
        let cost_ceiling = Arc::new(Mutex::new(cost_ceiling));

        let audit = Arc::new(Mutex::new(AuditLog::new(config.audit_path.clone())));
        let calibration_log = Arc::new(Mutex::new(CalibrationLog::new(
            config.calibration_path.clone(),
        )));
        let classifier = Classifier::new(calibration_log);

        Ok(Self {
            identity: SessionIdentity::new(),
            acl: Acl::default_scout(),
            routing: RoutingTable::default_v1_1(),
            audit,
            cost_ceiling,
            classifier,
            vision_judge: None,
            heartbeat: None,
            state: DriverState::Enumerate,
            config,
        })
    }

    /// Inject the vision judge. Kept separate from `new` so tests can
    /// construct a driver without the real specialist wiring.
    pub fn with_vision_judge(mut self, judge: Arc<VisionJudge>) -> Self {
        self.vision_judge = Some(judge);
        self
    }

    /// Spawn the heartbeat background task. Must be called from inside
    /// a tokio runtime.
    pub fn start_heartbeat(&mut self) -> std::io::Result<()> {
        let hb = Heartbeat::spawn(
            self.config.heartbeat_path.clone(),
            self.config.heartbeat_interval_ms,
        )?;
        self.heartbeat = Some(hb);
        Ok(())
    }

    /// Shut down the heartbeat cleanly. Safe to call even if the
    /// heartbeat was never started.
    pub async fn shutdown_heartbeat(&mut self) {
        if let Some(hb) = self.heartbeat.take() {
            hb.shutdown().await;
        }
    }

    /// Walk the state machine over the given work queue.
    ///
    /// For each page: update the heartbeat with the page name, then
    /// for each element walk Enumerate → Plan → Act → Observe →
    /// Classify → Report, updating the heartbeat on every transition,
    /// recording an audit entry on every state, and (unless
    /// `dry_run`) calling the vision judge in `Observe` and the
    /// classifier in `Classify`.
    pub async fn run(&mut self, work: Vec<PageWorkItem>) -> crate::Result<DriverOutcome> {
        let mut outcome = DriverOutcome {
            pages_visited: 0,
            elements_visited: 0,
            classifications: Vec::new(),
            vision_calls: 0,
        };

        for page in &work {
            outcome.pages_visited += 1;
            if let Some(hb) = &self.heartbeat {
                hb.set_position(&page.page, "PageStart");
            }
            self.append_audit(
                &page.page,
                "PageStart",
                "page_start",
                serde_json::json!({ "page": page.page, "elements": page.elements.len() }),
            )?;

            for element in &page.elements {
                outcome.elements_visited += 1;
                self.state = DriverState::Enumerate;
                let mut current = Some(DriverState::Enumerate);
                let mut last_verdict: Option<VisionVerdict> = None;

                while let Some(s) = current {
                    self.state = s;
                    let state_label = format!("{:?}", s);
                    if let Some(hb) = &self.heartbeat {
                        hb.set_position(&page.page, &state_label);
                    }

                    // Per-state work.
                    match s {
                        DriverState::Observe => {
                            if !self.config.dry_run {
                                if let Some(judge) = self.vision_judge.clone() {
                                    // Every LLM call goes through
                                    // SpecialistCall::record (via
                                    // VisionJudge::judge, which itself
                                    // records). The cost_ceiling check
                                    // is already inside the specialist.
                                    let screenshot_path = self
                                        .config
                                        .heartbeat_path
                                        .parent()
                                        .map(|p| p.join("last_screenshot.png"))
                                        .unwrap_or_else(|| PathBuf::from("last_screenshot.png"));
                                    match judge
                                        .judge(
                                            &screenshot_path,
                                            &format!(
                                                "Did clicking element {} on page {} change the UI?",
                                                element, page.page
                                            ),
                                        )
                                        .await
                                    {
                                        Ok(v) => {
                                            outcome.vision_calls += 1;
                                            last_verdict = Some(v);
                                        }
                                        Err(e) => {
                                            tracing::warn!(error = %e, "vision_judge failed");
                                        }
                                    }
                                }
                            }
                        }
                        DriverState::Classify => {
                            if !self.config.dry_run {
                                let verdict = last_verdict.clone().unwrap_or(VisionVerdict {
                                    verdict: VisionVerdictKind::Ambiguous,
                                    confidence: 0.0,
                                    reasoning: "no verdict".into(),
                                    detected_changes: vec![],
                                });
                                let input = ClassifierInput {
                                    vision_verdict: verdict,
                                    console_errors: vec![],
                                    ipc_traffic: vec![IpcEvent {
                                        command: "stub".into(),
                                    }],
                                    dom_mutations: vec![DomMutation {
                                        selector: "stub".into(),
                                    }],
                                    elapsed_ms: 600,
                                    signal_change_after_action: true,
                                };
                                let c = self.classifier.classify(&input);
                                outcome.classifications.push(c);
                            }
                        }
                        _ => {}
                    }

                    // Record a state-transition audit entry.
                    self.append_audit(
                        &page.page,
                        &state_label,
                        if self.config.dry_run {
                            "state_transition.dry_run"
                        } else {
                            "state_transition"
                        },
                        serde_json::json!({
                            "page": page.page,
                            "element": element,
                            "state": state_label,
                            "session": self.identity.session_id(),
                        }),
                    )?;

                    current = s.next();
                }
            }
        }

        Ok(outcome)
    }

    fn append_audit(
        &self,
        _page: &str,
        state_label: &str,
        action: &str,
        inputs: serde_json::Value,
    ) -> crate::Result<()> {
        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            state: state_label.to_string(),
            action: action.to_string(),
            specialist: None,
            inputs,
            output: serde_json::json!({}),
            prev_hash: String::new(),
            hash: String::new(),
        };
        self.audit
            .lock()
            .map_err(|_| crate::Error::InvariantViolation("audit mutex poisoned".into()))?
            .append(entry)
    }

    // --- accessors for tests and callers ---

    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
    }
    pub fn acl(&self) -> &Acl {
        &self.acl
    }
    pub fn routing(&self) -> &RoutingTable {
        &self.routing
    }
    pub fn config(&self) -> &DriverConfig {
        &self.config
    }
    pub fn audit_log(&self) -> Arc<Mutex<AuditLog>> {
        self.audit.clone()
    }
    pub fn cost_ceiling(&self) -> Arc<Mutex<CostCeiling>> {
        self.cost_ceiling.clone()
    }
    pub fn current_state(&self) -> DriverState {
        self.state
    }
}
