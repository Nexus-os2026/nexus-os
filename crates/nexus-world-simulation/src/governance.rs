use serde::{Deserialize, Serialize};

/// Governance policy for simulation access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationPolicy {
    pub min_autonomy_level: u8,
    pub max_steps: u32,
    pub max_concurrent_per_agent: usize,
    pub allow_branching: bool,
    pub cost_per_step: u64,
    pub base_cost: u64,
}

impl Default for SimulationPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 3,
            max_steps: 50,
            max_concurrent_per_agent: 3,
            allow_branching: true,
            cost_per_step: 500_000, // 0.5 NXC per step
            base_cost: 2_000_000,   // 2 NXC base cost
        }
    }
}

impl SimulationPolicy {
    /// Calculate total token cost for a simulation.
    pub fn calculate_cost(&self, step_count: u32) -> u64 {
        self.base_cost + (self.cost_per_step * step_count as u64)
    }

    /// Check if an agent is authorized to simulate.
    pub fn check_authorization(&self, autonomy_level: u8) -> Result<(), String> {
        if autonomy_level < self.min_autonomy_level {
            return Err(format!(
                "Simulation requires L{}+, agent is L{}",
                self.min_autonomy_level, autonomy_level,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_min_autonomy() {
        let policy = SimulationPolicy::default();
        assert!(policy.check_authorization(2).is_err()); // L2 denied
        assert!(policy.check_authorization(3).is_ok()); // L3 allowed
        assert!(policy.check_authorization(5).is_ok()); // L5 allowed
    }

    #[test]
    fn test_economy_cost_calculation() {
        let policy = SimulationPolicy::default();
        // base (2 NXC) + 5 steps × 0.5 NXC = 4.5 NXC = 4_500_000 micro
        assert_eq!(policy.calculate_cost(5), 4_500_000);
        // base (2 NXC) + 0 steps = 2 NXC
        assert_eq!(policy.calculate_cost(0), 2_000_000);
    }
}
