//! Automatic context compaction — triggers when context exceeds threshold.

use crate::llm::types::{Message, Role};

/// Configuration for automatic context compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Trigger compaction when context exceeds this percentage of max window.
    pub trigger_threshold: f64,
    /// Keep the last N messages uncompacted (recent context).
    pub preserve_recent: usize,
    /// Maximum tokens for the compacted summary.
    pub summary_max_tokens: u32,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            trigger_threshold: 0.80,
            preserve_recent: 4,
            summary_max_tokens: 500,
        }
    }
}

/// Check if compaction is needed.
pub fn should_compact(
    messages: &[Message],
    system_prompt: &str,
    max_context_tokens: u64,
    config: &CompactionConfig,
) -> bool {
    let measurement = super::measurement::ContextMeasurement::measure(messages, system_prompt);
    let threshold = (max_context_tokens as f64 * config.trigger_threshold) as u64;
    measurement.total_tokens > threshold
}

/// Compact messages by summarizing older ones into a context summary.
/// Uses the LLM to generate a summary of older messages, preserving recent ones.
pub async fn compact_messages(
    messages: &[Message],
    router: &crate::llm::router::ModelRouter,
    governance: &mut crate::governance::GovernanceKernel,
    config: &CompactionConfig,
) -> Result<Vec<Message>, crate::error::NxError> {
    if messages.len() <= config.preserve_recent + 1 {
        return Ok(messages.to_vec());
    }

    let split_point = messages.len().saturating_sub(config.preserve_recent);
    let old_messages = &messages[..split_point];
    let recent_messages = &messages[split_point..];

    // Build context text from old messages
    let mut context_text = String::new();
    for msg in old_messages {
        let role = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };
        let preview = if msg.content.len() > 500 {
            format!("{}...", &msg.content[..500])
        } else {
            msg.content.clone()
        };
        context_text.push_str(&format!("[{}]: {}\n", role, preview));
    }

    let summary_request = crate::llm::types::LlmRequest {
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Summarize this conversation context into a brief paragraph. \
                 Preserve: current task, files modified, decisions made, errors. \
                 Discard: verbose tool outputs, intermediate reasoning.\n\n\
                 CONVERSATION:\n{}\n\nSUMMARY (one paragraph, max 200 words):",
                context_text
            ),
        }],
        model: String::new(),
        max_tokens: config.summary_max_tokens,
        temperature: Some(0.3),
        stream: false,
        system: Some(
            "You are a context summarizer. Produce concise, factual summaries.".to_string(),
        ),
        tools: None,
    };

    // Try Compact slot, fall back to Execution
    let slot = crate::llm::router::ModelSlot::Compact;
    let response = match router.complete(slot, &summary_request).await {
        Ok(r) => r,
        Err(_) => {
            router
                .complete(crate::llm::router::ModelSlot::Execution, &summary_request)
                .await?
        }
    };

    governance.record_fuel(
        "compaction",
        crate::governance::FuelCost {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            fuel_units: response.usage.total_tokens,
            estimated_usd: 0.0,
        },
    );

    governance
        .audit
        .record(crate::governance::AuditAction::ToolInvocation {
            tool: "context_compaction".to_string(),
            args_summary: format!("Compacted {} messages into summary", old_messages.len()),
        });

    let mut compacted = vec![Message {
        role: Role::System,
        content: format!(
            "[Context Summary \u{2014} {} messages compacted]\n\n{}",
            old_messages.len(),
            response.content
        ),
    }];
    compacted.extend_from_slice(recent_messages);

    Ok(compacted)
}
