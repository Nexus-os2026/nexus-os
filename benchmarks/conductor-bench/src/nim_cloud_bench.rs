#![allow(
    dead_code,
    clippy::single_char_add_str,
    clippy::manual_is_multiple_of,
    clippy::needless_borrow,
    clippy::unnecessary_map_or,
    clippy::too_many_arguments,
    clippy::doc_overindented_list_items,
    clippy::unnecessary_unwrap
)]
//! Nexus OS — NVIDIA NIM Cloud Models Stress Test
//!
//! Tests all available NVIDIA NIM models with rate limiting to respect
//! free-tier constraints (40 model tests per minute).
//!
//! Tests per model:
//!   Phase 1: Probe — single query to verify model is live
//!   Phase 2: Determinism — 10 identical prompts, temp=0, seed=42
//!   Phase 3: Concurrency — 50 agents hitting model simultaneously
//!   Phase 4: Agentic — 5 decision-making tasks
//!
//! Run:
//!   GROQ_API_KEY=nvapi-xxx \
//!     cargo run -p nexus-conductor-benchmark --bin nim-cloud-bench --release
//!
//! Options:
//!   NIM_RATE_LIMIT=40       Models tested per minute (default 40)
//!   NIM_DETERMINISM_RUNS=10 Runs per determinism prompt (default 10)
//!   NIM_CONCURRENCY=50      Concurrent agents for latency test (default 50)
//!   NIM_SKIP_EMBED=1        Skip embedding models (default: skip)
//!   NIM_MODELS=5            Limit to first N models (for quick testing)

use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Configuration (overridable via env) ────────────────────────────────────

