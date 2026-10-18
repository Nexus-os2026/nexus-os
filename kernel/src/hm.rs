//! Historical-memory decisions (P3-HM-CONTRACT-02 §2.3, §5.2, §9, §11.0).
//!
//! Dormant: no production path calls this module yet. It owns the decisions
//! over the storage core in `nexus_persistence::hm`:
//! - the T₀ migration (§9.1 steps 1–6, one `BEGIN IMMEDIATE` transaction)
//!   with the Owner's recorded AD-5 **Option A** baseline;
//! - the start-up ledger checks L1–L4 and the per-member states (§9.5);
//! - the evaluated-label rule: a missing, malformed or binding-mismatched
//!   label evaluates as `UNBOUND`, which is `LOCAL_ONLY_ORIGIN` (§2.3, §5.2).
//!
//! AD-4 (declassification) is disabled: there is no grant creation,
//! revocation or bulk-scope code here. Events are content-free (§15.1).
//!
//! The authority core (§4.2, §6, §11, §12.3–§12.7, §15) lives in the
//! submodules: the sealed `DispatchContext` and the unit-taint algebra
//! (`context`); the `DispatchIssuer`, §4.2 resolution and the context
//! derivations (`issuer`); labelled writes (`write`); route-first recall
//! (`recall`); the §12.4 admission point and §12.6 reply acceptance
//! (`dispatch`); content-free events and the back-fill reconciler
//! (`events`).

mod context;
mod dispatch;
mod events;
mod issuer;
mod recall;
mod write;

pub use issuer::{
    AgentRegistry, ConfigInput, ConfigReadError, ConfigReading, DispatchIssuer, ProbePoint,
    Unresolved, MAX_RECORDS,
};

pub use dispatch::{
    Admission, BuildDispatch, Refused, ReplyRefused, RouteAttestation, TransportEvidence,
    TransportFailure,
};
pub use events::{backfill_authority_events, BackfillReport, BACKFILL_BOUND};
pub use recall::{RecallQuery, RecallRefused};
pub use write::{
    NewMemory, Parent, SourceKind, StrategyWrite, TaskWrite, WriteReceipt, WriteRefused,
};

pub use context::{
    unit_start_state, unit_taint_step, DerivedItem, DispatchContext, PolicySnapshot, Principal,
    PromptBuilder, PromptEnvelope, RecalledRecord, RecordRef, Restrictions, RouteClass, Unit,
    WorkingSet,
};

use std::collections::BTreeMap;
use std::fmt;

use nexus_persistence::hm::{
    self as store, HmTx, LabelRow, MemberRow, MigrationRow, PolicyChangeRow, PolicyRow, ScannedRow,
};
use nexus_persistence::{NexusDatabase, PersistenceError};
use uuid::Uuid;

/// `hm_migration.contract_version` written by T₀.
pub const CONTRACT_VERSION: &str = "P3-HM-CONTRACT-02";

/// AD-5 Option A: every registered agent starts local-only (§9.1 step 3).
const OPTION_A_LOCAL_ONLY: i64 = 1;

// ── Errors ──────────────────────────────────────────────────────────────────

/// A failure to establish historical-memory state. Callers treat every
/// error as `Unresolved` (§9.3): it never grants anything.
#[derive(Debug)]
pub enum HmError {
    Store(PersistenceError),
    /// `hm_migration` exists but holds no single `COMPLETE` row.
    CorruptMigrationState,
    /// A ledger row names a table outside §2.1.
    CorruptLedger,
}

impl fmt::Display for HmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HmError::Store(e) => write!(f, "historical-memory store: {e}"),
            HmError::CorruptMigrationState => {
                write!(f, "historical-memory migration state corrupt")
            }
            HmError::CorruptLedger => write!(f, "historical-memory ledger names an unknown table"),
        }
    }
}

impl std::error::Error for HmError {}

impl From<PersistenceError> for HmError {
    fn from(e: PersistenceError) -> Self {
        HmError::Store(e)
    }
}

// ── Labels ──────────────────────────────────────────────────────────────────

