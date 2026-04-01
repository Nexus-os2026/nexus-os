use serde::{Deserialize, Serialize};

/// The role of a message participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System instruction.
    System,
    /// User message.
    User,
    /// Assistant response.
    Assistant,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message author.
    pub role: Role,
    /// The text content of the message.
    pub content: String,
}

/// A request to an LLM provider.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    /// The conversation messages.
    pub messages: Vec<Message>,
    /// The model to use.
    pub model: String,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Whether to stream the response.
    pub stream: bool,
    /// System prompt (extracted from messages for providers that need it separate).
    pub system: Option<String>,
    /// Tool definitions to include in the request (provider formats them).
    pub tools: Option<Vec<serde_json::Value>>,
}

/// A complete (non-streaming) response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The generated text content.
    pub content: String,
    /// The model that generated the response.
    pub model: String,
    /// Token usage statistics.
    pub usage: TokenUsage,
    /// Why generation stopped.
    pub finish_reason: Option<String>,
    /// Raw content blocks (for Anthropic: may contain tool_use blocks;
    /// for Google: may contain functionCall parts).
    #[serde(default)]
    pub content_blocks: Option<Vec<serde_json::Value>>,
    /// Tool calls (for OpenAI-compatible: parsed from tool_calls array).
    #[serde(default)]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    /// Stop reason (e.g., "end_turn", "tool_use", "stop", "tool_calls").
    #[serde(default)]
    pub stop_reason: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of input (prompt) tokens.
    pub input_tokens: u64,
    /// Number of output (completion) tokens.
    pub output_tokens: u64,
    /// Total tokens (input + output).
    pub total_tokens: u64,
}

/// A chunk of a streamed response.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A text delta.
    Delta(String),
    /// Usage info (sent at end of stream by most providers).
    Usage(TokenUsage),
    /// Stream is done.
    Done,
    /// Error during streaming.
    Error(String),
}
