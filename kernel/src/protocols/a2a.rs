//! Core A2A (Agent-to-Agent) types following the Google A2A specification.
//!
//! Implemented manually for stability — the upstream `a2a-rs-core` crate is very
//! new and may not be stable. These types cover the essential A2A spec surface:
//! Agent Cards, Tasks, Messages, and Artifacts.
//!
//! The key Nexus extension is `AgentCard::from_manifest`, which auto-generates
//! a standards-compliant A2A Agent Card from an existing TOML `AgentManifest`.

use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A2A protocol version we implement.
pub const A2A_PROTOCOL_VERSION: &str = "0.2.1";

// ── Capability-to-skill mapping ─────────────────────────────────────────────

/// Metadata for mapping a Nexus capability to an A2A skill.
struct CapabilityMapping {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    tags: &'static [&'static str],
    input_modes: &'static [&'static str],
    output_modes: &'static [&'static str],
}

/// Map every registered Nexus capability to its A2A skill metadata.
/// Must cover all 11 entries in `CAPABILITY_REGISTRY`.
fn capability_to_skill(capability: &str) -> Option<CapabilityMapping> {
    match capability {
        "web.search" => Some(CapabilityMapping {
            id: "web-search",
            name: "Web Search",
            description: "Search the web and return relevant results",
            tags: &["web", "search", "information-retrieval"],
            input_modes: &["text/plain"],
            output_modes: &["application/json", "text/plain"],
        }),
        "web.read" => Some(CapabilityMapping {
            id: "web-read",
            name: "Web Read",
            description: "Fetch and extract content from web pages",
            tags: &["web", "scraping", "content-extraction"],
            input_modes: &["text/plain"],
            output_modes: &["text/plain", "text/html"],
        }),
        "llm.query" => Some(CapabilityMapping {
            id: "llm-query",
            name: "LLM Query",
            description: "Query a language model with governed fuel accounting",
            tags: &["llm", "ai", "generation"],
            input_modes: &["text/plain"],
            output_modes: &["text/plain"],
        }),
        "fs.read" => Some(CapabilityMapping {
            id: "fs-read",
            name: "File Read",
            description: "Read files from the governed filesystem sandbox",
            tags: &["filesystem", "read", "data"],
            input_modes: &["text/plain"],
            output_modes: &["application/octet-stream", "text/plain"],
        }),
        "fs.write" => Some(CapabilityMapping {
            id: "fs-write",
            name: "File Write",
            description: "Write files to the governed filesystem sandbox",
            tags: &["filesystem", "write", "data"],
            input_modes: &["application/octet-stream", "text/plain"],
            output_modes: &["application/json"],
        }),
        "process.exec" => Some(CapabilityMapping {
            id: "process-exec",
            name: "Process Execute",
            description: "Execute a sandboxed process with governance controls",
            tags: &["process", "execution", "sandbox"],
            input_modes: &["application/json"],
            output_modes: &["application/json", "text/plain"],
        }),
        "social.post" => Some(CapabilityMapping {
            id: "social-post",
            name: "Social Post",
            description: "Publish content to social media platforms",
            tags: &["social", "publishing"],
            input_modes: &["text/plain", "application/json"],
            output_modes: &["application/json"],
        }),
        "social.x.post" => Some(CapabilityMapping {
            id: "social-x-post",
            name: "X Post",
            description: "Publish a post to X (Twitter)",
            tags: &["social", "x", "twitter", "publishing"],
            input_modes: &["text/plain"],
            output_modes: &["application/json"],
        }),
        "social.x.read" => Some(CapabilityMapping {
            id: "social-x-read",
            name: "X Read",
            description: "Read posts and timelines from X (Twitter)",
            tags: &["social", "x", "twitter", "reading"],
            input_modes: &["text/plain"],
            output_modes: &["application/json"],
        }),
        "messaging.send" => Some(CapabilityMapping {
            id: "messaging-send",
            name: "Messaging Send",
            description: "Send messages through governed messaging channels",
            tags: &["messaging", "communication"],
            input_modes: &["text/plain", "application/json"],
            output_modes: &["application/json"],
        }),
        "audit.read" => Some(CapabilityMapping {
            id: "audit-read",
            name: "Audit Read",
            description: "Read audit trail events with hash-chain verification",
            tags: &["audit", "compliance", "governance"],
            input_modes: &["application/json"],
            output_modes: &["application/json"],
        }),
        _ => None,
    }
}

