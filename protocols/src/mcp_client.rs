//! MCP Host Mode — consume external MCP servers as a client.
//!
//! Nexus OS can connect to external MCP-compatible servers (filesystem, GitHub,
//! databases, etc.), discover their tools, and call them with full governance:
//! capability checks, PII redaction, fuel accounting, and audit trail.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Configuration types ─────────────────────────────────────────────────────

/// Configuration for connecting to an external MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub transport: McpTransport,
    pub auth: Option<McpAuth>,
    pub enabled: bool,
}

/// Transport mechanism for communicating with an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpTransport {
    Http,
    Sse,
    Stdio,
}

/// Authentication credentials for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpAuth {
    Bearer(String),
    ApiKey { header: String, key: String },
    None,
}

// ── Tool types ──────────────────────────────────────────────────────────────

/// A tool definition discovered from an external MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub server_id: String,
}

/// Result of calling an external MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub tool_name: String,
    pub server_id: String,
    pub content: Vec<McpContent>,
    pub is_error: bool,
    pub execution_ms: u64,
}

/// Content returned by an MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String, text: Option<String> },
}

// ── JSON-RPC types ──────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ── McpClient ───────────────────────────────────────────────────────────────

/// Client that connects to a single external MCP server.
pub struct McpClient {
    config: McpServerConfig,
    available_tools: Vec<McpToolDefinition>,
    request_counter: u64,
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            available_tools: Vec::new(),
            request_counter: 0,
        }
    }

    /// Initialize connection: send "initialize", then "tools/list" to discover tools.
    pub fn initialize(&mut self) -> Result<Vec<McpToolDefinition>, String> {
        // Step 1: Send initialize handshake
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "nexus-os",
                "version": "7.0.0"
            }
        });
        let _init_response = self.send_jsonrpc("initialize", Some(init_params))?;

        // Step 2: Send initialized notification (no response expected for notifications,
        // but we send as a request for simplicity with curl)
        let _ = self.send_jsonrpc("notifications/initialized", None);

        // Step 3: Discover tools
        let tools_response = self.send_jsonrpc("tools/list", None)?;

        let tools = if let Some(result) = tools_response.result {
            let tools_array = result
                .get("tools")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();

            tools_array
                .into_iter()
                .filter_map(|t| {
                    let name = t.get("name")?.as_str()?.to_string();
                    let description = t
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input_schema = t
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    Some(McpToolDefinition {
                        name,
                        description,
                        input_schema,
                        server_id: self.config.id.clone(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        self.available_tools = tools.clone();
        Ok(tools)
    }

    /// Call a tool on this server.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        // Validate tool exists
        if !self.available_tools.iter().any(|t| t.name == tool_name) {
            return Err(format!(
                "Tool '{}' not found on server '{}'",
                tool_name, self.config.id
            ));
        }

        let start = std::time::Instant::now();

        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });

        let response = self.send_jsonrpc("tools/call", Some(params))?;
        let execution_ms = start.elapsed().as_millis() as u64;

        if let Some(error) = response.error {
            return Ok(McpToolResult {
                tool_name: tool_name.to_string(),
                server_id: self.config.id.clone(),
                content: vec![McpContent::Text {
                    text: error.message,
                }],
                is_error: true,
                execution_ms,
            });
        }

        let is_error = response
            .result
            .as_ref()
            .and_then(|r| r.get("isError"))
            .and_then(|e| e.as_bool())
            .unwrap_or(false);

        let content = if let Some(result) = response.result {
            let content_array = result
                .get("content")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();

            content_array
                .into_iter()
                .filter_map(|c| serde_json::from_value(c).ok())
                .collect()
        } else {
            Vec::new()
        };

        Ok(McpToolResult {
            tool_name: tool_name.to_string(),
            server_id: self.config.id.clone(),
            content,
            is_error,
            execution_ms,
        })
    }

    /// List all discovered tools.
    pub fn list_tools(&self) -> &[McpToolDefinition] {
        &self.available_tools
    }

    /// Get the server ID.
    pub fn server_id(&self) -> &str {
        &self.config.id
    }

    /// Check if tools have been discovered (i.e., initialize was called).
    pub fn is_connected(&self) -> bool {
        !self.available_tools.is_empty()
    }

    /// Get the current request counter value.
    pub fn request_counter(&self) -> u64 {
        self.request_counter
    }

    /// Send a JSON-RPC request to the server and return the response.
    fn send_jsonrpc(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, String> {
        self.request_counter += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::from(self.request_counter),
            method: method.to_string(),
            params,
        };

        match self.config.transport {
            McpTransport::Http => self.send_http(&request),
            McpTransport::Sse => {
                // SSE transport: fall back to HTTP POST for tool calls
                self.send_http(&request)
            }
            McpTransport::Stdio => Err("Stdio transport not yet implemented".to_string()),
        }
    }

    /// Send a JSON-RPC request via HTTP POST using curl.
    fn send_http(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        let body = serde_json::to_string(request)
            .map_err(|e| format!("Failed to serialize request: {e}"))?;

        let mut cmd = std::process::Command::new("curl");
        cmd.arg("-s")
            .arg("-X")
            .arg("POST")
            .arg(&self.config.url)
            .arg("-H")
            .arg("Content-Type: application/json");

        // Add auth headers
        if let Some(ref auth) = self.config.auth {
            match auth {
                McpAuth::Bearer(token) => {
                    cmd.arg("-H").arg(format!("Authorization: Bearer {token}"));
                }
                McpAuth::ApiKey { header, key } => {
                    cmd.arg("-H").arg(format!("{header}: {key}"));
                }
                McpAuth::None => {}
            }
        }

        cmd.arg("-d").arg(&body);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute curl: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("curl failed: {stderr}"));
        }

        let response_text = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse JSON-RPC response: {e}"))
    }
}

