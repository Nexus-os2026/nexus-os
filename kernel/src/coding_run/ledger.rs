//! The coding-run ledger as seen by the kernel: a closed set of event kinds,
//! a fail-closed append, and recovery analysis over verified records.
//!
//! P0-B: [`record`] is the single write point, and it enforces the ledger
//! schema ([`check`]) before anything is serialized. A record keeps only
//! evidence: digests, counts, booleans, controlled ids and codes from closed
//! registries. A path is recorded as `{path_sha256, path_bytes}`, a scope as
//! `{entries, sha256}`, other free text (a model's name and endpoint) as
//! `<key>_sha256`, and an error or reason as its code. An unlisted key, a
//! string where a digest, count, boolean or id belongs, a code outside its
//! registry, or a payload that is not an object is `Rejected`: nothing is
//! appended, and the operation fails closed as for any other ledger failure.
//! Plain hashes of low-entropy values are guessable: this keeps working
//! content out of the ledger; it does not make a guessable value secret.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_persistence::coding_run_ledger::{CodingRunLedger, LedgerRecord, NewLedgerEvent};
use serde_json::{json, Map, Value};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::apply::{ApplyError, InterruptCause, Refusal, REMOVALS};
use super::local_model::{ModelPin, LOCAL_PROVIDER};
use super::manifest::put_bytes;
use super::review::ChangeKind;
use super::scope::{ScopeError, ScopeSet};
use super::worker::ProposalRejection;
use super::{
    CleanupStatus, EditRejection, FailureReason, RecoveryReason, RevocationReason, RunError,
    SnapshotRejection,
};

/// Why the ledger could not record or verify. Any of these means fail closed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerFailure {
    #[error("coding-run ledger unavailable")]
    Unavailable,
    #[error("coding-run ledger integrity violation")]
    Integrity,
    #[error("coding-run ledger rejected the event")]
    Rejected,
}

/// Storage behind a run's ledger. Production uses the file-backed
/// [`CodingRunLedger`]; the trait exists so tests can inject failures.
pub trait LedgerStore: Send + Sync {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure>;
    /// Verified records of one run (every hash recomputed).
    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure>;
}

impl LedgerStore for CodingRunLedger {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        use nexus_persistence::coding_run_ledger::LedgerError;
        CodingRunLedger::append(self, event).map_err(|error| match error {
            LedgerError::InvalidEvent(_) => LedgerFailure::Rejected,
            LedgerError::Integrity(_) => LedgerFailure::Integrity,
            _ => LedgerFailure::Unavailable,
        })
    }

    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        use nexus_persistence::coding_run_ledger::LedgerError;
        self.verify_run(run).map_err(|error| match error {
            LedgerError::Integrity(_) => LedgerFailure::Integrity,
            _ => LedgerFailure::Unavailable,
        })
    }
}

/// Who caused an event. P1A-002 has only backend actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Backend,
}

impl ActorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backend => "backend",
        }
    }
}

/// The closed set of coding-run ledger events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
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
}

impl EventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RunCreated => "run.created",
            Self::RunGranted => "run.granted",
            Self::OperationPrepared => "op.prepared",
            Self::OperationOutcome => "op.outcome",
            Self::EditRejected => "edit.rejected",
            Self::StructuralVerification => "verify.structural",
            Self::RunCancelled => "run.cancelled",
            Self::RunRevoked => "run.revoked",
            Self::RunFailed => "run.failed",
            Self::RecoveryRequired => "run.recovery_required",
            Self::StagingDiscarded => "staging.discarded",
            Self::ModelPinned => "run.model_pinned",
            Self::WorkerStarted => "worker.started",
            Self::WorkerTurn => "worker.turn",
            Self::WorkerRead => "worker.read",
            Self::ProposalRejected => "worker.proposal_rejected",
            Self::WorkerFinished => "worker.finished",
            Self::ReviewComputed => "review.computed",
            Self::CandidateWithdrawn => "review.candidate_withdrawn",
            Self::ApprovalGranted => "approval.granted",
            Self::ApprovalDeclined => "approval.declined",
            Self::ApplyRejected => "apply.rejected",
            Self::ApplyPrepared => "apply.prepared",
            Self::ApplyOp => "apply.op",
            Self::ApplyCompleted => "apply.completed",
            Self::ApplyRolledBack => "apply.rolled_back",
            Self::ApplyRecoveryRequired => "apply.recovery_required",
            Self::RestoreApproved => "restore.approved",
            Self::RestoreDeclined => "restore.declined",
            Self::RestoreRejected => "restore.rejected",
            Self::RestorePrepared => "restore.prepared",
            Self::RestoreCompleted => "restore.completed",
            Self::VerificationPrepared => "verify.prepared",
            Self::VerificationApproved => "verify.approval_granted",
            Self::VerificationDeclined => "verify.approval_declined",
            Self::VerificationLaunched => "verify.launch",
            Self::VerificationFinished => "verify.result",
            Self::VerificationCleanup => "verify.cleanup",
        }
    }
}

pub(crate) fn now_unix_nanos() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or(0)
}

/// Append one event, failing closed on any error. The payload must satisfy
/// the event's schema ([`check`]); otherwise nothing is appended and the
/// event is `Rejected`.
pub(crate) fn record(
    store: &dyn LedgerStore,
    run: Uuid,
    event: EventKind,
    payload: &Value,
) -> Result<LedgerRecord, LedgerFailure> {
    check(event, payload)?;
    let payload = serde_json::to_string(payload).map_err(|_| LedgerFailure::Rejected)?;
    store.append(NewLedgerEvent {
        run_id: run,
        timestamp_unix_nanos: now_unix_nanos(),
        actor_kind: ActorKind::Backend.as_str(),
        event_kind: event.as_str(),
        payload: &payload,
    })
}

