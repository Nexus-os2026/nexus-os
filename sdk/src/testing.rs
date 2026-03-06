//! Test harness for agent developers to test without a real kernel.

use crate::context::AgentContext;
use uuid::Uuid;

pub struct TestHarness {
    capabilities: Vec<String>,
    fuel: u64,
    agent_id: Uuid,
}

impl TestHarness {
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            fuel: 10_000,
            agent_id: Uuid::new_v4(),
        }
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    pub fn with_agent_id(mut self, agent_id: Uuid) -> Self {
        self.agent_id = agent_id;
        self
    }

    pub fn build_context(self) -> AgentContext {
        AgentContext::new(self.agent_id, self.capabilities, self.fuel)
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_kernel::errors::AgentError;

    #[test]
    fn harness_creates_working_context() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["llm.query".to_string(), "fs.read".to_string()])
            .with_fuel(500)
            .build_context();

        assert_eq!(ctx.fuel_remaining(), 500);
        assert_eq!(ctx.fuel_budget(), 500);
        assert!(ctx.require_capability("llm.query").is_ok());
        assert!(ctx.require_capability("fs.read").is_ok());
        assert!(matches!(
            ctx.require_capability("fs.write"),
            Err(AgentError::CapabilityDenied(_))
        ));

        // Operations work through harness context
        let result = ctx.llm_query("test prompt", 50);
        assert!(result.is_ok());
        assert_eq!(ctx.fuel_remaining(), 490); // 500 - 10

        let result = ctx.read_file("/tmp/test");
        assert!(result.is_ok());
        assert_eq!(ctx.fuel_remaining(), 488); // 490 - 2
    }

    #[test]
    fn harness_default_fuel() {
        let ctx = TestHarness::new().build_context();
        assert_eq!(ctx.fuel_remaining(), 10_000);
    }

    #[test]
    fn harness_custom_agent_id() {
        let id = Uuid::new_v4();
        let ctx = TestHarness::new().with_agent_id(id).build_context();
        assert_eq!(ctx.agent_id(), id);
    }

    #[test]
    fn harness_fuel_exhaustion_works() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["fs.read".to_string()])
            .with_fuel(3) // Only 3 fuel, read costs 2
            .build_context();

        assert!(ctx.read_file("/first").is_ok());
        assert_eq!(ctx.fuel_remaining(), 1);

        // Second read costs 2 but only 1 remaining
        let result = ctx.read_file("/second");
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }
}
