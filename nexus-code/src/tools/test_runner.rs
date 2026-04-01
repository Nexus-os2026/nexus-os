//! TestRunnerTool — run project tests with structured result parsing.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Run project tests with structured result parsing.
pub struct TestRunnerTool;

impl TestRunnerTool {
    /// Detect the test command from project files.
    pub fn detect_test_command(working_dir: &std::path::Path) -> String {
        if working_dir.join("Cargo.toml").exists() {
            "cargo test".to_string()
        } else if working_dir.join("package.json").exists() {
            "npm test".to_string()
        } else if working_dir.join("pyproject.toml").exists()
            || working_dir.join("pytest.ini").exists()
        {
            "python -m pytest".to_string()
        } else if working_dir.join("go.mod").exists() {
            "go test ./...".to_string()
        } else if working_dir.join("Makefile").exists() {
            "make test".to_string()
        } else {
            "echo 'No test runner detected'".to_string()
        }
    }

    /// Parse test results from cargo test output.
    pub fn parse_cargo_test(output: &str) -> (u32, u32, u32, Vec<String>) {
        let mut passed = 0u32;
        let mut failed = 0u32;
        let mut skipped = 0u32;
        let mut failing_tests = Vec::new();

        for line in output.lines() {
            if line.starts_with("test result:") {
                // Extract numbers from patterns like "5 passed", "0 failed", etc.
                for part in line.split(';') {
                    let part = part.trim();
                    // Find the number before "passed", "failed", etc.
                    for word_pair in part.split_whitespace().collect::<Vec<_>>().windows(2) {
                        if let Ok(n) = word_pair[0].parse::<u32>() {
                            if word_pair[1].starts_with("passed") {
                                passed += n;
                            } else if word_pair[1].starts_with("failed") {
                                failed += n;
                            } else if word_pair[1].starts_with("ignored")
                                || word_pair[1].starts_with("filtered")
                            {
                                skipped += n;
                            }
                        }
                    }
                }
            }
            if line.contains("FAILED") && line.starts_with("test ") {
                if let Some(name) = line
                    .strip_prefix("test ")
                    .and_then(|s| s.split(" ...").next())
                {
                    failing_tests.push(name.trim().to_string());
                }
            }
        }

        (passed, failed, skipped, failing_tests)
    }
}

#[async_trait]
impl NxTool for TestRunnerTool {
    fn name(&self) -> &str {
        "test_runner"
    }

    fn description(&self) -> &str {
        "Run project tests and return structured results (pass/fail counts, failing test names). \
         Auto-detects the test runner from project files. Override with the 'command' parameter."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Override test command (default: auto-detected)"
                },
                "filter": {
                    "type": "string",
                    "description": "Filter tests by name pattern"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120, max: 600)"
                }
            }
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        30
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::ShellExecute)
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let mut test_cmd = input
            .get("command")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| Self::detect_test_command(&ctx.working_dir));

        if let Some(filter) = input.get("filter").and_then(|v| v.as_str()) {
            test_cmd = format!("{} {}", test_cmd, filter);
        }

        let timeout = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120)
            .min(600);

        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&test_cmd)
                .current_dir(&ctx.working_dir)
                .env_remove("ANTHROPIC_API_KEY")
                .env_remove("OPENAI_API_KEY")
                .output(),
        )
        .await;

        let duration = start.elapsed().as_secs_f64();

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}\n{}", stdout, stderr);

                let (passed, failed, skipped, failing_tests) = if test_cmd.starts_with("cargo test")
                {
                    Self::parse_cargo_test(&combined)
                } else {
                    (0, 0, 0, Vec::new())
                };
                let total = passed + failed + skipped;

                let mut summary = format!(
                    "Test Results: {} total, {} passed, {} failed, {} skipped ({:.1}s)",
                    total, passed, failed, skipped, duration
                );

                if !failing_tests.is_empty() {
                    summary.push_str("\n\nFailing tests:");
                    for t in &failing_tests {
                        summary.push_str(&format!("\n  \u{2717} {}", t));
                    }
                }

                if failed > 0 || !output.status.success() {
                    let raw_preview = if combined.len() > 5000 {
                        &combined[..5000]
                    } else {
                        &combined
                    };
                    summary.push_str(&format!("\n\nOutput:\n{}", raw_preview));
                    ToolResult::error(summary)
                } else {
                    ToolResult::success(summary)
                }
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to execute tests: {}", e)),
            Err(_) => ToolResult::error(format!("Tests timed out after {} seconds", timeout)),
        }
    }
}
