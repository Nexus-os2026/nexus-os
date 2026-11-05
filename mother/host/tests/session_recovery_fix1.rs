//! Fix round 1 D: canonical semantic recovery is required before bootstrap.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::contract::ContractHash;
use nexus_mother::effect::PendingAction;
use nexus_mother::journal::{
    ContractRef, Entry, Event, MemoryJournal, MissionId, GENESIS, SESSION,
};
use nexus_mother::private::{ContentId, ContentKind};
use nexus_mother::state::Trigger;
use support::{Fixture, Pty};

fn lines(events: Vec<(MissionId, Event)>) -> Vec<String> {
    let mut previous = GENESIS.to_owned();
    events
        .into_iter()
        .enumerate()
        .map(|(seq, (mission, event))| {
            let entry = Entry::chained(seq as u64, &previous, mission, event);
            previous = entry.hash.clone();
            entry.to_line()
        })
        .collect()
}
fn draft() -> Event {
    Event::Drafted {
        contract: ContractRef {
            contract_hash: ContractHash::from_hex(&"a".repeat(64)).unwrap(),
            content: ContentId::of(ContentKind::Contract, b"report-only fixture"),
            bytes: 19,
        },
    }
}

#[test]
fn fix1_d_canonical_recovery_rejects_invalid_history_before_workspace() {
    let mission = MissionId(1);
    let cases = [
        ("singleton_close", vec![(SESSION, Event::SessionClosed)]),
        (
            "duplicate_close",
            vec![
                (SESSION, Event::SessionOpened),
                (SESSION, Event::SessionClosed),
                (SESSION, Event::SessionClosed),
            ],
        ),
        (
            "invalid_transition",
            vec![
                (SESSION, Event::SessionOpened),
                (mission, draft()),
                (
                    mission,
                    Event::Transition {
                        trigger: Trigger::Start,
                    },
                ),
            ],
        ),
        (
            "inconsistent_pending",
            vec![
                (SESSION, Event::SessionOpened),
                (mission, draft()),
                (
                    mission,
                    Event::ApprovalPending {
                        effect: 17,
                        pending: PendingAction {
                            run: "not-a-run".into(),
                            commitment: "not-an-action".into(),
                            deadline_mono_ms: 10,
                        },
                    },
                ),
            ],
        ),
    ];
    let mut failures = Vec::new();
    for (name, events) in cases {
        let f = Fixture::new();
        nexus_mother::adapter::create_private(&f.home).unwrap();
        let lines = lines(events);
        let memory = MemoryJournal::new();
        memory.set_lines(lines.clone());
        assert!(
            nexus_mother::coordinator::inspect_history(&memory).is_err(),
            "canonical control {name}"
        );
        let raw = format!("{}\n", lines.join("\n"));
        std::fs::write(f.home.join("mother-journal.jsonl"), &raw).unwrap();
        let mut p = Pty::start(f.command("must not start"));
        p.line("quit");
        if p.finish().success() {
            failures.push(format!("{name}: invalid source cleanly opened"));
        }
        if !p.output.contains("RECOVERY FAILED mother_history") || p.output.contains("HANDS READY")
        {
            failures.push(format!("{name}: {}", p.output));
        }
        if std::fs::read_to_string(f.home.join("mother-journal.jsonl")).unwrap() != raw {
            failures.push(format!("{name}: invalid history changed"));
        }
        f.empty();
        println!("FIX1 D input={name} canonical_rejected=true workspace_files=0");
    }
    assert!(
        failures.is_empty(),
        "all semantically invalid sources must fail before bootstrap: {failures:#?}"
    );
}

#[test]
fn fix1_d_valid_empty_and_closed_histories_still_bootstrap() {
    for events in [
        vec![],
        vec![
            (SESSION, Event::SessionOpened),
            (SESSION, Event::SessionClosed),
        ],
    ] {
        let f = Fixture::new();
        nexus_mother::adapter::create_private(&f.home).unwrap();
        let lines = lines(events);
        let memory = MemoryJournal::new();
        memory.set_lines(lines.clone());
        assert!(nexus_mother::coordinator::inspect_history(&memory).is_ok());
        std::fs::write(
            f.home.join("mother-journal.jsonl"),
            if lines.is_empty() {
                String::new()
            } else {
                format!("{}\n", lines.join("\n"))
            },
        )
        .unwrap();
        let mut p = Pty::start(f.command("positive history control"));
        p.ready();
        p.quit();
        f.empty();
        println!("FIX1 D valid_history ready=true clean_close=true");
    }
}

#[test]
fn fix1_d_direct_broker_refuses_invalid_session_order() {
    const CASE: &str = "M2DD_FIX1_DIRECT_RECOVERY";
    let Ok(case) = std::env::var(CASE) else {
        for case in ["singleton", "duplicate"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "fix1_d_direct_broker_refuses_invalid_session_order",
                    "--nocapture",
                ])
                .env(CASE, case)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{case}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    use nexus_governed_control::authority::approval::*;
    struct Owner;
    impl ControlConfirmer for Owner {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool {
            false
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            false
        }
        fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
            false
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            false
        }
        fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
            false
        }
    }
    assert!(nexus_governed_control::files::make_non_dumpable());
    let f = Fixture::new();
    nexus_mother::adapter::create_private(&f.home).unwrap();
    let events = if case == "singleton" {
        vec![(SESSION, Event::SessionClosed)]
    } else {
        vec![
            (SESSION, Event::SessionOpened),
            (SESSION, Event::SessionClosed),
            (SESSION, Event::SessionClosed),
        ]
    };
    std::fs::write(
        f.home.join("mother-journal.jsonl"),
        format!("{}\n", lines(events).join("\n")),
    )
    .unwrap();
    let evidence = nexus_governed_control::evidence_store::PrimaryEvidence::open(
        &f.home.join("broker/p3-evidence/evidence.sqlite3"),
        vec![],
    )
    .unwrap();
    let session = nexus_governed_control::host_session::HostSession::open_with_history(
        nexus_governed_control::host_session::SessionSeed::new().unwrap(),
        &f.home.join("broker"),
        &f.home,
        evidence,
        std::sync::Arc::new(nexus_governed_control::authority::clock::SystemClock::default()),
        std::sync::Arc::new(Owner),
    )
    .unwrap();
    assert!(
        session.startup_report().mother.is_err(),
        "invalid session order is not a verified Mother source"
    );
    assert!(session.bootstrap_workspace(&f.workspace, &[]).is_err());
    assert!(session.workspace().is_none());
    println!("FIX1 D direct={case} canonical_source_refused=true workspace_registered=false");
}
