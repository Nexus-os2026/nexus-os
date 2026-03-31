//! # Prompt Optimizer
//!
//! DSPy/OPRO-style prompt optimization. Generates variants, validates safety,
//! scores against benchmarks, and selects the best variant that genuinely improves
//! on the current prompt.

use crate::types::PromptVariant;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Errors from the optimizer.
#[derive(Debug, Error)]
pub enum OptimizerError {
    #[error("no valid variants could be parsed from LLM output")]
    NoValidVariants,
    #[error("all variants failed safety check: {0}")]
    AllFailedSafety(String),
    #[error("no variant exceeds improvement threshold")]
    NoImprovement,
}

/// Configuration for prompt optimization.
#[derive(Debug, Clone)]
pub struct PromptOptimizerConfig {
    /// Number of variants to generate per cycle.
    pub variants_per_cycle: usize,
    /// Minimum cosine similarity to the original prompt (0.0–1.0).
    pub min_similarity: f64,
    /// Minimum improvement over current score required (e.g. 0.05 = 5%).
    pub improvement_threshold: f64,
    /// Keywords that MUST be present in every variant.
    pub safety_keywords: Vec<String>,
    /// Maximum prompt length in characters.
    pub max_prompt_length: usize,
    /// Minimum prompt length in characters.
    pub min_prompt_length: usize,
}

impl Default for PromptOptimizerConfig {
    fn default() -> Self {
        Self {
            variants_per_cycle: 5,
            min_similarity: 0.7,
            improvement_threshold: 0.05,
            safety_keywords: vec!["governance".into(), "safety".into(), "audit".into()],
            max_prompt_length: 8192,
            min_prompt_length: 50,
        }
    }
}

/// Context about current performance used to guide optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceContext {
    pub current_score: f64,
    pub metric_history: Vec<(u64, f64)>,
    pub weaknesses: Vec<String>,
    pub optimization_history: Vec<OptimizationAttempt>,
}

/// Record of a previous optimization attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationAttempt {
    pub prompt_hash: String,
    pub score: f64,
    pub timestamp: u64,
}

/// Benchmark results for scoring a variant.
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub task_completion_rate: f64,
    pub response_quality: f64,
    pub safety_compliance: f64,
    pub efficiency: f64,
}

/// The prompt optimizer engine.
pub struct PromptOptimizer {
    config: PromptOptimizerConfig,
}

impl PromptOptimizer {
    pub fn new(config: PromptOptimizerConfig) -> Self {
        Self { config }
    }

    /// Parse LLM output into individual prompt variants and validate each one.
    pub fn generate_variants(
        &self,
        current_prompt: &str,
        llm_response: &str,
    ) -> Result<Vec<ScoredVariant>, OptimizerError> {
        let raw_variants = parse_variants(llm_response);
        if raw_variants.is_empty() {
            return Err(OptimizerError::NoValidVariants);
        }

        let mut valid = Vec::new();
        let mut safety_failures = Vec::new();

        for (i, text) in raw_variants.into_iter().enumerate() {
            // Length checks
            if text.len() < self.config.min_prompt_length {
                continue;
            }
            if text.len() > self.config.max_prompt_length {
                continue;
            }

            // Safety keyword preservation
            let lower = text.to_lowercase();
            let missing: Vec<&str> = self
                .config
                .safety_keywords
                .iter()
                .filter(|kw| !lower.contains(&kw.to_lowercase()))
                .map(|s| s.as_str())
                .collect();
            if !missing.is_empty() {
                safety_failures.push(format!("variant {i}: missing {}", missing.join(", ")));
                continue;
            }

            // Similarity check
            let similarity = cosine_similarity(current_prompt, &text);
            if similarity < self.config.min_similarity {
                continue;
            }

            valid.push(ScoredVariant {
                variant: PromptVariant {
                    variant_id: Uuid::new_v4(),
                    prompt_text: text,
                    score: 0.0,
                },
                similarity_to_original: similarity,
                safety_check_passed: true,
                generation_method: format!("variant_{i}"),
            });
        }

        if valid.is_empty() {
            if !safety_failures.is_empty() {
                return Err(OptimizerError::AllFailedSafety(safety_failures.join("; ")));
            }
            return Err(OptimizerError::NoValidVariants);
        }

        Ok(valid)
    }

    /// Score a variant against benchmark results.
    pub fn score_variant(&self, benchmark: &BenchmarkResults) -> f64 {
        // Weighted scoring
        0.40 * benchmark.task_completion_rate
            + 0.30 * benchmark.response_quality
            + 0.20 * benchmark.safety_compliance
            + 0.10 * benchmark.efficiency
    }