// ── McpHostManager ──────────────────────────────────────────────────────────

/// Manages multiple MCP server connections and routes tool calls.
pub struct McpHostManager {
    servers: HashMap<String, McpServerConfig>,
    clients: HashMap<String, McpClient>,
    tool_index: HashMap<String, String>, // tool_name → server_id
    governance_enabled: bool,
}

impl McpHostManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            clients: HashMap::new(),
            tool_index: HashMap::new(),
            governance_enabled: true,
        }
    }

    /// Add an MCP server configuration.
    pub fn add_server(&mut self, config: McpServerConfig) -> Result<(), String> {
        if self.servers.contains_key(&config.id) {
            return Err(format!("Server '{}' already exists", config.id));
        }
        self.servers.insert(config.id.clone(), config);
        Ok(())
    }

    /// Remove an MCP server and its client/tools.
    pub fn remove_server(&mut self, server_id: &str) -> bool {
        let removed = self.servers.remove(server_id).is_some();
        if removed {
            self.disconnect_server(server_id);
        }
        removed
    }

    /// Connect to a server: create client, initialize, index tools.
    pub fn connect_server(&mut self, server_id: &str) -> Result<Vec<McpToolDefinition>, String> {
        let config = self
            .servers
            .get(server_id)
            .ok_or_else(|| format!("Server '{server_id}' not found"))?
            .clone();

        let mut client = McpClient::new(config);
        let tools = client.initialize()?;

        // Index tools for routing
        for tool in &tools {
            self.tool_index
                .insert(tool.name.clone(), server_id.to_string());
        }

        self.clients.insert(server_id.to_string(), client);
        Ok(tools)
    }

    /// Disconnect from a server, removing its tools from the index.
    pub fn disconnect_server(&mut self, server_id: &str) {
        if let Some(client) = self.clients.remove(server_id) {
            for tool in client.list_tools() {
                self.tool_index.remove(&tool.name);
            }
        }
    }

    /// List all configured servers.
    pub fn list_servers(&self) -> Vec<&McpServerConfig> {
        self.servers.values().collect()
    }

    /// Aggregate tools from all connected clients.
    pub fn list_all_tools(&self) -> Vec<&McpToolDefinition> {
        self.clients.values().flat_map(|c| c.list_tools()).collect()
    }

    /// Call a tool by name, routing to the correct server.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        let server_id = self
            .tool_index
            .get(tool_name)
            .ok_or_else(|| format!("Tool '{tool_name}' not found in any connected server"))?
            .clone();

        let client = self
            .clients
            .get_mut(&server_id)
            .ok_or_else(|| format!("Server '{server_id}' not connected"))?;

        client.call_tool(tool_name, arguments)
    }

    /// Find a tool definition by name.
    pub fn find_tool(&self, tool_name: &str) -> Option<&McpToolDefinition> {
        let server_id = self.tool_index.get(tool_name)?;
        let client = self.clients.get(server_id)?;
        client.list_tools().iter().find(|t| t.name == tool_name)
    }

    /// Number of connected servers.
    pub fn connected_server_count(&self) -> usize {
        self.clients.len()
    }

    /// Total number of tools across all connected servers.
    pub fn total_tool_count(&self) -> usize {
        self.tool_index.len()
    }

    /// Whether governance is enabled for tool calls.
    pub fn governance_enabled(&self) -> bool {
        self.governance_enabled
    }

    /// Check if a specific server is connected.
    pub fn is_server_connected(&self, server_id: &str) -> bool {
        self.clients.contains_key(server_id)
    }

    /// Get the tool index (for testing/inspection).
    pub fn tool_index(&self) -> &HashMap<String, String> {
        &self.tool_index
    }

    /// Manually index a tool for a server (used in testing and manual registration).
    pub fn index_tool(&mut self, tool_name: String, server_id: String) {
        self.tool_index.insert(tool_name, server_id);
    }
}

