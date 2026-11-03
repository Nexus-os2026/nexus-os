//! M2d-C round 5, decisions D3 and D4(c), against the real broker,
//! pre-registered before the repairs (run after `m2d_files_r5`, with its
//! scenario).
//!
//! - D3: Mother's own deadline passes while the broker's commitment is
//!   still valid and authorized. Withdrawn first, then read once: a File
//!   the Owner published just before the withdrawal is recorded truthfully
//!   (a Block, never an ordinary lapse); with the lapse's journal write
//!   held instead, the commitment can no longer be executed and the lapse
//!   is the truth.
//! - D4(c): a crash after the ordinary `ActionApproved` is durable and
//!   before its acknowledgement: the replacement door's startup never
//!   adopts the action (no membership, no view), and acknowledges it.

use super::m2c_support::Home;
use super::m2d_files_r5::Scenario;
use super::mother_agent;
use crate::journal::Event;
use crate::state::{MissionState, Trigger};
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(30);

pub(super) fn on_the_real_broker(home: &Home) {
    d3_a_file_published_before_the_withdrawal_is_recorded(home);
    d3_with_the_lapse_write_held_nothing_executes(home);
    d4c_a_replacement_door_acknowledges_an_approved_file(home);
}

fn lapsed(s: &Scenario<'_>, id: crate::journal::MissionId) -> bool {
    s.mother.journal().unwrap().into_iter().any(|e| {
        e.mission == id
            && matches!(
                e.event,
                Event::Transition {
                    trigger: Trigger::ApprovalLapsed { .. }
                }
            )
    })
}

fn d3_a_file_published_before_the_withdrawal_is_recorded(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r5-d3.jsonl");
    let id = s.waiting("r5-d3");
    let (run, commitment) = s.approve(id);
    s.port.set_deadline_for_test(id, 0);
    let control = home.control.clone();
    s.port.before_withdraw.arm(0, move || {
        control
            .execute(commitment, &mother_agent(), run)
            .expect("still valid at the broker");
    });
    assert_eq!(s.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert!(s.ws.join("r5-d3-a.txt").exists(), "published");
    assert!(!lapsed(&s, id), "never an ordinary lapse");
    assert_eq!(
        s.mother.reason(id).as_deref(),
        Some("file_publication_interrupted:approval_deadline")
    );
}

fn d3_with_the_lapse_write_held_nothing_executes(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r5-d3-held.jsonl");
    let id = s.waiting("r5-d3-held");
    let poised = s.approve(id);
    s.port.set_deadline_for_test(id, 0);
    let (entered, release) = s.hold_next_append();
    std::thread::scope(|scope| {
        let advancing = scope.spawn(|| s.mother.advance(id));
        entered.recv_timeout(BOUND).expect("the lapse's write held");
        assert!(!s.execute(poised), "withdrawn before the write");
        release.send(()).unwrap();
        let _ = advancing.join().unwrap();
    });
    assert!(!s.ws.join("r5-d3-held-a.txt").exists());
    assert_eq!(s.mother.reason(id).as_deref(), Some("approval_expired"));
    assert!(lapsed(&s, id), "confirmed not published: the lapse");
}

fn d4c_a_replacement_door_acknowledges_an_approved_file(home: &Home) {
    let journal = "m2d-c-r5-d4c.jsonl";
    let s = Scenario::new(home, journal);
    let id = s.waiting("r5-d4c");
    let action = s.approve(id);
    assert!(s.execute(action), "published");
    // advance's first step, the wait's keeping step, then ActionApproved:
    // the crash at that step's owed write, before the acknowledgement.
    s.mother
        .hooks
        .before_owed_write
        .arm(2, || panic!("a crash after ActionApproved (test)"));
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.advance(id)));
    assert!(crashed.is_err());
    assert_eq!(s.port.acks_tried(), 0, "not acknowledged before the crash");
    drop(s);
    let again = Scenario::new(home, journal);
    assert_eq!(again.port.adopts_tried(), 0, "never adopted");
    assert_eq!(again.port.run_of(id), None, "no membership");
    assert_eq!(again.port.acks_tried(), 1, "acknowledged on startup");
    assert_eq!(again.port.unacked(), 0, "by ack-only ownership");
}
