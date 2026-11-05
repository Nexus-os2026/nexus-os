//! A12: primitive Owner answers, bytes and verified journals select expectations.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use nexus_mother::journal::{verify, Event, JournalStore};
use nexus_mother::state::Trigger;
use support::{Fixture, Pty};

#[test]
fn session_result_envelope_preserves_answer_physical_records_and_exact_ack() {
    for affirmative in [false, true] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("result oracle\n"));
        p.ready();
        p.mission();
        p.expect("ANSWERS grant=Affirmative door=Affirmative mission=Affirmative action=NotAsked undo=NotAsked\n");
        p.expect("OPERATION END\n");
        p.line("approve");
        let code = p.code();
        p.line(if affirmative { &code } else { "no" });
        p.expect(if affirmative {
            "MISSION done\n"
        } else {
            "ACTION DENIED\n"
        });
        p.expect(if affirmative {
            "answer=Affirmative"
        } else {
            "answer=Declined"
        });
        p.expect(if affirmative {
            "physical=Published"
        } else {
            "physical=ConfirmedNotPublished"
        });
        p.expect("mother=Recorded acknowledgement=Verified");
        p.expect("OPERATION END\n");
        assert_eq!(
            std::fs::read(f.workspace.join("hello.txt")).ok(),
            affirmative.then(|| b"result oracle\n".to_vec())
        );
        let journal =
            nexus_mother::adapter::FileJournal::open(&f.home.join("mother-journal.jsonl")).unwrap();
        let entries = verify(&journal.load().unwrap()).unwrap();
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(
                    &e.event,
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    }
                ))
                .count(),
            usize::from(affirmative)
        );
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(
                    &e.event,
                    Event::Transition {
                        trigger: Trigger::ActionDenied { .. }
                    }
                ))
                .count(),
            usize::from(!affirmative)
        );
        if affirmative {
            p.line("undo");
            p.yes();
            p.expect("UNDO REMOVED");
            p.expect("answer=Affirmative");
            p.expect("restore=Completed grant=Closed");
            p.expect("OPERATION END\n");
            assert!(!f.workspace.join("hello.txt").exists());
        }
        p.quit();
    }
}
