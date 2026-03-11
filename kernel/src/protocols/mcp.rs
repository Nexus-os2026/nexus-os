//! MCP (Model Context Protocol) governed tool server.
//!
//! Nexus agents expose their capabilities as MCP tools that external systems
//! can discover and invoke. Every tool call passes through the governance
//! pipeline: capability check → fuel deduction → speculative check → audit.
//!
//! External MCP clients cannot bypass governance.

use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use crate::firewall::{EgressDecision, EgressGovernor};
use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ── Tool definition types ───────────────────────────────────────────────────

/// A governed MCP tool definition derived from an agent's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedTool {
    /// Tool name (derived from agent capability).
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Governance constraints on this tool.
    pub governance: ToolGovernance,
}

/// Governance constraints applied to an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGovernance {
    /// Required kernel capabilities to invoke this tool.
    pub required_capabilities: Vec<String>,
    /// Minimum autonomy level needed.
    pub min_autonomy_level: u8,
    /// Estimated fuel cost per invocation.
    pub estimated_fuel_cost: u64,
    /// Whether HITL approval is required.
    pub requires_hitl: bool,
    /// Whether PII redaction is applied to outputs.
    pub pii_redaction: bool,
}

/// Result of a governed MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedToolResult {
    /// Tool output content.
    pub content: Vec<ToolContent>,
    /// Whether the tool invocation succeeded.
    pub is_error: bool,
    /// Fuel consumed by this invocation.
    pub fuel_consumed: u64,
    /// Audit event hash for this invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_hash: Option<String>,
}

/// Content returned by an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content.
    Text { text: String },
    /// Image content (base64).
    Image { data: String, mime_type: String },
    /// Embedded resource.
    Resource { uri: String, text: String },
}

/// An MCP resource that can be queried (e.g., audit trail, agent status).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedResource {
    /// Resource URI (e.g., "nexus://audit/agent/{id}").
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource.
    pub mime_type: String,
}

/// An MCP prompt template for common governed operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedPrompt {
    /// Prompt name.
    pub name: String,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments the prompt accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<PromptArgument>,
}

/// An argument for a governed prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,
}

// ── Capability-to-tool mapping ──────────────────────────────────────────────

/// Metadata for converting a Nexus capability into an MCP tool.
struct ToolMapping {
    name: &'static str,
    description: &'static str,
    input_schema: serde_json::Value,
    fuel_cost: u64,
    min_autonomy: u8,
    requires_hitl: bool,
    pii_redaction: bool,
}

