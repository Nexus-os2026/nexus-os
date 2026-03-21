use crate::types::InferenceMetrics;
use nexus_llama_bridge::PerfStats;

/// Input parameters for creating a metrics snapshot.
pub struct MetricsInput<'a> {
    pub session_id: &'a str,
    pub stats: &'a PerfStats,
    pub memory_used_mb: u64,
    pub memory_budget_mb: u64,
    pub context_used: u32,
    pub context_max: u32,
    pub total_tokens: u64,
    pub uptime_secs: f64,
}

/// Create metrics snapshot from a generation run.
pub fn metrics_from_stats(input: &MetricsInput<'_>) -> InferenceMetrics {
    let utilization = if input.memory_budget_mb > 0 {
        input.memory_used_mb as f64 / input.memory_budget_mb as f64
    } else {
        0.0
    };

    InferenceMetrics {
        session_id: input.session_id.to_string(),
        tokens_per_second: input.stats.tokens_per_second,
        prompt_tokens_per_second: input.stats.prompt_tokens_per_second,
        memory_used_mb: input.memory_used_mb,
        memory_budget_mb: input.memory_budget_mb,
        memory_utilization: utilization.min(1.0),
        expert_cache_hit_rate: 0.0, // Populated by backend if MoE
        io_read_mb_per_sec: 0.0,    // Populated by OS-level monitoring
        cpu_utilization: 0.0,       // Populated by OS-level monitoring
        context_used: input.context_used,
        context_max: input.context_max,
        total_tokens_generated: input.total_tokens,
        uptime_seconds: input.uptime_secs,
    }
}

/// Aggregate metrics across multiple sessions.
pub fn aggregate_metrics(metrics: &[InferenceMetrics]) -> AggregateMetrics {
    let total_memory_used: u64 = metrics.iter().map(|m| m.memory_used_mb).sum();
    let total_tokens: u64 = metrics.iter().map(|m| m.total_tokens_generated).sum();
    let avg_tok_s = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(|m| m.tokens_per_second).sum::<f64>() / metrics.len() as f64
    };

    AggregateMetrics {
        active_sessions: metrics.len(),
        total_memory_used_mb: total_memory_used,
        total_tokens_generated: total_tokens,
        average_tokens_per_second: avg_tok_s,
    }
}

/// Aggregate metrics across all active sessions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AggregateMetrics {
    pub active_sessions: usize,
    pub total_memory_used_mb: u64,
    pub total_tokens_generated: u64,
    pub average_tokens_per_second: f64,
}
