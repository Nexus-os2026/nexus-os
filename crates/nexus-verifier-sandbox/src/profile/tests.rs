use super::*;

const NAME: &str = "rust.cargo-test.offline.v1";

#[test]
fn p2b_exactly_one_production_profile_is_advertised() {
    assert_eq!(
        VerifierProfileId::PRODUCTION,
        &[VerifierProfileId::RustCargoTestOfflineV1]
    );
    assert_eq!(VerifierProfileId::RustCargoTestOfflineV1.name(), NAME);
}

#[test]
fn p2b_a_profile_name_only_selects_an_exact_compiled_in_profile() {
    assert_eq!(
        VerifierProfileId::lookup(NAME),
        Some(VerifierProfileId::RustCargoTestOfflineV1)
    );
    for other in [
        "",
        " rust.cargo-test.offline.v1",
        "rust.cargo-test.offline.v1 ",
        "RUST.cargo-test.offline.v1",
        "rust.cargo-test.offline.v2",
        "rust.cargo-test.offline",
        "rust.cargo-test.offline.v1\0",
        "sh",
        "/bin/sh",
        "shell",
    ] {
        assert_eq!(VerifierProfileId::lookup(other), None, "{other:?}");
    }
}

#[test]
fn p2b_the_offline_rust_profile_is_the_frozen_invocation() {
    let profile = VerifierProfileId::RustCargoTestOfflineV1.profile();
    assert_eq!(profile.executable, "bin/cargo");
    assert_eq!(profile.argv0, "cargo");
    assert_eq!(
        profile.args,
        &[
            "test",
            "--offline",
            "--locked",
            "--no-fail-fast",
            "--lib",
            "--tests"
        ]
    );
    assert_eq!(profile.cwd, Location::Input);
    assert_eq!(profile.passing_exit_code, 0);
    assert_eq!(profile.resources, ResourcePolicy::RUST_OFFLINE_V1);
    assert_eq!(profile.toolchain.version, "1.94.0");
    assert_eq!(profile.toolchain.target, "x86_64-unknown-linux-musl");
}

#[test]
fn p2b_the_profile_has_no_shell_path_or_network_escape() {
    let profile = VerifierProfileId::RustCargoTestOfflineV1.profile();
    let keys: Vec<&str> = profile.env.iter().map(|var| var.key).collect();
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "no duplicate variable");
    for forbidden in [
        "PATH",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "http_proxy",
        "https_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "SSH_AUTH_SOCK",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        assert!(!keys.contains(&forbidden), "{forbidden} must not be set");
    }
    let literal = |key: &str| {
        profile
            .env
            .iter()
            .find(|var| var.key == key)
            .map(|var| var.value)
    };
    assert_eq!(literal("CARGO_NET_OFFLINE"), Some(Value::Literal("true")));
    for arg in profile.args.iter().chain([&profile.argv0]) {
        for shell in ["sh", "bash", "-c", "--config", "-Z"] {
            assert_ne!(*arg, shell, "no shell or override argument");
        }
    }
    assert!(
        !profile.executable.starts_with('/'),
        "toolchain-relative only"
    );
    assert!(!profile.executable.contains(".."));
}

#[test]
fn p2b_every_path_value_names_a_backend_location_without_escape() {
    let profile = VerifierProfileId::RustCargoTestOfflineV1.profile();
    for var in profile.env {
        if let Value::Path(_, relative) = var.value {
            assert!(!relative.starts_with('/'), "{}", var.key);
            assert!(
                relative.split('/').all(|part| part != ".." && part != "."),
                "{}",
                var.key
            );
        }
    }
    let paths: Vec<Location> = profile
        .env
        .iter()
        .filter_map(|var| match var.value {
            Value::Path(location, _) => Some(location),
            Value::Literal(_) => None,
        })
        .collect();
    assert!(
        !paths.contains(&Location::Input),
        "no writable use of input"
    );
}

#[test]
fn p2b_profile_hash_is_deterministic_and_covers_every_field() {
    let base = RUST_CARGO_TEST_OFFLINE_V1;
    assert_eq!(base.hash(), RUST_CARGO_TEST_OFFLINE_V1.hash());
    let variants = [
        VerifierProfile {
            name: "rust.cargo-test.offline.v2",
            ..base
        },
        VerifierProfile { version: 2, ..base },
        VerifierProfile {
            display_name: "x",
            ..base
        },
        VerifierProfile {
            toolchain: ToolchainRequirement {
                version: "1.95.0",
                ..base.toolchain
            },
            ..base
        },
        VerifierProfile {
            toolchain: ToolchainRequirement {
                target: "x86_64-unknown-linux-gnu",
                ..base.toolchain
            },
            ..base
        },
        VerifierProfile {
            executable: "bin/rustc",
            ..base
        },
        VerifierProfile { argv0: "x", ..base },
        VerifierProfile {
            args: &["test"],
            ..base
        },
        VerifierProfile {
            cwd: Location::Target,
            ..base
        },
        VerifierProfile {
            env: &base.env[1..],
            ..base
        },
        VerifierProfile {
            resources: ResourcePolicy {
                pids_max: 1,
                ..base.resources
            },
            ..base
        },
        VerifierProfile {
            passing_exit_code: 1,
            ..base
        },
    ];
    for variant in variants {
        assert_ne!(variant.hash(), base.hash(), "{variant:?}");
    }
    // A value moved between literal and path forms changes the identity.
    static LITERAL: [EnvVar; 1] = [EnvVar {
        key: "HOME",
        value: Value::Literal(""),
    }];
    static PATH: [EnvVar; 1] = [EnvVar {
        key: "HOME",
        value: Value::Path(Location::Home, ""),
    }];
    assert_ne!(
        VerifierProfile {
            env: &LITERAL,
            ..base
        }
        .hash(),
        VerifierProfile { env: &PATH, ..base }.hash()
    );
}
