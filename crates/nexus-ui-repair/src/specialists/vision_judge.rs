//! Phase 1.4 Deliverable 4 — real `vision_judge` over Codex CLI and
//! Anthropic Haiku 4.5.
//!
//! Two paths:
//!
//! 1. [`VisionJudge::judge`] — default path. Spawns Codex CLI as a
//!    subprocess (free via the user's ChatGPT Plus account) with the
//!    schema enforced via `--output-schema`. Records the call to the
//!    audit log; records $0.00 spend so the cost ceiling still has a
//!    full audit trail.
//!
//! 2. [`VisionJudge::judge_with_anthropic_escalation`] — called only
//!    when the classifier returns `Ambiguous`. Routes to Anthropic
//!    Haiku 4.5 via HTTPS, capped by the running cost ceiling. Real
//!    USD spend is recorded from the API response usage block.
//!
//! Both paths route through the v1.1 §4 routing table and assert at
//! call time that the provider they intend to use is in the allow
//! list. A mutated routing table panics loud — defense in depth.
//!
//! HTTP is abstracted behind the [`AnthropicClient`] trait so unit
//! tests can plug in a mock without spinning up a fake server.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::governance::audit::AuditLog;
use crate::governance::cost_ceiling::{CostCeiling, CostCeilingError};
use crate::governance::routing::{Provider, RoutingTable, ANTHROPIC_MODEL_HAIKU_4_5};
use crate::specialists::specialist_call::SpecialistCall;
use crate::specialists::vision_schema::{
    write_schema_to_disk, SCHEMA_VERSION, VISION_VERDICT_SCHEMA,
};

/// The four verdict labels Codex / Anthropic must return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionVerdictKind {
    Changed,
    Unchanged,
    Error,
    Ambiguous,
}

/// The full vision verdict, matching [`VISION_VERDICT_SCHEMA`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionVerdict {
    pub verdict: VisionVerdictKind,
    pub confidence: f64,
    pub reasoning: String,
    pub detected_changes: Vec<String>,
}

/// Default path to the Codex CLI binary.
pub const DEFAULT_CODEX_PATH: &str = "/home/nexus/.npm-global/bin/codex";

/// Anthropic API endpoint for the messages API.
pub const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Cost-per-token for Haiku 4.5 (USD per token). $1/M input, $5/M output.
pub const HAIKU_INPUT_USD_PER_TOKEN: f64 = 1.0 / 1_000_000.0;
pub const HAIKU_OUTPUT_USD_PER_TOKEN: f64 = 5.0 / 1_000_000.0;

/// Estimate of an escalation call's cost, used for the pre-call
/// `can_afford` check before we know the real usage figures. 2000
/// input tokens (image + prompt) and 1000 output tokens.
pub const HAIKU_ESCALATION_ESTIMATE_USD: f64 =
    2000.0 * HAIKU_INPUT_USD_PER_TOKEN + 1000.0 * HAIKU_OUTPUT_USD_PER_TOKEN;

/// HTTP layer for the Anthropic call. Production uses
/// [`ReqwestAnthropicClient`]; tests pass a mock.
#[async_trait]
pub trait AnthropicClient: Send + Sync {
    async fn send_vision_request(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        screenshot_png_base64: &str,
    ) -> Result<AnthropicResponse, VisionJudgeError>;
}

/// Subset of the Anthropic messages API response that vision_judge
/// cares about: the JSON body the model produced and the usage
/// figures used for cost recording.
#[derive(Debug, Clone)]
pub struct AnthropicResponse {
    /// The text content of the first content block, expected to be a
    /// JSON-encoded [`VisionVerdict`].
    pub body_text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Production HTTP client for the Anthropic messages API.
pub struct ReqwestAnthropicClient {
    client: reqwest::Client,
}

impl Default for ReqwestAnthropicClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestAnthropicClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AnthropicClient for ReqwestAnthropicClient {
    async fn send_vision_request(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        screenshot_png_base64: &str,
    ) -> Result<AnthropicResponse, VisionJudgeError> {
        let body = serde_json::json!({
            "model": model,
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": screenshot_png_base64
                        }
                    },
                    { "type": "text", "text": prompt }
                ]
            }]
        });
        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VisionJudgeError::AnthropicHttp(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(VisionJudgeError::AnthropicHttp(format!(
                "status {}: {}",
                status, text
            )));
        }
        let parsed: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| VisionJudgeError::AnthropicHttp(e.to_string()))?;
        let body_text = parsed["content"][0]["text"]
            .as_str()
            .ok_or_else(|| VisionJudgeError::AnthropicHttp("missing content[0].text".into()))?
            .to_string();
        let input_tokens = parsed["usage"]["input_tokens"].as_u64().unwrap_or(0);
        let output_tokens = parsed["usage"]["output_tokens"].as_u64().unwrap_or(0);
        Ok(AnthropicResponse {
            body_text,
            input_tokens,
            output_tokens,
        })
    }
}

