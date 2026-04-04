//! Local SLM provider using candle for on-device inference.
//!
//! This module is only compiled when the `local-slm` feature flag is enabled.
//! It implements the `LlmProvider` trait so it can be plugged into the
//! `ProviderRouter` alongside cloud providers.
//!
//! ## KV Cache / Incremental Inference
//!
//! The model architecture is a pure MLP stack (embedding → gate_proj/up_proj/
//! down_proj layers with SiLU activation → lm_head) with no attention mechanism. Each token is processed
//! independently through the layers, which means we can skip reprocessing
//! already-seen tokens during autoregressive generation.
//!
//! [`KvCache`] tracks per-layer hidden-state history so the generation loop
//! only runs the *new* token through the forward pass each step, reducing
//! total work from O(N·T) to O(N + T).

use super::{LlmProvider, LlmResponse};
use crate::model_registry::{LoadedModel, ModelRegistry};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use candle_core::{DType, Tensor};

/// Cache for incremental inference through the MLP stack.
///
/// For this position-independent MLP architecture (no attention), the cache
/// stores per-layer key and value tensors that allow the forward pass to
/// process only newly-appended tokens instead of the full sequence.
///
/// In a traditional transformer this would hold attention K/V projections.
/// Here it holds the hidden states entering/exiting each MLP layer so that
/// the final LM-head projection can be computed on just the new position.
#[derive(Debug)]
pub struct KvCache {
    /// Per-layer key tensors.  `keys[i]` has shape `[cached_seq_len, dim]`.
    keys: Vec<Option<Tensor>>,
    /// Per-layer value tensors.  `values[i]` has shape `[cached_seq_len, dim]`.
    values: Vec<Option<Tensor>>,
}

impl KvCache {
    /// Create a new empty cache for `num_layers` layers.
    pub fn new(num_layers: usize) -> Self {
        Self {
            keys: (0..num_layers).map(|_| None).collect(),
            values: (0..num_layers).map(|_| None).collect(),
        }
    }

    /// Return cached (key, value) for `layer_idx`, if present.
    pub fn get(&self, layer_idx: usize) -> Option<(&Tensor, &Tensor)> {
        let k = self.keys.get(layer_idx)?.as_ref()?;
        let v = self.values.get(layer_idx)?.as_ref()?;
        Some((k, v))
    }

    /// Append `new_key` / `new_value` to the cache for `layer_idx`.
    ///
    /// If the layer already has cached tensors, the new tensors are
    /// concatenated along dimension 0 (the sequence-length axis).
    /// Returns the full `(all_keys, all_values)` after concatenation
    /// and stores the updated tensors back in the cache.
    pub fn update(
        &mut self,
        layer_idx: usize,
        new_key: Tensor,
        new_value: Tensor,
    ) -> Result<(Tensor, Tensor), candle_core::Error> {
        // Grow vectors if needed (models may have more layers than initially guessed).
        while self.keys.len() <= layer_idx {
            self.keys.push(None);
            self.values.push(None);
        }

        let full_k = match self.keys[layer_idx].take() {
            Some(prev) => Tensor::cat(&[&prev, &new_key], 0)?,
            None => new_key,
        };
        let full_v = match self.values[layer_idx].take() {
            Some(prev) => Tensor::cat(&[&prev, &new_value], 0)?,
            None => new_value,
        };

        self.keys[layer_idx] = Some(full_k.clone());
        self.values[layer_idx] = Some(full_v.clone());

        Ok((full_k, full_v))
    }

    /// Clear all cached entries back to `None`.
    pub fn reset(&mut self) {
        for slot in &mut self.keys {
            *slot = None;
        }
        for slot in &mut self.values {
            *slot = None;
        }
    }

    /// Current cached sequence length (from the first non-`None` layer).
    pub fn seq_len(&self) -> usize {
        for t in self.keys.iter().flatten() {
            let dims = t.dims();
            if !dims.is_empty() {
                return dims[0];
            }
        }
        0
    }
}

/// Sampling configuration for text generation.
#[derive(Debug, Clone)]
pub struct SamplingConfig {
    /// Temperature for softmax sampling. 0.0 = greedy (argmax).
    pub temperature: f64,
    /// Top-p (nucleus) sampling cutoff. 1.0 = disabled.
    pub top_p: f64,
    /// Repetition penalty applied to already-generated tokens.
    pub repetition_penalty: f32,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            repetition_penalty: 1.1,
        }
    }
}

/// Configuration for speculative decoding.
///
/// When enabled, a smaller *draft* model generates candidate tokens cheaply
/// and the full *target* model verifies them in a batched forward pass.
/// Tokens where the target agrees with the draft are accepted, yielding
/// multiple accepted tokens per target forward pass.
#[derive(Clone, Debug)]
pub struct SpeculativeConfig {
    /// How many candidate tokens the draft model generates per round.
    pub draft_steps: usize,
    /// Minimum target-to-draft probability ratio to accept a draft token.
    pub acceptance_threshold: f32,
    /// Master toggle — speculative decoding is off by default.
    pub enabled: bool,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            draft_steps: 4,
            acceptance_threshold: 0.1,
            enabled: false,
        }
    }
}

/// Statistics collected during a speculative decoding run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpeculativeStats {
    /// Total draft tokens proposed across all rounds.
    pub total_draft_tokens: usize,
    /// Draft tokens accepted by the target model.
    pub accepted_tokens: usize,
    /// Draft tokens rejected by the target model.
    pub rejected_tokens: usize,
    /// `accepted / total_draft` (0.0 if no drafts).
    pub acceptance_rate: f32,
    /// Number of target-model forward passes (verify rounds).
    pub target_forward_passes: usize,
    /// `accepted / target_forward_passes` — effective throughput multiplier.
    pub effective_tokens_per_pass: f32,
}

/// Local small language model provider backed by candle.
///
/// When a model is loaded via `load_model()`, inference runs entirely
/// on-device using candle-core. No network calls are made.
///
/// The active model is held behind `Arc<RwLock<...>>` so the provider
/// is `Send + Sync` and can serve concurrent `query()` calls.
pub struct LocalSlmProvider {
    /// Model registry for discovering and loading models.
    registry: RwLock<ModelRegistry>,
    /// Currently active model for inference (the *target* model).
    active_model: RwLock<Option<Arc<LoadedModel>>>,
    /// Optional smaller *draft* model for speculative decoding.
    draft_model: RwLock<Option<Arc<LoadedModel>>>,
    /// Sampling configuration.
    sampling: RwLock<SamplingConfig>,
    /// Speculative decoding configuration.
    speculative_config: RwLock<SpeculativeConfig>,
}

impl std::fmt::Debug for LocalSlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalSlmProvider")
            .field("has_model", &self.is_available())
            .field("has_draft", &self.has_draft_model())
            .field(
                "sampling",
                &*self.sampling.read().unwrap_or_else(|e| e.into_inner()),
            )
            .field(
                "speculative",
                &*self
                    .speculative_config
                    .read()
                    .unwrap_or_else(|e| e.into_inner()),
            )
            .finish()
    }
}

impl LocalSlmProvider {
    /// Create a new provider with the given model registry.
    pub fn new(registry: ModelRegistry) -> Self {
        Self {
            registry: RwLock::new(registry),
            active_model: RwLock::new(None),
            draft_model: RwLock::new(None),
            sampling: RwLock::new(SamplingConfig::default()),
            speculative_config: RwLock::new(SpeculativeConfig::default()),
        }
    }

    /// Create a provider using the default model directory.
    pub fn with_defaults() -> Self {
        Self::new(ModelRegistry::default_dir())
    }

    /// Set the sampling temperature.
    pub fn set_temperature(&self, temperature: f64) {
        if let Ok(mut s) = self.sampling.write() {
            s.temperature = temperature;
        }
    }

    /// Set top-p (nucleus) sampling cutoff.
    pub fn set_top_p(&self, top_p: f64) {
        if let Ok(mut s) = self.sampling.write() {
            s.top_p = top_p;
        }
    }

    /// Set repetition penalty.
    pub fn set_repetition_penalty(&self, penalty: f32) {
        if let Ok(mut s) = self.sampling.write() {
            s.repetition_penalty = penalty;
        }
    }

    /// Get the current sampling configuration.
    pub fn sampling_config(&self) -> SamplingConfig {
        self.sampling.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// Whether a model is loaded and ready for inference.
    pub fn is_available(&self) -> bool {
        self.active_model
            .read()
            .map(|m| m.is_some())
            .unwrap_or(false)
    }

    /// Load a model by ID and make it the active model for inference.
    ///
    /// Discovers models if not already discovered, then loads the specified
    /// model via the registry.
    pub fn load_model(&self, model_id: &str) -> Result<(), String> {
        let mut reg = self
            .registry
            .write()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;

        if reg.available_models().is_empty() {
            reg.discover();
        }

        let loaded = reg.load(model_id)?;

        let mut active = self
            .active_model
            .write()
            .map_err(|e| format!("active_model lock poisoned: {e}"))?;
        *active = Some(loaded);
        Ok(())
    }

    /// Unload the active model, freeing memory.
    pub fn unload_model(&self) -> bool {
        if let Ok(mut active) = self.active_model.write() {
            if active.is_some() {
                *active = None;
                return true;
            }
        }
        false
    }

    /// Get the active loaded model's ID, if any.
    pub fn active_model_id(&self) -> Option<String> {
        self.active_model
            .read()
            // Optional: return None if RwLock is poisoned rather than panicking
            .ok()
            .and_then(|m| m.as_ref().map(|l| l.config.model_id.clone()))
    }

    /// Access the model registry (read-only).
    pub fn with_registry<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&ModelRegistry) -> R,
    {
        let reg = self
            .registry
            .read()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        Ok(f(&reg))
    }

