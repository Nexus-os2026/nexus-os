use nexus_code::tools::{search::SearchTool, NxTool, ToolContext};
use serde_json::json;

#[tokio::test]
async fn p0_002b_gitignore_privacy_through_public_search_tool() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".gitignore"), "private.env\n").unwrap();
    std::fs::write(temp.path().join("public.txt"), "privacy_witness\n").unwrap();
    std::fs::write(temp.path().join("private.env"), "privacy_witness\n").unwrap();
    let ctx = ToolContext {
        working_dir: temp.path().to_path_buf(),
        blocked_paths: vec![],
        max_file_scope: None,
        non_interactive: true,
    };
    let result = SearchTool
        .execute(json!({"pattern":"privacy_witness"}), &ctx)
        .await;
    assert!(result.success, "{}", result.output);
    assert!(
        result.output.contains("public.txt:1:privacy_witness"),
        "{}",
        result.output
    );
    assert!(!result.output.contains("private.env"), "{}", result.output);
    assert_eq!(
        std::fs::read(temp.path().join("private.env")).unwrap(),
        b"privacy_witness\n"
    );
}
