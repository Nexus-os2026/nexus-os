//! P0-002C5A: the Nexus OS desktop configures, diagnoses and builds Nexus Code
//! without running, preferring or registering the external Claude CLI agent.
//!
//! This binary holds a single test because it puts a fake `claude` first on
//! `PATH` for the whole process.

use nexus_code::app::App;
use nexus_code::config::NxConfig;
use nexus_code::llm::ModelSlot;
use nexus_code::setup;

fn claude_cli_config() -> NxConfig {
    NxConfig {
        default_provider: "claude_cli".to_string(),
        default_model: "claude-cli".to_string(),
        ..NxConfig::default()
    }
}

/// Installs a fake `claude` that records each run, and returns the directory
/// guard and the record file.
#[cfg(unix)]
fn install_fake_claude() -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let runs = dir.path().join("runs");
    let fake = dir.path().join("claude");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\necho '2.1.0 (Claude Code)'\n",
            runs.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![dir.path().to_path_buf()];
    dirs.extend(std::env::split_paths(&path));
    std::env::set_var("PATH", std::env::join_paths(dirs).unwrap());
    (dir, runs)
}

#[test]
fn desktop_entry_points_never_run_prefer_or_register_the_claude_cli() {
    #[cfg(unix)]
    let (_dir, runs) = install_fake_claude();

    let status = setup::diagnose_without_cli_agents();
    assert!(!status
        .configured_providers
        .iter()
        .any(|p| p == "claude_cli"));
    assert!(status
        .unconfigured_providers
        .iter()
        .any(|(name, reason)| name == "claude_cli" && reason == setup::CLI_AGENT_UNAVAILABLE));

    // Auto-detection prefers the CLI only after running it, so no recorded
    // run below also means it was never preferred.
    NxConfig::load_without_cli_agents().unwrap();

    let app = App::new_without_cli_agents(claude_cli_config()).unwrap();
    let error = app
        .router
        .resolve(ModelSlot::Execution)
        .err()
        .expect("the Claude CLI provider must not be registered");
    assert!(error.to_string().contains("not registered"), "{error}");

    #[cfg(unix)]
    {
        assert_eq!(
            std::fs::read_to_string(&runs).unwrap_or_default(),
            "",
            "a desktop entry point ran the Claude CLI"
        );

        // Positive control: the probing forms do run the fake CLI, so the
        // empty record above is meaningful.
        assert!(setup::diagnose()
            .configured_providers
            .iter()
            .any(|p| p == "claude_cli"));
        assert!(std::fs::read_to_string(&runs)
            .unwrap()
            .contains("--version"));
        assert!(App::new(claude_cli_config())
            .unwrap()
            .router
            .resolve(ModelSlot::Execution)
            .is_ok());
    }
}