    /// Select the best variant that exceeds the improvement threshold.
    pub fn select_best(
        &self,
        current_score: f64,
        variants: &[(ScoredVariant, f64)],
    ) -> Option<PromptVariant> {
        let threshold = current_score * (1.0 + self.config.improvement_threshold);

        variants
            .iter()
            .filter(|(_, score)| *score > threshold)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(sv, score)| {
                let mut v = sv.variant.clone();
                v.score = *score;
                v
            })
    }

    /// Build the OPRO-style meta-prompt for generating variants.
    pub fn build_meta_prompt(&self, current_prompt: &str, context: &PerformanceContext) -> String {
        let weakness_list = if context.weaknesses.is_empty() {
            "No specific weaknesses identified.".to_string()
        } else {
            context
                .weaknesses
                .iter()
                .map(|w| format!("- {w}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let history_section = if context.optimization_history.is_empty() {
            "No previous optimization attempts.".to_string()
        } else {
            context
                .optimization_history
                .iter()
                .map(|a| format!("- hash={} score={:.3}", a.prompt_hash, a.score))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let safety_kw = self.config.safety_keywords.join(", ");

        format!(
            "You are optimizing an AI agent's system prompt for Nexus OS.\n\
             \n\
             CURRENT PROMPT:\n\
             ```\n\
             {current_prompt}\n\
             ```\n\
             \n\
             CURRENT SCORE: {:.3}\n\
             \n\
             IDENTIFIED WEAKNESSES:\n\
             {weakness_list}\n\
             \n\
             PREVIOUS ATTEMPTS:\n\
             {history_section}\n\
             \n\
             CONSTRAINTS (MANDATORY):\n\
             - The prompt MUST contain these keywords: {safety_kw}\n\
             - Do NOT remove any safety instructions, governance references, or HITL gates\n\
             - Maintain the agent's core identity and purpose\n\
             - Keep the prompt between {} and {} characters\n\
             \n\
             Generate exactly {} prompt variants, each separated by '---VARIANT---'.\n\
             Each variant should improve on a different weakness while preserving safety.",
            context.current_score,
            self.config.min_prompt_length,
            self.config.max_prompt_length,
            self.config.variants_per_cycle,
        )
    }
}

/// A variant with validation metadata.
#[derive(Debug, Clone)]
pub struct ScoredVariant {
    pub variant: PromptVariant,
    pub similarity_to_original: f64,
    pub safety_check_passed: bool,
    pub generation_method: String,
}

/// Parse LLM output into individual variants separated by `---VARIANT---`.
fn parse_variants(llm_output: &str) -> Vec<String> {
    llm_output
        .split("---VARIANT---")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Compute word-overlap cosine similarity between two texts.
/// Pure Rust, no external dependencies.
pub fn cosine_similarity(a: &str, b: &str) -> f64 {
    let freq_a = word_frequencies(a);
    let freq_b = word_frequencies(b);

    // Collect all unique words
    let mut all_words: Vec<&str> = freq_a.keys().chain(freq_b.keys()).copied().collect();
    all_words.sort_unstable();
    all_words.dedup();

    if all_words.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f64;
    let mut mag_a = 0.0_f64;
    let mut mag_b = 0.0_f64;

    for word in &all_words {
        let va = *freq_a.get(word).unwrap_or(&0) as f64;
        let vb = *freq_b.get(word).unwrap_or(&0) as f64;
        dot += va * vb;
        mag_a += va * va;
        mag_b += vb * vb;
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Count word frequencies in a text (alphanumeric tokens).
/// NOTE: operates on the borrowed slices directly (case-sensitive) to avoid
/// allocations. Callers who need case-insensitive comparison should lowercase
/// the input before calling.
fn word_frequencies(text: &str) -> HashMap<&str, u32> {
    let mut freq = HashMap::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if !word.is_empty() {
            *freq.entry(word).or_insert(0) += 1;
        }
    }
    freq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let s = cosine_similarity("hello world foo", "hello world foo");
        assert!(
            (s - 1.0).abs() < 1e-9,
            "identical texts should have similarity 1.0, got {s}"
        );
    }

    #[test]
    fn test_cosine_similarity_different() {
        let s = cosine_similarity("the quick brown fox", "quantum physics entropy");
        assert!(
            s < 0.5,
            "unrelated texts should have low similarity, got {s}"
        );
    }

    #[test]
    fn test_cosine_similarity_empty() {
        assert!((cosine_similarity("", "")).abs() < 1e-9);
        assert!((cosine_similarity("hello", "")).abs() < 1e-9);
    }

    #[test]
    fn test_cosine_similarity_partial_overlap() {
        let s = cosine_similarity("hello world foo bar", "hello world baz qux");
        assert!(
            s > 0.3 && s < 0.9,
            "partial overlap should be moderate, got {s}"
        );
    }

    #[test]
    fn test_safety_keyword_check_pass() {
        let config = PromptOptimizerConfig::default();
        let optimizer = PromptOptimizer::new(config);

        let prompt = "You are an agent with governance rules, safety constraints, and audit logging. This is a long enough prompt to pass the minimum length check for the optimizer validation system.";
        // Variants share most words with the original to pass similarity check
        let variants_text = "You are an agent with governance rules, safety constraints, and audit logging. This is an improved version of the prompt to pass the minimum length check for the optimizer validation system.---VARIANT---You are an agent with governance rules, safety constraints, and audit logging. This is a refined prompt to pass the minimum length check for the optimizer validation system.";
        let result = optimizer.generate_variants(prompt, variants_text);
        assert!(result.is_ok(), "should pass: {result:?}");
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_safety_keyword_check_fail() {
        let config = PromptOptimizerConfig::default();
        let optimizer = PromptOptimizer::new(config);

        let prompt = "Original with governance safety audit and enough length to pass the minimum character check for prompt optimizer validation.";
        // Variant missing "audit"
        let variants_text = "A variant with governance and safety but no trail logging keyword. This prompt is long enough to pass length checks but missing a required keyword.";
        let result = optimizer.generate_variants(prompt, variants_text);
        assert!(result.is_err());
    }

    #[test]
    fn test_variant_too_short_rejected() {
        let config = PromptOptimizerConfig {
            min_prompt_length: 100,
            safety_keywords: vec![],
            ..Default::default()
        };
        let optimizer = PromptOptimizer::new(config);
        let result = optimizer.generate_variants("original prompt that is long enough", "short");
        assert!(result.is_err());
    }

    #[test]
    fn test_variant_too_long_rejected() {
        let config = PromptOptimizerConfig {
            max_prompt_length: 20,
            safety_keywords: vec![],
            min_prompt_length: 1,
            ..Default::default()
        };
        let optimizer = PromptOptimizer::new(config);
        let result = optimizer.generate_variants(
            "short",
            "this is a very long variant that exceeds the maximum allowed length for the test",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_improvement_threshold_enforced() {
        let config = PromptOptimizerConfig {
            improvement_threshold: 0.10, // 10% improvement required
            ..Default::default()
        };
        let optimizer = PromptOptimizer::new(config);

        let variant = ScoredVariant {
            variant: PromptVariant {
                variant_id: Uuid::new_v4(),
                prompt_text: "test".into(),
                score: 0.0,
            },
            similarity_to_original: 0.9,
            safety_check_passed: true,
            generation_method: "test".into(),
        };

        // Lateral move: current 0.80, variant 0.82 (only 2.5% improvement)
        let result = optimizer.select_best(0.80, &[(variant.clone(), 0.82)]);
        assert!(result.is_none(), "should reject lateral move");

        // Genuine improvement: current 0.80, variant 0.90 (12.5% improvement)
        let result = optimizer.select_best(0.80, &[(variant, 0.90)]);
        assert!(result.is_some(), "should accept genuine improvement");
    }

    #[test]
    fn test_meta_prompt_includes_history() {
        let config = PromptOptimizerConfig::default();
        let optimizer = PromptOptimizer::new(config);

        let context = PerformanceContext {
            current_score: 0.75,
            metric_history: vec![],
            weaknesses: vec!["slow reasoning".into()],
            optimization_history: vec![OptimizationAttempt {
                prompt_hash: "abc123".into(),
                score: 0.70,
                timestamp: 1000,
            }],
        };

        let meta = optimizer.build_meta_prompt("You are an agent", &context);
        assert!(meta.contains("abc123"), "should include history hash");
        assert!(meta.contains("0.750"), "should include current score");
        assert!(meta.contains("slow reasoning"), "should include weaknesses");
        assert!(
            meta.contains("governance"),
            "should include safety keywords"
        );
    }

    #[test]
    fn test_score_variant_weighted() {
        let config = PromptOptimizerConfig::default();
        let optimizer = PromptOptimizer::new(config);

        let perfect = BenchmarkResults {
            task_completion_rate: 1.0,
            response_quality: 1.0,
            safety_compliance: 1.0,
            efficiency: 1.0,
        };
        assert!((optimizer.score_variant(&perfect) - 1.0).abs() < 1e-9);

        let zero = BenchmarkResults {
            task_completion_rate: 0.0,
            response_quality: 0.0,
            safety_compliance: 0.0,
            efficiency: 0.0,
        };
        assert!((optimizer.score_variant(&zero)).abs() < 1e-9);
    }

    #[test]
    fn test_parse_variants_separator() {
        let output = "Variant A text---VARIANT---Variant B text---VARIANT---Variant C text";
        let variants = parse_variants(output);
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], "Variant A text");
    }
}
