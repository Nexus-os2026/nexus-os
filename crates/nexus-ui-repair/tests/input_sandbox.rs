//! Hole A Layer 2 structural tests. See v1.1 amendment §3.1.
//!
//! The denial test is load-bearing: it must exercise the real
//! `AppGrantManager` code path, not just compare strings. See the
//! module docs in `src/governance/input_sandbox.rs` for the strategy.

use nexus_computer_use::governance::app_registry::{AppCategory, AppInfo};
use nexus_ui_repair::governance::InputSandbox;
use nexus_ui_repair::Error;

fn make_app(wm_class: &str, category: AppCategory) -> AppInfo {
    AppInfo {
        name: format!("Test-{}", wm_class),
        wm_class: wm_class.to_string(),
        pid: 1234,
        window_id: 0x0100_0001,
        title: format!("{} window", wm_class),
        category,
        is_focused: true,
    }
}

#[test]
fn permits_whitelisted_nexus_os_window() {
    let mut sandbox = InputSandbox::for_nexus_os_window("nexus-os-scout-phase-1-3", Some(1234));
    // Use a category whose auto-grant would otherwise deny clicks — so
    // that the permit can only come from the explicit Full grant.
    let target = make_app("nexus-os-scout-phase-1-3", AppCategory::Communication);
    sandbox
        .validate_target_window(&target)
        .expect("whitelisted window must be permitted");
}

#[test]
fn refuses_non_whitelisted_window() {
    let mut sandbox = InputSandbox::for_nexus_os_window("nexus-os-scout-phase-1-3", Some(1234));
    // A Communication app that does NOT contain the whitelisted
    // wm_class. The explicit Full grant will not match (substring
    // lookup), and the Communication auto-grant is ReadOnly, which
    // denies MouseClick — so the probe comes back CapabilityDenied.
    let target = make_app("firefox", AppCategory::Communication);
    let result = sandbox.validate_target_window(&target);
    match result {
        Err(Error::InvariantViolation(msg)) => {
            assert!(
                msg.contains("firefox"),
                "denial message must name the target wm_class: {}",
                msg
            );
            assert!(
                msg.contains("not whitelisted"),
                "denial message must say `not whitelisted`: {}",
                msg
            );
        }
        other => panic!("expected InvariantViolation, got {:?}", other),
    }
}

#[test]
fn stores_pid_for_phase_1_3_5_use() {
    let sandbox = InputSandbox::for_nexus_os_window("nexus-os", Some(9876));
    assert_eq!(sandbox.allowed_wm_class(), "nexus-os");
    assert_eq!(sandbox.allowed_pid(), Some(9876));

    let sandbox_no_pid = InputSandbox::for_nexus_os_window("nexus-os", None);
    assert_eq!(sandbox_no_pid.allowed_pid(), None);
}
