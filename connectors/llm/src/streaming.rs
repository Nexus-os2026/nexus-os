//! Streaming LLM support: SSE-based token streaming for real-time progress.
//!
//! Providers that support streaming implement [`StreamingLlmProvider`] which
//! returns a [`StreamingResponse`] — an iterator of [`StreamChunk`] values
//! produced by parsing Server-Sent Events from the provider's HTTP response.

use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// A single chunk of streamed LLM output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamChunk {
    /// The text content of this chunk.
    pub text: String,
    /// Estimated token count for this chunk (1 if unknown).
    pub token_count: Option<usize>,
}

/// Final usage statistics from a completed stream.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

/// Shared cell for passing usage from the SSE iterator back to the caller.
pub type UsageCell = Arc<Mutex<Option<StreamUsage>>>;

/// Create a new shared usage cell.
pub fn new_usage_cell() -> UsageCell {
    Arc::new(Mutex::new(None))
}

/// An iterator-based streaming response from an LLM provider.
///
/// Call `next()` to get chunks until `None`. After the stream ends,
/// call `usage()` for final token counts.
pub struct StreamingResponse {
    /// The underlying reader that produces chunks from SSE lines.
    reader: Box<dyn Iterator<Item = Result<StreamChunk, AgentError>> + Send>,
    /// Shared usage cell — the iterator writes to this, caller reads after.
    usage_cell: UsageCell,
}

impl StreamingResponse {
    /// Create a streaming response with a shared usage cell.
    ///
    /// The iterator implementation should write usage into the same `UsageCell`
    /// when it encounters a usage event (e.g., `message_delta` for Anthropic).
    pub fn new(
        reader: Box<dyn Iterator<Item = Result<StreamChunk, AgentError>> + Send>,
        usage_cell: UsageCell,
    ) -> Self {
        Self { reader, usage_cell }
    }

    /// Get the final usage statistics (available after stream ends).
    pub fn usage(&self) -> StreamUsage {
        self.usage_cell
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl Iterator for StreamingResponse {
    type Item = Result<StreamChunk, AgentError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next()
    }
}

/// Trait for LLM providers that support streaming responses.
///
/// This is separate from [`LlmProvider`] because not all providers support
/// streaming. The conductor checks for this trait before attempting to stream.
pub trait StreamingLlmProvider: Send + Sync {
    /// Start a streaming completion request.
    ///
    /// Returns a [`StreamingResponse`] that yields [`StreamChunk`] values
    /// as the model generates tokens. The caller should consume all chunks,
    /// then call `usage()` on the response for final token counts.
    fn stream_query(
        &self,
        prompt: &str,
        system_prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<StreamingResponse, AgentError>;

    /// Provider name (for logging/display).
    fn streaming_provider_name(&self) -> &str;
}
