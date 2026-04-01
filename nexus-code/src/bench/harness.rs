//! Task execution harness — runs nx against a single SWE-bench task.

use super::swe_bench::SweBenchTask;
use super::TaskResult;

/// Run nx against a single SWE-bench task.
pub async fn run_task(
    task: &SweBenchTask,
    repo_dir: &std::path::Path,
    provider: &str,
    model: &str,
    fuel_budget: u64,
    max_turns: u32,
) -> TaskResult {
    let start = std::time::Instant::now();

    let prompt = format!(
        "You are fixing a bug in the {} repository.\n\n\
         ## Issue\n\n{}\n\n\
         ## Instructions\n\n\
         1. Read the relevant files to understand the codebase\n\
         2. Identify the root cause of the bug\n\
         3. Make the minimal code change to fix it\n\
         4. Do NOT add tests — only fix the existing code\n",
        task.repo, task.problem_statement
    );

    let config = crate::config::NxConfig {
        fuel_budget,
        default_provider: provider.to_string(),
        default_model: model.to_string(),
        ..Default::default()
    };

    let mut app = match crate::app::App::new(config) {
        Ok(a) => a,
        Err(e) => {
            return TaskResult {
                task_id: task.instance_id.clone(),
                success: false,
                patch: String::new(),
                turns: 0,
                fuel_consumed: 0,
                time_secs: start.elapsed().as_secs_f64(),
                tools_used: Vec::new(),
                audit_entries: 0,
                error: Some(format!("Failed to create app: {}", e)),
            };
        }
    };

    let agent_config = crate::agent::AgentConfig {
        max_turns,
        system_prompt: "You are a software engineer fixing a bug. Be precise.".to_string(),
        model_slot: crate::llm::router::ModelSlot::Execution,
        auto_approve_tier2: true,
        auto_approve_tier3: false,
    };

    let tool_ctx = crate::tools::ToolContext {
        working_dir: repo_dir.to_path_buf(),
        blocked_paths: Vec::new(),
        max_file_scope: None,
        non_interactive: true,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    let consent_handler: std::sync::Arc<
        dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync,
    > = std::sync::Arc::new(|request| {
        matches!(
            request.tier,
            crate::governance::ConsentTier::Tier1 | crate::governance::ConsentTier::Tier2
        )
    });

    let cancel = tokio_util::sync::CancellationToken::new();
    let mut messages = vec![crate::llm::types::Message {
        role: crate::llm::types::Role::User,
        content: prompt,
    }];

    let result = crate::agent::run_agent_loop(
        &mut messages,
        &app.router,
        &app.tool_registry,
        &tool_ctx,
        &mut app.governance,
        &agent_config,
        event_tx,
        consent_handler,
        cancel,
    )
    .await;

    // Collect tool usage from events
    let mut tools_used = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        if let crate::agent::AgentEvent::ToolCallComplete { name, .. } = event {
            if !tools_used.contains(&name) {
                tools_used.push(name);
            }
        }
    }

    let time_secs = start.elapsed().as_secs_f64();
    let patch = super::swe_bench::extract_patch(repo_dir)
        .await
        .unwrap_or_default();
    let success = !patch.is_empty() && result.is_ok();
    let error = result.err().map(|e| format!("{}", e));

    TaskResult {
        task_id: task.instance_id.clone(),
        success,
        patch,
        turns: max_turns,
        fuel_consumed: app.governance.fuel.budget().consumed,
        time_secs,
        tools_used,
        audit_entries: app.governance.audit.len(),
        error,
    }
}
