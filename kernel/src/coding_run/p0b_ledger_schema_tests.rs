//! P0-B: tests of the ledger schema itself, written with the
//! implementation. The pre-registered oracles are in
//! `p0b_ledger_privacy_tests.rs`; this file reuses their scenarios to re-check
//! every recorded payload against its schema, and checks the code registries
//! and the class rules directly.

use super::p0b_ledger_privacy::{
    o10_worker_rejected_paths, o11_apply_and_restore, o12_runs_snapshots_reads_edits_and_pins,
    o24_remaining_producers, Canary,
};
use super::*;
use crate::coding_run::ledger::{
    check, pin_facts, schema, Class, EventKind, PROVIDERS, VERIFIER_EXITS,
};
use crate::coding_run::local_model::pin_for_test_at;
use crate::coding_run::review::ChangeKind;
use std::collections::BTreeSet;

fn all_kinds() -> Vec<EventKind> {
    use EventKind::*;
    let all = vec![
        RunCreated,
        RunGranted,
        OperationPrepared,
        OperationOutcome,
        EditRejected,
        StructuralVerification,
        RunCancelled,
        RunRevoked,
        RunFailed,
        RecoveryRequired,
        StagingDiscarded,
        ModelPinned,
        WorkerStarted,
        WorkerTurn,
        WorkerRead,
        ProposalRejected,
        WorkerFinished,
        ReviewComputed,
        CandidateWithdrawn,
        ApprovalGranted,
        ApprovalDeclined,
        ApplyRejected,
        ApplyPrepared,
        ApplyOp,
        ApplyCompleted,
        ApplyRolledBack,
        ApplyRecoveryRequired,
        RestoreApproved,
        RestoreDeclined,
        RestoreRejected,
        RestorePrepared,
        RestoreCompleted,
        VerificationPrepared,
        VerificationApproved,
        VerificationDeclined,
        VerificationLaunched,
        VerificationFinished,
        VerificationCleanup,
        PageInertCheck,
        PageInertRecheck,
    ];
    assert_eq!(
        all.iter()
            .map(|k| k.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        all.len()
    );
    all
}

fn kind_named(name: &str) -> EventKind {
    all_kinds()
        .into_iter()
        .find(|kind| kind.as_str() == name)
        .unwrap_or_else(|| panic!("no event kind {name}"))
}

/// Every payload every P0-B scenario records satisfies its schema when it
/// is checked again, independently of the append that admitted it.
#[test]
fn p0b_schema_every_recorded_payload_is_valid_under_its_schema() {
    let canary = Canary::new();
    let mut labs = o10_worker_rejected_paths(&canary);
    labs.extend(o11_apply_and_restore(&canary));
    labs.extend(o12_runs_snapshots_reads_edits_and_pins(&canary));
    labs.extend(o24_remaining_producers(&canary));
    let mut checked = 0;
    for lab in &labs {
        for rec in lab.records() {
            let kind = kind_named(&rec.record.event_kind);
            assert!(
                check(kind, &rec.payload).is_ok(),
                "{}: {} is not valid under its schema: {}",
                lab.name,
                rec.record.event_kind,
                rec.record.payload
            );
            checked += 1;
        }
    }
    assert!(checked > 300, "the scenarios recorded {checked} events");
}

#[test]
fn p0b_schema_classes_admit_only_their_values() {
    let digest = "0123456789abcdef".repeat(4);
    let id = Uuid::new_v4().to_string();
    let admitted = [
        (
            EventKind::WorkerRead,
            json!({"path_sha256": digest, "path_bytes": 1}),
        ),
        (EventKind::VerificationFinished, json!({"detail": -3})),
        (EventKind::VerificationFinished, json!({"detail": u64::MAX})),
        (EventKind::ApplyOp, json!({"ok": false})),
        (EventKind::RunGranted, json!({"staging_grant": id})),
        (EventKind::RunRevoked, json!({"reason": "EmergencyStop"})),
        (EventKind::RunGranted, json!({"staging_grant": null})),
        (EventKind::ReviewComputed, json!({"verification": null})),
        (EventKind::ApplyPrepared, json!({"files": []})),
        (EventKind::OperationOutcome, json!({"facts": {}})),
        (EventKind::RunCancelled, json!({})),
    ];
    for (kind, payload) in admitted {
        assert_eq!(check(kind, &payload), Ok(()), "{kind:?} {payload}");
    }
    let refused = [
        (
            EventKind::WorkerRead,
            json!({"path_sha256": digest.to_uppercase()}),
        ),
        (EventKind::WorkerRead, json!({"path_sha256": &digest[1..]})),
        (
            EventKind::WorkerRead,
            json!({"path_sha256": format!("{digest}0")}),
        ),
        (EventKind::WorkerRead, json!({"path_bytes": 1.5})),
        (EventKind::WorkerRead, json!({"path_bytes": "1"})),
        (EventKind::ApplyOp, json!({"ok": "true"})),
        (EventKind::ApplyOp, json!({"ok": 1})),
        (
            EventKind::RunGranted,
            json!({"staging_grant": id.to_uppercase()}),
        ),
        (
            EventKind::RunGranted,
            json!({"staging_grant": id.replace('-', "")}),
        ),
        (
            EventKind::RunGranted,
            json!({"staging_grant": format!("{{{id}}}")}),
        ),
        (EventKind::RunRevoked, json!({"reason": "emergency_stop"})),
        (EventKind::RunRevoked, json!({"reason": "EmergencyStop(x)"})),
        (EventKind::RunRevoked, json!({"note": null})),
        (EventKind::RunRevoked, json!({"reason": {"x": "y"}})),
        (EventKind::RunRevoked, json!({"reason": ["EmergencyStop"]})),
        (EventKind::OperationOutcome, json!({"facts": 1})),
        (EventKind::OperationOutcome, json!({"facts": {"note": 1}})),
        (EventKind::OperationOutcome, json!({"facts.files": 1})),
        (
            EventKind::OperationPrepared,
            json!({"op": 0, "Kind": "edit"}),
        ),
        (EventKind::ApplyPrepared, json!({"files": [{"path": "x"}]})),
        (EventKind::ApplyPrepared, json!({"files": ["x"]})),
        (EventKind::ApplyPrepared, json!({"unlisted": []})),
        (EventKind::RunCancelled, json!(null)),
        (EventKind::RunCancelled, json!(["cancelled"])),
        (EventKind::RunCancelled, json!("cancelled")),
    ];
    for (kind, payload) in refused {
        assert_eq!(
            check(kind, &payload),
            Err(LedgerFailure::Rejected),
            "{kind:?} {payload}"
        );
    }
}

/// Every schema key is a well-formed path, unique, and never both a leaf
/// and a branch.
#[test]
fn p0b_schema_keys_are_well_formed() {
    for kind in all_kinds() {
        let keys: Vec<&str> = schema(kind).iter().map(|(key, _)| *key).collect();
        let unique: BTreeSet<&str> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len(), "{kind:?} repeats a key");
        for key in &keys {
            for segment in key.split('.') {
                let name = segment.strip_suffix("[]").unwrap_or(segment);
                assert!(
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                    "{kind:?}: {key}"
                );
            }
            assert!(
                !keys
                    .iter()
                    .any(|other| other.starts_with(&format!("{key}."))
                        || other.starts_with(&format!("{key}[]"))),
                "{kind:?}: {key} is both a leaf and a branch"
            );
        }
        for (_, class) in schema(kind) {
            if let Class::Code(codes) = class {
                assert!(!codes.is_empty());
                let unique: BTreeSet<&&str> = codes.iter().collect();
                assert_eq!(unique.len(), codes.len(), "{kind:?} repeats a code");
            }
        }
    }
}

