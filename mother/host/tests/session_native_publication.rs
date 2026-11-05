//! Native dir-sync failure after the one publication, through the broker's
//! real extraction and Mother's real journal. No constructed final enum is
//! used as a primitive observation.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::evidence::SettlementOutcome;
use nexus_mother::journal::{verify, Event};
use nexus_mother::state::Trigger;
use support::{Fixture, Pty};

fn published_recovery(interrupt: bool) {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "published-dir-sync");
    let mut command = f.command("native published recovery\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.yes();
    let receipt = fault.reached();
    assert_eq!(receipt.path.trim_end(), f.workspace.to_str().unwrap());
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"native published recovery\n"
    );
    if interrupt {
        p.line("stop");
        p.expect("STOP REQUEST RECEIVED");
    }
    receipt.release();
    p.expect("ACTION FINISHED");
    p.expect("MISSION blocked");
    let path = f.home.join("mother-journal.jsonl");
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.ends_with('\n'));
    let lines: Vec<_> = raw.lines().map(str::to_owned).collect();
    let entries = verify(&lines).unwrap();
    assert!(
        !entries.iter().any(|e| matches!(
            e.event,
            Event::Transition {
                trigger: Trigger::ActionApproved { .. }
            }
        )),
        "recovery is never ordinary approval success"
    );
    let outcomes: Vec<_> = entries
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Transition {
                trigger: Trigger::Settled { outcome, .. },
            } => Some(outcome),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes.len(), 1);
    let SettlementOutcome::Published {
        recovery_required,
        interrupted,
    } = outcomes[0]
    else {
        panic!("native publication must survive extraction and terminal journaling")
    };
    assert!(*recovery_required);
    assert_eq!(!interrupted.is_empty(), interrupt);
    for count in 0..=lines.len() {
        verify(&lines[..count]).unwrap();
    }
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"native published recovery\n"
    );
    println!("NATIVE PUBLICATION OBSERVED published=true recovery=true interruption={interrupt} journal_prefixes={}", lines.len()+1);
    p.line("quit");
    assert!(
        !p.finish().success(),
        "unverified native directory durability remains a close obligation"
    );
    assert!(!p.output.contains("SESSION CLOSED recorded"));
    println!("NATIVE PUBLICATION VERIFIED published=true recovery=true interruption={interrupt} journal_prefixes={}", lines.len()+1);
}

#[test]
fn session_native_published_recovery_without_interruption() {
    published_recovery(false);
}

#[test]
fn session_native_published_recovery_with_interruption() {
    published_recovery(true);
}
