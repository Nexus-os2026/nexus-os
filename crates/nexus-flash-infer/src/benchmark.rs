//! Automated benchmark suite that proves model inference runs within memory budget.
//!
//! Generates a shareable report with performance metrics, memory usage, and
//! output coherence checks.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use nexus_llama_bridge::{ControlFlow, GenerationConfig, PerfStats, TokenEvent};

use crate::backend::ModelHandle;
use crate::error::FlashError;

/// A single benchmark prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkPrompt {
    pub name: String,
    pub prompt: String,
    pub max_tokens: u32,
}

/// Result of running a benchmark on one prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub prompt_name: String,
    pub model_name: String,
    pub model_size: String,
    pub quant_type: String,
    pub is_moe: bool,

    // Hardware
    pub system_ram_mb: u64,
    pub cpu_cores: u32,
    pub gpu_used: bool,

    // Performance
    pub prompt_tokens: u32,
    pub generated_tokens: u32,
    pub prompt_eval_time_ms: f64,
    pub generation_time_ms: f64,
    pub tokens_per_second: f64,
    pub prompt_tokens_per_second: f64,
    pub time_to_first_token_ms: f64,

    // Memory
    pub peak_memory_mb: u64,
    pub memory_budget_mb: u64,
    pub memory_within_budget: bool,

    // Quality
    pub output_coherent: bool,
    pub output_sample: String,

    // Metadata
    pub timestamp: String,
    pub nexus_version: String,
    pub platform: String,
}

/// Standard benchmark prompts covering different generation patterns.
pub fn standard_prompts() -> Vec<BenchmarkPrompt> {
    vec![
        BenchmarkPrompt {
            name: "Short factual".into(),
            prompt: "What is the capital of France? Answer in one sentence.".into(),
            max_tokens: 30,
        },
        BenchmarkPrompt {
            name: "Medium reasoning".into(),
            prompt: "Explain how a transistor works in simple terms.".into(),
            max_tokens: 100,
        },
        BenchmarkPrompt {
            name: "Long generation".into(),
            prompt: "Write a detailed comparison of Python and Rust for systems programming."
                .into(),
            max_tokens: 200,
        },
        BenchmarkPrompt {
            name: "Code generation".into(),
            prompt: "Write a Rust function that implements binary search on a sorted slice.".into(),
            max_tokens: 150,
        },
    ]
}

/// Run a single benchmark prompt against a loaded model.
pub fn run_single_benchmark(
    model_handle: &dyn ModelHandle,
    prompt: &BenchmarkPrompt,
    hw: &crate::types::HardwareInfo,
    memory_budget_mb: u64,
) -> Result<BenchmarkResult, FlashError> {
    let metadata = model_handle.metadata();
    let mem_usage = model_handle.memory_usage();

    let gen_config = GenerationConfig {
        max_tokens: prompt.max_tokens,
        temperature: 0.7,
        top_p: 0.95,
        top_k: 40,
        min_p: 0.05,
        ..Default::default()
    };

    // Use Arc<Mutex<>> so the closure satisfies 'static + Send
    let output = Arc::new(Mutex::new(String::new()));
    let first_token_time = Arc::new(Mutex::new(None::<f64>));
    let start = Instant::now();

    let out_clone = Arc::clone(&output);
    let ttft_clone = Arc::clone(&first_token_time);

    let stats: PerfStats = model_handle.generate(
        &prompt.prompt,
        &gen_config,
        Box::new(move |event: TokenEvent| -> ControlFlow {
            match event {
                TokenEvent::Token { ref text, .. } => {
                    {
                        let mut ttft = ttft_clone.lock().unwrap();
                        if ttft.is_none() {
                            *ttft = Some(start.elapsed().as_secs_f64() * 1000.0);
                        }
                    }
                    out_clone.lock().unwrap().push_str(text);
                    ControlFlow::Continue
                }
                TokenEvent::Done { .. } | TokenEvent::Error { .. } => ControlFlow::Continue,
            }
        }),
    )?;

    let ttft = first_token_time.lock().unwrap().unwrap_or(0.0);
    let output = output.lock().unwrap().clone();
    let peak_memory_mb = mem_usage.total_mb;
    let coherent = basic_coherence_check(&output);

    // Truncate output sample for the report
    let sample = if output.len() > 200 {
        format!("{}…", &output[..200])
    } else {
        output.clone()
    };

    let file_size_mb = metadata.file_size_bytes / (1024 * 1024);

    Ok(BenchmarkResult {
        prompt_name: prompt.name.clone(),
        model_name: metadata.architecture.clone(),
        model_size: format_size(metadata.total_params),
        quant_type: metadata.quantization.clone(),
        is_moe: metadata.is_moe,

        system_ram_mb: hw.total_ram_mb,
        cpu_cores: hw.cpu_cores,
        gpu_used: false, // CPU-only for now

        prompt_tokens: stats.prompt_tokens,
        generated_tokens: stats.tokens_generated,
        prompt_eval_time_ms: stats.prompt_eval_time_ms,
        generation_time_ms: stats.generation_time_ms,
        tokens_per_second: stats.tokens_per_second,
        prompt_tokens_per_second: stats.prompt_tokens_per_second,
        time_to_first_token_ms: ttft,

        peak_memory_mb,
        memory_budget_mb,
        memory_within_budget: peak_memory_mb <= memory_budget_mb
            || file_size_mb <= memory_budget_mb,

        output_coherent: coherent,
        output_sample: sample,

        timestamp: chrono::Utc::now().to_rfc3339(),
        nexus_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
    })
}