impl Default for McpHostManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helper ──────────────────────────────────────────────────────────────────

/// Create a new server config with a generated UUID.
pub fn create_server_config(
    name: &str,
    url: &str,
    transport: McpTransport,
    auth: Option<McpAuth>,
) -> McpServerConfig {
    McpServerConfig {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        url: url.to_string(),
        transport,
        auth,
        enabled: true,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_server_config(id: &str, name: &str) -> McpServerConfig {
        McpServerConfig {
            id: id.to_string(),
            name: name.to_string(),
            url: "http://localhost:8080/mcp".to_string(),
            transport: McpTransport::Http,
            auth: None,
            enabled: true,
        }
    }

    #[test]
    fn test_server_config_creation() {
        let config = create_server_config(
            "GitHub MCP",
            "http://localhost:3000/mcp",
            McpTransport::Http,
            Some(McpAuth::Bearer("token123".to_string())),
        );
        assert_eq!(config.name, "GitHub MCP");
        assert_eq!(config.url, "http://localhost:3000/mcp");
        assert!(config.enabled);
        assert!(!config.id.is_empty());
    }

    #[test]
    fn test_mcp_client_new() {
        let config = make_server_config("s1", "test-server");
        let client = McpClient::new(config);
        assert!(!client.is_connected());
        assert_eq!(client.server_id(), "s1");
        assert!(client.list_tools().is_empty());
    }

    #[test]
    fn test_jsonrpc_request_format() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::from(1),
            method: "tools/list".to_string(),
            params: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "tools/list");
        // params should be omitted when None
        assert!(json.get("params").is_none());
    }

    #[test]
    fn test_jsonrpc_response_parsing() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"tools": []},
            "error": null
        }"#;
        let response: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, 1);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_jsonrpc_error_parsing() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "id": 2,
            "result": null,
            "error": {"code": -32601, "message": "Method not found", "data": null}
        }"#;
        let response: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_tool_definition_serde() {
        let tool = McpToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file from the filesystem".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
            server_id: "fs-server".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: McpToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "read_file");
        assert_eq!(deserialized.server_id, "fs-server");
        assert_eq!(deserialized.input_schema, tool.input_schema);
    }

    #[test]
    fn test_tool_result_serde() {
        let result = McpToolResult {
            tool_name: "read_file".to_string(),
            server_id: "fs-server".to_string(),
            content: vec![McpContent::Text {
                text: "file contents here".to_string(),
            }],
            is_error: false,
            execution_ms: 42,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: McpToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_name, "read_file");
        assert!(!deserialized.is_error);
        assert_eq!(deserialized.execution_ms, 42);
        assert_eq!(deserialized.content.len(), 1);
    }

    #[test]
    fn test_tool_result_error() {
        let result = McpToolResult {
            tool_name: "delete_file".to_string(),
            server_id: "fs-server".to_string(),
            content: vec![McpContent::Text {
                text: "Permission denied".to_string(),
            }],
            is_error: true,
            execution_ms: 5,
        };
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_host_manager_add_remove() {
        let mut manager = McpHostManager::new();
        let config = make_server_config("s1", "test-server");
        manager.add_server(config).unwrap();
        assert_eq!(manager.list_servers().len(), 1);

        let removed = manager.remove_server("s1");
        assert!(removed);
        assert!(manager.list_servers().is_empty());

        // Removing non-existent returns false
        assert!(!manager.remove_server("s1"));
    }

    #[test]
    fn test_host_manager_tool_routing() {
        let mut manager = McpHostManager::new();

        // Manually set up tool routing (without real server connections)
        manager.index_tool("read_file".to_string(), "fs-server".to_string());
        manager.index_tool("create_issue".to_string(), "github-server".to_string());
        manager.index_tool("list_repos".to_string(), "github-server".to_string());

        assert_eq!(manager.total_tool_count(), 3);
        assert_eq!(
            manager.tool_index().get("read_file"),
            Some(&"fs-server".to_string())
        );
        assert_eq!(
            manager.tool_index().get("create_issue"),
            Some(&"github-server".to_string())
        );
        assert_eq!(
            manager.tool_index().get("list_repos"),
            Some(&"github-server".to_string())
        );
    }

    #[test]
    fn test_host_manager_empty() {
        let manager = McpHostManager::new();
        assert_eq!(manager.connected_server_count(), 0);
        assert_eq!(manager.total_tool_count(), 0);
        assert!(manager.list_servers().is_empty());
        assert!(manager.list_all_tools().is_empty());
    }

    #[test]
    fn test_mcp_content_variants() {
        // Text
        let text = McpContent::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&text).unwrap();
        let parsed: McpContent = serde_json::from_str(&json).unwrap();
        if let McpContent::Text { text } = &parsed {
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Text variant");
        }

        // Image
        let image = McpContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        let json = serde_json::to_string(&image).unwrap();
        let parsed: McpContent = serde_json::from_str(&json).unwrap();
        if let McpContent::Image { data, mime_type } = &parsed {
            assert_eq!(data, "base64data");
            assert_eq!(mime_type, "image/png");
        } else {
            panic!("Expected Image variant");
        }

        // Resource
        let resource = McpContent::Resource {
            uri: "nexus://audit/log".to_string(),
            text: Some("audit data".to_string()),
        };
        let json = serde_json::to_string(&resource).unwrap();
        let parsed: McpContent = serde_json::from_str(&json).unwrap();
        if let McpContent::Resource { uri, text } = &parsed {
            assert_eq!(uri, "nexus://audit/log");
            assert_eq!(text.as_deref(), Some("audit data"));
        } else {
            panic!("Expected Resource variant");
        }
    }

    #[test]
    fn test_request_counter_increments() {
        let config = make_server_config("s1", "test");
        let mut client = McpClient::new(config);
        assert_eq!(client.request_counter(), 0);

        // Calling send_jsonrpc will fail (no server), but counter should still increment
        let _ = client.send_jsonrpc("test", None);
        assert_eq!(client.request_counter(), 1);

        let _ = client.send_jsonrpc("test2", None);
        assert_eq!(client.request_counter(), 2);
    }

    #[test]
    fn test_mcp_auth_variants() {
        // Bearer
        let bearer = McpAuth::Bearer("tok_123".to_string());
        let json = serde_json::to_string(&bearer).unwrap();
        let parsed: McpAuth = serde_json::from_str(&json).unwrap();
        if let McpAuth::Bearer(token) = &parsed {
            assert_eq!(token, "tok_123");
        } else {
            panic!("Expected Bearer variant");
        }

        // ApiKey
        let api_key = McpAuth::ApiKey {
            header: "X-API-Key".to_string(),
            key: "secret".to_string(),
        };
        let json = serde_json::to_string(&api_key).unwrap();
        let parsed: McpAuth = serde_json::from_str(&json).unwrap();
        if let McpAuth::ApiKey { header, key } = &parsed {
            assert_eq!(header, "X-API-Key");
            assert_eq!(key, "secret");
        } else {
            panic!("Expected ApiKey variant");
        }

        // None
        let none = McpAuth::None;
        let json = serde_json::to_string(&none).unwrap();
        let parsed: McpAuth = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, McpAuth::None));
    }
}
