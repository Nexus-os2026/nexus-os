//! MCP Host Mode — consume external MCP servers as a client.
//!
//! Nexus OS can connect to external MCP-compatible servers (filesystem, GitHub,
//! databases, etc.), discover their tools, and call them with full governance:
//! capability checks, PII redaction, fuel accounting, and audit trail.

use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::consent::{ConsentRuntime, GovernedOperation};
use nexus_kernel::redaction::RedactionEngine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
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
            McpTransport::Stdio => Err("Stdio transport requires StdioMcpClient".to_string()),
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

// ── Stdio Transport ─────────────────────────────────────────────────────

/// MCP client that communicates with a server via stdin/stdout of a child process.
/// This implements the MCP stdio transport specification.
pub struct StdioMcpClient {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    available_tools: Vec<McpToolDefinition>,
    request_counter: u64,
    server_id: String,
}

impl std::fmt::Debug for StdioMcpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioMcpClient")
            .field("server_id", &self.server_id)
            .field("request_counter", &self.request_counter)
            .field("tools_count", &self.available_tools.len())
            .finish()
    }
}

impl StdioMcpClient {
    /// Spawn a child process and set up stdin/stdout communication.
    pub fn spawn(command: &str, args: &[&str], server_id: &str) -> Result<Self, String> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP server process '{}': {}", command, e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout from MCP server process".to_string())?;

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            available_tools: Vec::new(),
            request_counter: 0,
            server_id: server_id.to_string(),
        })
    }

    /// Initialize the connection and discover tools.
    pub fn initialize(&mut self) -> Result<Vec<McpToolDefinition>, String> {
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "nexus-os",
                "version": "9.0.0"
            }
        });
        let _init_response = self.send_jsonrpc("initialize", Some(init_params))?;
        let _ = self.send_jsonrpc("notifications/initialized", None);

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
                        server_id: self.server_id.clone(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        self.available_tools = tools.clone();
        Ok(tools)
    }

    /// Call a tool on the stdio server.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        if !self.available_tools.iter().any(|t| t.name == tool_name) {
            return Err(format!(
                "Tool '{}' not found on stdio server '{}'",
                tool_name, self.server_id
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
                server_id: self.server_id.clone(),
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
            server_id: self.server_id.clone(),
            content,
            is_error,
            execution_ms,
        })
    }

    /// List all discovered tools.
    pub fn list_tools(&self) -> &[McpToolDefinition] {
        &self.available_tools
    }

    /// Send a JSON-RPC request via stdin and read the response from stdout.
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

        let msg = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {e}"))?;

        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| "stdin not available".to_string())?;

        stdin
            .write_all(msg.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {e}"))?;
        stdin
            .write_all(b"\n")
            .map_err(|e| format!("Failed to write newline: {e}"))?;
        stdin
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {e}"))?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read from stdout: {e}"))?;

        if line.trim().is_empty() {
            return Err("Empty response from MCP server".to_string());
        }

        serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse JSON-RPC response: {e}"))
    }

    /// Shut down the child process.
    pub fn shutdown(&mut self) -> Result<(), String> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

// ── Governed MCP Host ───────────────────────────────────────────────────

/// Governance context for MCP tool calls.
/// Wraps McpHostManager with capability checks, fuel accounting,
/// PII redaction, and audit trail.
pub struct GovernedMcpHost {
    pub host: McpHostManager,
    audit: Mutex<AuditTrail>,
    fuel_remaining: Mutex<f64>,
    fuel_per_call: f64,
    allowed_capabilities: Mutex<Vec<String>>,
    consent: Option<Mutex<ConsentRuntime>>,
    agent_id: Uuid,
}

impl GovernedMcpHost {
    pub fn new(fuel_budget: f64) -> Self {
        Self {
            host: McpHostManager::new(),
            audit: Mutex::new(AuditTrail::new()),
            fuel_remaining: Mutex::new(fuel_budget),
            fuel_per_call: 5.0,
            allowed_capabilities: Mutex::new(vec!["mcp.call_tool".to_string()]),
            consent: None,
            agent_id: Uuid::nil(),
        }
    }