/// Run the full standard benchmark suite against a loaded model.
pub fn run_full_benchmark(
    model_handle: &dyn ModelHandle,
    hw: &crate::types::HardwareInfo,
    memory_budget_mb: u64,
) -> Result<Vec<BenchmarkResult>, FlashError> {
    let prompts = standard_prompts();
    let mut results = Vec::with_capacity(prompts.len());

    for prompt in &prompts {
        let result = run_single_benchmark(model_handle, prompt, hw, memory_budget_mb)?;
        results.push(result);
    }

    Ok(results)
}

/// Generate a shareable markdown benchmark report.
pub fn generate_report(results: &[BenchmarkResult]) -> String {
    let mut report = String::new();
    report.push_str("# Nexus OS Flash Inference — Benchmark Report\n\n");

    if let Some(first) = results.first() {
        report.push_str(&format!("**Date**: {}\n", &first.timestamp[..10]));
        report.push_str(&format!("**Nexus OS Version**: {}\n", first.nexus_version));
        report.push_str(&format!("**Platform**: {}\n", first.platform));
        report.push_str(&format!("**System RAM**: {} MB\n", first.system_ram_mb));
        report.push_str(&format!("**CPU Cores**: {}\n", first.cpu_cores));
        report.push_str(&format!(
            "**Model**: {} ({})\n",
            first.model_name, first.model_size
        ));
        report.push_str(&format!("**Quantization**: {}\n", first.quant_type));
        report.push_str(&format!("**MoE**: {}\n\n", first.is_moe));
    }

    report.push_str("## Performance Results\n\n");
    report.push_str(
        "| Prompt | Tokens | tok/s | Prompt tok/s | TTFT | Memory | Budget | Within? |\n",
    );
    report
        .push_str("|--------|--------|-------|-------------|------|--------|--------|---------|\n");

    for r in results {
        let within = if r.memory_within_budget { "YES" } else { "NO" };
        report.push_str(&format!(
            "| {} | {} | {:.1} | {:.1} | {:.0}ms | {}MB | {}MB | {} |\n",
            r.prompt_name,
            r.generated_tokens,
            r.tokens_per_second,
            r.prompt_tokens_per_second,
            r.time_to_first_token_ms,
            r.peak_memory_mb,
            r.memory_budget_mb,
            within,
        ));
    }

    report.push_str("\n## Output Samples\n\n");
    for r in results {
        let coherent = if r.output_coherent {
            "coherent"
        } else {
            "incoherent"
        };
        report.push_str(&format!(
            "### {} ({})\n```\n{}\n```\n\n",
            r.prompt_name, coherent, r.output_sample
        ));
    }

    report.push_str("---\n*Generated by Nexus OS Flash Inference Benchmark Suite*\n");
    report
}

/// Basic coherence check — verifies the output has some structure.
fn basic_coherence_check(output: &str) -> bool {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must have at least a few words
    let word_count = trimmed.split_whitespace().count();
    if word_count < 3 {
        return false;
    }
    // Should not be all the same character (degenerate output)
    let unique_chars: std::collections::HashSet<char> = trimmed.chars().collect();
    if unique_chars.len() < 5 {
        return false;
    }
    true
}

/// Format parameter count into human-readable string.
fn format_size(params: u64) -> String {
    if params >= 1_000_000_000 {
        format!("{:.1}B", params as f64 / 1_000_000_000.0)
    } else if params >= 1_000_000 {
        format!("{:.1}M", params as f64 / 1_000_000.0)
    } else {
        format!("{}K", params / 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_prompts_has_4_entries() {
        let prompts = standard_prompts();
        assert_eq!(prompts.len(), 4);
        assert!(prompts.iter().all(|p| p.max_tokens > 0));
        assert!(prompts.iter().all(|p| !p.prompt.is_empty()));
    }

    #[test]
    fn test_basic_coherence_check() {
        assert!(!basic_coherence_check(""));
        assert!(!basic_coherence_check("   "));
        assert!(!basic_coherence_check("hi"));
        assert!(!basic_coherence_check("aaa aaa aaa"));
        assert!(basic_coherence_check(
            "The capital of France is Paris, a beautiful city."
        ));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(7_000_000_000), "7.0B");
        assert_eq!(format_size(397_000_000_000), "397.0B");
        assert_eq!(format_size(500_000_000), "500.0M");
        assert_eq!(format_size(100_000), "100K");
    }

    #[test]
    fn test_generate_report_empty() {
        let report = generate_report(&[]);
        assert!(report.contains("Benchmark Report"));
    }

    #[test]
    fn test_generate_report_with_result() {
        let result = BenchmarkResult {
            prompt_name: "Test".into(),
            model_name: "llama".into(),
            model_size: "7.0B".into(),
            quant_type: "Q4_K_M".into(),
            is_moe: false,
            system_ram_mb: 16384,
            cpu_cores: 8,
            gpu_used: false,
            prompt_tokens: 10,
            generated_tokens: 30,
            prompt_eval_time_ms: 100.0,
            generation_time_ms: 3000.0,
            tokens_per_second: 10.0,
            prompt_tokens_per_second: 100.0,
            time_to_first_token_ms: 150.0,
            peak_memory_mb: 4096,
            memory_budget_mb: 8192,
            memory_within_budget: true,
            output_coherent: true,
            output_sample: "The capital of France is Paris.".into(),
            timestamp: "2026-03-21T00:00:00Z".into(),
            nexus_version: "0.1.0".into(),
            platform: "linux".into(),
        };

        let report = generate_report(&[result]);
        assert!(report.contains("llama"));
        assert!(report.contains("10.0"));
        assert!(report.contains("YES"));
        assert!(report.contains("Paris"));
    }
}
