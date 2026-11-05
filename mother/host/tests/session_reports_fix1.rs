//! Fix round 1 E: exact report-only restart and final close debt receipts.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::journal::{verify, Event};
use nexus_mother::state::Trigger;
use support::faults::Fault;
use support::{Fixture, Pty};

fn directory_identities(root: &std::path::Path) -> Vec<(std::ffi::OsString, u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let mut result: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let m = entry.metadata().unwrap();
            (entry.file_name(), m.dev(), m.ino())
        })
        .collect();
    result.sort();
    result
}
fn reap(p: &mut Pty) {
    p.child.kill().unwrap();
    assert!(!p.child.wait().unwrap().success());
}
fn restart(f: &Fixture) -> String {
    let mut p = Pty::start(f.command("no reconstructed authority"));
    p.expect("RECOVERY REPORT");
    assert!(!p.finish().success());
    assert!(!p.output.contains("HANDS READY"));
    assert!(!p.output.contains("Answer code:"));
    p.output.clone()
}

#[test]
fn fix1_e_restart_lists_allocation_before_snapshot_and_prepared_only() {
    let mut failures = Vec::new();
    for point in ["before-snapshot", "allocation-prepared-only"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, point);
        let mut command = f.command("never published");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        fault.arm();
        p.line("run");
        p.yes();
        p.yes();
        p.yes();
        let receipt = fault.reached();
        assert!(receipt.path.contains("/files/ledger.db"));
        reap(&mut p);
        drop(p);
        let staging = f.home.join("broker/files/coding-runs");
        let before = support::privacy::files_under(&staging, None);
        let before_directories = directory_identities(&staging);
        assert!(
            std::fs::read_dir(&staging).unwrap().next().is_some(),
            "real allocated staging directory at {point}"
        );
        for repeat in 0..2 {
            let output = restart(&f);
            let facts = if point == "before-snapshot" {
                "allocation_attempted=true allocated=true"
            } else {
                "allocation_attempted=true allocated=false"
            };
            if !output.contains("RECORDED RESOURCE run=")
                || !output.contains(facts)
                || !output.contains("discard_recorded=false")
                || !output.contains("current_existence=unobserved")
            {
                failures.push(format!("{point}/{repeat}: {output}"));
            }
            assert_eq!(
                support::privacy::files_under(&staging, None),
                before,
                "report-only restart preserves staging"
            );
            assert_eq!(
                directory_identities(&staging),
                before_directories,
                "including empty staging directory identities"
            );
            f.empty();
        }
        println!("FIX1 E crash={point} killed_reaped=true fresh_reports=2 staging_preserved=true");
    }
    assert!(
        failures.is_empty(),
        "all exact native allocation facts survive projection: {failures:#?}"
    );
}

#[test]
fn fix1_e_restart_lists_exact_terminal_action_and_acknowledgement_uncertainty() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "mother-terminal-after-sync");
    let mut command = f.command("declined before process death");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.code();
    p.line("no");
    let receipt = fault.reached();
    assert!(receipt.path.ends_with("/mother-journal.jsonl\n"));
    let lines: Vec<_> = std::fs::read_to_string(f.home.join("mother-journal.jsonl"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let entries = verify(&lines).unwrap();
    let terminal = entries.last().unwrap();
    let Event::Transition {
        trigger: Trigger::ActionDenied { effect },
    } = &terminal.event
    else {
        panic!("exact terminal File denial before ack")
    };
    let action = entries
        .iter()
        .rev()
        .find_map(|entry| match &entry.event {
            Event::ApprovalPending {
                effect: candidate,
                pending,
            } if candidate == effect => Some(pending),
            _ => None,
        })
        .expect("exact pending identity survives terminal record");
    let expected = format!(
        "mission={} state=Denied terminal=action_denied terminal_seq={} effect={} run={} commitment={}",
        terminal.mission, terminal.seq, effect, action.run, action.commitment
    );
    reap(&mut p);
    drop(p);
    for _ in 0..2 {
        let output = restart(&f);
        assert!(
            output.contains(&expected),
            "exact durable terminal association: {expected}; {output}"
        );
        assert!(
            output.contains("prior_acknowledgement=unavailable"),
            "lost in-memory fact stays unavailable: {output}"
        );
        assert!(
            output.contains("primary_recovery_required=")
                && output.contains("primary_unsettled=")
                && output.contains("primary_unclean_sessions="),
            "complete primary health: {output}"
        );
    }
    f.empty();
    println!("FIX1 E crash=mother_terminal_before_ack killed_reaped=true fresh_reports=2 exact_terminal=true prior_acknowledgement=unavailable");
}

#[test]
fn fix1_e_unclean_close_lists_failed_or_inflight_finalization_separately() {
    let mut failures = Vec::new();
    for point in ["close-parent-fail", "close-parent-hold"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, point);
        let mut command = f.command("retain published owner file");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        p.create();
        p.expect("INVENTORY END");
        fault.arm();
        p.line("quit");
        let held = if point == "close-parent-hold" {
            Some(fault.reached())
        } else {
            None
        };
        assert!(!p.finish().success());
        let expected = if held.is_some() {
            "CLOSE OBLIGATIONS resource_snapshot=unknown in_flight_or_unknown=true"
        } else {
            "CLOSE OBLIGATIONS reservations_unresolved=1"
        };
        if !p.output.contains(expected)
            || !p.output.contains("PRODUCERS outstanding=0")
            || p.output.contains("UNCLEAN outstanding=0\n")
        {
            failures.push(format!("{point}: expected {expected}; {}", p.output));
        }
        if held.is_none() && !p.output.contains("parent_synced=false") {
            failures.push(format!(
                "{point}: missing newly-created parent sync debt; {}",
                p.output
            ));
        }
        assert_eq!(
            std::fs::read(f.workspace.join("hello.txt")).unwrap(),
            b"retain published owner file"
        );
        assert!(!p.output.contains("SESSION CLOSED recorded"));
        println!("FIX1 E close={point} nonzero=true published_file_preserved=true");
    }
    assert!(
        failures.is_empty(),
        "complete bounded final obligation report: {failures:#?}"
    );
}

#[test]
fn fix1_e_source_open_failure_keeps_its_controlled_provenance() {
    let f = Fixture::new();
    nexus_mother::adapter::create_private(&f.home.join("broker/p3-evidence")).unwrap();
    std::fs::write(
        f.home.join("broker/p3-evidence/evidence.sqlite3"),
        b"not a sqlite source",
    )
    .unwrap();
    let mut p = Pty::start(f.command("cannot open evidence"));
    assert!(!p.finish().success());
    assert!(
        p.output.contains("evidence_unavailable"),
        "source-specific error must not become home_unavailable: {}",
        p.output
    );
    f.empty();
}
