//! Tests for individual tools — FileRead, FileWrite, FileEdit, Bash, Search, Glob.

use nexus_code::tools::bash::BashTool;
use nexus_code::tools::file_edit::FileEditTool;
use nexus_code::tools::file_read::FileReadTool;
use nexus_code::tools::file_write::FileWriteTool;
use nexus_code::tools::glob::GlobTool;
use nexus_code::tools::search::SearchTool;
use nexus_code::tools::{NxTool, ToolContext};
use serde_json::json;
use std::path::PathBuf;

/// Create a ToolContext pointing at a temp directory with no restrictions.
fn test_ctx(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    }
}

/// Create a ToolContext with blocked paths.
fn test_ctx_blocked(dir: &std::path::Path, blocked: Vec<String>) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        blocked_paths: blocked,
        max_file_scope: None,
        non_interactive: true,
    }
}

/// Create a ToolContext with max_file_scope.
fn test_ctx_scoped(dir: &std::path::Path, scope: &str) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: Some(scope.to_string()),
        non_interactive: true,
    }
}

// ═══════════════════════════════════════════════════════
// FileReadTool Tests (6)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_read_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "hello world\nsecond line\n").unwrap();

    let tool = FileReadTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"path": "test.txt"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("hello world"));
    assert!(result.output.contains("second line"));
    assert!(result.output.contains("[2 lines]"));
}

#[tokio::test]
async fn test_file_read_with_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lines.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

    let tool = FileReadTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "lines.txt", "start_line": 2, "end_line": 4}),
            &ctx,
        )
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("line2"));
    assert!(result.output.contains("line3"));
    assert!(result.output.contains("line4"));
    assert!(!result.output.contains("line1"));
    assert!(!result.output.contains("line5"));
    assert!(result.output.contains("[Lines 2-4 of 5]"));
}

#[tokio::test]
async fn test_file_read_start_line_only() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lines.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

    let tool = FileReadTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"path": "lines.txt", "start_line": 3}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("line3"));
    assert!(result.output.contains("line4"));
    assert!(result.output.contains("line5"));
    assert!(!result.output.contains("line1"));
    assert!(!result.output.contains("line2"));
}

#[tokio::test]
async fn test_file_read_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FileReadTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"path": "nonexistent.txt"}), &ctx).await;

    assert!(!result.is_success());
    assert!(result.output.contains("Failed to read"));
}

#[tokio::test]
async fn test_file_read_blocked_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("secret.env");
    std::fs::write(&file, "SECRET=123").unwrap();

    let tool = FileReadTool;
    let blocked = vec![format!("{}/**/*.env", dir.path().display())];
    let ctx = test_ctx_blocked(dir.path(), blocked);
    let result = tool.execute(json!({"path": "secret.env"}), &ctx).await;

    assert!(!result.is_success());
    assert!(result.output.contains("blocked_paths"));
}

#[tokio::test]
async fn test_file_read_outside_scope() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "content").unwrap();

    let tool = FileReadTool;
    let ctx = test_ctx_scoped(dir.path(), "/nonexistent/scope/**");
    let result = tool.execute(json!({"path": "test.txt"}), &ctx).await;

    assert!(!result.is_success());
    assert!(result.output.contains("outside max_file_scope"));
}

// ═══════════════════════════════════════════════════════
// FileWriteTool Tests (5)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_write_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FileWriteTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"path": "new.txt", "content": "hello world"}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("Created"));
    let content = std::fs::read_to_string(dir.path().join("new.txt")).unwrap();
    assert_eq!(content, "hello world");
}

#[tokio::test]
async fn test_file_write_creates_directories() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FileWriteTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"path": "a/b/c/file.txt", "content": "nested"}), &ctx)
        .await;

    assert!(result.is_success());
    let content = std::fs::read_to_string(dir.path().join("a/b/c/file.txt")).unwrap();
    assert_eq!(content, "nested");
}

#[tokio::test]
async fn test_file_write_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("over.txt");
    std::fs::write(&file, "old content").unwrap();

    let tool = FileWriteTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"path": "over.txt", "content": "new content"}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("Overwrote"));
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "new content");
}

