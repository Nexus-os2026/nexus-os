//! Structure: the manifest is pinned, the port is sealed to the adapter,
//! and the adapter calls only the agent side of the broker. (The scans that
//! read the tree from disk live in `adapter/scan.rs`: only the adapter
//! module touches the host.)

const MANIFEST: &str = include_str!("../../Cargo.toml");
const ADAPTER: &str = include_str!("../adapter/mod.rs");
const EFFECT: &str = include_str!("../effect.rs");
const COORDINATOR: &str = include_str!("../coordinator.rs");
const CONSENT: &str = include_str!("../consent.rs");

/// A source without its comment lines.
fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn structure_the_manifest_pins_the_dependencies() {
    let mut section = String::new();
    let mut dependencies = Vec::new();
    for line in MANIFEST.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.to_string();
            assert!(
                matches!(
                    section.as_str(),
                    "[package]" | "[dependencies]" | "[[example]]"
                ),
                "unexpected manifest section {section}"
            );
            continue;
        }
        if section == "[dependencies]" && !line.is_empty() && !line.starts_with('#') {
            dependencies.push(line.split('=').next().unwrap().trim().to_string());
        }
        assert!(!line.starts_with("build"), "{line}");
    }
    assert_eq!(
        dependencies,
        [
            "nexus-governed-control",
            "serde",
            "serde_json",
            "sha2",
            "hex"
        ]
    );
}

#[test]
fn structure_the_effect_port_is_sealed_to_the_adapter() {
    let effect = code(EFFECT);
    assert!(effect.contains("pub trait EffectPort: Send + Sync + sealed::Sealed {"));
    assert!(
        effect.contains("\nmod sealed {"),
        "the sealing module is private"
    );
    let sealed: Vec<&str> = effect
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("impl Sealed for"))
        .collect();
    assert_eq!(
        sealed,
        [
            "impl Sealed for crate::adapter::GovernedPort {}",
            "impl Sealed for crate::oracles::RecordingPort {}"
        ]
    );
    // The recording port's seal is test-only.
    let before = effect
        .lines()
        .map(str::trim)
        .take_while(|l| !l.contains("crate::oracles::RecordingPort"))
        .last()
        .unwrap();
    assert_eq!(before, "#[cfg(test)]");
    assert!(code(ADAPTER).contains("impl EffectPort for GovernedPort {"));
}

#[test]
fn structure_the_adapter_calls_only_the_agent_side_of_the_broker() {
    let adapter = code(ADAPTER);
    for needle in [
        "open_owner_run",
        "is_owner_run",
        "authorize(",
        "execute(",
        "propose(",
        ".deny(",
        "request_grant",
        "revoke_grant",
        "grant_scope",
        "resume(",
        "emergency_stop(",
        "shut_down(",
        "start_display",
        "ControlConfirmer",
        "RunOrigin::Command",
        "settle(",
    ] {
        assert!(!adapter.contains(needle), "adapter: {needle}");
    }
    for needle in [
        "RunOrigin::AgentGoal",
        ".agent_action(",
        ".cancel_run(",
        ".finish_when_settled(",
        ".runs().is_stopped()",
    ] {
        assert!(adapter.contains(needle), "adapter: {needle}");
    }
}

#[test]
fn structure_only_the_coordinator_asks_the_owner_and_only_it_mints_consent() {
    assert_eq!(code(COORDINATOR).matches("confirm_mission(").count(), 1);
    let consent = code(CONSENT);
    assert!(consent.contains("pub struct Consent {\n    hash: ContractHash,\n}"));
    assert!(consent.contains("pub(crate) fn minted("));
    let derive = consent
        .lines()
        .take_while(|l| !l.contains("pub struct Consent"))
        .last()
        .unwrap();
    // Not Clone, Copy, Default or Deserialize.
    assert_eq!(derive.trim(), "#[derive(Debug, PartialEq, Eq)]");
}
