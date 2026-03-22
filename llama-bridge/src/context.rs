//! Inference context — wraps `llama_context` and drives the generate loop.

use std::ptr;

use tracing::{debug, warn};

use crate::batch;
use crate::chat_template;
use crate::error::LlamaError;
use crate::ffi;
use crate::model::LlamaModel;
use crate::sampling;
use crate::tokenizer;
use crate::types::{ControlFlow, GenerationConfig, PerfStats, TokenEvent};

/// An inference context bound to a loaded model.
///
/// Owns the underlying `llama_context` and sampler chain. Freed on drop.
pub struct LlamaContext {
    ctx: *mut ffi::LlamaContextRaw,
    sampler: *mut ffi::LlamaSampler,
    vocab: *const ffi::LlamaVocab,
    model_ptr: *const ffi::LlamaModel,
    architecture: String,
    n_ctx: u32,
}

// The context is not thread-safe (single decode stream), but can be moved
// between threads.
unsafe impl Send for LlamaContext {}

impl LlamaContext {
    /// Create a new inference context from a loaded model.
    pub fn new(model: &LlamaModel, config: &GenerationConfig) -> Result<Self, LlamaError> {
        let params = unsafe { ffi::nexus_ctx_params_create() };
        if params.is_null() {
            return Err(LlamaError::ContextCreationFailed(
                "failed to allocate context params".into(),
            ));
        }

        // Thread count: use config override or auto-detect
        let n_threads = config.n_threads.map(|n| n as i32).unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|p| p.get() as i32)
                .unwrap_or(4)
        });

        unsafe {
            ffi::nexus_ctx_params_set_n_ctx(params, config.n_ctx);
            ffi::nexus_ctx_params_set_n_batch(params, config.n_batch);
            if config.n_ubatch > 0 {
                ffi::nexus_ctx_params_set_n_ubatch(params, config.n_ubatch);
            }
            ffi::nexus_ctx_params_set_n_threads(params, n_threads);
            ffi::nexus_ctx_params_set_n_threads_batch(params, n_threads);
            ffi::nexus_ctx_params_set_flash_attn(params, config.flash_attn);
            ffi::nexus_ctx_params_set_no_perf(params, false);
            if let Some(type_k) = config.type_k {
                ffi::nexus_ctx_params_set_type_k(params, type_k.as_ggml_type());
            }
            if let Some(type_v) = config.type_v {
                ffi::nexus_ctx_params_set_type_v(params, type_v.as_ggml_type());
            }
        }

        let ctx = unsafe { ffi::nexus_init_from_model(model.as_mut_ptr(), params) };
        unsafe { ffi::nexus_ctx_params_free(params) };
        if ctx.is_null() {
            return Err(LlamaError::ContextCreationFailed(
                "llama_init_from_model returned null".into(),
            ));
        }

        let sampler = sampling::build_sampler_chain(config)?;
        let vocab = model.vocab();
        let model_ptr = model.as_mut_ptr() as *const ffi::LlamaModel;
        let architecture = model.metadata().architecture.clone();
        let n_ctx = unsafe { ffi::llama_n_ctx(ctx) };

        Ok(Self {
            ctx,
            sampler,
            vocab,
            model_ptr,
            architecture,
            n_ctx,
        })
    }

    /// Context window size in tokens.
    pub fn context_size(&self) -> u32 {
        self.n_ctx
    }

    /// Run synchronous text generation.
    ///
    /// Calls `callback` for each generated token. The callback can return
    /// [`ControlFlow::Stop`] to halt generation early.
    pub fn generate_sync(
        &mut self,
        prompt: &str,
        config: &GenerationConfig,
        mut callback: impl FnMut(TokenEvent) -> ControlFlow,
    ) -> Result<PerfStats, LlamaError> {
        // Apply chat template to format the prompt for the model's expected format.
        // Without this, models like DeepSeek-R1 treat raw text as continuation
        // and produce garbage repetition instead of following instructions.
        let formatted_prompt =
            chat_template::apply_chat_template(self.model_ptr, &self.architecture, prompt);

        // Tokenize the formatted prompt (add_special=false since the template
        // already includes BOS/special tokens)
        let prompt_tokens = tokenizer::tokenize(self.vocab, &formatted_prompt, false)?;
        let prompt_token_count = prompt_tokens.len() as u32;

        if prompt_tokens.is_empty() {
            return Err(LlamaError::TokenizationFailed(
                "prompt produced zero tokens".into(),
            ));
        }

        if prompt_tokens.len() as u32 >= self.n_ctx {
            return Err(LlamaError::InvalidConfig(format!(
                "prompt ({} tokens) exceeds context window ({} tokens)",
                prompt_tokens.len(),
                self.n_ctx
            )));
        }

        debug!(
            prompt_tokens = prompt_tokens.len(),
            max_tokens = config.max_tokens,
            "starting generation"
        );

        // Clear KV cache and reset sampler so each generation starts fresh.
        // Without this, positions accumulate across multi-turn calls and
        // eventually exceed the context window, causing decode to return -1.
        let mem = unsafe { ffi::llama_get_memory(self.ctx) };
        if !mem.is_null() {
            unsafe { ffi::llama_memory_clear(mem, false) };
        }
        unsafe { ffi::llama_sampler_reset(self.sampler) };

        // Reset perf counters
        unsafe { ffi::llama_perf_context_reset(self.ctx) };

        // Process prompt
        let mut prompt_batch = batch::create_batch(prompt_tokens.len() as i32);
        unsafe { batch::fill_batch_prompt(&mut prompt_batch, &prompt_tokens) };

        let ret = unsafe { ffi::llama_decode(self.ctx, prompt_batch) };
        // Do not free prompt_batch here — llama_decode consumed it.
        // The batch memory was allocated by llama_batch_init and the
        // C side manages it. We call free after we're done.
        if ret != 0 {
            batch::free_batch(prompt_batch);
            return Err(LlamaError::DecodeFailed(ret));
        }
        batch::free_batch(prompt_batch);

        // Generation loop
        let eos = tokenizer::eos_token(self.vocab);
        let mut generated_text = String::new();
        let mut tokens_generated = 0u32;
        let mut pos = prompt_tokens.len() as i32;
        let mut gen_batch = batch::create_batch(1);

        for _ in 0..config.max_tokens {
            // Sample next token
            let token = unsafe { ffi::llama_sampler_sample(self.sampler, self.ctx, -1) };

            // Check for EOS
            if token == eos || token < 0 {
                break;
            }

            // Decode token to text
            let piece = tokenizer::token_to_text(self.vocab, token);

            // Check for stop sequences
            generated_text.push_str(&piece);
            let should_stop = config
                .stop_sequences
                .iter()
                .any(|s| generated_text.ends_with(s.as_str()));

            // Emit token event
            let flow = callback(TokenEvent::Token {
                text: piece,
                token_id: token,
            });

            tokens_generated += 1;

            if should_stop || flow == ControlFlow::Stop {
                break;
            }

            // Check context window boundary before decoding next token.
            // pos is the next position we'd write to; n_ctx is the limit.
            if (pos + 1) as u32 >= self.n_ctx {
                debug!(
                    pos,
                    n_ctx = self.n_ctx,
                    "context window full, stopping generation"
                );
                break;
            }

            // Prepare next decode
            unsafe { batch::fill_batch_single(&mut gen_batch, token, pos) };
            let ret = unsafe { ffi::llama_decode(self.ctx, gen_batch) };
            if ret != 0 {
                warn!(
                    code = ret,
                    pos,
                    n_ctx = self.n_ctx,
                    "decode failed during generation"
                );
                callback(TokenEvent::Error {
                    message: format!("decode failed with code {ret}"),
                });
                break;
            }

            pos += 1;
        }

        batch::free_batch(gen_batch);

        // Collect performance stats
        let perf = unsafe { ffi::llama_perf_context(self.ctx) };
        let stats = PerfStats {
            tokens_generated,
            prompt_tokens: prompt_token_count,
            prompt_eval_time_ms: perf.t_p_eval_ms,
            generation_time_ms: perf.t_eval_ms,
            tokens_per_second: if perf.t_eval_ms > 0.0 {
                (perf.n_eval as f64 / perf.t_eval_ms) * 1000.0
            } else {
                0.0
            },
            prompt_tokens_per_second: if perf.t_p_eval_ms > 0.0 {
                (perf.n_p_eval as f64 / perf.t_p_eval_ms) * 1000.0
            } else {
                0.0
            },
            memory_used_mb: 0, // filled by caller if needed
        };

        callback(TokenEvent::Done {
            stats: stats.clone(),
        });

        Ok(stats)
    }
}

impl Drop for LlamaContext {
    fn drop(&mut self) {
        if !self.sampler.is_null() {
            unsafe { ffi::llama_sampler_free(self.sampler) };
            self.sampler = ptr::null_mut();
        }
        if !self.ctx.is_null() {
            unsafe { ffi::llama_free(self.ctx) };
            self.ctx = ptr::null_mut();
        }
    }
}
