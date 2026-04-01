//! Session 8 tests — MCP JSON-RPC, SWE-bench parser, benchmark reports.

use nexus_code::bench::report::{format_report, generate_report, save_report};
use nexus_code::bench::{BenchmarkReport, TaskResult};
use nexus_code::mcp::jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use nexus_code::mcp::{McpServerConfig, McpToolInfo, McpTransport};
use serde_json::json;

// ═══════════════════════════════════════════════════════
// JSON-RPC Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_jsonrpc_request_serialization() {
    let req = JsonRpcRequest::new(1, "initialize", Some(json!({"key": "value"})));
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);
    assert_eq!(json["method"], "initialize");
    assert_eq!(json["params"]["key"], "value");
}

#[test]
fn test_jsonrpc_response_success() {
    let resp: JsonRpcResponse =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#).unwrap();
    assert!(!resp.is_error());
    assert!(resp.result.is_some());
    assert!(resp.result_or_error().is_ok());
}

#[test]
fn test_jsonrpc_response_error() {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: None,
        error: Some(JsonRpcError {
            code: -32600,
            message: "Invalid request".to_string(),
            data: None,
        }),
    };
    assert!(resp.is_error());
    let err = resp.result_or_error();
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("-32600"));
}

#[test]
fn test_jsonrpc_result_or_error() {
    // Success path
    let ok_resp = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: Some(json!({"data": "hello"})),
        error: None,
    };
    assert_eq!(ok_resp.result_or_error().unwrap()["data"], "hello");

    // Error path
    let err_resp = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: None,
        error: Some(JsonRpcError {
            code: -1,
            message: "fail".to_string(),
            data: None,
        }),
    };
    assert!(err_resp.result_or_error().is_err());

    // Empty path
    let empty_resp = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: None,
        error: None,
    };
    assert!(empty_resp.result_or_error().is_err());
}

// ═══════════════════════════════════════════════════════
// MCP Protocol Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_mcp_tool_info_fields() {
    let info = McpToolInfo {
        server_name: "test-server".to_string(),
        tool_name: "mcp_test_read_file".to_string(),
        description: "Read a file".to_string(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    };
    assert_eq!(info.server_name, "test-server");
    assert!(info.tool_name.starts_with("mcp_"));
    assert!(info.input_schema.is_object());
}

#[test]
fn test_mcp_tool_name_format() {
    // Tool names from MCP follow "mcp_{server}_{tool}" pattern
    let name = format!("mcp_{}_{}", "myserver", "read_file");
    assert_eq!(name, "mcp_myserver_read_file");
    assert!(name.starts_with("mcp_"));
}

#[test]
fn test_mcp_server_config_stdio() {
    let config = McpServerConfig {
        name: "filesystem".to_string(),
        transport: McpTransport::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
        },
        capability_scope: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: McpServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "filesystem");
    assert!(matches!(parsed.transport, McpTransport::Stdio { .. }));
}

#[test]
fn test_mcp_server_config_sse() {
    let config = McpServerConfig {
        name: "remote".to_string(),
        transport: McpTransport::Sse {
            url: "http://localhost:3000".to_string(),
        },
        capability_scope: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: McpServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "remote");
    assert!(matches!(parsed.transport, McpTransport::Sse { .. }));
}

// ═══════════════════════════════════════════════════════
// SWE-bench Parser Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_load_swe_bench_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.jsonl");
    std::fs::write(
        &path,
        r#"{"instance_id":"django__django-12345","repo":"django/django","base_commit":"abc123","problem_statement":"Bug in queryset"}"#,
    )
    .unwrap();

    let tasks = nexus_code::bench::swe_bench::load_tasks(&path).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].instance_id, "django__django-12345");
    assert_eq!(tasks[0].repo, "django/django");
}

#[test]
fn test_swe_bench_task_fields() {
    let task: nexus_code::bench::swe_bench::SweBenchTask = serde_json::from_str(
        r#"{"instance_id":"test-1","repo":"owner/repo","base_commit":"def456","problem_statement":"Fix the bug","hints_text":"Look at utils.py"}"#,
    )
    .unwrap();
    assert_eq!(task.instance_id, "test-1");
    assert_eq!(task.problem_statement, "Fix the bug");
    assert_eq!(task.hints_text.as_deref(), Some("Look at utils.py"));
}

#[test]
fn test_load_multiple_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multi.jsonl");
    std::fs::write(
        &path,
        r#"{"instance_id":"t1","repo":"a/b","base_commit":"c1","problem_statement":"p1"}
{"instance_id":"t2","repo":"c/d","base_commit":"c2","problem_statement":"p2"}
{"instance_id":"t3","repo":"e/f","base_commit":"c3","problem_statement":"p3"}"#,
    )
    .unwrap();

    let tasks = nexus_code::bench::swe_bench::load_tasks(&path).unwrap();
    assert_eq!(tasks.len(), 3);
}

#[test]
fn test_load_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.jsonl");
    std::fs::write(&path, "").unwrap();

    let tasks = nexus_code::bench::swe_bench::load_tasks(&path).unwrap();
    assert!(tasks.is_empty());
}

// ═══════════════════════════════════════════════════════
// Benchmark Report Tests (5)
// ═══════════════════════════════════════════════════════