fn cfg_rate_limit() -> usize {
    std::env::var("NIM_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

fn cfg_determinism_runs() -> usize {
    std::env::var("NIM_DETERMINISM_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

fn cfg_concurrency() -> usize {
    std::env::var("NIM_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
}

fn cfg_model_limit() -> Option<usize> {
    std::env::var("NIM_MODELS")
        .ok()
        .and_then(|v| v.parse().ok())
}

const MAX_TOKENS: u32 = 64;
const ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

// ── Full NVIDIA NIM Model Catalog (93 models) ─────────────────────────────

const NIM_MODELS: &[(&str, &str, &str)] = &[
    // (model_id, display_name, family)
    // DeepSeek
    (
        "deepseek-ai/deepseek-v3_1-terminus",
        "DeepSeek V3.1 Terminus 671B",
        "deepseek",
    ),
    ("deepseek-ai/deepseek-v3_1", "DeepSeek V3.1", "deepseek"),
    ("deepseek-ai/deepseek-v3", "DeepSeek V3", "deepseek"),
    (
        "deepseek-ai/deepseek-r1",
        "DeepSeek R1 Reasoning",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-llama-70b",
        "DeepSeek R1 Distill 70B",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-qwen-32b",
        "DeepSeek R1 Distill Qwen 32B",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-qwen-14b",
        "DeepSeek R1 Distill Qwen 14B",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-r1-distill-llama-8b",
        "DeepSeek R1 Distill 8B",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-coder-v2-instruct",
        "DeepSeek Coder V2 236B",
        "deepseek",
    ),
    (
        "deepseek-ai/deepseek-coder-v2-lite-instruct",
        "DeepSeek Coder V2 Lite 16B",
        "deepseek",
    ),
    // Meta Llama
    (
        "meta/llama-4-scout-17b-16e-instruct",
        "Llama 4 Scout 17B",
        "meta",
    ),
    (
        "meta/llama-4-maverick-17b-128e-instruct",
        "Llama 4 Maverick 17B",
        "meta",
    ),
    ("meta/llama-3.3-70b-instruct", "Llama 3.3 70B", "meta"),
    ("meta/llama-3.1-405b-instruct", "Llama 3.1 405B", "meta"),
    ("meta/llama-3.1-70b-instruct", "Llama 3.1 70B", "meta"),
    ("meta/llama-3.1-8b-instruct", "Llama 3.1 8B", "meta"),
    (
        "meta/llama-3.2-90b-vision-instruct",
        "Llama 3.2 90B Vision",
        "meta",
    ),
    (
        "meta/llama-3.2-11b-vision-instruct",
        "Llama 3.2 11B Vision",
        "meta",
    ),
    ("meta/llama-3.2-3b-instruct", "Llama 3.2 3B", "meta"),
    ("meta/llama-3.2-1b-instruct", "Llama 3.2 1B", "meta"),
    ("meta/codellama-70b-instruct", "CodeLlama 70B", "meta"),
    ("meta/llama-guard-3-8b", "Llama Guard 3 8B", "meta"),
    // NVIDIA Nemotron
    (
        "nvidia/llama-3.1-nemotron-ultra-253b-v1",
        "Nemotron Ultra 253B",
        "nvidia",
    ),
    (
        "nvidia/llama-3.1-nemotron-70b-instruct",
        "Nemotron 70B",
        "nvidia",
    ),
    (
        "nvidia/nemotron-4-340b-instruct",
        "Nemotron 4 340B",
        "nvidia",
    ),
    (
        "nvidia/nemotron-3-super-120b-a12b",
        "Nemotron 3 Super 120B",
        "nvidia",
    ),
    (
        "nvidia/nemotron-3-nano-30b-a3b",
        "Nemotron Nano 30B",
        "nvidia",
    ),
    (
        "nvidia/nemotron-mini-4b-instruct",
        "Nemotron Mini 4B",
        "nvidia",
    ),
    (
        "nvidia/llama-3.1-nemotron-51b-instruct",
        "Nemotron 51B",
        "nvidia",
    ),
    (
        "nvidia/usdcode-llama3.1-70b-instruct",
        "USDCode 70B",
        "nvidia",
    ),
    // Qwen
    ("qwen/qwen3.5-vl-400b", "Qwen 3.5 VL 400B", "qwen"),
    ("qwen/qwen2.5-72b-instruct", "Qwen 2.5 72B", "qwen"),
    ("qwen/qwen2.5-32b-instruct", "Qwen 2.5 32B", "qwen"),
    ("qwen/qwen2.5-14b-instruct", "Qwen 2.5 14B", "qwen"),
    ("qwen/qwen2.5-7b-instruct", "Qwen 2.5 7B", "qwen"),
    (
        "qwen/qwen2.5-coder-32b-instruct",
        "Qwen 2.5 Coder 32B",
        "qwen",
    ),
    (
        "qwen/qwen2.5-coder-7b-instruct",
        "Qwen 2.5 Coder 7B",
        "qwen",
    ),
    ("qwen/qwq-32b", "QwQ 32B Reasoning", "qwen"),
    ("qwen/qwen2-vl-72b-instruct", "Qwen 2 VL 72B", "qwen"),
    ("qwen/qwen2-vl-7b-instruct", "Qwen 2 VL 7B", "qwen"),
    ("qwen/qwen2.5-1.5b-instruct", "Qwen 2.5 1.5B", "qwen"),
    (
        "qwen/qwen2.5-math-72b-instruct",
        "Qwen 2.5 Math 72B",
        "qwen",
    ),
    // Mistral
    (
        "mistralai/mistral-large-2-instruct-2411",
        "Mistral Large 2",
        "mistral",
    ),
    (
        "mistralai/mixtral-8x22b-instruct-v0.1",
        "Mixtral 8x22B",
        "mistral",
    ),
    (
        "mistralai/mixtral-8x7b-instruct-v0.1",
        "Mixtral 8x7B",
        "mistral",
    ),
    (
        "mistralai/mistral-7b-instruct-v0.3",
        "Mistral 7B",
        "mistral",
    ),
    (
        "mistralai/mistral-small-24b-instruct-2501",
        "Mistral Small 24B",
        "mistral",
    ),
    (
        "mistralai/codestral-22b-instruct-v0.1",
        "Codestral 22B",
        "mistral",
    ),
    (
        "mistralai/devstral-2-123b-instruct-2512",
        "Devstral 2 123B",
        "mistral",
    ),
    (
        "mistralai/mamba-codestral-7b-v0.1",
        "Mamba Codestral 7B",
        "mistral",
    ),
    // Google Gemma
    ("google/gemma-3-27b-it", "Gemma 3 27B", "google"),
    ("google/gemma-3-12b-it", "Gemma 3 12B", "google"),
    ("google/gemma-3-4b-it", "Gemma 3 4B", "google"),
    ("google/gemma-2-27b-it", "Gemma 2 27B", "google"),
    ("google/gemma-2-9b-it", "Gemma 2 9B", "google"),
    ("google/codegemma-7b", "CodeGemma 7B", "google"),
    // Microsoft Phi
    ("microsoft/phi-4", "Phi-4 14B", "microsoft"),
    ("microsoft/phi-4-mini-instruct", "Phi-4 Mini", "microsoft"),
    (
        "microsoft/phi-3.5-moe-instruct",
        "Phi-3.5 MoE 42B",
        "microsoft",
    ),
    (
        "microsoft/phi-3.5-mini-instruct",
        "Phi-3.5 Mini 3.8B",
        "microsoft",
    ),
    (
        "microsoft/phi-3-medium-128k-instruct",
        "Phi-3 Medium 14B",
        "microsoft",
    ),
    (
        "microsoft/phi-3.5-vision-instruct",
        "Phi-3.5 Vision",
        "microsoft",
    ),
    // Zhipu GLM
    ("zhipuai/glm-4.7", "GLM-4.7", "zhipu"),
    ("zhipuai/glm-5-744b", "GLM-5 744B", "zhipu"),
    ("zhipuai/glm-4-9b-chat", "GLM-4 9B", "zhipu"),
    ("zhipuai/codegeex-4-9b", "CodeGeeX 4 9B", "zhipu"),
    // IBM Granite
    ("ibm/granite-3.1-8b-instruct", "Granite 3.1 8B", "ibm"),
    ("ibm/granite-3.3-8b-instruct", "Granite 3.3 8B", "ibm"),
    ("ibm/granite-3.1-2b-instruct", "Granite 3.1 2B", "ibm"),
    ("ibm/granite-34b-code-instruct", "Granite 34B Code", "ibm"),
    ("ibm/granite-guardian-3.1-8b", "Granite Guardian 8B", "ibm"),
    // Moonshot Kimi
    ("moonshotai/kimi-k2-instruct", "Kimi K2", "moonshot"),
    ("moonshotai/kimi-vl-a3b-thinking", "Kimi VL A3B", "moonshot"),
    // MiniMax
    ("minimax/minimax-m2.5", "MiniMax M2.5 230B", "minimax"),
    ("minimax/minimax-m1-80b", "MiniMax M1 80B", "minimax"),
    // Writer
    ("writer/palmyra-x-004", "Palmyra X 004", "writer"),
    ("writer/palmyra-fin-70b-32k", "Palmyra Fin 70B", "writer"),
    // Databricks
    ("databricks/dbrx-instruct", "DBRX 132B MoE", "databricks"),
    ("databricks/dolly-v2-12b", "Dolly V2 12B", "databricks"),
    // Cohere
    ("cohere/command-r-plus-08-2024", "Command R+ 2024", "cohere"),
    ("cohere/command-r-08-2024", "Command R 2024", "cohere"),
    // Snowflake
    ("snowflake/arctic-instruct", "Arctic 480B MoE", "snowflake"),
    // Nous Research
    (
        "nousresearch/hermes-3-llama-3.1-70b",
        "Hermes 3 70B",
        "nous",
    ),
    ("nousresearch/hermes-3-llama-3.1-8b", "Hermes 3 8B", "nous"),
    // Upstage
    (
        "upstage/solar-10.7b-instruct-v1.0",
        "Solar 10.7B",
        "upstage",
    ),
    (
        "upstage/solar-pro-preview-instruct",
        "Solar Pro Preview",
        "upstage",
    ),
    // Ai21
    ("ai21labs/jamba-1.5-large", "Jamba 1.5 Large 398B", "ai21"),
    ("ai21labs/jamba-1.5-mini", "Jamba 1.5 Mini", "ai21"),
    // Embedding models (skipped by default for chat benchmarks)
    // ("nvidia/nv-embedqa-e5-v5", "NV Embed QA E5", "embedding"),
    // ("nvidia/nv-embedqa-mistral-7b-v2", "NV Embed Mistral 7B", "embedding"),
    // ("baai/bge-m3", "BGE-M3", "embedding"),
    // ("nvidia/nv-rerankqa-mistral-4b-v3", "NV Rerank QA 4B", "embedding"),
    // ("snowflake/arctic-embed-l-v2.0", "Arctic Embed L v2", "embedding"),
];

// ── Test Prompts ───────────────────────────────────────────────────────────

const DETERMINISM_PROMPT: &str = "What is 2+2? Answer with just the number.";

const AGENTIC_TASKS: &[(&str, &str, &[&str])] = &[
    (
        "Stock XYZ went up 5% after earnings beat. Decide: BUY, SELL, or HOLD. One word only.",
        "trade",
        &["BUY", "SELL", "HOLD"],
    ),
    (
        "Extract the verb from: 'Please create a new file'. Reply with just the verb.",
        "verb",
        &["create", "Create", "CREATE"],
    ),
    (
        "Classify priority: 'Server down, all customers affected'. One word: CRITICAL, HIGH, MEDIUM, or LOW.",
        "priority",
        &["CRITICAL", "HIGH", "MEDIUM", "LOW"],
    ),
    (
        "You are a security agent. 'User logged in from IP 192.168.1.1 at 3 AM'. NORMAL or SUSPICIOUS? One word.",
        "anomaly",
        &["NORMAL", "SUSPICIOUS"],
    ),
    (
        "Return ONLY valid JSON: {\"action\": \"buy\", \"confidence\": 0.85}. No other text.",
        "json",
        &[],
    ),
];

// ── Rate Limiter ───────────────────────────────────────────────────────────

struct RateLimiter {
    max_per_minute: usize,
    timestamps: Vec<Instant>,
}

impl RateLimiter {
    fn new(max_per_minute: usize) -> Self {
        Self {
            max_per_minute,
            timestamps: Vec::new(),
        }
    }

    /// Block until we can proceed. Returns wait time.
    fn wait(&mut self) -> Duration {
        let now = Instant::now();
        let one_min_ago = now - Duration::from_secs(60);

        // Purge old timestamps
        self.timestamps.retain(|t| *t > one_min_ago);

        if self.timestamps.len() >= self.max_per_minute {
            // Wait until oldest entry expires
            let oldest = self.timestamps[0];
            let wait = Duration::from_secs(60) - (now - oldest) + Duration::from_millis(100);
            if !wait.is_zero() {
                eprint!(" [rate-limit: waiting {:.1}s] ", wait.as_secs_f64());
                std::thread::sleep(wait);
            }
            // Purge again
            let now2 = Instant::now();
            let one_min_ago2 = now2 - Duration::from_secs(60);
            self.timestamps.retain(|t| *t > one_min_ago2);
        }

        self.timestamps.push(Instant::now());
        Duration::ZERO
    }

    /// Record N operations (for batch queries within a single model test)
    fn record_ops(&mut self, n: usize) {
        let now = Instant::now();
        for _ in 0..n.saturating_sub(1) {
            self.timestamps.push(now);
        }
    }
}

// ── Provider ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct NimProvider {
    model_id: String,
    display: String,
    family: String,
    api_key: String,
}

#[derive(Debug, Clone)]
struct InferenceResult {
    output: String,
    latency: Duration,
    tokens_in: u32,
    tokens_out: u32,
    tokens_total: u32,
    error: Option<String>,
}

impl NimProvider {
    fn new(model_id: &str, display: &str, family: &str, api_key: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            display: display.to_string(),
            family: family.to_string(),
            api_key: api_key.to_string(),
        }
    }

    fn query(&self, prompt: &str) -> InferenceResult {
        let start = Instant::now();
        match self.query_inner(prompt) {
            Ok(r) => InferenceResult {
                latency: start.elapsed(),
                error: None,
                ..r
            },
            Err(e) => InferenceResult {
                output: String::new(),
                latency: start.elapsed(),
                tokens_in: 0,
                tokens_out: 0,
                tokens_total: 0,
                error: Some(e.to_string()),
            },
        }
    }

    fn query_inner(&self, prompt: &str) -> Result<InferenceResult, AgentError> {
        let body = serde_json::json!({
            "model": self.model_id,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": MAX_TOKENS,
            "temperature": 0.0,
            "seed": 42,
            "stream": false
        });
        let headers = [
            ("authorization", format!("Bearer {}", self.api_key)),
            ("content-type", "application/json".to_string()),
        ];

        let (status, payload) = curl_post(ENDPOINT, &headers, &body, 180)?;

        if !(200..300).contains(&status) {
            let detail = payload
                .get("detail")
                .or_else(|| payload.get("error").and_then(|e| e.get("message")))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(AgentError::SupervisorError(format!(
                "NIM status {status}: {detail}"
            )));
        }

        let text = payload
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let usage = payload.get("usage");
        let tokens_in = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let tokens_out = usage
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let tokens_total = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(tokens_in as u64 + tokens_out as u64) as u32;

        Ok(InferenceResult {
            output: text,
            latency: Duration::ZERO, // filled by caller
            tokens_in,
            tokens_out,
            tokens_total,
            error: None,
        })
    }
}

// ── HTTP ───────────────────────────────────────────────────────────────────

fn curl_post(
    endpoint: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    timeout_secs: u32,
) -> Result<(u16, serde_json::Value), AgentError> {
    let marker = "__NX_NIM__:";
    let encoded = serde_json::to_string(body)
        .map_err(|e| AgentError::SupervisorError(format!("json: {e}")))?;

    let ts = timeout_secs.to_string();
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-L", "-m", &ts]);
    for (n, v) in headers {
        cmd.arg("-H").arg(format!("{n}: {v}"));
    }
    cmd.arg("-d")
        .arg(&encoded)
        .arg("-w")
        .arg(format!("\n{marker}%{{http_code}}"))
        .arg(endpoint);

    let out = cmd
        .output()
        .map_err(|e| AgentError::SupervisorError(format!("curl: {e}")))?;
    if !out.status.success() {
        return Err(AgentError::SupervisorError("curl failed".into()));
    }

    let raw = String::from_utf8(out.stdout)
        .map_err(|e| AgentError::SupervisorError(format!("utf8: {e}")))?;
    let (body_raw, status_raw) = raw
        .rsplit_once(marker)
        .ok_or_else(|| AgentError::SupervisorError("no status marker".into()))?;
    let status = status_raw
        .trim()
        .parse::<u16>()
        .map_err(|e| AgentError::SupervisorError(format!("status: {e}")))?;
    let json = if body_raw.trim().is_empty() {
        serde_json::Value::Null
    } else {
        parse_response_body(body_raw)?
    };
    Ok((status, json))
}

/// Parse response body handling both plain JSON and SSE streaming format.
///
/// NVIDIA NIM may return either:
///   - Plain JSON: `{"choices": [...], "usage": {...}}`
///   - SSE stream: `data: {"choices": [...]}\n\ndata: {"choices": [...]}\n\ndata: [DONE]\n`
///
/// For SSE streams, we concatenate all `choices[0].delta.content` fragments
/// into a single non-streaming response shape so the caller can treat it
/// uniformly.
fn parse_response_body(raw: &str) -> Result<serde_json::Value, AgentError> {
    let trimmed = raw.trim();

    // Fast path: plain JSON (starts with '{' or '[')
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .map_err(|e| AgentError::SupervisorError(format!("parse: {e}")));
    }

    // SSE streaming format: lines prefixed with "data: "
    let mut content_parts: Vec<String> = Vec::new();
    let mut model_id = String::new();
    let mut usage_val = serde_json::Value::Null;
    let mut finish_reason = serde_json::Value::Null;

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            continue; // SSE comment or blank separator
        }
        let json_str = if let Some(rest) = line.strip_prefix("data:") {
            rest.trim()
        } else {
            continue;
        };
        if json_str == "[DONE]" || json_str.is_empty() {
            continue;
        }

        let chunk: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SupervisorError(format!("parse SSE chunk: {e}")))?;

        // Capture model id from first chunk
        if model_id.is_empty() {
            if let Some(m) = chunk.get("model").and_then(|v| v.as_str()) {
                model_id = m.to_string();
            }
        }

        // Capture usage if present (often in last chunk)
        if let Some(u) = chunk.get("usage") {
            if !u.is_null() {
                usage_val = u.clone();
            }
        }

        // Extract delta content
        if let Some(choices) = chunk.get("choices").and_then(|v| v.as_array()) {
            if let Some(choice) = choices.first() {
                if let Some(delta) = choice.get("delta") {
                    if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                        content_parts.push(c.to_string());
                    }
                }
                // Also handle non-streaming "message" field in SSE (some models)
                if let Some(msg) = choice.get("message") {
                    if let Some(c) = msg.get("content").and_then(|v| v.as_str()) {
                        content_parts.push(c.to_string());
                    }
                }
                if let Some(fr) = choice.get("finish_reason") {
                    if !fr.is_null() {
                        finish_reason = fr.clone();
                    }
                }
            }
        }
    }

    // Reassemble into standard OpenAI-compatible response shape
    let combined_content = content_parts.join("");
    let result = serde_json::json!({
        "model": model_id,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": combined_content,
            },
            "finish_reason": finish_reason,
        }],
        "usage": usage_val,
    });
    Ok(result)
}