/// Recovery facts derived from a run's verified records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRecovery {
    /// Prepared operation ids with no recorded outcome.
    pub unmatched_prepared: Vec<u64>,
}

impl LedgerRecovery {
    pub fn is_clean(&self) -> bool {
        self.unmatched_prepared.is_empty()
    }
}

/// Verify a run's ledger and find prepared operations with no outcome.
pub fn analyze(store: &dyn LedgerStore, run: Uuid) -> Result<LedgerRecovery, LedgerFailure> {
    let records = store.verified_records(run)?;
    let mut open = BTreeSet::new();
    for record in &records {
        let op = || -> Result<u64, LedgerFailure> {
            let payload: Value =
                serde_json::from_str(&record.payload).map_err(|_| LedgerFailure::Integrity)?;
            payload
                .get("op")
                .and_then(Value::as_u64)
                .ok_or(LedgerFailure::Integrity)
        };
        if record.event_kind == EventKind::OperationPrepared.as_str() {
            open.insert(op()?);
        } else if record.event_kind == EventKind::OperationOutcome.as_str() {
            open.remove(&op()?);
        }
    }
    Ok(LedgerRecovery {
        unmatched_prepared: open.into_iter().collect(),
    })
}

/// What a run's ledger says is uncertain or owed about its project
/// writes (M2d startup recovery report). Reporting only: nothing is swept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyFinding {
    /// An apply was prepared and has no recorded outcome: publication and
    /// temporary-file status are unknown. The planned temp ids are listed.
    PreparedWithoutOutcome { files: Vec<PreparedFile> },
    /// An undo was prepared and has no recorded outcome.
    RestorePreparedWithoutOutcome { files: Vec<PreparedFile> },
    /// Temporary files the kernel observed it could not remove.
    CleanupOwed { temps: Vec<TempLeft> },
    /// The Owner's own versions, kept under temporary names (by id and
    /// path digest) where an apply or undo could not put them back. They are
    /// the Owner's files: never cleanup.
    OwnerVersionKept { versions: Vec<TempLeft> },
    /// Recovery is required for another recorded reason (a code).
    RecoveryRequired { failed: &'static str },
    /// A verified candidate was awaiting approval; its staging may remain.
    StagingAwaitingApproval,
}

/// One file of a prepared operation: its path digest and the backend-random
/// id of the temporary name planned for it (recorded before creation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFile {
    pub path_sha256: String,
    pub temp_id: Option<String>,
}

/// A temporary file left behind, by its backend-random id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempLeft {
    pub temp_id: String,
    pub path_sha256: Option<String>,
}

/// Read one run's verified ledger and report what its project writes left
/// uncertain or owed (M2d §6.7, M6′). It reads the ledger only: nothing on
/// disk is opened, swept or called disposable. A prepared apply or undo with
/// no recorded outcome is reported with the temp ids planned before any
/// creation; a temp the kernel observed it could not remove is owed until a
/// later record shows its removal established.
pub fn analyze_apply(
    store: &dyn LedgerStore,
    run: Uuid,
) -> Result<Vec<ApplyFinding>, LedgerFailure> {
    let mut apply: Option<Vec<PreparedFile>> = None;
    let mut restore: Option<Vec<PreparedFile>> = None;
    let mut owed: Vec<TempLeft> = Vec::new();
    let mut recovery: Option<&'static str> = None;
    let mut kept: Vec<TempLeft> = Vec::new();
    let mut awaiting = false;
    for record in store.verified_records(run)? {
        let payload: Value =
            serde_json::from_str(&record.payload).map_err(|_| LedgerFailure::Integrity)?;
        let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_string);
        let prepared = |key: &str| -> Vec<PreparedFile> {
            payload
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|file| PreparedFile {
                    path_sha256: file
                        .get("path_sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    temp_id: file
                        .get("temp_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
                .collect()
        };
        match record.event_kind.as_str() {
            "verify.structural" => {
                awaiting = payload.get("passed").and_then(Value::as_bool) == Some(true)
            }
            "apply.prepared" => {
                apply = Some(prepared("files"));
                awaiting = false;
            }
            "apply.completed" => apply = None,
            "apply.rolled_back" => {
                apply = None;
                owed.clear();
                recovery = None;
            }
            "apply.recovery_required" => {
                apply = None;
                // A prepared undo stays reported unless this record accounts
                // for its moved file (an owed temp id): a failed or undurable
                // return leaves its outcome uncertain.
                if payload.get("temp_id").is_some() {
                    restore = None;
                }
                let failed = text("failed")
                    .and_then(|code| APPLY_STEPS.iter().copied().find(|step| *step == code))
                    .ok_or(LedgerFailure::Integrity)?;
                let unrestored = payload.get("unrestored").and_then(Value::as_array);
                for entry in unrestored.into_iter().flatten() {
                    if let Some(temp_id) = entry.get("owner_version_temp").and_then(Value::as_str) {
                        kept.push(TempLeft {
                            temp_id: temp_id.to_string(),
                            path_sha256: entry
                                .get("path_sha256")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    }
                }
                match text("temp_id") {
                    Some(temp_id) => owed.push(TempLeft {
                        temp_id,
                        path_sha256: text("path_sha256"),
                    }),
                    None => {
                        if failed == "cleanup" {
                            owed.clear();
                        }
                        recovery = Some(failed);
                    }
                }
            }
            "restore.prepared" => restore = Some(prepared("planned")),
            "restore.completed" => {
                restore = None;
                owed.clear();
                if recovery == Some("dir_sync") {
                    recovery = None;
                }
            }
            "restore.rejected" => restore = None,
            "review.candidate_withdrawn"
            | "staging.discarded"
            | "run.cancelled"
            | "run.revoked"
            | "run.failed"
            | "run.recovery_required" => awaiting = false,
            _ => {}
        }
    }
    let mut findings = Vec::new();
    if let Some(files) = apply {
        findings.push(ApplyFinding::PreparedWithoutOutcome { files });
    }
    if let Some(files) = restore {
        findings.push(ApplyFinding::RestorePreparedWithoutOutcome { files });
    }
    if !owed.is_empty() {
        findings.push(ApplyFinding::CleanupOwed { temps: owed });
    }
    if !kept.is_empty() {
        findings.push(ApplyFinding::OwnerVersionKept { versions: kept });
    }
    if let Some(failed) = recovery {
        findings.push(ApplyFinding::RecoveryRequired { failed });
    }
    if awaiting {
        findings.push(ApplyFinding::StagingAwaitingApproval);
    }
    Ok(findings)
}

// ── The schema (P0-B, I5) ───────────────────────────────────────────────────

/// The class of one recorded value.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Class {
    /// 64 lowercase hex characters.
    Digest,
    /// An integer.
    Count,
    Bool,
    /// A canonical (hyphenated, lowercase) UUID.
    Id,
    /// One of a closed set of codes.
    Code(&'static [&'static str]),
}

impl Class {
    fn admits(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Digest, Value::String(text)) => {
                text.len() == 64
                    && text
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }
            (Self::Count, Value::Number(number)) => number.is_i64() || number.is_u64(),
            (Self::Bool, Value::Bool(_)) => true,
            (Self::Id, Value::String(text)) => {
                Uuid::parse_str(text).is_ok_and(|id| id.hyphenated().to_string() == *text)
            }
            (Self::Code(codes), Value::String(text)) => codes.contains(&text.as_str()),
            _ => false,
        }
    }
}