/// A stored origin label, ordered by restriction (§2.3). There is no public
/// `FromStr` and no public constructor from a string: stored text is decoded
/// only by the private `decode_origin`.
///
/// Positive control (compiles):
/// ```
/// use nexus_kernel::hm::OriginLabel;
/// let l = OriginLabel::LegacyUnknown;
/// assert_eq!(l.class_name(), "LEGACY_UNKNOWN");
/// ```
/// No parse, conversion or deserialisation from a string:
/// ```compile_fail
/// use nexus_kernel::hm::OriginLabel;
/// let _ = "UNRESTRICTED_ORIGIN".parse::<OriginLabel>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OriginLabel;
/// let _ = OriginLabel::from("UNRESTRICTED_ORIGIN");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OriginLabel;
/// let _ = OriginLabel::from(String::from("UNRESTRICTED_ORIGIN"));
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OriginLabel;
/// let _ = <OriginLabel as TryFrom<&str>>::try_from("UNRESTRICTED_ORIGIN");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OriginLabel;
/// let _ = serde_json::from_str::<OriginLabel>("\"UNRESTRICTED_ORIGIN\"");
/// ```
/// ```compile_fail
/// let _ = nexus_kernel::hm::decode_origin("UNRESTRICTED_ORIGIN");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginLabel {
    UnrestrictedOrigin,
    LegacyUnknown,
    LocalOnlyOrigin,
}

impl OriginLabel {
    /// The class name used in storage and in event fields.
    pub fn class_name(self) -> &'static str {
        match self {
            OriginLabel::UnrestrictedOrigin => "UNRESTRICTED_ORIGIN",
            OriginLabel::LegacyUnknown => "LEGACY_UNKNOWN",
            OriginLabel::LocalOnlyOrigin => "LOCAL_ONLY_ORIGIN",
        }
    }

    /// Join (⊔): the more restrictive of the two.
    pub fn join(self, other: OriginLabel) -> OriginLabel {
        self.max(other)
    }
}

fn decode_origin(stored: &str) -> Option<OriginLabel> {
    match stored {
        "UNRESTRICTED_ORIGIN" => Some(OriginLabel::UnrestrictedOrigin),
        "LEGACY_UNKNOWN" => Some(OriginLabel::LegacyUnknown),
        "LOCAL_ONLY_ORIGIN" => Some(OriginLabel::LocalOnlyOrigin),
        _ => None,
    }
}

/// The label a record evaluates to (§2.3). `Unbound` is never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatedLabel {
    /// A label row exists, decodes, and binds `(table, id, agent_id, digest)`.
    Bound(OriginLabel),
    /// Missing, unreadable, malformed or binding-mismatched label, or no
    /// record: evaluates as `LOCAL_ONLY_ORIGIN`.
    Unbound,
}

impl EvaluatedLabel {
    /// The label used for every decision: `Unbound` = `LOCAL_ONLY_ORIGIN`.
    pub fn effective(self) -> OriginLabel {
        match self {
            EvaluatedLabel::Bound(l) => l,
            EvaluatedLabel::Unbound => OriginLabel::LocalOnlyOrigin,
        }
    }
}

/// One of the five in-scope record tables (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordTable {
    AgentMemory,
    TaskHistory,
    StrategyScores,
    WorldModelEntities,
    WorldModelRelationships,
}

impl RecordTable {
    pub const ALL: [RecordTable; 5] = [
        RecordTable::AgentMemory,
        RecordTable::TaskHistory,
        RecordTable::StrategyScores,
        RecordTable::WorldModelEntities,
        RecordTable::WorldModelRelationships,
    ];

    pub fn table_name(self) -> &'static str {
        match self {
            RecordTable::AgentMemory => "agent_memory",
            RecordTable::TaskHistory => "task_history",
            RecordTable::StrategyScores => "strategy_scores",
            RecordTable::WorldModelEntities => "world_model_entities",
            RecordTable::WorldModelRelationships => "world_model_relationships",
        }
    }

    fn from_table_name(name: &str) -> Option<RecordTable> {
        RecordTable::ALL
            .into_iter()
            .find(|t| t.table_name() == name)
    }
}

