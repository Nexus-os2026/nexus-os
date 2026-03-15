use nexus_cli::{execute_agent_command, AgentCommand};

#[test]
fn test_cli_start_coding_agent_dry_run() {
    // Use a unique temp directory to avoid permission conflicts with stale
    // directories owned by other users (e.g. gitlab-runner).
    let tmp =
        std::env::temp_dir().join(format!("nexus-self-improve-test-{}", uuid::Uuid::new_v4()));
    std::env::set_var("NEXUS_SELF_IMPROVE_DIR", &tmp);

    let output = execute_agent_command(AgentCommand::Start {
        agent_id: "coding-agent".to_string(),
        dry_run: true,
    })
    .expect("coding-agent dry-run should complete");

    assert!(output.contains("Agent 'coding-agent' completed"));
    assert!(output.contains("dry_run=true"));
    assert!(output.contains("iterations="));

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("NEXUS_SELF_IMPROVE_DIR");
}