/// The vision judge specialist.
pub struct VisionJudge {
    pub codex_path: PathBuf,
    pub schema_path: PathBuf,
    pub anthropic_api_key: Option<String>,
    pub cost_ceiling: Arc<Mutex<CostCeiling>>,
    pub audit_log: Arc<Mutex<AuditLog>>,
    pub routing_table: RoutingTable,
    pub anthropic_client: Arc<dyn AnthropicClient>,
}

impl VisionJudge {
    /// Construct a `VisionJudge` with the production Anthropic client.
    pub fn new(
        codex_path: PathBuf,
        schema_path: PathBuf,
        anthropic_api_key: Option<String>,
        cost_ceiling: Arc<Mutex<CostCeiling>>,
        audit_log: Arc<Mutex<AuditLog>>,
    ) -> Self {
        Self {
            codex_path,
            schema_path,
            anthropic_api_key,
            cost_ceiling,
            audit_log,
            routing_table: RoutingTable::default_v1_1(),
            anthropic_client: Arc::new(ReqwestAnthropicClient::new()),
        }
    }

    /// Construct with an injected Anthropic client (for tests).
    pub fn with_anthropic_client(
        codex_path: PathBuf,
        schema_path: PathBuf,
        anthropic_api_key: Option<String>,
        cost_ceiling: Arc<Mutex<CostCeiling>>,
        audit_log: Arc<Mutex<AuditLog>>,
        anthropic_client: Arc<dyn AnthropicClient>,
    ) -> Self {
        Self {
            codex_path,
            schema_path,
            anthropic_api_key,
            cost_ceiling,
            audit_log,
            routing_table: RoutingTable::default_v1_1(),
            anthropic_client,
        }
    }

    /// Default path: spawn Codex CLI against the screenshot.
    pub async fn judge(
        &self,
        screenshot: &Path,
        prompt: &str,
    ) -> Result<VisionVerdict, VisionJudgeError> {
        // Defense in depth: assert CodexCli is permitted right now.
        assert!(
            self.routing_table
                .allowed()
                .iter()
                .any(|p| matches!(p, Provider::CodexCli)),
            "CodexCli missing from routing table — refusing to call"
        );

        // Ensure the schema file exists on disk for codex --output-schema.
        write_schema_to_disk(&self.schema_path)?;

        // Output sink for codex --output-last-message.
        let tmp = tempfile::tempdir()?;
        let output_path = tmp.path().join("codex_output.txt");

        let status_and_output = tokio::process::Command::new(&self.codex_path)
            .arg("exec")
            .arg("--json")
            .arg("--image")
            .arg(screenshot)
            .arg("--output-schema")
            .arg(&self.schema_path)
            .arg("--output-last-message")
            .arg(&output_path)
            .arg(prompt)
            .output()
            .await
            .map_err(|e| VisionJudgeError::CodexSpawnFailed(e.to_string()))?;

        if !status_and_output.status.success() {
            return Err(VisionJudgeError::CodexExitedNonZero {
                code: status_and_output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&status_and_output.stderr).into_owned(),
            });
        }

        if !output_path.exists() {
            return Err(VisionJudgeError::OutputFileMissing);
        }
        let output_bytes = std::fs::read(&output_path)?;
        let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
        let verdict: VisionVerdict = serde_json::from_str(output_str.trim())
            .map_err(|e| VisionJudgeError::OutputParseFailed(e.to_string()))?;

        // Record the call as a SpecialistCall in the audit log.
        let call = SpecialistCall::new(
            "vision_judge.codex",
            serde_json::json!({
                "schema_version": SCHEMA_VERSION,
                "prompt": prompt,
                "screenshot": screenshot.display().to_string(),
                "provider": "CodexCli"
            }),
            serde_json::to_value(&verdict).unwrap_or(serde_json::Value::Null),
        );
        self.audit_log
            .lock()
            .map_err(|_| VisionJudgeError::AuditLogFailed("mutex poisoned".into()))?
            .record_specialist_call(call)
            .map_err(|e| VisionJudgeError::AuditLogFailed(e.to_string()))?;

        // Codex calls are free, but record $0.00 so the audit trail
        // shows zero-cost calls explicitly.
        self.cost_ceiling
            .lock()
            .map_err(|_| VisionJudgeError::AuditLogFailed("cost ceiling mutex poisoned".into()))?
            .record_spend(0.0)
            .map_err(|e| match e {
                CostCeilingError::CeilingExceeded { ceiling, attempted } => {
                    VisionJudgeError::CostCeilingExceeded { ceiling, attempted }
                }
                CostCeilingError::PersistenceFailure { source } => {
                    VisionJudgeError::AuditLogFailed(source.to_string())
                }
            })?;

