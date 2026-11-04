//! Spec §§1.1–1.4, 3.4, B A1/A5/A6/A9. Registered before the facade.
#![deny(unsafe_code)]
use nexus_governed_control::host_session::{CommandKind, SessionError, SessionSeed, SessionState};
use std::time::{Duration, Instant};

#[test]
fn original_identity_precedes_bootstrap_and_retained_ingress_observes_end() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let identity = seed.identity();
    let request = ingress.accept(CommandKind::Run, 1).unwrap();
    assert_eq!(request.session(), identity);
    assert_eq!(seed.state(), SessionState::Bootstrap);
    let first = ingress.emergency().unwrap();
    assert_eq!(first.generation(), 1);
    assert_eq!(seed.identity(), identity);
    assert_eq!(seed.state(), SessionState::Emergency);
    assert!(matches!(
        ingress.accept(CommandKind::Run, 2),
        Err(SessionError::Restricted)
    ));
    assert!(ingress.accept(CommandKind::Status, 3).is_ok());
    assert!(ingress.accept(CommandKind::Evidence, 4).is_ok());
    assert!(matches!(
        ingress.accept(CommandKind::Lift, 5),
        Err(SessionError::Unavailable)
    ));
}

#[test]
fn input_context_cannot_bind_an_approval_or_undo_to_a_future_target() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    assert!(matches!(
        ingress.accept(CommandKind::Approve, 1),
        Err(SessionError::NoTarget)
    ));
    assert!(matches!(
        ingress.accept(CommandKind::Undo, 2),
        Err(SessionError::NoTarget)
    ));
    assert!(matches!(
        ingress.accept(CommandKind::Run, 2),
        Err(SessionError::Sequence)
    ));
}

#[test]
fn first_quit_closes_immediately_and_repeated_quit_keeps_original_deadline() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let before = Instant::now();
    let quit = ingress.accept(CommandKind::Quit, 1).unwrap();
    let after = Instant::now();
    assert_eq!(seed.state(), SessionState::ClosingNormal);
    let deadline = quit.deadline();
    assert!(deadline >= before + Duration::from_secs(3));
    assert!(deadline <= after + Duration::from_secs(3));
    assert!(matches!(
        ingress.accept(CommandKind::Run, 2),
        Err(SessionError::Restricted)
    ));
    let repeated = ingress.accept(CommandKind::Quit, 3).unwrap();
    assert_eq!(repeated.close_identity(), quit.close_identity());
    assert_eq!(repeated.deadline(), deadline);
    ingress.emergency().unwrap();
    assert_eq!(seed.state(), SessionState::ClosingRestricted);
    assert_eq!(seed.close_deadline(), Some(deadline));
}

#[test]
fn fatal_and_emergency_are_independent_monotone_restrictions() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    ingress.fail();
    assert_eq!(seed.state(), SessionState::Failed);
    ingress.emergency().unwrap();
    assert!(seed.failed());
    assert!(seed.emergency_seen());
    ingress.accept(CommandKind::Quit, 1).unwrap();
    assert_eq!(seed.state(), SessionState::ClosingRestricted);
    assert!(seed.failed());
}
