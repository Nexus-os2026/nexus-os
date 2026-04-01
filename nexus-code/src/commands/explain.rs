//! /explain <code or concept> — Ask the agent to explain.

/// Execute the /explain command.
/// Returns an AgentPrompt so the agent investigates and explains.
pub fn execute(args: &str) -> super::CommandResult {
    if args.is_empty() {
        return super::CommandResult::Error(
            "Usage: /explain <code, file path, or concept>".to_string(),
        );
    }
    super::CommandResult::AgentPrompt(format!(
        "Explain the following clearly and concisely. If it's a file path, read the file first. \
         If it's a concept, explain with examples.\n\n{}",
        args
    ))
}
