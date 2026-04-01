//! Slash commands — /commit, /diff, /test, /fix, /compact, /search, /session, /explain,
//! /review, /refactor, /memory, /rollback.

pub mod commit;
pub mod compact;
pub mod coordinate;
pub mod diff;
pub mod explain;
pub mod fix;
pub mod memory_cmd;
pub mod refactor;
pub mod review;
pub mod rollback;
pub mod search_cmd;
pub mod session;
pub mod test_cmd;

/// Result of executing a slash command.
pub enum CommandResult {
    /// Display this message to the user.
    Output(String),
    /// An error occurred.
    Error(String),
    /// Command requires the agent loop (e.g., /fix, /explain).
    /// Contains the prompt to feed to the agent loop.
    AgentPrompt(String),
    /// Silently handled (no output needed).
    Silent,
}

/// Execute a slash command.
/// Returns None if the command is not recognized (fall through to existing handler).
pub async fn execute_command(input: &str, app: &mut crate::app::App) -> Option<CommandResult> {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0];
    let args = parts.get(1).copied().unwrap_or("");

    match cmd {
        "/commit" => Some(commit::execute(args, app).await),
        "/diff" => Some(diff::execute(args)),
        "/test" => Some(test_cmd::execute(args)),
        "/fix" => Some(fix::execute(args)),
        "/compact" => Some(compact::execute()),
        "/search" => Some(search_cmd::execute(args, app).await),
        "/session" => Some(session::execute(args)),
        "/explain" => Some(explain::execute(args)),
        "/review" => Some(review::execute(args)),
        "/refactor" => Some(refactor::execute(args)),
        "/memory" => Some(memory_cmd::execute(args)),
        "/rollback" => Some(rollback::execute(args)),
        "/coordinate" => Some(coordinate::execute(args)),
        _ => None,
    }
}
