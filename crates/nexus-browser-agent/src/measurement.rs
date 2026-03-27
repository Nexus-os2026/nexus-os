//! Capability measurement integration — scoring browser task execution.

use serde::{Deserialize, Serialize};

/// Score a browser task result for capability measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTaskScore {
    pub task_completed: bool,
    pub steps_taken: usize,
    pub steps_budget: usize,
    pub efficiency: f64,
    pub score: f64,
}

/// Score a browser task based on completion and efficiency.
pub fn score_browser_task(
    completed: bool,
    steps_taken: usize,
    max_steps: usize,
) -> BrowserTaskScore {
    let efficiency = if max_steps > 0 && steps_taken > 0 {
        1.0 - (steps_taken as f64 / max_steps as f64).min(1.0)
    } else {
        0.0
    };

    let score = if completed {
        0.5 + (efficiency * 0.5) // 50% for completion + up to 50% for efficiency
    } else {
        efficiency * 0.3 // Up to 30% partial credit for progress
    };

    BrowserTaskScore {
        task_completed: completed,
        steps_taken,
        steps_budget: max_steps,
        efficiency,
        score,
    }
}
