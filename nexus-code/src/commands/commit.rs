//! /commit <message> — Stage all changes, create a governed git commit.

/// Execute the /commit command.
/// Runs `git add -A && git commit -m "message"` through governance.
pub async fn execute(args: &str, app: &mut crate::app::App) -> super::CommandResult {
    if args.is_empty() {
        return super::CommandResult::Error("Usage: /commit <message>".to_string());
    }

    // Escape single quotes in the message
    let safe_msg = args.replace('\'', "'\\''");
    let command = format!("git add -A && git commit -m '{}'", safe_msg);

    let tool = match crate::tools::create_tool("bash") {
        Some(t) => t,
        None => return super::CommandResult::Error("bash tool unavailable".to_string()),
    };

    let input = serde_json::json!({"command": command});
    let tool_ctx = crate::tools::ToolContext {
        working_dir: app
            .config
            .project_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
        blocked_paths: app.config.blocked_paths.clone(),
        max_file_scope: app.config.max_file_scope.clone(),
        non_interactive: false,
    };

    match crate::tools::execute_governed(tool.as_ref(), input, &tool_ctx, &mut app.governance).await
    {
        Ok(result) => {
            if result.is_success() {
                super::CommandResult::Output(format!("Committed: {}\n{}", args, result.output))
            } else {
                super::CommandResult::Error(result.output)
            }
        }
        Err(crate::error::NxError::ConsentRequired { .. }) => {
            // Bash is Tier3 — consent needed
            super::CommandResult::AgentPrompt(format!(
                "Create a git commit with message: '{}'. \
                 First run `git add -A`, then `git commit -m '{}'`. \
                 Report the result.",
                args, safe_msg
            ))
        }
        Err(e) => super::CommandResult::Error(format!("{}", e)),
    }
}
