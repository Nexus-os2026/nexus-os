//! Benchmark report generation and formatting.

use super::{BenchmarkReport, TaskResult};

/// Generate an aggregate report from task results.
pub fn generate_report(results: &[TaskResult], provider: &str, model: &str) -> BenchmarkReport {
    let total = results.len();
    let passed = results.iter().filter(|r| r.success).count();
    let errored = results.iter().filter(|r| r.error.is_some()).count();
    let failed = total.saturating_sub(passed + errored);

    let pass_rate = if total > 0 {
        passed as f64 / total as f64
    } else {
        0.0
    };

    let successful: Vec<&TaskResult> = results.iter().filter(|r| r.success).collect();
    let avg_turns = if !successful.is_empty() {
        successful.iter().map(|r| r.turns as f64).sum::<f64>() / successful.len() as f64
    } else {
        0.0
    };
    let avg_fuel = if !successful.is_empty() {
        successful
            .iter()
            .map(|r| r.fuel_consumed as f64)
            .sum::<f64>()
            / successful.len() as f64
    } else {
        0.0
    };
    let avg_time = if !successful.is_empty() {
        successful.iter().map(|r| r.time_secs).sum::<f64>() / successful.len() as f64
    } else {
        0.0
    };
    let total_time = results.iter().map(|r| r.time_secs).sum();

    BenchmarkReport {
        total_tasks: total,
        passed,
        failed,
        errored,
        pass_rate,
        avg_turns,
        avg_fuel,
        avg_time_secs: avg_time,
        total_time_secs: total_time,
        provider: provider.to_string(),
        model: model.to_string(),
        results: results.to_vec(),
    }
}

/// Format a report as a human-readable string.
pub fn format_report(report: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str("\n=== Nexus Code Benchmark Report ===\n");
    out.push_str(&format!("Provider: {}/{}\n", report.provider, report.model));
    out.push_str(&format!(
        "Tasks: {} total, {} passed, {} failed, {} errored\n",
        report.total_tasks, report.passed, report.failed, report.errored
    ));
    out.push_str(&format!("Pass rate: {:.1}%\n", report.pass_rate * 100.0));
    out.push_str(&format!("Avg turns: {:.1}\n", report.avg_turns));
    out.push_str(&format!("Avg fuel: {:.0}\n", report.avg_fuel));
    out.push_str(&format!("Avg time: {:.1}s\n", report.avg_time_secs));
    out.push_str(&format!("Total time: {:.0}s\n", report.total_time_secs));

    out.push_str("\nResults:\n");
    for result in &report.results {
        let status = if result.success {
            "\u{2713}"
        } else {
            "\u{2717}"
        };
        out.push_str(&format!(
            "  {} {} \u{2014} {}fu, {:.1}s, {} tools\n",
            status,
            result.task_id,
            result.fuel_consumed,
            result.time_secs,
            result.tools_used.len()
        ));
        if let Some(ref err) = result.error {
            let short = if err.len() > 60 { &err[..60] } else { err };
            out.push_str(&format!("    Error: {}\n", short));
        }
    }

    out
}

/// Format a comparison of multiple benchmark reports.
pub fn format_comparison(reports: &[BenchmarkReport]) -> String {
    let mut out = String::new();
    out.push_str("\n=== Multi-Provider Benchmark Comparison ===\n\n");
    out.push_str(&format!(
        "  {:<25} {:>8} {:>8} {:>8} {:>8}\n",
        "Provider/Model", "Pass%", "AvgFuel", "AvgTurn", "Tasks"
    ));
    out.push_str(&format!("  {}\n", "\u{2500}".repeat(60)));

    for report in reports {
        out.push_str(&format!(
            "  {:<25} {:>7.1}% {:>8.0} {:>8.1} {:>8}\n",
            format!("{}/{}", report.provider, report.model),
            report.pass_rate * 100.0,
            report.avg_fuel,
            report.avg_turns,
            report.total_tasks,
        ));
    }
    out.push('\n');

    for report in reports {
        let metrics = super::GovernanceMetrics::from_results(&report.results);
        out.push_str(&format!("  {}/{}:\n", report.provider, report.model));
        out.push_str(&format!(
            "    Avg fuel/task: {:.0}\n",
            metrics.avg_fuel_per_task
        ));
        out.push_str(&format!(
            "    Avg audit/task: {:.0}\n",
            metrics.avg_audit_entries_per_task
        ));
        out.push_str(&format!(
            "    Avg tools/task: {:.1}\n",
            metrics.avg_tools_per_task
        ));
        out.push('\n');
    }

    out
}

/// Save a report to disk as JSON.
pub fn save_report(
    report: &BenchmarkReport,
    path: &std::path::Path,
) -> Result<(), crate::error::NxError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| crate::error::NxError::ConfigError(format!("Serialize: {}", e)))?;
    std::fs::write(path, json)?;
    Ok(())
}