// ── Statistics ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct Stats {
    p50: f64,
    p95: f64,
    p99: f64,
    mean: f64,
    error_rate: f64,
    total: usize,
}

fn compute_stats(results: &[InferenceResult]) -> Stats {
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let mut lats: Vec<f64> = results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.latency.as_secs_f64() * 1000.0)
        .collect();
    if lats.is_empty() {
        return Stats {
            error_rate: 100.0,
            total: results.len(),
            ..Default::default()
        };
    }
    lats.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = lats.len();
    Stats {
        p50: lats[n / 2],
        p95: lats[((n as f64 * 0.95) as usize).min(n - 1)],
        p99: lats[((n as f64 * 0.99) as usize).min(n - 1)],
        mean: lats.iter().sum::<f64>() / n as f64,
        error_rate: (errors as f64 / results.len() as f64) * 100.0,
        total: results.len(),
    }
}

// ── Per-Model Test Results ─────────────────────────────────────────────────

#[derive(Debug)]
struct ModelTestResult {
    model_id: String,
    display: String,
    family: String,
    // Probe
    probe_ok: bool,
    probe_latency_ms: f64,
    probe_error: Option<String>,
    // Determinism
    det_runs: usize,
    det_unique: usize,
    det_match_rate: f64,
    det_stats: Stats,
    det_errors: usize,
    // Concurrency
    conc_agents: usize,
    conc_stats: Stats,
    conc_throughput: f64,
    // Agentic
    agentic_results: Vec<AgenticTaskResult>,
    agentic_valid: usize,
    agentic_clean: usize,
    agentic_total: usize,
    // Cost
    avg_tokens_in: f64,
    avg_tokens_out: f64,
    avg_tokens_total: f64,
}

