//! One placeholder test per v1.1 invariant. Phase 1.1 just locks in
//! the slots; Phase 1.2 fills each test with real per-invariant logic.

use nexus_ui_repair::governance::invariants::{Invariant, InvariantRegistry};

#[test]
fn i1_kernel_allowlist() {
    let _ = Invariant::I1KernelAllowlist;
    let r = InvariantRegistry::new();
    assert!(r.check_all().is_ok());
}

#[test]
fn i2_read_only_filesystem() {
    let _ = Invariant::I2ReadOnlyFilesystem;
    let r = InvariantRegistry::new();
    assert!(r.check_all().is_ok());
}

#[test]
fn i3_hitl_by_definition() {
    let _ = Invariant::I3HitlByDefinition;
    let r = InvariantRegistry::new();
    assert!(r.check_all().is_ok());
}

#[test]
fn i4_immutable_provider_routing() {
    let _ = Invariant::I4ImmutableProviderRouting;
    let r = InvariantRegistry::new();
    assert!(r.check_all().is_ok());
}

#[test]
fn i5_replayable_sessions() {
    let _ = Invariant::I5ReplayableSessions;
    let r = InvariantRegistry::new();
    assert!(r.check_all().is_ok());
}
