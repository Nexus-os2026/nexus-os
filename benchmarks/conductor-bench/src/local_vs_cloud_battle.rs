//! Nexus OS — Local vs Cloud Inference Battle
//!
//! Pits local Ollama models against NVIDIA NIM cloud models across every
//! dimension that matters for autonomous agent workloads:
//!   • Determinism (50 identical prompts, temp=0, seed=42)
//!   • Latency (P50/P95/P99 at 50, 100, 500 concurrent agents)
//!   • Agentic accuracy (BUY/SELL/HOLD, verb extraction, priority, JSON output)
//!   • Cost per request (token-based for cloud, $0 for local)
//!
//! Run:
//!   cargo run -p nexus-conductor-benchmark --bin local-vs-cloud-battle --release
//!
//! Environment:
//!   OLLAMA_URL          (default http://localhost:11434)
//!   GROQ_API_KEY  (get free at https://build.nvidia.com)

use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Configuration ──────────────────────────────────────────────────────────

const DETERMINISM_RUNS: usize = 50;
const CONCURRENCY_LEVELS: &[usize] = &[50, 100, 500];
const MAX_TOKENS: u32 = 64;

// ── Test Prompts ───────────────────────────────────────────────────────────

const DETERMINISM_PROMPTS: &[&str] = &[
    "What is 2+2? Answer with just the number.",
    "Name the capital of France in one word.",
    "Is water wet? Answer yes or no.",
    "What color is the sky on a clear day? One word.",
    "How many sides does a triangle have? Answer with just the number.",
];

const AGENTIC_PROMPTS: &[(&str, &str, &[&str])] = &[
    (
        "You are a trading agent. Stock XYZ went up 5% today after earnings beat. Decide: BUY, SELL, or HOLD. Answer with one word only.",
        "trade_decision",
        &["BUY", "SELL", "HOLD"],
    ),
    (
        "Extract the action verb from: 'Please create a new file called report.txt'. Reply with just the verb.",
        "verb_extraction",
        &["create", "Create", "CREATE"],
    ),
    (
        "Classify priority: 'Production database is down, all customers affected'. Reply with exactly one word: CRITICAL, HIGH, MEDIUM, or LOW.",
        "priority_classification",
        &["CRITICAL", "HIGH", "MEDIUM", "LOW"],
    ),
    (
        "You are a security agent. Analyze: 'User logged in from IP 192.168.1.1 at 3 AM'. Is this NORMAL or SUSPICIOUS? One word.",
        "anomaly_detection",
        &["NORMAL", "SUSPICIOUS", "Normal", "Suspicious"],
    ),
    (
        "You are a router agent. Given the task 'Write unit tests for auth module', which specialist should handle it: CODER, TESTER, REVIEWER, or DEPLOYER? One word.",
        "task_routing",
        &["TESTER", "CODER", "REVIEWER", "DEPLOYER", "Tester", "Coder"],
    ),
    (
        "Return ONLY valid JSON: {\"action\": \"buy\", \"confidence\": 0.85}. No other text.",
        "json_output",
        &[],  // special: validated by JSON parse
    ),
];

// ── Model Definitions ──────────────────────────────────────────────────────

const LOCAL_MODELS: &[(&str, &str)] = &[
    ("qwen2.5-coder:7b", "Qwen 2.5 Coder 7B"),
    ("glm4:9b", "GLM-4 9B"),
    ("llama3.1:latest", "Llama 3.1 8B"),
    ("qwen3.5:4b", "Qwen 3.5 4B (thinking)"),
    ("qwen2.5-coder:14b", "Qwen 2.5 Coder 14B"),
    ("qwen3.5:9b", "Qwen 3.5 9B (thinking)"),
];

const CLOUD_MODELS: &[(&str, &str, f64)] = &[
    // (model_id, display_name, cost_per_1k_tokens)
    (
        "deepseek-ai/deepseek-v3_1-terminus",
        "DeepSeek V3.1 Terminus 671B",
        0.0,
    ),
    ("qwen/qwen2.5-72b-instruct", "Qwen 2.5 72B", 0.0),
    ("zhipuai/glm-4.7", "GLM-4.7", 0.0),
    ("moonshotai/kimi-k2-instruct", "Kimi K2", 0.0),
    (
        "nvidia/llama-3.1-nemotron-ultra-253b-v1",
        "Nemotron Ultra 253B",
        0.0,
    ),
    ("meta/llama-3.3-70b-instruct", "Llama 3.3 70B", 0.0),
    ("meta/llama-3.1-8b-instruct", "Llama 3.1 8B (NIM)", 0.0),
    ("google/gemma-3-27b-it", "Gemma 3 27B", 0.0),
    ("microsoft/phi-4", "Phi-4 14B", 0.0),
    (
        "mistralai/mistral-large-2-instruct-2411",
        "Mistral Large 2",
        0.0,
    ),
];

// ── Provider Abstraction ───────────────────────────────────────────────────

#[derive(Clone)]
struct BattleProvider {
    display: String,
    model_id: String,
    is_local: bool,
    endpoint: String,
    api_key: Option<String>,
    cost_per_1k_tokens: f64,
}