/// An event's schema: each recorded leaf's key path and class. A nested
/// object's keys are joined with `.`, a list's elements are at `<key>[]`.
/// `null` is allowed at any declared leaf.
pub(crate) type Schema = &'static [(&'static str, Class)];

const DIGEST: Class = Class::Digest;
const COUNT: Class = Class::Count;
const BOOL: Class = Class::Bool;
const ID: Class = Class::Id;

/// The operations `op.prepared` and `op.outcome` describe.
pub(crate) const OP_KINDS: &[&str] = &["snapshot", "edit", "allocate_staging"];
/// [`super::CandidateEdit`] kinds.
pub(crate) const EDIT_KINDS: &[&str] = &["replace", "create"];
/// The worker's turn actions ([`super::worker`]), `malformed` for none.
pub(crate) const WORKER_ACTIONS: &[&str] = &["read", "edit", "finish", "malformed"];
pub(crate) const PROVIDERS: &[&str] = &[LOCAL_PROVIDER];
/// [`super::VerifierExit::name`] of every exit.
pub(crate) const VERIFIER_EXITS: &[&str] = &[
    "passed",
    "failed",
    "timed_out",
    "output_limit_exceeded",
    "oom_killed",
    "process_limit",
    "signalled",
    "sandbox_unavailable",
    "sandbox_setup_failed",
    "sandbox_failed",
    "toolchain_unavailable",
    "candidate_changed",
    "cleanup_failed",
];
/// Where an apply, a rollback or a restore stopped: at one `file` (whose
/// path is recorded by digest) or at a named step.
pub(crate) const APPLY_STEPS: &[&str] = &[
    "file",
    "apply.completed",
    "restore",
    "restore.completed",
    "write grant closure",
    // M2d: an observed apply's one publication.
    "interrupted",
    "temp",
    "publish",
    "apply.op",
    "dir_sync",
    "cleanup",
    "verify",
];
/// The operations whose completion record can fail after their work.
pub(crate) const COMPLETIONS: &[&str] = &["rollback", "restore"];

