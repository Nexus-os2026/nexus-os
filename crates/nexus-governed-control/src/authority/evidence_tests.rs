//! P0-C unit tests of the seal, its registries and the run's class (the
//! frozen behavioural oracles are in `privacy_tests.rs`).

use super::*;
use crate::authority::evidence::Privacy::{Owner, Restricted};
use crate::authority::halt::HaltReason;
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::{RunClass, RunOrigin, RunRegistry};
use crate::authority::{AuthorityError, REASONS};

fn record(privacy: Privacy) -> EvidenceRecord {
    let mut record = EvidenceRecord::new(EvidencePhase::Finished, 1, 1, privacy);
    record.target = Some("https://example.invalid/a".into());
    record.target_digest = Some("ab".repeat(32));
    record.detail = vec![
        ("status".into(), "200".into()),
        ("content_type".into(), "text/x".into()),
    ];
    record
}

#[test]
fn an_owner_record_is_kept_as_written() {
    let written = record(Owner);
    let sealed = written.sealed();
    assert_eq!(sealed.target, written.target);
    assert_eq!(sealed.detail, written.detail);
}

#[test]
fn a_restricted_record_is_sealed_once() {
    let sealed = record(Restricted).sealed();
    assert_eq!(sealed.target, None);
    assert_eq!(sealed.target_digest, Some("ab".repeat(32)));
    assert_eq!(
        sealed.detail,
        vec![
            ("status".to_string(), "200".to_string()),
            ("content_type_sha256".to_string(), text_digest("text/x")),
        ]
    );
    assert_eq!(
        sealed.sealed().detail,
        sealed.detail,
        "a sealed record is never sealed again"
    );
}

#[test]
fn the_registries_admit_exact_values_only() {
    for count in ["0", "200", "18446744073709551615"] {
        assert!(Kept::Count.admits(count), "{count}");
    }
    for not in ["", "007", "+1", "1a", " 1", "18446744073709551616"] {
        assert!(!Kept::Count.admits(not), "{not:?}");
    }
    let lease = format!("lease-{}", "0a".repeat(16));
    assert!(Kept::Lease.admits(&lease));
    for not in [
        lease.to_uppercase(),
        "0a".repeat(16),
        format!("lease-{}", "0a".repeat(15)),
    ] {
        assert!(!Kept::Lease.admits(&not), "{not}");
    }
    assert!(Kept::Door.admits(&"0a".repeat(16)));
    assert!(!Kept::Door.admits(&"0A".repeat(16)));
    assert!(!Kept::Door.admits(&"0a".repeat(15)));
    let code_shaped = "nxsecret_0123456789abcdef";
    for key in ["service", "reason", "class", "area", "op"] {
        let kept = registered(key).expect("a registered key");
        assert!(!kept.admits(code_shaped), "{key}: a code-shaped secret");
    }
    assert!(registered("reason").unwrap().admits("halted"));
    assert!(registered("service").unwrap().admits("Gmail"));
    assert!(!registered("service").unwrap().admits("gmail"));
    assert!(registered("detail").is_none());
}

