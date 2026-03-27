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
//! Nexus OS — Cloud Models Comparison Stress Test
//!
//! Tests ALL available cloud providers and models for inference consistency,
//! determinism, and latency under agentic workloads.
//!
//! Providers tested:
//! - Local: Ollama (baseline)
//! - Cloud: DeepSeek, Groq, Mistral, Together, Fireworks, Perplexity,
//!          OpenRouter, OpenAI, Gemini, Cohere, NVIDIA NIM (93 models)
//!
//! Objectives:
//! 1. Determinism: identical prompts → identical outputs (10 runs per model)
//! 2. Latency: P50/P95/P99 at concurrent counts of 50, 100, 500
//! 3. Cross-provider stress: 100 agents querying ALL models simultaneously
//! 4. Identify fastest and most consistent models for agentic workloads
//! 5. Output to CLOUD_MODELS_COMPARISON_RESULTS.md
//!
//! Run: `cargo run -p nexus-conductor-benchmark --bin cloud-models-bench --release`
//!
//! Environment variables (set whichever keys you have):
//!   DEEPSEEK_API_KEY, GROQ_API_KEY, MISTRAL_API_KEY, TOGETHER_API_KEY,
//!   FIREWORKS_API_KEY, PERPLEXITY_API_KEY, OPENROUTER_API_KEY,
//!   OPENAI_API_KEY, GEMINI_API_KEY, COHERE_API_KEY, GROQ_API_KEY,
//!   OLLAMA_URL (default: http://localhost:11434)

use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Configuration ──────────────────────────────────────────────────────────

const DETERMINISM_RUNS: usize = 10;
const CONCURRENCY_LEVELS: &[usize] = &[50, 100, 500];
const CROSS_PROVIDER_AGENTS: usize = 100;
const MAX_TOKENS: u32 = 64;

/// Deterministic prompts — temperature=0, seed=42 where supported
const TEST_PROMPTS: &[&str] = &[
    "What is 2+2? Answer with just the number.",
    "Name the capital of France in one word.",
    "Is water wet? Answer yes or no.",
    "What color is the sky on a clear day? One word.",
    "How many sides does a triangle have? Answer with just the number.",
];

/// Agentic workload prompts — tests tool-call-like reasoning
const AGENTIC_PROMPTS: &[&str] = &[
    "You are an agent. Decide: should you BUY, SELL, or HOLD stock XYZ given price went up 5%? Answer one word.",
    "Extract the action from this instruction: 'Please create a new file called report.txt'. Reply with just the verb.",
    "Classify this task priority: 'Server is down, customers affected'. Reply: CRITICAL, HIGH, MEDIUM, or LOW.",
];

// ── Provider Registry ──────────────────────────────────────────────────────

/// All cloud providers Nexus OS supports, with their env var, endpoint, and
/// default model for benchmarking.
const CLOUD_PROVIDERS: &[CloudProviderSpec] = &[
    CloudProviderSpec {
        name: "deepseek",
        env_key: "DEEPSEEK_API_KEY",
        endpoint: "https://api.deepseek.com/v1/chat/completions",
        default_model: "deepseek-chat",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_002,
    },
    CloudProviderSpec {
        name: "groq",
        env_key: "GROQ_API_KEY",
        endpoint: "https://api.groq.com/openai/v1/chat/completions",
        default_model: "llama-3.3-70b-versatile",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_000_6,
    },
    CloudProviderSpec {
        name: "mistral",
        env_key: "MISTRAL_API_KEY",
        endpoint: "https://api.mistral.ai/v1/chat/completions",
        default_model: "mistral-large-latest",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_002_5,
    },
    CloudProviderSpec {
        name: "together",
        env_key: "TOGETHER_API_KEY",
        endpoint: "https://api.together.xyz/v1/chat/completions",
        default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_001_8,
    },
    CloudProviderSpec {
        name: "fireworks",
        env_key: "FIREWORKS_API_KEY",
        endpoint: "https://api.fireworks.ai/inference/v1/chat/completions",
        default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_001_2,
    },
    CloudProviderSpec {
        name: "perplexity",
        env_key: "PERPLEXITY_API_KEY",
        endpoint: "https://api.perplexity.ai/chat/completions",
        default_model: "sonar-pro",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_003,
    },
    CloudProviderSpec {
        name: "openrouter",
        env_key: "OPENROUTER_API_KEY",
        endpoint: "https://openrouter.ai/api/v1/chat/completions",
        default_model: "meta-llama/llama-3.3-70b-instruct",
        api_format: ApiFormat::OpenRouter,
        cost_per_token: 0.000_002,
    },
    CloudProviderSpec {
        name: "openai",
        env_key: "OPENAI_API_KEY",
        endpoint: "https://api.openai.com/v1/chat/completions",
        default_model: "gpt-4o-mini",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_005,
    },
    CloudProviderSpec {
        name: "gemini",
        env_key: "GEMINI_API_KEY",
        endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        default_model: "gemini-2.0-flash",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_003_5,
    },
    CloudProviderSpec {
        name: "cohere",
        env_key: "COHERE_API_KEY",
        endpoint: "https://api.cohere.ai/v2/chat",
        default_model: "command-r-plus",
        api_format: ApiFormat::Cohere,
        cost_per_token: 0.000_003,
    },
    CloudProviderSpec {
        name: "nvidia",
        env_key: "GROQ_API_KEY",
        endpoint: "https://integrate.api.nvidia.com/v1/chat/completions",
        default_model: "deepseek-ai/deepseek-v3_1-terminus",
        api_format: ApiFormat::OpenAiCompatible,
        cost_per_token: 0.000_001,
    },
];