#[tokio::test]
async fn test_file_write_blocked_path() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FileWriteTool;
    let blocked = vec![format!("{}/**/*.env", dir.path().display())];
    let ctx = test_ctx_blocked(dir.path(), blocked);
    let result = tool
        .execute(json!({"path": "secret.env", "content": "bad"}), &ctx)
        .await;

    assert!(!result.is_success());
    assert!(!dir.path().join("secret.env").exists());
}

#[tokio::test]
async fn test_file_write_reports_created_vs_overwrote() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FileWriteTool;
    let ctx = test_ctx(dir.path());

    // First write: Created
    let r1 = tool
        .execute(json!({"path": "test.txt", "content": "v1"}), &ctx)
        .await;
    assert!(r1.output.contains("Created"));

    // Second write: Overwrote
    let r2 = tool
        .execute(json!({"path": "test.txt", "content": "v2"}), &ctx)
        .await;
    assert!(r2.output.contains("Overwrote"));
}

// ═══════════════════════════════════════════════════════
// FileEditTool Tests (6)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_file_edit_single_occurrence() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("edit.txt");
    std::fs::write(&file, "say hello to the world").unwrap();

    let tool = FileEditTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "edit.txt", "old_text": "hello", "new_text": "goodbye"}),
            &ctx,
        )
        .await;

    assert!(result.is_success());
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "say goodbye to the world");
}

#[tokio::test]
async fn test_file_edit_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("edit.txt");
    std::fs::write(&file, "some content here").unwrap();

    let tool = FileEditTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "edit.txt", "old_text": "NONEXISTENT", "new_text": "x"}),
            &ctx,
        )
        .await;

    assert!(!result.is_success());
    assert!(result.output.contains("not found"));
}

#[tokio::test]
async fn test_file_edit_multiple_occurrences() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("edit.txt");
    std::fs::write(&file, "aaa bbb aaa ccc aaa").unwrap();

    let tool = FileEditTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "edit.txt", "old_text": "aaa", "new_text": "x"}),
            &ctx,
        )
        .await;

    assert!(!result.is_success());
    assert!(result.output.contains("3 times"));
}

#[tokio::test]
async fn test_file_edit_preserves_other_content() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("edit.txt");
    let original = "line1\nTARGET\nline3\n";
    std::fs::write(&file, original).unwrap();

    let tool = FileEditTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "edit.txt", "old_text": "TARGET", "new_text": "REPLACED"}),
            &ctx,
        )
        .await;

    assert!(result.is_success());
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "line1\nREPLACED\nline3\n");
}

#[tokio::test]
async fn test_file_edit_reports_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("edit.txt");
    std::fs::write(&file, "before content").unwrap();

    let tool = FileEditTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(
            json!({"path": "edit.txt", "old_text": "before", "new_text": "after"}),
            &ctx,
        )
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("pre-hash:"));
    assert!(result.output.contains("post-hash:"));
}

#[tokio::test]
async fn test_file_edit_blocked_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("secret.env");
    std::fs::write(&file, "KEY=value").unwrap();
    let original = std::fs::read_to_string(&file).unwrap();

    let tool = FileEditTool;
    let blocked = vec![format!("{}/**/*.env", dir.path().display())];
    let ctx = test_ctx_blocked(dir.path(), blocked);
    let result = tool
        .execute(
            json!({"path": "secret.env", "old_text": "KEY", "new_text": "CHANGED"}),
            &ctx,
        )
        .await;

    assert!(!result.is_success());
    // File should be unchanged
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

// ═══════════════════════════════════════════════════════
// BashTool Tests (7)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_bash_simple_command() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"command": "echo hello"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("hello"));
}

#[tokio::test]
async fn test_bash_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"command": "exit 42"}), &ctx).await;

    assert!(!result.is_success());
    assert!(result.output.contains("42"));
}