const RUN_CREATED: Schema = &[
    ("project_grant", ID),
    ("agent", ID),
    ("binding_run", ID),
    ("project", ID),
    ("read.entries", COUNT),
    ("read.sha256", DIGEST),
    ("write.entries", COUNT),
    ("write.sha256", DIGEST),
    ("protected.entries", COUNT),
    ("protected.sha256", DIGEST),
    ("profile", DIGEST),
];
const RUN_GRANTED: Schema = &[("staging_grant", ID)];
const OP_PREPARED: Schema = &[
    ("op", COUNT),
    ("kind", Class::Code(OP_KINDS)),
    ("facts.name", ID),
    ("facts.read.entries", COUNT),
    ("facts.read.sha256", DIGEST),
    ("facts.path_sha256", DIGEST),
    ("facts.path_bytes", COUNT),
    ("facts.kind", Class::Code(EDIT_KINDS)),
    ("facts.size", COUNT),
    ("facts.sha256", DIGEST),
];
const OP_OUTCOME: Schema = &[
    ("op", COUNT),
    ("ok", BOOL),
    ("facts.staging_grant", ID),
    ("facts.base_manifest", DIGEST),
    ("facts.files", COUNT),
    ("facts.bytes", COUNT),
    ("error", Class::Code(RunError::CODES)),
    ("detail", Class::Code(RunError::DETAIL_CODES)),
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
];
const EDIT_REJECTED: Schema = &[
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
    ("kind", Class::Code(EDIT_KINDS)),
    ("reason", Class::Code(EditRejection::CODES)),
];
const STRUCTURAL: Schema = &[
    ("base_manifest", DIGEST),
    ("candidate_manifest", DIGEST),
    ("profile", DIGEST),
    ("binding", DIGEST),
    ("passed", BOOL),
    ("violations", COUNT),
];
const RUN_CANCELLED: Schema = &[("reason", Class::Code(&["cancelled"]))];
const RUN_REVOKED: Schema = &[("reason", Class::Code(RevocationReason::CODES))];
const RUN_FAILED: Schema = &[("reason", Class::Code(FailureReason::CODES))];
const RUN_RECOVERY: Schema = &[("reason", Class::Code(RecoveryReason::CODES))];
const STAGING_DISCARDED: Schema = &[("status", Class::Code(CleanupStatus::CODES))];
const MODEL_PINNED: Schema = &[
    ("provider", Class::Code(PROVIDERS)),
    ("endpoint_sha256", DIGEST),
    ("endpoint_loopback", BOOL),
    ("model_sha256", DIGEST),
];
const WORKER_STARTED: Schema = &[
    ("task_sha256", DIGEST),
    ("task_bytes", COUNT),
    ("limits.turns", COUNT),
    ("limits.files_read", COUNT),
    ("limits.bytes_read", COUNT),
    ("limits.candidate_files", COUNT),
    ("limits.candidate_bytes", COUNT),
    ("limits.proposals", COUNT),
    ("limits.response_bytes", COUNT),
    ("limits.deadline_secs", COUNT),
];
const WORKER_TURN: Schema = &[
    ("turn", COUNT),
    ("response_bytes", COUNT),
    ("response_sha256", DIGEST),
    ("action", Class::Code(WORKER_ACTIONS)),
];
const WORKER_READ: Schema = &[
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
    ("size", COUNT),
    ("sha256", DIGEST),
];
const PROPOSAL_REJECTED: Schema = &[
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
    ("reason", Class::Code(ProposalRejection::CODES)),
];
const WORKER_FINISHED: Schema = &[
    ("turns", COUNT),
    ("files_read", COUNT),
    ("bytes_read", COUNT),
    ("accepted", COUNT),
    ("rejected", COUNT),
    ("summary_bytes", COUNT),
    ("summary_sha256", DIGEST),
];
const REVIEW_COMPUTED: Schema = &[
    ("binding", DIGEST),
    ("base_manifest", DIGEST),
    ("candidate_manifest", DIGEST),
    ("verification", DIGEST),
    ("create", COUNT),
    ("replace", COUNT),
    ("delete", COUNT),
];
const CANDIDATE_WITHDRAWN: Schema = &[
    ("reason", Class::Code(RunError::CODES)),
    ("detail", Class::Code(RunError::DETAIL_CODES)),
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
];
const BINDING_FILES: Schema = &[("binding", DIGEST), ("files", COUNT)];
const BINDING: Schema = &[("binding", DIGEST)];
const REFUSED: Schema = &[
    ("reason", Class::Code(Refusal::CODES)),
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
    ("interrupted", Class::Code(InterruptCause::CODES)),
];
const APPLY_PREPARED: Schema = &[
    ("binding", DIGEST),
    ("preimages[].path_sha256", DIGEST),
    ("preimages[].path_bytes", COUNT),
    ("preimages[].existed", BOOL),
    ("preimages[].slot", COUNT),
    ("preimages[].sha256", DIGEST),
    ("preimages[].size", COUNT),
    ("preimages[].mode", COUNT),
    ("files[].path_sha256", DIGEST),
    ("files[].path_bytes", COUNT),
    ("files[].kind", Class::Code(ChangeKind::CODES)),
    ("files[].base", DIGEST),
    ("files[].candidate", DIGEST),
    ("files[].temp_id", ID),
];
const APPLY_OP: Schema = &[("path_sha256", DIGEST), ("path_bytes", COUNT), ("ok", BOOL)];
const APPLY_FAILED: Schema = &[
    ("failed", Class::Code(APPLY_STEPS)),
    ("path_sha256", DIGEST),
    ("path_bytes", COUNT),
    ("unrestored[].path_sha256", DIGEST),
    ("unrestored[].path_bytes", COUNT),
    ("unrestored[].owner_version_temp", ID),
    ("attempt_ok", BOOL),
    ("completion_unrecorded", Class::Code(COMPLETIONS)),
    ("interrupted", Class::Code(InterruptCause::CODES)),
    ("temp_id", ID),
    ("removal", Class::Code(REMOVALS)),
];
const RESTORE_PREPARED: Schema = &[
    ("binding", DIGEST),
    ("files", COUNT),
    ("planned[].path_sha256", DIGEST),
    ("planned[].path_bytes", COUNT),
    ("planned[].temp_id", ID),
];
const VERIFY_PREPARED: Schema = &[
    ("binding", DIGEST),
    ("candidate_manifest", DIGEST),
    ("structural_binding", DIGEST),
    ("profile", DIGEST),
    ("toolchain", DIGEST),
    ("toolchain_generation", COUNT),
    ("sandbox_policy", DIGEST),
    ("resource_policy", DIGEST),
];
const VERIFY_LAUNCH: Schema = &[("binding", DIGEST), ("generation", COUNT)];
const VERIFY_RESULT: Schema = &[
    ("result", DIGEST),
    ("generation", COUNT),
    ("exit", Class::Code(VERIFIER_EXITS)),
    ("detail", COUNT),
    ("duration_ms", COUNT),
    ("stdout.bytes", COUNT),
    ("stdout.sha256", DIGEST),
    ("stdout.truncated", BOOL),
    ("stderr.bytes", COUNT),
    ("stderr.sha256", DIGEST),
    ("stderr.truncated", BOOL),
    ("cleanup", Class::Code(&["confirmed", "failed"])),
];
const VERIFY_CLEANUP: Schema = &[
    ("generation", COUNT),
    ("cleanup", Class::Code(&["confirmed"])),
];

