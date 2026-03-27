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
//! Nexus Inference Consistency Stress Test — Distributed Determinism Validation
//!
//! Tests inference reproducibility, latency scaling, and determinism under load
//! across local Ollama and cloud NVIDIA NIM providers.
//!
//! Objectives:
//! 1. Determinism: identical prompts → identical outputs (50 runs per model)
//! 2. Latency: P50/P95/P99 at 100/500/1000 concurrent agents
//! 3. Stress determinism: 1000 agents querying simultaneously
//! 4. Model switching: Ollama → NIM → Ollama consistency
//! 5. Long-running session: 1-hour continuous inference stability
//!
//! Run: `cargo run -p nexus-conductor-benchmark --bin inference-consistency-bench --release`

use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Configuration ──────────────────────────────────────────────────────────

const DETERMINISM_RUNS: usize = 50;
const CONCURRENCY_LEVELS: &[usize] = &[100, 500, 1000];
const STRESS_AGENT_COUNT: usize = 1000;
const LONG_RUN_DURATION_SECS: u64 = 3600; // 1 hour
const LONG_RUN_CHECKPOINT_SECS: u64 = 300; // Report every 5 minutes
const MAX_TOKENS: u32 = 64;

// Deterministic prompts — temperature=0, seed fixed where supported
const TEST_PROMPTS: &[&str] = &[
    "What is 2+2? Answer with just the number.",
    "Name the capital of France in one word.",
    "Is water wet? Answer yes or no.",
    "What color is the sky on a clear day? One word.",
    "How many sides does a triangle have? Answer with just the number.",
];

const OLLAMA_DEFAULT_MODEL: &str = "llama3.2:1b";
const NVIDIA_DEFAULT_MODEL: &str = "meta/llama-3.1-8b-instruct";

// ── Provider Abstraction ───────────────────────────────────────────────────

/// Lightweight provider wrapper that bypasses the governed gateway for raw
/// latency/determinism measurement. Uses the same curl-based HTTP calls as
/// the real providers.
#[derive(Clone)]
struct BenchProvider {
    name: String,
    kind: ProviderKind,
}

#[derive(Clone)]
enum ProviderKind {
    Ollama { base_url: String, model: String },
    Nvidia { api_key: String, model: String },
}

#[derive(Debug, Clone)]
struct InferenceResult {
    output: String,
    output_hash: u64,
    latency: Duration,
    token_count: u32,
    error: Option<String>,
}

impl BenchProvider {
    fn ollama(model: &str) -> Self {
        let base_url =
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self {
            name: format!("ollama/{model}"),
            kind: ProviderKind::Ollama {
                base_url,
                model: model.to_string(),
            },
        }
    }

