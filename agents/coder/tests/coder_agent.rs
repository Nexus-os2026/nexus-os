use coder_agent::analyzer::{analyze, ProjectType};
use coder_agent::context::build_context;
use coder_agent::fix_loop::{fix_until_pass_with, ErrorFixer, TestExecutor};
use coder_agent::scanner::{detect_language, scan_project, Language};
use coder_agent::test_runner::{TestError, TestFramework, TestResult};
use coder_agent::writer::{detect_style, NamingConvention};
use nexus_kernel::errors::AgentError;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

#[test]
fn test_scan_rust_project() {
    let project = scan_project(repo_root()).expect("project scan should succeed");
    let report = analyze(&project).expect("analysis should succeed");

    assert_eq!(report.project_type, ProjectType::RustWorkspace);
    assert!(
        project
            .config_files
            .iter()
            .any(|path| path.ends_with("Cargo.toml")),
        "expected Cargo.toml in config files"
    );

    let crate_count = project
        .config_files
        .iter()
        .filter(|path| path.ends_with("Cargo.toml"))
        .count();
    assert!(
        crate_count >= 15,
        "expected at least 15 Cargo.toml files, found {crate_count}"
    );

    assert!(
        project.test_files.len() >= 15,
        "expected rich test discovery across crates"
    );
    assert!(
        project
            .test_files
            .iter()
            .any(|path| path.ends_with("connectors/core/tests/placeholder.rs")),
        "expected connector core test file in scan results"
    );
}

#[test]
fn test_language_detection() {
    assert_eq!(detect_language(Path::new("main.rs"), None), Language::Rust);
    assert_eq!(
        detect_language(Path::new("component.ts"), None),
        Language::TypeScript
    );
    assert_eq!(
        detect_language(Path::new("script.py"), None),
        Language::Python
    );
    assert_eq!(detect_language(Path::new("server.go"), None), Language::Go);
}

#[test]
fn test_context_building() {
    let project = scan_project(repo_root()).expect("project scan should succeed");
    let context = build_context(&project, "add a new connector").expect("context should build");

    let paths = context
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    assert!(
        paths.contains(&"connectors/core/src/connector.rs"),
        "expected Connector trait file in context"
    );
    assert!(
        paths.contains(&"connectors/core/src/github_connector.rs"),
        "expected existing connector example in context"
    );

    let trait_file = context
        .files
        .iter()
        .find(|file| file.path == "connectors/core/src/connector.rs")
        .expect("connector trait file should be selected");
    assert!(
        trait_file.content.contains("trait Connector"),
        "expected Connector trait definition in context payload"
    );
}

struct AlwaysFailExecutor;

impl TestExecutor for AlwaysFailExecutor {
    fn run_tests(&mut self, _project_path: &Path) -> Result<TestResult, AgentError> {
        Ok(TestResult {
            framework: TestFramework::Unknown,
            passed: 0,
            failed: 1,
            errors: vec![TestError {
                test_name: "cannot_fix::test".to_string(),
                error_message: "unfixable failure".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(42),
                stack_trace: "stack trace".to_string(),
            }],
            stdout: String::new(),
            stderr: "unfixable failure".to_string(),
        })
    }
}

struct NoopFixer;

impl ErrorFixer for NoopFixer {
    fn propose_fixes(
        &mut self,
        _project_path: &Path,
        _errors: &[TestError],
        _iteration: u32,
    ) -> Result<Vec<coder_agent::writer::FileChange>, AgentError> {
        Ok(Vec::new())
    }
}

#[test]
fn test_fix_loop_max_iterations() {
    let project_dir = tempdir().expect("temp dir should be created");
    let mut executor = AlwaysFailExecutor;
    let mut fixer = NoopFixer;

    let result = fix_until_pass_with(project_dir.path(), Vec::new(), 5, &mut executor, &mut fixer)
        .expect("fix loop should complete");

    match result {
        coder_agent::fix_loop::FixResult::MaxIterationsReached {
            iterations,
            remaining_errors,
            ..
        } => {
            assert_eq!(iterations, 5);
            assert!(!remaining_errors.is_empty());
        }
        other => panic!("expected MaxIterationsReached, got {other:?}"),
    }
}

#[test]
fn test_style_detection() {
    let project = scan_project(repo_root()).expect("project scan should succeed");
    let style = detect_style(&project).expect("style detection should succeed");

    assert_eq!(style.naming_convention, NamingConvention::SnakeCase);
    assert_eq!(style.indent_width, 4);
    assert_eq!(style.comment_style, "//");
}