/// The schema of every event kind.
pub(crate) fn schema(kind: EventKind) -> Schema {
    use EventKind as K;
    match kind {
        K::RunCreated => RUN_CREATED,
        K::RunGranted => RUN_GRANTED,
        K::OperationPrepared => OP_PREPARED,
        K::OperationOutcome => OP_OUTCOME,
        K::EditRejected => EDIT_REJECTED,
        K::StructuralVerification => STRUCTURAL,
        K::RunCancelled => RUN_CANCELLED,
        K::RunRevoked => RUN_REVOKED,
        K::RunFailed => RUN_FAILED,
        K::RecoveryRequired => RUN_RECOVERY,
        K::StagingDiscarded => STAGING_DISCARDED,
        K::ModelPinned => MODEL_PINNED,
        K::WorkerStarted => WORKER_STARTED,
        K::WorkerTurn => WORKER_TURN,
        K::WorkerRead => WORKER_READ,
        K::ProposalRejected => PROPOSAL_REJECTED,
        K::WorkerFinished => WORKER_FINISHED,
        K::ReviewComputed => REVIEW_COMPUTED,
        K::CandidateWithdrawn => CANDIDATE_WITHDRAWN,
        K::ApprovalGranted | K::ApprovalDeclined => BINDING_FILES,
        K::ApplyRejected | K::RestoreRejected => REFUSED,
        K::ApplyPrepared => APPLY_PREPARED,
        K::ApplyOp => APPLY_OP,
        K::ApplyCompleted | K::RestoreCompleted => BINDING_FILES,
        K::RestorePrepared => RESTORE_PREPARED,
        K::ApplyRolledBack | K::ApplyRecoveryRequired => APPLY_FAILED,
        K::RestoreApproved | K::RestoreDeclined => BINDING,
        K::VerificationPrepared => VERIFY_PREPARED,
        K::VerificationApproved | K::VerificationDeclined => BINDING,
        K::VerificationLaunched => VERIFY_LAUNCH,
        K::VerificationFinished => VERIFY_RESULT,
        K::VerificationCleanup => VERIFY_CLEANUP,
    }
}

/// Check one payload against its event's schema: it must be an object whose
/// every leaf is at a declared key path and of that key's class.
pub(crate) fn check(kind: EventKind, payload: &Value) -> Result<(), LedgerFailure> {
    match payload {
        Value::Object(map) if object_admitted(schema(kind), "", map) => Ok(()),
        _ => Err(LedgerFailure::Rejected),
    }
}

fn object_admitted(schema: Schema, prefix: &str, map: &Map<String, Value>) -> bool {
    map.iter().all(|(key, value)| {
        let well_formed = !key.is_empty()
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        well_formed && value_admitted(schema, &path, value)
    })
}

fn value_admitted(schema: Schema, path: &str, value: &Value) -> bool {
    match value {
        Value::Object(map) => is_branch(schema, path) && object_admitted(schema, path, map),
        Value::Array(items) => {
            let element = format!("{path}[]");
            (is_branch(schema, &element) || leaf(schema, &element).is_some())
                && items
                    .iter()
                    .all(|item| value_admitted(schema, &element, item))
        }
        Value::Null => leaf(schema, path).is_some(),
        value => leaf(schema, path).is_some_and(|class| class.admits(value)),
    }
}

fn is_branch(schema: Schema, path: &str) -> bool {
    schema.iter().any(|(key, _)| {
        key.strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('.'))
    })
}

fn leaf(schema: Schema, path: &str) -> Option<Class> {
    schema
        .iter()
        .find(|(key, _)| *key == path)
        .map(|(_, class)| *class)
}

// ── Evidence: what producers record instead of content ─────────────────────

/// Digest domain of a path spelling recorded in the ledger (I9).
pub(crate) const PATH_DOMAIN: &[u8] = b"nexus.evidence.path.v1";
/// Digest domain of other free text recorded by digest (I9).
pub(crate) const TEXT_DOMAIN: &[u8] = b"nexus.evidence.text.v1";

/// SHA-256 over `domain` and `bytes`, each length-prefixed, as hex.
pub(crate) fn digest_in(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    put_bytes(&mut hasher, domain);
    put_bytes(&mut hasher, bytes);
    hex::encode(hasher.finalize())
}

/// The `path_sha256` of a path spelling (with `path_bytes`, its length,
/// this is how the ledger names a path).
pub(crate) fn path_sha256(text: &str) -> String {
    digest_in(PATH_DOMAIN, text.as_bytes())
}

fn insert_path(facts: &mut Map<String, Value>, text: &str) {
    facts.insert("path_sha256".to_string(), json!(path_sha256(text)));
    facts.insert("path_bytes".to_string(), json!(text.len()));
}

/// `{path_sha256, path_bytes}` of a path spelling.
pub(crate) fn path_facts(text: &str) -> Value {
    let mut facts = Map::new();
    insert_path(&mut facts, text);
    Value::Object(facts)
}