        Ok(verdict)
    }

    /// Escalation path: route through Anthropic Haiku 4.5 with cost
    /// ceiling enforcement. Called only when the classifier returns
    /// `Ambiguous`.
    pub async fn judge_with_anthropic_escalation(
        &self,
        screenshot: &Path,
        prompt: &str,
    ) -> Result<VisionVerdict, VisionJudgeError> {
        // Defense in depth: assert AnthropicApi haiku-4.5 is permitted.
        assert!(
            self.routing_table.allowed().iter().any(|p| matches!(
                p,
                Provider::AnthropicApi { model } if model == ANTHROPIC_MODEL_HAIKU_4_5
            )),
            "AnthropicApi haiku-4.5 missing from routing table — refusing to call"
        );

        let api_key = self
            .anthropic_api_key
            .as_ref()
            .ok_or(VisionJudgeError::MissingAnthropicApiKey)?
            .clone();

        // Pre-call ceiling check using the per-call estimate.
        {
            let ceiling = self.cost_ceiling.lock().map_err(|_| {
                VisionJudgeError::AuditLogFailed("cost ceiling mutex poisoned".into())
            })?;
            if !ceiling.can_afford(HAIKU_ESCALATION_ESTIMATE_USD) {
                return Err(VisionJudgeError::CostCeilingExceeded {
                    ceiling: ceiling.ceiling_usd(),
                    attempted: ceiling.spent_usd() + HAIKU_ESCALATION_ESTIMATE_USD,
                });
            }
        }

        // Read screenshot and base64-encode for the API.
        let png_bytes = std::fs::read(screenshot)?;
        let png_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

        // Embed the schema in the prompt so the model is told what to
        // emit. The schema is a constant; the user prompt is appended.
        let combined_prompt = format!(
            "You are a UI vision judge. Reply with ONLY a JSON object \
             matching this schema and nothing else:\n\n{}\n\nQuestion: {}",
            VISION_VERDICT_SCHEMA, prompt
        );

        let response = self
            .anthropic_client
            .send_vision_request(
                &api_key,
                ANTHROPIC_MODEL_HAIKU_4_5,
                &combined_prompt,
                &png_base64,
            )
            .await?;

        let verdict: VisionVerdict = serde_json::from_str(response.body_text.trim())
            .map_err(|e| VisionJudgeError::OutputParseFailed(e.to_string()))?;

        // Compute real spend from usage.
        let real_cost = response.input_tokens as f64 * HAIKU_INPUT_USD_PER_TOKEN
            + response.output_tokens as f64 * HAIKU_OUTPUT_USD_PER_TOKEN;

        // Record cost (this also persists).
        self.cost_ceiling
            .lock()
            .map_err(|_| VisionJudgeError::AuditLogFailed("cost ceiling mutex poisoned".into()))?
            .record_spend(real_cost)
            .map_err(|e| match e {
                CostCeilingError::CeilingExceeded { ceiling, attempted } => {
                    VisionJudgeError::CostCeilingExceeded { ceiling, attempted }
                }
                CostCeilingError::PersistenceFailure { source } => {
                    VisionJudgeError::AuditLogFailed(source.to_string())
                }
            })?;

        // Record the call to the audit log.
        let call = SpecialistCall::new(
            "vision_judge.anthropic_haiku45",
            serde_json::json!({
                "schema_version": SCHEMA_VERSION,
                "prompt": prompt,
                "screenshot": screenshot.display().to_string(),
                "provider": "AnthropicApi",
                "model": ANTHROPIC_MODEL_HAIKU_4_5,
                "input_tokens": response.input_tokens,
                "output_tokens": response.output_tokens,
                "cost_usd": real_cost
            }),
            serde_json::to_value(&verdict).unwrap_or(serde_json::Value::Null),
        );
        self.audit_log
            .lock()
            .map_err(|_| VisionJudgeError::AuditLogFailed("audit log mutex poisoned".into()))?
            .record_specialist_call(call)
            .map_err(|e| VisionJudgeError::AuditLogFailed(e.to_string()))?;

        Ok(verdict)
    }
}

/// Errors raised by [`VisionJudge`].
#[derive(Debug, thiserror::Error)]
pub enum VisionJudgeError {
    #[error("codex spawn failed: {0}")]
    CodexSpawnFailed(String),
    #[error("codex exited non-zero (code={code}): {stderr}")]
    CodexExitedNonZero { code: i32, stderr: String },
    #[error("codex output file missing")]
    OutputFileMissing,
    #[error("codex output parse failed: {0}")]
    OutputParseFailed(String),
    #[error("audit log write failed: {0}")]
    AuditLogFailed(String),
    #[error("anthropic http error: {0}")]
    AnthropicHttp(String),
    #[error("anthropic api key missing")]
    MissingAnthropicApiKey,
    #[error("cost ceiling exceeded: ceiling=${ceiling:.4} attempted=${attempted:.4}")]
    CostCeilingExceeded { ceiling: f64, attempted: f64 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("crate error: {0}")]
    Crate(#[from] crate::Error),
}