/// The §5.2 digest-covered columns of an in-scope table.
fn digest_columns_of(table: RecordTable) -> &'static [&'static str] {
    store::digest_columns(table.table_name()).unwrap_or(&[])
}

/// The §5.2 binding rule. `label` is `(agent_id, origin, content_sha256)`
/// of the record's label row, or `None` when the row is missing or
/// unreadable. The label binds only when its origin decodes and its agent
/// and digest equal the current row's; anything else is `Unbound`.
fn bind(label: Option<(&str, &str, &str)>, current: &ScannedRow) -> EvaluatedLabel {
    let Some((agent_id, origin, content_sha256)) = label else {
        return EvaluatedLabel::Unbound;
    };
    let Some(origin) = decode_origin(origin) else {
        return EvaluatedLabel::Unbound;
    };
    if agent_id != current.agent_id || content_sha256 != current.content_sha256 {
        return EvaluatedLabel::Unbound;
    }
    EvaluatedLabel::Bound(origin)
}

/// Evaluates one record's label against its current row (§5.2 binding rule).
fn evaluate_in(
    tx: &HmTx<'_>,
    table: RecordTable,
    record_id: &str,
    current: Option<&ScannedRow>,
) -> Result<EvaluatedLabel, HmError> {
    let Some(current) = current else {
        return Ok(EvaluatedLabel::Unbound);
    };
    let label = tx.label_row(table.table_name(), record_id)?;
    Ok(bind(
        label.as_ref().map(|l| {
            (
                l.agent_id.as_str(),
                l.origin.as_str(),
                l.content_sha256.as_str(),
            )
        }),
        current,
    ))
}

/// Whether the T₀ migration is recorded as complete. A missing migration
/// table means not migrated; anything else malformed is an error.
fn migrated(tx: &HmTx<'_>) -> Result<bool, HmError> {
    if !tx.table_exists("hm_migration")? {
        return Ok(false);
    }
    match tx.migration_rows()?.as_slice() {
        [m] if m.state == "COMPLETE" => Ok(true),
        _ => Err(HmError::CorruptMigrationState),
    }
}

/// The evaluated label of one record. Before T₀ every record is `Unbound`.
pub fn evaluate_label(
    db: &NexusDatabase,
    table: RecordTable,
    record_id: &str,
) -> Result<EvaluatedLabel, HmError> {
    db.hm()
        .immediate(|tx| Ok(evaluate_migrated(tx, table, record_id)))?
}

fn evaluate_migrated(
    tx: &HmTx<'_>,
    table: RecordTable,
    record_id: &str,
) -> Result<EvaluatedLabel, HmError> {
    if !migrated(tx)? {
        return Ok(EvaluatedLabel::Unbound);
    }
    let current = tx.current_row(table.table_name(), record_id)?;
    evaluate_in(tx, table, record_id, current.as_ref())
}

/// Whether a record identity is in the T₀ membership ledger. Membership is
/// read from `hm_t0_member` only, never from a timestamp or id watermark
/// (§2.9, §9.5).
pub fn is_t0_member(
    db: &NexusDatabase,
    table: RecordTable,
    record_id: &str,
) -> Result<bool, HmError> {
    db.hm()
        .immediate(|tx| Ok(ledger_has(tx, table, record_id)))?
}

fn ledger_has(tx: &HmTx<'_>, table: RecordTable, record_id: &str) -> Result<bool, HmError> {
    if !migrated(tx)? {
        return Ok(false);
    }
    Ok(tx.ledger_member(table.table_name(), record_id)?.is_some())
}

// ── T₀ migration (§9.1) ─────────────────────────────────────────────────────

/// Points inside the T₀ transaction at which a probe is called (after the
/// named §9.1 step, before COMMIT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0Point {
    /// Step 1: tables and triggers created.
    SchemaCreated,
    /// Step 2: every in-scope row ledgered and labelled `LEGACY_UNKNOWN`.
    MembersLabelled,
    /// Step 3: Option A policy and history rows written.
    PoliciesWritten,
    /// Step 4: `hm_migration` written; the ledger is closed.
    MigrationRowWritten,
    /// Step 5: `memory.migration_t0` written; next is COMMIT (step 6).
    EventWritten,
}

