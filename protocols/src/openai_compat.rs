//! OpenAI-compatible Chat Completions API types and handlers.
//!
//! Reference: <https://platform.openai.com/docs/api-reference/chat/create>
//!
//! Two routing modes:
//! - **Agent mode** (`model: "agent/<name>"`) — routes to a governed Nexus OS agent
//! - **LLM passthrough** (`model: "gpt-4o"`, etc.) — routes to the configured LLM provider

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    /// Model ID — either `"agent/<name>"` for agent routing or a model name for LLM passthrough.
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Sampling temperature (0–2). Default 1.0.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum tokens to generate.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Whether to stream the response via SSE.
    #[serde(default)]
    pub stream: bool,
    /// Stop sequences.
    #[serde(default)]
    pub stop: Option<StopValue>,
    /// Top-p (nucleus) sampling.
    #[serde(default)]
    pub top_p: Option<f32>,
    /// Frequency penalty (−2 to 2).
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    /// Presence penalty (−2 to 2).
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    /// End-user identifier for audit trail.
    #[serde(default)]
    pub user: Option<String>,
    /// Tool/function definitions.
    #[serde(default)]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice constraint.
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
    /// Number of choices to return.
    #[serde(default)]
    pub n: Option<u32>,
}

/// `stop` can be a single string or an array of strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopValue {
    Single(String),
    Multiple(Vec<String>),
}

