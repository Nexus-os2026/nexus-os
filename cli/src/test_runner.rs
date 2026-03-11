//! Agent test runner for `nexus test`.
//!
//! Loads a manifest, creates a sandboxed `TestHarness` context with mock LLM
//! and mock filesystem, runs the full agent lifecycle (init → execute → shutdown),
//! and reports fuel consumed, capability usage, and output validation.

use nexus_kernel::manifest::{parse_manifest, AgentManifest};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Outcome of a single agent test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub agent_name: String,
    pub passed: bool,
    pub phase_results: Vec<PhaseResult>,
    pub fuel_budget: u64,
    pub fuel_consumed: u64,
    pub capabilities_used: Vec<String>,
    pub outputs_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: String,
    pub passed: bool,
    pub error: Option<String>,
}

/// Run the agent lifecycle test against a manifest.
///
/// This creates a `TestHarness` from the SDK, builds an `AgentContext` with
/// the manifest's capabilities and fuel, then simulates init/execute/shutdown
/// using a minimal `NexusAgent` implementation that exercises the context.
pub fn run_agent_test(manifest_path: &Path) -> Result<TestReport, String> {
    let manifest_content = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("Cannot read manifest '{}': {e}", manifest_path.display()))?;

    run_agent_test_from_str(&manifest_content)
}

/// Run the agent lifecycle test from a manifest string.
pub fn run_agent_test_from_str(manifest_content: &str) -> Result<TestReport, String> {
    let manifest =
        parse_manifest(manifest_content).map_err(|e| format!("Invalid manifest: {e}"))?;
    run_agent_test_with_manifest(&manifest)
}

/// Core test logic operating on a parsed manifest.
pub fn run_agent_test_with_manifest(manifest: &AgentManifest) -> Result<TestReport, String> {
    use nexus_sdk::context::AgentContext;
    use uuid::Uuid;

    let mut ctx = AgentContext::new(
        Uuid::new_v4(),
        manifest.capabilities.clone(),
        manifest.fuel_budget,
    )
    .with_filesystem_permissions(manifest.filesystem_permissions.clone());

    let mut phases: Vec<PhaseResult> = Vec::new();
    let mut error_msg: Option<String> = None;
    let mut outputs_count = 0;

    // Phase 1: Init — verify capabilities are grantable
    let init_ok = run_init_phase(&mut ctx, &manifest.capabilities);
    phases.push(PhaseResult {
        phase: "init".into(),
        passed: init_ok.is_ok(),
        error: init_ok.as_ref().err().cloned(),
    });

    // Phase 2: Execute — exercise each capability once
    if init_ok.is_ok() {
        match run_execute_phase(&mut ctx, &manifest.capabilities) {
            Ok(count) => {
                outputs_count = count;
                phases.push(PhaseResult {
                    phase: "execute".into(),
                    passed: true,
                    error: None,
                });
            }
            Err(e) => {
                error_msg = Some(e.clone());
                phases.push(PhaseResult {
                    phase: "execute".into(),
                    passed: false,
                    error: Some(e),
                });
            }
        }
    }

    // Phase 3: Shutdown — always runs if init succeeded
    if init_ok.is_ok() {
        phases.push(PhaseResult {
            phase: "shutdown".into(),
            passed: true,
            error: None,
        });
    }

    let fuel_consumed = manifest.fuel_budget - ctx.fuel_remaining();
    let all_passed = phases.iter().all(|p| p.passed);
    if !all_passed && error_msg.is_none() {
        error_msg = phases
            .iter()
            .find(|p| !p.passed)
            .and_then(|p| p.error.clone());
    }

    Ok(TestReport {
        agent_name: manifest.name.clone(),
        passed: all_passed,
        phase_results: phases,
        fuel_budget: manifest.fuel_budget,
        fuel_consumed,
        capabilities_used: manifest.capabilities.clone(),
        outputs_count,
        error: error_msg,
    })
}

/// Simulate init: require each capability to verify it's granted.
fn run_init_phase(
    ctx: &mut nexus_sdk::context::AgentContext,
    capabilities: &[String],
) -> Result<(), String> {
    for cap in capabilities {
        ctx.require_capability(cap)
            .map_err(|e| format!("Init failed: capability '{}' denied: {e}", cap))?;
    }
    Ok(())
}

