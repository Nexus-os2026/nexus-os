use coder_agent::editor::MultiFileEditor;
use coder_agent::git::auto_commit;
use coder_agent::init::init_project_in;
use coder_agent::terminal::{CommandError, TerminalExecutor, TERMINAL_EXECUTE_CAPABILITY};
use coder_agent::writer::FileChange;
use nexus_sdk::autonomy::AutonomyLevel;
use nexus_sdk::consent::{
    ApprovalQueue, ConsentPolicyEngine, ConsentRuntime, GovernedOperation, HitlTier,
};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_safe_command_execution() {
    let project = tempdir().expect("tempdir must be created");

    let mut capabilities = HashSet::new();
    capabilities.insert(TERMINAL_EXECUTE_CAPABILITY.to_string());

    // Configure consent policy with explicit allowed approvers
    let mut policy = ConsentPolicyEngine::default();
    policy.set_policy(
        GovernedOperation::TerminalCommand,
        HitlTier::Tier2,
        vec!["approver.a".to_string()],
    );
    let consent = ConsentRuntime::new(
        policy,
        ApprovalQueue::in_memory(),
        "terminal.test".to_string(),
    );
    let mut executor = TerminalExecutor::with_capabilities_autonomy_and_consent(
        capabilities,
        AutonomyLevel::L1,
        consent,
    );

    let request_id = match executor.execute(
        "git --version",
        project.path(),
        Some(Duration::from_secs(10)),
    ) {
        Err(CommandError::ApprovalRequired(request_id)) => request_id,
        other => panic!("expected approval request for terminal command, got: {other:?}"),
    };
    executor
        .approve_request(request_id.as_str(), "approver.a")
        .expect("approval should succeed");

    let result = executor
        .execute(
            "git --version",
            project.path(),
            Some(Duration::from_secs(10)),
        )
        .expect("git --version should execute successfully after approval");
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("git"),
        "expected git version output in stdout"
    );

    let blocked_request = match executor.execute("rm -rf /", project.path(), None) {
        Err(CommandError::ApprovalRequired(request_id)) => request_id,
        other => panic!("expected approval request for destructive command, got: {other:?}"),
    };
    executor
        .approve_request(blocked_request.as_str(), "approver.a")
        .expect("approval should succeed");
    let blocked = executor.execute("rm -rf /", project.path(), None);
    assert!(matches!(blocked, Err(CommandError::CommandBlocked(_))));
}

#[test]
fn test_atomic_changeset() {
    let project = tempdir().expect("tempdir must be created");
    write_file(project.path().join("one.txt"), "one\n");
    write_file(project.path().join("two.txt"), "two\n");

    let mut editor = MultiFileEditor::new(project.path()).expect("editor should initialize");
    editor
        .apply_changeset(vec![
            FileChange::Modify(
                "one.txt".to_string(),
                "one\n".to_string(),
                "uno\n".to_string(),
            ),
            FileChange::Modify(
                "two.txt".to_string(),
                "two\n".to_string(),
                "dos\n".to_string(),
            ),
            FileChange::Create("three.txt".to_string(), "tres\n".to_string()),
        ])
        .expect("changeset should apply atomically");

    assert_eq!(read_file(project.path().join("one.txt")), "uno\n");
    assert_eq!(read_file(project.path().join("two.txt")), "dos\n");
    assert_eq!(read_file(project.path().join("three.txt")), "tres\n");

    write_file(project.path().join("one.txt"), "one\n");
    write_file(project.path().join("two.txt"), "two\n");
    let _ = fs::remove_file(project.path().join("three.txt"));

    let rollback_result = editor.apply_changeset(vec![
        FileChange::Modify(
            "one.txt".to_string(),
            "one\n".to_string(),
            "uno\n".to_string(),
        ),
        FileChange::Modify(
            "two.txt".to_string(),
            "not-the-current-content\n".to_string(),
            "dos\n".to_string(),
        ),
        FileChange::Create("three.txt".to_string(), "tres\n".to_string()),
    ]);
    assert!(rollback_result.is_err(), "expected changeset failure");

    assert_eq!(
        read_file(project.path().join("one.txt")),
        "one\n",
        "first file should be rolled back"
    );
    assert_eq!(
        read_file(project.path().join("two.txt")),
        "two\n",
        "second file should remain unchanged"
    );
    assert!(
        !project.path().join("three.txt").exists(),
        "third file should not be created after rollback"
    );
}

#[test]
#[ignore = "requires Ollama with a deployed model"]
fn test_project_init_rust() {
    let base = tempdir().expect("tempdir must be created");
    let project = init_project_in(base.path(), "rust", "binary", "my-app", Some("sample app"))
        .expect("rust binary template should be initialized");

    assert!(project.join("Cargo.toml").exists());
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("README.md").exists());
    assert!(project.join(".gitignore").exists());
    assert!(project.join("tests").exists());
}

#[test]
fn test_git_auto_commit() {
    let project = tempdir().expect("tempdir must be created");
    git(project.path(), &["init"]);
    git(
        project.path(),
        &["config", "user.email", "nexus@example.com"],
    );
    git(project.path(), &["config", "user.name", "Nexus Test"]);

    write_file(project.path().join("README.md"), "# Demo\n");
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "chore: init"]);

    write_file(project.path().join("README.md"), "# Demo\n\nupdated\n");
    let hash = auto_commit(project.path(), "update readme")
        .expect("auto commit should stage and commit changes");
    assert!(!hash.trim().is_empty());

    let subject = git(project.path(), &["log", "-1", "--pretty=%s"]);
    assert_eq!(subject.trim(), "feat: update readme");
}

fn write_file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    fs::write(path, contents).expect("file should be written");
}

fn read_file(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).expect("file should be readable")
}

fn git(project: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(project)
        .args(args)
        .output()
        .expect("git command should start");

    if !output.status.success() {
        panic!(
            "git {} failed:\nstdout: {}\nstderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8_lossy(&output.stdout).to_string()
}
