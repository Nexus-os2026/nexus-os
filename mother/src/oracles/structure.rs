//! I7 effects only through the existing broker; I8 a bounded, content-free
//! journal.

use super::fake::Reply;
use super::*;
use crate::coordinator::{JournalEvent, MissionState, MAX_JOURNAL, MAX_MISSIONS};

const SOURCES: &[(&str, &str)] = &[
    ("lib.rs", include_str!("../lib.rs")),
    ("broker.rs", include_str!("../broker.rs")),
    ("contract.rs", include_str!("../contract.rs")),
    ("coordinator.rs", include_str!("../coordinator.rs")),
];

const MANIFEST: &str = include_str!("../../Cargo.toml");

/// A source without its comments and without its `#[cfg(test)]` items
/// (each a single line in this crate, checked here).
fn production(text: &str) -> String {
    let mut out = Vec::new();
    let mut skip_next = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if skip_next {
            assert!(
                trimmed.ends_with(';') || trimmed.ends_with("{}"),
                "a #[cfg(test)] item must be one line: {line}"
            );
            skip_next = false;
            continue;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            skip_next = true;
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

fn all_production() -> Vec<(&'static str, String)> {
    SOURCES
        .iter()
        .map(|(name, text)| (*name, production(text)))
        .collect()
}

#[test]
fn m1_i7_the_crate_has_no_effect_of_its_own() {
    for (name, text) in all_production() {
        for needle in [
            "std::fs",
            "std::process",
            "std::net",
            "std::env",
            "std::os",
            "std::thread",
            "reqwest",
            "tokio",
            "libc",
            "Command::new",
            "File::",
            "TcpStream",
            "UdpSocket",
            "unsafe {",
            "unsafe fn",
            "unsafe impl",
            "extern ",
            "include_bytes!",
            "env!(",
        ] {
            assert!(!text.contains(needle), "{name}: {needle}");
        }
    }
    assert!(SOURCES[0].1.contains("#![deny(unsafe_code)]"));
}

#[test]
fn m1_i7_mother_calls_only_the_agent_side_of_the_broker() {
    for (name, text) in all_production() {
        for needle in [
            "open_owner_run",
            "is_owner_run",
            "authorize(",
            "execute(",
            "propose(",
            ".deny(",
            "::deny(",
            "request_grant",
            "revoke_grant",
            "grant_scope",
            "resume(",
            "emergency_stop(",
            "shut_down(",
            "start_display",
            "stop_display",
            "ControlConfirmer",
            "RunOrigin::Command",
            "settle(",
        ] {
            assert!(!text.contains(needle), "{name}: {needle}");
        }
    }
    let broker = production(SOURCES[1].1);
    for needle in [
        "GovernedControl::open_run(self, agent.clone(), RunOrigin::AgentGoal)",
        "GovernedControl::agent_action(self, agent, run, intent)",
        "GovernedControl::cancel_run(self, run)",
        "GovernedControl::finish_when_settled(self, run)",
        ".authority().commitments().view(id)",
        ".authority().runs().is_stopped()",
    ] {
        assert!(broker.contains(needle), "broker.rs: {needle}");
    }
}

#[test]
fn m1_i7_the_broker_trait_is_sealed_to_governed_control() {
    let production = all_production();
    let impls = |needle: &str| -> Vec<String> {
        production
            .iter()
            .flat_map(|(name, text)| {
                text.lines()
                    .filter(|line| line.contains(needle))
                    .map(move |line| format!("{name}: {}", line.trim()))
            })
            .collect()
    };
    assert_eq!(
        impls("impl Sealed for"),
        ["broker.rs: impl Sealed for nexus_governed_control::governed::GovernedControl {}"]
    );
    assert_eq!(
        impls("impl Broker for"),
        ["broker.rs: impl Broker for GovernedControl {"]
    );
    assert!(production[1]
        .1
        .contains("pub trait Broker: Send + Sync + sealed::Sealed {"));
    // The sealing module is private.
    assert!(production[1].1.contains("\nmod sealed {"));
}

#[test]
fn m1_i7_mother_has_no_consent_of_her_own() {
    for (name, text) in all_production() {
        assert!(!text.contains("impl OwnerConsent for"), "{name}");
        assert!(!text.contains("confirm_mission(&"), "{name}: only the coordinator asks");
    }
}

#[test]
fn m1_i7_the_manifest_pins_the_dependencies() {
    let mut section = String::new();
    let mut dependencies = Vec::new();
    for line in MANIFEST.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.to_string();
            assert!(
                matches!(section.as_str(), "[package]" | "[dependencies]"),
                "unexpected manifest section {section}"
            );
            continue;
        }
        if section == "[dependencies]" && !line.is_empty() && !line.starts_with('#') {
            dependencies.push(line.split('=').next().unwrap().trim().to_string());
        }
        if section == "[package]" {
            assert!(!line.starts_with("build"), "{line}");
        }
    }
    assert_eq!(
        dependencies,
        ["nexus-governed-control", "serde", "serde_json", "sha2", "hex"]
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.join("build.rs").exists());
}

