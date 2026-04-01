//! Session 4 tests — dual-agent, commands, context engine, streaming.

use nexus_code::agent::planner;
use nexus_code::context::measurement::{estimate_tokens, ContextMeasurement};
use nexus_code::context::nexuscode::NexusCodeMd;
use nexus_code::llm::streaming::CollectedResponse;
use nexus_code::tools::ToolRegistry;
use serde_json::json;

// ═══════════════════════════════════════════════════════
// Planner Tests (5)
// ═══════════════════════════════════════════════════════

#[test]
fn test_parse_plan_from_json() {
    let response = r#"{"summary": "Fix the bug", "steps": [{"step": 1, "description": "Read file", "tool": "file_read", "input": {"path": "src/main.rs"}}]}"#;
    let plan = planner::parse_plan(response).unwrap();
    assert_eq!(plan.summary, "Fix the bug");
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].step, 1);
    assert_eq!(plan.steps[0].tool, "file_read");
    assert_eq!(plan.steps[0].input["path"], "src/main.rs");
}

#[test]
fn test_parse_plan_from_markdown_block() {
    let response = r#"I'll create a plan.

```json
{
  "summary": "Add feature",
  "steps": [
    {"step": 1, "description": "Edit config", "tool": "file_edit", "input": {"path": "config.rs", "old_text": "a", "new_text": "b"}},
    {"step": 2, "description": "Run tests", "tool": "bash", "input": {"command": "cargo test"}}
  ]
}
```

This plan will add the feature."#;
    let plan = planner::parse_plan(response).unwrap();
    assert_eq!(plan.summary, "Add feature");
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].tool, "file_edit");
    assert_eq!(plan.steps[1].tool, "bash");
}

#[test]
fn test_parse_plan_invalid_json() {
    let response = "This is not JSON at all, just text.";
    let result = planner::parse_plan(response);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Failed to parse plan"));
}

#[test]
fn test_planner_tool_registry_read_only() {
    let registry = planner::planner_tool_registry();
    let tools = registry.list();

    // Should have read-only tools
    assert!(tools.contains(&"file_read"));
    assert!(tools.contains(&"search"));
    assert!(tools.contains(&"glob"));

    // Should NOT have write tools
    assert!(!tools.contains(&"file_write"));
    assert!(!tools.contains(&"file_edit"));
    assert!(!tools.contains(&"bash"));
}

#[test]
fn test_planner_system_prompt_includes_task() {
    let prompt = planner::planner_system_prompt("Base prompt.", "Fix the authentication bug");
    assert!(prompt.contains("PLANNER"));
    assert!(prompt.contains("Read-Only"));
    assert!(prompt.contains("Fix the authentication bug"));
    assert!(prompt.contains("CANNOT write"));
}

// ═══════════════════════════════════════════════════════
// Executor Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_step_result_success() {
    let result = nexus_code::agent::executor::StepResult {
        step: 1,
        success: true,
        output: "File read successfully".to_string(),
        duration_ms: 5,
    };
    assert!(result.success);
    assert_eq!(result.step, 1);
    assert_eq!(result.duration_ms, 5);
}

#[tokio::test]
async fn test_execute_plan_unknown_tool() {
    let plan = planner::Plan {
        summary: "Test plan".to_string(),
        steps: vec![planner::PlanStep {
            step: 1,
            description: "Do something".to_string(),
            tool: "nonexistent_tool".to_string(),
            input: json!({}),
        }],
    };

    let tool_ctx = nexus_code::tools::ToolContext {
        working_dir: std::env::temp_dir(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };

    let mut kernel = nexus_code::governance::GovernanceKernel::new(50_000).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let consent = |_: &nexus_code::governance::ConsentRequest| -> bool { false };

    let results =
        nexus_code::agent::executor::execute_plan(&plan, &tool_ctx, &mut kernel, &tx, &consent)
            .await
            .unwrap();

    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(results[0].output.contains("Unknown tool"));
}

#[tokio::test]
async fn test_execute_plan_records_audit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

    let plan = planner::Plan {
        summary: "Read a file".to_string(),
        steps: vec![planner::PlanStep {
            step: 1,
            description: "Read test.txt".to_string(),
            tool: "file_read".to_string(),
            input: json!({"path": "test.txt"}),
        }],
    };

    let tool_ctx = nexus_code::tools::ToolContext {
        working_dir: dir.path().to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };

    let mut kernel = nexus_code::governance::GovernanceKernel::new(50_000).unwrap();
    let initial_entries = kernel.audit.len();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let consent = |_: &nexus_code::governance::ConsentRequest| -> bool { true };

    let results =
        nexus_code::agent::executor::execute_plan(&plan, &tool_ctx, &mut kernel, &tx, &consent)
            .await
            .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert!(kernel.audit.len() > initial_entries);
}

