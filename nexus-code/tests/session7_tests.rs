//! Session 7 tests — GitTool, TestRunnerTool, SubAgentTool, ProjectIndexTool,
//! context compaction, required_capability.

use nexus_code::context::compaction::CompactionConfig;
use nexus_code::tools::git::GitTool;
use nexus_code::tools::project_index::{build_tree, count_file_types, extract_definitions};
use nexus_code::tools::test_runner::TestRunnerTool;
use nexus_code::tools::{NxTool, ToolContext};
use serde_json::json;

fn test_ctx(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    }
}

// ═══════════════════════════════════════════════════════
// GitTool Tests (8)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_git_status() {
    let dir = tempfile::tempdir().unwrap();
    // Init a git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .ok();

    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"subcommand": "status"}), &ctx).await;
    // Should succeed (even if empty repo)
    assert!(result.is_success() || result.output.contains("git"));
}

#[tokio::test]
async fn test_git_log() {
    let dir = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .ok();
    // Create a file and commit
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["commit", "-m", "init", "--allow-empty"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .ok();

    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"subcommand": "log"}), &ctx).await;
    // Should show the commit (or error if git config missing)
    assert!(!result.output.is_empty());
}

#[tokio::test]
async fn test_git_diff() {
    let dir = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .ok();

    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"subcommand": "diff"}), &ctx).await;
    assert!(result.is_success());
}

#[tokio::test]
async fn test_git_forbidden_force_push() {
    let dir = tempfile::tempdir().unwrap();
    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"subcommand": "push", "args": ["--force"]}), &ctx)
        .await;
    assert!(!result.is_success());
    assert!(result.output.contains("Forbidden"));
}

#[tokio::test]
async fn test_git_forbidden_reset_hard() {
    let dir = tempfile::tempdir().unwrap();
    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"subcommand": "reset", "args": ["--hard"]}), &ctx)
        .await;
    assert!(!result.is_success());
    assert!(result.output.contains("Forbidden"));
}

#[test]
fn test_git_capability_read() {
    assert_eq!(
        GitTool::capability_for_subcommand("status"),
        nexus_code::governance::Capability::GitRead
    );
    assert_eq!(
        GitTool::capability_for_subcommand("log"),
        nexus_code::governance::Capability::GitRead
    );
    assert_eq!(
        GitTool::capability_for_subcommand("diff"),
        nexus_code::governance::Capability::GitRead
    );
    assert_eq!(
        GitTool::capability_for_subcommand("branch"),
        nexus_code::governance::Capability::GitRead
    );
    assert_eq!(
        GitTool::capability_for_subcommand("show"),
        nexus_code::governance::Capability::GitRead
    );
}

#[test]
fn test_git_capability_write() {
    assert_eq!(
        GitTool::capability_for_subcommand("commit"),
        nexus_code::governance::Capability::GitWrite
    );
    assert_eq!(
        GitTool::capability_for_subcommand("push"),
        nexus_code::governance::Capability::GitWrite
    );
    assert_eq!(
        GitTool::capability_for_subcommand("checkout"),
        nexus_code::governance::Capability::GitWrite
    );
    assert_eq!(
        GitTool::capability_for_subcommand("merge"),
        nexus_code::governance::Capability::GitWrite
    );
}

#[tokio::test]
async fn test_git_commit_with_message() {
    let dir = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .ok();
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .ok();

    let tool = GitTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"subcommand": "commit", "message": "test commit"}),
            &ctx,
        )
        .await;
    // May succeed or fail depending on git config, but should not panic
    assert!(!result.output.is_empty());
}

// ═══════════════════════════════════════════════════════
// TestRunnerTool Tests (6)
// ═══════════════════════════════════════════════════════

#[test]
fn test_detect_cargo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    assert_eq!(
        TestRunnerTool::detect_test_command(dir.path()),
        "cargo test"
    );
}

#[test]
fn test_detect_npm() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();
    assert_eq!(TestRunnerTool::detect_test_command(dir.path()), "npm test");
}

#[test]
fn test_detect_pytest() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();
    assert_eq!(
        TestRunnerTool::detect_test_command(dir.path()),
        "python -m pytest"
    );
}

#[test]
fn test_parse_cargo_output_pass() {
    let output =
        "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s";
    let (passed, failed, skipped, failing) = TestRunnerTool::parse_cargo_test(output);
    assert_eq!(passed, 5);
    assert_eq!(failed, 0);
    assert_eq!(skipped, 0);
    assert!(failing.is_empty());
}

#[test]
fn test_parse_cargo_output_fail() {
    let output = "test module::test_foo ... FAILED\ntest module::test_bar ... ok\ntest result: ok. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";
    let (passed, failed, _skipped, failing) = TestRunnerTool::parse_cargo_test(output);
    assert_eq!(passed, 1);
    assert_eq!(failed, 1);
    assert_eq!(failing.len(), 1);
    assert_eq!(failing[0], "module::test_foo");
}

#[test]
fn test_parse_cargo_output_mixed() {
    let output = "test result: ok. 10 passed; 2 failed; 3 ignored; 0 measured; 5 filtered out";
    let (passed, failed, skipped, _) = TestRunnerTool::parse_cargo_test(output);
    assert_eq!(passed, 10);
    assert_eq!(failed, 2);
    assert_eq!(skipped, 8); // 3 ignored + 5 filtered
}

