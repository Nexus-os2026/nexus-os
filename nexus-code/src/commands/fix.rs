//! /fix [context] — Auto-fix the last error via the agent loop.

/// Execute the /fix command.
/// Returns an AgentPrompt so the agent investigates and fixes the issue.
pub fn execute(args: &str) -> super::CommandResult {
    let context = if args.is_empty() {
        "the last error in this conversation".to_string()
    } else {
        args.to_string()
    };

    super::CommandResult::AgentPrompt(format!(
        "Fix {}. Read the relevant files, identify the problem, and apply the fix. \
         After fixing, run the tests to verify the fix works.",
        context
    ))
}