// ═══════════════════════════════════════════════════════
// Command Tests (5)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_command_dispatch_known() {
    // /diff is a known command
    let mut app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let result = nexus_code::commands::execute_command("/diff", &mut app).await;
    assert!(result.is_some());
}

#[tokio::test]
async fn test_command_dispatch_unknown() {
    let mut app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let result = nexus_code::commands::execute_command("/nonexistent", &mut app).await;
    assert!(result.is_none());
}

#[test]
fn test_fix_produces_agent_prompt() {
    let result = nexus_code::commands::fix::execute("the compile error");
    match result {
        nexus_code::commands::CommandResult::AgentPrompt(prompt) => {
            assert!(prompt.contains("the compile error"));
            assert!(prompt.contains("Fix"));
        }
        _ => panic!("Expected AgentPrompt"),
    }
}

#[test]
fn test_explain_produces_agent_prompt() {
    let result = nexus_code::commands::explain::execute("What is the governance kernel?");
    match result {
        nexus_code::commands::CommandResult::AgentPrompt(prompt) => {
            assert!(prompt.contains("What is the governance kernel?"));
            assert!(prompt.contains("Explain"));
        }
        _ => panic!("Expected AgentPrompt"),
    }
}

#[test]
fn test_session_list_empty() {
    // Use a temp dir that won't have sessions
    let result = nexus_code::commands::session::execute("list");
    // Either "No saved sessions" or lists existing ones — both are valid Output
    match result {
        nexus_code::commands::CommandResult::Output(msg) => {
            assert!(msg.contains("sessions") || msg.contains("Sessions"));
        }
        _ => panic!("Expected Output"),
    }
}

// ═══════════════════════════════════════════════════════
// NEXUSCODE.md Tests (6)
// ═══════════════════════════════════════════════════════

#[test]
fn test_parse_nexuscode_project() {
    let content = r#"
## Project

name: my-project
language: rust
build: cargo build
test: cargo test
lint: cargo clippy
"#;
    let config = NexusCodeMd::parse(content);
    assert_eq!(config.project_name.as_deref(), Some("my-project"));
    assert_eq!(config.language.as_deref(), Some("rust"));
    assert_eq!(config.build_command.as_deref(), Some("cargo build"));
    assert_eq!(config.test_command.as_deref(), Some("cargo test"));
    assert_eq!(config.lint_command.as_deref(), Some("cargo clippy"));
}

