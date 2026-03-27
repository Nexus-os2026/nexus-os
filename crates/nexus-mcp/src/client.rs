use serde::{Deserialize, Serialize};

use crate::types::{JsonRpcResponse, McpTool};

/// Configuration for connecting to an external MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
}

/// MCP client for consuming external MCP servers.
pub struct McpClient {
    servers: Vec<ExternalServerConfig>,
    discovered_tools: Vec<(String, McpTool)>,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            discovered_tools: Vec::new(),
        }
    }

    pub fn add_server(&mut self, config: ExternalServerConfig) {
        self.servers.push(config);
    }

    pub fn remove_server(&mut self, server_id: &str) {
        self.servers.retain(|s| s.id != server_id);
        self.discovered_tools.retain(|(sid, _)| sid != server_id);
    }

    pub fn list_servers(&self) -> &[ExternalServerConfig] {
        &self.servers
    }

    /// Discover tools from a server by sending initialize + tools/list.
    pub fn discover_tools(&mut self, server_id: &str) -> Result<Vec<McpTool>, String> {
        let config = self
            .servers
            .iter()
            .find(|s| s.id == server_id)
            .ok_or_else(|| format!("Server not found: {server_id}"))?
            .clone();

        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "nexus-os", "version": "9.6.0"}
            }
        });

        let _init_resp = self.send_to_server(&config, &init_req)?;

        let list_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });

        let list_resp = self.send_to_server(&config, &list_req)?;
        let tools_json = list_resp
            .result
            .and_then(|r| r.get("tools").cloned())
            .unwrap_or(serde_json::json!([]));

        let tools: Vec<McpTool> =
            serde_json::from_value(tools_json).map_err(|e| format!("Parse tools: {e}"))?;

        // Cache discovered tools
        self.discovered_tools.retain(|(sid, _)| sid != server_id);
        for tool in &tools {
            self.discovered_tools.push((server_id.into(), tool.clone()));
        }

        Ok(tools)
    }

    /// Call a tool on an external server.
    pub fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let config = self
            .servers
            .iter()
            .find(|s| s.id == server_id)
            .ok_or_else(|| format!("Server not found: {server_id}"))?
            .clone();

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let resp = self.send_to_server(&config, &req)?;
        resp.result.ok_or_else(|| {
            resp.error
                .map(|e| e.message)
                .unwrap_or_else(|| "Unknown error".into())
        })
    }

    /// Get all discovered tools across all servers.
    pub fn all_tools(&self) -> &[(String, McpTool)] {
        &self.discovered_tools
    }

    fn send_to_server(
        &self,
        config: &ExternalServerConfig,
        request: &serde_json::Value,
    ) -> Result<JsonRpcResponse, String> {
        let encoded = serde_json::to_string(request).map_err(|e| format!("json: {e}"))?;

        let mut cmd = std::process::Command::new(&config.command);
        cmd.args(&config.args);
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            writeln!(stdin, "{encoded}").map_err(|e| format!("write: {e}"))?;
        }

        let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("");

        serde_json::from_str(first_line).map_err(|e| format!("parse response: {e}"))
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_add_remove_server() {
        let mut client = McpClient::new();
        client.add_server(ExternalServerConfig {
            id: "test".into(),
            name: "Test Server".into(),
            command: "echo".into(),
            args: vec![],
            env: std::collections::HashMap::new(),
        });
        assert_eq!(client.list_servers().len(), 1);
        client.remove_server("test");
        assert_eq!(client.list_servers().len(), 0);
    }

    #[test]
    fn test_client_call_nonexistent_server() {
        let client = McpClient::new();
        let result = client.call_tool("nonexistent", "tool", serde_json::json!({}));
        assert!(result.is_err());
    }
}
