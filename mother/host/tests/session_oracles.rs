//! Oracle checkpoint, prior to production changes. Model coverage and real
//! route coverage are counted separately; one never stands in for the other.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
#[path = "session/model.rs"]
#[allow(dead_code)]
mod model;
mod support;
use model::*;
use support::{Fixture, Pty};

#[derive(Clone, Copy, Debug)]
enum Pause {
    Accepted,
    Captured,
    BeforeDialogue,
    AfterDialogue,
    BeforeSlow,
    AfterSlow,
    BeforeHold,
    Reserved,
    BeforeAdmission,
    AfterAdmission,
    KernelEntry,
    PublicationObservation,
    PhysicalEffect,
    Successor,
    Transfer,
    CloseAssessment,
    Quiescence,
    Seal,
    EndRecord,
    Unwind,
}
const PAUSES: [Pause; 20] = [
    Pause::Accepted,
    Pause::Captured,
    Pause::BeforeDialogue,
    Pause::AfterDialogue,
    Pause::BeforeSlow,
    Pause::AfterSlow,
    Pause::BeforeHold,
    Pause::Reserved,
    Pause::BeforeAdmission,
    Pause::AfterAdmission,
    Pause::KernelEntry,
    Pause::PublicationObservation,
    Pause::PhysicalEffect,
    Pause::Successor,
    Pause::Transfer,
    Pause::CloseAssessment,
    Pause::Quiescence,
    Pause::Seal,
    Pause::EndRecord,
    Pause::Unwind,
];
const INGRESS: [&str; 17] = [
    "CR",
    "LF",
    "CRLF",
    "original_cell_unscheduled",
    "repeated",
    "EOF",
    "partial_Ctrl_D",
    "master_close",
    "read_failure",
    "mission_stop",
    "stop_all",
    "door_fence",
    "budget",
    "grant_revocation",
    "grant_expiry",
    "quit",
    "fatal",
];
const FAILURES: [&str; 6] = [
    "none",
    "decline",
    "record_failed",
    "record_unknown",
    "poison",
    "panic",
];

#[test]
fn frozen_manifest_has_every_catalogue_row_and_unique_sink() {
    let mut rows = std::collections::BTreeSet::new();
    let mut sinks = std::collections::BTreeSet::new();
    for line in include_str!("session/sinks.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3);
        rows.insert(fields[0].to_owned());
        assert!(sinks.insert((fields[0], fields[2])));
        assert!(matches!(fields[1], "W" | "R" | "S" | "O" | "L" | "D"));
    }
    assert_eq!(rows, (1..=37).map(|n| format!("R{n:02}")).collect());
    assert_eq!(include_str!("session/desktop-live.txt").lines().count(), 23);
    println!(
        "manifest: {} catalogue rows, {} named sinks",
        rows.len(),
        sinks.len()
    );
}

#[test]
fn independently_generated_schedule_manifest_is_nonempty_for_every_sink() {
    // This enumerates required work, not successful production coverage.
    let mut cases = 0usize;
    let mut work = 0usize;
    for line in include_str!("session/sinks.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if !matches!(fields[1], "W" | "R") {
            continue;
        }
        work += 1;
        for pause in PAUSES {
            for ingress in INGRESS {
                for failure in FAILURES {
                    // Each key is retained verbatim; a runner must supply a
                    // structural exclusion or actual fixture observations.
                    let key = format!("{}:{}:{pause:?}:{ingress}:{failure}", fields[0], fields[2]);
                    assert!(!key.is_empty());
                    cases += 1;
                }
            }
        }
    }
    assert!(work > 50);
    assert_eq!(cases, work * 20 * 17 * 6);
    println!(
        "scheduled matrix: {work} W/R sinks, {cases} keys; production execution counted separately"
    );
}

#[test]
fn literal_mission_table_covers_terminal_states_with_debt() {
    assert_eq!(TERMINALS.len(), 13);
    let mut count = 0;
    for (row, state) in MISSIONS.into_iter().enumerate() {
        for (column, _cause) in CAUSES.into_iter().enumerate() {
            for publication in [
                Publication::NotDispatched,
                Publication::Pending,
                Publication::Absent,
                Publication::Published,
                Publication::Uncertain,
            ] {
                for ack_owed in [false, true] {
                    for record in [
                        Record::Owed,
                        Record::Recorded,
                        Record::Failed,
                        Record::Unknown,
                    ] {
                        let mut m = Model::normal();
                        m.publication = publication;
                        m.ack_owed = ack_owed;
                        m.terminal_record = record;
                        m.reserve(1);
                        m.abandon(1);
                        m.quit();
                        m.quiescent = true;
                        assert!(
                            !m.seal(),
                            "terminal {state:?} does not discharge resource debt"
                        );
                        if row >= 8 {
                            assert_eq!(TERMINALS[row][column], Some(state));
                        }
                        count += 1;
                    }
                }
            }
        }
    }
    assert_eq!(count, 4160);
    println!("literal state/cause/publication/ack/record model: {count} cases");
}