#[test]
fn test_parse_nexuscode_governance() {
    let content = r#"
## Governance

fuel_budget: 100000
max_file_scope: src/**
hitl_tier: 2
blocked_paths: .env, secrets/
"#;
    let config = NexusCodeMd::parse(content);
    assert_eq!(config.fuel_budget, Some(100000));
    assert_eq!(config.max_file_scope.as_deref(), Some("src/**"));
    assert_eq!(config.hitl_tier, Some(2));
    assert_eq!(config.blocked_paths, vec![".env", "secrets/"]);
}

#[test]
fn test_parse_nexuscode_models() {
    let content = r#"
## Models

execution: claude-sonnet-4
thinking: claude-opus-4
critique: gpt-4o
compact: claude-haiku-4
vision: gemini-2.5-pro
"#;
    let config = NexusCodeMd::parse(content);
    assert_eq!(config.execution_model.as_deref(), Some("claude-sonnet-4"));
    assert_eq!(config.thinking_model.as_deref(), Some("claude-opus-4"));
    assert_eq!(config.critique_model.as_deref(), Some("gpt-4o"));
    assert_eq!(config.compact_model.as_deref(), Some("claude-haiku-4"));
    assert_eq!(config.vision_model.as_deref(), Some("gemini-2.5-pro"));
}

#[test]
fn test_parse_nexuscode_style() {
    let content = r#"
## Style

prefer_short_responses: true
show_diffs_inline: true
auto_run_tests_after_edit: false
"#;
    let config = NexusCodeMd::parse(content);
    assert!(config.prefer_short_responses);
    assert!(config.show_diffs_inline);
    assert!(!config.auto_run_tests_after_edit);
}

#[test]
fn test_parse_nexuscode_empty() {
    let config = NexusCodeMd::parse("");
    assert!(config.project_name.is_none());
    assert!(config.fuel_budget.is_none());
    assert!(config.blocked_paths.is_empty());
    assert!(!config.prefer_short_responses);
}

#[test]
fn test_parse_nexuscode_full_example() {
    let content = r#"# NEXUSCODE.md

## Project

name: nexus-os
language: rust
build: cargo build
test: cargo test
lint: cargo clippy

## Governance

fuel_budget: 75000
max_file_scope: crates/**
hitl_tier: 2

## Models

execution: claude-sonnet-4-20250514
thinking: claude-opus-4-20250514

## Memory

persist_across_sessions: true
max_memory_entries: 100

## Style

prefer_short_responses: true
show_diffs_inline: false
auto_run_tests_after_edit: true
"#;
    let config = NexusCodeMd::parse(content);
    assert_eq!(config.project_name.as_deref(), Some("nexus-os"));
    assert_eq!(config.language.as_deref(), Some("rust"));
    assert_eq!(config.fuel_budget, Some(75000));
    assert_eq!(config.max_file_scope.as_deref(), Some("crates/**"));
    assert_eq!(
        config.execution_model.as_deref(),
        Some("claude-sonnet-4-20250514")
    );
    assert!(config.persist_across_sessions);
    assert_eq!(config.max_memory_entries, Some(100));
    assert!(config.prefer_short_responses);
    assert!(config.auto_run_tests_after_edit);
}

// ═══════════════════════════════════════════════════════
// Context Measurement Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_estimate_tokens() {
    // "hello world" = 11 chars -> ceil(11/4) = 3 tokens
    assert_eq!(estimate_tokens("hello world"), 3);
    // Empty string -> 0 tokens
    assert_eq!(estimate_tokens(""), 0);
    // 100 chars -> 25 tokens
    let text = "a".repeat(100);
    assert_eq!(estimate_tokens(&text), 25);
}

#[test]
fn test_context_measurement() {
    let messages = vec![
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::User,
            content: "Hello, how are you?".to_string(),
        },
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::Assistant,
            content: "I'm doing well! How can I help?".to_string(),
        },
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::User,
            content: "Read this file".to_string(),
        },
    ];
    let measurement = ContextMeasurement::measure(&messages, "You are a coding agent.");
    assert_eq!(measurement.message_count, 3);
    assert!(measurement.total_tokens > 0);
    assert!(measurement.system_prompt_tokens > 0);
}

#[test]
fn test_usage_percentage() {
    let measurement = ContextMeasurement {
        total_tokens: 50_000,
        message_count: 10,
        tool_result_count: 2,
        system_prompt_tokens: 500,
    };
    let pct = measurement.usage_percentage(200_000);
    assert!((pct - 25.0).abs() < 0.1);

    let summary = measurement.summary(200_000);
    assert!(summary.contains("50000"));
    assert!(summary.contains("25.0%"));
    assert!(summary.contains("200K"));
}

// ═══════════════════════════════════════════════════════
// Streaming Tests (2)
// ═══════════════════════════════════════════════════════

#[test]
fn test_collected_response_default() {
    let response = CollectedResponse::default();
    assert!(response.text.is_empty());
    assert!(response.tool_use_blocks.is_empty());
    assert!(response.stop_reason.is_none());
    assert_eq!(response.usage.total_tokens, 0);
}

#[test]
fn test_collected_response_with_tools() {
    let mut response = CollectedResponse {
        text: "I'll read that file.".to_string(),
        stop_reason: Some("tool_use".to_string()),
        ..Default::default()
    };
    response.tool_use_blocks.push(json!({
        "type": "tool_use",
        "id": "toolu_123",
        "name": "file_read",
        "input": {"path": "src/main.rs"}
    }));

    assert_eq!(response.tool_use_blocks.len(), 1);
    assert_eq!(response.tool_use_blocks[0]["name"], "file_read");
    assert_eq!(response.stop_reason.as_deref(), Some("tool_use"));
}

// ═══════════════════════════════════════════════════════
// ToolRegistry::with_tools Test (1)
// ═══════════════════════════════════════════════════════

#[test]
fn test_tool_registry_with_tools() {
    let registry = ToolRegistry::with_tools(vec![
        Box::new(nexus_code::tools::file_read::FileReadTool),
        Box::new(nexus_code::tools::search::SearchTool),
    ]);
    let tools = registry.list();
    assert_eq!(tools.len(), 2);
    assert!(tools.contains(&"file_read"));
    assert!(tools.contains(&"search"));
    assert!(!tools.contains(&"bash"));
}
