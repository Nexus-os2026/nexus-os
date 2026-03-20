use nexus_cli::{execute_agent_command, AgentCommand};

#[test]
fn test_cli_start_coding_agent_dry_run() {
    // Use a unique temp directory to avoid permission conflicts with stale
    // directories owned by other users (e.g. gitlab-runner).
    let tmp =
        std::env::temp_dir().join(format!("nexus-self-improve-test-{}", uuid::Uuid::new_v4()));
    std::env::set_var("NEXUS_SELF_IMPROVE_DIR", &tmp);

    let result = execute_agent_command(AgentCommand::Start {
        agent_id: "coding-agent".to_string(),
        dry_run: true,
    });

    // Clean up early so we don't leak the temp dir on any exit path.
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("NEXUS_SELF_IMPROVE_DIR");

    match result {
        Ok(output) => {
            assert!(output.contains("Agent 'coding-agent' completed"));
            assert!(output.contains("dry_run=true"));
            assert!(output.contains("iterations="));
        }
        Err(e)
            if e.contains("ollama")
                || e.contains("provider")
                || e.contains("LLM")
                || e.contains("404") =>
        {
            // Ollama not running, model not pulled, or provider unavailable —
            // this is an environment issue, not a code bug.
            eprintln!(
                "SKIPPED: coding-agent dry-run requires a working LLM provider. Error: {e}\n\
                 To run this test: ollama pull llama3.2"
            );
        }
        Err(e) => {
            panic!("coding-agent dry-run failed with unexpected error: {e}");
        }
    }
}
