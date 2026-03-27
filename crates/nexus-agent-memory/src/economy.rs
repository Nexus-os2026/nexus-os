use crate::governance::MemoryPolicy;

pub struct MemoryEconomy;

impl MemoryEconomy {
    pub fn store_cost(policy: &MemoryPolicy) -> u64 {
        policy.store_cost
    }

    pub fn query_cost(policy: &MemoryPolicy) -> u64 {
        policy.query_cost
    }

    pub fn context_cost(policy: &MemoryPolicy) -> u64 {
        policy.context_cost
    }

    /// Consolidation is free — it reduces memory usage.
    pub fn consolidation_cost() -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economy_costs() {
        let policy = MemoryPolicy::default();
        assert_eq!(MemoryEconomy::store_cost(&policy), 100_000);
        assert_eq!(MemoryEconomy::query_cost(&policy), 50_000);
        assert_eq!(MemoryEconomy::context_cost(&policy), 200_000);
        assert_eq!(MemoryEconomy::consolidation_cost(), 0);
    }
}
