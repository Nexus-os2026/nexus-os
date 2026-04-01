//! /test [args] — Run project tests via the agent loop.

/// Execute the /test command.
/// Returns an AgentPrompt so the agent runs tests and reports results.
pub fn execute(args: &str) -> super::CommandResult {
    let base_cmd = "cargo test";
    let command = if args.is_empty() {
        base_cmd.to_string()
    } else {
        format!("{} {}", base_cmd, args)
    };

    super::CommandResult::AgentPrompt(format!(
        "Run the following test command and report the results:\n```\n{}\n```\n\
         If tests fail, show the failing test names and error messages.",
        command
    ))
}