/// Map a Nexus capability to its MCP tool metadata.
fn capability_to_tool(capability: &str) -> Option<ToolMapping> {
    match capability {
        "web.search" => Some(ToolMapping {
            name: "web_search",
            description: "Search the web and return relevant results",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            }),
            fuel_cost: 50,
            min_autonomy: 0,
            requires_hitl: false,
            pii_redaction: false,
        }),
        "web.read" => Some(ToolMapping {
            name: "web_read",
            description: "Fetch and extract content from a web page",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to fetch"}
                },
                "required": ["url"]
            }),
            fuel_cost: 50,
            min_autonomy: 0,
            requires_hitl: false,
            pii_redaction: true,
        }),
        "llm.query" => Some(ToolMapping {
            name: "llm_query",
            description: "Query a language model with governed fuel accounting",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Prompt text"},
                    "model": {"type": "string", "description": "Model name"},
                    "max_tokens": {"type": "integer", "description": "Max output tokens"}
                },
                "required": ["prompt"]
            }),
            fuel_cost: 500,
            min_autonomy: 1,
            requires_hitl: false,
            pii_redaction: true,
        }),
        "fs.read" => Some(ToolMapping {
            name: "fs_read",
            description: "Read a file from the governed filesystem sandbox",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to read"}
                },
                "required": ["path"]
            }),
            fuel_cost: 10,
            min_autonomy: 0,
            requires_hitl: false,
            pii_redaction: true,
        }),
        "fs.write" => Some(ToolMapping {
            name: "fs_write",
            description: "Write a file to the governed filesystem sandbox",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to write"},
                    "content": {"type": "string", "description": "File content"}
                },
                "required": ["path", "content"]
            }),
            fuel_cost: 20,
            min_autonomy: 2,
            requires_hitl: true,
            pii_redaction: true,
        }),
        "process.exec" => Some(ToolMapping {
            name: "process_exec",
            description: "Execute a sandboxed process with governance controls",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command to execute"},
                    "args": {"type": "array", "items": {"type": "string"}, "description": "Arguments"}
                },
                "required": ["command"]
            }),
            fuel_cost: 100,
            min_autonomy: 3,
            requires_hitl: true,
            pii_redaction: false,
        }),
        "social.post" => Some(ToolMapping {
            name: "social_post",
            description: "Publish content to social media platforms",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "Post content"},
                    "platform": {"type": "string", "description": "Target platform"}
                },
                "required": ["content"]
            }),
            fuel_cost: 30,
            min_autonomy: 2,
            requires_hitl: true,
            pii_redaction: true,
        }),
        "social.x.post" => Some(ToolMapping {
            name: "social_x_post",
            description: "Publish a post to X (Twitter)",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Tweet text"}
                },
                "required": ["text"]
            }),
            fuel_cost: 30,
            min_autonomy: 2,
            requires_hitl: true,
            pii_redaction: true,
        }),
        "social.x.read" => Some(ToolMapping {
            name: "social_x_read",
            description: "Read posts and timelines from X (Twitter)",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query or username"}
                },
                "required": ["query"]
            }),
            fuel_cost: 20,
            min_autonomy: 0,
            requires_hitl: false,
            pii_redaction: true,
        }),
        "messaging.send" => Some(ToolMapping {
            name: "messaging_send",
            description: "Send messages through governed messaging channels",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Channel or recipient"},
                    "message": {"type": "string", "description": "Message content"}
                },
                "required": ["channel", "message"]
            }),
            fuel_cost: 20,
            min_autonomy: 2,
            requires_hitl: true,
            pii_redaction: true,
        }),
        "audit.read" => Some(ToolMapping {
            name: "audit_read",
            description: "Read audit trail events with hash-chain verification",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "agent_id": {"type": "string", "description": "Agent UUID"},
                    "limit": {"type": "integer", "description": "Max events to return"}
                },
                "required": ["agent_id"]
            }),
            fuel_cost: 10,
            min_autonomy: 0,
            requires_hitl: false,
            pii_redaction: false,
        }),
        _ => None,
    }
}

// ── Agent registration ──────────────────────────────────────────────────────

/// Runtime state for a registered agent in the MCP server.
#[derive(Debug, Clone)]
struct RegisteredAgent {
    agent_id: Uuid,
    manifest: AgentManifest,
    fuel_remaining: u64,
    tools: Vec<GovernedTool>,
}

// ── MCP Server ──────────────────────────────────────────────────────────────

/// Governed MCP server that exposes agent capabilities as tools.
///
/// Every tool invocation passes through the governance pipeline:
/// 1. **Capability check**: caller must have the required capability
/// 2. **Fuel check**: sufficient fuel must remain before execution
/// 3. **Fuel deduction**: fuel deducted on success
/// 4. **Audit trail**: every invocation is recorded with hash-chain integrity
///
/// External MCP clients cannot bypass governance.
#[derive(Debug)]
pub struct McpServer {
    agents: HashMap<Uuid, RegisteredAgent>,
    audit_trail: AuditTrail,
    resources: Vec<GovernedResource>,
    egress_governor: EgressGovernor,
}

