use futures_util::StreamExt;
use tokio::sync::mpsc;

use super::types::{StreamChunk, TokenUsage};
use crate::error::NxError;

/// Parse an OpenAI-compatible SSE stream.
/// Works for: OpenAI, Ollama, OpenRouter, Groq, DeepSeek.
pub async fn parse_openai_sse_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamChunk>,
) -> Result<(), NxError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| NxError::StreamingError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    let _ = tx.send(StreamChunk::Done);
                    return Ok(());
                }

                match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(json) => {
                        // Extract delta content
                        if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                            if let Some(choice) = choices.first() {
                                if let Some(delta) = choice.get("delta") {
                                    if let Some(content) =
                                        delta.get("content").and_then(|c| c.as_str())
                                    {
                                        let _ = tx.send(StreamChunk::Delta(content.to_string()));
                                    }
                                }
                            }
                        }

                        // Extract usage (sent at end by some providers)
                        if let Some(usage) = json.get("usage") {
                            let input = usage
                                .get("prompt_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let output = usage
                                .get("completion_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let _ = tx.send(StreamChunk::Usage(TokenUsage {
                                input_tokens: input,
                                output_tokens: output,
                                total_tokens: input + output,
                            }));
                        }
                    }
                    Err(_) => {
                        // Skip unparseable lines (common with SSE comments)
                    }
                }
            }
        }
    }

    let _ = tx.send(StreamChunk::Done);
    Ok(())
}

