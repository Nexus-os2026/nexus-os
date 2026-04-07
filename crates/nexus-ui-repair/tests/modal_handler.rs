//! Hole B Layer 2 tests. See v1.1 amendment §6.5.

use std::path::PathBuf;

use nexus_ui_repair::specialists::modal_handler::{ModalAction, ModalHandler, ModalKind};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("read fixture")
}

#[test]
fn classifies_confirmation_modal() {
    let html = read_fixture("teams_with_delete_modal.html");
    let handler = ModalHandler::new();
    assert_eq!(handler.classify_modal(&html), ModalKind::Confirmation);
}

#[test]
fn classifies_no_cancel_modal_as_confirmation() {
    let html = read_fixture("teams_modal_no_cancel.html");
    let handler = ModalHandler::new();
    assert_eq!(handler.classify_modal(&html), ModalKind::Confirmation);
}

#[test]
fn classifies_unrecognized_modal_as_unrecognized() {
    let html = read_fixture("teams_unrecognized_modal.html");
    let handler = ModalHandler::new();
    assert_eq!(handler.classify_modal(&html), ModalKind::Unrecognized);
}

#[test]
fn confirmation_with_cancel_clicks_cancel() {
    let html = read_fixture("teams_with_delete_modal.html");
    let mut handler = ModalHandler::new();
    let action = handler.decide_action(ModalKind::Confirmation, &html);
    match action {
        ModalAction::ClickCancel { control_id } => {
            assert_eq!(control_id, "cancel-delete");
        }
        other => panic!("expected ClickCancel, got {:?}", other),
    }
}

#[test]
fn confirmation_without_cancel_triggers_hitl() {
    let html = read_fixture("teams_modal_no_cancel.html");
    let mut handler = ModalHandler::new();
    let action = handler.decide_action(ModalKind::Confirmation, &html);
    match action {
        ModalAction::Hitl { reason } => {
            assert!(
                reason.contains("no cancel"),
                "reason must mention 'no cancel', got: {}",
                reason
            );
        }
        other => panic!("expected Hitl, got {:?}", other),
    }
}

#[test]
fn unrecognized_modal_increments_counter_and_halts_on_third() {
    let html = read_fixture("teams_unrecognized_modal.html");
    let mut handler = ModalHandler::new();

    // First: HITL.
    let a1 = handler.decide_action(ModalKind::Unrecognized, &html);
    assert!(matches!(a1, ModalAction::Hitl { .. }), "first: {:?}", a1);

    // Second: HITL.
    let a2 = handler.decide_action(ModalKind::Unrecognized, &html);
    assert!(matches!(a2, ModalAction::Hitl { .. }), "second: {:?}", a2);

    // Third: HALT.
    let a3 = handler.decide_action(ModalKind::Unrecognized, &html);
    match a3 {
        ModalAction::Halt { reason } => {
            assert!(reason.contains('3'), "reason should cite count: {}", reason);
            assert!(
                reason.contains("threshold"),
                "reason should cite threshold: {}",
                reason
            );
        }
        other => panic!("expected Halt on third unrecognized, got {:?}", other),
    }
}

#[test]
fn login_and_error_modals_route_to_hitl() {
    let login_html =
        r#"<div role="dialog"><input type="password" /><button>Sign in</button></div>"#;
    let error_html =
        r#"<div role="dialog"><p>An error occurred: request failed.</p><button>OK</button></div>"#;

    let handler = ModalHandler::new();
    assert_eq!(handler.classify_modal(login_html), ModalKind::Login);
    assert_eq!(handler.classify_modal(error_html), ModalKind::Error);

    let mut h = ModalHandler::new();
    assert!(matches!(
        h.decide_action(ModalKind::Login, login_html),
        ModalAction::Hitl { .. }
    ));
    assert!(matches!(
        h.decide_action(ModalKind::Error, error_html),
        ModalAction::Hitl { .. }
    ));
}
