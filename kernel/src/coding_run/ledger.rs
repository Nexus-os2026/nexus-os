//! The coding-run ledger as seen by the kernel: a closed set of event kinds,
//! a fail-closed append, and recovery analysis over verified records.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_persistence::coding_run_ledger::{CodingRunLedger, LedgerRecord, NewLedgerEvent};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

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

/// Append one event, failing closed on any error.
pub(crate) fn record(
    store: &dyn LedgerStore,
    run: Uuid,
    event: EventKind,
    payload: &Value,
) -> Result<LedgerRecord, LedgerFailure> {
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