/// Every refusal class is a registered reason (a new variant must be added
/// to the match below, and so to `REASONS`).
#[test]
fn every_authority_class_and_halt_reason_is_a_registered_reason() {
    use AuthorityError::*;
    let every = [
        UnknownCommitment,
        UnknownRun,
        UnknownGrant,
        UnknownLease,
        WrongAgent,
        WrongRun,
        NotPending,
        NotAuthorized,
        Expired,
        Stale,
        GrantNotLive,
        NoCoveringGrant,
        RunCancelled,
        RunNotActive,
        EmergencyStopped,
        ResumeSuperseded,
        ApprovalRequired,
        ApprovalNotApplicable,
        ApprovalMismatch,
        Declined,
        TargetChanged,
        ParametersChanged,
        EvidenceUnavailable,
        Capacity,
        NotMember,
        Halted,
        NotAdmitted,
        NoConsent,
        AlreadyAdmitted,
        FileExists,
        ChangeTooLargeToShow,
        UnsupportedMountTopology,
        RecoveryRequired,
        Closed(""),
        InvalidAction(""),
        Unavailable(""),
        Ended {
            cause: Box::new(Halted),
            evidence: ExecutionEvidence {
                start: 1,
                end: EndStatus::Owed,
            },
        },
    ];
    for error in &every {
        // Exhaustive: a variant added later fails to compile here.
        match error {
            UnknownCommitment
            | UnknownRun
            | UnknownGrant
            | UnknownLease
            | WrongAgent
            | WrongRun
            | NotPending
            | NotAuthorized
            | Expired
            | Stale
            | GrantNotLive
            | NoCoveringGrant
            | RunCancelled
            | RunNotActive
            | EmergencyStopped
            | ResumeSuperseded
            | ApprovalRequired
            | ApprovalNotApplicable
            | ApprovalMismatch
            | Declined
            | TargetChanged
            | ParametersChanged
            | EvidenceUnavailable
            | Capacity
            | NotMember
            | Halted
            | NotAdmitted
            | NoConsent
            | AlreadyAdmitted
            | FileExists
            | ChangeTooLargeToShow
            | UnsupportedMountTopology
            | RecoveryRequired
            | Closed(_)
            | InvalidAction(_)
            | Unavailable(_)
            | Ended { .. } => {}
        }
        assert!(REASONS.contains(&error.class()), "{}", error.class());
    }
    assert!(REASONS.contains(&HaltReason::JournalFailed.as_str()));
}

#[test]
fn every_catalog_credential_service_is_a_registered_service() {
    for connector in crate::connector::catalog::production() {
        for operation in &connector.operations {
            if let Some(spec) = operation.credential {
                assert!(SERVICES.contains(&spec.service), "{}", spec.service);
            }
        }
    }
}

#[test]
fn a_run_s_class_is_fixed_at_insertion_and_an_unknown_run_is_restricted() {
    let runs = RunRegistry::default();
    let agent = AgentId::new("agent").unwrap();
    let owner = runs
        .open(agent.clone(), RunOrigin::AgentGoal, RunClass::Owner, 1)
        .unwrap();
    let theirs = runs
        .open(agent, RunOrigin::AgentGoal, RunClass::Agent, 1)
        .unwrap();
    assert_eq!(runs.privacy(owner), Owner);
    assert_eq!(runs.privacy(theirs), Restricted);
    assert_eq!(runs.privacy(RunId::fresh()), Restricted, "unknown");
    runs.cancel(owner);
    runs.finish(theirs);
    assert_eq!(runs.privacy(owner), Owner, "an ended run keeps its class");
    assert_eq!(runs.privacy(theirs), Restricted);
}

#[test]
fn a_door_s_mission_labels_are_mother_s_exactly() {
    for id in [
        "msn-000001",
        "msn-123456",
        "msn-1234567",
        "msn-18446744073709551615",
    ] {
        assert!(is_mission_id(id), "{id}");
    }
    for not in [
        "msn-00001",
        "msn-0000001",
        "msn-00000a",
        "MSN-000001",
        "m-1",
        "msn-",
        "msn-18446744073709551616",
        "nxsecret_0123456789abcdef",
    ] {
        assert!(!is_mission_id(not), "{not}");
    }
    assert!(is_mission_request("msn-000001/2/1"));
    for not in [
        "msn-000001/2",
        "msn-000001/2/1/0",
        "msn-000001/02/1",
        "msn-000001/2/",
        "m-1/2/1",
        "msn-000001/2/1 ",
        "msn-000001/1/NXCANARY-00",
    ] {
        assert!(!is_mission_request(not), "{not}");
    }
    assert!(is_held_reason("unclean_shutdown"));
    assert!(!is_held_reason("Unclean_shutdown"));
    for (key, kept, not) in [
        ("contract_hash", "ab".repeat(32), "AB".repeat(32)),
        ("summary_digest", "ab".repeat(32), "ab".repeat(31)),
        ("mission", "msn-000001".into(), "msn-1".into()),
        ("request", "msn-000001/2/1".into(), "msn-000001/x/1".into()),
        ("held_reason", "unclean_shutdown".into(), "held".into()),
    ] {
        let registered = registered(key).expect("a registered key");
        assert!(registered.admits(&kept), "{key}: {kept}");
        assert!(!registered.admits(&not), "{key}: {not}");
    }
}