// ── Agent Card ──────────────────────────────────────────────────────────────

/// An Agent Card describes an agent's identity, capabilities, and endpoint
/// for discovery by other agents. Served at `/.well-known/agent.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Human-readable name of the agent.
    pub name: String,
    /// Optional description of what this agent does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// URL where this agent's A2A endpoint is hosted.
    pub url: String,
    /// Protocol version this agent supports.
    pub version: String,
    /// Capabilities this agent advertises.
    pub capabilities: AgentCapabilities,
    /// Skills this agent can perform.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,
    /// Authentication schemes the agent accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authentication: Vec<AuthScheme>,
    /// Default input content types accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_input_modes: Vec<String>,
    /// Default output content types produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_output_modes: Vec<String>,
    /// Rate limit hint derived from fuel budget (requests per minute).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_rpm: Option<u64>,
}

impl AgentCard {
    /// Auto-generate an A2A Agent Card from an existing Nexus `AgentManifest`.
    ///
    /// Mapping rules:
    /// - `manifest.name` → `name`
    /// - `manifest.capabilities` → A2A `skills` (one skill per capability)
    /// - `manifest.fuel_budget` → `rate_limit_rpm` (budget / 100, min 1)
    /// - `manifest.autonomy_level` → authentication requirements:
    ///   - L0-L1: no auth required (public/suggest-only)
    ///   - L2: bearer token required
    ///   - L3-L5: bearer token + mTLS required
    pub fn from_manifest(manifest: &AgentManifest, base_url: &str) -> Self {
        let skills: Vec<AgentSkill> = manifest
            .capabilities
            .iter()
            .filter_map(|cap| {
                capability_to_skill(cap).map(|m| AgentSkill {
                    id: m.id.to_string(),
                    name: m.name.to_string(),
                    description: Some(m.description.to_string()),
                    tags: m.tags.iter().map(|t| (*t).to_string()).collect(),
                    input_modes: m.input_modes.iter().map(|m| (*m).to_string()).collect(),
                    output_modes: m.output_modes.iter().map(|m| (*m).to_string()).collect(),
                })
            })
            .collect();

        let autonomy = manifest.autonomy_level.unwrap_or(0);
        let authentication = auth_schemes_for_autonomy(autonomy);

        // Derive rate limit from fuel budget: rough heuristic of 100 fuel per request.
        let rate_limit_rpm = Some((manifest.fuel_budget / 100).max(1));

        Self {
            name: manifest.name.clone(),
            description: Some(format!(
                "Nexus governed agent '{}' v{}",
                manifest.name, manifest.version
            )),
            url: format!("{}/a2a/{}", base_url.trim_end_matches('/'), manifest.name),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                // Multi-turn supported if agent has LLM capability.
                state_transition_history: manifest.capabilities.contains(&"llm.query".to_string()),
            },
            skills,
            authentication,
            default_input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
            default_output_modes: vec!["application/json".to_string(), "text/plain".to_string()],
            rate_limit_rpm,
        }
    }
}

/// Determine authentication requirements based on autonomy level.
fn auth_schemes_for_autonomy(autonomy_level: u8) -> Vec<AuthScheme> {
    match autonomy_level {
        // L0 (Inert) / L1 (Suggest): public, no auth needed.
        0 | 1 => vec![],
        // L2 (Act-with-approval): bearer token required.
        2 => vec![AuthScheme {
            scheme_type: "bearer".to_string(),
            description: Some("JWT bearer token for L2 governed access".to_string()),
        }],
        // L3+ (Act-then-report / Autonomous / Full): bearer + mTLS.
        _ => vec![
            AuthScheme {
                scheme_type: "bearer".to_string(),
                description: Some("JWT bearer token for governed access".to_string()),
            },
            AuthScheme {
                scheme_type: "mtls".to_string(),
                description: Some("Mutual TLS required for L3+ autonomous agents".to_string()),
            },
        ],
    }
}

/// Capability flags for an A2A agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Agent supports streaming responses via SSE.
    #[serde(default)]
    pub streaming: bool,
    /// Agent supports push notifications.
    #[serde(default)]
    pub push_notifications: bool,
    /// Agent supports multi-turn task conversations.
    #[serde(default)]
    pub state_transition_history: bool,
}

