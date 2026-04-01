//! Context window measurement — estimate token count of the conversation.

/// Measure and report context window usage.
#[derive(Debug, Clone)]
pub struct ContextMeasurement {
    /// Estimated total tokens in current conversation.
    pub total_tokens: u64,
    /// Number of messages.
    pub message_count: usize,
    /// Number of tool result messages (typically large).
    pub tool_result_count: usize,
    /// Estimated system prompt tokens.
    pub system_prompt_tokens: u64,
}

impl ContextMeasurement {
    /// Estimate token count for a conversation.
    /// Uses a rough heuristic: 1 token ~ 4 characters.
    pub fn measure(messages: &[crate::llm::types::Message], system_prompt: &str) -> Self {
        let system_prompt_tokens = estimate_tokens(system_prompt);
        let mut total_tokens = system_prompt_tokens;
        let mut tool_result_count = 0;

        for msg in messages {
            total_tokens += estimate_tokens(&msg.content);
            if msg.content.contains("[Tool:") || msg.content.len() > 1000 {
                tool_result_count += 1;
            }
        }

        Self {
            total_tokens,
            message_count: messages.len(),
            tool_result_count,
            system_prompt_tokens,
        }
    }

    /// Get usage as a percentage of a context window size.
    pub fn usage_percentage(&self, max_tokens: u64) -> f64 {
        if max_tokens == 0 {
            return 100.0;
        }
        (self.total_tokens as f64 / max_tokens as f64) * 100.0
    }

    /// Get a human-readable summary.
    pub fn summary(&self, max_tokens: u64) -> String {
        format!(
            "Context: ~{} tokens ({:.1}% of {}K) | {} messages ({} tool results) | System: ~{} tokens",
            self.total_tokens,
            self.usage_percentage(max_tokens),
            max_tokens / 1000,
            self.message_count,
            self.tool_result_count,
            self.system_prompt_tokens,
        )
    }
}

/// Estimate token count from text (rough: 1 token ~ 4 chars).
pub fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64).div_ceil(4)
}
