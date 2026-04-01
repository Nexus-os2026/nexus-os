//! Agent loop — the runtime that connects the LLM to the governed tool system.
//!
//! When the user asks "read main.rs and fix the bug", the LLM reasons about it,
//! decides to call `file_read`, the tool executes through the governance pipeline,
//! the result feeds back to the LLM, and the LLM continues until the task is done.

pub mod envelope;
pub mod executor;
pub mod loop_runtime;
pub mod planner;
pub mod sub_agent;
pub mod tool_protocol;

pub use loop_runtime::{run_agent_loop, AgentConfig};
pub use tool_protocol::{ToolCall, ToolDefinition, ToolProtocol, ToolResultMessage};

/// Events emitted by the agent loop for the UI to consume.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A text delta from the LLM (stream to terminal).
    TextDelta(String),
    /// The LLM is requesting a tool call.
    ToolCallStart { name: String, id: String },
    /// Tool execution completed.
    ToolCallComplete {
        name: String,
        success: bool,
        duration_ms: u64,
        summary: String,
    },
    /// Tool was denied by governance (consent denied or capability denied).
    ToolCallDenied { name: String, reason: String },
    /// A turn completed (may or may not have more turns).
    TurnComplete { turn: u32, has_more: bool },
    /// Token usage for this turn.
    TokenUsage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// Agent loop finished (all turns done or stopped).
    Done { reason: String, total_turns: u32 },
    /// Error during agent loop.
    Error(String),
}

/// Build the complete system prompt including tool descriptions.
pub fn build_system_prompt(
    base_prompt: &str,
    tool_registry: &crate::tools::ToolRegistry,
) -> String {
    let tool_descriptions = tool_registry.build_tool_prompt();
    format!(
        "{}\n\n## Available Tools\n\nYou have access to the following tools. \
         To use a tool, respond with a tool_use block.\n\n{}\n\n\
         ## Tool Usage Rules\n\n\
         - Always explain what you're about to do before calling a tool.\n\
         - If a tool call is denied, explain what happened and ask the user how to proceed.\n\
         - Use file_read before file_edit to understand the current content.\n\
         - Use search and glob to find relevant files before editing.\n\
         - Use bash for commands like running tests, building, or checking status.\n\
         - Be concise in tool inputs — don't include unnecessary content.",
        base_prompt, tool_descriptions
    )
}
