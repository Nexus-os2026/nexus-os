//! The core NexusAgent trait that all plugin agents implement.

use crate::context::AgentContext;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub status: String,
    pub outputs: Vec<Value>,
    pub fuel_used: u64,
}

pub trait NexusAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError>;

    fn shutdown(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError>;

    fn checkpoint(&self) -> Result<Vec<u8>, AgentError> {
        Ok(Vec::new())
    }

    fn restore(&mut self, _data: &[u8]) -> Result<(), AgentError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestHarness;
    use serde_json::json;

    struct EchoAgent {
        initialized: bool,
    }

    impl EchoAgent {
        fn new() -> Self {
            Self { initialized: false }
        }
    }

    impl NexusAgent for EchoAgent {
        fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
            ctx.require_capability("llm.query")?;
            self.initialized = true;
            Ok(())
        }

        fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
            if !self.initialized {
                return Err(AgentError::SupervisorError("not initialized".to_string()));
            }
            let response = ctx.llm_query("hello world", 100)?;
            let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();
            Ok(AgentOutput {
                status: "ok".to_string(),
                outputs: vec![json!({"response": response})],
                fuel_used,
            })
        }

        fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
            self.initialized = false;
            Ok(())
        }
    }

    #[test]
    fn agent_lifecycle_through_context() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["llm.query".to_string()])
            .with_fuel(1000)
            .build_context();

        let mut agent = EchoAgent::new();

        assert!(agent.init(&mut ctx).is_ok());
        assert!(agent.initialized);

        let output = agent.execute(&mut ctx);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.status, "ok");
        assert!(output.fuel_used > 0);

        assert!(agent.shutdown(&mut ctx).is_ok());
        assert!(!agent.initialized);
    }

    #[test]
    fn default_checkpoint_and_restore() {
        let agent = EchoAgent::new();
        let data = agent.checkpoint().unwrap();
        assert!(data.is_empty());
    }
}