/// `codes` holds exactly the codes of `all`, each once.
fn registry_is(codes: &[&str], all: &[&'static str], what: &str) {
    let produced: BTreeSet<&str> = all.iter().copied().collect();
    let registered: BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(registered.len(), codes.len(), "{what}: a repeated code");
    assert_eq!(produced, registered, "{what}");
}

#[test]
fn p0b_schema_code_registries_are_closed_and_keep_their_spellings() {
    use EditRejection as E;
    use FailureReason as F;
    use RecoveryReason as R;
    use RevocationReason as V;
    use ScopeError as S;
    let ledger = [
        LedgerFailure::Unavailable,
        LedgerFailure::Integrity,
        LedgerFailure::Rejected,
    ];
    let revocations = [
        V::Explicit,
        V::GrantRevoked,
        V::GrantExpired,
        V::EmergencyStop,
    ];
    let failures = [
        F::AuthorityDenied,
        F::IdentityChanged,
        F::SnapshotRejected,
        F::StagingRedirect,
        F::StagingIo,
        F::StructuralRejected,
        F::ModelUnavailable,
        F::WorkerLimitExceeded,
        F::AuditUnavailable,
    ];
    let recoveries = [
        R::OutcomeNotRecorded,
        R::UnmatchedPrepared,
        R::LedgerIntegrity,
        R::TerminalNotRecorded,
        R::GrantedNotRecorded,
        R::RejectionNotRecorded,
        R::StagingRevocationFailed,
        R::ProjectRevocationFailed,
        R::WorkerPanicked,
    ];
    let edits = [
        E::OutsideWriteScope,
        E::ProtectedInput,
        E::NotText,
        E::TooLarge,
        E::NotStaged,
        E::AlreadyExists,
        E::OutsideReadScope,
    ];
    let scopes = [
        S::Absolute,
        S::Empty,
        S::Traversal,
        S::GitMetadata,
        S::ForbiddenCharacter,
        S::TooLong,
        S::Reserved,
        S::WriteOutsideRead,
        S::ProtectedOutsideRead,
        S::EmptyReadScope,
    ];
    let cleanups = [
        CleanupStatus::NotStarted,
        CleanupStatus::Discarded,
        CleanupStatus::DiscardFailed,
    ];
    let changes = [ChangeKind::Create, ChangeKind::Replace, ChangeKind::Delete];
    let text = String::new;
    let snapshots = [
        SnapshotRejection::Symlink(text()),
        SnapshotRejection::SpecialFile(text()),
        SnapshotRejection::HardLink(text()),
        SnapshotRejection::OtherDevice(text()),
        SnapshotRejection::InvalidName(text()),
        SnapshotRejection::NotADirectory(text()),
        SnapshotRejection::LeftoverTemporary(text()),
        SnapshotRejection::FileTooLarge(text()),
        SnapshotRejection::FileCountCap,
        SnapshotRejection::TotalBytesCap,
        SnapshotRejection::Io(text()),
    ];
    let refusals = [
        Refusal::Declined,
        Refusal::NotApproved,
        Refusal::ApprovalMismatch,
        Refusal::WrongProject,
        Refusal::AuthorityDenied,
        Refusal::IdentityChanged,
        Refusal::CandidateChanged,
        Refusal::Stale(text()),
        Refusal::Redirect(text()),
        Refusal::OutsideScope(text()),
        Refusal::Unsupported(text()),
        Refusal::PreimageUnavailable,
        Refusal::Unrecorded,
        Refusal::VerificationInProgress,
        Refusal::VerificationCleanupFailed,
        Refusal::EmergencyStopped,
        Refusal::Interrupted(InterruptCause::Cancelled),
        Refusal::ProcessDumpable,
    ];

    // Unit variants keep the spelling the ledger used before P0-B.
    macro_rules! same_as_debug {
        ($all:expr, $codes:expr, $what:literal) => {{
            let codes: Vec<&'static str> = $all.iter().map(|v| v.code()).collect();
            for (value, code) in $all.iter().zip(&codes) {
                assert_eq!(format!("{value:?}"), *code, $what);
            }
            registry_is($codes, &codes, $what);
        }};
    }
    same_as_debug!(ledger, LedgerFailure::CODES, "LedgerFailure");
    same_as_debug!(revocations, RevocationReason::CODES, "RevocationReason");
    same_as_debug!(failures, FailureReason::CODES, "FailureReason");
    same_as_debug!(recoveries, RecoveryReason::CODES, "RecoveryReason");
    same_as_debug!(edits, EditRejection::CODES, "EditRejection");
    same_as_debug!(scopes, ScopeError::CODES, "ScopeError");
    same_as_debug!(cleanups, CleanupStatus::CODES, "CleanupStatus");
    same_as_debug!(changes, ChangeKind::CODES, "ChangeKind");

    // Variants that carry text record only their name.
    let codes: Vec<&'static str> = snapshots.iter().map(SnapshotRejection::code).collect();
    for (value, code) in snapshots.iter().zip(&codes) {
        assert!(format!("{value:?}").starts_with(code));
    }
    registry_is(SnapshotRejection::CODES, &codes, "SnapshotRejection");
    let codes: Vec<&'static str> = refusals.iter().map(Refusal::code).collect();
    for (value, code) in refusals.iter().zip(&codes) {
        assert!(format!("{value:?}").starts_with(code));
    }
    registry_is(Refusal::CODES, &codes, "Refusal");

    // Proposal rejections: every combination, spelled as before.
    let mut proposals = vec![
        ProposalRejection::MalformedResponse,
        ProposalRejection::UnsupportedOperation,
        ProposalRejection::NotText,
        ProposalRejection::TooLarge,
        ProposalRejection::LimitReached,
    ];
    proposals.extend(scopes.iter().map(|s| ProposalRejection::InvalidPath(*s)));
    proposals.extend(edits.iter().map(|e| ProposalRejection::Refused(*e)));
    same_as_debug!(proposals, ProposalRejection::CODES, "ProposalRejection");

    // Run errors and their details.
    let mut errors = vec![
        RunError::InvalidState {
            operation: "x",
            state: RunState::Created,
        },
        RunError::AuthorityDenied,
        RunError::IdentityChanged,
        RunError::StagingRedirect,
        RunError::StagingIo,
        RunError::StagingUnavailable,
        RunError::CandidateChanged,
        RunError::CandidateUnavailable,
        RunError::VerificationInput,
    ];
    errors.extend(ledger.iter().cloned().map(RunError::Ledger));
    errors.extend(scopes.iter().copied().map(RunError::Scope));
    errors.extend(revocations.iter().copied().map(RunError::Revoked));
    errors.extend(snapshots.iter().cloned().map(RunError::Snapshot));
    errors.extend(edits.iter().copied().map(RunError::EditRejected));
    errors.extend(recoveries.iter().copied().map(RunError::RecoveryRequired));
    let codes: Vec<&'static str> = errors.iter().map(RunError::code).collect();
    registry_is(RunError::CODES, &codes, "RunError");
    let details: Vec<&'static str> = errors.iter().filter_map(RunError::detail_code).collect();
    registry_is(RunError::DETAIL_CODES, &details, "RunError details");

    let applies = [
        ApplyError::InvalidState,
        ApplyError::Refused(Refusal::Declined),
        ApplyError::RolledBack { failed: text() },
        ApplyError::RecoveryRequired {
            failed: text(),
            unrestored: Vec::new(),
        },
        ApplyError::CompletionUnrecorded { operation: "x" },
        ApplyError::AuthorityNotClosed {
            after: Box::new(Err(ApplyError::InvalidState)),
        },
        ApplyError::Interrupted {
            cause: InterruptCause::Revoked,
        },
        ApplyError::CleanupOwed {
            cause: CleanupCause::Undo,
            temps: Vec::new(),
        },
    ];
    let codes: Vec<&'static str> = applies.iter().map(ApplyError::code).collect();
    registry_is(ApplyError::CODES, &codes, "ApplyError");

    let exits = [
        VerifierExit::Passed,
        VerifierExit::Failed { exit_code: 1 },
        VerifierExit::TimedOut,
        VerifierExit::OutputLimitExceeded,
        VerifierExit::OomKilled,
        VerifierExit::ProcessLimit,
        VerifierExit::Signalled { signal: 9 },
        VerifierExit::SandboxUnavailable,
        VerifierExit::SandboxSetupFailed,
        VerifierExit::SandboxFailed,
        VerifierExit::ToolchainUnavailable,
        VerifierExit::CandidateChanged,
        VerifierExit::CleanupFailed,
    ];
    let names: Vec<&'static str> = exits.iter().map(|e| e.name()).collect();
    registry_is(VERIFIER_EXITS, &names, "VerifierExit");
    assert_eq!(PROVIDERS, &[LOCAL_PROVIDER]);
}

#[test]
fn p0b_schema_scope_digest_names_the_entries_not_their_order() {
    let a = ScopeSet::new([ScopeEntry::Tree(rel("src")), ScopeEntry::File(rel("a.md"))]);
    let b = ScopeSet::new([ScopeEntry::File(rel("a.md")), ScopeEntry::Tree(rel("src"))]);
    assert_eq!(a.digest(), b.digest());
    let file = ScopeSet::new([ScopeEntry::File(rel("src"))]);
    let tree = ScopeSet::new([ScopeEntry::Tree(rel("src"))]);
    assert_ne!(file.digest(), tree.digest());
    assert_ne!(tree.digest(), ScopeSet::default().digest());
    assert_ne!(
        ScopeSet::new([ScopeEntry::WholeProject]).digest(),
        ScopeSet::default().digest()
    );
}

#[test]
fn p0b_schema_a_pin_records_whether_its_host_is_loopback() {
    for (endpoint, loopback) in [
        ("http://127.0.0.1:11434", true),
        ("http://[::1]:11434", true),
        ("http://192.0.2.1:11434", false),
        ("http://localhost:11434", false),
        ("not a url", false),
    ] {
        let facts = pin_facts(&pin_for_test_at(endpoint, "m:1"));
        assert_eq!(facts["endpoint_loopback"], loopback, "{endpoint}");
        assert_eq!(check(EventKind::ModelPinned, &facts), Ok(()));
    }
}
