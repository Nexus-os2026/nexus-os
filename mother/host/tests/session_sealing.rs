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
// Dependency-free matcher for pub(\s+(const|async|unsafe|extern(\s+"[^"]*")?))*\s+fn.
fn public_function(mut line: &str) -> bool {
    fn word<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
        line.strip_prefix(prefix)
            .filter(|r| r.starts_with(char::is_whitespace))
            .map(str::trim_start)
    }
    let Some(rest) = word(line, "pub") else {
        return false;
    };
    line = rest.trim_start();
    loop {
        if let Some(rest) = ["const", "async", "unsafe"]
            .iter()
            .find_map(|p| word(line, p))
        {
            line = rest.trim_start();
        } else if let Some(rest) = word(line, "extern") {
            line = if let Some(abi) = rest.strip_prefix('"') {
                let Some(end) = abi.find('"') else {
                    return false;
                };
                abi[end + 1..].trim_start()
            } else {
                rest
            };
        } else {
            return word(line, "fn").is_some();
        }
    }
}
fn signatures(source: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if public_function(trimmed) {
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
            "model",
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
// Oracle amendment U1/U3, Owner: "Make these 5 fixes don't keep pending."
// Re-seal only the already registered oracle changes, not new behavior:
// session_protocol.rs (1577232a): old fresh-home "Recovery report only..."
// -> "Fresh start: no earlier session in this home; nothing to recover."
// Old digest c707a7359fc2122f01e80893f135e2b38ced6aa1eafb7f1066ff2b7f66c51d89.
// faults.c (2349a9e6): old "no echo-write hold" -> "echo-hold plus native
// Stop-recording receipt". Original death/torn/receipt checks are unchanged.
// Old digest eb49fa95f07b430062cc396c80a3c1677a42dff75bb652599eb83990e0078de5.
// Keep the exact source-digest assertions below; any further edit must fail.
#[test]
fn session_death_harness_provenance_rejects_error_torn_and_fake_receipt_substitutions() {
    for (file, expected) in [
        (
            "mother/host/tests/session_protocol.rs",
            "bbc0a15ace1bcd682f1daebeead1793c12b82bfd20c3e2a72bc342077e99b71b",
        ),
        (
            "mother/host/tests/support/faults.rs",
            "d2e637a4a287bb02301a776403ab63ef2eb4ea47a9d6f83ff8543b43b82ba53d",
        ),
        (
            "mother/host/tests/support/faults.c",
            "68c4c3dc0e2dfc69f7584154788d71f008270fa6b4a1c1e8ecdfd0f5647a2c1b",
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

// C1-S1-SB2 adds private machinery, never a public nested escape.
fn sb2_public_escape(text: &str) -> bool {
    let text = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    if signatures(&text).iter().any(|s| {
        let s = s
            .replace("crate::authority::ids::CommitmentId", "CommitmentId")
            .to_ascii_lowercase();
        [
            "model",
            "inference",
            "governedcontrol",
            "agentdoor",
            "transport",
            "stopwatch",
            "fnonce",
            "fnmut",
            "fn(",
        ]
        .iter()
        .any(|n| s.contains(n))
            || s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|w| w == "authority")
    }) {
        return true;
    }
    text.lines().map(str::trim).any(|l| {
        l.starts_with("pub use ")
            || l.starts_with("pub type ")
            || (l.starts_with("pub ")
                && !l.contains("fn ")
                && [
                    "GovernedControl",
                    "AgentDoor",
                    "Transport",
                    "StopWatch",
                    "Authority",
                ]
                .iter()
                .any(|n| l.contains(n)))
    })
}
#[test]
fn sb2_private_model_submodules_expose_no_callable_route() {
    fn visit(path: &Path) {
        if path.is_dir() {
            for e in std::fs::read_dir(path).unwrap() {
                visit(&e.unwrap().path());
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            assert!(
                !sb2_public_escape(&code(path)),
                "nested escape {}",
                path.display()
            );
        }
    }
    visit(&root().join("crates/nexus-governed-control/src/host_session"));
    assert!(!sb2_public_escape(
        "pub fn lookup(id: crate::authority::ids::CommitmentId) {}"
    ));
    assert!(sb2_public_escape(
        "pub fn raw() -> crate::authority::Authority {}"
    ));
    assert!(!sb2_public_escape(
        "pub fn settle_close_authority(&self) {}"
    ));
    for plant in [
        "pub fn raw(&self) -> Arc<GovernedControl> { unreachable!() }",
        "pub fn model_identity(&self) -> String { unreachable!() }",
    ] {
        assert!(sb2_public_escape(plant));
    }
}

#[test]
fn sb2_signature_collection_catches_qualified_model_aliases() {
    for qualifiers in [
        "",
        "const ",
        "async ",
        "unsafe ",
        "extern \"C\" ",
        "extern ",
        "const unsafe ",
        "unsafe extern \"C\" ",
        "unsafe extern ",
    ] {
        let plant = format!(
            "pub {qualifiers}fn raw(\n&self,\n) -> model::RegistrationReceipt {{ unreachable!() }}"
        );
        assert_eq!(
            signatures(&plant).len(),
            1,
            "collect qualified facade: {plant}"
        );
        assert!(sb2_public_escape(&plant), "reject aliased model signature");
    }
}
