//! /review [target] — Code review via the agent loop.

pub fn execute(args: &str) -> super::CommandResult {
    let target = if args.is_empty() {
        "the recent changes"
    } else {
        args
    };
    super::CommandResult::AgentPrompt(format!(
        "Review {}. Read the relevant files, analyze the code quality, and provide feedback on:\n\
         1. Correctness — any bugs or logic errors\n\
         2. Security — any vulnerabilities\n\
         3. Performance — any obvious inefficiencies\n\
         4. Style — readability and maintainability\n\
         Be specific with file names and line numbers.",
        target
    ))
}