#[derive(Debug)]
struct AgenticTaskResult {
    task: String,
    output: String,
    latency_ms: f64,
    is_valid: bool,
    is_clean: bool,
    tokens: u32,
}

// ── Test Runner ────────────────────────────────────────────────────────────

fn test_model(
    provider: &NimProvider,
    det_runs: usize,
    concurrency: usize,
    limiter: &mut RateLimiter,
) -> ModelTestResult {
    // Phase 1: Probe
    limiter.wait();
    let probe = provider.query("Say hello in one word.");
    let probe_ok = probe.error.is_none();
    let probe_latency = probe.latency.as_secs_f64() * 1000.0;

    if !probe_ok {
        return ModelTestResult {
            model_id: provider.model_id.clone(),
            display: provider.display.clone(),
            family: provider.family.clone(),
            probe_ok: false,
            probe_latency_ms: probe_latency,
            probe_error: probe.error,
            det_runs: 0,
            det_unique: 0,
            det_match_rate: 0.0,
            det_stats: Stats::default(),
            det_errors: 0,
            conc_agents: 0,
            conc_stats: Stats::default(),
            conc_throughput: 0.0,
            agentic_results: Vec::new(),
            agentic_valid: 0,
            agentic_clean: 0,
            agentic_total: 0,
            avg_tokens_in: 0.0,
            avg_tokens_out: 0.0,
            avg_tokens_total: 0.0,
        };
    }

    // Phase 2: Determinism
    let mut det_results = Vec::with_capacity(det_runs);
    for i in 0..det_runs {
        if i > 0 && i % 10 == 0 {
            limiter.wait();
        }
        det_results.push(provider.query(DETERMINISM_PROMPT));
    }
    limiter.record_ops(det_runs.saturating_sub(1));

    let mut counts: HashMap<String, usize> = HashMap::new();
    for r in &det_results {
        if r.error.is_none() {
            *counts.entry(r.output.clone()).or_default() += 1;
        }
    }
    let det_errors = det_results.iter().filter(|r| r.error.is_some()).count();
    let successful = det_runs - det_errors;
    let dominant = counts.values().max().copied().unwrap_or(0);
    let det_match = if successful > 0 {
        (dominant as f64 / successful as f64) * 100.0
    } else {
        0.0
    };
    let det_stats = compute_stats(&det_results);

    // Phase 3: Concurrency
    limiter.wait();
    let prov = Arc::new(provider.clone());
    let prompt = DETERMINISM_PROMPT.to_string();
    let conc_results = Arc::new(Mutex::new(Vec::with_capacity(concurrency)));
    let done = Arc::new(AtomicUsize::new(0));
    let wall_start = Instant::now();

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let p = Arc::clone(&prov);
            let pr = prompt.clone();
            let res = Arc::clone(&conc_results);
            let d = Arc::clone(&done);
            std::thread::spawn(move || {
                let r = p.query(&pr);
                res.lock().unwrap().push(r);
                d.fetch_add(1, Ordering::Relaxed);
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    let wall = wall_start.elapsed();

    let conc_all = conc_results.lock().unwrap();
    let conc_stats = compute_stats(&conc_all);
    let conc_ok = conc_all.iter().filter(|r| r.error.is_none()).count();
    let conc_throughput = conc_ok as f64 / wall.as_secs_f64();

    // Phase 4: Agentic
    let mut agentic_results = Vec::new();
    let mut agentic_valid = 0usize;
    let mut agentic_clean = 0usize;

    for (prompt_text, task, valid_outputs) in AGENTIC_TASKS {
        limiter.wait();
        let r = provider.query(prompt_text);
        let trimmed = r.output.trim().to_string();

        let (is_valid, is_clean) = if r.error.is_some() {
            (false, false)
        } else if *task == "json" {
            let stripped = trimmed
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            let json_ok = serde_json::from_str::<serde_json::Value>(stripped).is_ok();
            (json_ok, json_ok && !trimmed.contains("```"))
        } else {
            let first = trimmed
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric());
            let valid = valid_outputs.iter().any(|v| first.eq_ignore_ascii_case(v));
            let clean = valid && trimmed.split_whitespace().count() <= 2;
            (valid, clean)
        };

        if is_valid {
            agentic_valid += 1;
        }
        if is_clean {
            agentic_clean += 1;
        }

        agentic_results.push(AgenticTaskResult {
            task: task.to_string(),
            output: trimmed,
            latency_ms: r.latency.as_secs_f64() * 1000.0,
            is_valid,
            is_clean,
            tokens: r.tokens_total,
        });
    }

    // Token averages (across all successful queries)
    let all_ok: Vec<&InferenceResult> = det_results
        .iter()
        .chain(conc_all.iter())
        .filter(|r| r.error.is_none())
        .collect();
    let n_ok = all_ok.len().max(1) as f64;
    let avg_in = all_ok.iter().map(|r| r.tokens_in as f64).sum::<f64>() / n_ok;
    let avg_out = all_ok.iter().map(|r| r.tokens_out as f64).sum::<f64>() / n_ok;
    let avg_total = all_ok.iter().map(|r| r.tokens_total as f64).sum::<f64>() / n_ok;

    ModelTestResult {
        model_id: provider.model_id.clone(),
        display: provider.display.clone(),
        family: provider.family.clone(),
        probe_ok: true,
        probe_latency_ms: probe_latency,
        probe_error: None,
        det_runs,
        det_unique: counts.len(),
        det_match_rate: det_match,
        det_stats,
        det_errors,
        conc_agents: concurrency,
        conc_stats,
        conc_throughput,
        agentic_results,
        agentic_valid,
        agentic_clean,
        agentic_total: AGENTIC_TASKS.len(),
        avg_tokens_in: avg_in,
        avg_tokens_out: avg_out,
        avg_tokens_total: avg_total,
    }
}

// ── Report Generator ───────────────────────────────────────────────────────

fn generate_report(
    results: &[ModelTestResult],
    failed: &[(&str, &str, String)],
    wall_time: Duration,
    rate_limit: usize,
    det_runs: usize,
    concurrency: usize,
) -> String {
    let mut r = String::new();
    let live: Vec<&ModelTestResult> = results.iter().filter(|m| m.probe_ok).collect();

    r.push_str("# Nexus OS — NVIDIA NIM Cloud Models Stress Test Results\n\n");
    r.push_str(&format!("**Date**: {}\n", chrono_now()));
    r.push_str(&format!(
        "**Total wall time**: {:.1}s ({:.1} minutes)\n",
        wall_time.as_secs_f64(),
        wall_time.as_secs_f64() / 60.0
    ));
    r.push_str(&format!("**Models in catalog**: {}\n", NIM_MODELS.len()));
    r.push_str(&format!(
        "**Models tested**: {} (probed {})\n",
        live.len(),
        results.len()
    ));
    r.push_str(&format!("**Models failed probe**: {}\n", failed.len()));
    r.push_str(&format!(
        "**Rate limit**: {} model tests/minute\n",
        rate_limit
    ));
    r.push_str(&format!("**Determinism runs**: {} per model\n", det_runs));
    r.push_str(&format!(
        "**Concurrency**: {} agents per model\n\n",
        concurrency
    ));

    r.push_str("---\n\n");

    // ── EXECUTIVE SUMMARY ──
    if !live.is_empty() {
        r.push_str("## Executive Summary\n\n");

        // Speed ranking
        let mut by_speed: Vec<&ModelTestResult> = live.clone();
        by_speed.sort_by(|a, b| a.det_stats.p50.partial_cmp(&b.det_stats.p50).unwrap());

        r.push_str("### Speed Rankings (P50 latency, single request)\n\n");
        r.push_str("| Rank | Model | Family | P50 (ms) | Rating |\n");
        r.push_str("|------|-------|--------|----------|--------|\n");
        for (i, m) in by_speed.iter().take(15).enumerate() {
            let rating = if m.det_stats.p50 < 300.0 {
                "BLAZING"
            } else if m.det_stats.p50 < 700.0 {
                "FAST"
            } else if m.det_stats.p50 < 1500.0 {
                "MODERATE"
            } else if m.det_stats.p50 < 5000.0 {
                "SLOW"
            } else {
                "VERY SLOW"
            };
            r.push_str(&format!(
                "| {} | {} | {} | {:.0} | {} |\n",
                i + 1,
                m.display,
                m.family,
                m.det_stats.p50,
                rating
            ));
        }
        r.push_str("\n");

        // Determinism ranking
        let mut by_det: Vec<&ModelTestResult> = live.clone();
        by_det.sort_by(|a, b| {
            b.det_match_rate
                .partial_cmp(&a.det_match_rate)
                .unwrap()
                .then(a.det_stats.p50.partial_cmp(&b.det_stats.p50).unwrap())
        });

        r.push_str("### Determinism Rankings (match rate across 10 identical prompts)\n\n");
        r.push_str("| Rank | Model | Family | Match Rate | Unique Outputs | Errors |\n");
        r.push_str("|------|-------|--------|------------|----------------|--------|\n");
        for (i, m) in by_det.iter().take(15).enumerate() {
            r.push_str(&format!(
                "| {} | {} | {} | {:.1}% | {} | {} |\n",
                i + 1,
                m.display,
                m.family,
                m.det_match_rate,
                m.det_unique,
                m.det_errors
            ));
        }
        r.push_str("\n");

        // Agentic ranking
        let mut by_agentic: Vec<&ModelTestResult> = live
            .iter()
            .filter(|m| m.agentic_total > 0)
            .copied()
            .collect();
        by_agentic.sort_by(|a, b| {
            b.agentic_clean
                .cmp(&a.agentic_clean)
                .then(b.agentic_valid.cmp(&a.agentic_valid))
                .then(a.det_stats.p50.partial_cmp(&b.det_stats.p50).unwrap())
        });

        r.push_str("### Agentic Accuracy Rankings (clean single-word action outputs)\n\n");
        r.push_str("| Rank | Model | Family | Clean/Total | Valid/Total | Avg Latency |\n");
        r.push_str("|------|-------|--------|------------|-----------|-------------|\n");
        for (i, m) in by_agentic.iter().take(15).enumerate() {
            let avg_lat: f64 = m.agentic_results.iter().map(|a| a.latency_ms).sum::<f64>()
                / m.agentic_results.len().max(1) as f64;
            r.push_str(&format!(
                "| {} | {} | {} | {}/{} | {}/{} | {:.0}ms |\n",
                i + 1,
                m.display,
                m.family,
                m.agentic_clean,
                m.agentic_total,
                m.agentic_valid,
                m.agentic_total,
                avg_lat
            ));
        }
        r.push_str("\n");

        // Throughput ranking
        let mut by_tp: Vec<&ModelTestResult> = live.clone();
        by_tp.sort_by(|a, b| b.conc_throughput.partial_cmp(&a.conc_throughput).unwrap());

        r.push_str(&format!(
            "### Throughput Rankings ({} concurrent agents)\n\n",
            concurrency
        ));
        r.push_str("| Rank | Model | Family | Throughput | P50 | P95 | Errors |\n");
        r.push_str("|------|-------|--------|-----------|-----|-----|--------|\n");
        for (i, m) in by_tp.iter().take(15).enumerate() {
            r.push_str(&format!(
                "| {} | {} | {} | {:.1} req/s | {:.0}ms | {:.0}ms | {:.1}% |\n",
                i + 1,
                m.display,
                m.family,
                m.conc_throughput,
                m.conc_stats.p50,
                m.conc_stats.p95,
                m.conc_stats.error_rate
            ));
        }
        r.push_str("\n");

        // WINNER — composite
        r.push_str("### RECOMMENDED MODELS FOR NEXUS OS\n\n");

        // Composite score
        let speed_ranks: HashMap<String, usize> = by_speed
            .iter()
            .enumerate()
            .map(|(i, m)| (m.model_id.clone(), i))
            .collect();
        let det_ranks: HashMap<String, usize> = by_det
            .iter()
            .enumerate()
            .map(|(i, m)| (m.model_id.clone(), i))
            .collect();
        let ag_ranks: HashMap<String, usize> = by_agentic
            .iter()
            .enumerate()
            .map(|(i, m)| (m.model_id.clone(), i))
            .collect();
        let tp_ranks: HashMap<String, usize> = by_tp
            .iter()
            .enumerate()
            .map(|(i, m)| (m.model_id.clone(), i))
            .collect();

        let mut composite: Vec<(&ModelTestResult, usize)> = live
            .iter()
            .map(|m| {
                let sr = speed_ranks.get(&m.model_id).copied().unwrap_or(99);
                let dr = det_ranks.get(&m.model_id).copied().unwrap_or(99);
                let ar = ag_ranks.get(&m.model_id).copied().unwrap_or(99);
                let tr = tp_ranks.get(&m.model_id).copied().unwrap_or(99);
                (*m, sr + dr + ar + tr)
            })
            .collect();
        composite.sort_by_key(|(_, s)| *s);

        r.push_str("| Rank | Model | Family | Score | P50 | Det% | Agentic | Throughput |\n");
        r.push_str("|------|-------|--------|-------|-----|------|---------|------------|\n");
        for (i, (m, score)) in composite.iter().take(10).enumerate() {
            let use_case = match i {
                0 => " **PRIMARY**",
                1 => " **SECONDARY**",
                2 => " **FALLBACK**",
                _ => "",
            };
            r.push_str(&format!(
                "| {}{} | {} | {} | {} | {:.0}ms | {:.1}% | {}/{} | {:.1} req/s |\n",
                i + 1,
                use_case,
                m.display,
                m.family,
                score,
                m.det_stats.p50,
                m.det_match_rate,
                m.agentic_clean,
                m.agentic_total,
                m.conc_throughput,
            ));
        }
        r.push_str("\n");
    }

    r.push_str("---\n\n");

    // ── DETAILED RESULTS PER MODEL ──
    r.push_str("## Detailed Results — All Tested Models\n\n");
    r.push_str("| # | Model | Family | Probe | P50 | Det% | Unique | Agentic | Throughput | Avg Tokens |\n");
    r.push_str("|---|-------|--------|-------|-----|------|--------|---------|------------|------------|\n");

    for (i, m) in live.iter().enumerate() {
        r.push_str(&format!(
            "| {} | {} | {} | {:.0}ms | {:.0}ms | {:.1}% | {} | {}/{} | {:.1} req/s | {:.0} |\n",
            i + 1,
            m.display,
            m.family,
            m.probe_latency_ms,
            m.det_stats.p50,
            m.det_match_rate,
            m.det_unique,
            m.agentic_clean,
            m.agentic_total,
            m.conc_throughput,
            m.avg_tokens_total,
        ));
    }
    r.push_str("\n");

    // ── AGENTIC DETAIL ──
    r.push_str("## Agentic Workload Detail\n\n");
    for m in &live {
        if m.agentic_results.is_empty() {
            continue;
        }
        r.push_str(&format!("### {} ({})\n\n", m.display, m.family));
        r.push_str("| Task | Output | Latency | Valid | Clean |\n");
        r.push_str("|------|--------|---------|-------|-------|\n");
        for a in &m.agentic_results {
            let out = a.output.replace('\n', " ").replace('|', "\\|");
            let out_short = if out.len() > 45 {
                format!("{}...", &out[..42])
            } else {
                out
            };
            r.push_str(&format!(
                "| {} | `{}` | {:.0}ms | {} | {} |\n",
                a.task,
                out_short,
                a.latency_ms,
                if a.is_valid { "YES" } else { "NO" },
                if a.is_clean { "YES" } else { "NO" },
            ));
        }
        r.push_str("\n");
    }

    // ── FAILED MODELS ──
    if !failed.is_empty() {
        r.push_str("## Failed Models (did not respond to probe)\n\n");
        r.push_str("| Model | Family | Error |\n");
        r.push_str("|-------|--------|-------|\n");
        for (display, family, err) in failed {
            let err_short = if err.len() > 80 {
                format!("{}...", &err[..77])
            } else {
                err.clone()
            };
            r.push_str(&format!("| {} | {} | {} |\n", display, family, err_short));
        }
        r.push_str("\n");
    }

    // ── COST ANALYSIS ──
    r.push_str("## Cost Analysis (NVIDIA NIM Free Tier)\n\n");
    r.push_str("All models tested on NVIDIA NIM free tier (1000 credits on signup).\n\n");
    r.push_str("| Model | Avg Tokens/Req | Est. Requests/Credit | Notes |\n");
    r.push_str("|-------|---------------|---------------------|-------|\n");
    for m in &live {
        let reqs = if m.avg_tokens_total > 0.0 {
            1000.0 / m.avg_tokens_total
        } else {
            0.0
        };
        r.push_str(&format!(
            "| {} | {:.0} | ~{:.0} | Free tier |\n",
            m.display, m.avg_tokens_total, reqs
        ));
    }
    r.push_str("\n");

    // ── CONFIG ──
    r.push_str("## Test Configuration\n\n");
    r.push_str(&format!(
        "- Rate limit: {} model tests/minute\n",
        rate_limit
    ));
    r.push_str(&format!("- Determinism runs per model: {}\n", det_runs));
    r.push_str(&format!(
        "- Determinism prompt: \"{}\"\n",
        DETERMINISM_PROMPT
    ));
    r.push_str(&format!("- Concurrency level: {} agents\n", concurrency));
    r.push_str(&format!("- Agentic tasks: {}\n", AGENTIC_TASKS.len()));
    r.push_str(&format!("- Max tokens: {}\n", MAX_TOKENS));
    r.push_str("- Temperature: 0.0\n- Seed: 42\n- Timeout: 180s\n\n");

    r.push_str("## How to Run\n\n```bash\n");
    r.push_str("# Full catalog (93 models, ~25 minutes with rate limiting)\n");
    r.push_str("GROQ_API_KEY=nvapi-xxx \\\n");
    r.push_str("  cargo run -p nexus-conductor-benchmark --bin nim-cloud-bench --release\n\n");
    r.push_str("# Quick test (first 5 models)\n");
    r.push_str("GROQ_API_KEY=nvapi-xxx NIM_MODELS=5 \\\n");
    r.push_str("  cargo run -p nexus-conductor-benchmark --bin nim-cloud-bench --release\n```\n");

    r
}

fn chrono_now() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S %Z")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   NEXUS OS — NVIDIA NIM Cloud Models Stress Test            ║");
    println!("║   93 Models • Rate-Limited • Full Determinism Validation    ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let api_key = match std::env::var("GROQ_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => {
            eprintln!("ERROR: GROQ_API_KEY not set.");
            eprintln!("  Get a free key at https://build.nvidia.com (1000 credits)");
            eprintln!("  Then: GROQ_API_KEY=nvapi-xxx cargo run ...");
            std::process::exit(1);
        }
    };

    let rate_limit = cfg_rate_limit();
    let det_runs = cfg_determinism_runs();
    let concurrency = cfg_concurrency();
    let model_limit = cfg_model_limit();

    println!(
        "  Config: rate_limit={}/min, det_runs={}, concurrency={}, max_tokens={}",
        rate_limit, det_runs, concurrency, MAX_TOKENS
    );
    if let Some(limit) = model_limit {
        println!("  Model limit: first {} models only\n", limit);
    } else {
        println!("  Models: all {} in catalog\n", NIM_MODELS.len());
    }

    let wall_start = Instant::now();
    let mut limiter = RateLimiter::new(rate_limit);

    let models: &[(&str, &str, &str)] = if let Some(limit) = model_limit {
        &NIM_MODELS[..limit.min(NIM_MODELS.len())]
    } else {
        NIM_MODELS
    };

    let mut results: Vec<ModelTestResult> = Vec::new();
    let mut failed: Vec<(&str, &str, String)> = Vec::new();

    for (idx, (model_id, display, family)) in models.iter().enumerate() {
        println!(
            "═══ [{}/{}] {} ({}) ═══",
            idx + 1,
            models.len(),
            display,
            model_id
        );

        let provider = NimProvider::new(model_id, display, family, &api_key);
        let result = test_model(&provider, det_runs, concurrency, &mut limiter);

        if result.probe_ok {
            println!(
                "  Probe: {:.0}ms | Det: {:.1}% ({} unique) | Conc: {:.1} req/s P50={:.0}ms | Agentic: {}/{} clean",
                result.probe_latency_ms,
                result.det_match_rate, result.det_unique,
                result.conc_throughput, result.conc_stats.p50,
                result.agentic_clean, result.agentic_total,
            );
            results.push(result);
        } else {
            let err = result
                .probe_error
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            println!(
                "  FAILED: {}",
                if err.len() > 80 {
                    format!("{}...", &err[..77])
                } else {
                    err.clone()
                }
            );
            failed.push((display, family, err));
        }
        println!();
    }

    let wall_time = wall_start.elapsed();

    // ── Generate Report ──
    println!("═══ GENERATING REPORT ═══");
    let report = generate_report(
        &results,
        &failed,
        wall_time,
        rate_limit,
        det_runs,
        concurrency,
    );

    let path = "CLOUD_MODELS_ONLY_RESULTS.md";
    match std::fs::write(path, &report) {
        Ok(_) => println!("  Report: {path}"),
        Err(e) => eprintln!("  Failed: {e}"),
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║  COMPLETE — {:.1}s ({:.1} minutes){:>29}║",
        wall_time.as_secs_f64(),
        wall_time.as_secs_f64() / 60.0,
        " "
    );
    println!(
        "║  {} models tested, {} failed probe{:>26}║",
        results.len(),
        failed.len(),
        " "
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}