fn default_temperature() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Convenience: extract content as a plain string.
    pub fn content_text(&self) -> String {
        match &self.content {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".into(),
            content: Some(serde_json::Value::String(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    /// Nexus OS extension: governance metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nexus_governance: Option<GovernanceMetadata>,
}

impl ChatCompletionResponse {
    /// Build a simple single-choice response.
    pub fn simple(model: &str, content: &str, prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            id: format!("chatcmpl-{}", Uuid::new_v4().simple()),
            object: "chat.completion".into(),
            created: now_secs(),
            model: model.into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(content),
                finish_reason: "stop".into(),
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            nexus_governance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ── Streaming types (SSE) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

impl ChatCompletionChunk {
    /// Create a role announcement chunk (first chunk in a stream).
    pub fn role_chunk(id: &str, model: &str) -> Self {
        Self {
            id: id.into(),
            object: "chat.completion.chunk".into(),
            created: now_secs(),
            model: model.into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: ChatDelta {
                    role: Some("assistant".into()),
                    content: None,
                    tool_calls: None,
                },
                finish_reason: None,
            }],
        }
    }

    /// Create a content delta chunk.
    pub fn content_chunk(id: &str, model: &str, text: &str) -> Self {
        Self {
            id: id.into(),
            object: "chat.completion.chunk".into(),
            created: now_secs(),
            model: model.into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: ChatDelta {
                    role: None,
                    content: Some(text.into()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
        }
    }

    /// Create a termination chunk.
    pub fn stop_chunk(id: &str, model: &str) -> Self {
        Self {
            id: id.into(),
            object: "chat.completion.chunk".into(),
            created: now_secs(),
            model: model.into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: ChatDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".into()),
            }],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: ChatDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

// ── Tool types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ── Governance extension ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMetadata {
    pub agent_id: Option<String>,
    pub autonomy_level: Option<u8>,
    pub fuel_consumed: Option<f64>,
    pub audit_hash: Option<String>,
    pub hitl_required: bool,
}

// ── Models endpoint ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

impl ModelInfo {
    pub fn new(id: &str, owned_by: &str) -> Self {
        Self {
            id: id.into(),
            object: "model".into(),
            created: now_secs(),
            owned_by: owned_by.into(),
        }
    }
}

// ── Error response ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl ErrorResponse {
    pub fn new(message: &str, error_type: &str, code: Option<&str>) -> Self {
        Self {
            error: ApiError {
                message: message.into(),
                error_type: error_type.into(),
                param: None,
                code: code.map(String::from),
            },
        }
    }

    pub fn auth_error(message: &str) -> Self {
        Self::new(message, "authentication_error", Some("invalid_api_key"))
    }

    pub fn invalid_request(message: &str) -> Self {
        Self::new(message, "invalid_request_error", Some("invalid_request"))
    }

    pub fn model_not_found(model: &str) -> Self {
        Self::new(
            &format!("The model '{model}' does not exist"),
            "invalid_request_error",
            Some("model_not_found"),
        )
    }

    pub fn server_error(message: &str) -> Self {
        Self::new(message, "server_error", Some("internal_error"))
    }
}

// ── Model routing ───────────────────────────────────────────────────────────

/// How a model request should be routed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelRoute {
    /// Route to a governed Nexus OS agent.
    Agent { agent_id: String },
    /// Route to an LLM provider.
    LlmPassthrough { provider: String, model: String },
}

impl ModelRoute {
    /// Get the provider name for this route.
    pub fn provider_name(&self) -> &str {
        match self {
            ModelRoute::Agent { .. } => "nexus-os",
            ModelRoute::LlmPassthrough { provider, .. } => provider,
        }
    }
}

/// Resolve a model string to a routing decision.
pub fn resolve_model(model: &str, known_agents: &[String]) -> ModelRoute {
    // Explicit agent prefix
    if let Some(agent_id) = model.strip_prefix("agent/") {
        return ModelRoute::Agent {
            agent_id: agent_id.to_string(),
        };
    }

    // Match against known agent names
    if known_agents.iter().any(|a| a == model) {
        return ModelRoute::Agent {
            agent_id: model.to_string(),
        };
    }

    // Provider-prefixed: "provider/model"
    if model.contains('/') {
        let mut parts = model.splitn(2, '/');
        let provider = parts.next().unwrap_or("ollama");
        let model_name = parts.next().unwrap_or(model);
        return ModelRoute::LlmPassthrough {
            provider: provider.into(),
            model: model_name.into(),
        };
    }

    // Infer provider from model name
    let provider = infer_provider(model);
    ModelRoute::LlmPassthrough {
        provider: provider.into(),
        model: model.into(),
    }
}

fn infer_provider(model: &str) -> &str {
    if model.starts_with("gpt-")
        || model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
    {
        return "openai";
    }
    if model.starts_with("claude-") {
        return "anthropic";
    }
    if model.starts_with("gemini") {
        return "google";
    }
    if model.starts_with("deepseek") {
        return "deepseek";
    }
    if model.starts_with("command") || model.starts_with("c4ai") {
        return "cohere";
    }
    if model.starts_with("mixtral") || model.starts_with("mistral") {
        return "mistral";
    }
    // Default: treat as local Ollama model
    "ollama"
}

/// Build a prompt string from chat messages (for providers that take a flat prompt).
pub fn messages_to_prompt(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .map(|m| format!("[{}]\n{}", m.role, m.content_text()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Estimate token count from text length (4 chars ≈ 1 token).
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Split text into word-sized chunks for simulated streaming.
pub fn chunk_text(text: &str, max_words_per_chunk: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_inclusive(char::is_whitespace).collect();
    if words.is_empty() {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut count = 0;

    for word in words {
        current.push_str(word);
        count += 1;
        if count >= max_words_per_chunk {
            chunks.push(std::mem::take(&mut current));
            count = 0;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Format a chunk as an SSE data line.
pub fn sse_data(chunk: &ChatCompletionChunk) -> String {
    match serde_json::to_string(chunk) {
        Ok(json) => format!("data: {json}\n\n"),
        Err(_) => String::new(),
    }
}

/// The SSE termination sentinel.
pub const SSE_DONE: &str = "data: [DONE]\n\n";

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_deserialization() {
        let json = r#"{
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant"},
                {"role": "user", "content": "Hello"}
            ],
            "temperature": 0.7,
            "max_tokens": 100,
            "stream": false
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.messages.len(), 2);
        assert!((req.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(req.max_tokens, Some(100));
        assert!(!req.stream);
    }

    #[test]
    fn test_request_defaults() {
        let json = r#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert!((req.temperature - 1.0).abs() < f32::EPSILON);
        assert!(!req.stream);
        assert!(req.max_tokens.is_none());
        assert!(req.tools.is_none());
    }

    #[test]
    fn test_response_serialization() {
        let resp = ChatCompletionResponse::simple("gpt-4o", "Hello!", 10, 5);
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(json["choices"][0]["finish_reason"], "stop");
        assert_eq!(json["usage"]["prompt_tokens"], 10);
        assert_eq!(json["usage"]["completion_tokens"], 5);
        assert_eq!(json["usage"]["total_tokens"], 15);
        // nexus_governance should be absent when None
        assert!(json.get("nexus_governance").is_none());
    }

    #[test]
    fn test_chunk_serialization() {
        let chunk = ChatCompletionChunk::content_chunk("chatcmpl-123", "gpt-4o", "Hello");
        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["object"], "chat.completion.chunk");
        assert_eq!(json["choices"][0]["delta"]["content"], "Hello");
        assert!(json["choices"][0]["finish_reason"].is_null());
    }

    #[test]
    fn test_stop_chunk() {
        let chunk = ChatCompletionChunk::stop_chunk("chatcmpl-123", "gpt-4o");
        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["choices"][0]["finish_reason"], "stop");
        assert!(json["choices"][0]["delta"].get("content").is_none());
    }

    #[test]
    fn test_error_response_format() {
        let err = ErrorResponse::auth_error("invalid key");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["error"]["type"], "authentication_error");
        assert_eq!(json["error"]["code"], "invalid_api_key");
        assert_eq!(json["error"]["message"], "invalid key");
    }

    #[test]
    fn test_model_not_found_error() {
        let err = ErrorResponse::model_not_found("foo-bar");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["error"]["code"], "model_not_found");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("foo-bar"));
    }

    #[test]
    fn test_models_response_serialization() {
        let resp = ModelsResponse {
            object: "list".into(),
            data: vec![
                ModelInfo::new("agent/researcher", "nexus-os"),
                ModelInfo::new("gpt-4o", "openai"),
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["object"], "list");
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
        assert_eq!(json["data"][0]["id"], "agent/researcher");
        assert_eq!(json["data"][0]["object"], "model");
    }

    #[test]
    fn test_resolve_model_agent_prefix() {
        let route = resolve_model("agent/researcher", &[]);
        assert_eq!(
            route,
            ModelRoute::Agent {
                agent_id: "researcher".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_known_agent() {
        let agents = vec!["nexus-oracle".to_string()];
        let route = resolve_model("nexus-oracle", &agents);
        assert_eq!(
            route,
            ModelRoute::Agent {
                agent_id: "nexus-oracle".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_gpt() {
        let route = resolve_model("gpt-4o", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "openai".into(),
                model: "gpt-4o".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_claude() {
        let route = resolve_model("claude-3-opus", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "anthropic".into(),
                model: "claude-3-opus".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_gemini() {
        let route = resolve_model("gemini-1.5-pro", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "google".into(),
                model: "gemini-1.5-pro".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_deepseek() {
        let route = resolve_model("deepseek-coder", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "deepseek".into(),
                model: "deepseek-coder".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_provider_prefixed() {
        let route = resolve_model("ollama/llama3", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "ollama".into(),
                model: "llama3".into()
            }
        );
    }

    #[test]
    fn test_resolve_model_unknown_defaults_ollama() {
        let route = resolve_model("phi-3", &[]);
        assert_eq!(
            route,
            ModelRoute::LlmPassthrough {
                provider: "ollama".into(),
                model: "phi-3".into()
            }
        );
    }

    #[test]
    fn test_messages_to_prompt() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: Some(serde_json::Value::String("You are helpful".into())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".into(),
                content: Some(serde_json::Value::String("Hello".into())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("[system]"));
        assert!(prompt.contains("You are helpful"));
        assert!(prompt.contains("[user]"));
        assert!(prompt.contains("Hello"));
    }

    #[test]
    fn test_content_text_string() {
        let msg = ChatMessage {
            role: "user".into(),
            content: Some(serde_json::Value::String("hello".into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        assert_eq!(msg.content_text(), "hello");
    }

    #[test]
    fn test_content_text_null() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        assert_eq!(msg.content_text(), "");
    }

    #[test]
    fn test_chunk_text() {
        let chunks = chunk_text("Hello world this is a test", 2);
        assert!(chunks.len() >= 2);
        let joined: String = chunks.concat();
        assert_eq!(joined, "Hello world this is a test");
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("", 3);
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn test_sse_data_format() {
        let chunk = ChatCompletionChunk::content_chunk("id1", "m", "Hi");
        let line = sse_data(&chunk);
        assert!(line.starts_with("data: {"));
        assert!(line.ends_with("\n\n"));
        assert!(line.contains("\"content\":\"Hi\""));
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hi"), 1);
        assert_eq!(estimate_tokens("hello world"), 3);
    }

    #[test]
    fn test_governance_metadata_serialization() {
        let meta = GovernanceMetadata {
            agent_id: Some("researcher".into()),
            autonomy_level: Some(3),
            fuel_consumed: Some(42.5),
            audit_hash: Some("abc123".into()),
            hitl_required: false,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["agent_id"], "researcher");
        assert_eq!(json["autonomy_level"], 3);
        assert!(!json["hitl_required"].as_bool().unwrap());
    }

    #[test]
    fn test_tool_definition_roundtrip() {
        let tool = ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_weather".into(),
                description: Some("Get the weather".into()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                })),
            },
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.function.name, "get_weather");
        assert_eq!(parsed.tool_type, "function");
    }

    #[test]
    fn test_request_with_tools() {
        let json = r#"{
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Weather?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object"}
                }
            }],
            "tool_choice": "auto"
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert!(req.tools.is_some());
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);
        assert!(req.tool_choice.is_some());
    }

    #[test]
    fn test_stop_value_single() {
        let json = r#"{"model": "m", "messages": [], "stop": "END"}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.stop, Some(StopValue::Single(ref s)) if s == "END"));
    }

    #[test]
    fn test_stop_value_multiple() {
        let json = r#"{"model": "m", "messages": [], "stop": ["END", "STOP"]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.stop, Some(StopValue::Multiple(ref v)) if v.len() == 2));
    }

    #[test]
    fn test_response_with_governance() {
        let mut resp = ChatCompletionResponse::simple("agent/researcher", "result", 10, 20);
        resp.nexus_governance = Some(GovernanceMetadata {
            agent_id: Some("researcher".into()),
            autonomy_level: Some(3),
            fuel_consumed: Some(100.0),
            audit_hash: Some("hash".into()),
            hitl_required: false,
        });
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("nexus_governance").is_some());
        assert_eq!(json["nexus_governance"]["agent_id"], "researcher");
    }
}