#[derive(Debug, Clone)]
struct InferenceResult {
    output: String,
    latency: Duration,
    token_count: u32,
    error: Option<String>,
}

impl BattleProvider {
    fn local(model_id: &str, display: &str) -> Self {
        let base =
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self {
            display: display.to_string(),
            model_id: model_id.to_string(),
            is_local: true,
            endpoint: format!("{}/api/generate", base.trim_end_matches('/')),
            api_key: None,
            cost_per_1k_tokens: 0.0,
        }
    }

    fn cloud(model_id: &str, display: &str, cost: f64, api_key: &str) -> Self {
        Self {
            display: display.to_string(),
            model_id: model_id.to_string(),
            is_local: false,
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key: Some(api_key.to_string()),
            cost_per_1k_tokens: cost,
        }
    }

    fn tag(&self) -> &str {
        if self.is_local {
            "LOCAL"
        } else {
            "CLOUD"
        }
    }

    fn query(&self, prompt: &str) -> InferenceResult {
        let start = Instant::now();
        match self.query_inner(prompt) {
            Ok((text, tokens)) => InferenceResult {
                output: text,
                latency: start.elapsed(),
                token_count: tokens,
                error: None,
            },
            Err(e) => InferenceResult {
                output: String::new(),
                latency: start.elapsed(),
                token_count: 0,
                error: Some(e.to_string()),
            },
        }
    }

