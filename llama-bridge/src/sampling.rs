//! Sampler chain construction from generation configuration.

use crate::error::LlamaError;
use crate::ffi;
use crate::types::GenerationConfig;

/// Build a llama sampler chain from a [`GenerationConfig`].
///
/// Samplers are applied in order: penalties → top_k → top_p → min_p → temperature.
/// Returns a pointer to the chain that must be freed with `llama_sampler_free`.
pub fn build_sampler_chain(
    config: &GenerationConfig,
) -> Result<*mut ffi::LlamaSampler, LlamaError> {
    let chain_params = unsafe { ffi::llama_sampler_chain_default_params() };
    let chain = unsafe { ffi::llama_sampler_chain_init(chain_params) };
    if chain.is_null() {
        return Err(LlamaError::BackendNotAvailable(
            "sampler chain init returned null".into(),
        ));
    }

    // Penalties
    let has_penalties = config.repeat_penalty != 1.0
        || config.frequency_penalty != 0.0
        || config.presence_penalty != 0.0;

    if has_penalties {
        let penalties = unsafe {
            ffi::llama_sampler_init_penalties(
                64, // last_n tokens to consider
                config.repeat_penalty,
                config.frequency_penalty,
                config.presence_penalty,
            )
        };
        if !penalties.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, penalties) };
        }
    }

    // Top-K
    if config.top_k > 0 {
        let top_k = unsafe { ffi::llama_sampler_init_top_k(config.top_k) };
        if !top_k.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, top_k) };
        }
    }

    // Top-P (nucleus)
    if config.top_p < 1.0 {
        let top_p = unsafe { ffi::llama_sampler_init_top_p(config.top_p, 1) };
        if !top_p.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, top_p) };
        }
    }

    // Min-P
    if config.min_p > 0.0 {
        let min_p = unsafe { ffi::llama_sampler_init_min_p(config.min_p, 1) };
        if !min_p.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, min_p) };
        }
    }

    // Temperature
    if config.temperature > 0.0 {
        let temp = unsafe { ffi::llama_sampler_init_temp(config.temperature) };
        if !temp.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, temp) };
        }
    }

    // Final token selection sampler — required by llama.cpp to actually pick a token.
    // Use dist (random sampling) when temperature > 0, greedy otherwise.
    if config.temperature > 0.0 {
        let seed = config.seed.unwrap_or(0) as u32;
        let dist = unsafe { ffi::llama_sampler_init_dist(seed) };
        if !dist.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, dist) };
        }
    } else {
        let greedy = unsafe { ffi::llama_sampler_init_greedy() };
        if !greedy.is_null() {
            unsafe { ffi::llama_sampler_chain_add(chain, greedy) };
        }
    }

    Ok(chain)
}