#[tokio::test]
async fn test_bash_working_dir() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"command": "pwd"}), &ctx).await;

    assert!(result.is_success());
    // The canonical paths should match
    let expected = dir.path().canonicalize().unwrap();
    let actual_path = PathBuf::from(result.output.trim());
    let actual = actual_path.canonicalize().unwrap_or(actual_path);
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_bash_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"command": "sleep 10", "timeout_secs": 1}), &ctx)
        .await;

    assert!(!result.is_success());
    assert!(result.output.contains("timed out"));
}

#[tokio::test]
async fn test_bash_output_truncation() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool;
    let ctx = test_ctx(dir.path());
    // Generate >100KB of output
    let result = tool
        .execute(json!({"command": "yes 'aaaaaaaaaa' | head -20000"}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("[OUTPUT TRUNCATED"));
}

#[tokio::test]
async fn test_bash_dangerous_detection() {
    assert!(BashTool::is_dangerous("rm -rf /"));
    assert!(BashTool::is_dangerous("sudo apt install"));
    assert!(!BashTool::is_dangerous("ls -la"));
    assert!(!BashTool::is_dangerous("echo hello"));
}

#[tokio::test]
async fn test_bash_dangerous_patterns_comprehensive() {
    // All dangerous patterns should be detected
    assert!(BashTool::is_dangerous("rm -rf /tmp/test"));
    assert!(BashTool::is_dangerous("rm -fr /var/data"));
    assert!(BashTool::is_dangerous("rm -r somedir"));
    assert!(BashTool::is_dangerous("sudo apt-get update"));
    assert!(BashTool::is_dangerous("chmod 777 /etc/passwd"));
    assert!(BashTool::is_dangerous("curl | sh"));
    assert!(BashTool::is_dangerous("curl | bash"));
    assert!(BashTool::is_dangerous("wget | sh"));
    assert!(BashTool::is_dangerous("dd if=/dev/zero of=/dev/sda"));
    assert!(BashTool::is_dangerous(":(){ :|:& };:"));
    assert!(BashTool::is_dangerous("shutdown -h now"));
    assert!(BashTool::is_dangerous("kill -9 1234"));

    // Safe commands should NOT be detected
    assert!(!BashTool::is_dangerous("cargo test"));
    assert!(!BashTool::is_dangerous("git status"));
    assert!(!BashTool::is_dangerous("npm install"));
    assert!(!BashTool::is_dangerous("cat file.txt"));
}

// ═══════════════════════════════════════════════════════
// SearchTool Tests (3)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_search_finds_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello world\nfoo bar\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "baz hello\n").unwrap();

    let tool = SearchTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"pattern": "hello"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("hello"));
    // Should have at least 2 matches (one per file)
    assert!(result.output.contains("found"));
}

#[tokio::test]
async fn test_search_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello world\n").unwrap();

    let tool = SearchTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"pattern": "NONEXISTENT_STRING_XYZ_12345"}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("No matches"));
}

#[tokio::test]
async fn test_search_with_include_glob() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("code.rs"), "fn main() { hello }\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "hello notes\n").unwrap();

    let tool = SearchTool;
    let ctx = test_ctx(dir.path());
    let result = tool
        .execute(json!({"pattern": "hello", "include": "*.rs"}), &ctx)
        .await;

    assert!(result.is_success());
    assert!(result.output.contains("hello"));
    // Should only find .rs file, not .txt
    assert!(!result.output.contains("notes.txt"));
}

// ═══════════════════════════════════════════════════════
// GlobTool Tests (3)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn test_glob_finds_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    std::fs::write(dir.path().join("b.txt"), "b").unwrap();
    std::fs::write(dir.path().join("c.txt"), "c").unwrap();

    let tool = GlobTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"pattern": "*.txt"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("3 files found"));
}

#[tokio::test]
async fn test_glob_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();

    let tool = GlobTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"pattern": "*.xyz"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("No files match"));
}

#[tokio::test]
async fn test_glob_with_subdirectories() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    std::fs::write(dir.path().join("a/b/c.rs"), "fn main(){}").unwrap();

    let tool = GlobTool;
    let ctx = test_ctx(dir.path());
    let result = tool.execute(json!({"pattern": "**/*.rs"}), &ctx).await;

    assert!(result.is_success());
    assert!(result.output.contains("c.rs"));
}