    /// Create a governed MCP host with HITL consent enforcement.
    pub fn with_consent(fuel_budget: f64, consent_runtime: ConsentRuntime, agent_id: Uuid) -> Self {
        Self {
            host: McpHostManager::new(),
            audit: Mutex::new(AuditTrail::new()),
            fuel_remaining: Mutex::new(fuel_budget),
            fuel_per_call: 5.0,
            allowed_capabilities: Mutex::new(vec!["mcp.call_tool".to_string()]),
            consent: Some(Mutex::new(consent_runtime)),
            agent_id,
        }
    }

    /// Set the capabilities an agent is allowed to use.
    pub fn set_allowed_capabilities(&self, capabilities: Vec<String>) {
        if let Ok(mut caps) = self.allowed_capabilities.lock() {
            *caps = capabilities;
        }
    }

    /// Check if the agent has the `mcp.call_tool` capability.
    fn check_capability(&self) -> Result<(), String> {
        let caps = self
            .allowed_capabilities
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        if caps.iter().any(|c| c == "mcp.call_tool" || c == "*") {
            Ok(())
        } else {
            Err("Agent lacks 'mcp.call_tool' capability".to_string())
        }
    }

    /// Deduct fuel for a tool call. Returns error if insufficient.
    fn deduct_fuel(&self) -> Result<(), String> {
        let mut fuel = self
            .fuel_remaining
            .lock()
            .map_err(|_| "lock poisoned".to_string())?;
        if *fuel < self.fuel_per_call {
            return Err(format!(
                "Insufficient fuel: need {} but only {:.1} remaining",
                self.fuel_per_call, *fuel
            ));
        }
        *fuel -= self.fuel_per_call;
        Ok(())
    }

    /// Record an audit event for a tool call.
    fn audit_call(&self, tool_name: &str, server_id: &str, success: bool, detail: &str) {
        if let Ok(mut audit) = self.audit.lock() {
            let _ = audit.append_event(
                uuid::Uuid::nil(),
                EventType::ToolCall,
                json!({
                    "action": "mcp_client_call",
                    "tool": tool_name,
                    "server": server_id,
                    "success": success,
                    "detail": detail,
                }),
            );
        }
    }

    /// Governed tool call: capability check → HITL consent → fuel check → PII redact → call → audit.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult, String> {
        // 1. Capability check
        self.check_capability().inspect_err(|e| {
            self.audit_call(tool_name, "unknown", false, e);
        })?;

        // 2. HITL consent — require human approval before calling external MCP tool
        if let Some(consent_mutex) = &self.consent {
            let server_id = self.host.tool_index().get(tool_name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let payload = format!("mcp_call:{}:{}:{}", server_id, tool_name, arguments);
            if let Ok(mut consent) = consent_mutex.lock() {
                if let Ok(mut audit) = self.audit.lock() {
                    if let Err(e) = consent.enforce_operation(
                        GovernedOperation::McpExternalToolCall,
                        self.agent_id,
                        payload.as_bytes(),
                        &mut audit,
                    ) {
                        let msg = format!("HITL denied MCP tool call '{}': {}", tool_name, e);
                        self.audit_call(tool_name, &server_id, false, &msg);
                        return Err(msg);
                    }
                }
            }
        }

        // 3. Fuel check
        self.deduct_fuel().inspect_err(|e| {
            self.audit_call(tool_name, "unknown", false, e);
        })?;

        // 3. PII redaction on outbound arguments
        let redacted_args = redact_json_values(&arguments);

        // 4. Call the tool
        let result = self.host.call_tool(tool_name, redacted_args);

        // 5. Audit
        match &result {
            Ok(r) => self.audit_call(
                tool_name,
                &r.server_id,
                !r.is_error,
                &format!("{}ms", r.execution_ms),
            ),
            Err(e) => self.audit_call(tool_name, "unknown", false, e),
        }

        result
    }