/// Subset of NVIDIA NIM models to benchmark (representative from each family)
const NVIDIA_BENCH_MODELS: &[(&str, &str)] = &[
    // DeepSeek family
    (
        "deepseek-ai/deepseek-v3_1-terminus",
        "DeepSeek V3.1 Terminus 671B",
    ),
    ("deepseek-ai/deepseek-r1", "DeepSeek R1 Reasoning"),
    (
        "deepseek-ai/deepseek-r1-distill-llama-8b",
        "DeepSeek R1 Distill 8B",
    ),
    // Llama family
    ("meta/llama-4-scout-17b-16e-instruct", "Llama 4 Scout 17B"),
    ("meta/llama-3.3-70b-instruct", "Llama 3.3 70B"),
    ("meta/llama-3.1-8b-instruct", "Llama 3.1 8B"),
    // Qwen family
    ("qwen/qwen2.5-72b-instruct", "Qwen 2.5 72B"),
    ("qwen/qwq-32b", "QwQ 32B Reasoning"),
    ("qwen/qwen2.5-coder-32b-instruct", "Qwen 2.5 Coder 32B"),
    // Mistral family
    ("mistralai/mistral-large-2-instruct-2411", "Mistral Large 2"),
    ("mistralai/mamba-codestral-7b-v0.1", "Mamba Codestral 7B"),
    // Gemma family
    ("google/gemma-3-27b-it", "Gemma 3 27B"),
    ("google/gemma-3-4b-it", "Gemma 3 4B"),
    // GLM family
    ("zhipuai/glm-4.7", "GLM-4.7"),
    ("zhipuai/glm-5-744b", "GLM-5 744B"),
    // Phi family
    ("microsoft/phi-4", "Phi-4 14B"),
    // Kimi
    ("moonshotai/kimi-k2-instruct", "Kimi K2"),
    // Nemotron
    (
        "nvidia/llama-3.1-nemotron-ultra-253b-v1",
        "Nemotron Ultra 253B",
    ),
    ("nvidia/nemotron-3-nano-30b-a3b", "Nemotron Nano 30B"),
    // IBM Granite
    ("ibm/granite-3.3-8b-instruct", "Granite 3.3 8B"),
];

#[derive(Debug, Clone, Copy)]
enum ApiFormat {
    OpenAiCompatible,
    OpenRouter,
    Cohere,
    Ollama,
}

#[derive(Debug, Clone)]
struct CloudProviderSpec {
    name: &'static str,
    env_key: &'static str,
    endpoint: &'static str,
    default_model: &'static str,
    api_format: ApiFormat,
    cost_per_token: f64,
}