/// A skill that an agent can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    /// Unique identifier for this skill.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of what this skill does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags for categorization and discovery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Input content types this skill accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_modes: Vec<String>,
    /// Output content types this skill produces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_modes: Vec<String>,
}

/// Authentication scheme accepted by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthScheme {
    /// Scheme type (e.g., "bearer", "oauth2", "apiKey", "mtls").
    #[serde(rename = "type")]
    pub scheme_type: String,
    /// Optional description or hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ── Task Status ─────────────────────────────────────────────────────────────

/// The lifecycle states an A2A task can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Task has been received but not yet started.
    Submitted,
    /// Task is currently being processed.
    Working,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was canceled.
    Canceled,
}

impl TaskStatus {
    /// Whether this status represents a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }

    /// Validate a state transition. Returns `true` if the transition is allowed.
    pub fn can_transition_to(self, next: Self) -> bool {
        match self {
            Self::Submitted => matches!(next, Self::Working | Self::Canceled | Self::Failed),
            Self::Working => matches!(next, Self::Completed | Self::Failed | Self::Canceled),
            // Terminal states cannot transition.
            Self::Completed | Self::Failed | Self::Canceled => false,
        }
    }
}

// ── A2A Task ────────────────────────────────────────────────────────────────

/// An A2A task: a governed unit of work with sender/receiver identity,
/// status lifecycle, and correlation tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATask {
    /// Unique task identifier.
    pub id: String,
    /// Agent or external caller that sent this task.
    pub sender: String,
    /// Agent that will execute this task.
    pub receiver: String,
    /// Current lifecycle status.
    pub status: TaskStatus,
    /// Task payload (the request content).
    pub payload: TaskPayload,
    /// Correlation ID linking related tasks across agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// History of messages exchanged for this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<TaskMessage>,
    /// Artifacts produced by this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    /// Nexus governance context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance: Option<GovernanceContext>,
}

impl A2ATask {
    /// Create a new task in the Submitted state.
    pub fn new(
        sender: impl Into<String>,
        receiver: impl Into<String>,
        payload: TaskPayload,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender: sender.into(),
            receiver: receiver.into(),
            status: TaskStatus::Submitted,
            payload,
            correlation_id: None,
            history: Vec::new(),
            artifacts: Vec::new(),
            governance: None,
        }
    }

    /// Attempt to transition the task to a new status.
    /// Returns `false` if the transition is invalid.
    pub fn transition_to(&mut self, next: TaskStatus) -> bool {
        if self.status.can_transition_to(next) {
            self.status = next;
            true
        } else {
            false
        }
    }
}

/// The payload of an A2A task request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    /// Primary message content.
    pub message: TaskMessage,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ── Messages & Parts ────────────────────────────────────────────────────────

/// A message in a task conversation, either from user or agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    /// Role of the message sender.
    pub role: MessageRole,
    /// Content parts of the message.
    pub parts: Vec<MessagePart>,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Who sent the message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
}

/// A part of a message — text, file, or structured data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessagePart {
    /// Plain text content.
    Text { text: String },
    /// File content (inline or by URI).
    File { file: FileContent },
    /// Structured data content.
    Data { data: serde_json::Value },
}

/// File content, either inline bytes or a URI reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    /// Optional filename.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Base64-encoded inline data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    /// URI reference to the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

// ── Artifacts ───────────────────────────────────────────────────────────────

/// An artifact produced by a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Optional artifact name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Content parts of the artifact.
    pub parts: Vec<MessagePart>,
    /// Index for ordering multiple artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    /// Whether this is the last chunk (for streaming).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_chunk: Option<bool>,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ── JSON-RPC ────────────────────────────────────────────────────────────────

/// A2A uses JSON-RPC 2.0 as its wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Request ID.
    pub id: serde_json::Value,
    /// Method name (e.g., "tasks/send", "tasks/get").
    pub method: String,
    /// Method parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 success response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Request ID (must match the request).
    pub id: serde_json::Value,
    /// Result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Optional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ── A2A-specific request params ─────────────────────────────────────────────

/// Parameters for `tasks/send`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSendParams {
    /// Task ID. If omitted, the server generates one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The message to send.
    pub message: TaskMessage,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Parameters for `tasks/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGetParams {
    /// Task ID to retrieve.
    pub id: String,
    /// How many history items to include.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_length: Option<u32>,
}

/// Parameters for `tasks/cancel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCancelParams {
    /// Task ID to cancel.
    pub id: String,
}

