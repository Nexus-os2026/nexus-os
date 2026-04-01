//! SubAgentTool — spawn governed sub-agents via the tool-calling protocol.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Spawn a governed sub-agent to handle a subtask.
pub struct SubAgentTool;

#[async_trait]
impl NxTool for SubAgentTool {
    fn name(&self) -> &str {
        "sub_agent"
    }

    fn description(&self) -> &str {
        "Spawn a governed sub-agent to handle a subtask independently. \
         The sub-agent gets its own identity, a fuel budget sliced from yours, \
         and scoped capabilities. Use for parallel investigation or focused subtasks."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Task description for the sub-agent"
                },
                "fuel_budget": {
                    "type": "integer",
                    "description": "Fuel units to allocate (default: 5000, max: 10000)"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum turns (default: 5, max: 10)"
                }
            },
            "required": ["task"]
        })
    }

    fn estimated_fuel(&self, input: &serde_json::Value) -> u64 {
        input
            .get("fuel_budget")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000)
            .min(10_000)
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::ProcessSpawn)
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        // Actual spawning is handled at the agent loop level where
        // governance kernel access is available. This tool declaration
        // captures parameters for the agent loop to act on.
        let task = input
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("<no task>");
        let fuel = input
            .get("fuel_budget")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000);
        let turns = input.get("max_turns").and_then(|v| v.as_u64()).unwrap_or(5);

        ToolResult::error(format!(
            "Sub-agent spawning is handled at the agent loop level. \
             Task: '{}', Fuel: {}, Turns: {}",
            task, fuel, turns
        ))
    }
}