// ── Bench Provider ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct BenchProvider {
    name: String,
    model: String,
    endpoint: String,
    api_key: Option<String>,
    format: ApiFormat,
    cost_per_token: f64,
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
    fn from_spec(spec: &CloudProviderSpec) -> Option<Self> {
        let api_key = std::env::var(spec.env_key)
            .ok()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())?;
        Some(Self {
            name: spec.name.to_string(),
            model: spec.default_model.to_string(),
            endpoint: spec.endpoint.to_string(),
            api_key: Some(api_key),
            format: spec.api_format,
            cost_per_token: spec.cost_per_token,
        })
    }

    fn nvidia_with_model(api_key: &str, model_id: &str, model_name: &str) -> Self {
        Self {
            name: format!("nvidia/{}", model_name),
            model: model_id.to_string(),
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key: Some(api_key.to_string()),
            format: ApiFormat::OpenAiCompatible,
            cost_per_token: 0.000_001,
        }
    }

    fn ollama(model: &str) -> Self {
        let base_url =
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self {
            name: "ollama".to_string(),
            model: model.to_string(),
            endpoint: format!("{}/api/generate", base_url.trim_end_matches('/')),
            api_key: None,
            format: ApiFormat::Ollama,
            cost_per_token: 0.0,
        }
    }

    fn display_name(&self) -> String {
        format!("{}/{}", self.name, self.model)
    }

    fn query(&self, prompt: &str) -> InferenceResult {
        let start = Instant::now();
        match self.query_inner(prompt) {
            Ok((text, tokens)) => InferenceResult {
                output_hash: hash_output(&text),
                output: text,
                latency: start.elapsed(),
                token_count: tokens,
                error: None,
            },
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
        match self.format {
            ApiFormat::Ollama => {
                let body = serde_json::json!({
                    "model": self.model,
                    "prompt": prompt,
                    "stream": false,
                    "options": {
                        "num_predict": MAX_TOKENS,
                        "temperature": 0.0,
                        "seed": 42
                    }
                });
                let (status, payload) = curl_post_json(&self.endpoint, &[], &body, 120)?;
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
            ApiFormat::Cohere => {
                let api_key = self.api_key.as_deref().unwrap_or_default();
                let body = serde_json::json!({
                    "model": self.model,
                    "message": prompt,
                    "max_tokens": MAX_TOKENS,
                    "temperature": 0.0,
                    "seed": 42
                });
                let headers = [
                    ("authorization", format!("Bearer {api_key}")),
                    ("content-type", "application/json".to_string()),
                ];
                let (status, payload) = curl_post_json(&self.endpoint, &headers, &body, 60)?;
                if !(200..300).contains(&status) {
                    return Err(AgentError::SupervisorError(format!(
                        "cohere returned status {status}"
                    )));
                }
                // Cohere v2 response: { "text": "...", ... }
                let text = payload
                    .get("text")
                    .or_else(|| {
                        payload.get("message").and_then(|m| {
                            m.get("content")
                                .and_then(|c| c.get(0))
                                .and_then(|t| t.get("text"))
                        })
                    })
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let tokens = payload
                    .get("meta")
                    .and_then(|m| m.get("tokens"))
                    .and_then(|t| t.get("output_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                Ok((text, tokens))
            }
            ApiFormat::OpenRouter => {
                let api_key = self.api_key.as_deref().unwrap_or_default();
                let body = serde_json::json!({
                    "model": self.model,
                    "messages": [{"role": "user", "content": prompt}],
                    "max_tokens": MAX_TOKENS,
                    "temperature": 0.0,
                    "seed": 42
                });
                let headers = [
                    ("authorization", format!("Bearer {api_key}")),
                    ("content-type", "application/json".to_string()),
                    ("http-referer", "https://nexus-os.dev".to_string()),
                    ("x-title", "Nexus OS Cloud Benchmark".to_string()),
                ];
                let (status, payload) = curl_post_json(&self.endpoint, &headers, &body, 120)?;
                if !(200..300).contains(&status) {
                    return Err(AgentError::SupervisorError(format!(
                        "openrouter returned status {status}"
                    )));
                }
                parse_openai_response(&payload)
            }
            ApiFormat::OpenAiCompatible => {
                let api_key = self.api_key.as_deref().unwrap_or_default();
                let body = serde_json::json!({
                    "model": self.model,
                    "messages": [{"role": "user", "content": prompt}],
                    "max_tokens": MAX_TOKENS,
                    "temperature": 0.0,
                    "seed": 42
                });
                let headers = [
                    ("authorization", format!("Bearer {api_key}")),
                    ("content-type", "application/json".to_string()),
                ];
                // NVIDIA NIM needs longer timeout for large models
                let timeout = if self.name.starts_with("nvidia") {
                    120
                } else {
                    60
                };
                let (status, payload) = curl_post_json(&self.endpoint, &headers, &body, timeout)?;
                if !(200..300).contains(&status) {
                    return Err(AgentError::SupervisorError(format!(
                        "{} returned status {status}",
                        self.name
                    )));
                }
                parse_openai_response(&payload)
            }
        }
    }

    fn health_check(&self) -> bool {
        match self.format {
            ApiFormat::Ollama => {
                let url = self.endpoint.replace("/api/generate", "");
                let addr = url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://");
                std::net::TcpStream::connect_timeout(
                    &addr
                        .parse()
                        .unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap()),
                    Duration::from_millis(500),
                )
                .is_ok()
            }
            _ => {
                // Quick test query
                self.query_inner("Say hello").is_ok()
            }
        }
    }
}

fn parse_openai_response(payload: &serde_json::Value) -> Result<(String, u32), AgentError> {
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

// ── HTTP Helper ────────────────────────────────────────────────────────────

fn curl_post_json(
    endpoint: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
    timeout_secs: u32,
) -> Result<(u16, serde_json::Value), AgentError> {
    let marker = "__NEXUS_CLOUD_BENCH__:";
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
    let p95 = latencies[((n as f64 * 0.95) as usize).min(n - 1)];
    let p99 = latencies[((n as f64 * 0.99) as usize).min(n - 1)];

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
    errors: usize,
    latency: LatencyStats,
}

fn run_determinism_check(provider: &BenchProvider, prompt: &str, runs: usize) -> DeterminismResult {
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

    DeterminismResult {
        prompt: prompt.to_string(),
        provider: provider.display_name(),
        total_runs: runs,
        unique_outputs: output_counts.len(),
        match_rate_pct: match_rate,
        dominant_output,
        dominant_count,
        errors,
        latency: compute_latency_stats(&results),
    }
}

// ── Concurrent Latency Test ────────────────────────────────────────────────

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
                if c % 50 == 0 || c == concurrency {
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
        provider: provider.display_name(),
        latency,
        throughput_rps: throughput,
        wall_time,
    }
}

// ── Cross-Provider Stress Test ─────────────────────────────────────────────

struct CrossProviderResult {
    provider: String,
    model: String,
    agents: usize,
    unique_outputs: usize,
    match_rate_pct: f64,
    latency: LatencyStats,
}

fn run_cross_provider_stress(
    providers: &[BenchProvider],
    agents_per_provider: usize,
    prompt: &str,
) -> Vec<CrossProviderResult> {
    println!(
        "    Spawning {} total agents across {} providers...",
        agents_per_provider * providers.len(),
        providers.len()
    );

    let all_results: Arc<Mutex<HashMap<String, Vec<InferenceResult>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let total = agents_per_provider * providers.len();

    let mut handles = Vec::new();

    for provider in providers {
        let key = provider.display_name();
        all_results.lock().unwrap().insert(key.clone(), Vec::new());

        for _ in 0..agents_per_provider {
            let p = provider.clone();
            let pr = prompt.to_string();
            let res = Arc::clone(&all_results);
            let done = Arc::clone(&completed);
            let k = key.clone();
            handles.push(std::thread::spawn(move || {
                let r = p.query(&pr);
                res.lock().unwrap().get_mut(&k).unwrap().push(r);
                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                if c % 100 == 0 || c == total {
                    eprint!("\r      Progress: {c}/{total}");
                }
            }));
        }
    }

    for h in handles {
        let _ = h.join();
    }
    eprintln!();

    let results_map = all_results.lock().unwrap();
    let mut cross_results = Vec::new();

    for provider in providers {
        let key = provider.display_name();
        let results = results_map.get(&key).unwrap();
        let latency = compute_latency_stats(results);

        let mut output_counts: HashMap<String, usize> = HashMap::new();
        for r in results {
            if r.error.is_none() {
                *output_counts.entry(r.output.clone()).or_default() += 1;
            }
        }

        let successful = results.iter().filter(|r| r.error.is_none()).count();
        let dominant = output_counts.values().max().copied().unwrap_or(0);
        let match_rate = if successful > 0 {
            (dominant as f64 / successful as f64) * 100.0
        } else {
            0.0
        };

        cross_results.push(CrossProviderResult {
            provider: provider.name.clone(),
            model: provider.model.clone(),
            agents: agents_per_provider,
            unique_outputs: output_counts.len(),
            match_rate_pct: match_rate,
            latency,
        });
    }

    cross_results
}

// ── Agentic Workload Test ──────────────────────────────────────────────────

struct AgenticResult {
    provider: String,
    prompt: String,
    output: String,
    latency_ms: f64,
    is_valid_action: bool,
    error: Option<String>,
}

fn run_agentic_test(provider: &BenchProvider, prompt: &str) -> AgenticResult {
    let result = provider.query(prompt);
    let output = result.output.trim().to_string();

    // Check if the output looks like a valid single-word/short action
    let is_valid =
        result.error.is_none() && !output.is_empty() && output.split_whitespace().count() <= 5;

    AgenticResult {
        provider: provider.display_name(),
        prompt: prompt.to_string(),
        output,
        latency_ms: result.latency.as_secs_f64() * 1000.0,
        is_valid_action: is_valid,
        error: result.error,
    }
}

// ── Report Generator ───────────────────────────────────────────────────────

fn generate_report(
    available_providers: &[String],
    unavailable_providers: &[String],
    determinism_results: &[DeterminismResult],
    concurrency_results: &[ConcurrencyResult],
    cross_results: &[CrossProviderResult],
    nvidia_determinism: &[DeterminismResult],
    nvidia_concurrency: &[ConcurrencyResult],
    agentic_results: &[AgenticResult],
    wall_time: Duration,
) -> String {
    let mut r = String::new();

    r.push_str("# Nexus OS — Cloud Models Comparison Stress Test Results\n\n");
    r.push_str(&format!("**Date**: {}\n", chrono_now()));
    r.push_str(&format!(
        "**Total wall time**: {:.1}s\n",
        wall_time.as_secs_f64()
    ));
    r.push_str(&format!(
        "**Providers tested**: {} of {}\n",
        available_providers.len(),
        available_providers.len() + unavailable_providers.len()
    ));
    r.push_str(&format!("**Active**: {}\n", available_providers.join(", ")));
    if !unavailable_providers.is_empty() {
        r.push_str(&format!(
            "**Unavailable** (no API key): {}\n",
            unavailable_providers.join(", ")
        ));
    }
    r.push_str("\n---\n\n");

    // ── Executive Summary ──
    r.push_str("## Executive Summary\n\n");

    // Find fastest provider by P50
    let mut provider_latencies: HashMap<String, Vec<f64>> = HashMap::new();
    for d in determinism_results {
        provider_latencies
            .entry(d.provider.clone())
            .or_default()
            .push(d.latency.p50_ms);
    }
    let mut sorted_by_speed: Vec<(String, f64)> = provider_latencies
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    sorted_by_speed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Find most deterministic
    let mut provider_det: HashMap<String, Vec<f64>> = HashMap::new();
    for d in determinism_results {
        provider_det
            .entry(d.provider.clone())
            .or_default()
            .push(d.match_rate_pct);
    }
    let mut sorted_by_det: Vec<(String, f64)> = provider_det
        .iter()
        .map(|(k, v)| (k.clone(), v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    sorted_by_det.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    if !sorted_by_speed.is_empty() {
        r.push_str("### Speed Rankings (by avg P50 latency)\n\n");
        r.push_str("| Rank | Provider/Model | Avg P50 (ms) | Rating |\n");
        r.push_str("|------|---------------|--------------|--------|\n");
        for (i, (name, p50)) in sorted_by_speed.iter().enumerate() {
            let rating = if *p50 < 200.0 {
                "BLAZING"
            } else if *p50 < 500.0 {
                "FAST"
            } else if *p50 < 1000.0 {
                "MODERATE"
            } else if *p50 < 3000.0 {
                "SLOW"
            } else {
                "VERY SLOW"
            };
            r.push_str(&format!(
                "| {} | {} | {:.0} | {} |\n",
                i + 1,
                name,
                p50,
                rating
            ));
        }
        r.push_str("\n");
    }

    if !sorted_by_det.is_empty() {
        r.push_str("### Determinism Rankings (by avg match rate)\n\n");
        r.push_str("| Rank | Provider/Model | Avg Match Rate | Rating |\n");
        r.push_str("|------|---------------|---------------|--------|\n");
        for (i, (name, rate)) in sorted_by_det.iter().enumerate() {
            let rating = if *rate >= 100.0 {
                "PERFECT"
            } else if *rate >= 95.0 {
                "EXCELLENT"
            } else if *rate >= 80.0 {
                "GOOD"
            } else if *rate >= 50.0 {
                "FAIR"
            } else {
                "POOR"
            };
            r.push_str(&format!(
                "| {} | {} | {:.1}% | {} |\n",
                i + 1,
                name,
                rate,
                rating
            ));
        }
        r.push_str("\n");
    }

    // Best overall recommendation
    if !sorted_by_speed.is_empty() {
        r.push_str("### Recommendation for Nexus OS Agentic Workloads\n\n");

        // Score = speed_rank + determinism_rank (lower is better)
        let speed_ranks: HashMap<String, usize> = sorted_by_speed
            .iter()
            .enumerate()
            .map(|(i, (n, _))| (n.clone(), i))
            .collect();
        let det_ranks: HashMap<String, usize> = sorted_by_det
            .iter()
            .enumerate()
            .map(|(i, (n, _))| (n.clone(), i))
            .collect();

        let mut combined: Vec<(String, usize, f64, f64)> = speed_ranks
            .keys()
            .filter_map(|name| {
                let sr = speed_ranks.get(name)?;
                let dr = det_ranks.get(name)?;
                let speed = sorted_by_speed.iter().find(|(n, _)| n == name)?.1;
                let det = sorted_by_det.iter().find(|(n, _)| n == name)?.1;
                Some((name.clone(), sr + dr, speed, det))
            })
            .collect();
        combined.sort_by_key(|(_, score, _, _)| *score);

        if let Some((best, _, speed, det)) = combined.first() {
            r.push_str(&format!(
                "**WINNER: {}** — P50={:.0}ms, Determinism={:.1}%\n\n",
                best, speed, det
            ));
        }

        r.push_str("| Rank | Provider/Model | Combined Score | P50 (ms) | Determinism |\n");
        r.push_str("|------|---------------|---------------|----------|-------------|\n");
        for (i, (name, score, speed, det)) in combined.iter().take(10).enumerate() {
            r.push_str(&format!(
                "| {} | {} | {} | {:.0} | {:.1}% |\n",
                i + 1,
                name,
                score,
                speed,
                det
            ));
        }
        r.push_str("\n");
    }

    r.push_str("---\n\n");

    // ── Test 1: Determinism ──
    r.push_str("## 1. Determinism Test (10 identical prompts per model, temp=0)\n\n");
    if determinism_results.is_empty() {
        r.push_str("*No providers available for determinism testing.*\n\n");
    } else {
        r.push_str(
            "| Provider/Model | Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |\n",
        );
        r.push_str(
            "|---------------|--------|------|--------|------------|-----|-----|--------|\n",
        );
        for d in determinism_results {
            let prompt_short = if d.prompt.len() > 35 {
                format!("{}...", &d.prompt[..32])
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
        r.push_str("\n");
    }

    // ── Test 2: Latency Scaling ──
    r.push_str("## 2. Latency Scaling (concurrent agents)\n\n");
    if concurrency_results.is_empty() {
        r.push_str("*No providers available for concurrency testing.*\n\n");
    } else {
        r.push_str("| Provider/Model | Concurrency | P50 | P95 | P99 | Throughput | Errors | Wall Time |\n");
        r.push_str(
            "|---------------|-------------|-----|-----|-----|------------|--------|----------|\n",
        );
        for c in concurrency_results {
            r.push_str(&format!(
                "| {} | {} | {:.0}ms | {:.0}ms | {:.0}ms | {:.1} req/s | {:.1}% | {:.1}s |\n",
                c.provider,
                c.concurrency,
                c.latency.p50_ms,
                c.latency.p95_ms,
                c.latency.p99_ms,
                c.throughput_rps,
                c.latency.error_rate_pct,
                c.wall_time.as_secs_f64(),
            ));
        }
        r.push_str("\n");
    }

    // ── Test 3: Cross-Provider Stress ──
    r.push_str("## 3. Cross-Provider Stress Test (100 agents per provider, simultaneous)\n\n");
    if cross_results.is_empty() {
        r.push_str("*No providers available.*\n\n");
    } else {
        r.push_str("| Provider | Model | Agents | Unique Outputs | Match Rate | P50 | P95 | P99 | Errors |\n");
        r.push_str("|----------|-------|--------|----------------|------------|-----|-----|-----|--------|\n");
        for c in cross_results {
            r.push_str(&format!(
                "| {} | {} | {} | {} | {:.1}% | {:.0}ms | {:.0}ms | {:.0}ms | {:.1}% |\n",
                c.provider,
                c.model,
                c.agents,
                c.unique_outputs,
                c.match_rate_pct,
                c.latency.p50_ms,
                c.latency.p95_ms,
                c.latency.p99_ms,
                c.latency.error_rate_pct,
            ));
        }
        r.push_str("\n");
    }

    // ── Test 4: NVIDIA NIM Multi-Model ──
    r.push_str("## 4. NVIDIA NIM Multi-Model Comparison (20 representative models)\n\n");
    if nvidia_determinism.is_empty() {
        r.push_str("*NVIDIA NIM unavailable (set GROQ_API_KEY).*\n\n");
    } else {
        r.push_str("### 4a. Determinism per NVIDIA Model\n\n");
        r.push_str("| Model | Runs | Unique | Match Rate | P50 | P95 | Errors |\n");
        r.push_str("|-------|------|--------|------------|-----|-----|--------|\n");
        for d in nvidia_determinism {
            r.push_str(&format!(
                "| {} | {} | {} | {:.1}% | {:.0}ms | {:.0}ms | {} |\n",
                d.provider,
                d.total_runs,
                d.unique_outputs,
                d.match_rate_pct,
                d.latency.p50_ms,
                d.latency.p95_ms,
                d.errors,
            ));
        }
        r.push_str("\n");

        if !nvidia_concurrency.is_empty() {
            r.push_str("### 4b. Concurrency per NVIDIA Model (50 agents)\n\n");
            r.push_str("| Model | P50 | P95 | P99 | Throughput | Errors |\n");
            r.push_str("|-------|-----|-----|-----|------------|--------|\n");
            for c in nvidia_concurrency {
                r.push_str(&format!(
                    "| {} | {:.0}ms | {:.0}ms | {:.0}ms | {:.1} req/s | {:.1}% |\n",
                    c.provider,
                    c.latency.p50_ms,
                    c.latency.p95_ms,
                    c.latency.p99_ms,
                    c.throughput_rps,
                    c.latency.error_rate_pct,
                ));
            }
            r.push_str("\n");
        }
    }

    // ── Test 5: Agentic Workload ──
    r.push_str("## 5. Agentic Workload Test (decision-making prompts)\n\n");
    if agentic_results.is_empty() {
        r.push_str("*No providers available.*\n\n");
    } else {
        r.push_str("| Provider/Model | Prompt (short) | Output | Latency | Valid? |\n");
        r.push_str("|---------------|----------------|--------|---------|--------|\n");
        for a in agentic_results {
            let prompt_short = if a.prompt.len() > 40 {
                format!("{}...", &a.prompt[..37])
            } else {
                a.prompt.clone()
            };
            let output_clean = a.output.replace('\n', " ").replace('|', "\\|");
            let output_short = if output_clean.len() > 40 {
                format!("{}...", &output_clean[..37])
            } else {
                output_clean
            };
            let status = if a.error.is_some() {
                "ERROR"
            } else if a.is_valid_action {
                "YES"
            } else {
                "NOISY"
            };
            r.push_str(&format!(
                "| {} | {} | `{}` | {:.0}ms | {} |\n",
                a.provider, prompt_short, output_short, a.latency_ms, status,
            ));
        }
        r.push_str("\n");
    }

    // ── Test Configuration ──
    r.push_str("## Test Configuration\n\n");
    r.push_str(&format!(
        "- Determinism runs per prompt: {DETERMINISM_RUNS}\n"
    ));
    r.push_str(&format!("- Concurrency levels: {:?}\n", CONCURRENCY_LEVELS));
    r.push_str(&format!(
        "- Cross-provider agents per provider: {CROSS_PROVIDER_AGENTS}\n"
    ));
    r.push_str(&format!("- Max tokens per request: {MAX_TOKENS}\n"));
    r.push_str("- Temperature: 0.0 (forced deterministic)\n");
    r.push_str("- Seed: 42 (where supported)\n");
    r.push_str(&format!(
        "- Standard test prompts: {}\n",
        TEST_PROMPTS.len()
    ));
    r.push_str(&format!(
        "- Agentic test prompts: {}\n",
        AGENTIC_PROMPTS.len()
    ));
    r.push_str(&format!(
        "- NVIDIA NIM models tested: {}\n",
        NVIDIA_BENCH_MODELS.len()
    ));
    r.push_str("\n");

    // ── Provider Cost Comparison ──
    r.push_str("## Cost Comparison (per 1M tokens)\n\n");
    r.push_str("| Provider | Default Model | Cost/1M tokens |\n");
    r.push_str("|----------|--------------|----------------|\n");
    for spec in CLOUD_PROVIDERS {
        let cost_per_million = spec.cost_per_token * 1_000_000.0;
        r.push_str(&format!(
            "| {} | {} | ${:.2} |\n",
            spec.name, spec.default_model, cost_per_million,
        ));
    }
    r.push_str("| ollama | local | $0.00 (free) |\n\n");

    r
}

fn chrono_now() -> String {
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
    println!("║  NEXUS OS — Cloud Models Comparison Stress Test             ║");
    println!("║  All Providers • All Models • Full Determinism Validation   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let wall_start = Instant::now();

    // ── Discover ALL providers ──
    println!("═══ PROVIDER DISCOVERY ═══\n");

    let mut available: Vec<BenchProvider> = Vec::new();
    let mut available_names: Vec<String> = Vec::new();
    let mut unavailable_names: Vec<String> = Vec::new();

    // Check Ollama (local baseline)
    let ollama_model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:1b".to_string());
    let ollama = BenchProvider::ollama(&ollama_model);
    let ollama_ok = ollama.health_check();
    println!(
        "  ollama/{}: {}",
        ollama_model,
        if ollama_ok {
            "AVAILABLE"
        } else {
            "UNAVAILABLE"
        }
    );
    if ollama_ok {
        available.push(ollama);
        available_names.push(format!("ollama/{}", ollama_model));
    } else {
        unavailable_names.push("ollama".to_string());
    }

    // Check all cloud providers
    for spec in CLOUD_PROVIDERS {
        print!("  {}/{}: ", spec.name, spec.default_model);
        match BenchProvider::from_spec(spec) {
            Some(provider) => {
                let ok = provider.health_check();
                if ok {
                    println!("AVAILABLE");
                    available_names.push(provider.display_name());
                    available.push(provider);
                } else {
                    println!("KEY SET BUT FAILED");
                    unavailable_names.push(spec.name.to_string());
                }
            }
            None => {
                println!("NO KEY ({})", spec.env_key);
                unavailable_names.push(spec.name.to_string());
            }
        }
    }

    println!("\n  Active: {} providers", available.len());
    println!("  Missing: {} providers\n", unavailable_names.len());

    if available.is_empty() {
        eprintln!("ERROR: No providers available. Set at least one API key:");
        for spec in CLOUD_PROVIDERS {
            eprintln!("  export {}=<your-key>", spec.env_key);
        }
        eprintln!("  Or start Ollama: ollama serve && ollama pull llama3.2:1b");
        std::process::exit(1);
    }

    // ── Test 1: Determinism across all providers ──
    println!(
        "═══ TEST 1: Determinism ({} runs × {} prompts × {} providers) ═══",
        DETERMINISM_RUNS,
        TEST_PROMPTS.len(),
        available.len()
    );

    let mut determinism_results = Vec::new();
    for provider in &available {
        println!("\n  [{}/{}]", provider.name, provider.model);
        for prompt in TEST_PROMPTS {
            let result = run_determinism_check(provider, prompt, DETERMINISM_RUNS);
            println!(
                "    → {}/{} match ({:.1}%), {} unique, P50={:.0}ms",
                result.dominant_count,
                result.total_runs - result.errors,
                result.match_rate_pct,
                result.unique_outputs,
                result.latency.p50_ms,
            );
            determinism_results.push(result);
        }
    }

    // ── Test 2: Latency Scaling ──
    println!(
        "\n═══ TEST 2: Latency Scaling (concurrency: {:?}) ═══",
        CONCURRENCY_LEVELS
    );

    let mut concurrency_results = Vec::new();
    let latency_prompt = TEST_PROMPTS[0];
    for provider in &available {
        println!("\n  [{}/{}]", provider.name, provider.model);
        for &level in CONCURRENCY_LEVELS {
            print!("    {level} agents: ");
            let result = run_concurrency_test(provider, level, latency_prompt);
            println!(
                "P50={:.0}ms P95={:.0}ms P99={:.0}ms | {:.1} req/s | {:.1}% errors",
                result.latency.p50_ms,
                result.latency.p95_ms,
                result.latency.p99_ms,
                result.throughput_rps,
                result.latency.error_rate_pct,
            );
            concurrency_results.push(result);
        }
    }

    // ── Test 3: Cross-Provider Stress ──
    println!(
        "\n═══ TEST 3: Cross-Provider Stress ({} agents × {} providers simultaneously) ═══",
        CROSS_PROVIDER_AGENTS,
        available.len()
    );

    let cross_results =
        run_cross_provider_stress(&available, CROSS_PROVIDER_AGENTS, latency_prompt);
    for c in &cross_results {
        println!(
            "  {}/{}: {} unique, {:.1}% match, P50={:.0}ms, {:.1}% errors",
            c.provider,
            c.model,
            c.unique_outputs,
            c.match_rate_pct,
            c.latency.p50_ms,
            c.latency.error_rate_pct,
        );
    }

    // ── Test 4: NVIDIA NIM Multi-Model ──
    println!(
        "\n═══ TEST 4: NVIDIA NIM Multi-Model ({} models) ═══",
        NVIDIA_BENCH_MODELS.len()
    );

    let mut nvidia_determinism = Vec::new();
    let mut nvidia_concurrency = Vec::new();

    if let Ok(nvidia_key) = std::env::var("GROQ_API_KEY") {
        let nvidia_key = nvidia_key.trim().to_string();
        if !nvidia_key.is_empty() {
            for (model_id, model_name) in NVIDIA_BENCH_MODELS {
                let provider = BenchProvider::nvidia_with_model(&nvidia_key, model_id, model_name);
                print!("  {model_name}: ");

                // Quick determinism check (5 runs, single prompt)
                let det = run_determinism_check(&provider, latency_prompt, 5);
                if det.errors == 5 {
                    println!("FAILED (all errors)");
                    continue;
                }
                println!(
                    "{:.1}% match, P50={:.0}ms",
                    det.match_rate_pct, det.latency.p50_ms,
                );
                nvidia_determinism.push(det);

                // Quick concurrency check (50 agents)
                let conc = run_concurrency_test(&provider, 50, latency_prompt);
                println!(
                    "    50 agents: P50={:.0}ms P95={:.0}ms | {:.1} req/s",
                    conc.latency.p50_ms, conc.latency.p95_ms, conc.throughput_rps,
                );
                nvidia_concurrency.push(conc);
            }
        } else {
            println!("  Skipped: GROQ_API_KEY is empty");
        }
    } else {
        println!("  Skipped: GROQ_API_KEY not set");
    }

    // ── Test 5: Agentic Workload ──
    println!("\n═══ TEST 5: Agentic Workload Test ═══");

    let mut agentic_results = Vec::new();
    for provider in &available {
        println!("\n  [{}/{}]", provider.name, provider.model);
        for prompt in AGENTIC_PROMPTS {
            let result = run_agentic_test(provider, prompt);
            let status = if result.error.is_some() {
                "ERROR"
            } else if result.is_valid_action {
                "OK"
            } else {
                "NOISY"
            };
            println!(
                "    → {} | `{}` | {:.0}ms",
                status,
                if result.output.len() > 30 {
                    format!("{}...", &result.output[..27])
                } else {
                    result.output.clone()
                },
                result.latency_ms,
            );
            agentic_results.push(result);
        }
    }

    // ── Generate Report ──
    let wall_time = wall_start.elapsed();
    println!("\n═══ GENERATING REPORT ═══");

    let report = generate_report(
        &available_names,
        &unavailable_names,
        &determinism_results,
        &concurrency_results,
        &cross_results,
        &nvidia_determinism,
        &nvidia_concurrency,
        &agentic_results,
        wall_time,
    );

    let report_path = "CLOUD_MODELS_COMPARISON_RESULTS.md";
    match std::fs::write(report_path, &report) {
        Ok(_) => println!("\n  Report written to {report_path}"),
        Err(e) => eprintln!("\n  Failed to write report: {e}"),
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║  COMPLETE — {:.1}s total wall time{:>27}║",
        wall_time.as_secs_f64(),
        " "
    );
    println!(
        "║  {} providers tested, {} NVIDIA models{:>21}║",
        available.len(),
        nvidia_determinism.len(),
        " "
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}
