use crate::scanner::ProjectMap;
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestFramework {
    Cargo,
    Npm,
    Pytest,
    Go,
    Jest,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestError {
    pub test_name: String,
    pub error_message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub stack_trace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    pub framework: TestFramework,
    pub passed: usize,
    pub failed: usize,
    pub errors: Vec<TestError>,
    pub stdout: String,
    pub stderr: String,
}

pub fn detect_test_framework(project: &ProjectMap) -> TestFramework {
    if project
        .config_files
        .iter()
        .any(|path| path.ends_with("Cargo.toml"))
    {
        return TestFramework::Cargo;
    }
    if project
        .config_files
        .iter()
        .any(|path| path.ends_with("jest.config.js") || path.ends_with("jest.config.ts"))
    {
        return TestFramework::Jest;
    }
    if project
        .config_files
        .iter()
        .any(|path| path.ends_with("package.json"))
    {
        return TestFramework::Npm;
    }
    if project
        .config_files
        .iter()
        .any(|path| path.ends_with("pyproject.toml") || path.ends_with("pytest.ini"))
    {
        return TestFramework::Pytest;
    }
    if project
        .config_files
        .iter()
        .any(|path| path.ends_with("go.mod"))
    {
        return TestFramework::Go;
    }
    TestFramework::Unknown
}

pub fn run_tests(project_path: impl AsRef<Path>) -> Result<TestResult, AgentError> {
    let root = project_path.as_ref();
    let framework = detect_test_framework_from_path(root);
    match framework {
        TestFramework::Cargo => run_with_command(root, framework, "cargo test"),
        TestFramework::Npm => run_with_command(root, framework, "npm test"),
        TestFramework::Pytest => run_with_command(root, framework, "python3 -m pytest -v"),
        TestFramework::Go => run_with_command(root, framework, "go test ./..."),
        TestFramework::Jest => run_with_command(root, framework, "npx jest"),
        TestFramework::Unknown => Err(AgentError::SupervisorError(
            "unable to detect supported test framework".to_string(),
        )),
    }
}

fn detect_test_framework_from_path(root: &Path) -> TestFramework {
    if root.join("Cargo.toml").exists() {
        return TestFramework::Cargo;
    }
    if root.join("jest.config.js").exists() || root.join("jest.config.ts").exists() {
        return TestFramework::Jest;
    }
    if root.join("package.json").exists() {
        return TestFramework::Npm;
    }
    if root.join("pyproject.toml").exists() || root.join("pytest.ini").exists() {
        return TestFramework::Pytest;
    }
    if root.join("go.mod").exists() {
        return TestFramework::Go;
    }
    TestFramework::Unknown
}

fn run_with_command(
    root: &Path,
    framework: TestFramework,
    command: &str,
) -> Result<TestResult, AgentError> {
    let output = spawn_shell(command, root)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let (passed, failed) = parse_counts(framework, stdout.as_str(), stderr.as_str());
    let mut errors = parse_errors(framework, stdout.as_str(), stderr.as_str());
    if !output.status.success() && errors.is_empty() {
        errors.push(TestError {
            test_name: "unknown".to_string(),
            error_message: "test command failed".to_string(),
            file: None,
            line: None,
            stack_trace: format!("stdout:\n{}\nstderr:\n{}", stdout, stderr),
        });
    }

    Ok(TestResult {
        framework,
        passed,
        failed,
        errors,
        stdout,
        stderr,
    })
}

fn spawn_shell(command: &str, cwd: &Path) -> Result<std::process::Output, AgentError> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut shell_cmd = Command::new("cmd");
        shell_cmd.args(["/C", command]);
        shell_cmd
    } else {
        let mut shell_cmd = Command::new("sh");
        shell_cmd.args(["-lc", command]);
        shell_cmd
    };
    cmd.current_dir(cwd);
    cmd.output().map_err(|error| {
        AgentError::SupervisorError(format!(
            "failed to execute test command '{command}': {error}"
        ))
    })
}

fn parse_counts(framework: TestFramework, stdout: &str, stderr: &str) -> (usize, usize) {
    match framework {
        TestFramework::Cargo => parse_cargo_counts(stdout),
        TestFramework::Pytest => parse_pytest_counts(stdout),
        TestFramework::Go => parse_go_counts(stdout),
        TestFramework::Npm | TestFramework::Jest => parse_jest_or_npm_counts(stdout),
        TestFramework::Unknown => parse_generic_counts(stdout, stderr),
    }
}

