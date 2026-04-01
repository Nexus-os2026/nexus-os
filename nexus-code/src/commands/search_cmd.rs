//! /search <pattern> — Search the codebase for a pattern.

/// Execute the /search command using the search tool through governance.
pub async fn execute(args: &str, app: &mut crate::app::App) -> super::CommandResult {
    if args.is_empty() {
        return super::CommandResult::Error("Usage: /search <pattern>".to_string());
    }

    let tool = match crate::tools::create_tool("search") {
        Some(t) => t,
        None => return super::CommandResult::Error("Search tool not available".to_string()),
    };

    let input = serde_json::json!({"pattern": args});
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
        Ok(result) => super::CommandResult::Output(result.output),
        Err(e) => super::CommandResult::Error(format!("{}", e)),
    }
}