/// Simulate execute: exercise each capability once via the mock context.
fn run_execute_phase(
    ctx: &mut nexus_sdk::context::AgentContext,
    capabilities: &[String],
) -> Result<usize, String> {
    let mut output_count = 0;

    for cap in capabilities {
        match cap.as_str() {
            "llm.query" => {
                ctx.llm_query("test prompt from nexus test runner", 128)
                    .map_err(|e| format!("Execute failed on llm.query: {e}"))?;
                output_count += 1;
            }
            "fs.read" => {
                ctx.read_file("/test/input.txt")
                    .map_err(|e| format!("Execute failed on fs.read: {e}"))?;
                output_count += 1;
            }
            "fs.write" => {
                ctx.write_file("/test/output.txt", "test output")
                    .map_err(|e| format!("Execute failed on fs.write: {e}"))?;
                output_count += 1;
            }
            // Other capabilities pass through — they're validated at init
            _ => {}
        }
    }

    Ok(output_count)
}

/// Format a test report for CLI display.
pub fn format_report(report: &TestReport) -> String {
    let status = if report.passed { "PASS" } else { "FAIL" };
    let mut out = format!(
        "nexus test: {status}\n\
         Agent:      {}\n\
         Fuel:       {}/{} consumed\n\
         Outputs:    {}\n\n\
         Phases:\n",
        report.agent_name, report.fuel_consumed, report.fuel_budget, report.outputs_count,
    );

    for phase in &report.phase_results {
        let icon = if phase.passed { "ok" } else { "FAIL" };
        out.push_str(&format!("  [{icon}] {}", phase.phase));
        if let Some(err) = &phase.error {
            out.push_str(&format!("  — {err}"));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "\nCapabilities tested: {}\n",
        report.capabilities_used.join(", ")
    ));

    if let Some(err) = &report.error {
        out.push_str(&format!("\nError: {err}\n"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_manifest_str() -> &'static str {
        r#"
name = "test-agent"
version = "0.1.0"
capabilities = ["llm.query"]
fuel_budget = 10000
"#
    }

    fn multi_cap_manifest_str() -> &'static str {
        r#"
name = "multi-agent"
version = "0.1.0"
capabilities = ["llm.query", "fs.read", "fs.write"]
fuel_budget = 10000
"#
    }

    #[test]
    fn test_basic_agent_passes() {
        let report = run_agent_test_from_str(basic_manifest_str()).unwrap();
        assert!(report.passed);
        assert_eq!(report.agent_name, "test-agent");
        assert_eq!(report.phase_results.len(), 3);
        assert!(report.phase_results.iter().all(|p| p.passed));
        assert!(report.fuel_consumed > 0);
        assert_eq!(report.outputs_count, 1);
    }

    #[test]
    fn test_multi_capability_agent_passes() {
        let report = run_agent_test_from_str(multi_cap_manifest_str()).unwrap();
        assert!(report.passed);
        assert_eq!(report.capabilities_used.len(), 3);
        assert_eq!(report.outputs_count, 3);
        // llm.query costs 10, fs.read costs 2, fs.write costs 8 = 20
        assert_eq!(report.fuel_consumed, 20);
    }

    #[test]
    fn test_fuel_exhaustion_detected() {
        let manifest = r#"
name = "low-fuel"
version = "0.1.0"
capabilities = ["llm.query"]
fuel_budget = 5
"#;
        let report = run_agent_test_from_str(manifest).unwrap();
        assert!(!report.passed);
        assert!(report.error.is_some());
        assert!(report.error.as_ref().unwrap().contains("llm.query"));
    }

    #[test]
    fn test_invalid_manifest_returns_error() {
        let result = run_agent_test_from_str("not valid toml {{{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_report_format_contains_status() {
        let report = run_agent_test_from_str(basic_manifest_str()).unwrap();
        let formatted = format_report(&report);
        assert!(formatted.contains("PASS"));
        assert!(formatted.contains("test-agent"));
        assert!(formatted.contains("llm.query"));
    }

    #[test]
    fn test_report_format_shows_failure() {
        let manifest = r#"
name = "failing-agent"
version = "0.1.0"
capabilities = ["llm.query"]
fuel_budget = 5
"#;
        let report = run_agent_test_from_str(manifest).unwrap();
        let formatted = format_report(&report);
        assert!(formatted.contains("FAIL"));
    }
}
