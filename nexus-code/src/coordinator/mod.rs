//! Governed multi-agent coordinator — Research → Synthesize → Implement → Verify.

pub mod fuel_manager;

use serde::{Deserialize, Serialize};

/// Coordinator configuration.
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    pub research_workers: usize,
    pub research_fuel: u64,
    pub implementation_fuel: u64,
    pub verification_fuel: u64,
    pub worker_max_turns: u32,
    pub task: String,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            research_workers: 2,
            research_fuel: 8_000,
            implementation_fuel: 10_000,
            verification_fuel: 5_000,
            worker_max_turns: 8,
            task: String::new(),
        }
    }
}

/// Result of a coordinator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorResult {
    pub success: bool,
    pub summary: String,
    pub research_findings: Vec<String>,
    pub implementation_result: Option<String>,
    pub verification_result: Option<String>,
    pub total_fuel_consumed: u64,
    pub worker_count: usize,
    pub fuel_summary: String,
}

/// Compute total fuel needed for a coordinator run.
pub fn total_fuel_needed(config: &CoordinatorConfig) -> u64 {
    (config.research_workers as u64 * config.research_fuel)
        + config.implementation_fuel
        + config.verification_fuel
}
