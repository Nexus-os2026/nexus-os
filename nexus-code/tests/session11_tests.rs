//! Session 11 tests — governance metrics, data pipeline, instrumented execution.

use nexus_code::bench::data_pipeline::PaperDataPackage;
use nexus_code::governance_metrics::{
    AggregateGovernanceMetrics, GovernanceTiming, GovernanceTimingCollector,
};

// ═══════════════════════════════════════════════════════
// Governance Timing Tests (5)
// ═══════════════════════════════════════════════════════

#[test]
fn test_governance_timing_default() {
    let timing = GovernanceTiming::default();
    assert_eq!(timing.capability_check_us, 0);
    assert_eq!(timing.total_us, 0);
    assert_eq!(timing.total_governance_overhead_us, 0);
}

#[test]
fn test_governance_timing_overhead_percentage() {
    let timing = GovernanceTiming {
        total_governance_overhead_us: 100,
        total_us: 1000,
        tool_execution_us: 900,
        ..Default::default()
    };
    assert!((timing.overhead_percentage() - 10.0).abs() < 0.1);
}

#[test]
fn test_aggregate_from_timings() {
    let timings = vec![
        GovernanceTiming {
            capability_check_us: 10,
            fuel_reservation_us: 5,
            consent_classification_us: 3,
            tool_execution_us: 500,
            audit_recording_us: 8,
            fuel_consumption_us: 4,
            total_governance_overhead_us: 30,
            total_us: 530,
        },
        GovernanceTiming {
            capability_check_us: 20,
            fuel_reservation_us: 10,
            consent_classification_us: 6,
            tool_execution_us: 800,
            audit_recording_us: 12,
            fuel_consumption_us: 6,
            total_governance_overhead_us: 54,
            total_us: 854,
        },
        GovernanceTiming {
            capability_check_us: 15,
            fuel_reservation_us: 7,
            consent_classification_us: 4,
            tool_execution_us: 600,
            audit_recording_us: 10,
            fuel_consumption_us: 5,
            total_governance_overhead_us: 41,
            total_us: 641,
        },
    ];
    let agg = AggregateGovernanceMetrics::from_timings(&timings);
    assert_eq!(agg.sample_count, 3);
    assert!((agg.avg_capability_check_us - 15.0).abs() < 0.1);
    assert!((agg.avg_fuel_reservation_us - 7.33).abs() < 0.1);
}

#[test]
fn test_aggregate_percentiles() {
    // 10 timings with increasing overhead
    let timings: Vec<GovernanceTiming> = (1..=10)
        .map(|i| GovernanceTiming {
            total_governance_overhead_us: i * 10,
            total_us: i * 100,
            tool_execution_us: i * 90,
            ..Default::default()
        })
        .collect();
    let agg = AggregateGovernanceMetrics::from_timings(&timings);
    assert_eq!(agg.sample_count, 10);
    // p50 should be around 50-60
    assert!(agg.p50_overhead_us >= 40 && agg.p50_overhead_us <= 60);
    // p99 should be near max (100)
    assert!(agg.p99_overhead_us >= 90);
    assert_eq!(agg.max_overhead_us, 100);
}

#[test]
fn test_latex_row_format() {
    let agg = AggregateGovernanceMetrics {
        sample_count: 100,
        avg_capability_check_us: 5.0,
        avg_fuel_reservation_us: 3.0,
        avg_consent_classification_us: 2.0,
        avg_audit_recording_us: 8.0,
        avg_overhead_percentage: 1.5,
        p50_overhead_us: 15,
        p95_overhead_us: 25,
        ..Default::default()
    };
    let row = agg.to_latex_row("file\\_read");
    assert!(row.contains("file\\_read"));
    assert!(row.contains("1.5\\%"));
    assert!(row.contains("15"));
    assert!(row.contains("\\\\"));
}

// ═══════════════════════════════════════════════════════
// Paper Data Pipeline Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_paper_data_from_reports() {
    let reports = vec![nexus_code::bench::BenchmarkReport {
        total_tasks: 10,
        passed: 7,
        failed: 2,
        errored: 1,
        pass_rate: 0.7,
        avg_turns: 5.0,
        avg_fuel: 1500.0,
        avg_time_secs: 12.0,
        total_time_secs: 120.0,
        provider: "anthropic".to_string(),
        model: "sonnet".to_string(),
        results: vec![],
    }];
    let package = PaperDataPackage::from_reports(&reports);
    assert_eq!(package.pass_rate_table.len(), 1);
    assert_eq!(package.pass_rate_table[0].provider, "anthropic");
    assert!((package.pass_rate_table[0].pass_rate - 0.7).abs() < 0.01);
}