impl T0Point {
    pub const ALL: [T0Point; 5] = [
        T0Point::SchemaCreated,
        T0Point::MembersLabelled,
        T0Point::PoliciesWritten,
        T0Point::MigrationRowWritten,
        T0Point::EventWritten,
    ];

    pub fn name(self) -> &'static str {
        match self {
            T0Point::SchemaCreated => "schema_created",
            T0Point::MembersLabelled => "members_labelled",
            T0Point::PoliciesWritten => "policies_written",
            T0Point::MigrationRowWritten => "migration_row_written",
            T0Point::EventWritten => "event_written",
        }
    }
}

/// Result of a T₀ run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum T0Outcome {
    /// This call committed T₀.
    Migrated {
        member_count: u64,
        member_set_sha256: String,
    },
    /// T₀ was already complete; nothing was written.
    AlreadyComplete,
}

/// Runs the T₀ migration with the AD-5 Option A baseline. `registered_agents`
/// are the supervisor-registered agent UUIDs, supplied by the caller.
/// §9.1 step 7 (config mirror) is not part of this function.
pub fn run_t0_option_a(
    db: &NexusDatabase,
    registered_agents: &[Uuid],
) -> Result<T0Outcome, HmError> {
    run_t0_option_a_probed(db, registered_agents, &mut |_| {})
}

/// `run_t0_option_a` with a probe called inside the transaction after each
/// step. The probe observes only; it cannot change what is written. Used by
/// the crash and concurrency tests (A-M2, A-M3).
#[doc(hidden)]
pub fn run_t0_option_a_probed(
    db: &NexusDatabase,
    registered_agents: &[Uuid],
    probe: &mut dyn FnMut(T0Point),
) -> Result<T0Outcome, HmError> {
    let mut agents: Vec<String> = registered_agents.iter().map(Uuid::to_string).collect();
    agents.sort();
    agents.dedup();
    let t0_wallclock = chrono::Utc::now().to_rfc3339();

    db.hm().immediate(|tx| {
        if tx.table_exists("hm_migration")? {
            return Ok(match tx.migration_rows()?.as_slice() {
                [m] if m.state == "COMPLETE" => Ok(T0Outcome::AlreadyComplete),
                _ => Err(HmError::CorruptMigrationState),
            });
        }

        // Step 1: §5.1 tables and triggers.
        tx.create_schema()?;
        probe(T0Point::SchemaCreated);

        // Step 2: ledger and LEGACY_UNKNOWN label for every in-scope row.
        let mut rows_per_table: BTreeMap<&'static str, u64> = BTreeMap::new();
        for table in RecordTable::ALL {
            let name = table.table_name();
            let rows = tx.scan_table(name)?;
            for row in &rows {
                tx.insert_member(&MemberRow {
                    record_table: name.to_string(),
                    record_id: row.record_id.clone(),
                    agent_id: row.agent_id.clone(),
                    t0_content_sha256: row.content_sha256.clone(),
                })?;
                tx.insert_label(&LabelRow {
                    record_table: name.to_string(),
                    record_id: row.record_id.clone(),
                    agent_id: row.agent_id.clone(),
                    origin: OriginLabel::LegacyUnknown.class_name().to_string(),
                    source_kind: "legacy".to_string(),
                    content_sha256: row.content_sha256.clone(),
                    policy_generation: 0,
                })?;
            }
            rows_per_table.insert(name, rows.len() as u64);
        }
        probe(T0Point::MembersLabelled);

        // Step 3: AD-5 Option A. History row first (§5.1 trigger).
        for agent_id in &agents {
            tx.insert_policy_change(&PolicyChangeRow {
                agent_id: agent_id.clone(),
                policy_generation: 1,
                local_only: OPTION_A_LOCAL_ONLY,
                basis: "t0_baseline".to_string(),
                cause: "t0_baseline".to_string(),
            })?;
            tx.insert_policy(&PolicyRow {
                agent_id: agent_id.clone(),
                local_only: OPTION_A_LOCAL_ONLY,
                policy_generation: 1,
                revocation_seq: 0,
                basis: "t0_baseline".to_string(),
            })?;
        }
        probe(T0Point::PoliciesWritten);

        // Step 4: count and digest over the ledger rows just written.
        let ledger = tx.ledger_rows()?;
        let member_count = ledger.len() as u64;
        let member_set_sha256 = store::member_set_digest(&ledger);
        tx.insert_migration(&MigrationRow {
            contract_version: CONTRACT_VERSION.to_string(),
            state: "COMPLETE".to_string(),
            t0_wallclock: t0_wallclock.clone(),
            member_count: member_count as i64,
            member_set_sha256: member_set_sha256.clone(),
        })?;
        probe(T0Point::MigrationRowWritten);

        // Step 5: content-free event: counts and the member-set digest only.
        let fields = serde_json::json!({
            "rows": rows_per_table,
            "member_count": member_count,
            "member_set_sha256": member_set_sha256,
        });
        tx.insert_event(
            "migration:t0",
            "memory.migration_t0",
            None,
            &fields.to_string(),
        )?;
        probe(T0Point::EventWritten);

        Ok(Ok(T0Outcome::Migrated {
            member_count,
            member_set_sha256,
        }))
        // Step 6: `immediate` commits here. This commit is T₀.
    })?
}

