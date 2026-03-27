//! Adapter that wraps a Nexus OS agent for capability measurement.

use crate::evaluation::runner::AgentResponse;

/// Type alias for agent invocation function.
type InvokeFn = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Adapter that wraps any agent invocation mechanism for measurement.
pub struct AgentAdapter {
    /// Agent identifier.
    pub agent_id: String,
    /// Agent autonomy level (L0–L6).
    pub autonomy_level: u8,
    /// The function that invokes the agent with a prompt and returns text.
    invoke_fn: Box<InvokeFn>,
}

impl AgentAdapter {
    /// Create a new adapter with a custom invoke function.
    pub fn new(
        agent_id: String,
        autonomy_level: u8,
        invoke_fn: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            agent_id,
            autonomy_level,
            invoke_fn: Box::new(invoke_fn),
        }
    }

    /// Send a test problem to the agent and capture the response.
    pub fn evaluate(&self, problem_text: &str) -> Result<AgentResponse, String> {
        let start = std::time::Instant::now();
        let response_text = (self.invoke_fn)(problem_text)?;
        let elapsed = start.elapsed();

        Ok(AgentResponse {
            response_text,
            reasoning_trace: None,
            elapsed_ms: elapsed.as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_adapter_captures_response() {
        let adapter = AgentAdapter::new("test-agent".into(), 3, |prompt| {
            Ok(format!("Response to: {prompt}"))
        });

        let response = adapter.evaluate("What is 2+2?").unwrap();
        assert!(response.response_text.contains("What is 2+2?"));
        assert!(response.elapsed_ms < 1000);
    }

    #[test]
    fn test_agent_adapter_propagates_error() {
        let adapter = AgentAdapter::new("fail-agent".into(), 1, |_| Err("timeout".into()));

        let result = adapter.evaluate("test");
        assert!(result.is_err());
    }
}
