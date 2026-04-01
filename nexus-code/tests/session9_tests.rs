//! Session 9 tests — setup/doctor, benchmark comparison, distribution.

use nexus_code::bench::report::format_comparison;
use nexus_code::bench::{GovernanceMetrics, TaskResult};
use nexus_code::setup;

// ═══════════════════════════════════════════════════════
// Setup/Doctor Tests (5)
// ═══════════════════════════════════════════════════════

#[test]
fn test_diagnose_returns_status() {
    let status = setup::diagnose();
    // The status should have checked providers (some may be configured in CI)
    assert!(status.configured_providers.len() + status.unconfigured_providers.len() > 0);
}

#[test]
fn test_check_command_exists_echo() {
    // "echo" should exist on all Unix systems (it's a shell builtin,
    // but /usr/bin/echo exists on most systems)
    // Use "git" which is installed in this environment
    assert!(setup::check_command_exists("git"));
}

#[test]
fn test_check_command_nonexistent() {
    assert!(!setup::check_command_exists(
        "nonexistent_command_xyz_12345"
    ));
}

#[test]
fn test_init_creates_nexuscode() {
    let dir = tempfile::tempdir().unwrap();
    // Create a Cargo.toml to simulate Rust project
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    setup::init_nexuscode_md(dir.path()).unwrap();

    let path = dir.path().join("NEXUSCODE.md");
    assert!(path.exists());
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("language: rust"));
    assert!(content.contains("cargo test"));
    assert!(content.contains("cargo build"));
}

#[test]
fn test_init_fails_if_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("NEXUSCODE.md"), "existing").unwrap();

    let result = setup::init_nexuscode_md(dir.path());
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════
// Benchmark Comparison Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_governance_metrics_from_results() {
    let results = vec![
        TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 5,
            fuel_consumed: 1000,
            time_secs: 10.0,
            tools_used: vec!["file_read".to_string(), "search".to_string()],
            audit_entries: 20,
            error: None,
        },
        TaskResult {
            task_id: "t2".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 3,
            fuel_consumed: 2000,
            time_secs: 8.0,
            tools_used: vec!["file_read".to_string(), "bash".to_string()],
            audit_entries: 30,
            error: None,
        },
        TaskResult {
            task_id: "t3".to_string(),
            success: false,
            patch: String::new(),
            turns: 10,
            fuel_consumed: 3000,
            time_secs: 20.0,
            tools_used: vec!["file_read".to_string()],
            audit_entries: 10,
            error: Some("failed".to_string()),
        },
    ];

    let metrics = GovernanceMetrics::from_results(&results);
    assert!((metrics.avg_fuel_per_task - 2000.0).abs() < 0.1);
    assert!((metrics.avg_audit_entries_per_task - 20.0).abs() < 0.1);
    assert_eq!(metrics.total_fuel, 6000);
    assert_eq!(metrics.total_audit_entries, 60);
}

#[test]
fn test_governance_metrics_tool_distribution() {
    let results = vec![
        TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 5,
            fuel_consumed: 1000,
            time_secs: 10.0,
            tools_used: vec![
                "file_read".to_string(),
                "search".to_string(),
                "file_edit".to_string(),
            ],
            audit_entries: 20,
            error: None,
        },
        TaskResult {
            task_id: "t2".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 3,
            fuel_consumed: 800,
            time_secs: 8.0,
            tools_used: vec!["file_read".to_string(), "bash".to_string()],
            audit_entries: 15,
            error: None,
        },
    ];

    let metrics = GovernanceMetrics::from_results(&results);
    assert_eq!(metrics.tool_usage_distribution.get("file_read"), Some(&2));
    assert_eq!(metrics.tool_usage_distribution.get("search"), Some(&1));
    assert_eq!(metrics.tool_usage_distribution.get("bash"), Some(&1));
    assert_eq!(metrics.tool_usage_distribution.get("file_edit"), Some(&1));
}

#[test]
fn test_format_comparison_header() {
    let reports = vec![nexus_code::bench::BenchmarkReport {
        total_tasks: 5,
        passed: 3,
        failed: 2,
        errored: 0,
        pass_rate: 0.6,
        avg_turns: 5.0,
        avg_fuel: 1500.0,
        avg_time_secs: 12.0,
        total_time_secs: 60.0,
        provider: "anthropic".to_string(),
        model: "sonnet".to_string(),
        results: vec![],
    }];

    let output = format_comparison(&reports);
    assert!(output.contains("Provider/Model"));
    assert!(output.contains("Pass%"));
    assert!(output.contains("anthropic"));
}

#[test]
fn test_format_comparison_empty() {
    let output = format_comparison(&[]);
    assert!(output.contains("Comparison"));
}

// ═══════════════════════════════════════════════════════
// Distribution Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_init_detects_rust_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

    setup::init_nexuscode_md(dir.path()).unwrap();
    let content = std::fs::read_to_string(dir.path().join("NEXUSCODE.md")).unwrap();
    assert!(content.contains("language: rust"));
    assert!(content.contains("cargo build"));
    assert!(content.contains("cargo test"));
}

#[test]
fn test_init_detects_node_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();

    setup::init_nexuscode_md(dir.path()).unwrap();
    let content = std::fs::read_to_string(dir.path().join("NEXUSCODE.md")).unwrap();
    assert!(content.contains("language: javascript"));
    assert!(content.contains("npm test"));
}

#[test]
fn test_init_unknown_project() {
    let dir = tempfile::tempdir().unwrap();
    // Empty dir — no known project files

    setup::init_nexuscode_md(dir.path()).unwrap();
    let content = std::fs::read_to_string(dir.path().join("NEXUSCODE.md")).unwrap();
    assert!(content.contains("language: unknown"));
}
