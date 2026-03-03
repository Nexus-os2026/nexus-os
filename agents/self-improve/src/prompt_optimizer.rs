use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptOutcome {
    pub prompt: String,
    pub success: bool,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptVariant {
    pub id: u64,
    pub prompt: String,
    pub attempts: u64,
    pub successes: u64,
    pub average_score: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptOptimizer {
    variants_by_base: HashMap<String, Vec<PromptVariant>>,
    default_prompt_by_base: HashMap<String, String>,
    next_variant_id: u64,
}

impl PromptOptimizer {
    pub fn new() -> Self {
        Self {
            variants_by_base: HashMap::new(),
            default_prompt_by_base: HashMap::new(),
            next_variant_id: 1,
        }
    }

    pub fn optimize_prompt(&mut self, base_prompt: &str, outcomes: &[PromptOutcome]) -> String {
        self.ensure_base_variant(base_prompt);

        for outcome in outcomes {
            self.record_outcome(
                base_prompt,
                outcome.prompt.as_str(),
                outcome.success,
                outcome.score,
            );
        }

        let Some(variants) = self.variants_by_base.get(base_prompt) else {
            return base_prompt.to_string();
        };

        let best = variants
            .iter()
            .max_by(|left, right| {
                let left_rate = success_rate(left);
                let right_rate = success_rate(right);
                left_rate
                    .partial_cmp(&right_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        left.average_score
                            .partial_cmp(&right.average_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| left.attempts.cmp(&right.attempts))
            })
            .map(|variant| variant.prompt.clone())
            .unwrap_or_else(|| base_prompt.to_string());

        self.default_prompt_by_base
            .insert(base_prompt.to_string(), best.clone());
        best
    }

    pub fn default_prompt(&self, base_prompt: &str) -> Option<&str> {
        self.default_prompt_by_base
            .get(base_prompt)
            .map(|value| value.as_str())
    }

    pub fn set_default_prompt(&mut self, base_prompt: &str, prompt: &str) {
        self.ensure_base_variant(base_prompt);
        self.record_outcome(base_prompt, prompt, true, 1.0);
        self.default_prompt_by_base
            .insert(base_prompt.to_string(), prompt.to_string());
    }

    pub fn variants_for(&self, base_prompt: &str) -> Vec<PromptVariant> {
        self.variants_by_base
            .get(base_prompt)
            .cloned()
            .unwrap_or_default()
    }

    pub fn context_hints(task_type: &str) -> Vec<&'static str> {
        match task_type.to_ascii_lowercase().as_str() {
            "coding" => vec![
                "include failing tests and exact errors",
                "include file paths and architectural constraints",
                "include style conventions from the repository",
            ],
            "posting" => vec![
                "include platform character limits",
                "include audience and brand tone",
                "include recent engagement signals",
            ],
            "website" => vec![
                "include target mood and typography direction",
                "include responsive breakpoints",
                "include performance constraints",
            ],
            _ => vec!["include objective, constraints, and success criteria"],
        }
    }

    fn record_outcome(&mut self, base_prompt: &str, prompt: &str, success: bool, score: f64) {
        let variants = self
            .variants_by_base
            .entry(base_prompt.to_string())
            .or_default();

        let index = variants.iter().position(|variant| variant.prompt == prompt);
        let idx = if let Some(index) = index {
            index
        } else {
            let id = self.next_variant_id;
            self.next_variant_id = self.next_variant_id.saturating_add(1);
            variants.push(PromptVariant {
                id,
                prompt: prompt.to_string(),
                attempts: 0,
                successes: 0,
                average_score: 0.0,
            });
            variants.len().saturating_sub(1)
        };

        let variant = &mut variants[idx];
        variant.attempts = variant.attempts.saturating_add(1);
        if success {
            variant.successes = variant.successes.saturating_add(1);
        }
        let attempts = variant.attempts as f64;
        variant.average_score = ((variant.average_score * (attempts - 1.0)) + score) / attempts;
    }

    fn ensure_base_variant(&mut self, base_prompt: &str) {
        let variants = self
            .variants_by_base
            .entry(base_prompt.to_string())
            .or_default();
        if variants.iter().any(|variant| variant.prompt == base_prompt) {
            return;
        }
        let id = self.next_variant_id;
        self.next_variant_id = self.next_variant_id.saturating_add(1);
        variants.push(PromptVariant {
            id,
            prompt: base_prompt.to_string(),
            attempts: 1,
            successes: 0,
            average_score: 0.0,
        });
    }
}

pub fn optimize_prompt(
    optimizer: &mut PromptOptimizer,
    base_prompt: &str,
    outcomes: &[PromptOutcome],
) -> String {
    optimizer.optimize_prompt(base_prompt, outcomes)
}

fn success_rate(variant: &PromptVariant) -> f64 {
    if variant.attempts == 0 {
        return 0.0;
    }
    variant.successes as f64 / variant.attempts as f64
}
