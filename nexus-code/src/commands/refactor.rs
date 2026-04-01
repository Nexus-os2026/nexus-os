//! /refactor <description> — Refactoring via the agent loop.

pub fn execute(args: &str) -> super::CommandResult {
    if args.is_empty() {
        return super::CommandResult::Error(
            "Usage: /refactor <description of what to refactor>".to_string(),
        );
    }
    super::CommandResult::AgentPrompt(format!(
        "Refactor: {}. Read the relevant files first, then make the changes. \
         After refactoring, verify the changes compile and tests pass.",
        args
    ))
}