// ── Start-up verification (§9.5) ────────────────────────────────────────────

/// The ledger checks. A failure means corruption; it can only disable bulk
/// release, never widen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerChecks {
    /// L1: ledger row count = `hm_migration.member_count`.
    pub l1_count: bool,
    /// L2: recomputed ledger digest = `hm_migration.member_set_sha256`.
    pub l2_digest: bool,
    /// L3: every tombstone references a ledger row (by query).
    pub l3_tombstones: bool,
    /// L4: the ledger, tombstone and delete triggers are present with their
    /// expected definitions.
    pub l4_triggers: bool,
}

impl LedgerChecks {
    pub fn all_pass(&self) -> bool {
        self.l1_count && self.l2_digest && self.l3_tombstones && self.l4_triggers
    }

    /// Names of the failed checks (class names only).
    pub fn failed(&self) -> Vec<&'static str> {
        [
            (self.l1_count, "L1"),
            (self.l2_digest, "L2"),
            (self.l3_tombstones, "L3"),
            (self.l4_triggers, "L4"),
        ]
        .into_iter()
        .filter(|(ok, _)| !ok)
        .map(|(_, n)| n)
        .collect()
    }
}

/// Per-member state (§9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberState {
    /// The row exists and has no tombstone. `label` is its evaluated label:
    /// `Bound` when its label row binds, else `Unbound`. `unchanged_since_t0`
    /// is true when its current agent and digest equal the ledger's.
    Present {
        label: EvaluatedLabel,
        unchanged_since_t0: bool,
    },
    /// The row is absent and has a tombstone.
    Deleted,
    /// The row is absent and has no tombstone (inconsistent).
    MissingUntombstoned,
    /// A tombstone exists but a row with that identity is present
    /// (inconsistent). Never in bulk scope; evaluated by its own label.
    Reappeared { label: EvaluatedLabel },
}

/// One ledger member and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberEvaluation {
    pub table: RecordTable,
    pub record_id: String,
    pub agent_id: String,
    pub state: MemberState,
}

/// Result of start-up verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerReport {
    pub checks: LedgerChecks,
    pub members: Vec<MemberEvaluation>,
    /// `memory.member_set_mismatch` was written (only when a check failed).
    pub mismatch_event_written: bool,
    /// `memory.member_record_inconsistent` was written (only when a member
    /// is missing-untombstoned or reappeared).
    pub inconsistency_event_written: bool,
}

/// Start-up status of the historical-memory store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartStatus {
    /// T₀ has not run: every agent is `Unresolved` (§9.3).
    NotMigrated,
    Verified(LedgerReport),
}

