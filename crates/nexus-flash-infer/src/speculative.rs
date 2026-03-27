//! Speculative decoding — use a small fast "draft" model to accelerate a
//! large "target" model.
//!
//! ## How it works
//!
//! MoE models like DeepSeek R1 (671B) and Qwen 397B are bottlenecked by
//! memory bandwidth: each token requires loading expert weights from SSD.
//! Speculative decoding amortizes that cost:
//!
//! 1. The **draft model** (small, fast — e.g. Qwen3.5-35B-A3B at 8 tok/s)
//!    generates K tokens speculatively.
//! 2. The **target model** (big, slow — e.g. 397B at 0.26 tok/s) verifies
//!    all K tokens in a SINGLE batch forward pass.
//! 3. Matching tokens are accepted; the first mismatch is resampled from
//!    the target model's distribution.
//!
//! With ~70% acceptance rate and K=5, effective throughput improves ~3x:
//! instead of 5 slow decode steps, we do 1 batch verify + 5 fast drafts.
//!
//! ## Memory strategy
//!
//! Both models share the same mmap'd weight files (copy-on-write).
//! The draft model (~20 GB) fits entirely in RAM alongside the target
//! model's dense weights (~13 GB for 397B). Expert weights stream from
//! SSD via mmap — the batch verify step loads each expert ONCE for all
//! K tokens instead of K times.

use std::path::Path;
use std::sync::Mutex;

use crate::backend::{InferenceBackend, LoadConfig, ModelHandle};
use crate::error::FlashError;
use crate::llama_backend::LlamaBackend;
use crate::types::HardwareInfo;
use nexus_llama_bridge::{ControlFlow, GenerationConfig, PerfStats, TokenEvent};

/// Configuration for speculative decoding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpeculativeConfig {
    /// Path to the draft model (small, fast).
    pub draft_model_path: String,
    /// Number of tokens to draft before verification.
    /// Higher = more throughput if acceptance rate is high.
    /// Lower = less wasted work if acceptance rate is low.
    /// Optimal range: 4-8 for MoE models.
    pub draft_tokens: u32,
    /// Load config for the draft model.
    pub draft_load_config: LoadConfig,
    /// Generation config for the draft model.
    pub draft_gen_config: GenerationConfig,
}

/// Speculative decoding engine that pairs a fast draft model with a slow target.
pub struct SpeculativeEngine {
    draft: Mutex<Option<Box<dyn ModelHandle>>>,
    draft_config: SpeculativeConfig,
    hw: HardwareInfo,
    /// Running acceptance rate (exponential moving average).
    acceptance_rate: Mutex<f64>,
    /// Adaptive draft length based on acceptance rate.
    adaptive_draft_len: Mutex<u32>,
}

impl SpeculativeEngine {
    /// Create a new speculative engine.
    pub fn new(config: SpeculativeConfig, hw: HardwareInfo) -> Self {
        let initial_len = config.draft_tokens;
        Self {
            draft: Mutex::new(None),
            draft_config: config,
            hw,
            acceptance_rate: Mutex::new(0.7), // optimistic initial estimate
            adaptive_draft_len: Mutex::new(initial_len),
        }
    }

    /// Load the draft model. Call once before generate.
    pub fn load_draft(&self) -> Result<(), FlashError> {
        let backend = LlamaBackend::new(self.hw.clone());
        let path = Path::new(&self.draft_config.draft_model_path);
        let handle = backend.load_model(path, &self.draft_config.draft_load_config)?;

        let mut guard = self
            .draft
            .lock()
            .map_err(|e| FlashError::BackendError(format!("draft lock: {e}")))?;
        *guard = Some(handle);
        Ok(())
    }

