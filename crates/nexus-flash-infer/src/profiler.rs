use nexus_llama_bridge::ModelMetadata;
use serde::{Deserialize, Serialize};

use crate::budget::MemoryBudget;
use crate::types::HardwareInfo;

/// Profile of a model's memory and compute characteristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub name: String,
    pub architecture: String,
    pub total_params: u64,
    pub file_size_mb: u64,
    pub quantization: String,

    // MoE-specific
    pub is_moe: bool,
    pub num_experts: u32,
    pub num_active_experts: u32,
    pub num_layers: u32,
    pub num_kv_heads: u32,
    pub head_dim: u32,

    // Memory estimates (MB)
    pub dense_weight_size_mb: u64,
    pub expert_weight_size_mb: u64,
    pub single_expert_mb: f64,
    pub total_experts: u32,

    // Compute estimates
    pub active_params: u64,
    pub flops_per_token: u64,
}

impl ModelProfile {
    /// Create a profile from model metadata.
    /// Uses heuristics to estimate MoE layout and memory breakdown.
    pub fn from_metadata(meta: &ModelMetadata) -> Self {
        let is_moe = meta.is_moe;
        let num_experts = meta.num_experts.unwrap_or(0);
        let num_active = meta
            .num_active_experts
            .unwrap_or(if is_moe { 2 } else { 0 });
        let num_layers = meta.num_layers;
        let file_size_mb = meta.file_size_bytes / (1024 * 1024);

        // Estimate dense vs expert weight split
        let (dense_mb, expert_mb, single_expert_mb) = if is_moe && num_experts > 0 {
            // Dense weights: ~10-15% of total for large MoE models
            let dense_ratio = if num_experts > 128 { 0.03 } else { 0.10 };
            let dense = (file_size_mb as f64 * dense_ratio) as u64;
            let expert_total = file_size_mb.saturating_sub(dense);
            let total_experts = num_experts * num_layers;
            let single = if total_experts > 0 {
                expert_total as f64 / total_experts as f64
            } else {
                0.0
            };
            (dense, expert_total, single)
        } else {
            (file_size_mb, 0, 0.0)
        };

        // Estimate active parameters for MoE
        let active_params = if is_moe && num_experts > 0 {
            let expert_ratio = num_active as f64 / num_experts as f64;
            let dense_params = (meta.total_params as f64 * 0.1) as u64;
            let active_expert_params =
                ((meta.total_params - dense_params) as f64 * expert_ratio) as u64;
            dense_params + active_expert_params
        } else {
            meta.total_params
        };

        // Rough FLOPs estimate: ~2x active params per token
        let flops_per_token = active_params * 2;

        // Estimate head dimensions from embedding size and layers
        // Common default for modern transformer architectures
        let head_dim = 128;

        let num_kv_heads = if meta.embedding_size > 0 && head_dim > 0 {
            (meta.embedding_size / head_dim).max(1)
        } else {
            32
        };

        Self {
            name: meta.architecture.clone(),
            architecture: meta.architecture.clone(),
            total_params: meta.total_params,
            file_size_mb,
            quantization: meta.quantization.clone(),
            is_moe,
            num_experts,
            num_active_experts: num_active,
            num_layers,
            num_kv_heads,
            head_dim,
            dense_weight_size_mb: dense_mb,
            expert_weight_size_mb: expert_mb,
            single_expert_mb,
            total_experts: if is_moe { num_experts * num_layers } else { 0 },
            active_params,
            flops_per_token,
        }
    }

    /// Estimate performance on given hardware with a memory budget.
    pub fn estimate_performance(
        &self,
        hw: &HardwareInfo,
        budget: &MemoryBudget,
    ) -> PerformanceEstimate {
        let memory_feasible = budget.is_feasible();

        if !memory_feasible {
            return PerformanceEstimate {
                estimated_tok_per_sec: 0.0,
                estimated_prompt_tok_per_sec: 0.0,
                expert_cache_hit_rate: 0.0,
                io_bottlenecked: true,
                memory_feasible: false,
                max_context_length: 0,
                warnings: vec!["Model does not fit in available memory".into()],
            };
        }

        let max_ctx = budget.max_context_length(self);
        let cache_ratio = budget.expert_cache_ratio(self);

        // Estimate tok/s based on active params and memory bandwidth
        // Rough heuristic: DDR4 ~50GB/s, DDR5 ~80GB/s, assume ~60GB/s average
        let mem_bandwidth_gb_s = 60.0;
        let bytes_per_token = (self.active_params as f64 * 0.5) / 1e9; // Q4 ≈ 0.5 bytes/param
        let compute_tok_s = mem_bandwidth_gb_s / bytes_per_token;

        // If MoE and not all experts cached, IO becomes bottleneck
        let (tok_s, io_bottlenecked) = if self.is_moe && cache_ratio < 0.95 {
            let ssd_bandwidth = hw.ssd_read_speed_mb_s as f64 / 1024.0; // GB/s
            let expert_miss_rate = 1.0 - cache_ratio as f64;
            let expert_bytes = self.single_expert_mb / 1024.0; // GB
            let io_time_per_token =
                expert_miss_rate * self.num_active_experts as f64 * expert_bytes / ssd_bandwidth;
            let io_limited_tok_s = if io_time_per_token > 0.0 {
                1.0 / io_time_per_token
            } else {
                compute_tok_s
            };
            (
                io_limited_tok_s.min(compute_tok_s),
                io_limited_tok_s < compute_tok_s,
            )
        } else {
            (compute_tok_s, false)
        };

        // Prompt processing is typically faster (batched)
        let prompt_tok_s = tok_s * (hw.cpu_cores as f64).min(8.0);

        let mut warnings = Vec::new();
        if io_bottlenecked {
            warnings.push(format!(
                "Expert streaming limited by {:?} SSD ({} MB/s)",
                hw.ssd_type, hw.ssd_read_speed_mb_s
            ));
        }
        if max_ctx < 2048 {
            warnings.push(format!(
                "Context limited to {} tokens due to memory constraints",
                max_ctx
            ));
        }

        PerformanceEstimate {
            estimated_tok_per_sec: tok_s,
            estimated_prompt_tok_per_sec: prompt_tok_s,
            expert_cache_hit_rate: cache_ratio as f64,
            io_bottlenecked,
            memory_feasible,
            max_context_length: max_ctx,
            warnings,
        }
    }
}

/// Performance estimate for a model on specific hardware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEstimate {
    pub estimated_tok_per_sec: f64,
    pub estimated_prompt_tok_per_sec: f64,
    pub expert_cache_hit_rate: f64,
    pub io_bottlenecked: bool,
    pub memory_feasible: bool,
    pub max_context_length: u32,
    pub warnings: Vec<String>,
}
