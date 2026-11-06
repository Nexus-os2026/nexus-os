//! K04 oracle registered before extraction. Only imports may follow the move.
//! Defaults, field order, serde names, Debug and error text are compatibility
//! contracts; K01's fixtures and the existing config/vault tests stay unchanged.
use nexus_kernel::audit::AuditError;
use nexus_kernel::backup::BackupScheduleConfig;
use nexus_kernel::config::{
    is_config_envelope, load_config_from_path_with, save_config_checked_to_path, ConfigKeyMaterial,
    NexusConfig,
};
use nexus_kernel::errors::{on_error, AgentError, ErrorStrategy};
use nexus_kernel::fuel_hardening::FuelViolation;
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::rate_limit::{ApiHardeningConfig, RateLimitConfig};
use serde::de::value::{Error as ValueError, U32Deserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::any::TypeId;
use std::path::PathBuf;

fn golden(name: &str) -> String {
    let rows: Vec<Value> = include_str!("retirement/bytes.jsonl")
        .lines()
        .map(|row| serde_json::from_str(row).unwrap())
        .collect();
    rows.iter().find(|row| row["name"] == name).unwrap()["preimage"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn assert_bytes<T>(value: &T, expected: &str)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(serde_json::to_vec(value).unwrap(), expected.as_bytes());
    assert_eq!(&serde_json::from_str::<T>(expected).unwrap(), value);
}

fn rate() -> RateLimitConfig {
    RateLimitConfig {
        enabled: false,
        default_rpm: 1,
        llm_rpm: 2,
        agent_execute_rpm: 3,
        audit_export_rpm: 4,
        backup_create_rpm: 5,
        admin_rpm: 6,
        burst_size: 7,
    }
}

fn api() -> ApiHardeningConfig {
    ApiHardeningConfig {
        max_request_body_bytes: 11,
        max_string_length: 12,
        json_max_depth: 13,
    }
}

fn backup() -> BackupScheduleConfig {
    BackupScheduleConfig {
        enabled: true,
        schedule: "*/7 * * * *".into(),
        output_dir: "relative/backup-α".into(),
        retention_count: 9,
        encrypt: true,
        compression: "fixture".into(),
        include_audit: false,
        include_genomes: false,
    }
}

fn check_missing_fields<T>(custom: &T, defaults: &str)
where
    T: Serialize + DeserializeOwned,
{
    let custom = serde_json::to_value(custom).unwrap();
    let defaults: Value = serde_json::from_str(defaults).unwrap();
    for (field, default) in defaults.as_object().unwrap() {
        let mut input = custom.clone();
        input.as_object_mut().unwrap().remove(field);
        let parsed: T = serde_json::from_value(input).unwrap();
        let mut expected = custom.clone();
        expected[field] = default.clone();
        assert_eq!(serde_json::to_value(parsed).unwrap(), expected, "{field}");
    }
    assert_eq!(
        serde_json::to_value(serde_json::from_str::<T>("{}").unwrap()).unwrap(),
        defaults
    );
    let mut unknown = custom.clone();
    unknown["future_field"] = json!({"unknown": [null, 17]});
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<T>(unknown).unwrap()).unwrap(),
        custom
    );
}

#[test]
fn rate_and_api_defaults_and_every_missing_field_remain_exact() {
    assert_bytes(&RateLimitConfig::default(), &golden("rate"));
    assert_bytes(&ApiHardeningConfig::default(), &golden("api"));
    check_missing_fields(&rate(), &golden("rate"));
    check_missing_fields(&api(), &golden("api"));
}