    /// Access the model registry (mutable).
    pub fn with_registry_mut<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut ModelRegistry) -> R,
    {
        let mut reg = self
            .registry
            .write()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;
        Ok(f(&mut reg))
    }

    /// Load a draft model for speculative decoding.
    ///
    /// The draft model should be smaller/faster than the target model.
    /// It is loaded from the same registry as the target model.
    pub fn load_draft_model(&self, model_id: &str) -> Result<(), String> {
        let mut reg = self
            .registry
            .write()
            .map_err(|e| format!("registry lock poisoned: {e}"))?;

        if reg.available_models().is_empty() {
            reg.discover();
        }

        let loaded = reg.load(model_id)?;

        let mut draft = self
            .draft_model
            .write()
            .map_err(|e| format!("draft_model lock poisoned: {e}"))?;
        *draft = Some(loaded);
        Ok(())
    }

    /// Unload the draft model, freeing memory.
    pub fn unload_draft_model(&self) {
        if let Ok(mut draft) = self.draft_model.write() {
            *draft = None;
        }
    }

    /// Whether a draft model is loaded.
    pub fn has_draft_model(&self) -> bool {
        self.draft_model
            .read()
            .map(|d| d.is_some())
            .unwrap_or(false)
    }

    /// Get the current speculative decoding configuration.
    pub fn speculative_config(&self) -> SpeculativeConfig {
        self.speculative_config
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// Set the speculative decoding configuration.
    pub fn set_speculative_config(&self, config: SpeculativeConfig) {
        if let Ok(mut c) = self.speculative_config.write() {
            *c = config;
        }
    }

    /// Run the candle inference pipeline on a loaded model.
    ///
    /// Pipeline: tokenize → cached forward passes → sample → decode.
    ///
    /// Uses [`KvCache`] so the generation loop processes only the *new*
    /// token each step instead of the full sequence, reducing work from
    /// O(N·T) to O(N + T).
    fn run_inference(
        loaded: &LoadedModel,
        prompt: &str,
        max_tokens: u32,
        sampling: &SamplingConfig,
    ) -> Result<(String, u32, u64), AgentError> {
        let start = Instant::now();

        // 1. Tokenize the prompt
        let encoding = loaded
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| AgentError::SupervisorError(format!("tokenization failed: {e}")))?;
        let input_ids = encoding.get_ids();

        if input_ids.is_empty() {
            return Err(AgentError::SupervisorError(
                "tokenization produced empty input".to_string(),
            ));
        }

        // Check context length
        if input_ids.len() > loaded.config.max_context_length {
            return Err(AgentError::SupervisorError(format!(
                "prompt exceeds model context length: {} tokens > {} max",
                input_ids.len(),
                loaded.config.max_context_length
            )));
        }

        let device = &loaded.device;

        // 2. Build the weight lookup for forward passes
        let weight_map: std::collections::HashMap<&str, &Tensor> = loaded
            .weights
            .iter()
            .map(|(name, tensor)| (name.as_str(), tensor))
            .collect();

        let vocab_size = Self::infer_vocab_size(&weight_map);
        let num_layers = Self::count_layers(&weight_map);
        let mut kv_cache = KvCache::new(num_layers);

        // 3. PREFILL — process the entire prompt in one shot to populate cache
        let prompt_tensor = Tensor::new(input_ids, device).map_err(|e| {
            AgentError::SupervisorError(format!("failed to create prompt tensor: {e}"))
        })?;

        let prefill_logits = Self::forward_pass_cached(
            &prompt_tensor,
            &weight_map,
            vocab_size,
            device,
            &mut kv_cache,
        )?;

        // Get logits for the last prompt position
        let last_logits = Self::last_position_logits(&prefill_logits, input_ids.len())?;

        // 4. GENERATION — one new token per step through the cached path
        let mut generated_ids: Vec<u32> = Vec::with_capacity(max_tokens as usize);
        let mut all_ids: Vec<u32> = input_ids.to_vec();

        // Sample from the prefill output
        let last_logits = Self::apply_repetition_penalty(
            &last_logits,
            &all_ids,
            &generated_ids,
            sampling.repetition_penalty,
        )?;

        let mut next_token =
            Self::sample_token(&last_logits, sampling.temperature, sampling.top_p)?;

        for _step in 0..max_tokens {
            if next_token == 2 || next_token == 0 {
                break;
            }

            generated_ids.push(next_token);
            all_ids.push(next_token);

            if all_ids.len() >= loaded.config.max_context_length {
                break;
            }

            // Forward pass for the SINGLE new token only
            let token_tensor = Tensor::new(&[next_token], device).map_err(|e| {
                AgentError::SupervisorError(format!("failed to create token tensor: {e}"))
            })?;

            let logits = Self::forward_pass_cached(
                &token_tensor,
                &weight_map,
                vocab_size,
                device,
                &mut kv_cache,
            )?;

            let last_logits = Self::last_position_logits(&logits, 1)?;

            let last_logits = Self::apply_repetition_penalty(
                &last_logits,
                &all_ids,
                &generated_ids,
                sampling.repetition_penalty,
            )?;

            next_token = Self::sample_token(&last_logits, sampling.temperature, sampling.top_p)?;
        }

        // 5. Decode generated tokens
        let output_text = loaded
            .tokenizer
            .decode(&generated_ids, true)
            .map_err(|e| AgentError::SupervisorError(format!("decoding failed: {e}")))?;

        let inference_ms = start.elapsed().as_millis() as u64;
        let token_count = generated_ids.len() as u32;

        Ok((output_text, token_count, inference_ms))
    }

    /// Extract logits for the last position from a (possibly batched) tensor.
    fn last_position_logits(logits: &Tensor, seq_len: usize) -> Result<Tensor, AgentError> {
        if logits.dims().len() == 2 {
            // Shape: (seq_len, vocab_size) — take last row.
            logits
                .get(seq_len - 1)
                .map_err(|e| AgentError::SupervisorError(format!("failed to index logits: {e}")))
        } else {
            // Shape: (vocab_size,) — already a single position.
            Ok(logits.clone())
        }
    }

    /// Infer vocab size from the weight map by looking for common embedding
    /// layer names.
    fn infer_vocab_size(weight_map: &std::collections::HashMap<&str, &Tensor>) -> usize {
        // Common names for the token embedding / LM head weight
        let candidates = [
            "lm_head.weight",
            "model.embed_tokens.weight",
            "transformer.wte.weight",
            "embed_tokens.weight",
            "wte.weight",
        ];
        for name in &candidates {
            if let Some(tensor) = weight_map.get(name) {
                let dims = tensor.dims();
                if !dims.is_empty() {
                    return dims[0];
                }
            }
        }
        // Fallback: a reasonable default
        32000
    }

    /// Count how many MLP layers are present in the weight map.
    fn count_layers(weight_map: &std::collections::HashMap<&str, &Tensor>) -> usize {
        let mut n = 0;
        loop {
            let dense_key = format!("model.layers.{n}.mlp.down_proj.weight");
            let up_key = format!("model.layers.{n}.mlp.up_proj.weight");
            if !weight_map.contains_key(dense_key.as_str())
                && !weight_map.contains_key(up_key.as_str())
            {
                break;
            }
            n += 1;
            if n > 128 {
                break;
            }
        }
        n
    }

    /// Cached forward pass — processes only the *new* token(s) through the
    /// MLP stack and updates `kv_cache` so subsequent calls can skip
    /// already-processed positions.
    ///
    /// Because the MLP layers are position-independent (no attention), each
    /// token's hidden state is computed independently. The cache stores per-
    /// layer hidden states so the LM-head projection only needs the latest.
    fn forward_pass_cached(
        input_ids: &Tensor,
        weight_map: &std::collections::HashMap<&str, &Tensor>,
        vocab_size: usize,
        device: &candle_core::Device,
        kv_cache: &mut KvCache,
    ) -> Result<Tensor, AgentError> {
        let map_err =
            |e: candle_core::Error| AgentError::SupervisorError(format!("forward pass: {e}"));

        // Embedding lookup — only for the new token(s) being processed.
        // The caller is responsible for passing only new (uncached) tokens.
        let embed_weight = Self::find_embedding_weight(weight_map).ok_or_else(|| {
            AgentError::SupervisorError("no embedding weight found in model weights".to_string())
        })?;

        let hidden = embed_weight.index_select(input_ids, 0).map_err(map_err)?;

        // Apply MLP layers — each token processed independently.
        // For each layer, compute the forward pass only on new tokens, then
        // concatenate with cached results and store back.
        let mut current = hidden;
        let mut layer_idx = 0;
        loop {
            let down_key = format!("model.layers.{layer_idx}.mlp.down_proj.weight");
            let up_key = format!("model.layers.{layer_idx}.mlp.up_proj.weight");
            let gate_key = format!("model.layers.{layer_idx}.mlp.gate_proj.weight");

            let has_layer = weight_map.contains_key(down_key.as_str())
                || weight_map.contains_key(up_key.as_str());

            if !has_layer {
                break;
            }

            // Standard MLP: hidden = matmul(silu(gate) * up, down_proj.T)
            // If gate_proj/up_proj are missing, fall back to down_proj only.
            let has_gate = weight_map.contains_key(gate_key.as_str());
            let has_up = weight_map.contains_key(up_key.as_str());
            let has_down = weight_map.contains_key(down_key.as_str());

            if has_gate && has_up && has_down {
                let gate_w = weight_map[gate_key.as_str()];
                let up_w = weight_map[up_key.as_str()];
                let down_w = weight_map[down_key.as_str()];

                let gate_t = gate_w.t().map_err(map_err)?;
                let up_t = up_w.t().map_err(map_err)?;
                let down_t = down_w.t().map_err(map_err)?;

                // Validate dimensions
                let h_dim = current.dims().last().copied().unwrap_or(0);
                let gate_in = gate_t.dims().first().copied().unwrap_or(0);
                let up_in = up_t.dims().first().copied().unwrap_or(0);

                if h_dim != gate_in || h_dim != up_in {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         hidden_dim={h_dim}, gate_in={gate_in}, up_in={up_in}"
                    )));
                }

                // gate = matmul(hidden, gate_proj.T)
                let gate = current.matmul(&gate_t).map_err(map_err)?;
                // up = matmul(hidden, up_proj.T)
                let up = current.matmul(&up_t).map_err(map_err)?;
                // silu(x) = x * sigmoid(x) = x / (1 + exp(-x))
                // Compute element-wise on the flattened tensor, then reshape.
                let gate_silu = {
                    let gate_shape = gate.dims().to_vec();
                    let flat = gate.flatten_all().map_err(map_err)?;
                    let vals: Vec<f32> = flat.to_vec1::<f32>().map_err(map_err)?;
                    let silu_vals: Vec<f32> =
                        vals.iter().map(|&x| x / (1.0 + (-x).exp())).collect();
                    Tensor::new(&silu_vals[..], gate.device())
                        .map_err(map_err)?
                        .reshape(gate_shape.as_slice())
                        .map_err(map_err)?
                };
                // intermediate = silu(gate) * up
                let intermediate = gate_silu.mul(&up).map_err(map_err)?;

                // Validate intermediate -> down_proj dimension
                let inter_dim = intermediate.dims().last().copied().unwrap_or(0);
                let down_in = down_t.dims().first().copied().unwrap_or(0);
                if inter_dim != down_in {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         intermediate_dim={inter_dim}, down_in={down_in}"
                    )));
                }

                // hidden = matmul(intermediate, down_proj.T)
                current = intermediate.matmul(&down_t).map_err(map_err)?;
            } else if has_down {
                // Fallback: only down_proj available
                let w = weight_map[down_key.as_str()];
                let w_t = w.t().map_err(map_err)?;
                let h_dim = current.dims().last().copied().unwrap_or(0);
                let w_dim = w_t.dims().first().copied().unwrap_or(0);
                if h_dim != w_dim {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         hidden_dim={h_dim}, weight_dim={w_dim}"
                    )));
                }
                current = current.matmul(&w_t).map_err(map_err)?;
            } else {
                // Only up_proj without down_proj — unusual but not a hard error.
                // Skip this layer.
            }

            // Store the new tokens' hidden state in the cache.
            // For this MLP arch the "key" and "value" are both the hidden
            // state after the layer.
            // Best-effort: store hidden state in KV cache for incremental inference
            let _ = kv_cache
                .update(layer_idx, current.clone(), current.clone())
                .map_err(map_err)?;

            layer_idx += 1;
            if layer_idx > 128 {
                break;
            }
        }

        // The cache now holds the full sequence hidden states (old + new
        // concatenated by kv_cache.update).  For the LM-head projection we
        // only need the new tokens' hidden states, which is `current`.
        Self::project_lm_head(&current, weight_map, embed_weight, vocab_size, device)
    }

    /// Original uncached forward pass — processes the full sequence.
    ///
    /// Kept for correctness testing against the cached variant.
    #[cfg(test)]
    fn forward_pass_uncached(
        input_ids: &Tensor,
        weight_map: &std::collections::HashMap<&str, &Tensor>,
        vocab_size: usize,
        device: &candle_core::Device,
    ) -> Result<Tensor, AgentError> {
        let map_err =
            |e: candle_core::Error| AgentError::SupervisorError(format!("forward pass: {e}"));

        let embed_weight = Self::find_embedding_weight(weight_map).ok_or_else(|| {
            AgentError::SupervisorError("no embedding weight found in model weights".to_string())
        })?;

        let hidden = embed_weight.index_select(input_ids, 0).map_err(map_err)?;

        let mut current = hidden;
        let mut layer_idx = 0;
        loop {
            let down_key = format!("model.layers.{layer_idx}.mlp.down_proj.weight");
            let up_key = format!("model.layers.{layer_idx}.mlp.up_proj.weight");
            let gate_key = format!("model.layers.{layer_idx}.mlp.gate_proj.weight");

            let has_layer = weight_map.contains_key(down_key.as_str())
                || weight_map.contains_key(up_key.as_str());

            if !has_layer {
                break;
            }

            let has_gate = weight_map.contains_key(gate_key.as_str());
            let has_up = weight_map.contains_key(up_key.as_str());
            let has_down = weight_map.contains_key(down_key.as_str());

            if has_gate && has_up && has_down {
                let gate_w = weight_map[gate_key.as_str()];
                let up_w = weight_map[up_key.as_str()];
                let down_w = weight_map[down_key.as_str()];

                let gate_t = gate_w.t().map_err(map_err)?;
                let up_t = up_w.t().map_err(map_err)?;
                let down_t = down_w.t().map_err(map_err)?;

                let h_dim = current.dims().last().copied().unwrap_or(0);
                let gate_in = gate_t.dims().first().copied().unwrap_or(0);
                let up_in = up_t.dims().first().copied().unwrap_or(0);

                if h_dim != gate_in || h_dim != up_in {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         hidden_dim={h_dim}, gate_in={gate_in}, up_in={up_in}"
                    )));
                }

                let gate = current.matmul(&gate_t).map_err(map_err)?;
                let up = current.matmul(&up_t).map_err(map_err)?;
                let gate_silu = {
                    let neg_gate = gate.neg().map_err(map_err)?;
                    let sigmoid = {
                        let exp_neg = neg_gate.exp().map_err(map_err)?;
                        let ones = Tensor::ones_like(&exp_neg).map_err(map_err)?;
                        let denom = ones.add(&exp_neg).map_err(map_err)?;
                        let ones2 = Tensor::ones_like(&denom).map_err(map_err)?;
                        ones2.div(&denom).map_err(map_err)?
                    };
                    gate.mul(&sigmoid).map_err(map_err)?
                };
                let intermediate = gate_silu.mul(&up).map_err(map_err)?;

                let inter_dim = intermediate.dims().last().copied().unwrap_or(0);
                let down_in = down_t.dims().first().copied().unwrap_or(0);
                if inter_dim != down_in {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         intermediate_dim={inter_dim}, down_in={down_in}"
                    )));
                }

                current = intermediate.matmul(&down_t).map_err(map_err)?;
            } else if has_down {
                let w = weight_map[down_key.as_str()];
                let w_t = w.t().map_err(map_err)?;
                let h_dim = current.dims().last().copied().unwrap_or(0);
                let w_dim = w_t.dims().first().copied().unwrap_or(0);
                if h_dim != w_dim {
                    return Err(AgentError::SupervisorError(format!(
                        "MLP dimension mismatch at layer {layer_idx}: \
                         hidden_dim={h_dim}, weight_dim={w_dim}"
                    )));
                }
                current = current.matmul(&w_t).map_err(map_err)?;
            }

            layer_idx += 1;
            if layer_idx > 128 {
                break;
            }
        }

        Self::project_lm_head(&current, weight_map, embed_weight, vocab_size, device)
    }

    /// Shared LM head projection — maps hidden state to logits.
    fn project_lm_head(
        current: &Tensor,
        weight_map: &std::collections::HashMap<&str, &Tensor>,
        embed_weight: &Tensor,
        vocab_size: usize,
        device: &candle_core::Device,
    ) -> Result<Tensor, AgentError> {
        let map_err =
            |e: candle_core::Error| AgentError::SupervisorError(format!("forward pass: {e}"));

        let lm_head = Self::find_lm_head_weight(weight_map);
        let logits = if let Some(head_weight) = lm_head {
            let head_t = head_weight.t().map_err(map_err)?;
            let h_dim = current.dims().last().copied().unwrap_or(0);
            let w_dim = head_t.dims().first().copied().unwrap_or(0);
            if h_dim == w_dim {
                current.matmul(&head_t).map_err(map_err)?
            } else {
                let seq = if current.dims().len() == 2 {
                    current.dims()[0]
                } else {
                    1
                };
                Tensor::zeros(&[seq, vocab_size], DType::F32, device).map_err(map_err)?
            }
        } else {
            let embed_t = embed_weight.t().map_err(map_err)?;
            let h_dim = current.dims().last().copied().unwrap_or(0);
            let w_dim = embed_t.dims().first().copied().unwrap_or(0);
            if h_dim == w_dim {
                current.matmul(&embed_t).map_err(map_err)?
            } else {
                let seq = if current.dims().len() == 2 {
                    current.dims()[0]
                } else {
                    1
                };
                Tensor::zeros(&[seq, vocab_size], DType::F32, device).map_err(map_err)?
            }
        };

        Ok(logits)
    }

    /// Find the token embedding weight tensor.
    fn find_embedding_weight<'a>(
        weight_map: &std::collections::HashMap<&str, &'a Tensor>,
    ) -> Option<&'a Tensor> {
        let candidates = [
            "model.embed_tokens.weight",
            "transformer.wte.weight",
            "embed_tokens.weight",
            "wte.weight",
            "embeddings.word_embeddings.weight",
        ];
        for name in &candidates {
            if let Some(tensor) = weight_map.get(name) {
                return Some(tensor);
            }
        }
        None
    }

    /// Find the language model head weight tensor.
    fn find_lm_head_weight<'a>(
        weight_map: &std::collections::HashMap<&str, &'a Tensor>,
    ) -> Option<&'a Tensor> {
        let candidates = [
            "lm_head.weight",
            "output.weight",
            "cls.predictions.decoder.weight",
        ];
        for name in &candidates {
            if let Some(tensor) = weight_map.get(name) {
                return Some(tensor);
            }
        }
        None
    }

    /// Apply repetition penalty to logits for tokens that have already appeared.
    fn apply_repetition_penalty(
        logits: &Tensor,
        context_ids: &[u32],
        generated_ids: &[u32],
        penalty: f32,
    ) -> Result<Tensor, AgentError> {
        if (penalty - 1.0).abs() < f32::EPSILON {
            return Ok(logits.clone());
        }

        let logits_vec: Vec<f32> = logits
            .to_vec1::<f32>()
            .map_err(|e| AgentError::SupervisorError(format!("repetition penalty: {e}")))?;

        let mut modified = logits_vec;

        // Collect all token IDs that have appeared
        let mut seen = std::collections::HashSet::new();
        for &id in context_ids.iter().chain(generated_ids.iter()) {
            seen.insert(id as usize);
        }

        for &id in &seen {
            if id < modified.len() {
                if modified[id] > 0.0 {
                    modified[id] /= penalty;
                } else {
                    modified[id] *= penalty;
                }
            }
        }

        Tensor::new(&modified[..], logits.device())
            .map_err(|e| AgentError::SupervisorError(format!("repetition penalty tensor: {e}")))
    }

    /// Sample a token from logits using temperature and top-p.
    fn sample_token(logits: &Tensor, temperature: f64, top_p: f64) -> Result<u32, AgentError> {
        let map_err = |e: candle_core::Error| AgentError::SupervisorError(format!("sampling: {e}"));

        // Convert to f32 if needed
        let logits = if logits.dtype() != DType::F32 {
            logits.to_dtype(DType::F32).map_err(map_err)?
        } else {
            logits.clone()
        };

        let logits_vec: Vec<f32> = logits.to_vec1::<f32>().map_err(map_err)?;

        if logits_vec.is_empty() {
            return Err(AgentError::SupervisorError(
                "empty logits vector".to_string(),
            ));
        }

        // Greedy decoding for temperature ~0
        if temperature < 1e-6 {
            let (max_idx, _) = logits_vec
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, &0.0));
            return Ok(max_idx as u32);
        }

        // Apply temperature
        let temp = temperature as f32;
        let scaled: Vec<f32> = logits_vec.iter().map(|&x| x / temp).collect();

        // Softmax
        let max_val = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_vals: Vec<f32> = scaled.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f32 = exp_vals.iter().sum();
        let probs: Vec<f32> = exp_vals.iter().map(|&x| x / sum).collect();

        // Top-p (nucleus) filtering
        let mut indexed_probs: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
        indexed_probs.sort_unstable_by(|(_, a), (_, b)| {
            b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut cumulative = 0.0f32;
        let top_p_f32 = top_p as f32;
        let mut candidates: Vec<(usize, f32)> = Vec::new();
        for (idx, prob) in &indexed_probs {
            candidates.push((*idx, *prob));
            cumulative += prob;
            if cumulative >= top_p_f32 {
                break;
            }
        }

        // Renormalize candidates
        let cand_sum: f32 = candidates.iter().map(|(_, p)| p).sum();
        if cand_sum <= 0.0 {
            return Ok(candidates.first().map(|(i, _)| *i as u32).unwrap_or(0));
        }

        // Simple deterministic sampling: pick the highest probability token
        // from the nucleus set. For true randomness a PRNG would be used,
        // but deterministic behavior is more testable and reproducible
        // for governance tasks.
        let (best_idx, _) = candidates
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(&(0, 0.0));

        Ok(*best_idx as u32)
    }

    /// Convert a 1-D logits tensor to a probability vector via softmax.
    fn logits_to_probs(logits: &Tensor) -> Result<Vec<f32>, AgentError> {
        let vals: Vec<f32> = logits
            .to_vec1::<f32>()
            .map_err(|e| AgentError::SupervisorError(format!("logits_to_probs: {e}")))?;
        if vals.is_empty() {
            return Ok(vals);
        }
        let max_val = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_vals: Vec<f32> = vals.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f32 = exp_vals.iter().sum();
        if sum <= 0.0 {
            return Ok(exp_vals);
        }
        Ok(exp_vals.iter().map(|&x| x / sum).collect())
    }

    /// Speculative decoding inference loop.
    ///
    /// A small *draft* model proposes `config.draft_steps` candidate tokens,
    /// then the *target* model verifies them in a single batched forward pass.
    /// Tokens where `target_prob >= threshold * draft_prob` are accepted;
    /// rejection truncates the remaining candidates.
    ///
    /// Returns `(generated_token_ids, stats)`.
    fn run_inference_speculative(
        prompt_tokens: &[u32],
        max_tokens: u32,
        target: &LoadedModel,
        draft: &LoadedModel,
        config: &SpeculativeConfig,
        sampling: &SamplingConfig,
    ) -> Result<(Vec<u32>, SpeculativeStats), AgentError> {
        let device = &target.device;
        let draft_device = &draft.device;

        // Build weight maps for both models
        let target_wm: std::collections::HashMap<&str, &Tensor> = target
            .weights
            .iter()
            .map(|(n, t)| (n.as_str(), t))
            .collect();
        let draft_wm: std::collections::HashMap<&str, &Tensor> =
            draft.weights.iter().map(|(n, t)| (n.as_str(), t)).collect();

        let target_vocab = Self::infer_vocab_size(&target_wm);
        let draft_vocab = Self::infer_vocab_size(&draft_wm);
        let target_layers = Self::count_layers(&target_wm);
        let draft_layers = Self::count_layers(&draft_wm);

        let mut target_cache = KvCache::new(target_layers);
        let mut draft_cache = KvCache::new(draft_layers);

        // Prefill both models with the prompt
        let prompt_tensor = Tensor::new(prompt_tokens, device)
            .map_err(|e| AgentError::SupervisorError(format!("speculative: prompt tensor: {e}")))?;
        let draft_prompt = Tensor::new(prompt_tokens, draft_device).map_err(|e| {
            AgentError::SupervisorError(format!("speculative: draft prompt tensor: {e}"))
        })?;

        let target_prefill = Self::forward_pass_cached(
            &prompt_tensor,
            &target_wm,
            target_vocab,
            device,
            &mut target_cache,
        )?;
        let draft_prefill = Self::forward_pass_cached(
            &draft_prompt,
            &draft_wm,
            draft_vocab,
            draft_device,
            &mut draft_cache,
        )?;

        let mut stats = SpeculativeStats::default();
        let mut generated: Vec<u32> = Vec::with_capacity(max_tokens as usize);
        let mut all_ids: Vec<u32> = prompt_tokens.to_vec();

        // Sample first token from target prefill
        let last_target = Self::last_position_logits(&target_prefill, prompt_tokens.len())?;
        // Also advance draft pointer (we discard its logits — target decides)
        let _last_draft = Self::last_position_logits(&draft_prefill, prompt_tokens.len())?;

        let last_target_pen = Self::apply_repetition_penalty(
            &last_target,
            &all_ids,
            &generated,
            sampling.repetition_penalty,
        )?;
        let mut next_token =
            Self::sample_token(&last_target_pen, sampling.temperature, sampling.top_p)?;

        // `prev_target_logits` holds the target's logits from the last accepted
        // position.  These are needed to verify the first draft token in each
        // round (before any draft tokens have been fed to the target).
        // Initialized inside the loop from the target's sync pass.
        let mut prev_target_logits: Tensor;

        while (generated.len() as u32) < max_tokens {
            if next_token == 2 || next_token == 0 {
                break;
            }
            generated.push(next_token);
            all_ids.push(next_token);

            if all_ids.len() >= target.config.max_context_length {
                break;
            }

            // Feed the accepted token to BOTH caches so they stay in sync.
            let sync_tensor_t = Tensor::new(&[next_token], device).map_err(|e| {
                AgentError::SupervisorError(format!("speculative: sync tensor target: {e}"))
            })?;
            let target_after_sync = Self::forward_pass_cached(
                &sync_tensor_t,
                &target_wm,
                target_vocab,
                device,
                &mut target_cache,
            )?;
            prev_target_logits = Self::last_position_logits(&target_after_sync, 1)?;

            let sync_tensor_d = Tensor::new(&[next_token], draft_device).map_err(|e| {
                AgentError::SupervisorError(format!("speculative: sync tensor draft: {e}"))
            })?;
            let draft_sync_logits = Self::forward_pass_cached(
                &sync_tensor_d,
                &draft_wm,
                draft_vocab,
                draft_device,
                &mut draft_cache,
            )?;

            // --- DRAFT PHASE ---
            let k = config
                .draft_steps
                .min((max_tokens as usize).saturating_sub(generated.len()));
            if k == 0 {
                break;
            }

            let mut draft_tokens: Vec<u32> = Vec::with_capacity(k);
            let mut draft_probs_at_token: Vec<f32> = Vec::with_capacity(k);

            // Use the logits from the draft sync pass (above) to sample the
            // first draft token.  The cache was already advanced with
            // `next_token`, so we have the logits we need.
            let draft_last_logits = Self::last_position_logits(&draft_sync_logits, 1)?;
            let draft_p0 = Self::logits_to_probs(&draft_last_logits)?;
            let mut draft_next =
                Self::sample_token(&draft_last_logits, sampling.temperature, sampling.top_p)?;
            let prob0 = draft_p0.get(draft_next as usize).copied().unwrap_or(0.0);
            draft_tokens.push(draft_next);
            draft_probs_at_token.push(prob0);

            for _step in 1..k {
                let dt = Tensor::new(&[draft_next], draft_device).map_err(|e| {
                    AgentError::SupervisorError(format!("speculative: draft tensor: {e}"))
                })?;
                let draft_logits = Self::forward_pass_cached(
                    &dt,
                    &draft_wm,
                    draft_vocab,
                    draft_device,
                    &mut draft_cache,
                )?;
                let draft_last = Self::last_position_logits(&draft_logits, 1)?;
                let draft_p = Self::logits_to_probs(&draft_last)?;

                draft_next = Self::sample_token(&draft_last, sampling.temperature, sampling.top_p)?;
                let prob = draft_p.get(draft_next as usize).copied().unwrap_or(0.0);
                draft_tokens.push(draft_next);
                draft_probs_at_token.push(prob);
            }

            stats.total_draft_tokens += draft_tokens.len();

            // --- VERIFY PHASE ---
            // Feed all draft tokens to the target model in one batched pass.
            let verify_tensor = Tensor::new(draft_tokens.as_slice(), device).map_err(|e| {
                AgentError::SupervisorError(format!("speculative: verify tensor: {e}"))
            })?;
            let target_logits = Self::forward_pass_cached(
                &verify_tensor,
                &target_wm,
                target_vocab,
                device,
                &mut target_cache,
            )?;
            stats.target_forward_passes += 1;

            // --- ACCEPT / REJECT ---
            // For draft token i:
            //   - i==0: compare against prev_target_logits (target's view before
            //           seeing any draft tokens in this round)
            //   - i>0:  compare against target_logits[i-1] (target's view after
            //           seeing draft tokens 0..i-1)
            let mut accepted_count = 0usize;
            for i in 0..draft_tokens.len() {
                let t_logits_i = if i == 0 {
                    prev_target_logits.clone()
                } else if draft_tokens.len() == 1 {
                    // unreachable since i==0 is handled, but guard anyway
                    Self::last_position_logits(&target_logits, 1)?
                } else {
                    target_logits.get(i - 1).map_err(|e| {
                        AgentError::SupervisorError(format!("speculative: index logits: {e}"))
                    })?
                };
                let target_p = Self::logits_to_probs(&t_logits_i)?;
                let tp = target_p
                    .get(draft_tokens[i] as usize)
                    .copied()
                    .unwrap_or(0.0);
                let dp = draft_probs_at_token[i];

                if dp > 0.0 && tp >= config.acceptance_threshold * dp {
                    accepted_count += 1;
                    generated.push(draft_tokens[i]);
                    all_ids.push(draft_tokens[i]);

                    if draft_tokens[i] == 2 || draft_tokens[i] == 0 {
                        break;
                    }
                    if all_ids.len() >= target.config.max_context_length {
                        break;
                    }
                } else {
                    break;
                }
            }

            stats.accepted_tokens += accepted_count;
            stats.rejected_tokens += draft_tokens.len() - accepted_count;

            // Roll back the target cache for rejected tokens.
            let rejected = draft_tokens.len() - accepted_count;
            if rejected > 0 {
                Self::trim_cache(&mut target_cache, rejected);
            }

            // Adjust the draft cache to match the accepted position.
            //
            // During the draft phase the cache was advanced by (K-1) draft
            // token forward passes (the K-th token was sampled but not fed
            // to the forward pass).  Trim those draft positions, then feed
            // only the accepted tokens back.  This avoids the expensive
            // full-sequence rebuild.
            let draft_fwd_count = if draft_tokens.len() > 1 {
                draft_tokens.len() - 1
            } else {
                0
            };
            // Remove all draft-phase positions from the cache.
            if draft_fwd_count > 0 {
                Self::trim_cache(&mut draft_cache, draft_fwd_count);
            }
            // Feed back the accepted tokens so the cache covers all_ids.
            if accepted_count > 0 {
                let accepted_slice = &draft_tokens[..accepted_count];
                let at = Tensor::new(accepted_slice, draft_device).map_err(|e| {
                    AgentError::SupervisorError(format!(
                        "speculative: feed accepted to draft cache: {e}"
                    ))
                })?;
                let _ = Self::forward_pass_cached(
                    &at,
                    &draft_wm,
                    draft_vocab,
                    draft_device,
                    &mut draft_cache,
                )?;
            }

            // If all K tokens accepted, sample a bonus token from target's last logits.
            if accepted_count == draft_tokens.len() && !draft_tokens.is_empty() {
                let bonus_logits = Self::last_position_logits(&target_logits, draft_tokens.len())?;
                let bonus_logits = Self::apply_repetition_penalty(
                    &bonus_logits,
                    &all_ids,
                    &generated,
                    sampling.repetition_penalty,
                )?;
                next_token =
                    Self::sample_token(&bonus_logits, sampling.temperature, sampling.top_p)?;
            } else if accepted_count > 0 {
                // Use the target logits at the last accepted position.
                let idx = accepted_count - 1;
                let rl = if draft_tokens.len() == 1 {
                    Self::last_position_logits(&target_logits, 1)?
                } else {
                    target_logits.get(idx).map_err(|e| {
                        AgentError::SupervisorError(format!("speculative: resample: {e}"))
                    })?
                };
                let rl = Self::apply_repetition_penalty(
                    &rl,
                    &all_ids,
                    &generated,
                    sampling.repetition_penalty,
                )?;
                next_token = Self::sample_token(&rl, sampling.temperature, sampling.top_p)?;
            } else {
                // All rejected — use prev_target_logits to resample a different token.
                // Since the sampler is deterministic, we'd get the same token again.
                // Just advance with the target's own greedy pick.
                let rl = Self::apply_repetition_penalty(
                    &prev_target_logits,
                    &all_ids,
                    &generated,
                    sampling.repetition_penalty,
                )?;
                next_token = Self::sample_token(&rl, sampling.temperature, sampling.top_p)?;
            }
        }

        // Finalize stats
        if stats.total_draft_tokens > 0 {
            stats.acceptance_rate = stats.accepted_tokens as f32 / stats.total_draft_tokens as f32;
        }
        if stats.target_forward_passes > 0 {
            stats.effective_tokens_per_pass =
                stats.accepted_tokens as f32 / stats.target_forward_passes as f32;
        }

        Ok((generated, stats))
    }

    /// Trim the last `n` sequence positions from every layer in the cache.
    fn trim_cache(cache: &mut KvCache, n: usize) {
        for slot in cache.keys.iter_mut() {
            if let Some(t) = slot.take() {
                let seq = t.dims()[0];
                if seq > n {
                    if let Ok(trimmed) = t.narrow(0, 0, seq - n) {
                        *slot = Some(trimmed);
                    }
                }
                // If seq <= n the whole layer is cleared (slot is already None).
            }
        }
        for slot in cache.values.iter_mut() {
            if let Some(t) = slot.take() {
                let seq = t.dims()[0];
                if seq > n {
                    if let Ok(trimmed) = t.narrow(0, 0, seq - n) {
                        *slot = Some(trimmed);
                    }
                }
            }
        }
    }
}

