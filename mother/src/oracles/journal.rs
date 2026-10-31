//! The journal: append-only and hash-chained. Tampering is detected and
//! the journal refused; a journal that verifies but breaks the state
//! machine is refused; an effect whose outcome was not recorded blocks its
//! mission on restart.

use super::*;
use crate::journal::{verify, Entry, Event, JournalError, GENESIS};
use crate::state::Trigger;

/// A journal with a finished mission, a stopped one and a denied one.
fn written() -> World {
    let w = world();
    w.mission_in(MissionState::Done);
    w.mission_in(MissionState::Stopped);
    w.mission_in(MissionState::Denied);
    w
}

fn refused(w: &World, lines: Vec<String>, what: &str) {
    w.journal.set_lines(lines.clone());
    match w.reopen() {
        Err(CoordinatorError::Journal(
            JournalError::Tampered { .. } | JournalError::Malformed { .. },
        )) => {}
        Err(other) => panic!("{what}: refused for the wrong reason: {other:?}"),
        Ok(_) => panic!("{what}: a tampered journal was accepted"),
    }
    assert!(verify(&lines).is_err(), "{what}");
}

#[test]
fn journal_an_untouched_journal_verifies_and_reopens() {
    let w = written();
    let lines = w.journal.lines();
    let entries = verify(&lines).unwrap();
    assert_eq!(entries.len(), lines.len());
    assert_eq!(entries[0].prev, GENESIS);
    for (i, pair) in entries.windows(2).enumerate() {
        assert_eq!(pair[1].prev, pair[0].hash, "entry {}", i + 1);
        assert_eq!(pair[1].seq, pair[0].seq + 1);
    }
    for entry in &entries {
        assert_eq!(entry.hash, entry.expected_hash());
        assert_eq!(entry.hash.len(), 64);
    }
    assert!(w.reopen().is_ok());
}

#[test]
fn journal_every_kind_of_tampering_is_refused() {
    let w = written();
    let lines = w.journal.lines();
    let n = lines.len();
    assert!(n > 20, "a journal worth tampering with: {n}");
    let stop_line = lines
        .iter()
        .position(|l| l.contains("\"trigger\":\"stop\""))
        .expect("a stop entry");

    // Undo the Stop: turn it into something else.
    let mut t = lines.clone();
    t[stop_line] = t[stop_line].replace("\"trigger\":\"stop\"", "\"trigger\":\"start\"");
    refused(&w, t, "a Stop rewritten");

    // Remove the Stop.
    let mut t = lines.clone();
    t.remove(stop_line);
    refused(&w, t, "a Stop removed");

    // Every single entry, one character changed in its event.
    for i in 0..n {
        let mut t = lines.clone();
        let at = t[i].find("\"event\"").unwrap() + 3;
        let mut bytes = t[i].clone().into_bytes();
        bytes[at] = if bytes[at] == b'x' { b'y' } else { b'x' };
        t[i] = String::from_utf8(bytes).unwrap();
        refused(&w, t, &format!("entry {i} edited"));
    }

    // Removed from the middle, reordered, duplicated.
    let mut t = lines.clone();
    t.remove(n / 2);
    refused(&w, t, "an entry removed");
    let mut t = lines.clone();
    t.swap(3, 4);
    refused(&w, t, "two entries swapped");
    let mut t = lines.clone();
    t.insert(5, lines[5].clone());
    refused(&w, t, "an entry duplicated");

    // A forged entry appended with the right prev but a made-up hash.
    let last = verify(&lines).unwrap().pop().unwrap();
    let mut forged = Entry::chained(
        last.seq + 1,
        &last.hash,
        MissionId(2),
        Event::Transition {
            trigger: Trigger::Start,
        },
    );
    forged.hash = "f".repeat(64);
    let mut t = lines.clone();
    t.push(forged.to_line());
    refused(&w, t, "a forged entry");

    // A hash recomputed for an edited entry, without rewriting the rest.
    let mut entries = verify(&lines).unwrap();
    entries[2].mission = MissionId(99);
    entries[2].hash = entries[2].expected_hash();
    let mut t = lines.clone();
    t[2] = entries[2].to_line();
    refused(&w, t, "an entry re-hashed alone");

    // A torn last line, garbage, an empty line, a sequence gap.
    let mut t = lines.clone();
    let torn = t[n - 1][..t[n - 1].len() / 2].to_string();
    t[n - 1] = torn;
    refused(&w, t, "a torn last line");
    let mut t = lines.clone();
    t.push("not json".into());
    refused(&w, t, "garbage");
    let mut t = lines.clone();
    t.insert(3, String::new());
    refused(&w, t, "an empty line");
    let mut t = lines.clone();
    t[0] = t[0].replace("\"seq\":0", "\"seq\":1");
    refused(&w, t, "a sequence changed");
}