fn parse_errors(framework: TestFramework, stdout: &str, stderr: &str) -> Vec<TestError> {
    match framework {
        TestFramework::Cargo => parse_cargo_errors(stdout, stderr),
        TestFramework::Pytest => parse_pytest_errors(stdout, stderr),
        TestFramework::Go => parse_go_errors(stdout, stderr),
        TestFramework::Npm | TestFramework::Jest => parse_jest_errors(stdout, stderr),
        TestFramework::Unknown => Vec::new(),
    }
}

fn parse_cargo_counts(stdout: &str) -> (usize, usize) {
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    for line in stdout.lines() {
        if !line.contains("test result:") {
            continue;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        for window in words.windows(2) {
            if window[1].starts_with("passed;") {
                passed += window[0].parse::<usize>().unwrap_or(0);
            } else if window[1].starts_with("failed;") {
                failed += window[0].parse::<usize>().unwrap_or(0);
            }
        }
    }
    (passed, failed)
}

fn parse_pytest_counts(stdout: &str) -> (usize, usize) {
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    for line in stdout.lines() {
        if line.contains("... ok") {
            passed += 1;
        } else if line.contains("... FAIL") || line.contains("... FAILED") {
            failed += 1;
        }
    }
    (passed, failed)
}

fn parse_go_counts(stdout: &str) -> (usize, usize) {
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    for line in stdout.lines() {
        if line.starts_with("ok\t") || line.starts_with("ok  ") {
            passed += 1;
        } else if line.starts_with("--- FAIL:") || line.starts_with("FAIL\t") {
            failed += 1;
        }
    }
    (passed, failed)
}

fn parse_jest_or_npm_counts(stdout: &str) -> (usize, usize) {
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("PASS ") {
            passed += 1;
        } else if trimmed.starts_with("FAIL ") {
            failed += 1;
        } else if trimmed.starts_with("Tests:") {
            // Handles Jest summary like: Tests: 1 failed, 4 passed, 5 total
            let numbers = trimmed
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .collect::<Vec<_>>();
            for index in 0..numbers.len().saturating_sub(1) {
                let value = numbers[index].parse::<usize>().ok();
                let label = numbers[index + 1];
                if let Some(count) = value {
                    if label == "passed" {
                        passed = passed.max(count);
                    } else if label == "failed" {
                        failed = failed.max(count);
                    }
                }
            }
        }
    }
    (passed, failed)
}

fn parse_generic_counts(stdout: &str, stderr: &str) -> (usize, usize) {
    if stdout.contains("ok") && !stdout.contains("failed") && stderr.trim().is_empty() {
        return (1, 0);
    }
    (0, 1)
}

fn parse_cargo_errors(stdout: &str, stderr: &str) -> Vec<TestError> {
    let mut errors = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("test ") && trimmed.ends_with("... FAILED") {
            let name = trimmed
                .trim_start_matches("test ")
                .trim_end_matches("... FAILED")
                .trim()
                .to_string();
            errors.push(TestError {
                test_name: name,
                error_message: "assertion failed".to_string(),
                file: None,
                line: None,
                stack_trace: stderr.to_string(),
            });
        }
    }
    errors
}

fn parse_pytest_errors(stdout: &str, stderr: &str) -> Vec<TestError> {
    let mut errors = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("... FAIL") && !trimmed.contains("... FAILED") {
            continue;
        }
        let name = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string();
        errors.push(TestError {
            test_name: name,
            error_message: "pytest failure".to_string(),
            file: None,
            line: None,
            stack_trace: stderr.to_string(),
        });
    }
    errors
}

fn parse_go_errors(stdout: &str, stderr: &str) -> Vec<TestError> {
    let mut errors = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("--- FAIL:") {
            continue;
        }
        let name = trimmed
            .trim_start_matches("--- FAIL:")
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string();
        errors.push(TestError {
            test_name: name,
            error_message: "go test failure".to_string(),
            file: None,
            line: None,
            stack_trace: stderr.to_string(),
        });
    }
    errors
}

fn parse_jest_errors(stdout: &str, stderr: &str) -> Vec<TestError> {
    let mut errors = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("FAIL ") {
            continue;
        }
        let name = trimmed.trim_start_matches("FAIL ").trim().to_string();
        errors.push(TestError {
            test_name: name,
            error_message: "jest failure".to_string(),
            file: None,
            line: None,
            stack_trace: stderr.to_string(),
        });
    }
    errors
}