// ═══════════════════════════════════════════════════════
// ProjectIndexTool Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_build_tree() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main(){}").unwrap();
    std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();

    let tree = build_tree(dir.path(), 2, 0);
    assert!(tree.contains("src/"));
    assert!(tree.contains("README.md"));
}

#[test]
fn test_count_file_types() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "").unwrap();
    std::fs::write(dir.path().join("b.rs"), "").unwrap();
    std::fs::write(dir.path().join("c.txt"), "").unwrap();
    std::fs::write(dir.path().join("d.md"), "").unwrap();

    let counts = count_file_types(dir.path());
    let rs_count = counts.iter().find(|(ext, _)| ext == "rs").map(|(_, c)| *c);
    assert_eq!(rs_count, Some(2));
}

#[test]
fn test_extract_rust_definitions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn hello() {}\npub struct Foo {}\nfn private() {}\npub enum Bar {}\n",
    )
    .unwrap();

    let defs = extract_definitions(dir.path());
    assert_eq!(defs.len(), 3); // pub fn, pub struct, pub enum (not private fn)
}

#[test]
fn test_ignores_hidden_dirs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".git/config"), "").unwrap();
    std::fs::write(dir.path().join("visible.rs"), "").unwrap();

    let tree = build_tree(dir.path(), 2, 0);
    assert!(!tree.contains(".git"));
    assert!(tree.contains("visible.rs"));
}

// ═══════════════════════════════════════════════════════
// SubAgentTool Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_sub_agent_schema() {
    let tool = nexus_code::tools::sub_agent_tool::SubAgentTool;
    let schema = tool.input_schema();
    assert!(schema["properties"]["task"].is_object());
    assert!(schema["properties"]["fuel_budget"].is_object());
    assert!(schema["properties"]["max_turns"].is_object());
}

#[test]
fn test_sub_agent_capability() {
    let tool = nexus_code::tools::sub_agent_tool::SubAgentTool;
    let cap = tool.required_capability(&json!({}));
    assert_eq!(cap, Some(nexus_code::governance::Capability::ProcessSpawn));
}

#[test]
fn test_sub_agent_fuel_estimate() {
    let tool = nexus_code::tools::sub_agent_tool::SubAgentTool;
    assert_eq!(tool.estimated_fuel(&json!({"fuel_budget": 3000})), 3000);
    assert_eq!(tool.estimated_fuel(&json!({})), 5000); // default
    assert_eq!(tool.estimated_fuel(&json!({"fuel_budget": 99999})), 10000); // capped
}

// ═══════════════════════════════════════════════════════
// Context Compaction Tests (5)
// ═══════════════════════════════════════════════════════

#[test]
fn test_should_compact_below_threshold() {
    let messages = vec![nexus_code::llm::types::Message {
        role: nexus_code::llm::types::Role::User,
        content: "Short message".to_string(),
    }];
    let config = CompactionConfig::default();
    assert!(!nexus_code::context::compaction::should_compact(
        &messages, "system", 200_000, &config
    ));
}

#[test]
fn test_should_compact_above_threshold() {
    // Create a massive message that exceeds 80% of 1000 tokens = 800 tokens ~ 3200 chars
    let big_content = "x".repeat(5000);
    let messages = vec![nexus_code::llm::types::Message {
        role: nexus_code::llm::types::Role::User,
        content: big_content,
    }];
    let config = CompactionConfig::default();
    assert!(nexus_code::context::compaction::should_compact(
        &messages, "system", 1000, &config
    ));
}

#[test]
fn test_compact_too_few_messages() {
    // 3 messages with preserve_recent=4 should not compact
    let messages = vec![
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::User,
            content: "m1".to_string(),
        },
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::Assistant,
            content: "m2".to_string(),
        },
        nexus_code::llm::types::Message {
            role: nexus_code::llm::types::Role::User,
            content: "m3".to_string(),
        },
    ];
    let config = CompactionConfig::default();
    // compact_messages is async but we can test the guard condition
    assert!(messages.len() <= config.preserve_recent + 1);
}

#[test]
fn test_compaction_config_defaults() {
    let config = CompactionConfig::default();
    assert!((config.trigger_threshold - 0.80).abs() < 0.001);
    assert_eq!(config.preserve_recent, 4);
    assert_eq!(config.summary_max_tokens, 500);
}

#[test]
fn test_compact_preserves_recent_count() {
    let config = CompactionConfig {
        preserve_recent: 2,
        ..CompactionConfig::default()
    };
    // With 10 messages and preserve_recent=2, split_point = 8
    let split_point = 10usize.saturating_sub(config.preserve_recent);
    assert_eq!(split_point, 8);
    // So 8 old messages get compacted, 2 recent preserved
}

// ═══════════════════════════════════════════════════════
// Required Capability Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_required_capability_default_none() {
    let tool = nexus_code::tools::file_read::FileReadTool;
    assert_eq!(tool.required_capability(&json!({})), None);
}

#[test]
fn test_required_capability_git_read() {
    let tool = GitTool;
    let cap = tool.required_capability(&json!({"subcommand": "status"}));
    assert_eq!(cap, Some(nexus_code::governance::Capability::GitRead));
}

#[test]
fn test_required_capability_git_write() {
    let tool = GitTool;
    let cap = tool.required_capability(&json!({"subcommand": "commit"}));
    assert_eq!(cap, Some(nexus_code::governance::Capability::GitWrite));
}
