//! Frontend integration for MCP server and client.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::client::{ExternalServerConfig, McpClient};
use crate::server::McpServer;
use crate::types::McpTool;

/// In-memory MCP state held by the Tauri app.
pub struct McpState {
    pub server: RwLock<McpServer>,
    pub client: RwLock<McpClient>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            server: RwLock::new(McpServer::new()),
            client: RwLock::new(McpClient::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub tools_count: usize,
    pub resources_count: usize,
    pub prompts_count: usize,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn mcp_server_status(state: &McpState) -> Result<McpServerStatus, String> {
    let server = state.server.read().map_err(|e| format!("lock: {e}"))?;
    let resp = server.handle_raw(r#"{"jsonrpc":"2.0","id":0,"method":"tools/list","params":{}}"#);
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).map_err(|e| format!("parse: {e}"))?;
    let tools_count = parsed["result"]["tools"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(McpServerStatus {
        tools_count,
        resources_count: 3,
        prompts_count: 2,
    })
}

pub fn mcp_server_handle_request(state: &McpState, request_json: &str) -> Result<String, String> {
    let server = state.server.read().map_err(|e| format!("lock: {e}"))?;
    Ok(server.handle_raw(request_json))
}

pub fn mcp_server_list_tools(state: &McpState) -> Result<Vec<McpTool>, String> {
    let server = state.server.read().map_err(|e| format!("lock: {e}"))?;
    Ok(server
        .handle_request(&crate::types::JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(0),
            method: "tools/list".into(),
            params: serde_json::json!({}),
        })
        .result
        .and_then(|r| r.get("tools").cloned())
        .and_then(|t| serde_json::from_value(t).ok())
        .unwrap_or_default())
}

pub fn mcp_client_add_server(
    state: &McpState,
    id: &str,
    name: &str,
    command: &str,
    args: Vec<String>,
) -> Result<(), String> {
    let mut client = state.client.write().map_err(|e| format!("lock: {e}"))?;
    client.add_server(ExternalServerConfig {
        id: id.into(),
        name: name.into(),
        command: command.into(),
        args,
        env: std::collections::HashMap::new(),
    });
    Ok(())
}

pub fn mcp_client_remove_server(state: &McpState, server_id: &str) -> Result<(), String> {
    let mut client = state.client.write().map_err(|e| format!("lock: {e}"))?;
    client.remove_server(server_id);
    Ok(())
}

pub fn mcp_client_discover_tools(
    state: &McpState,
    server_id: &str,
) -> Result<Vec<McpTool>, String> {
    let mut client = state.client.write().map_err(|e| format!("lock: {e}"))?;
    client.discover_tools(server_id)
}

pub fn mcp_client_call_tool(
    state: &McpState,
    server_id: &str,
    tool_name: &str,
    arguments_json: &str,
) -> Result<serde_json::Value, String> {
    let arguments: serde_json::Value =
        serde_json::from_str(arguments_json).map_err(|e| format!("parse args: {e}"))?;
    let client = state.client.read().map_err(|e| format!("lock: {e}"))?;
    client.call_tool(server_id, tool_name, arguments)
}