#[test]
fn m1_i8_the_journal_holds_no_content() {
    let s = setup();
    let contract = MissionContract {
        goal: "GOAL-MARKER-7f3a".into(),
        steps: vec![
            step(request("http://127.0.0.1:9/PATH-MARKER-91c2")),
            step(tool("TOOL-MARKER-2b8e")),
        ],
        consent_ttl_ms: TTL_MS,
    };
    let id = s.mother.draft(contract.clone()).unwrap();
    s.mother.request_consent(id, &Yes).unwrap();
    s.broker.reply(Reply::Done(Some("OUTPUT-MARKER-55d0".into())));
    s.broker.reply(Reply::Refuse("no_covering_grant"));
    s.mother.step(id).unwrap();
    let _ = s.mother.step(id);
    let journal = format!("{:?}", s.mother.journal());
    for marker in [
        "GOAL-MARKER",
        "PATH-MARKER",
        "TOOL-MARKER",
        "OUTPUT-MARKER",
        "127.0.0.1",
    ] {
        assert!(!journal.contains(marker), "{marker}: {journal}");
    }
    let run = s.mother.run_of(id).unwrap();
    let digest = contract.digest().unwrap();
    let events: Vec<JournalEvent> = s
        .mother
        .journal()
        .into_iter()
        .filter(|entry| entry.mission == id)
        .map(|entry| entry.event)
        .collect();
    assert_eq!(
        events,
        vec![
            JournalEvent::Drafted { digest },
            JournalEvent::ConsentAsked { digest },
            JournalEvent::Consented { digest },
            JournalEvent::StepReserved { step: 0 },
            JournalEvent::RunOpened { run },
            JournalEvent::StepDone { step: 0 },
            JournalEvent::StepReserved { step: 1 },
            JournalEvent::StepRefused {
                step: 1,
                class: "no_covering_grant"
            },
            JournalEvent::RunCancelled { run },
        ]
    );
    let seqs: Vec<u64> = s.mother.journal().iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
}

#[test]
fn m1_i8_the_journal_is_bounded() {
    let s = setup();
    let n = 600;
    for _ in 0..n {
        let id = s.mother.draft(one_step()).unwrap();
        s.mother.stop(id).unwrap();
    }
    let journal = s.mother.journal();
    assert_eq!(journal.len(), MAX_JOURNAL);
    let total = 2 * n as u64;
    assert_eq!(journal.first().unwrap().seq, total - MAX_JOURNAL as u64);
    assert_eq!(journal.last().unwrap().seq, total - 1);
}

#[test]
fn m1_i8_ended_missions_make_room_and_live_ones_do_not() {
    let s = setup();
    let first = s.mother.draft(one_step()).unwrap();
    for _ in 1..MAX_MISSIONS {
        s.mother.draft(one_step()).unwrap();
    }
    s.mother.stop(first).unwrap();
    // The oldest ended mission makes room; no live mission is ever dropped.
    let next = s.mother.draft(one_step()).unwrap();
    assert_eq!(s.mother.state(first), None);
    assert_eq!(s.mother.state(next), Some(MissionState::Drafted));
    assert!(s.mother.draft(one_step()).is_err());
    assert_eq!(s.broker.calls(), vec![]);
}