impl LlmProvider for LocalSlmProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        // Get the active loaded model
        let active = self
            .active_model
            .read()
            .map_err(|e| AgentError::SupervisorError(format!("active_model lock poisoned: {e}")))?;

        let loaded = active.as_ref().ok_or_else(|| {
            AgentError::SupervisorError(
                "local SLM: no model loaded. Call load_model() first.".to_string(),
            )
        })?;

        let sampling = self.sampling_config();
        let spec_cfg = self.speculative_config();

        // Use speculative decoding when enabled and a draft model is loaded.
        if spec_cfg.enabled {
            let draft_guard = self.draft_model.read().map_err(|e| {
                AgentError::SupervisorError(format!("draft_model lock poisoned: {e}"))
            })?;
            if let Some(ref draft_loaded) = *draft_guard {
                let encoding = loaded.tokenizer.encode(prompt, true).map_err(|e| {
                    AgentError::SupervisorError(format!("tokenization failed: {e}"))
                })?;
                let input_ids = encoding.get_ids();
                if input_ids.is_empty() {
                    return Err(AgentError::SupervisorError(
                        "tokenization produced empty input".to_string(),
                    ));
                }

                let (generated, _stats) = Self::run_inference_speculative(
                    input_ids,
                    max_tokens,
                    loaded,
                    draft_loaded,
                    &spec_cfg,
                    &sampling,
                )?;

                // Fuel policy: charge only for ACCEPTED output tokens.
                // Draft-model work is internal and not billed.
                let output_text = loaded
                    .tokenizer
                    .decode(&generated, true)
                    .map_err(|e| AgentError::SupervisorError(format!("decoding failed: {e}")))?;

                return Ok(LlmResponse {
                    output_text,
                    token_count: generated.len() as u32,
                    model_name: model.to_string(),
                    tool_calls: Vec::new(),
                    input_tokens: None,
                });
            }
        }

        // Fallback: normal (non-speculative) inference
        let (output_text, token_count, _inference_ms) =
            Self::run_inference(loaded, prompt, max_tokens, &sampling)?;

        Ok(LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
            input_tokens: None,
        })
    }

    fn name(&self) -> &str {
        "local-slm"
    }

    fn cost_per_token(&self) -> f64 {
        0.0 // local inference is free
    }

    fn is_paid(&self) -> bool {
        false
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        // If a model is loaded, use its tokenizer for accurate count.
        if let Ok(active) = self.active_model.read() {
            if let Some(ref loaded) = *active {
                if let Ok(encoding) = loaded.tokenizer.encode(prompt, false) {
                    return encoding.get_ids().len() as u32;
                }
            }
        }
        // Fallback: rough char/4 estimate
        let chars = prompt.chars().count();
        u32::try_from(chars.saturating_div(4).saturating_add(1)).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_registry::ModelRegistry;
    use std::path::PathBuf;

    fn make_provider() -> LocalSlmProvider {
        LocalSlmProvider::new(ModelRegistry::new(PathBuf::from(
            "/tmp/nexus_test_nonexistent",
        )))
    }

    #[test]
    fn provider_name_is_local_slm() {
        let provider = make_provider();
        assert_eq!(provider.name(), "local-slm");
    }

    #[test]
    fn cost_per_token_is_zero() {
        let provider = make_provider();
        assert!((provider.cost_per_token() - 0.0).abs() < f64::EPSILON);
        assert!(!provider.is_paid());
    }

    #[test]
    fn not_available_without_loaded_model() {
        let provider = make_provider();
        assert!(!provider.is_available());
    }

    #[test]
    fn query_fails_without_loaded_model() {
        let provider = make_provider();
        let result = provider.query("test prompt", 100, "phi-4");
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("no model loaded"));
    }

    #[test]
    fn set_temperature() {
        let provider = make_provider();
        provider.set_temperature(0.3);
        let config = provider.sampling_config();
        assert!((config.temperature - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn set_top_p() {
        let provider = make_provider();
        provider.set_top_p(0.95);
        let config = provider.sampling_config();
        assert!((config.top_p - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn set_repetition_penalty() {
        let provider = make_provider();
        provider.set_repetition_penalty(1.2);
        let config = provider.sampling_config();
        assert!((config.repetition_penalty - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn default_sampling_config() {
        let config = SamplingConfig::default();
        assert!((config.temperature - 0.7).abs() < f64::EPSILON);
        assert!((config.top_p - 0.9).abs() < f64::EPSILON);
        assert!((config.repetition_penalty - 1.1).abs() < f32::EPSILON);
    }

    #[test]
    fn active_model_id_none_without_load() {
        let provider = make_provider();
        assert!(provider.active_model_id().is_none());
    }

    #[test]
    fn unload_model_returns_false_when_none() {
        let provider = make_provider();
        assert!(!provider.unload_model());
    }

    #[test]
    fn load_model_nonexistent_fails() {
        let provider = make_provider();
        let result = provider.load_model("nonexistent/model");
        assert!(result.is_err());
    }

    #[test]
    fn estimate_input_tokens_fallback() {
        let provider = make_provider();
        let count = provider.estimate_input_tokens("Hello, world!");
        assert!(count > 0);
        // ~13 chars / 4 + 1 = 4
        assert!((2..=20).contains(&count));
    }

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalSlmProvider>();
    }

    #[test]
    fn debug_format_works() {
        let provider = make_provider();
        let debug = format!("{:?}", provider);
        assert!(debug.contains("LocalSlmProvider"));
        assert!(debug.contains("has_model"));
        assert!(debug.contains("has_draft"));
        assert!(debug.contains("speculative"));
    }

    #[test]
    fn with_registry_read() {
        let provider = make_provider();
        let result = provider.with_registry(|reg| reg.available_models().len());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn with_registry_mut_discover() {
        let provider = make_provider();
        let result = provider.with_registry_mut(|reg| reg.discover());
        assert_eq!(result.unwrap(), 0);
    }

    // -----------------------------------------------------------------------
    // Sampling unit tests (test the core sampling logic directly)
    // -----------------------------------------------------------------------

    #[test]
    fn sample_token_greedy() {
        // Create a logits tensor with a clear maximum
        let logits = Tensor::new(&[0.1f32, 0.5, 10.0, 0.3], &candle_core::Device::Cpu).unwrap();
        let token = LocalSlmProvider::sample_token(&logits, 0.0, 1.0).unwrap();
        assert_eq!(token, 2); // index of 10.0
    }

    #[test]
    fn sample_token_with_temperature() {
        // With temperature, should still pick highest in deterministic mode
        let logits = Tensor::new(&[0.1f32, 0.5, 10.0, 0.3], &candle_core::Device::Cpu).unwrap();
        let token = LocalSlmProvider::sample_token(&logits, 0.7, 1.0).unwrap();
        assert_eq!(token, 2); // still picks highest (deterministic sampling)
    }

    #[test]
    fn sample_token_with_top_p() {
        // Top-p filtering with tight nucleus
        let logits = Tensor::new(&[0.1f32, 0.2, 100.0, 0.1], &candle_core::Device::Cpu).unwrap();
        let token = LocalSlmProvider::sample_token(&logits, 0.5, 0.1).unwrap();
        assert_eq!(token, 2); // dominant token should be selected
    }

    #[test]
    fn sample_token_empty_logits_fails() {
        let logits = Tensor::new(&[0f32; 0], &candle_core::Device::Cpu).unwrap();
        let result = LocalSlmProvider::sample_token(&logits, 0.7, 0.9);
        assert!(result.is_err());
    }

    #[test]
    fn apply_repetition_penalty_reduces_seen() {
        let logits = Tensor::new(&[5.0f32, 5.0, 5.0, 5.0], &candle_core::Device::Cpu).unwrap();
        let context_ids = vec![1u32, 3]; // tokens 1 and 3 appeared
        let generated_ids = vec![];

        let penalized =
            LocalSlmProvider::apply_repetition_penalty(&logits, &context_ids, &generated_ids, 2.0)
                .unwrap();

        let vals: Vec<f32> = penalized.to_vec1().unwrap();
        // Token 0: unpenalized = 5.0
        assert!((vals[0] - 5.0).abs() < f32::EPSILON);
        // Token 1: penalized = 5.0 / 2.0 = 2.5
        assert!((vals[1] - 2.5).abs() < f32::EPSILON);
        // Token 2: unpenalized = 5.0
        assert!((vals[2] - 5.0).abs() < f32::EPSILON);
        // Token 3: penalized = 5.0 / 2.0 = 2.5
        assert!((vals[3] - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn apply_repetition_penalty_no_op_at_one() {
        let logits = Tensor::new(&[3.0f32, 4.0, 5.0], &candle_core::Device::Cpu).unwrap();
        let penalized = LocalSlmProvider::apply_repetition_penalty(
            &logits,
            &[0, 1, 2],
            &[],
            1.0, // no penalty
        )
        .unwrap();

        let original: Vec<f32> = logits.to_vec1().unwrap();
        let result: Vec<f32> = penalized.to_vec1().unwrap();
        assert_eq!(original, result);
    }

    #[test]
    fn infer_vocab_size_from_weights() {
        let embed = Tensor::zeros(&[32000, 128], DType::F32, &candle_core::Device::Cpu).unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert("model.embed_tokens.weight", &embed);

        let vocab = LocalSlmProvider::infer_vocab_size(&map);
        assert_eq!(vocab, 32000);
    }

    #[test]
    fn infer_vocab_size_fallback() {
        let map: std::collections::HashMap<&str, &Tensor> = std::collections::HashMap::new();
        let vocab = LocalSlmProvider::infer_vocab_size(&map);
        assert_eq!(vocab, 32000); // default fallback
    }

    #[test]
    fn find_embedding_weight_finds_standard_names() {
        let embed = Tensor::zeros(&[100, 64], DType::F32, &candle_core::Device::Cpu).unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert("model.embed_tokens.weight", &embed);

        assert!(LocalSlmProvider::find_embedding_weight(&map).is_some());
    }

    #[test]
    fn find_embedding_weight_returns_none_if_missing() {
        let map: std::collections::HashMap<&str, &Tensor> = std::collections::HashMap::new();
        assert!(LocalSlmProvider::find_embedding_weight(&map).is_none());
    }

    #[test]
    fn find_lm_head_weight_finds_standard_names() {
        let head = Tensor::zeros(&[100, 64], DType::F32, &candle_core::Device::Cpu).unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert("lm_head.weight", &head);

        assert!(LocalSlmProvider::find_lm_head_weight(&map).is_some());
    }

    #[test]
    fn find_lm_head_weight_returns_none_if_missing() {
        let map: std::collections::HashMap<&str, &Tensor> = std::collections::HashMap::new();
        assert!(LocalSlmProvider::find_lm_head_weight(&map).is_none());
    }

    // -----------------------------------------------------------------------
    // KV cache unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_kv_cache_new() {
        let cache = KvCache::new(4);
        assert_eq!(cache.seq_len(), 0);
        assert!(cache.get(0).is_none());
        assert!(cache.get(3).is_none());
    }

    #[test]
    fn test_kv_cache_update() {
        let mut cache = KvCache::new(2);
        let device = candle_core::Device::Cpu;

        let k = Tensor::zeros(&[1, 8], DType::F32, &device).unwrap();
        let v = Tensor::ones(&[1, 8], DType::F32, &device).unwrap();

        let (full_k, full_v) = cache.update(0, k, v).unwrap();
        assert_eq!(full_k.dims(), &[1, 8]);
        assert_eq!(full_v.dims(), &[1, 8]);
        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn test_kv_cache_concatenation() {
        let mut cache = KvCache::new(1);
        let device = candle_core::Device::Cpu;

        // First token
        let k1 = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let v1 = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let (fk, fv) = cache.update(0, k1, v1).unwrap();
        assert_eq!(fk.dims(), &[1, 4]);
        assert_eq!(fv.dims(), &[1, 4]);

        // Second token — should concatenate along dim 0
        let k2 = Tensor::ones(&[1, 4], DType::F32, &device).unwrap();
        let v2 = Tensor::ones(&[1, 4], DType::F32, &device).unwrap();
        let (fk, fv) = cache.update(0, k2, v2).unwrap();
        assert_eq!(fk.dims(), &[2, 4]);
        assert_eq!(fv.dims(), &[2, 4]);

        // Third token — three rows now
        let k3 = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let v3 = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let (fk, _fv) = cache.update(0, k3, v3).unwrap();
        assert_eq!(fk.dims(), &[3, 4]);
    }

    #[test]
    fn test_kv_cache_reset() {
        let mut cache = KvCache::new(2);
        let device = candle_core::Device::Cpu;

        let k = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let v = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        cache.update(0, k.clone(), v.clone()).unwrap();
        cache.update(1, k, v).unwrap();

        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_some());

        cache.reset();
        assert!(cache.get(0).is_none());
        assert!(cache.get(1).is_none());
        assert_eq!(cache.seq_len(), 0);
    }

    #[test]
    fn test_kv_cache_seq_len() {
        let mut cache = KvCache::new(1);
        let device = candle_core::Device::Cpu;

        assert_eq!(cache.seq_len(), 0);

        let k1 = Tensor::zeros(&[3, 8], DType::F32, &device).unwrap();
        let v1 = Tensor::zeros(&[3, 8], DType::F32, &device).unwrap();
        cache.update(0, k1, v1).unwrap();
        assert_eq!(cache.seq_len(), 3);

        let k2 = Tensor::zeros(&[2, 8], DType::F32, &device).unwrap();
        let v2 = Tensor::zeros(&[2, 8], DType::F32, &device).unwrap();
        cache.update(0, k2, v2).unwrap();
        assert_eq!(cache.seq_len(), 5);
    }

    #[test]
    fn test_kv_cache_auto_grow() {
        // Updating a layer beyond initial capacity should grow the vectors
        let mut cache = KvCache::new(1);
        let device = candle_core::Device::Cpu;

        let k = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();
        let v = Tensor::zeros(&[1, 4], DType::F32, &device).unwrap();

        // Layer 5 is beyond initial capacity (1)
        let (fk, _) = cache.update(5, k, v).unwrap();
        assert_eq!(fk.dims(), &[1, 4]);
        assert!(cache.get(5).is_some());
    }

    // -----------------------------------------------------------------------
    // Cached vs uncached forward pass correctness
    // -----------------------------------------------------------------------

    /// Build a small toy weight map for forward pass tests.
    fn make_toy_weights() -> Vec<(String, Tensor)> {
        let device = candle_core::Device::Cpu;
        let vocab = 8;
        let hidden = 4;

        let embed = Tensor::randn(0f32, 1.0, &[vocab, hidden], &device).unwrap();
        let gate = Tensor::randn(0f32, 1.0, &[hidden, hidden], &device).unwrap();
        let up = Tensor::randn(0f32, 1.0, &[hidden, hidden], &device).unwrap();
        let down = Tensor::randn(0f32, 1.0, &[hidden, hidden], &device).unwrap();
        let lm_head = Tensor::randn(0f32, 1.0, &[vocab, hidden], &device).unwrap();

        vec![
            ("model.embed_tokens.weight".to_string(), embed),
            ("model.layers.0.mlp.gate_proj.weight".to_string(), gate),
            ("model.layers.0.mlp.up_proj.weight".to_string(), up),
            ("model.layers.0.mlp.down_proj.weight".to_string(), down),
            ("lm_head.weight".to_string(), lm_head),
        ]
    }

    #[test]
    fn test_cached_forward_single_token_matches_uncached() {
        let device = candle_core::Device::Cpu;
        let weights = make_toy_weights();
        let weight_map: std::collections::HashMap<&str, &Tensor> =
            weights.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let vocab_size = LocalSlmProvider::infer_vocab_size(&weight_map);
        let num_layers = LocalSlmProvider::count_layers(&weight_map);

        // Full sequence: [3, 5, 1]
        let ids = Tensor::new(&[3u32, 5, 1], &device).unwrap();

        // Uncached: process full sequence
        let logits_uncached =
            LocalSlmProvider::forward_pass_uncached(&ids, &weight_map, vocab_size, &device)
                .unwrap();
        // Get last position logits
        let uncached_last: Vec<f32> = logits_uncached.get(2).unwrap().to_vec1().unwrap();

        // Cached: process tokens one by one
        let mut cache = KvCache::new(num_layers);

        // Prefill [3, 5]
        let prefix = Tensor::new(&[3u32, 5], &device).unwrap();
        let _ = LocalSlmProvider::forward_pass_cached(
            &prefix,
            &weight_map,
            vocab_size,
            &device,
            &mut cache,
        )
        .unwrap();

        // Generate: process token [1]
        let single = Tensor::new(&[1u32], &device).unwrap();
        let logits_cached = LocalSlmProvider::forward_pass_cached(
            &single,
            &weight_map,
            vocab_size,
            &device,
            &mut cache,
        )
        .unwrap();
        let cached_last: Vec<f32> = logits_cached.get(0).unwrap().to_vec1().unwrap();

        // MLP is position-independent so single-token pass must match the
        // corresponding row from the full-sequence pass.
        for (a, b) in uncached_last.iter().zip(cached_last.iter()) {
            assert!(
                (a - b).abs() < 1e-4,
                "logits diverged: uncached={a}, cached={b}"
            );
        }
    }

    #[test]
    fn test_cached_forward_full_sequence_matches_uncached() {
        let device = candle_core::Device::Cpu;
        let weights = make_toy_weights();
        let weight_map: std::collections::HashMap<&str, &Tensor> =
            weights.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let vocab_size = LocalSlmProvider::infer_vocab_size(&weight_map);
        let num_layers = LocalSlmProvider::count_layers(&weight_map);

        let ids = Tensor::new(&[2u32, 7, 4, 0], &device).unwrap();

        let logits_uncached =
            LocalSlmProvider::forward_pass_uncached(&ids, &weight_map, vocab_size, &device)
                .unwrap();

        let mut cache = KvCache::new(num_layers);
        let logits_cached = LocalSlmProvider::forward_pass_cached(
            &ids,
            &weight_map,
            vocab_size,
            &device,
            &mut cache,
        )
        .unwrap();

        // All rows must match.
        let unc: Vec<f32> = logits_uncached.flatten_all().unwrap().to_vec1().unwrap();
        let cac: Vec<f32> = logits_cached.flatten_all().unwrap().to_vec1().unwrap();
        assert_eq!(unc.len(), cac.len());
        for (a, b) in unc.iter().zip(cac.iter()) {
            assert!(
                (a - b).abs() < 1e-4,
                "logits diverged: uncached={a}, cached={b}"
            );
        }
    }

    #[test]
    fn test_count_layers() {
        let weights = make_toy_weights();
        let weight_map: std::collections::HashMap<&str, &Tensor> =
            weights.iter().map(|(n, t)| (n.as_str(), t)).collect();
        assert_eq!(LocalSlmProvider::count_layers(&weight_map), 1);
    }

    #[test]
    fn test_count_layers_empty() {
        let map: std::collections::HashMap<&str, &Tensor> = std::collections::HashMap::new();
        assert_eq!(LocalSlmProvider::count_layers(&map), 0);
    }

    #[test]
    fn test_last_position_logits_2d() {
        let device = candle_core::Device::Cpu;
        let logits = Tensor::new(&[[1.0f32, 2.0], [3.0, 4.0], [5.0, 6.0]], &device).unwrap();
        let last = LocalSlmProvider::last_position_logits(&logits, 3).unwrap();
        let vals: Vec<f32> = last.to_vec1().unwrap();
        assert!((vals[0] - 5.0).abs() < f32::EPSILON);
        assert!((vals[1] - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_last_position_logits_1d() {
        let device = candle_core::Device::Cpu;
        let logits = Tensor::new(&[7.0f32, 8.0], &device).unwrap();
        let last = LocalSlmProvider::last_position_logits(&logits, 1).unwrap();
        let vals: Vec<f32> = last.to_vec1().unwrap();
        assert!((vals[0] - 7.0).abs() < f32::EPSILON);
        assert!((vals[1] - 8.0).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Speculative decoding tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_speculative_config_default() {
        let config = SpeculativeConfig::default();
        assert_eq!(config.draft_steps, 4);
        assert!((config.acceptance_threshold - 0.1).abs() < f32::EPSILON);
        assert!(!config.enabled);
    }

    #[test]
    fn test_speculative_stats_default() {
        let stats = SpeculativeStats::default();
        assert_eq!(stats.total_draft_tokens, 0);
        assert_eq!(stats.accepted_tokens, 0);
        assert_eq!(stats.rejected_tokens, 0);
        assert!((stats.acceptance_rate - 0.0).abs() < f32::EPSILON);
        assert_eq!(stats.target_forward_passes, 0);
        assert!((stats.effective_tokens_per_pass - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draft_model_load_unload() {
        let provider = make_provider();
        assert!(!provider.has_draft_model());

        // Loading a nonexistent model fails
        let result = provider.load_draft_model("nonexistent/model");
        assert!(result.is_err());
        assert!(!provider.has_draft_model());

        // Unloading when nothing loaded is a no-op
        provider.unload_draft_model();
        assert!(!provider.has_draft_model());
    }

    #[test]
    fn test_speculative_config_get_set() {
        let provider = make_provider();
        let default = provider.speculative_config();
        assert!(!default.enabled);

        provider.set_speculative_config(SpeculativeConfig {
            draft_steps: 8,
            acceptance_threshold: 0.5,
            enabled: true,
        });
        let updated = provider.speculative_config();
        assert!(updated.enabled);
        assert_eq!(updated.draft_steps, 8);
        assert!((updated.acceptance_threshold - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_speculative_disabled_fallback() {
        // With speculative disabled, query() should fall through to normal inference.
        // We can't run full inference without a loaded model, but we can verify the
        // code path: speculative is off → normal path → "no model loaded" error.
        let provider = make_provider();
        provider.set_speculative_config(SpeculativeConfig {
            enabled: false,
            ..SpeculativeConfig::default()
        });
        let result = provider.query("test", 10, "test-model");
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("no model loaded"));
    }

    /// Build "draft" weights: smaller model with 1 MLP layer (random).
    fn make_draft_weights() -> Vec<(String, Tensor)> {
        let device = candle_core::Device::Cpu;
        let vocab = 8;
        let hidden = 4;

        let embed = Tensor::randn(0f32, 0.5, &[vocab, hidden], &device).unwrap();
        let gate0 = Tensor::randn(0f32, 0.5, &[hidden, hidden], &device).unwrap();
        let up0 = Tensor::randn(0f32, 0.5, &[hidden, hidden], &device).unwrap();
        let down0 = Tensor::randn(0f32, 0.5, &[hidden, hidden], &device).unwrap();
        let lm_head = Tensor::randn(0f32, 0.5, &[vocab, hidden], &device).unwrap();

        vec![
            ("model.embed_tokens.weight".to_string(), embed),
            ("model.layers.0.mlp.gate_proj.weight".to_string(), gate0),
            ("model.layers.0.mlp.up_proj.weight".to_string(), up0),
            ("model.layers.0.mlp.down_proj.weight".to_string(), down0),
            ("lm_head.weight".to_string(), lm_head),
        ]
    }

    /// Build a mock LoadedModel from raw weights.
    fn mock_loaded_model(weights: Vec<(String, Tensor)>) -> LoadedModel {
        use crate::model_registry::{ModelConfig, Quantization};
        use std::sync::Arc;

        // Build a minimal byte-level tokenizer.
        let tokenizer = {
            use tokenizers::models::bpe::BPE;
            use tokenizers::Tokenizer;

            let mut vocab = std::collections::HashMap::new();
            for i in 0u32..256 {
                vocab.insert(format!("t{i}"), i);
            }
            // Add the merged token to the vocabulary so the merge is valid.
            vocab.insert("t0t1".to_string(), 256);
            let merges = vec![("t0".to_string(), "t1".to_string())];
            let bpe = BPE::builder()
                .vocab_and_merges(vocab, merges)
                .unk_token("t0".to_string())
                .build()
                .unwrap();
            Tokenizer::new(bpe)
        };

        LoadedModel {
            config: ModelConfig {
                model_id: "mock-model".to_string(),
                model_path: std::path::PathBuf::from("/tmp/mock"),
                quantization: Quantization::F32,
                max_context_length: 2048,
                recommended_tasks: vec![],
                min_ram_mb: 1,
            },
            weights: Arc::new(weights),
            tokenizer: Arc::new(tokenizer),
            device: candle_core::Device::Cpu,
        }
    }

    #[test]
    fn test_speculative_stats_tracking() {
        // Use deterministic weights for both target and draft to avoid flakiness.
        let target = mock_loaded_model(make_deterministic_weights());
        let draft = mock_loaded_model(make_draft_weights());

        let config = SpeculativeConfig {
            draft_steps: 3,
            acceptance_threshold: 0.1,
            enabled: true,
        };
        let sampling = SamplingConfig {
            temperature: 0.0, // greedy for determinism
            top_p: 1.0,
            repetition_penalty: 1.0,
        };

        // Use prompt tokens > 2 to avoid immediate EOS
        let prompt_tokens = &[3u32, 5, 4];
        let (generated, stats) = LocalSlmProvider::run_inference_speculative(
            prompt_tokens,
            8,
            &target,
            &draft,
            &config,
            &sampling,
        )
        .unwrap();

        // We should have generated something
        assert!(!generated.is_empty(), "should generate at least one token");
        // Stats should be populated
        assert!(stats.total_draft_tokens > 0);
        assert!(stats.accepted_tokens + stats.rejected_tokens == stats.total_draft_tokens);
        assert!(stats.target_forward_passes > 0);
        assert!(stats.acceptance_rate >= 0.0 && stats.acceptance_rate <= 1.0);
        assert!(stats.effective_tokens_per_pass >= 0.0);
    }

    /// Build deterministic weights that produce tokens > 2 (avoiding EOS).
    ///
    /// Uses fixed values instead of randn to ensure reproducible behavior.
    fn make_deterministic_weights() -> Vec<(String, Tensor)> {
        let device = candle_core::Device::Cpu;
        let vocab = 8;
        let hidden = 4;

        // Fixed-value embeddings that steer predictions away from tokens 0 and 2 (EOS).
        // Embedding rows 3..7 have large positive values so tokens 3+ are always
        // more likely than tokens 0 or 2, preventing immediate EOS termination.
        let embed_data: Vec<f32> = (0..vocab * hidden)
            .map(|i| {
                let row = i / hidden;
                let col = i % hidden;
                if row <= 2 {
                    // EOS-like tokens: small values
                    -1.0 + 0.1 * col as f32
                } else {
                    // Normal tokens: larger values
                    0.5 + 0.2 * (row as f32) + 0.1 * (col as f32)
                }
            })
            .collect();
        let embed = Tensor::new(&embed_data[..], &device)
            .unwrap()
            .reshape(&[vocab, hidden])
            .unwrap();

        // Gate and up projections: identity-ish so silu(gate)*up ≈ input
        // gate_proj: values that produce positive activations through SiLU
        let gate_data: Vec<f32> = (0..hidden * hidden)
            .map(|i| if i / hidden == i % hidden { 2.0 } else { 0.05 })
            .collect();
        let gate = Tensor::new(&gate_data[..], &device)
            .unwrap()
            .reshape(&[hidden, hidden])
            .unwrap();

        // up_proj: near-identity
        let up_data: Vec<f32> = (0..hidden * hidden)
            .map(|i| if i / hidden == i % hidden { 1.0 } else { 0.05 })
            .collect();
        let up = Tensor::new(&up_data[..], &device)
            .unwrap()
            .reshape(&[hidden, hidden])
            .unwrap();

        // down_proj: near-identity
        let down_data: Vec<f32> = (0..hidden * hidden)
            .map(|i| if i / hidden == i % hidden { 0.8 } else { 0.1 })
            .collect();
        let down = Tensor::new(&down_data[..], &device)
            .unwrap()
            .reshape(&[hidden, hidden])
            .unwrap();

        // LM head: same as embed (tied weights)
        let lm_head = embed.clone();

        vec![
            ("model.embed_tokens.weight".to_string(), embed),
            ("model.layers.0.mlp.gate_proj.weight".to_string(), gate),
            ("model.layers.0.mlp.up_proj.weight".to_string(), up),
            ("model.layers.0.mlp.down_proj.weight".to_string(), down),
            ("lm_head.weight".to_string(), lm_head),
        ]
    }

    #[test]
    fn test_speculative_acceptance_rate_identical_models() {
        // When draft and target use the SAME weights, acceptance should be 100%.
        let weights = make_deterministic_weights();
        let target = mock_loaded_model(weights.clone());
        let draft = mock_loaded_model(weights);

        let config = SpeculativeConfig {
            draft_steps: 4,
            acceptance_threshold: 0.1,
            enabled: true,
        };
        let sampling = SamplingConfig {
            temperature: 0.0,
            top_p: 1.0,
            repetition_penalty: 1.0,
        };

        // Use prompt tokens > 2 to avoid immediate EOS
        let prompt_tokens = &[3u32, 5];
        let (_generated, stats) = LocalSlmProvider::run_inference_speculative(
            prompt_tokens,
            12,
            &target,
            &draft,
            &config,
            &sampling,
        )
        .unwrap();

        // With identical models, every draft token should be accepted
        assert!(
            stats.total_draft_tokens > 0,
            "should have drafted some tokens, generated {} tokens",
            _generated.len()
        );
        assert_eq!(
            stats.accepted_tokens, stats.total_draft_tokens,
            "identical models should accept all: accepted={}, total={}",
            stats.accepted_tokens, stats.total_draft_tokens
        );
        assert!(stats.acceptance_rate > 0.99);
    }

    #[test]
    fn test_speculative_matches_normal_output_identical_models() {
        // With identical draft/target weights, speculative should produce
        // the same output as normal inference (greedy, no repetition penalty).
        let weights = make_deterministic_weights();
        let target = mock_loaded_model(weights.clone());
        let draft = mock_loaded_model(weights);

        let sampling = SamplingConfig {
            temperature: 0.0,
            top_p: 1.0,
            repetition_penalty: 1.0,
        };

        let prompt_tokens = &[3u32, 5];
        let max_tokens = 6;

        // Normal inference via forward_pass_cached
        let device = candle_core::Device::Cpu;
        let wm: std::collections::HashMap<&str, &Tensor> = target
            .weights
            .iter()
            .map(|(n, t)| (n.as_str(), t))
            .collect();
        let vocab_size = LocalSlmProvider::infer_vocab_size(&wm);
        let num_layers = LocalSlmProvider::count_layers(&wm);

        let mut cache = KvCache::new(num_layers);
        let prompt_tensor = Tensor::new(prompt_tokens, &device).unwrap();
        let prefill = LocalSlmProvider::forward_pass_cached(
            &prompt_tensor,
            &wm,
            vocab_size,
            &device,
            &mut cache,
        )
        .unwrap();
        let last = LocalSlmProvider::last_position_logits(&prefill, prompt_tokens.len()).unwrap();
        let mut normal_ids = Vec::new();
        let mut next = LocalSlmProvider::sample_token(&last, 0.0, 1.0).unwrap();
        for _ in 0..max_tokens {
            if next == 0 || next == 2 {
                break;
            }
            normal_ids.push(next);
            let tt = Tensor::new(&[next], &device).unwrap();
            let logits =
                LocalSlmProvider::forward_pass_cached(&tt, &wm, vocab_size, &device, &mut cache)
                    .unwrap();
            let ll = LocalSlmProvider::last_position_logits(&logits, 1).unwrap();
            next = LocalSlmProvider::sample_token(&ll, 0.0, 1.0).unwrap();
        }

        // Speculative inference
        let config = SpeculativeConfig {
            draft_steps: 3,
            acceptance_threshold: 0.1,
            enabled: true,
        };
        let (spec_ids, _stats) = LocalSlmProvider::run_inference_speculative(
            prompt_tokens,
            max_tokens as u32,
            &target,
            &draft,
            &config,
            &sampling,
        )
        .unwrap();

        // With identical models and greedy decoding, output should match
        assert_eq!(
            normal_ids, spec_ids,
            "speculative output should match normal: normal={normal_ids:?}, spec={spec_ids:?}"
        );
    }

    #[test]
    fn test_trim_cache() {
        let device = candle_core::Device::Cpu;
        let mut cache = KvCache::new(1);

        // Add 5 positions
        let k = Tensor::zeros(&[5, 4], DType::F32, &device).unwrap();
        let v = Tensor::zeros(&[5, 4], DType::F32, &device).unwrap();
        cache.update(0, k, v).unwrap();
        assert_eq!(cache.seq_len(), 5);

        // Trim last 2
        LocalSlmProvider::trim_cache(&mut cache, 2);
        assert_eq!(cache.seq_len(), 3);

        // Trim more than remaining
        LocalSlmProvider::trim_cache(&mut cache, 10);
        assert_eq!(cache.seq_len(), 0);
    }

    #[test]
    fn test_logits_to_probs() {
        let device = candle_core::Device::Cpu;
        let logits = Tensor::new(&[1.0f32, 2.0, 3.0], &device).unwrap();
        let probs = LocalSlmProvider::logits_to_probs(&logits).unwrap();

        // Should sum to ~1.0
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "probs should sum to 1, got {sum}");

        // Higher logit → higher prob
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_logits_to_probs_empty() {
        let device = candle_core::Device::Cpu;
        let logits = Tensor::new(&[0f32; 0], &device).unwrap();
        let probs = LocalSlmProvider::logits_to_probs(&logits).unwrap();
        assert!(probs.is_empty());
    }
}