/// The L1–L4 checks and member states, read inside one transaction.
/// `None` when T₀ has not run.
#[allow(clippy::type_complexity)]
fn check_ledger(tx: &HmTx<'_>) -> Result<Option<(LedgerChecks, Vec<MemberEvaluation>)>, HmError> {
    if !migrated(tx)? {
        return Ok(None);
    }
    let migration = tx
        .migration_rows()?
        .pop()
        .ok_or(HmError::CorruptMigrationState)?;
    let ledger = tx.ledger_rows()?;

    let l1_count = tx.ledger_count()? == migration.member_count;
    let l2_digest = store::member_set_digest(&ledger) == migration.member_set_sha256;
    let l3_tombstones = tx.orphan_tombstone_count()? == 0;
    let mut l4_triggers = true;
    for (name, expected) in store::L4_TRIGGERS {
        if tx.trigger_sql(name)?.as_deref() != Some(expected) {
            l4_triggers = false;
        }
    }

    let mut members = Vec::with_capacity(ledger.len());
    for m in &ledger {
        let table = RecordTable::from_table_name(&m.record_table).ok_or(HmError::CorruptLedger)?;
        let tombstoned = tx.is_tombstoned(&m.record_table, &m.record_id)?;
        let current = tx.current_row(&m.record_table, &m.record_id)?;
        let state = match (tombstoned, current.as_ref()) {
            (false, Some(row)) => MemberState::Present {
                label: evaluate_in(tx, table, &m.record_id, Some(row))?,
                unchanged_since_t0: row.agent_id == m.agent_id
                    && row.content_sha256 == m.t0_content_sha256,
            },
            (true, None) => MemberState::Deleted,
            (false, None) => MemberState::MissingUntombstoned,
            (true, Some(row)) => MemberState::Reappeared {
                label: evaluate_in(tx, table, &m.record_id, Some(row))?,
            },
        };
        members.push(MemberEvaluation {
            table,
            record_id: m.record_id.clone(),
            agent_id: m.agent_id.clone(),
            state,
        });
    }
    Ok(Some((
        LedgerChecks {
            l1_count,
            l2_digest,
            l3_tombstones,
            l4_triggers,
        },
        members,
    )))
}

/// Checks L1–L4 over the ledger (never over live labels) and evaluates every
/// member, in one `BEGIN IMMEDIATE` read. Observational events are then
/// written best effort in a separate transaction; their failure changes no
/// result (§5.3).
pub fn verify_at_start(db: &NexusDatabase) -> Result<StartStatus, HmError> {
    let checked = db.hm().immediate(|tx| Ok(check_ledger(tx)))??;

    let Some((checks, members)) = checked else {
        return Ok(StartStatus::NotMigrated);
    };

    let mismatch_event_written = !checks.all_pass()
        && emit_observational(
            db,
            "memory.member_set_mismatch",
            serde_json::json!({ "failed_checks": checks.failed() }),
        );
    let missing = members
        .iter()
        .filter(|m| m.state == MemberState::MissingUntombstoned)
        .count();
    let reappeared = members
        .iter()
        .filter(|m| matches!(m.state, MemberState::Reappeared { .. }))
        .count();
    let inconsistency_event_written = missing + reappeared > 0
        && emit_observational(
            db,
            "memory.member_record_inconsistent",
            serde_json::json!({ "missing_untombstoned": missing, "reappeared": reappeared }),
        );

    Ok(StartStatus::Verified(LedgerReport {
        checks,
        members,
        mismatch_event_written,
        inconsistency_event_written,
    }))
}

/// Writes one observational event with subject `obs:<kind>:<128-bit random
/// id>` (§5.3). Best effort: returns whether it was written.
fn emit_observational(db: &NexusDatabase, kind: &str, fields: serde_json::Value) -> bool {
    let mut id = [0u8; 16];
    if getrandom::getrandom(&mut id).is_err() {
        return false;
    }
    let subject = format!("obs:{kind}:{}", hex::encode(id));
    db.hm()
        .immediate(|tx| tx.insert_event(&subject, kind, None, &fields.to_string()))
        .is_ok()
}