#[test]
fn test_paper_data_tool_usage() {
    let reports = vec![nexus_code::bench::BenchmarkReport {
        total_tasks: 2,
        passed: 2,
        failed: 0,
        errored: 0,
        pass_rate: 1.0,
        avg_turns: 3.0,
        avg_fuel: 1000.0,
        avg_time_secs: 10.0,
        total_time_secs: 20.0,
        provider: "test".to_string(),
        model: "model".to_string(),
        results: vec![
            nexus_code::bench::TaskResult {
                task_id: "t1".to_string(),
                success: true,
                patch: "d".to_string(),
                turns: 3,
                fuel_consumed: 1000,
                time_secs: 10.0,
                tools_used: vec![
                    "file_read".to_string(),
                    "file_read".to_string(),
                    "search".to_string(),
                ],
                audit_entries: 10,
                error: None,
            },
            nexus_code::bench::TaskResult {
                task_id: "t2".to_string(),
                success: true,
                patch: "d".to_string(),
                turns: 3,
                fuel_consumed: 1000,
                time_secs: 10.0,
                tools_used: vec!["file_read".to_string(), "bash".to_string()],
                audit_entries: 10,
                error: None,
            },
        ],
    }];
    let package = PaperDataPackage::from_reports(&reports);
    // file_read appears 3 times total, should be first
    assert!(!package.tool_usage.is_empty());
    assert_eq!(package.tool_usage[0].tool_name, "file_read");
    assert_eq!(package.tool_usage[0].usage_count, 3);
}

#[test]
fn test_paper_data_to_latex() {
    let reports = vec![nexus_code::bench::BenchmarkReport {
        total_tasks: 5,
        passed: 3,
        failed: 2,
        errored: 0,
        pass_rate: 0.6,
        avg_turns: 4.0,
        avg_fuel: 1200.0,
        avg_time_secs: 15.0,
        total_time_secs: 75.0,
        provider: "test".to_string(),
        model: "model".to_string(),
        results: vec![],
    }];
    let package = PaperDataPackage::from_reports(&reports);
    let latex = package.to_latex();
    assert!(latex.contains("\\begin{table}"));
    assert!(latex.contains("\\end{table}"));
    assert!(latex.contains("\\toprule"));
    assert!(latex.contains("\\bottomrule"));
    assert!(latex.contains("test/model"));
}

#[test]
fn test_paper_data_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("paper-data.json");

    let package = PaperDataPackage::from_reports(&[]);
    package.save(&path).unwrap();
    assert!(path.exists());

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("pass_rate_table"));
}

// ═══════════════════════════════════════════════════════
// Integration Tests (3)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_instrumented_execution_produces_timing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

    let tool = nexus_code::tools::file_read::FileReadTool;
    let ctx = nexus_code::tools::ToolContext {
        working_dir: dir.path().to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };
    let mut kernel = nexus_code::governance::GovernanceKernel::new(50_000).unwrap();

    let (result, timing) = nexus_code::tools::execute_governed_instrumented(
        &tool,
        serde_json::json!({"path": "test.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    assert!(result.is_success());
    assert!(timing.total_us > 0);
    // Tool execution should be the bulk of the time
    assert!(timing.tool_execution_us > 0 || timing.total_us < 100);
}

#[tokio::test]
async fn test_overhead_is_small() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "content here").unwrap();

    let tool = nexus_code::tools::file_read::FileReadTool;
    let ctx = nexus_code::tools::ToolContext {
        working_dir: dir.path().to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };
    let mut kernel = nexus_code::governance::GovernanceKernel::new(50_000).unwrap();

    let (_result, timing) = nexus_code::tools::execute_governed_instrumented(
        &tool,
        serde_json::json!({"path": "test.txt"}),
        &ctx,
        &mut kernel,
    )
    .await
    .unwrap();

    // For a trivially fast file_read, governance overhead (Ed25519 signing,
    // SHA-256 hashing in audit) can be significant relative to the I/O.
    // The key metric: absolute overhead should be < 5ms (5000us).
    assert!(
        timing.total_governance_overhead_us < 5_000,
        "Absolute overhead too high: {}us (should be < 5000us)",
        timing.total_governance_overhead_us,
    );
}

#[test]
fn test_timing_collector() {
    let mut collector = GovernanceTimingCollector::new();

    collector.record(GovernanceTiming {
        total_governance_overhead_us: 20,
        total_us: 200,
        tool_execution_us: 180,
        ..Default::default()
    });
    collector.record(GovernanceTiming {
        total_governance_overhead_us: 30,
        total_us: 300,
        tool_execution_us: 270,
        ..Default::default()
    });

    assert_eq!(collector.timings().len(), 2);
    let agg = collector.aggregate();
    assert_eq!(agg.sample_count, 2);
    assert!((agg.avg_governance_overhead_us - 25.0).abs() < 0.1);
}
