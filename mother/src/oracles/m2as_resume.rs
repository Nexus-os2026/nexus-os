//! M2a-S Codex REJECT MAJOR (oracle first): a mission resumed and then
//! held again by a later unclean session has that new hold journaled too,
//! so the journal keeps reopening and the holds are right at every step.

use super::m2as_durability::*;
use super::*;
use crate::journal::{verify, Event};
use MissionState::*;

/// How many holds and resumes of `id` the journal has, checking that each
/// resume follows a hold.
fn holds_and_resumes(w: &World, id: MissionId) -> (usize, usize) {
    let (mut holds, mut resumes, mut held) = (0, 0, false);
    for entry in verify(&w.journal.lines()).unwrap() {
        if entry.mission != id {
            continue;
        }
        match entry.event {
            Event::Held { .. } => {
                assert!(!held, "a hold of a held mission");
                held = true;
                holds += 1;
            }
            Event::Resumed => {
                assert!(held, "a resume with no hold before it");
                held = false;
                resumes += 1;
            }
            _ => {}
        }
    }
    (holds, resumes)
}

#[test]
fn m2as_resume_crash_resume_crash_resume_close_reopens_with_the_right_holds() {
    let w = world();
    let id = reach(&w, Building);
    let mut w = w;
    for round in 1..=3 {
        w = crash(&w);
        assert_eq!(
            w.mother.held(id).as_deref(),
            Some("unclean_shutdown"),
            "{round}"
        );
        assert_eq!(w.mother.resume(id), Ok(Building), "{round}");
        assert_eq!(w.mother.held(id), None, "{round}");
        assert_eq!(holds_and_resumes(&w, id), (round, round), "{round}");
    }
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    assert_eq!(w.mother.held(id), None);
    assert_eq!(w.mother.advance(id).unwrap().to, Verifying);
}

#[test]
fn m2as_resume_a_hold_after_a_resume_is_journaled_before_a_clean_close() {
    let w = world();
    let id = reach(&w, Building);
    let w = crash(&w);
    assert_eq!(w.mother.resume(id), Ok(Building));
    // Held again, and closed before anything else is written.
    let w = crash(&w);
    assert_eq!(w.mother.held(id).as_deref(), Some("unclean_shutdown"));
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    assert_eq!(
        w.mother.held(id).as_deref(),
        Some("unclean_shutdown"),
        "the hold survives the clean close"
    );
    assert_eq!(holds_and_resumes(&w, id), (2, 1));
    // And the Owner's Stop of it holds.
    assert_eq!(w.mother.stop(id), Ok(Stopped));
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    assert_eq!(w.mother.state(id), Some(Stopped));
}
