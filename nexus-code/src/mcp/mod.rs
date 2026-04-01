//! MCP (Model Context Protocol) tool bridge.
//!
//! MCP tools from external servers are treated as first-class governed tools.
//! Each MCP tool is individually classified through the governance pipeline.

pub mod bridge;
pub mod jsonrpc;
pub mod protocol;
pub mod transport;

use serde::{Deserialize, Serialize};

/// Configuration for an MCP server connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name (for display and audit).
    pub name: String,
    /// Transport type.
    pub transport: McpTransport,
    /// Capability scope override.
    pub capability_scope: Option<crate::governance::CapabilityScope>,
}

/// MCP transport mechanism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpTransport {
    /// stdio-based MCP server (command to spawn).
    Stdio { command: String, args: Vec<String> },
    /// SSE-based MCP server (URL).
    Sse { url: String },
}

/// Discovered tool from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP connection manager with real protocol support.
pub struct McpManager {
    connections: Vec<protocol::McpConnection>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    /// Connect to an MCP server.
    pub async fn connect(&mut self, config: &McpServerConfig) -> Result<(), crate::error::NxError> {
        let mcp_transport: Box<dyn transport::McpTransportTrait> = match &config.transport {
            McpTransport::Stdio { command, args } => {
                Box::new(transport::StdioTransport::spawn(command, args).await?)
            }
            McpTransport::Sse { url } => Box::new(transport::SseTransport::new(url)),
        };

        let connection = protocol::McpConnection::connect(&config.name, mcp_transport).await?;

        tracing::info!(
            server = %config.name,
            tools = connection.tools.len(),
            "Connected to MCP server"
        );

        self.connections.push(connection);
        Ok(())
    }

    /// Connect to all configured servers (logs warnings for failures).
    pub async fn connect_all(&mut self, configs: &[McpServerConfig]) {
        for config in configs {
            if let Err(e) = self.connect(config).await {
                tracing::warn!(
                    server = %config.name,
                    error = %e,
                    "Failed to connect to MCP server"
                );
            }
        }
    }

    /// Get all discovered tools across all servers.
    pub fn all_tools(&self) -> Vec<McpToolInfo> {
        self.connections
            .iter()
            .flat_map(|conn| {
                conn.tools.iter().map(|t| McpToolInfo {
                    server_name: conn.server_name.clone(),
                    tool_name: format!("mcp_{}_{}", conn.server_name, t.name),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                })
            })
            .collect()
    }

    /// Call a tool on the appropriate server.
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<String, crate::error::NxError> {
        let conn = self
            .connections
            .iter_mut()
            .find(|c| c.server_name == server_name)
            .ok_or_else(|| {
                crate::error::NxError::ConfigError(format!(
                    "MCP server '{}' not connected",
                    server_name
                ))
            })?;

        conn.call_tool(tool_name, input).await
    }

    /// Get total tool count across all servers.
    pub fn tool_count(&self) -> usize {
        self.connections.iter().map(|c| c.tools.len()).sum()
    }

    /// Get connected server count.
    pub fn server_count(&self) -> usize {
        self.connections.len()
    }

    /// Register all discovered MCP tools into a ToolRegistry.
    pub fn register_tools(&self, registry: &mut crate::tools::ToolRegistry) {
        for tool_info in self.all_tools() {
            registry.register(Box::new(bridge::McpToolWrapper { info: tool_info }));
        }
    }

    /// Close all connections.
    pub async fn close_all(&mut self) {
        for conn in &mut self.connections {
            conn.close().await.ok();
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}
