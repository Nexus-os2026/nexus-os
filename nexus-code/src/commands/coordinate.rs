//! /coordinate <task> — Run the governed multi-agent coordinator.

/// Execute the /coordinate command.
pub fn execute(args: &str) -> super::CommandResult {
    if args.is_empty() {
        return super::CommandResult::Error("Usage: /coordinate <task description>".to_string());
    }
    super::CommandResult::AgentPrompt(format!(
        "You are in COORDINATOR mode. Break this task into phases:\n\
         1. RESEARCH: investigate the codebase\n\
         2. PLAN: create a detailed implementation plan\n\
         3. IMPLEMENT: make the changes\n\
         4. VERIFY: run tests to confirm\n\n\
         Task: {}",
        args
    ))
}
