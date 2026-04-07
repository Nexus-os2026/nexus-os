//! Hole B Layer 3 tests. See v1.1 amendment §6.5.

use nexus_ui_repair::descriptors::{DestructiveOptIn, FixtureKind, FixtureRef, PageDescriptor};
use nexus_ui_repair::Error;

fn make_valid_descriptor() -> PageDescriptor {
    PageDescriptor {
        route: "/builder/projects".into(),
        expected_elements: None,
        critical_flows: None,
        fixtures: Some(vec![FixtureRef {
            id: "throwaway_project".into(),
            kind: FixtureKind::Throwaway,
        }]),
        destructive_opt_ins: Some(vec![DestructiveOptIn {
            element_id: "delete-project-btn".into(),
            fixture_required: true,
            fixture_id: "throwaway_project".into(),
        }]),
    }
}

#[test]
fn valid_descriptor_passes() {
    let d = make_valid_descriptor();
    d.validate().expect("valid descriptor must pass");
}

#[test]
fn descriptor_without_optins_passes_even_on_protected_route() {
    // The rule is "opt-ins are forbidden on protected routes", not
    // "descriptors are forbidden on protected routes". A descriptor
    // for /settings without opt-ins is fine.
    let d = PageDescriptor {
        route: "/settings".into(),
        expected_elements: None,
        critical_flows: None,
        fixtures: None,
        destructive_opt_ins: None,
    };
    d.validate()
        .expect("no-optin descriptor on protected route must pass");
}

#[test]
fn optin_on_settings_route_is_rejected() {
    let mut d = make_valid_descriptor();
    d.route = "/settings/danger".into();
    match d.validate() {
        Err(Error::InvariantViolation(msg)) => {
            assert!(msg.contains("/settings"));
            assert!(msg.contains("protected"));
        }
        other => panic!("expected InvariantViolation, got {:?}", other),
    }
}

#[test]
fn optin_on_governance_route_is_rejected() {
    let mut d = make_valid_descriptor();
    d.route = "/governance/oracle".into();
    assert!(matches!(d.validate(), Err(Error::InvariantViolation(_))));
}

#[test]
fn optin_on_memory_route_is_rejected() {
    let mut d = make_valid_descriptor();
    d.route = "/memory/wipe".into();
    assert!(matches!(d.validate(), Err(Error::InvariantViolation(_))));
}

#[test]
fn optin_without_matching_fixture_is_rejected() {
    let mut d = make_valid_descriptor();
    d.fixtures = Some(vec![]); // empty fixture list
    match d.validate() {
        Err(Error::InvariantViolation(msg)) => {
            assert!(msg.contains("missing fixture"));
        }
        other => panic!("expected InvariantViolation, got {:?}", other),
    }
}

#[test]
fn optin_with_persistent_fixture_is_rejected() {
    let mut d = make_valid_descriptor();
    d.fixtures = Some(vec![FixtureRef {
        id: "throwaway_project".into(),
        kind: FixtureKind::Persistent, // wrong kind
    }]);
    match d.validate() {
        Err(Error::InvariantViolation(msg)) => {
            assert!(msg.contains("non-throwaway"));
        }
        other => panic!("expected InvariantViolation, got {:?}", other),
    }
}

#[test]
fn optin_with_fixture_required_false_is_rejected() {
    let mut d = make_valid_descriptor();
    if let Some(opt_ins) = d.destructive_opt_ins.as_mut() {
        opt_ins[0].fixture_required = false;
    }
    match d.validate() {
        Err(Error::InvariantViolation(msg)) => {
            assert!(msg.contains("fixture_required"));
        }
        other => panic!("expected InvariantViolation, got {:?}", other),
    }
}
