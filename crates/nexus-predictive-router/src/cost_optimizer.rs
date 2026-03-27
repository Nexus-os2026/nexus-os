//! Cost/latency-aware routing.

use crate::model_capability::ModelCapabilityProfile;
use serde::{Deserialize, Serialize};

/// Cost constraint for routing decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConstraint {
    /// Maximum cost per 1K input tokens.
    pub max_cost_per_1k: f64,
    /// Maximum acceptable latency in ms.
    pub max_latency_ms: u64,
    /// Prefer local models over cloud.
    pub prefer_local: bool,
}

impl Default for CostConstraint {
    fn default() -> Self {
        Self {
            max_cost_per_1k: f64::MAX,
            max_latency_ms: u64::MAX,
            prefer_local: true,
        }
    }
}

/// Filter models by cost and latency constraints.
pub fn filter_by_constraints<'a>(
    models: &'a [&'a ModelCapabilityProfile],
    constraints: &CostConstraint,
) -> Vec<&'a ModelCapabilityProfile> {
    let mut filtered: Vec<&ModelCapabilityProfile> = models
        .iter()
        .filter(|m| {
            m.cost_per_1k_input <= constraints.max_cost_per_1k
                && m.avg_latency_ms <= constraints.max_latency_ms
        })
        .copied()
        .collect();

    if constraints.prefer_local {
        filtered.sort_by(|a, b| {
            // Local models first, then by cost
            b.is_local.cmp(&a.is_local).then_with(|| {
                a.cost_per_1k_input
                    .partial_cmp(&b.cost_per_1k_input)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
    }

    filtered
}
