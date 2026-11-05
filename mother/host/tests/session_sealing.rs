//! Sealed route guards (§4/A1/A4/A10). These are structural security
//! invariants; behavioral native/PTY controls are registered separately.
#![deny(unsafe_code)]
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn code(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn signatures(source: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ") {
            assert!(current.is_none(), "unterminated public signature");
            current = Some(String::new());
        }
        if let Some(sig) = &mut current {
            sig.push_str(trimmed);
            if let Some(end) = sig.find('{') {
                sig.truncate(end);
                result.push(current.take().unwrap());
            }
        }
    }
    assert!(current.is_none());
    result
}

#[test]
fn session_hands_and_stop_call_graph_cannot_reach_raw_legacy_authority() {
    let mut violations = Vec::new();
    for file in ["hands.rs", "stop.rs", "terminal.rs"] {
        let path = root().join("mother/host/src").join(file);
        let text = code(&path);
        for forbidden in [
            "GovernedControl",
            "AgentDoor",
            "GovernedPort",
            "AdmissionGate",
            "GatedLane",
            "Home::",
            ".owner()",
            "door_with_halt",
            "_with_halt(",
            "Intent::",
            "RecoveryCheck",
            "struct Gate",
            ".lift_after_confirmation(",
        ] {
            if text.contains(forbidden) {
                violations.push(format!("{file}: {forbidden}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "unsealed Hands authority route: {}",
        violations.join("; ")
    );
}

#[test]
fn session_public_facade_has_no_raw_owner_door_kernel_or_checked_closure_escape() {
    let path = root().join("crates/nexus-governed-control/src/host_session.rs");
    let text = code(&path);
    assert!(
        text.contains("pub struct HostSession"),
        "mandatory HostSession facade unavailable at baseline"
    );
    let signatures = signatures(&text);
    assert!(!signatures.is_empty());
    for sig in signatures {
        for forbidden in [
            "GovernedControl",
            "Authority",
            "AgentDoor",
            "CodingRun",
            "Intent",
            "FnOnce",
            "FnMut",
            "Fn(",
            "Option<HaltCell>",
        ] {
            assert!(!sig.contains(forbidden), "public session escape: {sig}");
        }
    }
    for line in text.lines().map(str::trim) {
        assert!(
            !line.starts_with("pub use "),
            "session cannot re-export legacy authority: {line}"
        );
        assert!(
            !line.starts_with("pub type "),
            "public aliases cannot conceal legacy conversions: {line}"
        );
        if line.starts_with("pub ") && line.contains(':') && !line.contains("fn ") {
            for forbidden in [
                "GovernedControl",
                "Authority",
                "AgentDoor",
                "CodingRun",
                "Intent",
                "HaltCell",
            ] {
                assert!(!line.contains(forbidden), "public raw field: {line}");
            }
        }
    }
}

#[test]
fn session_sealed_exclusion_registry_preserves_legacy_controls_and_bypass_plant_ids() {
    let mut seen = std::collections::BTreeSet::new();
    for line in include_str!("session/exclusions.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5);
        assert!(seen.insert(fields[0]));
        assert!(
            !fields[1].is_empty()
                && !fields[2].is_empty()
                && !fields[3].is_empty()
                && !fields[4].is_empty()
        );
        assert!(matches!(
            fields[2],
            "unsupported_host_route"
                | "no_such_authority"
                | "no_filesystem_sink"
                | "unreachable_lifecycle"
        ));
    }
    assert_eq!(seen.len(), 17);
}

#[test]
fn session_signature_guard_self_test_detects_checked_closure_and_raw_escape() {
    for planted in [
        "pub fn raw(&self) -> Arc<GovernedControl> { unreachable!() }",
        "pub fn door(&self) -> AgentDoor { unreachable!() }",
        "pub fn checked<T>(&self, f: impl FnOnce() -> T) -> T { f() }",
        "pub fn convert(&self) -> Intent { unreachable!() }",
    ] {
        let parsed = signatures(planted);
        assert_eq!(parsed.len(), 1);
        assert!(["GovernedControl", "AgentDoor", "FnOnce", "Intent"]
            .iter()
            .any(|needle| parsed[0].contains(needle)));
    }
}

#[test]
fn session_literal_oracle_provenance_is_frozen_independently_of_product_classifiers() {
    // A semantic oracle cannot detect being replaced by the very classifier
    // it is supposed to judge. Freeze the specification-derived literal inputs.
    // Changing these fingerprints requires a separately reviewed amendment.
    for (file, expected) in [
        (
            "model.rs",
            "70dbc88bfb1d0777a2d1cb3e072d8f09ae51a6002359c84c349834dfc2d1a538",
        ),
        (
            "outcomes.rs",
            "469345afe230737a82371ec4b72e1e7b6263f195ac12f9ea41840651a3be2efa",
        ),
    ] {
        let path = root().join("mother/host/tests/session").join(file);
        let source = code(&path);
        for forbidden in [
            "nexus_",
            "SettleCause::every",
            "PublicationClass",
            "include!",
            "reported_inventory",
            "production_classifier",
        ] {
            assert!(
                !source.contains(forbidden),
                "expected model consumes a production classifier: {file}: {forbidden}"
            );
        }
        let output = std::process::Command::new("python3")
            .args(["-c", "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())"])
            .arg(path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap().trim(),
            expected,
            "frozen literal oracle changed: {file}"
        );
    }
}

// The death harness is itself an oracle. Freeze its successful native report,
// torn-line, and exact receipt checks so a weakening cannot green itself.
#[test]
fn session_death_harness_provenance_rejects_error_torn_and_fake_receipt_substitutions() {
    for (file, expected) in [
        (
            "mother/host/tests/session_protocol.rs",
            "c707a7359fc2122f01e80893f135e2b38ced6aa1eafb7f1066ff2b7f66c51d89",
        ),
        (
            "mother/host/tests/support/faults.rs",
            "d2e637a4a287bb02301a776403ab63ef2eb4ea47a9d6f83ff8543b43b82ba53d",
        ),
        (
            "mother/host/tests/support/faults.c",
            "6b371e57ce313bb824a1466165b8ff6a8a710b600f59b47e0f5032ac1e30ffd3",
        ),
    ] {
        let output = std::process::Command::new("python3")
            .args(["-c", "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())"])
            .arg(root().join(file)).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap().trim(),
            expected,
            "death harness oracle changed: {file}; separately reviewed amendment required"
        );
    }
}

#[test]
fn fix1_f13_published_hands_build_is_self_contained() {
    let readme = include_str!("../../README.md");
    let section = readme.split("## Run Hands (").nth(1).unwrap();
    let shell = section
        .split("```bash\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let build = shell.split("install -d").next().unwrap().trim();
    assert!(
        build.starts_with("cargo build "),
        "published build must start with the repository-supported cargo command, without a builder scratch script: {build}"
    );
    assert_eq!(
        build.lines().count(),
        1,
        "standalone build has no external slot assignment"
    );
    for argument in [
        "-p nexus-mother-host",
        "--bin mother-hands",
        "--locked",
        "--target-dir target",
    ] {
        assert!(
            build.contains(argument),
            "missing reproducible build argument {argument}"
        );
    }
    assert!(!build.contains("/tmp/") && !build.contains("scratchpad") && !build.contains("$slot"));
    assert!(
        shell.contains("target/debug/mother-hands"),
        "run the artifact built by the documented command"
    );
}