fn chained(events: Vec<(MissionId, Event)>) -> Vec<String> {
    let mut prev = GENESIS.to_string();
    events
        .into_iter()
        .enumerate()
        .map(|(seq, (mission, event))| {
            let entry = Entry::chained(seq as u64, &prev, mission, event);
            prev = entry.hash.clone();
            entry.to_line()
        })
        .collect()
}

#[test]
fn journal_a_chained_journal_that_breaks_the_state_machine_is_refused() {
    let c = contract();
    let id = MissionId(1);
    let t = |trigger| (id, Event::Transition { trigger });
    let cases: Vec<(&str, Vec<(MissionId, Event)>)> = vec![
        (
            "work without consent",
            vec![
                (
                    id,
                    Event::Drafted {
                        contract: c.clone(),
                    },
                ),
                t(Trigger::Submit { hash: c.hash() }),
                t(Trigger::Start),
            ],
        ),
        (
            "a stopped mission restarted",
            vec![
                (
                    id,
                    Event::Drafted {
                        contract: c.clone(),
                    },
                ),
                t(Trigger::Stop {
                    reason: "owner".into(),
                }),
                t(Trigger::Submit { hash: c.hash() }),
            ],
        ),
        (
            "a mission that was never drafted",
            vec![t(Trigger::Submit { hash: c.hash() })],
        ),
        (
            "a mission drafted twice",
            vec![
                (
                    id,
                    Event::Drafted {
                        contract: c.clone(),
                    },
                ),
                (
                    id,
                    Event::Drafted {
                        contract: c.clone(),
                    },
                ),
            ],
        ),
        (
            "a submit for another hash",
            vec![
                (
                    id,
                    Event::Drafted {
                        contract: c.clone(),
                    },
                ),
                t(Trigger::Submit {
                    hash: crate::contract::ContractHash::from_hex(&"00".repeat(32)).unwrap(),
                }),
            ],
        ),
    ];
    for (what, events) in cases {
        let w = world();
        w.journal.set_lines(chained(events));
        assert!(
            matches!(
                w.reopen(),
                Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
            ),
            "{what}"
        );
    }
}

#[test]
fn journal_an_effect_whose_outcome_was_not_recorded_blocks_its_mission() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.hold();
    let mother = w.mother.clone();
    let working = std::thread::spawn(move || mother.advance(id));
    assert!(w.port.wait_entered());
    // Mother "crashes" while the effect is with the port: the journal as it
    // stands now ends with the dispatch.
    let crashed = w.journal.lines();
    assert!(crashed.last().unwrap().contains("effect_dispatched"));
    w.mother.stop(id).unwrap();
    let _ = working.join();

    let after = world();
    after.journal.set_lines(crashed);
    let reopened = Coordinator::open(
        after.journal.clone(),
        after.port.clone(),
        Lanes::new(after.lanes.clone()).unwrap(),
        after.clock.clone(),
    )
    .unwrap();
    assert_eq!(reopened.state(id), Some(MissionState::Blocked));
    assert_eq!(
        reopened.reason(id).as_deref(),
        Some("effect_outcome_unknown")
    );
    assert!(reopened.advance(id).is_err());
    assert_eq!(after.port.effects(), vec![]);
    // The block is itself journaled.
    assert!(after
        .journal
        .lines()
        .last()
        .unwrap()
        .contains("effect_outcome_unknown"));
}

#[test]
fn journal_the_journal_records_no_lane_output_and_no_effect_content() {
    let w = world();
    let id = w.approved(contract_with(
        "TEXT-RESEARCHERA:OUTPUT-MARKER-55d0| ASK-BUILDER-NETWORK",
    ));
    w.port.reply(Ok(EffectOutcome::Done {
        text: Some("RESPONSE-MARKER-91c2".into()),
    }));
    for _ in 0..5 {
        w.mother.advance(id).unwrap();
    }
    for entry in w.mother.journal().unwrap() {
        if matches!(entry.event, Event::Drafted { .. }) {
            continue;
        }
        let line = entry.to_line();
        assert!(!line.contains("OUTPUT-MARKER"), "{line}");
        assert!(!line.contains("RESPONSE-MARKER"), "{line}");
        assert!(!line.contains("example.org"), "{line}");
    }
}