/// A scope as its entry count and digest.
pub(crate) fn scope_facts(scope: &ScopeSet) -> Value {
    json!({
        "entries": scope.entries().len(),
        "sha256": hex::encode(scope.digest()),
    })
}

/// A model pin: its provider, the digests of its endpoint and model name,
/// and whether the endpoint's host is a loopback address.
pub(crate) fn pin_facts(pin: &ModelPin) -> Value {
    let loopback = url::Url::parse(pin.endpoint())
        .ok()
        .is_some_and(|url| match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        });
    json!({
        "provider": pin.provider(),
        "endpoint_sha256": digest_in(TEXT_DOMAIN, pin.endpoint().as_bytes()),
        "endpoint_loopback": loopback,
        "model_sha256": digest_in(TEXT_DOMAIN, pin.model().as_bytes()),
    })
}

/// A run error as codes under `key` and `detail`, and the refused path of a
/// snapshot refusal by digest.
fn insert_error(facts: &mut Map<String, Value>, key: &str, error: &RunError) {
    facts.insert(key.to_string(), json!(error.code()));
    if let Some(detail) = error.detail_code() {
        facts.insert("detail".to_string(), json!(detail));
    }
    if let RunError::Snapshot(rejection) = error {
        if let Some(path) = rejection.path() {
            insert_path(facts, path);
        }
    }
}

/// The `op.outcome` of a failed operation.
pub(crate) fn failed_op(op: u64, error: &RunError) -> Value {
    let mut facts = Map::new();
    facts.insert("op".to_string(), json!(op));
    facts.insert("ok".to_string(), json!(false));
    insert_error(&mut facts, "error", error);
    Value::Object(facts)
}

/// The `review.candidate_withdrawn` of a withdrawn verification.
pub(crate) fn withdrawn(error: &RunError) -> Value {
    let mut facts = Map::new();
    insert_error(&mut facts, "reason", error);
    Value::Object(facts)
}

/// An apply or restore refusal: its code, and the refused path by digest.
pub(crate) fn refusal_facts(refusal: &Refusal) -> Value {
    let mut facts = Map::new();
    facts.insert("reason".to_string(), json!(refusal.code()));
    if let Some(path) = refusal.path() {
        insert_path(&mut facts, path);
    }
    if let Refusal::Interrupted(cause) = refusal {
        facts.insert("interrupted".to_string(), json!(cause.code()));
    }
    Value::Object(facts)
}

// ── Codes: closed registries of the kernel's reasons and errors ────────────
//
// A code is a fixed `&'static str` from its type's `CODES`. Unit variants keep
// the spelling the ledger used before P0-B (their `Debug` name), so recorded
// reasons stay comparable across the change; a variant that carries text
// records only its name.

impl LedgerFailure {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "Unavailable",
            Self::Integrity => "Integrity",
            Self::Rejected => "Rejected",
        }
    }

    pub const CODES: &'static [&'static str] = &["Unavailable", "Integrity", "Rejected"];
}

impl RevocationReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::Explicit => "Explicit",
            Self::GrantRevoked => "GrantRevoked",
            Self::GrantExpired => "GrantExpired",
            Self::EmergencyStop => "EmergencyStop",
        }
    }

    pub const CODES: &'static [&'static str] =
        &["Explicit", "GrantRevoked", "GrantExpired", "EmergencyStop"];
}

impl FailureReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::AuthorityDenied => "AuthorityDenied",
            Self::IdentityChanged => "IdentityChanged",
            Self::SnapshotRejected => "SnapshotRejected",
            Self::StagingRedirect => "StagingRedirect",
            Self::StagingIo => "StagingIo",
            Self::StructuralRejected => "StructuralRejected",
            Self::ModelUnavailable => "ModelUnavailable",
            Self::WorkerLimitExceeded => "WorkerLimitExceeded",
            Self::AuditUnavailable => "AuditUnavailable",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "AuthorityDenied",
        "IdentityChanged",
        "SnapshotRejected",
        "StagingRedirect",
        "StagingIo",
        "StructuralRejected",
        "ModelUnavailable",
        "WorkerLimitExceeded",
        "AuditUnavailable",
    ];
}

impl RecoveryReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::OutcomeNotRecorded => "OutcomeNotRecorded",
            Self::UnmatchedPrepared => "UnmatchedPrepared",
            Self::LedgerIntegrity => "LedgerIntegrity",
            Self::TerminalNotRecorded => "TerminalNotRecorded",
            Self::GrantedNotRecorded => "GrantedNotRecorded",
            Self::RejectionNotRecorded => "RejectionNotRecorded",
            Self::StagingRevocationFailed => "StagingRevocationFailed",
            Self::ProjectRevocationFailed => "ProjectRevocationFailed",
            Self::WorkerPanicked => "WorkerPanicked",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "OutcomeNotRecorded",
        "UnmatchedPrepared",
        "LedgerIntegrity",
        "TerminalNotRecorded",
        "GrantedNotRecorded",
        "RejectionNotRecorded",
        "StagingRevocationFailed",
        "ProjectRevocationFailed",
        "WorkerPanicked",
    ];
}

impl CleanupStatus {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::NotStarted => "NotStarted",
            Self::Discarded => "Discarded",
            Self::DiscardFailed => "DiscardFailed",
        }
    }

    pub(crate) const CODES: &'static [&'static str] = &["NotStarted", "Discarded", "DiscardFailed"];
}

