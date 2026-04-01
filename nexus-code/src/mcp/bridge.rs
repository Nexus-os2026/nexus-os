//! MCP tool wrapper — wraps MCP tools as governed NxTools.

use crate::tools::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;

/// Wraps an MCP tool as a governed NxTool.
/// Each MCP tool goes through the full governance pipeline.
pub struct McpToolWrapper {
    pub info: super::McpToolInfo,
}

impl McpToolWrapper {
    /// Infer the governance capability from the tool's name and description.
    pub fn infer_capability(&self) -> crate::governance::Capability {
        let name_lower = self.info.tool_name.to_lowercase();
        let desc_lower = self.info.description.to_lowercase();

        if name_lower.contains("read")
            || desc_lower.contains("read")
            || desc_lower.contains("get")
            || desc_lower.contains("list")
            || desc_lower.contains("search")
        {
            crate::governance::Capability::FileRead
        } else if name_lower.contains("write")
            || name_lower.contains("create")
            || name_lower.contains("update")
            || desc_lower.contains("write")
            || desc_lower.contains("modify")
        {
            crate::governance::Capability::FileWrite
        } else if name_lower.contains("exec")
            || name_lower.contains("run")
            || name_lower.contains("shell")
            || desc_lower.contains("execute")
        {
            crate::governance::Capability::ShellExecute
        } else {
            crate::governance::Capability::NetworkAccess
        }
    }
}

#[async_trait]
impl NxTool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.info.tool_name
    }

    fn description(&self) -> &str {
        &self.info.description
    }

    fn input_schema(&self) -> serde_json::Value {
        self.info.input_schema.clone()
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        15 // MCP tools have slightly higher fuel cost (network overhead)
    }

    async fn execute(&self, _input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::error(format!(
            "MCP tool {}/{} not yet connected (Session 6)",
            self.info.server_name, self.info.tool_name
        ))
    }
}
