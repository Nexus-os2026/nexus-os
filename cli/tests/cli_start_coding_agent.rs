use nexus_cli::{execute_agent_command, AgentCommand};

#[test]
fn test_cli_start_coding_agent_dry_run() {
    let output = execute_agent_command(AgentCommand::Start {
        agent_id: "coding-agent".to_string(),
        dry_run: true,
    })
    .expect("coding-agent dry-run should complete");

    assert!(output.contains("Agent 'coding-agent' completed"));
    assert!(output.contains("dry_run=true"));
    assert!(output.contains("iterations="));
}