impl EditRejection {
    pub fn code(self) -> &'static str {
        match self {
            Self::OutsideWriteScope => "OutsideWriteScope",
            Self::ProtectedInput => "ProtectedInput",
            Self::NotText => "NotText",
            Self::TooLarge => "TooLarge",
            Self::NotStaged => "NotStaged",
            Self::AlreadyExists => "AlreadyExists",
            Self::OutsideReadScope => "OutsideReadScope",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "OutsideWriteScope",
        "ProtectedInput",
        "NotText",
        "TooLarge",
        "NotStaged",
        "AlreadyExists",
        "OutsideReadScope",
    ];
}

impl SnapshotRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Symlink(_) => "Symlink",
            Self::SpecialFile(_) => "SpecialFile",
            Self::HardLink(_) => "HardLink",
            Self::OtherDevice(_) => "OtherDevice",
            Self::InvalidName(_) => "InvalidName",
            Self::NotADirectory(_) => "NotADirectory",
            Self::LeftoverTemporary(_) => "LeftoverTemporary",
            Self::FileTooLarge(_) => "FileTooLarge",
            Self::FileCountCap => "FileCountCap",
            Self::TotalBytesCap => "TotalBytesCap",
            Self::Io(_) => "Io",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "Symlink",
        "SpecialFile",
        "HardLink",
        "OtherDevice",
        "InvalidName",
        "NotADirectory",
        "LeftoverTemporary",
        "FileTooLarge",
        "FileCountCap",
        "TotalBytesCap",
        "Io",
    ];

    /// The project-relative path the refusal names, if any.
    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::Symlink(path)
            | Self::SpecialFile(path)
            | Self::HardLink(path)
            | Self::OtherDevice(path)
            | Self::InvalidName(path)
            | Self::NotADirectory(path)
            | Self::LeftoverTemporary(path)
            | Self::FileTooLarge(path)
            | Self::Io(path) => Some(path),
            Self::FileCountCap | Self::TotalBytesCap => None,
        }
    }
}

impl RunError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidState { .. } => "InvalidState",
            Self::Ledger(_) => "Ledger",
            Self::Scope(_) => "Scope",
            Self::Revoked(_) => "Revoked",
            Self::AuthorityDenied => "AuthorityDenied",
            Self::IdentityChanged => "IdentityChanged",
            Self::Snapshot(_) => "Snapshot",
            Self::EditRejected(_) => "EditRejected",
            Self::StagingRedirect => "StagingRedirect",
            Self::StagingIo => "StagingIo",
            Self::RecoveryRequired(_) => "RecoveryRequired",
            Self::StagingUnavailable => "StagingUnavailable",
            Self::CandidateChanged => "CandidateChanged",
            Self::CandidateUnavailable => "CandidateUnavailable",
            Self::VerificationInput => "VerificationInput",
        }
    }

    /// The code of the reason the error carries, if any.
    pub fn detail_code(&self) -> Option<&'static str> {
        match self {
            Self::Ledger(failure) => Some(failure.code()),
            Self::Scope(error) => Some(error.code()),
            Self::Revoked(reason) => Some(reason.code()),
            Self::Snapshot(rejection) => Some(rejection.code()),
            Self::EditRejected(rejection) => Some(rejection.code()),
            Self::RecoveryRequired(reason) => Some(reason.code()),
            _ => None,
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "InvalidState",
        "Ledger",
        "Scope",
        "Revoked",
        "AuthorityDenied",
        "IdentityChanged",
        "Snapshot",
        "EditRejected",
        "StagingRedirect",
        "StagingIo",
        "RecoveryRequired",
        "StagingUnavailable",
        "CandidateChanged",
        "CandidateUnavailable",
        "VerificationInput",
    ];

    /// Every [`Self::detail_code`]: the codes of [`LedgerFailure`],
    /// [`ScopeError`], [`RevocationReason`], [`SnapshotRejection`],
    /// [`EditRejection`] and [`RecoveryReason`].
    pub const DETAIL_CODES: &'static [&'static str] = &[
        "Unavailable",
        "Integrity",
        "Rejected",
        "Absolute",
        "Empty",
        "Traversal",
        "GitMetadata",
        "ForbiddenCharacter",
        "TooLong",
        "Reserved",
        "WriteOutsideRead",
        "ProtectedOutsideRead",
        "EmptyReadScope",
        "Explicit",
        "GrantRevoked",
        "GrantExpired",
        "EmergencyStop",
        "Symlink",
        "SpecialFile",
        "HardLink",
        "OtherDevice",
        "InvalidName",
        "NotADirectory",
        "LeftoverTemporary",
        "FileTooLarge",
        "FileCountCap",
        "TotalBytesCap",
        "Io",
        "OutsideWriteScope",
        "ProtectedInput",
        "NotText",
        "TooLarge",
        "NotStaged",
        "AlreadyExists",
        "OutsideReadScope",
        "OutcomeNotRecorded",
        "UnmatchedPrepared",
        "LedgerIntegrity",
        "TerminalNotRecorded",
        "GrantedNotRecorded",
        "RejectionNotRecorded",
        "StagingRevocationFailed",
        "ProjectRevocationFailed",
        "WorkerPanicked",
    ];
}

