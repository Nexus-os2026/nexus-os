//! /compact — Compact the conversation context via the agent loop.

/// Execute the /compact command.
/// Returns an AgentPrompt that asks the agent to summarize the conversation.
pub fn execute() -> super::CommandResult {
    super::CommandResult::AgentPrompt(
        "Summarize the conversation so far into a concise context summary. \
         Keep: current task, files modified, decisions made, errors encountered. \
         Discard: verbose tool outputs, intermediate reasoning, already-resolved issues. \
         Produce a brief summary paragraph that captures the essential state."
            .to_string(),
    )
}
