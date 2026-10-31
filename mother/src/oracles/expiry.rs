//! I6 consent expires.

use super::fake::Call;
use super::*;
use crate::coordinator::{CoordinatorError, MissionState};
use std::time::Duration;

#[test]
fn m1_i6_consent_lasts_until_just_before_its_lifetime_ends() {
    let s = setup();
    let id = s.approved(one_step());
    s.clock.advance(Duration::from_millis(TTL_MS - 1));
    assert!(s.mother.step(id).is_ok());
    assert_eq!(s.mother.state(id), Some(MissionState::Completed));
}

#[test]
fn m1_i6_a_step_after_the_consent_lifetime_expires_the_mission() {
    let s = setup();
    let id = s.approved(two_steps());
    s.clock.advance(Duration::from_millis(TTL_MS));
    assert_eq!(s.mother.step(id), Err(CoordinatorError::Expired));
    assert_eq!(s.mother.state(id), Some(MissionState::Expired));
    assert_eq!(s.broker.effect_calls(), vec![]);
    assert!(s.mother.step(id).is_err());
    assert_eq!(s.broker.effect_calls(), vec![]);
}

#[test]
fn m1_i6_expiry_cancels_an_open_run() {
    let s = setup();
    let id = s.approved(two_steps());
    s.mother.step(id).unwrap();
    let run = s.mother.run_of(id).unwrap();
    s.clock.advance(Duration::from_millis(TTL_MS));
    assert_eq!(s.mother.step(id), Err(CoordinatorError::Expired));
    assert_eq!(s.mother.state(id), Some(MissionState::Expired));
    assert!(s.broker.calls().contains(&Call::Cancel(run)));
    assert_eq!(s.broker.acts(), 1);
}

#[test]
fn m1_i6_the_lifetime_runs_from_consent_not_from_drafting() {
    let s = setup();
    let id = s.mother.draft(one_step()).unwrap();
    s.clock.advance(Duration::from_millis(TTL_MS * 10));
    s.mother.request_consent(id, &Yes).unwrap();
    assert!(s.mother.step(id).is_ok());
}
