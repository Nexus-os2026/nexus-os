use nexus_cli::{execute_self_improve_command, SelfImproveCommand};

#[test]
fn test_cli_self_improve_run() {
    // Use a unique temp directory to avoid permission conflicts with stale
    // directories owned by other users (e.g. gitlab-runner).
    let tmp =
        std::env::temp_dir().join(format!("nexus-self-improve-test-{}", uuid::Uuid::new_v4()));
    std::env::set_var("NEXUS_SELF_IMPROVE_DIR", &tmp);

    let result = execute_self_improve_command(SelfImproveCommand::Run {
        agent: "coding-agent".to_string(),
    });

    // Clean up early so we don't leak the temp dir on any exit path.
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("NEXUS_SELF_IMPROVE_DIR");

    match result {
        Ok(output) => {
            assert!(output.contains("Self-improve run complete for 'coding-agent'"));
            assert!(output.contains("version="));
        }
        Err(e) if e.contains("ollama") || e.contains("provider") || e.contains("LLM") => {
            // Ollama not running, model not pulled, or provider unavailable —
            // this is an environment issue, not a code bug.
            eprintln!(
                "SKIPPED: self-improve requires a working LLM provider. Error: {e}\n\
                 To run this test: ollama pull llama3"
            );
        }
        Err(e) => {
            panic!("self-improve run failed with unexpected error: {e}");
        }
    }
}
