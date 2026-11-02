//! M2a-S, oracle added after the gate review: Mother's production code
//! used the port's whole broker Authority (`EffectPort::broker`) only to put
//! a mission to the Owner.
//!
//! M2c-A (design §7.4): the port has no broker handle any more; the
//! type-level absence of `broker()` (A1-c, `lib.rs`) replaces the count
//! this scan made. What remains: no production file names it. Read at
//! compile time: no host access.

const PRODUCTION: &[(&str, &str)] = &[
    ("coordinator.rs", include_str!("../coordinator.rs")),
    ("consent.rs", include_str!("../consent.rs")),
    ("contract.rs", include_str!("../contract.rs")),
    ("effect.rs", include_str!("../effect.rs")),
    ("journal.rs", include_str!("../journal.rs")),
    ("lane.rs", include_str!("../lane.rs")),
    ("state.rs", include_str!("../state.rs")),
    ("lib.rs", include_str!("../lib.rs")),
    ("adapter/mod.rs", include_str!("mod.rs")),
];

#[test]
fn m2as_scan_the_unfenced_broker_authority_is_used_only_for_mission_consent() {
    for (name, text) in PRODUCTION {
        assert!(
            !text.contains(".broker()"),
            "{name}: a broker Authority handle in Mother's production code"
        );
    }
}