#[test]
fn test_generate_report_all_pass() {
    let results = vec![
        TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "diff".to_string(),
            turns: 5,
            fuel_consumed: 1000,
            time_secs: 10.0,
            tools_used: vec!["file_read".to_string()],
            audit_entries: 20,
            error: None,
        },
        TaskResult {
            task_id: "t2".to_string(),
            success: true,
            patch: "diff".to_string(),
            turns: 3,
            fuel_consumed: 800,
            time_secs: 8.0,
            tools_used: vec!["search".to_string()],
            audit_entries: 15,
            error: None,
        },
        TaskResult {
            task_id: "t3".to_string(),
            success: true,
            patch: "diff".to_string(),
            turns: 7,
            fuel_consumed: 1200,
            time_secs: 12.0,
            tools_used: vec!["bash".to_string()],
            audit_entries: 25,
            error: None,
        },
    ];
    let report = generate_report(&results, "anthropic", "claude-sonnet-4");
    assert_eq!(report.passed, 3);
    assert_eq!(report.failed, 0);
    assert!((report.pass_rate - 1.0).abs() < 0.001);
}

#[test]
fn test_generate_report_mixed() {
    let results = vec![
        TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 5,
            fuel_consumed: 1000,
            time_secs: 10.0,
            tools_used: vec![],
            audit_entries: 10,
            error: None,
        },
        TaskResult {
            task_id: "t2".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 3,
            fuel_consumed: 800,
            time_secs: 8.0,
            tools_used: vec![],
            audit_entries: 10,
            error: None,
        },
        TaskResult {
            task_id: "t3".to_string(),
            success: false,
            patch: String::new(),
            turns: 10,
            fuel_consumed: 2000,
            time_secs: 20.0,
            tools_used: vec![],
            audit_entries: 10,
            error: Some("timeout".to_string()),
        },
    ];
    let report = generate_report(&results, "openai", "gpt-4o");
    assert_eq!(report.passed, 2);
    assert!((report.pass_rate - 2.0 / 3.0).abs() < 0.01);
}

#[test]
fn test_generate_report_averages() {
    let results = vec![
        TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 4,
            fuel_consumed: 1000,
            time_secs: 10.0,
            tools_used: vec![],
            audit_entries: 0,
            error: None,
        },
        TaskResult {
            task_id: "t2".to_string(),
            success: true,
            patch: "d".to_string(),
            turns: 6,
            fuel_consumed: 2000,
            time_secs: 20.0,
            tools_used: vec![],
            audit_entries: 0,
            error: None,
        },
    ];
    let report = generate_report(&results, "test", "model");
    assert!((report.avg_turns - 5.0).abs() < 0.01);
    assert!((report.avg_fuel - 1500.0).abs() < 0.01);
    assert!((report.avg_time_secs - 15.0).abs() < 0.01);
    assert!((report.total_time_secs - 30.0).abs() < 0.01);
}

#[test]
fn test_format_report_contains_stats() {
    let report = BenchmarkReport {
        total_tasks: 10,
        passed: 7,
        failed: 2,
        errored: 1,
        pass_rate: 0.7,
        avg_turns: 5.0,
        avg_fuel: 1000.0,
        avg_time_secs: 15.0,
        total_time_secs: 150.0,
        provider: "anthropic".to_string(),
        model: "sonnet".to_string(),
        results: vec![],
    };
    let formatted = format_report(&report);
    assert!(formatted.contains("70.0%"));
    assert!(formatted.contains("anthropic"));
    assert!(formatted.contains("10 total"));
}

#[test]
fn test_save_load_report() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.json");

    let report = BenchmarkReport {
        total_tasks: 5,
        passed: 3,
        failed: 1,
        errored: 1,
        pass_rate: 0.6,
        avg_turns: 4.0,
        avg_fuel: 900.0,
        avg_time_secs: 12.0,
        total_time_secs: 60.0,
        provider: "test".to_string(),
        model: "test-model".to_string(),
        results: vec![TaskResult {
            task_id: "t1".to_string(),
            success: true,
            patch: "p".to_string(),
            turns: 4,
            fuel_consumed: 900,
            time_secs: 12.0,
            tools_used: vec!["file_read".to_string()],
            audit_entries: 10,
            error: None,
        }],
    };
    save_report(&report, &path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let loaded: BenchmarkReport = serde_json::from_str(&content).unwrap();
    assert_eq!(loaded.total_tasks, 5);
    assert_eq!(loaded.passed, 3);
    assert_eq!(loaded.results.len(), 1);
}

// ═══════════════════════════════════════════════════════
// Task Result Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_task_result_success() {
    let result = TaskResult {
        task_id: "test-task".to_string(),
        success: true,
        patch: "--- a/file.py\n+++ b/file.py".to_string(),
        turns: 5,
        fuel_consumed: 1500,
        time_secs: 25.0,
        tools_used: vec!["file_read".to_string(), "file_edit".to_string()],
        audit_entries: 30,
        error: None,
    };
    assert!(result.success);
    assert!(!result.patch.is_empty());
    assert_eq!(result.tools_used.len(), 2);
}

#[test]
fn test_task_result_with_error() {
    let result = TaskResult {
        task_id: "fail-task".to_string(),
        success: false,
        patch: String::new(),
        turns: 10,
        fuel_consumed: 5000,
        time_secs: 60.0,
        tools_used: vec![],
        audit_entries: 5,
        error: Some("Fuel exhausted".to_string()),
    };
    assert!(!result.success);
    assert!(result.error.is_some());
    assert!(result.error.as_ref().map_or(false, |e| e.contains("Fuel")));
}

#[test]
fn test_task_result_tool_usage() {
    let result = TaskResult {
        task_id: "t".to_string(),
        success: true,
        patch: "p".to_string(),
        turns: 3,
        fuel_consumed: 800,
        time_secs: 10.0,
        tools_used: vec![
            "file_read".to_string(),
            "search".to_string(),
            "file_edit".to_string(),
            "bash".to_string(),
        ],
        audit_entries: 20,
        error: None,
    };
    assert_eq!(result.tools_used.len(), 4);
    assert!(result.tools_used.contains(&"file_read".to_string()));
    assert!(result.tools_used.contains(&"bash".to_string()));
}
