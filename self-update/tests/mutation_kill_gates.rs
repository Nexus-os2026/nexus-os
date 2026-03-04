use nexus_kernel::kill_gates::GateStatus;
use nexus_self_update::mutation::{
    MutationError, MutationLifecycle, ReplayCase, ReplayExpectation,
};

#[test]
fn test_mutation_freezes_on_replay_mismatch() {
    let mut lifecycle = MutationLifecycle::new();
    let patch_id = lifecycle
        .propose(
            r#"
config.request_timeout_ms = "2500"
"#,
            "human.researcher",
        )
        .expect("propose should succeed");

    lifecycle
        .validate(patch_id.as_str())
        .expect("validate should succeed");

    let replay = lifecycle.replay_ab(
        patch_id.as_str(),
        &[ReplayCase {
            name: "timeout mismatch".to_string(),
            expectation: ReplayExpectation::ConfigEquals {
                key: "request_timeout_ms".to_string(),
                expected: "1000".to_string(),
            },
        }],
    );

    assert!(matches!(replay, Err(MutationError::ReplayFailed(_))));
    assert_eq!(lifecycle.mutation_gate_status(), Some(GateStatus::Frozen));

    let apply = lifecycle.apply(patch_id.as_str());
    assert_eq!(apply, Err(MutationError::KillGateFrozen("mutation")));
}
