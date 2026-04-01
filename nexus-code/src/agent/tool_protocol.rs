//! Provider-agnostic tool call/result types and protocol translation.
//!
//! Translates between our internal format and each provider's API format
//! (Anthropic, OpenAI-compatible, Google Gemini).

use serde::{Deserialize, Serialize};

/// A tool call requested by the LLM.
/// This is our internal, provider-agnostic representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call (from the LLM's response).
    /// Anthropic: tool_use.id, OpenAI: tool_calls[].id, Google: generated
    pub id: String,
    /// Tool name (must match NxTool::name())
    pub name: String,
    /// Tool input as JSON object
    pub input: serde_json::Value,
}

/// A tool result to send back to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    /// The tool_call.id this result corresponds to
    pub tool_call_id: String,
    /// The tool's name (needed for Google Gemini format)
    pub tool_name: String,
    /// The tool's output content
    pub content: String,
    /// Whether the tool succeeded
    pub is_error: bool,
}

/// Tool definition for the LLM system prompt / API request.
/// Each provider formats this differently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create from an NxTool.
    pub fn from_tool(tool: &dyn crate::tools::NxTool) -> Self {
        Self {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
        }
    }
}

// ─── Anthropic Format ───

/// Build tool definitions in Anthropic's format.
/// Used in the API request body as the "tools" array.
pub fn format_tools_anthropic(tools: &[ToolDefinition]) -> serde_json::Value {
    let tool_defs: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema
            })
        })
        .collect();
    serde_json::Value::Array(tool_defs)
}

/// Parse tool calls from an Anthropic response.
/// Anthropic returns tool_use content blocks:
/// {"type": "tool_use", "id": "toolu_...", "name": "file_read", "input": {...}}
pub fn parse_tool_calls_anthropic(content: &[serde_json::Value]) -> Vec<ToolCall> {
    content
        .iter()
        .filter_map(|block| {
            if block.get("type")?.as_str()? == "tool_use" {
                Some(ToolCall {
                    id: block.get("id")?.as_str()?.to_string(),
                    name: block.get("name")?.as_str()?.to_string(),
                    input: block
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::Value::Object(Default::default())),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Format tool results for Anthropic.
/// Returns a user message with tool_result content blocks.
pub fn format_tool_results_anthropic(results: &[ToolResultMessage]) -> serde_json::Value {
    let blocks: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "type": "tool_result",
                "tool_use_id": r.tool_call_id,
                "content": r.content,
                "is_error": r.is_error
            })
        })
        .collect();
    serde_json::json!({
        "role": "user",
        "content": blocks
    })
}

// ─── OpenAI-Compatible Format ───

/// Build tool definitions in OpenAI's format.
/// Used by: OpenAI, Ollama, OpenRouter, Groq, DeepSeek.
pub fn format_tools_openai(tools: &[ToolDefinition]) -> serde_json::Value {
    let tool_defs: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                }
            })
        })
        .collect();
    serde_json::Value::Array(tool_defs)
}

/// Parse tool calls from an OpenAI-compatible response.
/// OpenAI returns: message.tool_calls[{"id": "...", "function": {"name": "...", "arguments": "{...}"}}]
pub fn parse_tool_calls_openai(tool_calls: &[serde_json::Value]) -> Vec<ToolCall> {
    tool_calls
        .iter()
        .filter_map(|tc| {
            let function = tc.get("function")?;
            let name = function.get("name")?.as_str()?.to_string();
            let arguments_str = function.get("arguments")?.as_str()?;
            let input = serde_json::from_str(arguments_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Some(ToolCall {
                id: tc.get("id")?.as_str()?.to_string(),
                name,
                input,
            })
        })
        .collect()
}

/// Format tool results for OpenAI-compatible APIs.
/// Each result is a separate message with role "tool".
pub fn format_tool_results_openai(results: &[ToolResultMessage]) -> Vec<serde_json::Value> {
    results
        .iter()
        .map(|r| {
            serde_json::json!({
                "role": "tool",
                "tool_call_id": r.tool_call_id,
                "content": r.content
            })
        })
        .collect()
}

// ─── Google Gemini Format ───

/// Build tool definitions in Google Gemini's format.
pub fn format_tools_google(tools: &[ToolDefinition]) -> serde_json::Value {
    let function_declarations: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.input_schema
            })
        })
        .collect();
    serde_json::json!([{
        "functionDeclarations": function_declarations
    }])
}

/// Parse tool calls from a Google Gemini response.
/// Gemini returns: candidates[0].content.parts[{"functionCall": {"name": "...", "args": {...}}}]
pub fn parse_tool_calls_google(parts: &[serde_json::Value]) -> Vec<ToolCall> {
    parts
        .iter()
        .filter_map(|part| {
            let fc = part.get("functionCall")?;
            Some(ToolCall {
                id: uuid::Uuid::new_v4().to_string(), // Gemini doesn't provide IDs
                name: fc.get("name")?.as_str()?.to_string(),
                input: fc
                    .get("args")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(Default::default())),
            })
        })
        .collect()
}

/// Format tool results for Google Gemini.
/// Returns a user-role content with functionResponse parts.
pub fn format_tool_results_google(results: &[ToolResultMessage]) -> serde_json::Value {
    let parts: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "functionResponse": {
                    "name": r.tool_name,
                    "response": {
                        "content": r.content
                    }
                }
            })
        })
        .collect();
    serde_json::json!({
        "role": "user",
        "parts": parts
    })
}

/// Detect which tool-call protocol a provider uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolProtocol {
    Anthropic,
    OpenAi,
    Google,
}

impl ToolProtocol {
    /// Determine the protocol for a provider name.
    pub fn for_provider(provider_name: &str) -> Self {
        match provider_name {
            "anthropic" => Self::Anthropic,
            "google" => Self::Google,
            _ => Self::OpenAi, // OpenAI, Ollama, OpenRouter, Groq, DeepSeek
        }
    }
}
