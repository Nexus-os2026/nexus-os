use serde::{Deserialize, Serialize};

pub const MEMORY_CAPABILITY: &str = "agent_memory";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPolicy {
    pub min_autonomy_level: u8,
    pub max_memories_per_agent: usize,
    pub max_memory_size_bytes: u64,
    pub consolidation_enabled: bool,
    pub persistence_enabled: bool,
    /// Cost to store a new memory (micronexus).
    pub store_cost: u64,
    /// Cost to query memories (micronexus).
    pub query_cost: u64,
    /// Cost to build context (micronexus).
    pub context_cost: u64,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 1,
            max_memories_per_agent: 1000,
            max_memory_size_bytes: 50 * 1024 * 1024,
            consolidation_enabled: true,
            persistence_enabled: true,
            store_cost: 100_000,
            query_cost: 50_000,
            context_cost: 200_000,
        }
    }
}

impl MemoryPolicy {
    pub fn check_authorization(&self, autonomy_level: u8) -> Result<(), String> {
        if autonomy_level < self.min_autonomy_level {
            return Err(format!(
                "Memory requires L{}+, agent is L{}",
                self.min_autonomy_level, autonomy_level
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
        let policy = MemoryPolicy::default();
        assert!(policy.check_authorization(0).is_err());
        assert!(policy.check_authorization(1).is_ok());
        assert!(policy.check_authorization(2).is_ok());
    }
}
