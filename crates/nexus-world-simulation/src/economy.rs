use crate::governance::SimulationPolicy;

/// Simulation economy — calculates token burn for simulations and branches.
pub struct SimulationEconomy;

impl SimulationEconomy {
    /// Calculate burn for a full simulation.
    pub fn calculate_burn(policy: &SimulationPolicy, step_count: u32) -> u64 {
        policy.calculate_cost(step_count)
    }

    /// Calculate burn for a branch (cheaper — only the divergent portion).
    pub fn calculate_branch_burn(policy: &SimulationPolicy, remaining_steps: u32) -> u64 {
        let branch_cost = policy.cost_per_step * remaining_steps as u64;
        let branch_base = policy.base_cost / 2; // 50% of full base
        branch_base + branch_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::SimulationPolicy;

    #[test]
    fn test_branch_cost_cheaper() {
        let policy = SimulationPolicy::default();
        let full_cost = SimulationEconomy::calculate_burn(&policy, 10);
        let branch_cost = SimulationEconomy::calculate_branch_burn(&policy, 10);
        assert!(
            branch_cost < full_cost,
            "Branch ({branch_cost}) should be cheaper than full ({full_cost})"
        );
    }
}
