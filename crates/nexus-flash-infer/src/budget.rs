use serde::{Deserialize, Serialize};

use crate::profiler::ModelProfile;
use crate::types::HardwareInfo;

/// System memory budget for inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBudget {
    pub total_system_ram_mb: u64,
    pub os_reserved_mb: u64,
    pub app_overhead_mb: u64,
    /// Non-expert weights (attention, embeddings) — always in RAM.
    pub model_dense_mb: u64,
    /// KV cache for context window.
    pub kv_cache_mb: u64,
    /// Budget for cached MoE experts.
    pub expert_cache_mb: u64,
    /// 10% safety margin.
    pub safety_margin_mb: u64,
    /// Total available for model + KV + experts.
    pub available_for_inference_mb: u64,
}

impl MemoryBudget {
    /// Calculate optimal budget for given hardware and model.
    pub fn calculate(hw: &HardwareInfo, model: &ModelProfile, target_context_len: u32) -> Self {
        let total = hw.total_ram_mb;
        let os_reserved = Self::estimate_os_overhead(total);
        let app_overhead = 512; // Nexus OS app overhead
        let safety = total / 10; // 10% safety margin

        let model_dense = model.dense_weight_size_mb;
        let kv_cache = Self::estimate_kv_cache(model, target_context_len);

        let expert_cache = total
            .saturating_sub(os_reserved)
            .saturating_sub(app_overhead)
            .saturating_sub(safety)
            .saturating_sub(model_dense)
            .saturating_sub(kv_cache);

        let available = model_dense + kv_cache + expert_cache;

        Self {
            total_system_ram_mb: total,
            os_reserved_mb: os_reserved,
            app_overhead_mb: app_overhead,
            model_dense_mb: model_dense,
            kv_cache_mb: kv_cache,
            expert_cache_mb: expert_cache,
            safety_margin_mb: safety,
            available_for_inference_mb: available,
        }
    }

    /// Check if the model can run on this hardware at all.
    /// Needs at minimum: dense weights + KV cache for 512 tokens.
    pub fn is_feasible(&self) -> bool {
        self.expert_cache_mb > 0
    }

    /// Maximum context length that fits within budget.
    pub fn max_context_length(&self, model: &ModelProfile) -> u32 {
        if model.num_kv_heads == 0 || model.head_dim == 0 || model.num_layers == 0 {
            return 0;
        }

        let per_token_bytes =
            2u64 * model.num_layers as u64 * model.num_kv_heads as u64 * model.head_dim as u64 * 2; // f16

        if per_token_bytes == 0 {
            return 0;
        }

        let kv_budget_bytes = self.kv_cache_mb * 1024 * 1024;
        (kv_budget_bytes / per_token_bytes) as u32
    }

    /// Fraction of total experts that fit in the expert cache (0.0–1.0).
    pub fn expert_cache_ratio(&self, model: &ModelProfile) -> f32 {
        if !model.is_moe || model.single_expert_mb <= 0.0 || model.total_experts == 0 {
            return 1.0; // Dense model — all "experts" are in memory.
        }

        let experts_that_fit = (self.expert_cache_mb as f64 / model.single_expert_mb) as u32;
        let ratio = experts_that_fit as f32 / model.total_experts as f32;
        ratio.min(1.0)
    }

    fn estimate_os_overhead(total_ram_mb: u64) -> u64 {
        // Linux/macOS typically use 2–4GB, scale with total RAM.
        std::cmp::max(2048, total_ram_mb / 8)
    }

    fn estimate_kv_cache(model: &ModelProfile, ctx_len: u32) -> u64 {
        // KV cache: 2 (K+V) × layers × kv_heads × head_dim × ctx_len × sizeof(f16)
        let per_token_bytes =
            2u64 * model.num_layers as u64 * model.num_kv_heads as u64 * model.head_dim as u64 * 2; // f16
        (per_token_bytes * ctx_len as u64) / (1024 * 1024)
    }
}
