//! Final Gate item A: the public configuration functions take their key
//! material from the launch environment. A new or changed credential is
//! written only under a non-empty `NEXUS_CONFIG_KEY`.
//!
//! This binary holds a single test because it sets the configuration location
//! and the key material (`NEXUS_CONFIG_KEY` and the legacy ambient inputs HOME,
//! USER, USERNAME and HOSTNAME) for the whole process: always synthetic values
//! and a temporary file, never the user's configuration.

use nexus_kernel::config::{
    load_config, load_current_security_baseline, save_config, save_config_checked, ConfigSaveError,
    ConfigWriteRefusal, NexusConfig, SaveOutcome,
};

#[test]
fn launch_environment_key_material_governs_credential_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nexus").join("config.toml");
    std::env::set_var("NEXUS_CONFIG_PATH", &path);
    std::env::set_var("HOME", "/home/synthetic-nexus");
    std::env::set_var("USER", "synthetic-user");
    std::env::remove_var("USERNAME");
    std::env::set_var("HOSTNAME", "synthetic-host");
    std::env::remove_var("NEXUS_CONFIG_KEY");

    // First run: the default configuration, which holds no credential.
    let default_config = load_config().unwrap();
    assert_eq!(default_config, NexusConfig::default());
    assert!(path.exists());

    let mut with_credential = default_config.clone();
    with_credential.search.brave_api_key = "synthetic-brave".into();
    let refused = Err(ConfigSaveError::Refused(
        ConfigWriteRefusal::OperatorKeyRequired,
    ));

    // No operator key, an empty one or a whitespace-only one: refused with a
    // bounded reason, and the file is left as it was.
    let before = std::fs::read(&path).unwrap();
    for value in [None, Some(""), Some("  \t")] {
        match value {
            Some(value) => std::env::set_var("NEXUS_CONFIG_KEY", value),
            None => std::env::remove_var("NEXUS_CONFIG_KEY"),
        }
        assert_eq!(save_config_checked(&with_credential), refused, "{value:?}");
        let error = save_config(&with_credential).unwrap_err().to_string();
        assert!(error.contains("NEXUS_CONFIG_KEY"), "{error}");
        assert!(!error.contains("synthetic-brave"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    // A value that is not UTF-8 is no key material either.
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::env::set_var(
            "NEXUS_CONFIG_KEY",
            std::ffi::OsString::from_vec(vec![b'k', 0xff, b'y']),
        );
        assert_eq!(save_config_checked(&with_credential), refused);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    // A save without a credential is written under the legacy key.
    std::env::remove_var("NEXUS_CONFIG_KEY");
    let mut settings = default_config.clone();
    settings.llm.default_model = "synthetic-model".into();
    assert_eq!(save_config_checked(&settings), Ok(SaveOutcome::Written));

    // With the operator key the credential is written, and the ambient file
    // moves to the operator key.
    std::env::set_var("NEXUS_CONFIG_KEY", "synthetic-operator-key");
    with_credential.llm.default_model = "synthetic-model".into();
    assert_eq!(
        save_config_checked(&with_credential),
        Ok(SaveOutcome::RekeyedToOperatorKey)
    );
    assert_eq!(load_config().unwrap(), with_credential);
    assert!(load_current_security_baseline().is_ok());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("synthetic-brave"));

    // Without it the configuration no longer opens, and nothing overwrites it.
    std::env::remove_var("NEXUS_CONFIG_KEY");
    assert!(load_config().is_err());
    assert!(load_current_security_baseline().is_err());
    assert!(save_config(&settings).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    std::env::remove_var("NEXUS_CONFIG_PATH");
}