    fn query_inner(&self, prompt: &str) -> Result<(String, u32), AgentError> {
        if self.is_local {
            let body = serde_json::json!({
                "model": self.model_id,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "num_predict": MAX_TOKENS,
                    "temperature": 0.0,
                    "seed": 42
                }
            });
            let (status, payload) = curl_post(&self.endpoint, &[], &body, 120)?;
            if !(200..300).contains(&status) {
                return Err(AgentError::SupervisorError(format!(
                    "ollama status {status}"
                )));
            }
            let text = payload
                .get("response")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let tokens = payload
                .get("eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Ok((text, tokens))
        } else {
            let api_key = self.api_key.as_deref().unwrap_or_default();
            let body = serde_json::json!({
                "model": self.model_id,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": MAX_TOKENS,
                "temperature": 0.0,
                "seed": 42
            });
            let headers = [
                ("authorization", format!("Bearer {api_key}")),
                ("content-type", "application/json".to_string()),
            ];
            let (status, payload) = curl_post(&self.endpoint, &headers, &body, 180)?;
            if !(200..300).contains(&status) {
                let err_msg = payload
                    .get("detail")
                    .or_else(|| payload.get("error").and_then(|e| e.get("message")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(AgentError::SupervisorError(format!(
                    "nvidia nim status {status}: {err_msg}"
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
            let tokens = payload
                .get("usage")
                .and_then(|u| u.get("total_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Ok((text, tokens))
        }
    }

    fn health_check(&self) -> bool {
        if self.is_local {
            let url = self.endpoint.replace("/api/generate", "");
            let addr = url
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            if let Ok(sock) = addr.parse() {
                return std::net::TcpStream::connect_timeout(&sock, Duration::from_millis(500))
                    .is_ok();
            }
            false
        } else {
            // Quick probe — use a tiny prompt
            self.query_inner("Hi").is_ok()
        }
    }
}

// ── HTTP Helper ────────────────────────────────────────────────────────────

fn curl_post(
    endpoint: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    timeout_secs: u32,
) -> Result<(u16, serde_json::Value), AgentError> {
    let marker = "__NX_BATTLE__:";
    let encoded = serde_json::to_string(body)
        .map_err(|e| AgentError::SupervisorError(format!("json encode: {e}")))?;

    let timeout_str = timeout_secs.to_string();
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-L", "-m", &timeout_str]);
    for (name, value) in headers {
        cmd.arg("-H").arg(format!("{name}: {value}"));
    }
    if !headers.iter().any(|(n, _)| *n == "content-type") {
        cmd.arg("-H").arg("content-type: application/json");
    }
    cmd.arg("-d")
        .arg(&encoded)
        .arg("-w")
        .arg(format!("\n{marker}%{{http_code}}"))
        .arg(endpoint);

    let output = cmd
        .output()
        .map_err(|e| AgentError::SupervisorError(format!("curl failed: {e}")))?;
    if !output.status.success() {
        return Err(AgentError::SupervisorError("curl request failed".into()));
    }

    let raw = String::from_utf8(output.stdout)
        .map_err(|e| AgentError::SupervisorError(format!("not utf8: {e}")))?;
    let (body_raw, status_raw) = raw
        .rsplit_once(marker)
        .ok_or_else(|| AgentError::SupervisorError("missing status marker".into()))?;
    let status = status_raw
        .trim()
        .parse::<u16>()
        .map_err(|e| AgentError::SupervisorError(format!("bad status: {e}")))?;
    let json = if body_raw.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body_raw)
            .map_err(|e| AgentError::SupervisorError(format!("json parse: {e}")))?
    };
    Ok((status, json))
}

// ── Statistics ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct LatencyStats {
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    mean_ms: f64,
    min_ms: f64,
    max_ms: f64,
    error_rate: f64,
    total: usize,
}

fn compute_stats(results: &[InferenceResult]) -> LatencyStats {
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let mut latencies: Vec<f64> = results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.latency.as_secs_f64() * 1000.0)
        .collect();

    if latencies.is_empty() {
        return LatencyStats {
            error_rate: 100.0,
            total: results.len(),
            ..Default::default()
        };
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = latencies.len();

    LatencyStats {
        p50_ms: latencies[n / 2],
        p95_ms: latencies[((n as f64 * 0.95) as usize).min(n - 1)],
        p99_ms: latencies[((n as f64 * 0.99) as usize).min(n - 1)],
        mean_ms: latencies.iter().sum::<f64>() / n as f64,
        min_ms: latencies[0],
        max_ms: *latencies.last().unwrap(),
        error_rate: (errors as f64 / results.len() as f64) * 100.0,
        total: results.len(),
    }
}

// ── Determinism ────────────────────────────────────────────────────────────

struct DeterminismResult {
    provider: String,
    tag: String,
    prompt: String,
    runs: usize,
    unique_outputs: usize,
    match_rate: f64,
    dominant_count: usize,
    errors: usize,
    stats: LatencyStats,
}

fn run_determinism(provider: &BattleProvider, prompt: &str) -> DeterminismResult {
    let results: Vec<InferenceResult> = (0..DETERMINISM_RUNS)
        .map(|_| provider.query(prompt))
        .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for r in &results {
        if r.error.is_none() {
            *counts.entry(r.output.clone()).or_default() += 1;
        }
    }

    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let successful = DETERMINISM_RUNS - errors;
    let dominant = counts.values().max().copied().unwrap_or(0);
    let match_rate = if successful > 0 {
        (dominant as f64 / successful as f64) * 100.0
    } else {
        0.0
    };

    DeterminismResult {
        provider: provider.display.clone(),
        tag: provider.tag().to_string(),
        prompt: prompt.to_string(),
        runs: DETERMINISM_RUNS,
        unique_outputs: counts.len(),
        match_rate,
        dominant_count: dominant,
        errors,
        stats: compute_stats(&results),
    }
}

// ── Concurrency ────────────────────────────────────────────────────────────

struct ConcurrencyResult {
    provider: String,
    tag: String,
    concurrency: usize,
    stats: LatencyStats,
    throughput: f64,
    wall_secs: f64,
}

fn run_concurrency(
    provider: &BattleProvider,
    concurrency: usize,
    prompt: &str,
) -> ConcurrencyResult {
    let prov = Arc::new(provider.clone());
    let prompt = prompt.to_string();
    let results = Arc::new(Mutex::new(Vec::with_capacity(concurrency)));
    let done = Arc::new(AtomicUsize::new(0));

    let wall_start = Instant::now();
    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let p = Arc::clone(&prov);
            let pr = prompt.clone();
            let res = Arc::clone(&results);
            let d = Arc::clone(&done);
            std::thread::spawn(move || {
                let r = p.query(&pr);
                res.lock().unwrap().push(r);
                let c = d.fetch_add(1, Ordering::Relaxed) + 1;
                if c % 50 == 0 || c == concurrency {
                    eprint!("\r      {c}/{concurrency}");
                }
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }
    let wall = wall_start.elapsed();
    eprintln!();

    let all = results.lock().unwrap();
    let stats = compute_stats(&all);
    let ok = all.iter().filter(|r| r.error.is_none()).count();

    ConcurrencyResult {
        provider: provider.display.clone(),
        tag: provider.tag().to_string(),
        concurrency,
        stats,
        throughput: ok as f64 / wall.as_secs_f64(),
        wall_secs: wall.as_secs_f64(),
    }
}

// ── Agentic Accuracy ───────────────────────────────────────────────────────

struct AgenticResult {
    provider: String,
    tag: String,
    task: String,
    output: String,
    latency_ms: f64,
    is_valid: bool,
    is_clean: bool, // single word / exact match
    tokens: u32,
    error: Option<String>,
}

fn run_agentic(
    provider: &BattleProvider,
    prompt: &str,
    task: &str,
    valid_outputs: &[&str],
) -> AgenticResult {
    let r = provider.query(prompt);
    let trimmed = r.output.trim().to_string();

    let (is_valid, is_clean) = if r.error.is_some() {
        (false, false)
    } else if task == "json_output" {
        // Validate JSON
        let json_ok = serde_json::from_str::<serde_json::Value>(&trimmed).is_ok()
            || serde_json::from_str::<serde_json::Value>(
                &trimmed
                    .trim_start_matches("```json")
                    .trim_end_matches("```")
                    .trim(),
            )
            .is_ok();
        (json_ok, json_ok && !trimmed.contains("```"))
    } else {
        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(|c: char| !c.is_alphanumeric());
        let valid = valid_outputs
            .iter()
            .any(|v| first_word.eq_ignore_ascii_case(v) || trimmed.eq_ignore_ascii_case(v));
        let clean = valid && trimmed.split_whitespace().count() <= 2;
        (valid, clean)
    };

    AgenticResult {
        provider: provider.display.clone(),
        tag: provider.tag().to_string(),
        task: task.to_string(),
        output: trimmed,
        latency_ms: r.latency.as_secs_f64() * 1000.0,
        is_valid,
        is_clean,
        tokens: r.token_count,
        error: r.error,
    }
}

// ── Cost Calculator ────────────────────────────────────────────────────────

fn estimate_cost_per_request(provider: &BattleProvider, avg_tokens: f64) -> f64 {
    if provider.is_local {
        return 0.0;
    }
    // NVIDIA NIM free tier: 1000 credits on signup, effectively $0 for testing
    // But calculate theoretical cost based on token usage
    provider.cost_per_1k_tokens * avg_tokens / 1000.0
}

// ── Report Generator ───────────────────────────────────────────────────────

fn generate_report(
    local_providers: &[BattleProvider],
    cloud_providers: &[BattleProvider],
    det_results: &[DeterminismResult],
    conc_results: &[ConcurrencyResult],
    agentic_results: &[AgenticResult],
    wall_time: Duration,
) -> String {
    let mut r = String::new();

    // Header
    r.push_str("# Nexus OS — LOCAL vs CLOUD Inference Battle Results\n\n");
    r.push_str(&format!("**Date**: {}\n", chrono_now()));
    r.push_str(&format!(
        "**Total wall time**: {:.1}s ({:.1} minutes)\n",
        wall_time.as_secs_f64(),
        wall_time.as_secs_f64() / 60.0
    ));
    r.push_str(&format!(
        "**Local models tested**: {}\n",
        local_providers.len()
    ));
    r.push_str(&format!(
        "**Cloud models tested**: {}\n",
        cloud_providers.len()
    ));
    r.push_str(&format!(
        "**Total models**: {}\n\n",
        local_providers.len() + cloud_providers.len()
    ));

    r.push_str("| Side | Models |\n|------|--------|\n");
    for p in local_providers {
        r.push_str(&format!("| LOCAL | {} (`{}`) |\n", p.display, p.model_id));
    }
    for p in cloud_providers {
        r.push_str(&format!("| CLOUD | {} (`{}`) |\n", p.display, p.model_id));
    }
    r.push_str("\n---\n\n");

    // ── EXECUTIVE SUMMARY ──
    r.push_str("## BATTLE VERDICT\n\n");

    // Aggregate scores per model
    struct ModelScore {
        name: String,
        tag: String,
        avg_p50: f64,
        avg_det: f64,
        agentic_valid: usize,
        agentic_clean: usize,
        agentic_total: usize,
        throughput_50: f64,
        cost: f64,
    }

    let all_providers: Vec<&BattleProvider> = local_providers
        .iter()
        .chain(cloud_providers.iter())
        .collect();

    let mut scores: Vec<ModelScore> = Vec::new();
    for prov in &all_providers {
        let det_for_model: Vec<&DeterminismResult> = det_results
            .iter()
            .filter(|d| d.provider == prov.display)
            .collect();
        let avg_p50 = if det_for_model.is_empty() {
            f64::MAX
        } else {
            det_for_model.iter().map(|d| d.stats.p50_ms).sum::<f64>() / det_for_model.len() as f64
        };
        let avg_det = if det_for_model.is_empty() {
            0.0
        } else {
            det_for_model.iter().map(|d| d.match_rate).sum::<f64>() / det_for_model.len() as f64
        };

        let ag_for_model: Vec<&AgenticResult> = agentic_results
            .iter()
            .filter(|a| a.provider == prov.display)
            .collect();
        let ag_valid = ag_for_model.iter().filter(|a| a.is_valid).count();
        let ag_clean = ag_for_model.iter().filter(|a| a.is_clean).count();
        let ag_total = ag_for_model.len();

        let tp_50 = conc_results
            .iter()
            .find(|c| c.provider == prov.display && c.concurrency == 50)
            .map_or(0.0, |c| c.throughput);

        let avg_tokens: f64 = ag_for_model
            .iter()
            .filter(|a| a.error.is_none())
            .map(|a| a.tokens as f64)
            .sum::<f64>()
            / ag_for_model
                .iter()
                .filter(|a| a.error.is_none())
                .count()
                .max(1) as f64;

        scores.push(ModelScore {
            name: prov.display.clone(),
            tag: prov.tag().to_string(),
            avg_p50,
            avg_det,
            agentic_valid: ag_valid,
            agentic_clean: ag_clean,
            agentic_total: ag_total,
            throughput_50: tp_50,
            cost: estimate_cost_per_request(prov, avg_tokens),
        });
    }

    // Composite rank: speed_rank + determinism_rank + agentic_rank (lower = better)
    let mut speed_order: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.avg_p50))
        .collect();
    speed_order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut det_order: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.avg_det))
        .collect();
    det_order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut ag_order: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                i,
                if s.agentic_total > 0 {
                    s.agentic_clean as f64 / s.agentic_total as f64
                } else {
                    0.0
                },
            )
        })
        .collect();
    ag_order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let speed_rank: HashMap<usize, usize> = speed_order
        .iter()
        .enumerate()
        .map(|(r, (i, _))| (*i, r))
        .collect();
    let det_rank: HashMap<usize, usize> = det_order
        .iter()
        .enumerate()
        .map(|(r, (i, _))| (*i, r))
        .collect();
    let ag_rank: HashMap<usize, usize> = ag_order
        .iter()
        .enumerate()
        .map(|(r, (i, _))| (*i, r))
        .collect();

    let mut final_ranking: Vec<(usize, usize)> = (0..scores.len())
        .map(|i| (i, speed_rank[&i] + det_rank[&i] + ag_rank[&i]))
        .collect();
    final_ranking.sort_by_key(|(_, s)| *s);

    r.push_str("### Overall Rankings\n\n");
    r.push_str("| Rank | Side | Model | P50 (ms) | Determinism | Agentic (clean/total) | Throughput | Cost/req | Score |\n");
    r.push_str("|------|------|-------|----------|------------|----------------------|------------|----------|-------|\n");
    for (rank, (idx, composite)) in final_ranking.iter().enumerate() {
        let s = &scores[*idx];
        let agentic_str = format!("{}/{}", s.agentic_clean, s.agentic_total);
        let cost_str = if s.cost == 0.0 {
            "$0 (free)".to_string()
        } else {
            format!("${:.6}", s.cost)
        };
        r.push_str(&format!(
            "| {} | **{}** | {} | {:.0} | {:.1}% | {} | {:.1} req/s | {} | {} |\n",
            rank + 1,
            s.tag,
            s.name,
            s.avg_p50,
            s.avg_det,
            agentic_str,
            s.throughput_50,
            cost_str,
            composite,
        ));
    }
    r.push_str("\n");

    // Category winners
    if let Some((best_speed_idx, _)) = speed_order.first() {
        let s = &scores[*best_speed_idx];
        r.push_str(&format!(
            "**Fastest**: {} [{}] — {:.0}ms P50\n",
            s.name, s.tag, s.avg_p50
        ));
    }
    if let Some((best_det_idx, _)) = det_order.first() {
        let s = &scores[*best_det_idx];
        r.push_str(&format!(
            "**Most Deterministic**: {} [{}] — {:.1}%\n",
            s.name, s.tag, s.avg_det
        ));
    }
    if let Some((best_ag_idx, _)) = ag_order.first() {
        let s = &scores[*best_ag_idx];
        r.push_str(&format!(
            "**Best Agentic**: {} [{}] — {}/{} clean outputs\n",
            s.name, s.tag, s.agentic_clean, s.agentic_total
        ));
    }

    // Throughput winner
    if let Some(best_tp) = scores
        .iter()
        .max_by(|a, b| a.throughput_50.partial_cmp(&b.throughput_50).unwrap())
    {
        r.push_str(&format!(
            "**Highest Throughput**: {} [{}] — {:.1} req/s at 50 agents\n",
            best_tp.name, best_tp.tag, best_tp.throughput_50
        ));
    }
    r.push_str("\n---\n\n");

    // ── ROUTING STRATEGY ──
    r.push_str("## Recommended Routing Strategy for Nexus OS\n\n");

    let has_cloud = !cloud_providers.is_empty();
    let has_local = !local_providers.is_empty();

    if has_local && has_cloud {
        // Find best local and best cloud
        let best_local = final_ranking
            .iter()
            .find(|(i, _)| scores[*i].tag == "LOCAL")
            .map(|(i, _)| &scores[*i]);
        let best_cloud = final_ranking
            .iter()
            .find(|(i, _)| scores[*i].tag == "CLOUD")
            .map(|(i, _)| &scores[*i]);

        r.push_str("| Workload | Route To | Model | Why |\n");
        r.push_str("|----------|----------|-------|-----|\n");

        if let Some(bl) = best_local {
            r.push_str(&format!("| Simple agent tasks (classification, routing) | LOCAL | {} | {:.0}ms, $0 cost |\n",
                bl.name, bl.avg_p50));
            r.push_str(&format!("| High-volume batch inference (>100 agents) | LOCAL | {} | {:.1} req/s, zero API cost |\n",
                bl.name, bl.throughput_50));
        }
        if let Some(bc) = best_cloud {
            r.push_str(&format!("| Complex reasoning / code generation | CLOUD | {} | Higher quality, {:.1}% determinism |\n",
                bc.name, bc.avg_det));
            r.push_str(&format!("| Financial decisions (>$100 value) | CLOUD | {} | Larger model, better judgment |\n",
                bc.name));
        }
        r.push_str(
            "| Latency-critical (<200ms SLA) | LOCAL | Best local | No network round-trip |\n",
        );
        r.push_str(
            "| Fallback / resilience | LOCAL→CLOUD | Auto | If Ollama down, route to NIM |\n\n",
        );
    } else if has_local {
        r.push_str("*Cloud models not tested (GROQ_API_KEY not set).*\n\n");
        r.push_str("**Local-only routing recommendation:**\n\n");
        r.push_str("| Workload | Model | Why |\n");
        r.push_str("|----------|-------|-----|\n");
        for (rank, (idx, _)) in final_ranking.iter().take(3).enumerate() {
            let s = &scores[*idx];
            let why = match rank {
                0 => "Best overall (speed + accuracy + determinism)",
                1 => "Runner-up, use for secondary workloads",
                _ => "Fallback model",
            };
            r.push_str(&format!(
                "| {} | {} | {} |\n",
                if rank == 0 {
                    "Primary (all tasks)"
                } else if rank == 1 {
                    "Secondary"
                } else {
                    "Fallback"
                },
                s.name,
                why
            ));
        }
        r.push_str("\nSet `GROQ_API_KEY` to enable cloud model comparison.\n");
        r.push_str("Free signup: https://build.nvidia.com (1000 credits)\n\n");
    }

    r.push_str("---\n\n");

    // ── DETAILED RESULTS ──

    // 1. Determinism
    r.push_str("## 1. Determinism Test (50 identical prompts per model, temp=0, seed=42)\n\n");
    if det_results.is_empty() {
        r.push_str("*No results.*\n\n");
    } else {
        // Group by tag
        for tag in &["LOCAL", "CLOUD"] {
            let filtered: Vec<&DeterminismResult> =
                det_results.iter().filter(|d| d.tag == *tag).collect();
            if filtered.is_empty() {
                continue;
            }

            r.push_str(&format!("### {} Models\n\n", tag));
            r.push_str("| Model | Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |\n");
            r.push_str("|-------|--------|------|--------|------------|-----|-----|--------|\n");
            for d in &filtered {
                let prompt_short = if d.prompt.len() > 35 {
                    format!("{}...", &d.prompt[..32])
                } else {
                    d.prompt.clone()
                };
                r.push_str(&format!(
                    "| {} | {} | {} | {} | {:.1}% | {:.0}ms | {:.0}ms | {} |\n",
                    d.provider,
                    prompt_short,
                    d.runs,
                    d.unique_outputs,
                    d.match_rate,
                    d.stats.p50_ms,
                    d.stats.p95_ms,
                    d.errors,
                ));
            }
            r.push_str("\n");
        }
    }

    // 2. Latency Scaling
    r.push_str("## 2. Latency Scaling (concurrent agents)\n\n");
    if conc_results.is_empty() {
        r.push_str("*No results.*\n\n");
    } else {
        for tag in &["LOCAL", "CLOUD"] {
            let filtered: Vec<&ConcurrencyResult> =
                conc_results.iter().filter(|c| c.tag == *tag).collect();
            if filtered.is_empty() {
                continue;
            }

            r.push_str(&format!("### {} Models\n\n", tag));
            r.push_str("| Model | Agents | P50 | P95 | P99 | Throughput | Errors | Wall Time |\n");
            r.push_str("|-------|--------|-----|-----|-----|------------|--------|----------|\n");
            for c in &filtered {
                r.push_str(&format!(
                    "| {} | {} | {:.0}ms | {:.0}ms | {:.0}ms | {:.1} req/s | {:.1}% | {:.1}s |\n",
                    c.provider,
                    c.concurrency,
                    c.stats.p50_ms,
                    c.stats.p95_ms,
                    c.stats.p99_ms,
                    c.throughput,
                    c.stats.error_rate,
                    c.wall_secs,
                ));
            }
            r.push_str("\n");
        }
    }

    // 3. Agentic Workload
    r.push_str("## 3. Agentic Workload Accuracy\n\n");
    if agentic_results.is_empty() {
        r.push_str("*No results.*\n\n");
    } else {
        for tag in &["LOCAL", "CLOUD"] {
            let filtered: Vec<&AgenticResult> =
                agentic_results.iter().filter(|a| a.tag == *tag).collect();
            if filtered.is_empty() {
                continue;
            }

            r.push_str(&format!("### {} Models\n\n", tag));
            r.push_str("| Model | Task | Output | Latency | Valid | Clean |\n");
            r.push_str("|-------|------|--------|---------|-------|-------|\n");
            for a in &filtered {
                let out_clean = a.output.replace('\n', " ").replace('|', "\\|");
                let out_short = if out_clean.len() > 40 {
                    format!("{}...", &out_clean[..37])
                } else {
                    out_clean
                };
                r.push_str(&format!(
                    "| {} | {} | `{}` | {:.0}ms | {} | {} |\n",
                    a.provider,
                    a.task,
                    out_short,
                    a.latency_ms,
                    if a.error.is_some() {
                        "ERR"
                    } else if a.is_valid {
                        "YES"
                    } else {
                        "NO"
                    },
                    if a.is_clean { "YES" } else { "NO" },
                ));
            }
            r.push_str("\n");

            // Summary per model
            let mut model_agentic: HashMap<String, (usize, usize, usize)> = HashMap::new();
            for a in &filtered {
                let entry = model_agentic.entry(a.provider.clone()).or_default();
                entry.2 += 1;
                if a.is_valid {
                    entry.0 += 1;
                }
                if a.is_clean {
                    entry.1 += 1;
                }
            }
            r.push_str("**Agentic Summary:**\n\n");
            r.push_str("| Model | Valid/Total | Clean/Total | Accuracy | Cleanliness |\n");
            r.push_str("|-------|-----------|-----------|----------|-------------|\n");
            let mut sorted: Vec<_> = model_agentic.into_iter().collect();
            sorted.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .0.cmp(&a.1 .0)));
            for (name, (valid, clean, total)) in &sorted {
                r.push_str(&format!(
                    "| {} | {}/{} | {}/{} | {:.0}% | {:.0}% |\n",
                    name,
                    valid,
                    total,
                    clean,
                    total,
                    (*valid as f64 / *total as f64) * 100.0,
                    (*clean as f64 / *total as f64) * 100.0,
                ));
            }
            r.push_str("\n");
        }
    }

    // 4. Cost Analysis
    r.push_str("## 4. Cost Analysis\n\n");
    r.push_str("| Side | Model | Cost/Request | Cost/1K Requests | Cost/1M Requests |\n");
    r.push_str("|------|-------|-------------|-----------------|------------------|\n");
    for s in &scores {
        if s.cost == 0.0 {
            r.push_str(&format!(
                "| {} | {} | $0.00 | $0.00 | $0.00 |\n",
                s.tag, s.name
            ));
        } else {
            r.push_str(&format!(
                "| {} | {} | ${:.6} | ${:.3} | ${:.2} |\n",
                s.tag,
                s.name,
                s.cost,
                s.cost * 1000.0,
                s.cost * 1_000_000.0,
            ));
        }
    }
    r.push_str("\n*NVIDIA NIM free tier: 1000 credits on signup at build.nvidia.com*\n\n");

    // ── Test Configuration ──
    r.push_str("## Test Configuration\n\n");
    r.push_str(&format!(
        "- Determinism runs per prompt: {}\n",
        DETERMINISM_RUNS
    ));
    r.push_str(&format!(
        "- Determinism prompts: {}\n",
        DETERMINISM_PROMPTS.len()
    ));
    r.push_str(&format!("- Concurrency levels: {:?}\n", CONCURRENCY_LEVELS));
    r.push_str(&format!("- Agentic tasks: {}\n", AGENTIC_PROMPTS.len()));
    r.push_str(&format!("- Max tokens: {}\n", MAX_TOKENS));
    r.push_str("- Temperature: 0.0\n");
    r.push_str("- Seed: 42\n\n");

    r.push_str("## How to Run\n\n");
    r.push_str("```bash\n");
    r.push_str("# Local only\n");
    r.push_str("cargo run -p nexus-conductor-benchmark --bin local-vs-cloud-battle --release\n\n");
    r.push_str("# With NVIDIA NIM cloud models (free)\n");
    r.push_str("GROQ_API_KEY=nvapi-xxx \\\n");
    r.push_str("  cargo run -p nexus-conductor-benchmark --bin local-vs-cloud-battle --release\n");
    r.push_str("```\n");

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
    println!("║   NEXUS OS — LOCAL vs CLOUD INFERENCE BATTLE                ║");
    println!("║   Ollama vs NVIDIA NIM • Full Determinism Validation        ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let wall_start = Instant::now();

    // ── Discover Providers ──
    println!("═══ FIGHTER DISCOVERY ═══\n");

    // Local models
    let mut local_providers: Vec<BattleProvider> = Vec::new();
    println!("  LOCAL FIGHTERS (Ollama):");
    for (model_id, display) in LOCAL_MODELS {
        let prov = BattleProvider::local(model_id, display);
        print!("    {display}: ");
        // Check if model exists via Ollama tags
        let exists = check_ollama_model(model_id);
        if exists {
            println!("READY");
            local_providers.push(prov);
        } else {
            println!("NOT INSTALLED (ollama pull {model_id})");
        }
    }

    // Cloud models
    let mut cloud_providers: Vec<BattleProvider> = Vec::new();
    println!("\n  CLOUD FIGHTERS (NVIDIA NIM):");
    match std::env::var("GROQ_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            let key = key.trim().to_string();
            for (model_id, display, cost) in CLOUD_MODELS {
                let prov = BattleProvider::cloud(model_id, display, *cost, &key);
                print!("    {display}: ");
                let ok = prov.health_check();
                if ok {
                    println!("READY");
                    cloud_providers.push(prov);
                } else {
                    println!("FAILED (may not be available on NIM)");
                }
            }
        }
        _ => {
            println!("    GROQ_API_KEY not set — cloud battle skipped");
            println!("    Get free key: https://build.nvidia.com");
        }
    }

    let total = local_providers.len() + cloud_providers.len();
    println!(
        "\n  Battle roster: {} LOCAL vs {} CLOUD ({} total)\n",
        local_providers.len(),
        cloud_providers.len(),
        total
    );

    if total == 0 {
        eprintln!("ERROR: No fighters available!");
        eprintln!("  Start Ollama: ollama serve && ollama pull qwen2.5-coder:7b");
        eprintln!("  Or set: GROQ_API_KEY=nvapi-xxx");
        std::process::exit(1);
    }

    let all_providers: Vec<&BattleProvider> = local_providers
        .iter()
        .chain(cloud_providers.iter())
        .collect();

    // ── Test 1: Determinism ──
    println!(
        "═══ ROUND 1: DETERMINISM ({} runs × {} prompts × {} models) ═══",
        DETERMINISM_RUNS,
        DETERMINISM_PROMPTS.len(),
        total
    );

    let mut det_results = Vec::new();
    for prov in &all_providers {
        println!("\n  [{}] {} ({})", prov.tag(), prov.display, prov.model_id);
        for prompt in DETERMINISM_PROMPTS {
            let result = run_determinism(prov, prompt);
            println!(
                "    {}/{} match ({:.1}%), {} unique, P50={:.0}ms{}",
                result.dominant_count,
                result.runs - result.errors,
                result.match_rate,
                result.unique_outputs,
                result.stats.p50_ms,
                if result.errors > 0 {
                    format!(", {} errors", result.errors)
                } else {
                    String::new()
                },
            );
            det_results.push(result);
        }
    }

    // ── Test 2: Latency Scaling ──
    println!(
        "\n═══ ROUND 2: LATENCY SCALING (concurrency: {:?}) ═══",
        CONCURRENCY_LEVELS
    );

    let mut conc_results = Vec::new();
    let conc_prompt = DETERMINISM_PROMPTS[0];
    for prov in &all_providers {
        println!("\n  [{}] {}", prov.tag(), prov.display);
        for &level in CONCURRENCY_LEVELS {
            print!("    {} agents: ", level);
            let result = run_concurrency(prov, level, conc_prompt);
            println!(
                "P50={:.0}ms P95={:.0}ms P99={:.0}ms | {:.1} req/s | {:.1}% err | {:.1}s",
                result.stats.p50_ms,
                result.stats.p95_ms,
                result.stats.p99_ms,
                result.throughput,
                result.stats.error_rate,
                result.wall_secs,
            );
            // Skip 500-agent test for slow models (P50 > 5s at 50 agents)
            let slow_p50 = result.stats.p50_ms;
            let slow = level == 50 && slow_p50 > 5000.0;
            conc_results.push(result);

            if slow {
                println!(
                    "    Skipping 100/500 agents (model too slow: {:.0}ms P50 at 50)",
                    slow_p50
                );
                break;
            }
        }
    }

    // ── Test 3: Agentic Workload ──
    println!(
        "\n═══ ROUND 3: AGENTIC WORKLOAD ({} tasks × {} models) ═══",
        AGENTIC_PROMPTS.len(),
        total
    );

    let mut agentic_results = Vec::new();
    for prov in &all_providers {
        println!("\n  [{}] {}", prov.tag(), prov.display);
        for (prompt, task, valid) in AGENTIC_PROMPTS {
            let result = run_agentic(prov, prompt, task, valid);
            let status = if result.error.is_some() {
                "ERR"
            } else if result.is_clean {
                "CLEAN"
            } else if result.is_valid {
                "VALID"
            } else {
                "NOISY"
            };
            let out_preview = if result.output.len() > 30 {
                format!("{}...", &result.output[..27])
            } else {
                result.output.clone()
            };
            println!(
                "    {}: {} | `{}` | {:.0}ms | {} tokens",
                task,
                status,
                out_preview.replace('\n', " "),
                result.latency_ms,
                result.tokens
            );
            agentic_results.push(result);
        }
    }

    // ── Generate Report ──
    let wall_time = wall_start.elapsed();
    println!("\n═══ GENERATING BATTLE REPORT ═══");

    let report = generate_report(
        &local_providers,
        &cloud_providers,
        &det_results,
        &conc_results,
        &agentic_results,
        wall_time,
    );

    let path = "LOCAL_vs_CLOUD_BATTLE_RESULTS.md";
    match std::fs::write(path, &report) {
        Ok(_) => println!("  Report: {path}"),
        Err(e) => eprintln!("  Failed to write report: {e}"),
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║  BATTLE COMPLETE — {:.1}s total{:>33}║",
        wall_time.as_secs_f64(),
        " "
    );
    println!(
        "║  {} LOCAL vs {} CLOUD fighters{:>31}║",
        local_providers.len(),
        cloud_providers.len(),
        " "
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}

fn check_ollama_model(model_id: &str) -> bool {
    let base = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let url = format!("{}/api/tags", base.trim_end_matches('/'));

    let output = std::process::Command::new("curl")
        .args(["-sS", "-m", "5", &url])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                json.get("models")
                    .and_then(|v| v.as_array())
                    .map_or(false, |models| {
                        models.iter().any(|m| {
                            m.get("name").and_then(|n| n.as_str()).map_or(false, |n| {
                                n == model_id || n.starts_with(&format!("{model_id}:"))
                            })
                        })
                    })
            } else {
                false
            }
        }
        _ => false,
    }
}
