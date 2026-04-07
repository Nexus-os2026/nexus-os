//! Destructive Action Policy acceptance tests. See v1.1 amendment §6.5.
//!
//! Phase 1.3 implements all five acceptance cases. Cases 1 (pattern
//! denylist) carried over from Phase 1.2. Cases 2 and 3 exercise the
//! modal handler (Hole B Layer 2). Cases 4 and 5 exercise page
//! descriptor opt-in validation (Hole B Layer 3).

use std::path::PathBuf;

use nexus_ui_repair::descriptors::{DestructiveOptIn, FixtureKind, FixtureRef, PageDescriptor};
use nexus_ui_repair::specialists::destructive_policy::is_destructive_label;
use nexus_ui_repair::specialists::modal_handler::{ModalAction, ModalHandler, ModalKind};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_1_pattern_denylist_matches_all_destructive_labels() {
    for label in [
        "Delete team",
        "Remove member",
        "Reset settings",
        "Wipe cache",
        "Revoke access",
        "Destroy project",
        "Purge logs",
        "Drop database",
        "Clear all data",
        "Factory reset",
        "Uninstall plugin",
        "Forget device",
        "Erase history",
    ] {
        assert!(is_destructive_label(label), "should match: {}", label);
    }

    for label in [
        "Edit team",
        "Save",
        "Submit",
        "Cancel",
        "View details",
        "Open settings",
        "Close",
    ] {
        assert!(!is_destructive_label(label), "should NOT match: {}", label);
    }
}

#[test]
fn test_2_confirmation_modal_no_cancel_triggers_hitl() {
    let html = std::fs::read_to_string(fixture_path("teams_modal_no_cancel.html"))
        .expect("fixture must exist");
    let mut handler = ModalHandler::new();
    let kind = handler.classify_modal(&html);
    assert_eq!(kind, ModalKind::Confirmation);
    let action = handler.decide_action(kind, &html);
    match action {
        ModalAction::Hitl { reason } => {
            assert!(reason.contains("no cancel"), "reason: {}", reason);
        }
        other => panic!("expected Hitl, got {:?}", other),
    }
}

#[test]
fn test_3_confirmation_modal_with_cancel_clicks_cancel() {
    let html = std::fs::read_to_string(fixture_path("teams_with_delete_modal.html"))
        .expect("fixture must exist");
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
fn test_4_optin_without_fixture_is_ignored() {
    let d = PageDescriptor {
        route: "/builder/projects".into(),
        expected_elements: None,
        critical_flows: None,
        fixtures: Some(vec![]), // no matching fixture
        destructive_opt_ins: Some(vec![DestructiveOptIn {
            element_id: "delete-project-btn".into(),
            fixture_required: true,
            fixture_id: "missing_fixture".into(),
        }]),
    };
    assert!(
        d.validate().is_err(),
        "opt-in with no matching fixture must be rejected"
    );
}

#[test]
fn test_5_optin_for_protected_path_is_rejected() {
    let d = PageDescriptor {
        route: "/settings".into(),
        expected_elements: None,
        critical_flows: None,
        fixtures: Some(vec![FixtureRef {
            id: "throwaway".into(),
            kind: FixtureKind::Throwaway,
        }]),
        destructive_opt_ins: Some(vec![DestructiveOptIn {
            element_id: "reset-btn".into(),
            fixture_required: true,
            fixture_id: "throwaway".into(),
        }]),
    };
    assert!(
        d.validate().is_err(),
        "opt-in on protected /settings route must be rejected"
    );
}
