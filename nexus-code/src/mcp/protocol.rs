//! MCP protocol handler — initialize, tools/list, tools/call.

use super::jsonrpc::JsonRpcRequest;
use super::transport::McpTransportTrait;
use serde::{Deserialize, Serialize};

/// A tool discovered from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// An active MCP server connection.
pub struct McpConnection {
    pub server_name: String,
    transport: Box<dyn McpTransportTrait>,
    request_id: u64,
    pub server_capabilities: serde_json::Value,
    pub tools: Vec<McpDiscoveredTool>,
}

impl McpConnection {
    /// Connect to an MCP server and perform the initialization handshake.
    pub async fn connect(
        server_name: &str,
        transport: Box<dyn McpTransportTrait>,
    ) -> Result<Self, crate::error::NxError> {
        let mut conn = Self {
            server_name: server_name.to_string(),
            transport,
            request_id: 0,
            server_capabilities: serde_json::Value::Null,
            tools: Vec::new(),
        };

        conn.initialize().await?;
        conn.discover_tools().await?;
        Ok(conn)
    }

    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    async fn initialize(&mut self) -> Result<(), crate::error::NxError> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(
            id,
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "clientInfo": {
                    "name": "nexus-code",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        );

        let response = self.transport.send_request(request).await?;
        let result = response.result_or_error().map_err(|e| {
            crate::error::NxError::ConfigError(format!("MCP initialize failed: {}", e))
        })?;

        self.server_capabilities = result
            .get("capabilities")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        self.transport
            .send_notification("notifications/initialized", None)
            .await?;

        tracing::info!(server = %self.server_name, "MCP server initialized");
        Ok(())
    }

    async fn discover_tools(&mut self) -> Result<(), crate::error::NxError> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "tools/list", None);
        let response = self.transport.send_request(request).await?;
        let result = response.result_or_error().map_err(|e| {
            crate::error::NxError::ConfigError(format!("MCP tools/list failed: {}", e))
        })?;

        if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                if let (Some(name), Some(description)) = (
                    tool.get("name").and_then(|n| n.as_str()),
                    tool.get("description").and_then(|d| d.as_str()),
                ) {
                    self.tools.push(McpDiscoveredTool {
                        name: name.to_string(),
                        description: description.to_string(),
                        input_schema: tool
                            .get("inputSchema")
                            .cloned()
                            .unwrap_or(serde_json::json!({"type": "object"})),
                    });
                }
            }
        }

        tracing::info!(
            server = %self.server_name,
            tool_count = self.tools.len(),
            "MCP tools discovered"
        );
        Ok(())
    }

    /// Call a tool on this MCP server.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, crate::error::NxError> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(
            id,
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            })),
        );

        let response = self.transport.send_request(request).await?;
        let result = response.result_or_error().map_err(|e| {
            crate::error::NxError::ConfigError(format!("MCP tools/call failed: {}", e))
        })?;

        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            let text: Vec<String> = content
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect();
            Ok(text.join("\n"))
        } else {
            Ok(serde_json::to_string_pretty(result).unwrap_or_default())
        }
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), crate::error::NxError> {
        self.transport.close().await
    }
}