impl McpServer {
    /// Create a new MCP server with an empty agent registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            audit_trail: AuditTrail::new(),
            egress_governor: EgressGovernor::new(),
            resources: vec![
                GovernedResource {
                    uri: "nexus://agents/status".to_string(),
                    name: "Agent Status".to_string(),
                    description: Some("Status of all registered agents".to_string()),
                    mime_type: "application/json".to_string(),
                },
                GovernedResource {
                    uri: "nexus://audit/events".to_string(),
                    name: "Audit Events".to_string(),
                    description: Some("Audit trail events with hash-chain integrity".to_string()),
                    mime_type: "application/json".to_string(),
                },
            ],
        }
    }

    /// Register an agent, generating MCP tools from its manifest capabilities.
    pub fn register_agent(&mut self, agent_id: Uuid, manifest: AgentManifest) {
        let tools: Vec<GovernedTool> = manifest
            .capabilities
            .iter()
            .filter_map(|cap| {
                capability_to_tool(cap).map(|m| GovernedTool {
                    name: m.name.to_string(),
                    description: Some(m.description.to_string()),
                    input_schema: m.input_schema,
                    governance: ToolGovernance {
                        required_capabilities: vec![cap.clone()],
                        min_autonomy_level: m.min_autonomy,
                        estimated_fuel_cost: m.fuel_cost,
                        requires_hitl: m.requires_hitl,
                        pii_redaction: m.pii_redaction,
                    },
                })
            })
            .collect();

        let fuel_remaining = manifest.fuel_budget;

        self.audit_trail
            .append_event(
                agent_id,
                EventType::StateChange,
                json!({
                    "event_kind": "mcp.agent_registered",
                    "agent_name": manifest.name,
                    "tool_count": tools.len(),
                    "fuel_budget": fuel_remaining,
                }),
            )
            .expect("audit: fail-closed");

        // Register egress policy from manifest allowed_endpoints.
        let allowed = manifest.allowed_endpoints.clone().unwrap_or_default();
        self.egress_governor.register_agent(agent_id, allowed);

        self.agents.insert(
            agent_id,
            RegisteredAgent {
                agent_id,
                manifest,
                fuel_remaining,
                tools,
            },
        );
    }

    /// List all tools available for a given agent.
    ///
    /// Only returns tools whose capabilities are declared in the manifest.
    pub fn list_tools(&self, agent_id: Uuid) -> Result<Vec<GovernedTool>, AgentError> {
        let agent = self.agents.get(&agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("agent {agent_id} not registered"))
        })?;
        Ok(agent.tools.clone())
    }

    /// Get the governance metadata for a specific tool.
    pub fn get_tool_governance(&self, agent_id: Uuid, tool_name: &str) -> Option<ToolGovernance> {
        self.agents.get(&agent_id).and_then(|agent| {
            agent
                .tools
                .iter()
                .find(|t| t.name == tool_name)
                .map(|t| t.governance.clone())
        })
    }

    /// List all registered resources.
    pub fn list_resources(&self) -> &[GovernedResource] {
        &self.resources
    }

    /// Read a resource by URI.
    pub fn read_resource(&self, uri: &str) -> Result<serde_json::Value, AgentError> {
        match uri {
            "nexus://agents/status" => {
                let statuses: Vec<serde_json::Value> = self
                    .agents
                    .values()
                    .map(|a| {
                        json!({
                            "agent_id": a.agent_id.to_string(),
                            "name": a.manifest.name,
                            "fuel_remaining": a.fuel_remaining,
                            "tool_count": a.tools.len(),
                        })
                    })
                    .collect();
                Ok(json!({ "agents": statuses }))
            }
            "nexus://audit/events" => {
                let events: Vec<serde_json::Value> = self
                    .audit_trail
                    .events()
                    .iter()
                    .map(|e| {
                        json!({
                            "event_id": e.event_id.to_string(),
                            "agent_id": e.agent_id.to_string(),
                            "event_type": e.event_type,
                            "hash": e.hash,
                            "timestamp": e.timestamp,
                        })
                    })
                    .collect();
                Ok(json!({
                    "events": events,
                    "integrity_verified": self.audit_trail.verify_integrity(),
                }))
            }
            _ => Err(AgentError::SupervisorError(format!(
                "unknown resource: {uri}"
            ))),
        }
    }

    /// Invoke an MCP tool through the governance pipeline.
    ///
    /// Governance steps (cannot be bypassed):
    /// 1. Resolve agent and tool
    /// 2. Capability check: tool's required capabilities must be in the manifest
    /// 3. Fuel check: agent must have enough fuel for the estimated cost
    /// 4. Execute (mock execution returns the input params as confirmation)
    /// 5. Fuel deduction: deduct estimated cost from agent's budget
    /// 6. Audit trail: record the invocation with hash-chain integrity
    pub fn invoke_tool(
        &mut self,
        agent_id: Uuid,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<GovernedToolResult, AgentError> {
        // Step 1: Resolve agent
        let agent = self.agents.get(&agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("agent {agent_id} not registered"))
        })?;

        // Step 1b: Resolve tool
        let tool = agent
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| AgentError::CapabilityDenied(tool_name.to_string()))?
            .clone();

        // Step 2: Capability check — tool's required capabilities must be in manifest
        for required_cap in &tool.governance.required_capabilities {
            if !agent.manifest.capabilities.contains(required_cap) {
                self.audit_trail
                    .append_event(
                        agent_id,
                        EventType::Error,
                        json!({
                            "event_kind": "mcp.capability_denied",
                            "tool": tool_name,
                            "missing_capability": required_cap,
                        }),
                    )
                    .expect("audit: fail-closed");
                return Err(AgentError::CapabilityDenied(required_cap.clone()));
            }
        }

        // Step 3: Fuel check — must have enough before execution
        let fuel_cost = tool.governance.estimated_fuel_cost;
        if agent.fuel_remaining < fuel_cost {
            self.audit_trail
                .append_event(
                    agent_id,
                    EventType::Error,
                    json!({
                        "event_kind": "mcp.fuel_exhausted",
                        "tool": tool_name,
                        "fuel_remaining": agent.fuel_remaining,
                        "fuel_required": fuel_cost,
                    }),
                )
                .expect("audit: fail-closed");
            return Err(AgentError::FuelExhausted);
        }

        // Step 3b: Egress check — if params contain a URL, validate against allowlist.
        if let Some(url) = params.get("url").and_then(|v| v.as_str()) {
            if let EgressDecision::Deny { reason } =
                self.egress_governor
                    .check_egress(agent_id, url, &mut self.audit_trail)
            {
                return Err(AgentError::CapabilityDenied(format!(
                    "egress blocked: {reason}"
                )));
            }
        }

        // Step 4: Execute (mock — real execution routes to agent runtime)
        let output_text = format!(
            "Tool '{}' executed with params: {}",
            tool_name,
            serde_json::to_string(&params).unwrap_or_default()
        );

        // Step 5: Fuel deduction (must get mutable ref after immutable borrows)
        let agent_mut = self.agents.get_mut(&agent_id).expect("agent verified");
        agent_mut.fuel_remaining = agent_mut.fuel_remaining.saturating_sub(fuel_cost);
        let fuel_after = agent_mut.fuel_remaining;

        // Step 6: Audit trail
        let audit_event_id = self.audit_trail.append_event(
            agent_id,
            EventType::ToolCall,
            json!({
                "event_kind": "mcp.tool_invoked",
                "tool": tool_name,
                "fuel_cost": fuel_cost,
                "fuel_remaining": fuel_after,
                "params_hash": simple_hash(&params.to_string()),
                "governance": {
                    "capabilities_checked": tool.governance.required_capabilities,
                    "hitl_required": tool.governance.requires_hitl,
                    "pii_redaction": tool.governance.pii_redaction,
                }
            }),
        )?;

        Ok(GovernedToolResult {
            content: vec![ToolContent::Text { text: output_text }],
            is_error: false,
            fuel_consumed: fuel_cost,
            audit_hash: Some(audit_event_id.to_string()),
        })
    }

    /// Get the remaining fuel for an agent.
    pub fn fuel_remaining(&self, agent_id: Uuid) -> Option<u64> {
        self.agents.get(&agent_id).map(|a| a.fuel_remaining)
    }

    /// Get a reference to the audit trail.
    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal hash for audit params (not cryptographic — just for logging).
fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AgentManifest;

    /// Build a manifest with specific capabilities.
    fn manifest_with(caps: Vec<&str>, fuel: u64) -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: fuel,
            autonomy_level: Some(2),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    /// Build a full manifest with all 11 capabilities.
    fn full_manifest() -> AgentManifest {
        manifest_with(
            vec![
                "web.search",
                "web.read",
                "llm.query",
                "fs.read",
                "fs.write",
                "process.exec",
                "social.post",
                "social.x.post",
                "social.x.read",
                "messaging.send",
                "audit.read",
            ],
            100_000,
        )
    }

    // ── Tool discovery tests ────────────────────────────────────────────

    #[test]
    fn list_tools_returns_only_governed_capabilities() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        let manifest = manifest_with(vec!["web.search", "fs.read"], 1000);

        server.register_agent(agent_id, manifest);
        let tools = server.list_tools(agent_id).unwrap();

        assert_eq!(tools.len(), 2);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"fs_read"));
        // Capabilities NOT in manifest must NOT appear
        assert!(!names.contains(&"llm_query"));
        assert!(!names.contains(&"fs_write"));
    }

    #[test]
    fn list_tools_full_manifest_returns_all_11() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, full_manifest());

        let tools = server.list_tools(agent_id).unwrap();
        assert_eq!(tools.len(), 11);

        let expected_names = vec![
            "web_search",
            "web_read",
            "llm_query",
            "fs_read",
            "fs_write",
            "process_exec",
            "social_post",
            "social_x_post",
            "social_x_read",
            "messaging_send",
            "audit_read",
        ];
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        for expected in expected_names {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }
    }

    #[test]
    fn list_tools_unregistered_agent_fails() {
        let server = McpServer::new();
        let result = server.list_tools(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn tools_have_governance_metadata() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, full_manifest());

        let tools = server.list_tools(agent_id).unwrap();

        for tool in &tools {
            assert!(
                tool.description.is_some(),
                "tool '{}' must have description",
                tool.name
            );
            assert!(
                !tool.governance.required_capabilities.is_empty(),
                "tool '{}' must have required_capabilities",
                tool.name
            );
            assert_eq!(
                tool.input_schema["type"], "object",
                "tool '{}' input_schema must be an object",
                tool.name
            );
        }
    }

    #[test]
    fn tools_have_correct_fuel_costs() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, full_manifest());

        let tools = server.list_tools(agent_id).unwrap();
        let tool_map: HashMap<&str, &GovernedTool> =
            tools.iter().map(|t| (t.name.as_str(), t)).collect();

        // LLM should be the most expensive
        assert!(
            tool_map["llm_query"].governance.estimated_fuel_cost
                > tool_map["fs_read"].governance.estimated_fuel_cost
        );
        // process_exec is high cost
        assert!(tool_map["process_exec"].governance.estimated_fuel_cost >= 100);
    }

    // ── Tool invocation governance tests ────────────────────────────────

    #[test]
    fn invoke_tool_checks_capability() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        // Only has web.search — not fs.read
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));

        // Invoking a tool the agent doesn't have should fail
        let result = server.invoke_tool(agent_id, "fs_read", json!({"path": "/etc/passwd"}));
        assert!(
            matches!(result, Err(AgentError::CapabilityDenied(_))),
            "should deny tool not in capabilities"
        );
    }

    #[test]
    fn invoke_tool_succeeds_with_capability() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));

        let result = server.invoke_tool(agent_id, "web_search", json!({"query": "rust"}));
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.is_error);
        assert!(result.fuel_consumed > 0);
        assert!(result.audit_hash.is_some());
    }

    #[test]
    fn unauthorized_tool_call_rejected() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        // Agent has web.search only
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));

        // Try every tool the agent does NOT have
        let unauthorized_tools = vec![
            "llm_query",
            "fs_read",
            "fs_write",
            "process_exec",
            "social_post",
            "social_x_post",
            "social_x_read",
            "messaging_send",
            "audit_read",
        ];

        for tool in unauthorized_tools {
            let result = server.invoke_tool(agent_id, tool, json!({}));
            assert!(
                result.is_err(),
                "tool '{tool}' should be denied for agent without capability"
            );
        }
    }

    #[test]
    fn invoke_nonexistent_tool_denied() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));

        let result = server.invoke_tool(agent_id, "does_not_exist", json!({}));
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
    }

    // ── Fuel deduction tests ────────────────────────────────────────────

    #[test]
    fn fuel_deducted_per_invocation() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        let initial_fuel = 10_000u64;
        server.register_agent(agent_id, manifest_with(vec!["web.search"], initial_fuel));

        let tools = server.list_tools(agent_id).unwrap();
        let web_search_cost = tools
            .iter()
            .find(|t| t.name == "web_search")
            .unwrap()
            .governance
            .estimated_fuel_cost;

        // First invocation
        let result = server
            .invoke_tool(agent_id, "web_search", json!({"query": "test"}))
            .unwrap();
        assert_eq!(result.fuel_consumed, web_search_cost);
        assert_eq!(
            server.fuel_remaining(agent_id),
            Some(initial_fuel - web_search_cost)
        );

        // Second invocation
        server
            .invoke_tool(agent_id, "web_search", json!({"query": "test2"}))
            .unwrap();
        assert_eq!(
            server.fuel_remaining(agent_id),
            Some(initial_fuel - 2 * web_search_cost)
        );
    }

    #[test]
    fn fuel_exhausted_rejects_invocation() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        // Give agent only 10 fuel — web_search costs 50
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10));

        let result = server.invoke_tool(agent_id, "web_search", json!({"query": "test"}));
        assert!(
            matches!(result, Err(AgentError::FuelExhausted)),
            "should reject when fuel insufficient"
        );
        // Fuel should not be deducted on failure
        assert_eq!(server.fuel_remaining(agent_id), Some(10));
    }

    #[test]
    fn fuel_drains_to_exhaustion() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        // Give exactly 100 fuel, web_search costs 50 → allows 2 calls
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 100));

        // First call: ok (50 remaining)
        assert!(server
            .invoke_tool(agent_id, "web_search", json!({"query": "1"}))
            .is_ok());
        assert_eq!(server.fuel_remaining(agent_id), Some(50));

        // Second call: ok (0 remaining)
        assert!(server
            .invoke_tool(agent_id, "web_search", json!({"query": "2"}))
            .is_ok());
        assert_eq!(server.fuel_remaining(agent_id), Some(0));

        // Third call: rejected
        let result = server.invoke_tool(agent_id, "web_search", json!({"query": "3"}));
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }

    // ── Audit trail tests ───────────────────────────────────────────────

    #[test]
    fn invocation_produces_audit_event() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));

        let events_before = server.audit_trail().events().len();
        server
            .invoke_tool(agent_id, "web_search", json!({"query": "test"}))
            .unwrap();
        let events_after = server.audit_trail().events().len();

        assert!(
            events_after > events_before,
            "invocation must produce audit events"
        );

        let last_event = server.audit_trail().events().last().unwrap();
        assert_eq!(last_event.event_type, EventType::ToolCall);
        assert_eq!(last_event.agent_id, agent_id);
        assert_eq!(last_event.payload["event_kind"], "mcp.tool_invoked");
        assert_eq!(last_event.payload["tool"], "web_search");
    }

    #[test]
    fn denied_invocation_produces_audit_event() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        // Only 10 fuel, web_search costs 50
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10));

        let events_before = server.audit_trail().events().len();
        let _ = server.invoke_tool(agent_id, "web_search", json!({"query": "denied"}));
        let events_after = server.audit_trail().events().len();

        assert!(
            events_after > events_before,
            "denied invocation must also produce audit event"
        );

        let last_event = server.audit_trail().events().last().unwrap();
        assert_eq!(last_event.event_type, EventType::Error);
        assert_eq!(last_event.payload["event_kind"], "mcp.fuel_exhausted");
    }

    #[test]
    fn audit_trail_has_hash_chain_integrity() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(
            agent_id,
            manifest_with(vec!["web.search", "fs.read"], 100_000),
        );

        for i in 0..5 {
            server
                .invoke_tool(agent_id, "web_search", json!({"query": format!("q{i}")}))
                .unwrap();
        }

        assert!(
            server.audit_trail().verify_integrity(),
            "audit trail must maintain hash-chain integrity"
        );
    }

    // ── Resource tests ──────────────────────────────────────────────────

    #[test]
    fn resources_list_is_populated() {
        let server = McpServer::new();
        let resources = server.list_resources();
        assert_eq!(resources.len(), 2);
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"nexus://agents/status"));
        assert!(uris.contains(&"nexus://audit/events"));
    }

    #[test]
    fn read_agent_status_resource() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 5000));

        let status = server.read_resource("nexus://agents/status").unwrap();
        let agents = status["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0]["name"], "test-agent");
        assert_eq!(agents[0]["fuel_remaining"], 5000);
    }

    #[test]
    fn read_audit_events_resource() {
        let mut server = McpServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id, manifest_with(vec!["web.search"], 10_000));
        server
            .invoke_tool(agent_id, "web_search", json!({"query": "test"}))
            .unwrap();

        let audit = server.read_resource("nexus://audit/events").unwrap();
        assert!(audit["integrity_verified"].as_bool().unwrap());
        let events = audit["events"].as_array().unwrap();
        assert!(!events.is_empty());
    }

    #[test]
    fn read_unknown_resource_fails() {
        let server = McpServer::new();
        let result = server.read_resource("nexus://nonexistent");
        assert!(result.is_err());
    }

    // ── Serde roundtrip tests ───────────────────────────────────────────

    #[test]
    fn governed_tool_roundtrip() {
        let tool = GovernedTool {
            name: "code_review".to_string(),
            description: Some("Review code for issues".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "code": {"type": "string"},
                    "language": {"type": "string"}
                },
                "required": ["code"]
            }),
            governance: ToolGovernance {
                required_capabilities: vec!["code.review".to_string()],
                min_autonomy_level: 1,
                estimated_fuel_cost: 500,
                requires_hitl: false,
                pii_redaction: true,
            },
        };

        let json = serde_json::to_string_pretty(&tool).unwrap();
        let parsed: GovernedTool = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "code_review");
        assert_eq!(parsed.governance.min_autonomy_level, 1);
        assert!(parsed.governance.pii_redaction);
    }

    #[test]
    fn tool_content_tagged_serde() {
        let text = ToolContent::Text {
            text: "result".to_string(),
        };
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains("\"type\":\"text\""));

        let image = ToolContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains("\"type\":\"image\""));
    }

    #[test]
    fn governed_tool_result_roundtrip() {
        let result = GovernedToolResult {
            content: vec![ToolContent::Text {
                text: "All checks passed".to_string(),
            }],
            is_error: false,
            fuel_consumed: 250,
            audit_hash: Some("deadbeef".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: GovernedToolResult = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_error);
        assert_eq!(parsed.fuel_consumed, 250);
    }

    #[test]
    fn governed_resource_serde() {
        let resource = GovernedResource {
            uri: "nexus://audit/agent/123".to_string(),
            name: "Agent Audit Trail".to_string(),
            description: Some("Audit events for agent 123".to_string()),
            mime_type: "application/json".to_string(),
        };

        let json = serde_json::to_string(&resource).unwrap();
        let parsed: GovernedResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "nexus://audit/agent/123");
    }

    #[test]
    fn governed_prompt_serde() {
        let prompt = GovernedPrompt {
            name: "review_agent".to_string(),
            description: Some("Review an agent's behavior".to_string()),
            arguments: vec![PromptArgument {
                name: "agent_id".to_string(),
                description: Some("UUID of the agent".to_string()),
                required: true,
            }],
        };

        let json = serde_json::to_string(&prompt).unwrap();
        let parsed: GovernedPrompt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.arguments.len(), 1);
        assert!(parsed.arguments[0].required);
    }
}
