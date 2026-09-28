//! P0-002C5B: an interface configuration update has a security baseline only
//! when a current configuration already exists and loads. Only a load of a
//! missing configuration creates the first-run default; an empty file is
//! refused, never replaced.
//!
//! This binary holds a single test because it points the process-wide
//! configuration location (`NEXUS_CONFIG_PATH`) at an isolated temporary file.

use nexus_kernel::config::{load_config, load_current_security_baseline};

#[test]
fn interface_baselines_come_only_from_an_existing_loadable_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nexus").join("config.toml");
    std::env::set_var("NEXUS_CONFIG_PATH", &path);

    // Missing: no baseline, and resolving the baseline creates nothing.
    assert!(load_current_security_baseline().is_err());
    assert!(!path.exists());
    assert!(!path.parent().unwrap().exists());

    // Empty or whitespace-only (a truncated file): no baseline, and the file
    // is left exactly as it was.
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for content in ["", "  \n\t\n"] {
        std::fs::write(&path, content).unwrap();
        assert!(load_current_security_baseline().is_err(), "{content:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    // Backend bootstrap still initialises the first run, and only then does
    // an interface update find a baseline: the configuration startup uses.
    std::fs::remove_file(&path).unwrap();
    let bootstrapped = load_config().unwrap();
    assert!(path.exists());
    assert_eq!(
        load_current_security_baseline().unwrap(),
        bootstrapped.security
    );
}