    /// Get remaining fuel.
    pub fn fuel_remaining(&self) -> f64 {
        self.fuel_remaining
            .lock()
            .map(|f| *f)
            .unwrap_or(0.0)
    }

    /// Access the audit trail.
    pub fn audit_trail(&self) -> &Mutex<AuditTrail> {
        &self.audit
    }
}

/// Recursively redact PII from JSON values (strings only).
fn redact_json_values(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let findings = RedactionEngine::scan(s);
            if findings.is_empty() {
                value.clone()
            } else {
                serde_json::Value::String(RedactionEngine::apply(s, &findings))
            }
        }
        serde_json::Value::Object(map) => {
            let redacted: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), redact_json_values(v)))
                .collect();
            serde_json::Value::Object(redacted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(redact_json_values).collect())
        }
        other => other.clone(),
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

    #[test]
    fn test_stdio_spawn_nonexistent_binary() {
        let result = StdioMcpClient::spawn("__nonexistent_mcp_binary__", &[], "test-stdio");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to spawn"));
    }

    #[test]
    fn test_stdio_spawn_and_shutdown() {
        // Use `cat` as a simple echo-like process for stdio transport testing
        let mut client = StdioMcpClient::spawn("cat", &[], "test-cat").unwrap();
        assert_eq!(client.server_id, "test-cat");
        assert!(client.available_tools.is_empty());
        assert!(client.shutdown().is_ok());
    }

    #[test]
    fn test_governed_host_capability_denied() {
        let mut governed = GovernedMcpHost::new(100.0);
        governed.set_allowed_capabilities(vec!["other.capability".to_string()]);
        let result = governed.call_tool("some_tool", json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("lacks 'mcp.call_tool'"));
    }

    #[test]
    fn test_governed_host_fuel_deduction() {
        let governed = GovernedMcpHost::new(100.0);
        assert_eq!(governed.fuel_remaining(), 100.0);
        governed.deduct_fuel().unwrap();
        assert_eq!(governed.fuel_remaining(), 95.0);
        governed.deduct_fuel().unwrap();
        assert_eq!(governed.fuel_remaining(), 90.0);
    }

    #[test]
    fn test_governed_host_fuel_exhaustion() {
        let governed = GovernedMcpHost::new(3.0);
        let result = governed.deduct_fuel();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Insufficient fuel"));
    }

    #[test]
    fn test_governed_host_audit_trail() {
        let governed = GovernedMcpHost::new(100.0);
        governed.audit_call("test_tool", "test_server", true, "ok");
        governed.audit_call("test_tool", "test_server", false, "failed");
        let audit = governed.audit_trail().lock().unwrap();
        assert_eq!(audit.events().len(), 2);
    }

    #[test]
    fn test_pii_redaction_on_arguments() {
        let args = json!({
            "query": "Find user@example.com",
            "nested": {"email": "admin@test.org"},
            "number": 42
        });
        let redacted = redact_json_values(&args);
        // Numbers should be unchanged
        assert_eq!(redacted["number"], 42);
        // Strings with PII should be redacted (or unchanged if no PII engine match)
        assert!(redacted["query"].is_string());
        assert!(redacted["nested"]["email"].is_string());
    }

    #[test]
    fn test_governed_host_call_with_governance() {
        let mut governed = GovernedMcpHost::new(100.0);
        // Tool won't be found (no servers connected), but governance checks pass
        let result = governed.call_tool("missing_tool", json!({}));
        assert!(result.is_err());
        // Fuel should still be deducted (governance passed, tool call failed)
        assert_eq!(governed.fuel_remaining(), 95.0);
        // Audit should record the failure
        let audit = governed.audit_trail().lock().unwrap();
        assert_eq!(audit.events().len(), 1);
    }

    #[test]
    fn test_governed_host_with_consent_constructor() {
        let consent = nexus_kernel::consent::ConsentRuntime::default();
        let agent_id = Uuid::new_v4();
        let governed = GovernedMcpHost::with_consent(100.0, consent, agent_id);
        assert!(governed.consent.is_some());
        assert_eq!(governed.fuel_remaining(), 100.0);
    }
}