// ── Governance extensions (Nexus-specific) ──────────────────────────────────

/// Nexus governance metadata attached to A2A tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceContext {
    /// The autonomy level the task runs at (L0-L5).
    pub autonomy_level: u8,
    /// Fuel budget allocated for this task.
    pub fuel_budget: u64,
    /// Fuel consumed so far.
    pub fuel_consumed: u64,
    /// Required capabilities for this task.
    pub required_capabilities: Vec<String>,
    /// Whether HITL approval was required/obtained.
    pub hitl_approved: bool,
    /// Audit trail hash for this task's events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AgentManifest;

    /// Helper: build a manifest with all 11 capabilities.
    fn full_manifest() -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![
                "web.search".to_string(),
                "web.read".to_string(),
                "llm.query".to_string(),
                "fs.read".to_string(),
                "fs.write".to_string(),
                "process.exec".to_string(),
                "social.post".to_string(),
                "social.x.post".to_string(),
                "social.x.read".to_string(),
                "messaging.send".to_string(),
                "audit.read".to_string(),
            ],
            fuel_budget: 50_000,
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: Some("claude-sonnet-4-5".to_string()),
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    /// Helper: build a minimal manifest.
    fn minimal_manifest() -> AgentManifest {
        AgentManifest {
            name: "min-agent".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["web.search".to_string()],
            fuel_budget: 100,
            autonomy_level: None,
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

    // ── from_manifest tests ─────────────────────────────────────────────────

    #[test]
    fn from_manifest_produces_valid_agent_card() {
        let manifest = full_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://nexus.example.com");

        assert_eq!(card.name, "test-agent");
        assert_eq!(card.version, A2A_PROTOCOL_VERSION);
        assert_eq!(card.url, "https://nexus.example.com/a2a/test-agent");
        assert!(card.description.as_ref().unwrap().contains("test-agent"));
    }

    #[test]
    fn from_manifest_maps_all_11_capabilities_to_skills() {
        let manifest = full_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://example.com");

        assert_eq!(
            card.skills.len(),
            11,
            "all 11 capabilities must map to skills"
        );

        let skill_ids: Vec<&str> = card.skills.iter().map(|s| s.id.as_str()).collect();
        assert!(skill_ids.contains(&"web-search"));
        assert!(skill_ids.contains(&"web-read"));
        assert!(skill_ids.contains(&"llm-query"));
        assert!(skill_ids.contains(&"fs-read"));
        assert!(skill_ids.contains(&"fs-write"));
        assert!(skill_ids.contains(&"process-exec"));
        assert!(skill_ids.contains(&"social-post"));
        assert!(skill_ids.contains(&"social-x-post"));
        assert!(skill_ids.contains(&"social-x-read"));
        assert!(skill_ids.contains(&"messaging-send"));
        assert!(skill_ids.contains(&"audit-read"));
    }

    #[test]
    fn from_manifest_skills_have_descriptions_and_modes() {
        let manifest = full_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://example.com");

        for skill in &card.skills {
            assert!(
                skill.description.is_some(),
                "skill '{}' must have a description",
                skill.id
            );
            assert!(
                !skill.input_modes.is_empty(),
                "skill '{}' must have input_modes",
                skill.id
            );
            assert!(
                !skill.output_modes.is_empty(),
                "skill '{}' must have output_modes",
                skill.id
            );
            assert!(
                !skill.tags.is_empty(),
                "skill '{}' must have tags",
                skill.id
            );
        }
    }

    #[test]
    fn from_manifest_fuel_budget_to_rate_limit() {
        let manifest = full_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        // 50_000 / 100 = 500 rpm
        assert_eq!(card.rate_limit_rpm, Some(500));
    }

    #[test]
    fn from_manifest_low_fuel_rate_limit_floors_to_one() {
        let manifest = minimal_manifest(); // fuel_budget = 100
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        // 100 / 100 = 1 rpm (floor)
        assert_eq!(card.rate_limit_rpm, Some(1));
    }

    #[test]
    fn from_manifest_l0_no_auth() {
        let mut manifest = minimal_manifest();
        manifest.autonomy_level = Some(0);
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert!(card.authentication.is_empty(), "L0 should have no auth");
    }

    #[test]
    fn from_manifest_l1_no_auth() {
        let mut manifest = minimal_manifest();
        manifest.autonomy_level = Some(1);
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert!(card.authentication.is_empty(), "L1 should have no auth");
    }

    #[test]
    fn from_manifest_l2_bearer_auth() {
        let mut manifest = minimal_manifest();
        manifest.autonomy_level = Some(2);
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert_eq!(card.authentication.len(), 1);
        assert_eq!(card.authentication[0].scheme_type, "bearer");
    }

    #[test]
    fn from_manifest_l3_bearer_and_mtls() {
        let manifest = full_manifest(); // autonomy_level = 3
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert_eq!(card.authentication.len(), 2);
        let types: Vec<&str> = card
            .authentication
            .iter()
            .map(|a| a.scheme_type.as_str())
            .collect();
        assert!(types.contains(&"bearer"));
        assert!(types.contains(&"mtls"));
    }

    #[test]
    fn from_manifest_l5_bearer_and_mtls() {
        let mut manifest = minimal_manifest();
        manifest.autonomy_level = Some(5);
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert_eq!(card.authentication.len(), 2);
    }

    #[test]
    fn from_manifest_none_autonomy_defaults_to_no_auth() {
        let manifest = minimal_manifest(); // autonomy_level = None
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert!(card.authentication.is_empty());
    }

    #[test]
    fn from_manifest_llm_enables_state_history() {
        let manifest = full_manifest(); // has llm.query
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert!(card.capabilities.state_transition_history);
    }

    #[test]
    fn from_manifest_no_llm_disables_state_history() {
        let manifest = minimal_manifest(); // only web.search
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        assert!(!card.capabilities.state_transition_history);
    }

    #[test]
    fn from_manifest_url_trailing_slash_handled() {
        let manifest = minimal_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://example.com/");
        assert_eq!(card.url, "https://example.com/a2a/min-agent");
    }

    #[test]
    fn from_manifest_roundtrip_json() {
        let manifest = full_manifest();
        let card = AgentCard::from_manifest(&manifest, "https://example.com");
        let json = serde_json::to_string_pretty(&card).unwrap();
        let parsed: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, card.name);
        assert_eq!(parsed.skills.len(), 11);
        assert_eq!(parsed.version, A2A_PROTOCOL_VERSION);
    }

    // ── TaskStatus tests ────────────────────────────────────────────────────

    #[test]
    fn task_status_serde_kebab_case() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::Submitted).unwrap(),
            "\"submitted\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Working).unwrap(),
            "\"working\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Canceled).unwrap(),
            "\"canceled\""
        );
    }

    #[test]
    fn task_status_terminal_states() {
        assert!(!TaskStatus::Submitted.is_terminal());
        assert!(!TaskStatus::Working.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Canceled.is_terminal());
    }

    #[test]
    fn task_status_valid_transitions() {
        // Submitted can go to Working, Canceled, Failed.
        assert!(TaskStatus::Submitted.can_transition_to(TaskStatus::Working));
        assert!(TaskStatus::Submitted.can_transition_to(TaskStatus::Canceled));
        assert!(TaskStatus::Submitted.can_transition_to(TaskStatus::Failed));
        assert!(!TaskStatus::Submitted.can_transition_to(TaskStatus::Completed));

        // Working can go to Completed, Failed, Canceled.
        assert!(TaskStatus::Working.can_transition_to(TaskStatus::Completed));
        assert!(TaskStatus::Working.can_transition_to(TaskStatus::Failed));
        assert!(TaskStatus::Working.can_transition_to(TaskStatus::Canceled));
        assert!(!TaskStatus::Working.can_transition_to(TaskStatus::Submitted));

        // Terminal states cannot transition.
        assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::Working));
        assert!(!TaskStatus::Failed.can_transition_to(TaskStatus::Submitted));
        assert!(!TaskStatus::Canceled.can_transition_to(TaskStatus::Working));
    }

    // ── A2ATask tests ───────────────────────────────────────────────────────

    #[test]
    fn a2a_task_new_starts_submitted() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let task = A2ATask::new("agent-a", "agent-b", payload);
        assert_eq!(task.status, TaskStatus::Submitted);
        assert_eq!(task.sender, "agent-a");
        assert_eq!(task.receiver, "agent-b");
        assert!(!task.id.is_empty());
        assert!(task.correlation_id.is_none());
    }

    #[test]
    fn a2a_task_lifecycle_transitions() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "do work".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("caller", "worker", payload);

        // Submitted → Working
        assert!(task.transition_to(TaskStatus::Working));
        assert_eq!(task.status, TaskStatus::Working);

        // Working → Completed
        assert!(task.transition_to(TaskStatus::Completed));
        assert_eq!(task.status, TaskStatus::Completed);

        // Completed → anything: rejected
        assert!(!task.transition_to(TaskStatus::Working));
        assert_eq!(task.status, TaskStatus::Completed); // unchanged
    }

    #[test]
    fn a2a_task_cancel_from_submitted() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "cancel me".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("caller", "worker", payload);
        assert!(task.transition_to(TaskStatus::Canceled));
        assert_eq!(task.status, TaskStatus::Canceled);
        assert!(!task.transition_to(TaskStatus::Working)); // terminal
    }

    #[test]
    fn a2a_task_fail_from_working() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "fail".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("caller", "worker", payload);
        assert!(task.transition_to(TaskStatus::Working));
        assert!(task.transition_to(TaskStatus::Failed));
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[test]
    fn a2a_task_cannot_skip_to_completed_from_submitted() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "skip".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("caller", "worker", payload);
        // Submitted → Completed not allowed (must go through Working).
        assert!(!task.transition_to(TaskStatus::Completed));
        assert_eq!(task.status, TaskStatus::Submitted);
    }

    #[test]
    fn a2a_task_with_correlation_id() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "correlated".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("a", "b", payload);
        task.correlation_id = Some("corr-123".to_string());

        let json = serde_json::to_string(&task).unwrap();
        let parsed: A2ATask = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.correlation_id, Some("corr-123".to_string()));
    }

    #[test]
    fn a2a_task_with_governance_context() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: "governed".to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };
        let mut task = A2ATask::new("caller", "worker", payload);
        task.governance = Some(GovernanceContext {
            autonomy_level: 2,
            fuel_budget: 10_000,
            fuel_consumed: 0,
            required_capabilities: vec!["llm.query".to_string()],
            hitl_approved: true,
            audit_hash: None,
        });

        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: A2ATask = serde_json::from_str(&json).unwrap();
        let gov = parsed.governance.unwrap();
        assert_eq!(gov.autonomy_level, 2);
        assert!(gov.hitl_approved);
    }

    #[test]
    fn a2a_task_roundtrip_json() {
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![
                    MessagePart::Text {
                        text: "hello".to_string(),
                    },
                    MessagePart::Data {
                        data: serde_json::json!({"key": 42}),
                    },
                ],
                metadata: Some(serde_json::json!({"trace": true})),
            },
            metadata: None,
        };
        let mut task = A2ATask::new("agent-x", "agent-y", payload);
        task.correlation_id = Some("corr-456".to_string());
        task.artifacts.push(Artifact {
            name: Some("result.json".to_string()),
            description: None,
            parts: vec![MessagePart::Text {
                text: "{}".to_string(),
            }],
            index: Some(0),
            last_chunk: Some(true),
            metadata: None,
        });

        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: A2ATask = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sender, "agent-x");
        assert_eq!(parsed.receiver, "agent-y");
        assert_eq!(parsed.artifacts.len(), 1);
    }

    // ── JSON-RPC tests ──────────────────────────────────────────────────────

    #[test]
    fn jsonrpc_response_success() {
        let resp =
            JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"status": "ok"}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn jsonrpc_response_error() {
        let resp = JsonRpcResponse::error(serde_json::json!(1), -32600, "Invalid request");
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid request");
    }

    #[test]
    fn message_part_tagged_serde() {
        let text = MessagePart::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains("\"type\":\"text\""));

        let data = MessagePart::Data {
            data: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"type\":\"data\""));
    }

    #[test]
    fn governance_context_roundtrip() {
        let ctx = GovernanceContext {
            autonomy_level: 2,
            fuel_budget: 10000,
            fuel_consumed: 500,
            required_capabilities: vec!["llm.query".to_string()],
            hitl_approved: true,
            audit_hash: Some("abc123".to_string()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: GovernanceContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.autonomy_level, 2);
        assert_eq!(parsed.fuel_budget, 10000);
        assert!(parsed.hitl_approved);
    }
}
