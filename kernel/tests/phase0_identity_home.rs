//! P0-002C5B: per-user Nexus state has a location only under a validated
//! identity home. With none, nothing falls back to the working directory, a
//! shared temporary directory or a literal `~`.
//!
//! This binary holds a single test because it changes HOME, the operator
//! overrides and the working directory for the whole process.

use nexus_kernel::backup::{create_backup, BackupConfig};
use nexus_kernel::config::{config_path, load_config};
use nexus_kernel::identity_home::{identity_home, nexus_db_path, nexus_state_dir};
use nexus_kernel::policy_engine::PolicyEngine;

#[test]
fn invalid_identity_homes_leave_nexus_state_without_a_location() {
    let cwd = tempfile::tempdir().unwrap();
    std::env::set_current_dir(cwd.path()).unwrap();
    std::env::remove_var("NEXUS_DB_PATH");
    std::env::remove_var("NEXUS_CONFIG_PATH");

    for home in ["", ".", "relative/home", "~", "~/nexus"] {
        std::env::set_var("HOME", home);
        assert!(identity_home().is_err(), "{home:?}");
        assert!(nexus_state_dir().is_err(), "{home:?}");
        assert!(nexus_db_path().is_err(), "{home:?}");
        assert!(config_path().is_err(), "{home:?}");
        assert!(load_config().is_err(), "{home:?}");

        let backup = BackupConfig::default();
        assert!(backup.output_dir.as_os_str().is_empty(), "{home:?}");
        assert!(
            create_backup(&backup, cwd.path(), None).is_err(),
            "{home:?}"
        );

        let mut policies = PolicyEngine::default();
        assert_eq!(policies.load_policies().unwrap(), 0, "{home:?}");
    }
    // Nothing was created relative to the working directory.
    assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 0);

    // A valid home yields deterministic locations beneath it.
    let home = tempfile::tempdir().unwrap();
    let home_path = home.path().to_path_buf();
    std::env::set_var("HOME", &home_path);
    let nexus = home_path.join(".nexus");
    assert_eq!(identity_home().unwrap(), home_path);
    assert_eq!(nexus_state_dir().unwrap(), nexus);
    assert_eq!(nexus_db_path().unwrap(), nexus.join("nexus.db"));
    assert_eq!(config_path().unwrap(), nexus.join("config.toml"));
    assert_eq!(
        BackupConfig::default().output_dir,
        home_path
            .join(".local")
            .join("share")
            .join("nexus-os")
            .join("backups")
    );
}