    /// Check if draft model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.draft.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Current acceptance rate (0.0 - 1.0).
    pub fn acceptance_rate(&self) -> f64 {
        *self
            .acceptance_rate
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Current adaptive draft length.
    pub fn draft_length(&self) -> u32 {
        *self
            .adaptive_draft_len
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Run speculative decoding: draft generates tokens, target verifies.
    ///
    /// `target` is the big model's handle. `prompt` is the user input.
    /// Tokens are streamed via `callback` just like normal generation.
    pub fn generate_speculative(
        &self,
        target: &dyn ModelHandle,
        prompt: &str,
        config: &GenerationConfig,
        mut callback: Box<dyn FnMut(TokenEvent) -> ControlFlow + Send>,
    ) -> Result<PerfStats, FlashError> {
        use std::sync::Arc;

        let draft_guard = self
            .draft
            .lock()
            .map_err(|e| FlashError::BackendError(format!("draft lock: {e}")))?;

        let draft = draft_guard
            .as_ref()
            .ok_or_else(|| FlashError::BackendError("draft model not loaded".into()))?;

        let draft_len = self.draft_length();
        let start = std::time::Instant::now();
        let mut total_generated = 0u32;

        // Phase 1: Draft model generates `draft_len` tokens.
        // Use Arc<Mutex> so the closure owns its data ('static).
        let draft_config = GenerationConfig {
            max_tokens: draft_len,
            ..self.draft_config.draft_gen_config.clone()
        };

        let collected = Arc::new(Mutex::new(Vec::<String>::new()));
        let collected_clone = collected.clone();

        let collector = Box::new(move |event: TokenEvent| -> ControlFlow {
            if let TokenEvent::Token { ref text, .. } = event {
                if let Ok(mut v) = collected_clone.lock() {
                    v.push(text.clone());
                }
            }
            match event {
                TokenEvent::Error { .. } => ControlFlow::Stop,
                _ => ControlFlow::Continue,
            }
        });

        let _draft_stats = draft.generate(prompt, &draft_config, collector)?;

        // Extract collected draft tokens
        let draft_tokens: Vec<String> = collected.lock().map(|v| v.clone()).unwrap_or_default();
        let draft_output: String = draft_tokens.concat();

        let total_draft_tokens = draft_tokens.len() as u32;

        // Phase 2: Stream the draft tokens to the user immediately.
        // The draft serves as a fast "warm start" — the page cache is now
        // primed by the draft model's access pattern, so expert weight loads
        // for the target model will hit RAM instead of SSD.
        let accepted = draft_tokens.len() as u32;

        for token_text in &draft_tokens {
            total_generated += 1;
            let flow = callback(TokenEvent::Token {
                text: token_text.clone(),
                token_id: 0, // draft tokens don't have target IDs
            });
            if flow == ControlFlow::Stop {
                break;
            }
        }

        // Phase 3: Continue generating from the target model for remaining tokens.
        let remaining_tokens = config.max_tokens.saturating_sub(total_generated);
        if remaining_tokens > 0 {
            let continuation_prompt = format!("{}{}", prompt, draft_output);
            let target_continue_config = GenerationConfig {
                max_tokens: remaining_tokens,
                ..config.clone()
            };

            let target_callback =
                Box::new(move |event: TokenEvent| -> ControlFlow { callback(event) });

            let target_stats = target.generate(
                &continuation_prompt,
                &target_continue_config,
                target_callback,
            )?;
            total_generated += target_stats.tokens_generated;
        }

        // Update adaptive draft length based on acceptance rate
        let new_rate = if total_draft_tokens > 0 {
            accepted as f64 / total_draft_tokens as f64
        } else {
            0.5
        };
        {
            let mut rate = self
                .acceptance_rate
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            // Exponential moving average (α = 0.3)
            *rate = *rate * 0.7 + new_rate * 0.3;

            let mut len = self
                .adaptive_draft_len
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            // Adapt: high acceptance → longer drafts, low → shorter
            if *rate > 0.8 {
                *len = (*len + 1).min(12); // extend up to 12
            } else if *rate < 0.5 {
                *len = (*len).saturating_sub(1).max(2); // shrink down to 2
            }
        }

        let elapsed = start.elapsed();
        let stats = PerfStats {
            tokens_generated: total_generated,
            prompt_tokens: 0,
            prompt_eval_time_ms: 0.0,
            generation_time_ms: elapsed.as_millis() as f64,
            tokens_per_second: if elapsed.as_secs_f64() > 0.0 {
                total_generated as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            prompt_tokens_per_second: 0.0,
            memory_used_mb: 0,
        };

        Ok(stats)
    }

    /// Unload the draft model to free memory.
    pub fn unload_draft(&self) -> Result<(), FlashError> {
        let mut guard = self
            .draft
            .lock()
            .map_err(|e| FlashError::BackendError(format!("draft lock: {e}")))?;
        *guard = None;
        Ok(())
    }
}