impl ProposalRejection {
    pub fn code(self) -> &'static str {
        match self {
            Self::MalformedResponse => "MalformedResponse",
            Self::UnsupportedOperation => "UnsupportedOperation",
            Self::InvalidPath(error) => match error {
                ScopeError::Absolute => "InvalidPath(Absolute)",
                ScopeError::Empty => "InvalidPath(Empty)",
                ScopeError::Traversal => "InvalidPath(Traversal)",
                ScopeError::GitMetadata => "InvalidPath(GitMetadata)",
                ScopeError::ForbiddenCharacter => "InvalidPath(ForbiddenCharacter)",
                ScopeError::TooLong => "InvalidPath(TooLong)",
                ScopeError::Reserved => "InvalidPath(Reserved)",
                ScopeError::WriteOutsideRead => "InvalidPath(WriteOutsideRead)",
                ScopeError::ProtectedOutsideRead => "InvalidPath(ProtectedOutsideRead)",
                ScopeError::EmptyReadScope => "InvalidPath(EmptyReadScope)",
            },
            Self::NotText => "NotText",
            Self::TooLarge => "TooLarge",
            Self::LimitReached => "LimitReached",
            Self::Refused(rejection) => match rejection {
                EditRejection::OutsideWriteScope => "Refused(OutsideWriteScope)",
                EditRejection::ProtectedInput => "Refused(ProtectedInput)",
                EditRejection::NotText => "Refused(NotText)",
                EditRejection::TooLarge => "Refused(TooLarge)",
                EditRejection::NotStaged => "Refused(NotStaged)",
                EditRejection::AlreadyExists => "Refused(AlreadyExists)",
                EditRejection::OutsideReadScope => "Refused(OutsideReadScope)",
            },
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "MalformedResponse",
        "UnsupportedOperation",
        "InvalidPath(Absolute)",
        "InvalidPath(Empty)",
        "InvalidPath(Traversal)",
        "InvalidPath(GitMetadata)",
        "InvalidPath(ForbiddenCharacter)",
        "InvalidPath(TooLong)",
        "InvalidPath(Reserved)",
        "InvalidPath(WriteOutsideRead)",
        "InvalidPath(ProtectedOutsideRead)",
        "InvalidPath(EmptyReadScope)",
        "NotText",
        "TooLarge",
        "LimitReached",
        "Refused(OutsideWriteScope)",
        "Refused(ProtectedInput)",
        "Refused(NotText)",
        "Refused(TooLarge)",
        "Refused(NotStaged)",
        "Refused(AlreadyExists)",
        "Refused(OutsideReadScope)",
    ];
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Declined => "Declined",
            Self::NotApproved => "NotApproved",
            Self::ApprovalMismatch => "ApprovalMismatch",
            Self::WrongProject => "WrongProject",
            Self::AuthorityDenied => "AuthorityDenied",
            Self::IdentityChanged => "IdentityChanged",
            Self::CandidateChanged => "CandidateChanged",
            Self::Stale(_) => "Stale",
            Self::Redirect(_) => "Redirect",
            Self::OutsideScope(_) => "OutsideScope",
            Self::Unsupported(_) => "Unsupported",
            Self::PreimageUnavailable => "PreimageUnavailable",
            Self::Unrecorded => "Unrecorded",
            Self::VerificationInProgress => "VerificationInProgress",
            Self::VerificationCleanupFailed => "VerificationCleanupFailed",
            Self::EmergencyStopped => "EmergencyStopped",
            Self::Interrupted(_) => "Interrupted",
            Self::ProcessDumpable => "ProcessDumpable",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "Declined",
        "NotApproved",
        "ApprovalMismatch",
        "WrongProject",
        "AuthorityDenied",
        "IdentityChanged",
        "CandidateChanged",
        "Stale",
        "Redirect",
        "OutsideScope",
        "Unsupported",
        "PreimageUnavailable",
        "Unrecorded",
        "VerificationInProgress",
        "VerificationCleanupFailed",
        "EmergencyStopped",
        "Interrupted",
        "ProcessDumpable",
    ];

    /// The project-relative path the refusal names, if any.
    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::Stale(path)
            | Self::Redirect(path)
            | Self::OutsideScope(path)
            | Self::Unsupported(path) => Some(path),
            _ => None,
        }
    }
}

impl ApplyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidState => "InvalidState",
            Self::Refused(_) => "Refused",
            Self::RolledBack { .. } => "RolledBack",
            Self::RecoveryRequired { .. } => "RecoveryRequired",
            Self::CompletionUnrecorded { .. } => "CompletionUnrecorded",
            Self::AuthorityNotClosed { .. } => "AuthorityNotClosed",
            Self::Interrupted { .. } => "Interrupted",
            Self::CleanupOwed { .. } => "CleanupOwed",
        }
    }

    /// The refusal's code, for a refused apply or restore.
    pub fn detail_code(&self) -> Option<&'static str> {
        match self {
            Self::Refused(refusal) => Some(refusal.code()),
            _ => None,
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "InvalidState",
        "Refused",
        "RolledBack",
        "RecoveryRequired",
        "CompletionUnrecorded",
        "AuthorityNotClosed",
        "Interrupted",
        "CleanupOwed",
    ];
}

impl InterruptCause {
    pub fn code(self) -> &'static str {
        match self {
            Self::EmergencyStop => "EmergencyStop",
            Self::Cancelled => "Cancelled",
            Self::Revoked => "Revoked",
            Self::Expired => "Expired",
            Self::CommitmentExpired => "CommitmentExpired",
        }
    }

    pub const CODES: &'static [&'static str] = &[
        "EmergencyStop",
        "Cancelled",
        "Revoked",
        "Expired",
        "CommitmentExpired",
    ];
}

impl ChangeKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Create => "Create",
            Self::Replace => "Replace",
            Self::Delete => "Delete",
        }
    }

    pub(crate) const CODES: &'static [&'static str] = &["Create", "Replace", "Delete"];
}