#[test]
fn schemas_keep_independent_nondefault_json_toml_and_debug_bytes() {
    assert_bytes(
        &rate(),
        r#"{"enabled":false,"default_rpm":1,"llm_rpm":2,"agent_execute_rpm":3,"audit_export_rpm":4,"backup_create_rpm":5,"admin_rpm":6,"burst_size":7}"#,
    );
    assert_bytes(
        &api(),
        r#"{"max_request_body_bytes":11,"max_string_length":12,"json_max_depth":13}"#,
    );
    assert_bytes(
        &backup(),
        r#"{"enabled":true,"schedule":"*/7 * * * *","output_dir":"relative/backup-α","retention_count":9,"encrypt":true,"compression":"fixture","include_audit":false,"include_genomes":false}"#,
    );
    for (actual, expected) in [
        (toml::to_string(&rate()).unwrap(), "enabled = false\ndefault_rpm = 1\nllm_rpm = 2\nagent_execute_rpm = 3\naudit_export_rpm = 4\nbackup_create_rpm = 5\nadmin_rpm = 6\nburst_size = 7\n"),
        (toml::to_string(&api()).unwrap(), "max_request_body_bytes = 11\nmax_string_length = 12\njson_max_depth = 13\n"),
        (toml::to_string(&backup()).unwrap(), "enabled = true\nschedule = \"*/7 * * * *\"\noutput_dir = \"relative/backup-α\"\nretention_count = 9\nencrypt = true\ncompression = \"fixture\"\ninclude_audit = false\ninclude_genomes = false\n"),
        (format!("{:?}", rate()), "RateLimitConfig { enabled: false, default_rpm: 1, llm_rpm: 2, agent_execute_rpm: 3, audit_export_rpm: 4, backup_create_rpm: 5, admin_rpm: 6, burst_size: 7 }"),
        (format!("{:?}", api()), "ApiHardeningConfig { max_request_body_bytes: 11, max_string_length: 12, json_max_depth: 13 }"),
        (format!("{:?}", backup()), "BackupScheduleConfig { enabled: true, schedule: \"*/7 * * * *\", output_dir: \"relative/backup-α\", retention_count: 9, encrypt: true, compression: \"fixture\", include_audit: false, include_genomes: false }"),
    ] {
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }
    assert_eq!(
        toml::from_str::<RateLimitConfig>(&toml::to_string(&rate()).unwrap()).unwrap(),
        rate()
    );
    assert_eq!(
        toml::from_str::<ApiHardeningConfig>(&toml::to_string(&api()).unwrap()).unwrap(),
        api()
    );
    assert_eq!(
        toml::from_str::<BackupScheduleConfig>(&toml::to_string(&backup()).unwrap()).unwrap(),
        backup()
    );
    let mut escaped = backup();
    escaped.schedule = "α\n\"\\".into();
    assert_eq!(
        serde_json::to_value(&escaped).unwrap()["schedule"],
        "α\n\"\\"
    );
    assert!(serde_json::to_string(&escaped)
        .unwrap()
        .contains(r#""schedule":"α\n\"\\""#));
}

#[test]
fn explicit_zero_false_empty_and_numeric_boundaries_are_not_defaulted() {
    let mut b = serde_json::to_value(backup()).unwrap();
    for field in ["enabled", "encrypt", "include_audit", "include_genomes"] {
        b[field] = json!(false);
    }
    for field in ["schedule", "output_dir", "compression"] {
        b[field] = json!("");
    }
    b["retention_count"] = json!(0);
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<BackupScheduleConfig>(b.clone()).unwrap())
            .unwrap(),
        b
    );
    let mut r = serde_json::to_value(rate()).unwrap();
    for value in r.as_object_mut().unwrap().values_mut() {
        if value.is_number() {
            *value = json!(0);
        }
    }
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<RateLimitConfig>(r.clone()).unwrap())
            .unwrap(),
        r
    );
    let a = json!({"max_request_body_bytes": 0, "max_string_length": 0, "json_max_depth": 0});
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<ApiHardeningConfig>(a.clone()).unwrap())
            .unwrap(),
        a
    );
    for (custom, integer, boolean) in [
        (
            serde_json::to_value(backup()).unwrap(),
            "retention_count",
            Some("encrypt"),
        ),
        (
            serde_json::to_value(rate()).unwrap(),
            "default_rpm",
            Some("enabled"),
        ),
        (serde_json::to_value(api()).unwrap(), "json_max_depth", None),
    ] {
        let parse = |value: Value| {
            if integer == "retention_count" {
                serde_json::from_value::<BackupScheduleConfig>(value).map(|_| ())
            } else if integer == "default_rpm" {
                serde_json::from_value::<RateLimitConfig>(value).map(|_| ())
            } else {
                serde_json::from_value::<ApiHardeningConfig>(value).map(|_| ())
            }
        };
        for bad in [json!(null), json!(-1), json!(1.5), json!("2")] {
            let mut input = custom.clone();
            input[integer] = bad;
            assert!(parse(input).is_err());
        }
        if let Some(field) = boolean {
            for bad in [json!(null), json!(0), json!("false")] {
                let mut input = custom.clone();
                input[field] = bad;
                assert!(parse(input).is_err());
            }
        }
    }
    assert!(serde_json::from_str::<RateLimitConfig>(r#"{"default_rpm":4294967296}"#).is_err());
    assert!(
        serde_json::from_str::<BackupScheduleConfig>(r#"{"retention_count":4294967296}"#).is_err()
    );
    assert_eq!(
        serde_json::from_str::<RateLimitConfig>(r#"{"default_rpm":4294967295}"#)
            .unwrap()
            .default_rpm,
        u32::MAX
    );
    assert_eq!(
        serde_json::from_str::<BackupScheduleConfig>(r#"{"retention_count":4294967295}"#)
            .unwrap()
            .retention_count,
        u32::MAX
    );
}

#[test]
fn backup_defaults_child() {
    let Ok(case) = std::env::var("K04_BACKUP_HOME_CASE") else {
        return;
    };
    let dir = if case == "valid" {
        PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/share/nexus-os/backups")
    } else {
        PathBuf::new()
    };
    let expected = golden("backup").replace(
        "\"<HOME>/.local/share/nexus-os/backups\"",
        &serde_json::to_string(&dir.to_string_lossy()).unwrap(),
    );
    assert_bytes(&BackupScheduleConfig::default(), &expected);
    check_missing_fields(&backup(), &expected);
    // The retained archive config uses the same lossless PathBuf default.
    assert_eq!(
        nexus_kernel::backup::BackupConfig::default().output_dir,
        dir
    );
}

#[test]
fn backup_defaults_validate_home_in_isolated_children() {
    let home = tempfile::tempdir().unwrap();
    let mut cases = vec![
        ("valid", Some(home.path().as_os_str().to_owned())),
        ("empty", Some("".into())),
        ("relative", Some("relative".into())),
    ];
    // Windows deliberately permits the native profile only when HOME is absent.
    #[cfg(not(windows))]
    cases.push(("absent", None));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut raw = home.path().as_os_str().as_encoded_bytes().to_vec();
        raw.extend_from_slice(b"/non-utf8-\xff");
        cases.push(("valid", Some(std::ffi::OsString::from_vec(raw))));
    }
    for (case, home) in cases {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "backup_defaults_child", "--nocapture"])
            .env("K04_BACKUP_HOME_CASE", case);
        if let Some(home) = home {
            command.env("HOME", home);
        } else {
            command.env_remove("HOME");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}

#[test]
fn states_and_violations_keep_names_variant_indices_and_error_text() {
    use AgentState::*;
    let states = [
        (Created, "Created"),
        (Starting, "Starting"),
        (Running, "Running"),
        (Paused, "Paused"),
        (Stopping, "Stopping"),
        (Stopped, "Stopped"),
        (Destroyed, "Destroyed"),
    ];
    for (index, (state, name)) in states.iter().enumerate() {
        assert_bytes(state, &format!("\"{name}\""));
        assert_eq!(format!("{state:?}"), *name);
        assert_eq!(state.to_string(), *name);
        let de: U32Deserializer<ValueError> = (index as u32).into_deserializer();
        assert_eq!(AgentState::deserialize(de).unwrap(), *state);
        for (to, to_name) in &states {
            let error = AgentError::InvalidTransition {
                from: *state,
                to: *to,
            };
            assert_eq!(
                error.to_string(),
                format!("invalid state transition from '{name}' to '{to_name}'")
            );
            assert_eq!(on_error(&error), ErrorStrategy::Skip);
        }
    }
    for (index, (violation, name)) in [
        (FuelViolation::OverMonthlyCap, "OverMonthlyCap"),
        (FuelViolation::AnomalyDetected, "AnomalyDetected"),
        (
            FuelViolation::ProviderTokenMismatch,
            "ProviderTokenMismatch",
        ),
    ]
    .iter()
    .enumerate()
    {
        assert_bytes(violation, &format!("\"{name}\""));
        assert_eq!(format!("{violation:?}"), *name);
        let de: U32Deserializer<ValueError> = (index as u32).into_deserializer();
        assert_eq!(FuelViolation::deserialize(de).unwrap(), *violation);
        let error = AgentError::FuelViolation {
            violation: violation.clone(),
            reason: "fixture-α\nreason".into(),
        };
        assert_eq!(
            error.to_string(),
            format!("fuel violation '{name}': fixture-α\nreason")
        );
        assert_eq!(on_error(&error), ErrorStrategy::Escalate);
    }
    let state_end: U32Deserializer<ValueError> = 7u32.into_deserializer();
    let violation_end: U32Deserializer<ValueError> = 3u32.into_deserializer();
    assert!(AgentState::deserialize(state_end).is_err());
    assert!(FuelViolation::deserialize(violation_end).is_err());
    for bad in [
        "null",
        "0",
        "\"created\"",
        "\"OverMonthlyCap \"",
        "\"Unknown\"",
    ] {
        assert!(serde_json::from_str::<AgentState>(bad).is_err());
        assert!(serde_json::from_str::<FuelViolation>(bad).is_err());
    }
}

#[test]
fn every_agent_error_keeps_exact_display_and_strategy() {
    use ErrorStrategy::*;
    for (error, text, strategy) in [
        (AgentError::FuelExhausted, "fuel budget exhausted", Escalate),
        (AgentError::ManifestError("fixture".into()), "manifest error: fixture", Skip),
        (AgentError::CapabilityDenied("fixture".into()), "capability denied: 'fixture' is not allowed", Escalate),
        (AgentError::ApprovalRequired { request_id: "fixture".into() }, "approval required: request_id='fixture'", Escalate),
        (AgentError::SupervisorError("fixture".into()), "supervisor error: fixture", Retry { max_attempts: 3 }),
        (AgentError::KeyDestroyed("fixture".into()), "key 'fixture' has been destroyed", Escalate),
        (AgentError::AdversarialBlock("fixture".into()), "adversarial challenge failed: fixture", Escalate),
        (AgentError::AuditFailure(AuditError::BatcherPoisoned), "audit failure: audit batcher mutex poisoned — audit integrity compromised", Escalate),
        (AgentError::AuditFailure(AuditError::SerializationFailed), "audit failure: audit event serialization failed — fail-closed, no hash without payload", Escalate),
    ] {
        assert_eq!(error.to_string().as_bytes(), text.as_bytes());
        assert_eq!(on_error(&error), strategy);
    }
}

#[test]
fn transitional_paths_are_the_same_types_until_their_registered_retirement() {
    // Retire fuel/lifecycle checks at K16, backup at K20 and rate_limit at K21.
    // K22 still requires no old-path imports or exports.
    assert_eq!(
        TypeId::of::<BackupScheduleConfig>(),
        TypeId::of::<nexus_kernel::backup::BackupScheduleConfig>()
    );
    assert_eq!(
        TypeId::of::<RateLimitConfig>(),
        TypeId::of::<nexus_kernel::rate_limit::RateLimitConfig>()
    );
    assert_eq!(
        TypeId::of::<ApiHardeningConfig>(),
        TypeId::of::<nexus_kernel::rate_limit::ApiHardeningConfig>()
    );
    assert_eq!(
        TypeId::of::<AgentState>(),
        TypeId::of::<nexus_kernel::lifecycle::AgentState>()
    );
    assert_eq!(
        TypeId::of::<FuelViolation>(),
        TypeId::of::<nexus_kernel::fuel_hardening::FuelViolation>()
    );
}

#[test]
fn encrypted_config_roundtrip_preserves_schemas_and_load_does_not_migrate() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let config = NexusConfig {
        backup: backup(),
        rate_limiting: rate(),
        api: api(),
        ..NexusConfig::default()
    };
    let plaintext = toml::to_string(&config).unwrap();
    std::fs::write(&path, &plaintext).unwrap();
    let keys = ConfigKeyMaterial::from_values(Some("K04 synthetic test material"), [None; 4]);
    assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), config);
    assert_eq!(std::fs::read(&path).unwrap(), plaintext.as_bytes());
    save_config_checked_to_path(&path, &config, &keys).unwrap();
    let encrypted = std::fs::read_to_string(&path).unwrap();
    assert!(is_config_envelope(&encrypted));
    assert!(!encrypted.contains("relative/backup-α"));
    assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), config);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), encrypted);
    let wrong = ConfigKeyMaterial::from_values(Some("K04 different test material"), [None; 4]);
    assert!(load_config_from_path_with(&path, &wrong).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), encrypted);
}
