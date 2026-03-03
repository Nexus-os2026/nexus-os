use nexus_cli::{execute_self_improve_command, SelfImproveCommand};

#[test]
fn test_cli_self_improve_run() {
    let output = execute_self_improve_command(SelfImproveCommand::Run {
        agent: "coding-agent".to_string(),
    })
    .expect("self-improve run should succeed");

    assert!(output.contains("Self-improve run complete for 'coding-agent'"));
    assert!(output.contains("version="));
}
