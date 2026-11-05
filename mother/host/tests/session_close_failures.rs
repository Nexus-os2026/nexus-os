//! Real post-seal store and delivery failures. Durable ends are independent
//! of full close success; a lost delivery cannot erase recorded facts.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::journal::{verify, Event};
use support::{Fixture, Pty};

const UNCLEAN: &str = "UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.";
fn mother_end(f: &Fixture) -> usize {
    let raw = std::fs::read_to_string(f.home.join("mother-journal.jsonl")).unwrap();
    assert!(raw.ends_with('\n'));
    let lines: Vec<_> = raw.lines().map(str::to_owned).collect();
    verify(&lines)
        .unwrap()
        .iter()
        .filter(|entry| matches!(entry.event, Event::SessionClosed))
        .count()
}

#[test]
fn session_final_delivery_failure_is_nonzero_even_when_both_ends_are_recorded() {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "final-write-fail");
    let mut command = f.command("final delivery fixture\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    fault.arm();
    p.line("quit");
    assert!(!p.finish().success());
    assert_eq!(mother_end(&f), 1);
    assert_eq!(support::evidence::counts(&f), [0, 1, 1]);
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"final delivery fixture\n"
    );
    println!("CLOSE DELIVERY failure=true mother_end=Recorded primary_end=Recorded exit=nonzero");
}

#[test]
fn session_primary_failure_after_mother_end_is_reported_as_partial_close() {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "primary-close-fail");
    let mut command = f.command("partial close fixture\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    fault.arm();
    p.line("quit");
    assert!(!p.finish().success());
    assert_eq!(mother_end(&f), 1);
    assert_eq!(support::evidence::counts(&f), [0, 1, 0]);
    assert!(!p.output.contains("SESSION CLOSED recorded"));
    assert!(p.output.contains(UNCLEAN));
    assert!(p.output.contains("MOTHER END Recorded"));
    assert!(p.output.contains("PRIMARY END Failed") || p.output.contains("PRIMARY END Unknown"));
    println!("CLOSE PARTIAL mother_end=Recorded primary_end=not_recorded exit=nonzero");
}

#[test]
fn session_primary_append_timeout_keeps_unknown_separate_from_mother_recorded() {
    let f = Fixture::new();
    let fault = support::faults::Fault::new(&f, "primary-close-hold");
    let mut command = f.command("unknown primary fixture\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    fault.arm();
    let shutdown = std::time::Instant::now();
    p.line("quit");
    let receipt = fault.reached();
    assert!(receipt.path.contains("/p3-evidence/evidence.sqlite3"));
    assert_eq!(mother_end(&f), 1);
    // Never release the blocked append. The process must bound its own
    // shutdown, retain the outstanding executor and return nonzero.
    assert!(!p.finish().success());
    assert!(shutdown.elapsed() < std::time::Duration::from_secs(5));
    assert!(!p.output.contains("SESSION CLOSED recorded"));
    assert!(p.output.contains(UNCLEAN));
    assert!(p.output.contains("MOTHER END Recorded"));
    assert!(p.output.contains("PRIMARY END Unknown"));
    assert_eq!(support::evidence::counts(&f), [0, 1, 0]);
    println!("CLOSE TIMEOUT mother_end=Recorded primary_end=Unknown exit=nonzero");
}
