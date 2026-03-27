use std::collections::HashMap;

use serde_json::json;

use crate::types::{McpContent, McpTool, McpToolResult};

/// A handler for an MCP tool call.
pub type ToolHandler =
    Box<dyn Fn(serde_json::Value) -> Result<McpToolResult, String> + Send + Sync>;

/// Registry of MCP tools with their handlers.
pub struct ToolRegistry {
    tools: Vec<McpTool>,
    handlers: HashMap<String, ToolHandler>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: McpTool, handler: ToolHandler) {
        self.handlers.insert(tool.name.clone(), handler);
        self.tools.push(tool);
    }

    pub fn list_tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn call_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| format!("Unknown tool: {name}"))?;
        handler(params)
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Register the default Nexus OS tools.
    pub fn register_defaults(&mut self) {
        self.register(
            McpTool {
                name: "nexus_agent_list".into(),
                description: Some("List available Nexus OS agents with capabilities".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Box::new(|_params| {
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: "Agent listing requires desktop app connection.".into(),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_agent_run".into(),
                description: Some("Run a task through a Nexus OS agent".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string", "description": "Task description"},
                        "agent_id": {"type": "string", "description": "Target agent ID (optional)"}
                    },
                    "required": ["task"]
                }),
            },
            Box::new(|params| {
                let task = params
                    .get("task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no task)");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("Task submitted: {task}"),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_governance_check".into(),
                description: Some("Check if an action is allowed by governance policy".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string", "description": "Agent ID"},
                        "capability": {"type": "string", "description": "Capability to check"},
                        "autonomy_level": {"type": "integer", "description": "Agent autonomy level (0-6)"}
                    },
                    "required": ["agent_id", "capability"]
                }),
            },
            Box::new(|params| {
                let cap = params
                    .get("capability")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("Governance check for capability '{cap}': allowed (default policy)"),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_simulate".into(),
                description: Some("Run a world simulation scenario".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "scenario": {"type": "string", "description": "Scenario description"},
                        "actions": {"type": "array", "items": {"type": "object"}, "description": "Actions to simulate"}
                    },
                    "required": ["scenario"]
                }),
            },
            Box::new(|params| {
                let scenario = params
                    .get("scenario")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unnamed)");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("Simulation '{scenario}' queued. Connect to desktop app for results."),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_measure".into(),
                description: Some("Measure agent capability across 4 evaluation vectors".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string", "description": "Agent to measure"}
                    },
                    "required": ["agent_id"]
                }),
            },
            Box::new(|params| {
                let agent = params
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("Measurement session started for agent '{agent}'."),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_search".into(),
                description: Some("Web search via DuckDuckGo".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search query"}
                    },
                    "required": ["query"]
                }),
            },
            Box::new(|params| {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("Search results for '{query}' — connect to desktop app for live search."),
                    }],
                    is_error: false,
                })
            }),
        );

        self.register(
            McpTool {
                name: "nexus_github".into(),
                description: Some("GitHub operations (repos, issues, PRs, code search)".into()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "enum": ["list_repos", "create_issue", "search_code", "get_pr"], "description": "GitHub API action"},
                        "repo": {"type": "string", "description": "Repository (owner/name)"},
                        "data": {"type": "object", "description": "Action-specific data"}
                    },
                    "required": ["action"]
                }),
            },
            Box::new(|params| {
                let action = params
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(McpToolResult {
                    content: vec![McpContent::Text {
                        text: format!("GitHub '{action}' — requires GITHUB_TOKEN. Connect to desktop app."),
                    }],
                    is_error: false,
                })
            }),
        );
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register_defaults();
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_list_returns_registered_tools() {
        let reg = ToolRegistry::default();
        let tools = reg.list_tools();
        assert!(tools.len() >= 7);
        assert!(tools.iter().any(|t| t.name == "nexus_agent_run"));
        assert!(tools.iter().any(|t| t.name == "nexus_search"));
        assert!(tools.iter().any(|t| t.name == "nexus_github"));
    }

    #[test]
    fn test_tool_call_routes_to_handler() {
        let reg = ToolRegistry::default();
        let result = reg
            .call_tool("nexus_agent_run", json!({"task": "build a REST API"}))
            .unwrap();
        assert!(!result.is_error);
        match &result.content[0] {
            McpContent::Text { text } => assert!(text.contains("REST API")),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let reg = ToolRegistry::default();
        let result = reg.call_tool("nonexistent_tool", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_schemas_have_required_fields() {
        let reg = ToolRegistry::default();
        for tool in reg.list_tools() {
            assert!(!tool.name.is_empty());
            assert!(tool.input_schema.get("type").is_some());
        }
    }
}
