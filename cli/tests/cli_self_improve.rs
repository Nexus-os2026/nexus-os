use nexus_cli::{execute_self_improve_command, SelfImproveCommand};

#[test]
fn test_cli_self_improve_run() {
    // Use a unique temp directory to avoid permission conflicts with stale
    // directories owned by other users (e.g. gitlab-runner).
    let tmp =
        std::env::temp_dir().join(format!("nexus-self-improve-test-{}", uuid::Uuid::new_v4()));
    std::env::set_var("NEXUS_SELF_IMPROVE_DIR", &tmp);

    let output = execute_self_improve_command(SelfImproveCommand::Run {
        agent: "coding-agent".to_string(),
    })
    .expect("self-improve run should succeed");

    assert!(output.contains("Self-improve run complete for 'coding-agent'"));
    assert!(output.contains("version="));

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("NEXUS_SELF_IMPROVE_DIR");
}