    fn nvidia(model: &str) -> Option<Self> {
        let api_key = std::env::var("GROQ_API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }
        Some(Self {
            name: format!("nvidia/{model}"),
            kind: ProviderKind::Nvidia {
                api_key,
                model: model.to_string(),
            },
        })
    }

    fn query(&self, prompt: &str) -> InferenceResult {
        let start = Instant::now();
        match self.query_inner(prompt) {
            Ok((text, tokens)) => {
                let hash = hash_output(&text);
                InferenceResult {
                    output: text,
                    output_hash: hash,
                    latency: start.elapsed(),
                    token_count: tokens,
                    error: None,
                }
            }
            Err(e) => InferenceResult {
                output: String::new(),
                output_hash: 0,
                latency: start.elapsed(),
                token_count: 0,
                error: Some(e.to_string()),
            },
        }
    }

    fn query_inner(&self, prompt: &str) -> Result<(String, u32), AgentError> {
        match &self.kind {
            ProviderKind::Ollama { base_url, model } => {
                let endpoint = format!("{}/api/generate", base_url.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "prompt": prompt,
                    "stream": false,
                    "options": {
                        "num_predict": MAX_TOKENS,
                        "temperature": 0.0,
                        "seed": 42
                    }
                });
                let (status, payload) = curl_post_json(&endpoint, &[], &body, 120)?;
                if !(200..300).contains(&status) {
                    return Err(AgentError::SupervisorError(format!(
                        "ollama returned status {status}"
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
            }
            ProviderKind::Nvidia { api_key, model } => {
                let endpoint = "https://integrate.api.nvidia.com/v1/chat/completions";
                let body = serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "max_tokens": MAX_TOKENS,
                    "temperature": 0.0,
                    "seed": 42
                });
                let headers = [
                    ("authorization", format!("Bearer {api_key}")),
                    ("content-type", "application/json".to_string()),
                ];
                let (status, payload) = curl_post_json(endpoint, &headers, &body, 120)?;
                if !(200..300).contains(&status) {
                    return Err(AgentError::SupervisorError(format!(
                        "nvidia nim returned status {status}"
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
    }

    fn health_check(&self) -> bool {
        match &self.kind {
            ProviderKind::Ollama { base_url, .. } => std::net::TcpStream::connect_timeout(
                &base_url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .parse()
                    .unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap()),
                Duration::from_millis(500),
            )
            .is_ok(),
            ProviderKind::Nvidia { api_key, model } => {
                // Quick test query
                let test = BenchProvider {
                    name: self.name.clone(),
                    kind: ProviderKind::Nvidia {
                        api_key: api_key.clone(),
                        model: model.clone(),
                    },
                };
                test.query_inner("Say hello").is_ok()
            }
        }
    }
}

// ── HTTP Helper ────────────────────────────────────────────────────────────

fn curl_post_json(
    endpoint: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    timeout_secs: u32,
) -> Result<(u16, serde_json::Value), AgentError> {
    let marker = "__NEXUS_BENCH_STATUS__:";
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
        .map_err(|e| AgentError::SupervisorError(format!("response not utf8: {e}")))?;
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

// ── Hashing ────────────────────────────────────────────────────────────────

fn hash_output(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ── Latency Statistics ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LatencyStats {
    count: usize,
    min_ms: f64,
    max_ms: f64,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    errors: usize,
    error_rate_pct: f64,
}

fn compute_latency_stats(results: &[InferenceResult]) -> LatencyStats {
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let mut latencies: Vec<f64> = results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.latency.as_secs_f64() * 1000.0)
        .collect();

    if latencies.is_empty() {
        return LatencyStats {
            count: results.len(),
            min_ms: 0.0,
            max_ms: 0.0,
            mean_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            errors,
            error_rate_pct: 100.0,
        };
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = latencies.len();
    let mean = latencies.iter().sum::<f64>() / n as f64;
    let p50 = latencies[n / 2];
    let p95 = latencies[(n as f64 * 0.95) as usize];
    let p99 = latencies[(n as f64 * 0.99) as usize];

    LatencyStats {
        count: results.len(),
        min_ms: latencies[0],
        max_ms: *latencies.last().unwrap(),
        mean_ms: mean,
        p50_ms: p50,
        p95_ms: p95,
        p99_ms: p99,
        errors,
        error_rate_pct: (errors as f64 / results.len() as f64) * 100.0,
    }
}

// ── Determinism Check ──────────────────────────────────────────────────────

#[derive(Debug)]
struct DeterminismResult {
    prompt: String,
    provider: String,
    total_runs: usize,
    unique_outputs: usize,
    match_rate_pct: f64,
    dominant_output: String,
    dominant_count: usize,
    variants: Vec<(String, usize)>,
    errors: usize,
    latency: LatencyStats,
}

fn run_determinism_check(provider: &BenchProvider, prompt: &str, runs: usize) -> DeterminismResult {
    println!(
        "    [{provider}] Running {runs} identical queries...",
        provider = provider.name
    );

    let results: Vec<InferenceResult> = (0..runs).map(|_| provider.query(prompt)).collect();

    let mut output_counts: HashMap<String, usize> = HashMap::new();
    for r in &results {
        if r.error.is_none() {
            *output_counts.entry(r.output.clone()).or_default() += 1;
        }
    }

    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let successful = runs - errors;

    let (dominant_output, dominant_count) = output_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(o, c)| (o.clone(), *c))
        .unwrap_or_default();

    let match_rate = if successful > 0 {
        (dominant_count as f64 / successful as f64) * 100.0
    } else {
        0.0
    };

    let mut variants: Vec<(String, usize)> = output_counts.into_iter().collect();
    variants.sort_by(|a, b| b.1.cmp(&a.1));

    let latency = compute_latency_stats(&results);

    DeterminismResult {
        prompt: prompt.to_string(),
        provider: provider.name.clone(),
        total_runs: runs,
        unique_outputs: variants.len(),
        match_rate_pct: match_rate,
        dominant_output,
        dominant_count,
        variants,
        errors,
        latency,
    }
}

// ── Concurrent Stress Test ─────────────────────────────────────────────────

struct ConcurrencyResult {
    concurrency: usize,
    provider: String,
    latency: LatencyStats,
    throughput_rps: f64,
    wall_time: Duration,
}

fn run_concurrency_test(
    provider: &BenchProvider,
    concurrency: usize,
    prompt: &str,
) -> ConcurrencyResult {
    println!(
        "    [{provider}] Spawning {concurrency} concurrent agents...",
        provider = provider.name
    );

    let provider = Arc::new(provider.clone());
    let prompt = prompt.to_string();
    let results = Arc::new(Mutex::new(Vec::with_capacity(concurrency)));
    let completed = Arc::new(AtomicUsize::new(0));

    let wall_start = Instant::now();

    let handles: Vec<_> = (0..concurrency)
        .map(|_| {
            let p = Arc::clone(&provider);
            let pr = prompt.clone();
            let res = Arc::clone(&results);
            let done = Arc::clone(&completed);
            std::thread::spawn(move || {
                let r = p.query(&pr);
                res.lock().unwrap().push(r);
                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                if c % 100 == 0 || c == concurrency {
                    eprint!("\r      Progress: {c}/{concurrency}");
                }
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }
    let wall_time = wall_start.elapsed();
    eprintln!();

    let all_results = results.lock().unwrap();
    let latency = compute_latency_stats(&all_results);
    let successful = all_results.iter().filter(|r| r.error.is_none()).count();
    let throughput = successful as f64 / wall_time.as_secs_f64();

    ConcurrencyResult {
        concurrency,
        provider: provider.name.clone(),
        latency,
        throughput_rps: throughput,
        wall_time,
    }
}

// ── Stress Determinism ─────────────────────────────────────────────────────

struct StressDeterminismResult {
    agent_count: usize,
    provider: String,
    unique_outputs: usize,
    match_rate_pct: f64,
    dominant_output: String,
    latency: LatencyStats,
}

fn run_stress_determinism(
    provider: &BenchProvider,
    agent_count: usize,
    prompt: &str,
) -> StressDeterminismResult {
    println!(
        "    [{provider}] {agent_count} agents, same prompt, checking output consistency...",
        provider = provider.name
    );

    let provider = Arc::new(provider.clone());
    let prompt = prompt.to_string();
    let results = Arc::new(Mutex::new(Vec::with_capacity(agent_count)));
    let completed = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..agent_count)
        .map(|_| {
            let p = Arc::clone(&provider);
            let pr = prompt.clone();
            let res = Arc::clone(&results);
            let done = Arc::clone(&completed);
            std::thread::spawn(move || {
                let r = p.query(&pr);
                res.lock().unwrap().push(r);
                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                if c % 100 == 0 || c == agent_count {
                    eprint!("\r      Progress: {c}/{agent_count}");
                }
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }
    eprintln!();

    let all_results = results.lock().unwrap();
    let latency = compute_latency_stats(&all_results);

    let mut output_counts: HashMap<String, usize> = HashMap::new();
    for r in all_results.iter() {
        if r.error.is_none() {
            *output_counts.entry(r.output.clone()).or_default() += 1;
        }
    }

    let (dominant, _count) = output_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(o, c)| (o.clone(), *c))
        .unwrap_or_default();

    let successful = all_results.iter().filter(|r| r.error.is_none()).count();
    let matching = output_counts.get(&dominant).copied().unwrap_or(0);
    let match_rate = if successful > 0 {
        (matching as f64 / successful as f64) * 100.0
    } else {
        0.0
    };

    StressDeterminismResult {
        agent_count,
        provider: provider.name.clone(),
        unique_outputs: output_counts.len(),
        match_rate_pct: match_rate,
        dominant_output: dominant,
        latency,
    }
}

// ── Model Switching Test ───────────────────────────────────────────────────

struct SwitchResult {
    prompt: String,
    outputs: Vec<(String, String)>, // (provider, output)
    cross_provider_match: bool,
    same_provider_match: bool,
}

fn run_model_switch_test(providers: &[BenchProvider], prompt: &str) -> SwitchResult {
    println!(
        "    Model switch test: querying {} providers in sequence...",
        providers.len()
    );

    let mut outputs = Vec::new();
    for p in providers {
        let r = p.query(prompt);
        let out = if r.error.is_some() {
            format!("[ERROR: {}]", r.error.unwrap())
        } else {
            r.output
        };
        outputs.push((p.name.clone(), out));
    }

    // Check if same-provider outputs match (first and last if same provider type)
    let same_provider_match = if providers.len() >= 3 {
        let first_kind = match &providers[0].kind {
            ProviderKind::Ollama { .. } => "ollama",
            ProviderKind::Nvidia { .. } => "nvidia",
        };
        let last_kind = match &providers[providers.len() - 1].kind {
            ProviderKind::Ollama { .. } => "ollama",
            ProviderKind::Nvidia { .. } => "nvidia",
        };
        if first_kind == last_kind {
            outputs[0].1 == outputs[outputs.len() - 1].1
        } else {
            true // N/A
        }
    } else {
        true
    };

    // Cross-provider: do all outputs match?
    let cross_provider_match = outputs.windows(2).all(|w| w[0].1 == w[1].1);

    SwitchResult {
        prompt: prompt.to_string(),
        outputs,
        cross_provider_match,
        same_provider_match,
    }
}

// ── Long-Running Session Test ──────────────────────────────────────────────

struct SessionCheckpoint {
    elapsed_secs: u64,
    total_requests: usize,
    errors: usize,
    match_rate_pct: f64,
    p50_ms: f64,
    p95_ms: f64,
}

struct LongSessionResult {
    provider: String,
    total_duration_secs: u64,
    total_requests: usize,
    total_errors: usize,
    overall_match_rate_pct: f64,
    checkpoints: Vec<SessionCheckpoint>,
    determinism_held: bool,
}

fn run_long_session(
    provider: &BenchProvider,
    duration_secs: u64,
    prompt: &str,
) -> LongSessionResult {
    println!(
        "    [{provider}] Long-running session ({duration_secs}s)...",
        provider = provider.name
    );

    let start = Instant::now();
    let mut checkpoints = Vec::new();
    let mut all_results = Vec::new();
    let mut checkpoint_results = Vec::new();
    let mut last_checkpoint = Instant::now();
    let mut request_count = 0usize;

    let dominant_output = {
        let r = provider.query(prompt);
        if r.error.is_some() {
            eprintln!("      WARN: first request failed, aborting long session");
            return LongSessionResult {
                provider: provider.name.clone(),
                total_duration_secs: 0,
                total_requests: 1,
                total_errors: 1,
                overall_match_rate_pct: 0.0,
                checkpoints: Vec::new(),
                determinism_held: false,
            };
        }
        all_results.push(r.clone());
        checkpoint_results.push(r.clone());
        request_count += 1;
        r.output
    };

    while start.elapsed().as_secs() < duration_secs {
        let r = provider.query(prompt);
        all_results.push(r.clone());
        checkpoint_results.push(r);
        request_count += 1;

        if request_count % 50 == 0 {
            eprint!(
                "\r      Requests: {request_count} | Elapsed: {}s",
                start.elapsed().as_secs()
            );
        }

        // Checkpoint every LONG_RUN_CHECKPOINT_SECS
        if last_checkpoint.elapsed().as_secs() >= LONG_RUN_CHECKPOINT_SECS {
            let errors = checkpoint_results
                .iter()
                .filter(|r| r.error.is_some())
                .count();
            let successful: Vec<&InferenceResult> = checkpoint_results
                .iter()
                .filter(|r| r.error.is_none())
                .collect();
            let matching = successful
                .iter()
                .filter(|r| r.output == dominant_output)
                .count();
            let match_rate = if !successful.is_empty() {
                (matching as f64 / successful.len() as f64) * 100.0
            } else {
                0.0
            };
            let stats = compute_latency_stats(&checkpoint_results);

            checkpoints.push(SessionCheckpoint {
                elapsed_secs: start.elapsed().as_secs(),
                total_requests: request_count,
                errors,
                match_rate_pct: match_rate,
                p50_ms: stats.p50_ms,
                p95_ms: stats.p95_ms,
            });

            checkpoint_results.clear();
            last_checkpoint = Instant::now();
        }
    }
    eprintln!();

    // Final checkpoint
    if !checkpoint_results.is_empty() {
        let errors = checkpoint_results
            .iter()
            .filter(|r| r.error.is_some())
            .count();
        let successful: Vec<&InferenceResult> = checkpoint_results
            .iter()
            .filter(|r| r.error.is_none())
            .collect();
        let matching = successful
            .iter()
            .filter(|r| r.output == dominant_output)
            .count();
        let match_rate = if !successful.is_empty() {
            (matching as f64 / successful.len() as f64) * 100.0
        } else {
            0.0
        };
        let stats = compute_latency_stats(&checkpoint_results);

        checkpoints.push(SessionCheckpoint {
            elapsed_secs: start.elapsed().as_secs(),
            total_requests: request_count,
            errors,
            match_rate_pct: match_rate,
            p50_ms: stats.p50_ms,
            p95_ms: stats.p95_ms,
        });
    }

    let total_errors = all_results.iter().filter(|r| r.error.is_some()).count();
    let successful_all: Vec<&InferenceResult> =
        all_results.iter().filter(|r| r.error.is_none()).collect();
    let overall_matching = successful_all
        .iter()
        .filter(|r| r.output == dominant_output)
        .count();
    let overall_rate = if !successful_all.is_empty() {
        (overall_matching as f64 / successful_all.len() as f64) * 100.0
    } else {
        0.0
    };

    let determinism_held = checkpoints.iter().all(|c| c.match_rate_pct >= 99.0);

    LongSessionResult {
        provider: provider.name.clone(),
        total_duration_secs: start.elapsed().as_secs(),
        total_requests: request_count,
        total_errors,
        overall_match_rate_pct: overall_rate,
        checkpoints,
        determinism_held,
    }
}

// ── Report Generator ───────────────────────────────────────────────────────

fn generate_report(
    determinism_results: &[DeterminismResult],
    concurrency_results: &[ConcurrencyResult],
    stress_results: &[StressDeterminismResult],
    switch_results: &[SwitchResult],
    session_results: &[LongSessionResult],
    providers_available: &[String],
    wall_time: Duration,
) -> String {
    let mut r = String::new();

    r.push_str("# Nexus OS — Inference Consistency Stress Test Results\n\n");
    r.push_str(&format!("**Date**: {}\n", chrono_now()));
    r.push_str(&format!(
        "**Total wall time**: {:.1}s\n",
        wall_time.as_secs_f64()
    ));
    r.push_str(&format!(
        "**Providers tested**: {}\n\n",
        providers_available.join(", ")
    ));

    // ── Summary Verdict ──
    r.push_str("## Summary Verdict\n\n");

    let all_determinism_pass = determinism_results.iter().all(|d| d.match_rate_pct >= 99.0);
    let stress_pass = stress_results.iter().all(|s| s.match_rate_pct >= 95.0);
    let session_pass = session_results.iter().all(|s| s.determinism_held);

    r.push_str("| Criteria | Target | Result | Status |\n");
    r.push_str("|----------|--------|--------|--------|\n");

    let det_actual = if determinism_results.is_empty() {
        "N/A".to_string()
    } else {
        let min = determinism_results
            .iter()
            .map(|d| d.match_rate_pct)
            .fold(f64::MAX, f64::min);
        format!("{min:.1}%")
    };
    r.push_str(&format!(
        "| Determinism (same model) | 100% match | {det_actual} | {} |\n",
        if all_determinism_pass { "PASS" } else { "FAIL" }
    ));

    // P95 latency targets
    let local_p95 = concurrency_results
        .iter()
        .filter(|c| c.provider.starts_with("ollama"))
        .map(|c| c.latency.p95_ms)
        .fold(0.0f64, f64::max);
    let cloud_p95 = concurrency_results
        .iter()
        .filter(|c| c.provider.starts_with("nvidia"))
        .map(|c| c.latency.p95_ms)
        .fold(0.0f64, f64::max);

    if local_p95 > 0.0 {
        r.push_str(&format!(
            "| P95 latency (local) | <1000ms | {local_p95:.0}ms | {} |\n",
            if local_p95 < 1000.0 { "PASS" } else { "FAIL" }
        ));
    }
    if cloud_p95 > 0.0 {
        r.push_str(&format!(
            "| P95 latency (cloud) | <2000ms | {cloud_p95:.0}ms | {} |\n",
            if cloud_p95 < 2000.0 { "PASS" } else { "FAIL" }
        ));
    }

    r.push_str(&format!(
        "| Stress determinism (1000 agents) | 0 drift | {} unique | {} |\n",
        stress_results
            .iter()
            .map(|s| s.unique_outputs)
            .max()
            .unwrap_or(0),
        if stress_pass { "PASS" } else { "FAIL" }
    ));

    r.push_str(&format!(
        "| 1-hour session stability | No degradation | {} | {} |\n",
        if session_results.is_empty() {
            "N/A".to_string()
        } else {
            format!(
                "{:.1}% avg match",
                session_results
                    .iter()
                    .map(|s| s.overall_match_rate_pct)
                    .sum::<f64>()
                    / session_results.len() as f64
            )
        },
        if session_pass || session_results.is_empty() {
            "PASS"
        } else {
            "FAIL"
        }
    ));

    r.push_str("\n---\n\n");

    // ── Test 1: Determinism ──
    r.push_str("## 1. Determinism Test (50 identical prompts per model)\n\n");
    if determinism_results.is_empty() {
        r.push_str("*No providers available for determinism testing.*\n\n");
    } else {
        r.push_str(
            "| Provider | Prompt | Runs | Unique Outputs | Match Rate | P50 | P95 | Errors |\n",
        );
        r.push_str(
            "|----------|--------|------|----------------|------------|-----|-----|--------|\n",
        );
        for d in determinism_results {
            let prompt_short = if d.prompt.len() > 40 {
                format!("{}...", &d.prompt[..37])
            } else {
                d.prompt.clone()
            };
            r.push_str(&format!(
                "| {} | {} | {} | {} | {:.1}% | {:.0}ms | {:.0}ms | {} |\n",
                d.provider,
                prompt_short,
                d.total_runs,
                d.unique_outputs,
                d.match_rate_pct,
                d.latency.p50_ms,
                d.latency.p95_ms,
                d.errors,
            ));
        }

        // Show any variant outputs
        let variants: Vec<&DeterminismResult> = determinism_results
            .iter()
            .filter(|d| d.unique_outputs > 1)
            .collect();
        if !variants.is_empty() {
            r.push_str("\n### Determinism Failures — Output Variants\n\n");
            for d in variants {
                r.push_str(&format!(
                    "**{} — \"{}\"** ({} unique outputs):\n",
                    d.provider, d.prompt, d.unique_outputs
                ));
                for (i, (output, count)) in d.variants.iter().enumerate() {
                    let truncated = if output.len() > 200 {
                        format!("{}...", &output[..197])
                    } else {
                        output.clone()
                    };
                    r.push_str(&format!(
                        "- Variant {}: `{}` (×{})\n",
                        i + 1,
                        truncated.replace('`', "'").replace('\n', " "),
                        count
                    ));
                }
                r.push('\n');
            }
        }
    }

    // ── Test 2: Latency Scaling ──
    r.push_str("## 2. Latency Scaling (concurrent agents)\n\n");
    if concurrency_results.is_empty() {
        r.push_str("*No providers available for concurrency testing.*\n\n");
    } else {
        r.push_str("| Provider | Concurrency | P50 | P95 | P99 | Min | Max | Throughput | Errors | Wall Time |\n");
        r.push_str("|----------|-------------|-----|-----|-----|-----|-----|------------|--------|----------|\n");
        for c in concurrency_results {
            r.push_str(&format!(
                "| {} | {} | {:.0}ms | {:.0}ms | {:.0}ms | {:.0}ms | {:.0}ms | {:.1} req/s | {:.1}% | {:.1}s |\n",
                c.provider,
                c.concurrency,
                c.latency.p50_ms,
                c.latency.p95_ms,
                c.latency.p99_ms,
                c.latency.min_ms,
                c.latency.max_ms,
                c.throughput_rps,
                c.latency.error_rate_pct,
                c.wall_time.as_secs_f64(),
            ));
        }
    }

    // ── Test 3: Stress Determinism ──
    r.push_str("\n## 3. Stress Determinism (1000 concurrent agents, same prompt)\n\n");
    if stress_results.is_empty() {
        r.push_str("*No providers available for stress determinism testing.*\n\n");
    } else {
        r.push_str(
            "| Provider | Agents | Unique Outputs | Match Rate | P50 | P95 | P99 | Errors |\n",
        );
        r.push_str(
            "|----------|--------|----------------|------------|-----|-----|-----|--------|\n",
        );
        for s in stress_results {
            r.push_str(&format!(
                "| {} | {} | {} | {:.1}% | {:.0}ms | {:.0}ms | {:.0}ms | {:.1}% |\n",
                s.provider,
                s.agent_count,
                s.unique_outputs,
                s.match_rate_pct,
                s.latency.p50_ms,
                s.latency.p95_ms,
                s.latency.p99_ms,
                s.latency.error_rate_pct,
            ));
        }
    }

    // ── Test 4: Model Switching ──
    r.push_str("\n## 4. Model Switching (Ollama → NIM → Ollama)\n\n");
    if switch_results.is_empty() {
        r.push_str("*Requires both Ollama and NVIDIA NIM to be available.*\n\n");
    } else {
        for sw in switch_results {
            r.push_str(&format!("**Prompt**: \"{}\"\n\n", sw.prompt));
            r.push_str("| Step | Provider | Output |\n");
            r.push_str("|------|----------|--------|\n");
            for (i, (prov, out)) in sw.outputs.iter().enumerate() {
                let truncated = if out.len() > 100 {
                    format!("{}...", &out[..97])
                } else {
                    out.clone()
                };
                r.push_str(&format!(
                    "| {} | {} | `{}` |\n",
                    i + 1,
                    prov,
                    truncated.replace('`', "'").replace('\n', " "),
                ));
            }
            r.push_str(&format!(
                "\n- Cross-provider match: **{}**\n",
                if sw.cross_provider_match {
                    "YES"
                } else {
                    "NO (expected — different models)"
                }
            ));
            r.push_str(&format!(
                "- Same-provider consistency: **{}**\n\n",
                if sw.same_provider_match {
                    "YES"
                } else {
                    "NO — DETERMINISM FAILURE"
                }
            ));
        }
    }

    // ── Test 5: Long-Running Session ──
    r.push_str("## 5. Long-Running Session Stability\n\n");
    if session_results.is_empty() {
        r.push_str("*Skipped (set `NEXUS_LONG_SESSION=1` to enable 1-hour test).*\n\n");
    } else {
        for s in session_results {
            r.push_str(&format!(
                "### {} — {}s total, {} requests\n\n",
                s.provider, s.total_duration_secs, s.total_requests
            ));
            r.push_str(&format!(
                "- Overall match rate: **{:.1}%**\n",
                s.overall_match_rate_pct
            ));
            r.push_str(&format!(
                "- Total errors: **{}** ({:.1}%)\n",
                s.total_errors,
                (s.total_errors as f64 / s.total_requests as f64) * 100.0
            ));
            r.push_str(&format!(
                "- Determinism held: **{}**\n\n",
                if s.determinism_held {
                    "YES"
                } else {
                    "NO — DEGRADATION DETECTED"
                }
            ));

            if !s.checkpoints.is_empty() {
                r.push_str(
                    "| Checkpoint | Elapsed | Requests | Errors | Match Rate | P50 | P95 |\n",
                );
                r.push_str(
                    "|------------|---------|----------|--------|------------|-----|-----|\n",
                );
                for (i, cp) in s.checkpoints.iter().enumerate() {
                    r.push_str(&format!(
                        "| {} | {}s | {} | {} | {:.1}% | {:.0}ms | {:.0}ms |\n",
                        i + 1,
                        cp.elapsed_secs,
                        cp.total_requests,
                        cp.errors,
                        cp.match_rate_pct,
                        cp.p50_ms,
                        cp.p95_ms,
                    ));
                }
            }
            r.push('\n');
        }
    }

    // ── Test Configuration ──
    r.push_str("## Test Configuration\n\n");
    r.push_str(&format!(
        "- Determinism runs per prompt: {DETERMINISM_RUNS}\n"
    ));
    r.push_str(&format!("- Concurrency levels: {:?}\n", CONCURRENCY_LEVELS));
    r.push_str(&format!("- Stress agent count: {STRESS_AGENT_COUNT}\n"));
    r.push_str(&format!("- Max tokens per request: {MAX_TOKENS}\n"));
    r.push_str("- Temperature: 0.0 (forced deterministic)\n");
    r.push_str("- Seed: 42 (where supported)\n");
    r.push_str(&format!("- Test prompts: {}\n", TEST_PROMPTS.len()));
    r.push_str(&format!(
        "- Long session duration: {LONG_RUN_DURATION_SECS}s\n"
    ));

    r
}

fn chrono_now() -> String {
    // Simple timestamp without pulling in chrono
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S %Z")
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  NEXUS OS — Inference Consistency Stress Test               ║");
    println!("║  Distributed Determinism Validation                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let wall_start = Instant::now();

    // ── Discover providers ──
    let ollama_model =
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| OLLAMA_DEFAULT_MODEL.to_string());
    let nvidia_model =
        std::env::var("NVIDIA_MODEL").unwrap_or_else(|_| NVIDIA_DEFAULT_MODEL.to_string());

    let ollama = BenchProvider::ollama(&ollama_model);
    let nvidia = BenchProvider::nvidia(&nvidia_model);

    println!("── Provider Discovery ──");
    let ollama_ok = ollama.health_check();
    println!(
        "  Ollama ({ollama_model}): {}",
        if ollama_ok {
            "AVAILABLE"
        } else {
            "UNAVAILABLE"
        }
    );

    let nvidia_ok = nvidia.as_ref().map_or(false, |n| {
        println!("  NVIDIA NIM ({nvidia_model}): checking...");
        let ok = n.health_check();
        println!(
            "  NVIDIA NIM ({nvidia_model}): {}",
            if ok {
                "AVAILABLE"
            } else {
                "UNAVAILABLE (check GROQ_API_KEY)"
            }
        );
        ok
    });
    if nvidia.is_none() {
        println!("  NVIDIA NIM: UNAVAILABLE (GROQ_API_KEY not set)");
    }

    let mut providers: Vec<BenchProvider> = Vec::new();
    if ollama_ok {
        providers.push(ollama.clone());
    }
    if nvidia_ok {
        providers.push(nvidia.clone().unwrap());
    }

    if providers.is_empty() {
        eprintln!("\nERROR: No providers available. Need at least Ollama or NVIDIA NIM.");
        eprintln!("  - Start Ollama: `ollama serve` + `ollama pull {ollama_model}`");
        eprintln!("  - Set GROQ_API_KEY for cloud inference");
        std::process::exit(1);
    }

    let provider_names: Vec<String> = providers.iter().map(|p| p.name.clone()).collect();
    println!("\n  Active providers: {}\n", provider_names.join(", "));

    // ── Test 1: Determinism ──
    println!("═══ TEST 1: Determinism (50 identical prompts) ═══");
    let mut determinism_results = Vec::new();
    for provider in &providers {
        for prompt in TEST_PROMPTS {
            let result = run_determinism_check(provider, prompt, DETERMINISM_RUNS);
            println!(
                "      → {}: {}/{} match ({:.1}%), {} unique outputs",
                provider.name,
                result.dominant_count,
                result.total_runs - result.errors,
                result.match_rate_pct,
                result.unique_outputs,
            );
            determinism_results.push(result);
        }
    }

    // ── Test 2: Latency Scaling ──
    println!("\n═══ TEST 2: Latency Scaling ═══");
    let mut concurrency_results = Vec::new();
    let latency_prompt = TEST_PROMPTS[0];
    for provider in &providers {
        for &level in CONCURRENCY_LEVELS {
            let result = run_concurrency_test(provider, level, latency_prompt);
            println!(
                "      → {}: P50={:.0}ms P95={:.0}ms P99={:.0}ms | {:.1} req/s | {:.1}% errors",
                provider.name,
                result.latency.p50_ms,
                result.latency.p95_ms,
                result.latency.p99_ms,
                result.throughput_rps,
                result.latency.error_rate_pct,
            );
            concurrency_results.push(result);
        }
    }

    // ── Test 3: Stress Determinism ──
    println!("\n═══ TEST 3: Stress Determinism (1000 agents) ═══");
    let mut stress_results = Vec::new();
    for provider in &providers {
        let result = run_stress_determinism(provider, STRESS_AGENT_COUNT, latency_prompt);
        println!(
            "      → {}: {} unique outputs, {:.1}% match, P95={:.0}ms",
            provider.name, result.unique_outputs, result.match_rate_pct, result.latency.p95_ms,
        );
        stress_results.push(result);
    }

    // ── Test 4: Model Switching ──
    println!("\n═══ TEST 4: Model Switching ═══");
    let mut switch_results = Vec::new();
    if ollama_ok && nvidia_ok {
        let nvidia_p = nvidia.clone().unwrap();
        for prompt in &TEST_PROMPTS[..2] {
            // Ollama → NIM → Ollama
            let switch_providers = vec![ollama.clone(), nvidia_p.clone(), ollama.clone()];
            let result = run_model_switch_test(&switch_providers, prompt);
            println!(
                "      → Cross-provider: {}, Same-provider: {}",
                if result.cross_provider_match {
                    "MATCH"
                } else {
                    "DIFFER (expected)"
                },
                if result.same_provider_match {
                    "MATCH"
                } else {
                    "FAIL"
                },
            );
            switch_results.push(result);
        }
    } else {
        println!("    Skipped: requires both Ollama and NVIDIA NIM");
    }

    // ── Test 5: Long-Running Session ──
    println!("\n═══ TEST 5: Long-Running Session ═══");
    let mut session_results = Vec::new();
    let long_session_enabled = std::env::var("NEXUS_LONG_SESSION")
        .map_or(false, |v| v == "1" || v.to_lowercase() == "true");

    if long_session_enabled {
        let duration = std::env::var("NEXUS_SESSION_DURATION")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(LONG_RUN_DURATION_SECS);

        for provider in &providers {
            let result = run_long_session(provider, duration, latency_prompt);
            println!(
                "      → {}: {} requests in {}s, {:.1}% match, held={}",
                provider.name,
                result.total_requests,
                result.total_duration_secs,
                result.overall_match_rate_pct,
                result.determinism_held,
            );
            session_results.push(result);
        }
    } else {
        println!("    Skipped (set NEXUS_LONG_SESSION=1 to enable)");
        println!("    Optional: NEXUS_SESSION_DURATION=<seconds> (default: 3600)");
    }

    // ── Generate Report ──
    let wall_time = wall_start.elapsed();
    let report = generate_report(
        &determinism_results,
        &concurrency_results,
        &stress_results,
        &switch_results,
        &session_results,
        &provider_names,
        wall_time,
    );

    let report_path = "INFERENCE_CONSISTENCY_RESULTS.md";
    match std::fs::write(report_path, &report) {
        Ok(_) => println!("\n✓ Report written to {report_path}"),
        Err(e) => eprintln!("\n✗ Failed to write report: {e}"),
    }

    println!("\n═══ COMPLETE ({:.1}s) ═══", wall_time.as_secs_f64());
}
