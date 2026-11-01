//! M2a-S, oracle added after the gate review (it does not replace or edit
//! the earlier ones): the port's whole broker Authority (`EffectPort::broker`)
//! is not behind the gate, so Mother's production code may use it only to
//! put a mission to the Owner (`confirm_mission`, no effect; an answer that
//! comes after a break is discarded). Read at compile time: no host access.

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
fn m2as_scan_the_ungated_broker_authority_is_used_only_for_mission_consent() {
    let mut uses = 0;
    for (name, text) in PRODUCTION {
        for (at, _) in text.match_indices(".broker()") {
            let rest = text[at + ".broker()".len()..].trim_start();
            assert!(
                rest.starts_with(".confirm_mission("),
                "{name}: the broker Authority used for something else than mission consent: {}",
                &text[at..(at + 80).min(text.len())]
            );
            uses += 1;
        }
    }
    assert_eq!(
        uses, 1,
        "Mother asks the Owner about a mission in one place"
    );
}