#[test]
fn enumerate_three_actors_and_three_interfering_events() {
    // Each actor must preserve accept -> reserve -> admission order. Enumerate
    // all 1,680 linear extensions of those nine operations. The three Stop
    // events are independently placed at every gap (including repeats).
    fn schedules(
        prefix: &mut Vec<(usize, usize)>,
        next: &mut [usize; 3],
        out: &mut Vec<Vec<(usize, usize)>>,
    ) {
        if prefix.len() == 9 {
            out.push(prefix.clone());
            return;
        }
        for actor in 0..3 {
            if next[actor] == 3 {
                continue;
            }
            let step = next[actor];
            next[actor] += 1;
            prefix.push((actor, step));
            schedules(prefix, next, out);
            prefix.pop();
            next[actor] -= 1;
        }
    }
    let mut orders = Vec::new();
    schedules(&mut Vec::new(), &mut [0; 3], &mut orders);
    assert_eq!(orders.len(), 1680);
    let mut cases = 0;
    for order in orders {
        for a in 0..=9 {
            for b in a..=9 {
                for c in b..=9 {
                    let mut m = Model::normal();
                    let mut requests = [None, None, None];
                    for (gap, action) in order
                        .iter()
                        .copied()
                        .map(Some)
                        .chain(std::iter::once(None))
                        .enumerate()
                    {
                        for event in [a, b, c] {
                            if event == gap {
                                m.stop();
                            }
                        }
                        let Some((actor, step)) = action else { break };
                        match step {
                            0 => requests[actor] = Some(m.accept(actor as u16, None)),
                            1 => {
                                m.reserve(actor as u16);
                            }
                            2 => {
                                let allowed = m.admit(requests[actor].as_mut().unwrap(), 1);
                                assert_eq!(
                                    allowed,
                                    gap < a,
                                    "event order, not production output, sets expectation"
                                );
                            }
                            _ => unreachable!(),
                        }
                        assert!(m.conserved());
                    }
                    assert_eq!(m.ingress.len(), 3);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 369600);
    println!("bounded model: 3 actors, 3 events, {cases} traces");
}

#[test]
fn independent_close_and_ack_controls_and_negative_cases() {
    let mut m = Model::normal();
    m.reserve(7);
    m.abandon(7);
    m.quit();
    assert!(m.close_cleanup(7), "normal-close positive control");
    m.stop();
    assert!(!m.close_cleanup(7));
    assert!(!m.close_cleanup(8));
    assert!(m.conserved());
    let r = m.resources.get_mut(&7).unwrap();
    r.state = ResourceState::Released;
    r.observed_absent = true;
    r.authority_closed = true;
    r.records = [Record::Recorded; 3];
    assert!(!r.discharged(), "absence is not durability");
    r.removal_durable = true;
    assert!(r.discharged());
    m.quiescent = true;
    assert!(!m.seal(), "uncovered Stop record");
    m.recorded_ingress = m.ingress.clone();
    assert!(
        !m.ack([1; 6], false, true, true),
        "unknown is not duplicate"
    );
    assert!(!m.ack([1; 6], true, false, true));
    assert!(!m.ack([1; 6], true, true, false));
    assert!(m.ack([1; 6], true, true, true));
    assert!(m.ack([1; 6], false, true, true));
    assert!(!m.ack([2; 6], true, true, true));
    assert!(m.seal());
    assert!(!m.stop());
    m.end_records = [Record::Recorded; 2];
    assert!(!m.finish(true), "delivery remains a final obligation");
    let mut idle = Model::normal();
    idle.stop();
    idle.quit();
    idle.quiescent = true;
    idle.recorded_ingress = idle.ingress.clone();
    assert!(idle.seal(), "idle emergency clean-close control");
    idle.end_records = [Record::Recorded; 2];
    idle.delivered = true;
    assert!(idle.finish(true));
}

#[test]
fn session_surface_has_no_raw_owner_or_generic_effect_escape() {
    let hands = include_str!("../src/hands.rs");
    for forbidden in [
        "GovernedControl",
        "GovernedPort",
        "AgentDoor",
        ".owner()",
        ".door_with_halt()",
        "GatedLane",
        "Gate::",
    ] {
        assert!(
            !hands.contains(forbidden),
            "sealed Hands route still exposes {forbidden}"
        );
    }
}

#[test]
fn emergency_commands_refuse_without_dialogue_real_terminal() {
    for newline in [b"\r".as_slice(), b"\n", b"\r\n"] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("no work\n"));
        p.ready();
        p.send(b"stop");
        p.send(newline);
        p.expect("STOP RECORDING COMPLETED");
        let dialogue_count = p.output.matches("Answer code:").count();
        for command in [
            "lift-for-undo",
            "undo",
            "cleanup",
            "run",
            "approve",
            "detail",
        ] {
            p.line(command);
            p.expect("REFUSED");
            assert_eq!(
                p.output.matches("Answer code:").count(),
                dialogue_count,
                "{command}: emergency must refuse without a fresh dialogue"
            );
        }
        f.empty();
        p.quit();
    }
}

#[test]
fn emergency_pending_drop_retains_real_staging_and_refuses_clean_close() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("owned candidate\n"));
    p.ready();
    p.mission();
    let staging = f.home.join("broker/files/coding-runs");
    let before = support::privacy::files_under(&staging, None);
    assert!(!before.is_empty());
    p.line("stop");
    p.expect("STOP RECORDING COMPLETED");
    p.line("cleanup");
    p.expect("REFUSED");
    assert_eq!(support::privacy::files_under(&staging, None), before);
    p.line("quit");
    assert!(!p.finish().success());
    assert!(!p.output.contains("SESSION CLOSED recorded"));
    assert_eq!(
        support::privacy::files_under(&staging, None),
        before,
        "unclean final-owner destruction may not remove staging"
    );
}