/// Parse an Anthropic SSE stream.
pub async fn parse_anthropic_sse_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamChunk>,
) -> Result<(), NxError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| NxError::StreamingError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(event_type) = line.strip_prefix("event: ") {
                current_event = event_type.trim().to_string();
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    match current_event.as_str() {
                        "content_block_delta" => {
                            if let Some(delta) = json.get("delta") {
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    let _ = tx.send(StreamChunk::Delta(text.to_string()));
                                }
                            }
                        }
                        "message_delta" => {
                            if let Some(usage) = json.get("usage") {
                                let output = usage
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let _ = tx.send(StreamChunk::Usage(TokenUsage {
                                    input_tokens: 0,
                                    output_tokens: output,
                                    total_tokens: output,
                                }));
                            }
                        }
                        "message_start" => {
                            // Extract input token count from message_start
                            if let Some(message) = json.get("message") {
                                if let Some(usage) = message.get("usage") {
                                    let input = usage
                                        .get("input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    if input > 0 {
                                        let _ = tx.send(StreamChunk::Usage(TokenUsage {
                                            input_tokens: input,
                                            output_tokens: 0,
                                            total_tokens: input,
                                        }));
                                    }
                                }
                            }
                        }
                        "message_stop" => {
                            let _ = tx.send(StreamChunk::Done);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let _ = tx.send(StreamChunk::Done);
    Ok(())
}

/// A fully collected streaming response (text + optional tool calls).
/// Used for streaming with tool detection (e.g., Anthropic tool_use blocks).
#[derive(Debug, Clone, Default)]
pub struct CollectedResponse {
    /// Accumulated text content.
    pub text: String,
    /// Tool use blocks collected from the stream.
    pub tool_use_blocks: Vec<serde_json::Value>,
    /// Token usage statistics.
    pub usage: super::types::TokenUsage,
    /// Stop reason from the LLM.
    pub stop_reason: Option<String>,
}

/// Collect an Anthropic SSE stream, detecting tool_use blocks.
/// Streams text deltas via `text_tx` for real-time display while
/// accumulating tool_use blocks for later execution.
pub async fn collect_anthropic_stream(
    response: reqwest::Response,
    text_tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<CollectedResponse, NxError> {
    let mut collected = CollectedResponse::default();
    let mut current_block_type: Option<String> = None;
    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut current_tool_input_json = String::new();
    let mut current_event = String::new();

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| NxError::StreamingError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(event_type) = line.strip_prefix("event: ") {
                current_event = event_type.trim().to_string();
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    match current_event.as_str() {
                        "content_block_start" => {
                            if let Some(block) = json.get("content_block") {
                                let block_type =
                                    block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                current_block_type = Some(block_type.to_string());

                                if block_type == "tool_use" {
                                    current_tool_id = block
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_name = block
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_input_json.clear();
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = json.get("delta") {
                                let delta_type =
                                    delta.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                match delta_type {
                                    "text_delta" => {
                                        if let Some(text) =
                                            delta.get("text").and_then(|t| t.as_str())
                                        {
                                            collected.text.push_str(text);
                                            let _ = text_tx.send(text.to_string());
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(partial) =
                                            delta.get("partial_json").and_then(|t| t.as_str())
                                        {
                                            current_tool_input_json.push_str(partial);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "content_block_stop" => {
                            if current_block_type.as_deref() == Some("tool_use") {
                                // Assemble complete tool_use block
                                let input: serde_json::Value =
                                    serde_json::from_str(&current_tool_input_json)
                                        .unwrap_or(serde_json::Value::Object(Default::default()));
                                collected.tool_use_blocks.push(serde_json::json!({
                                    "type": "tool_use",
                                    "id": current_tool_id,
                                    "name": current_tool_name,
                                    "input": input,
                                }));
                            }
                            current_block_type = None;
                        }
                        "message_delta" => {
                            if let Some(delta) = json.get("delta") {
                                if let Some(reason) =
                                    delta.get("stop_reason").and_then(|r| r.as_str())
                                {
                                    collected.stop_reason = Some(reason.to_string());
                                }
                            }
                            if let Some(usage) = json.get("usage") {
                                let output = usage
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                collected.usage.output_tokens += output;
                                collected.usage.total_tokens =
                                    collected.usage.input_tokens + collected.usage.output_tokens;
                            }
                        }
                        "message_start" => {
                            if let Some(message) = json.get("message") {
                                if let Some(usage) = message.get("usage") {
                                    let input = usage
                                        .get("input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    collected.usage.input_tokens = input;
                                    collected.usage.total_tokens =
                                        input + collected.usage.output_tokens;
                                }
                            }
                        }
                        "message_stop" => {
                            return Ok(collected);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(collected)
}

/// Parse a Google Gemini SSE stream.
pub async fn parse_google_sse_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamChunk>,
) -> Result<(), NxError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| NxError::StreamingError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // Extract text from candidates[0].content.parts[0].text
                    if let Some(candidates) = json.get("candidates").and_then(|c| c.as_array()) {
                        if let Some(candidate) = candidates.first() {
                            if let Some(content) = candidate.get("content") {
                                if let Some(parts) = content.get("parts").and_then(|p| p.as_array())
                                {
                                    for part in parts {
                                        if let Some(text) =
                                            part.get("text").and_then(|t| t.as_str())
                                        {
                                            let _ = tx.send(StreamChunk::Delta(text.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Extract usage metadata
                    if let Some(usage) = json.get("usageMetadata") {
                        let input = usage
                            .get("promptTokenCount")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let output = usage
                            .get("candidatesTokenCount")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if input > 0 || output > 0 {
                            let _ = tx.send(StreamChunk::Usage(TokenUsage {
                                input_tokens: input,
                                output_tokens: output,
                                total_tokens: input + output,
                            }));
                        }
                    }
                }
            }
        }
    }

    let _ = tx.send(StreamChunk::Done);
    Ok(())
}

/// Collect an OpenAI-compatible SSE stream, extracting text and tool calls.
/// Works for: OpenAI, Ollama, OpenRouter, Groq, DeepSeek.
///
/// Streams text deltas to `text_tx` in real-time.
/// Collects tool call blocks and assembles partial JSON arguments.
/// Returns a CollectedResponse with text + tool calls + usage.
pub async fn collect_openai_stream(
    response: reqwest::Response,
    text_tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<CollectedResponse, NxError> {
    let mut collected = CollectedResponse::default();

    // Track in-progress tool calls by index.
    // Each entry: (id, name, accumulated_arguments_json)
    let mut tool_calls_in_progress: std::collections::HashMap<u64, (String, String, String)> =
        std::collections::HashMap::new();

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(NxError::StreamingError(format!("Stream read error: {}", e)));
            }
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            // Skip empty lines and SSE comments
            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            // Handle data lines
            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();

                // Check for stream termination
                if data == "[DONE]" {
                    break;
                }

                // Parse JSON
                let json: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue, // Skip malformed JSON
                };

                // Extract from choices[0]
                if let Some(choice) = json
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                {
                    let delta = choice.get("delta").unwrap_or(&serde_json::Value::Null);

                    // Text content
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() {
                            let _ = text_tx.send(content.to_string());
                            collected.text.push_str(content);
                        }
                    }

                    // Tool calls
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for tc in tool_calls {
                            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0);

                            // First chunk for this tool call has id + function.name
                            if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                let name = tc
                                    .get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let initial_args = tc
                                    .get("function")
                                    .and_then(|f| f.get("arguments"))
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                tool_calls_in_progress
                                    .insert(index, (id.to_string(), name, initial_args));
                            } else if let Some(args_delta) = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                            {
                                // Subsequent chunk — append to arguments
                                if let Some(entry) = tool_calls_in_progress.get_mut(&index) {
                                    entry.2.push_str(args_delta);
                                }
                            }
                        }
                    }

                    // Finish reason
                    if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                        collected.stop_reason = Some(reason.to_string());
                    }
                }

                // Usage (may appear at top level with stream_options.include_usage)
                if let Some(usage) = json.get("usage") {
                    collected.usage.input_tokens = usage
                        .get("prompt_tokens")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);
                    collected.usage.output_tokens = usage
                        .get("completion_tokens")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);
                    collected.usage.total_tokens = usage
                        .get("total_tokens")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(collected.usage.input_tokens + collected.usage.output_tokens);
                }
            }
        }
    }

    // Assemble completed tool calls, sorted by index for deterministic order
    let mut sorted_calls: Vec<(u64, (String, String, String))> =
        tool_calls_in_progress.into_iter().collect();
    sorted_calls.sort_by_key(|(idx, _)| *idx);

    for (_, (id, name, arguments_json)) in sorted_calls {
        // Store in OpenAI format so parse_tool_calls_openai() can read them
        collected.tool_use_blocks.push(serde_json::json!({
            "id": id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments_json
            }
        }));
    }

    Ok(collected)
}
