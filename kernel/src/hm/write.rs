//! Labelled writes (P3-HM-CONTRACT-02 §6.1–§6.3, §6.5–§6.7, §14).
//!
//! Every write is one `BEGIN IMMEDIATE` covering the record row, its label
//! row and its derivation edges (§6.2). The label is computed from the
//! context and the explicit parents only (§6.1):
//! `unit taint ⊔ every verbatim label in the working set ⊔ the parents`,
//! with each record label re-evaluated inside the transaction. Content is
//! never read to decide a label (§1).

use std::fmt;

pub use nexus_persistence::hm::TaskBookkeeping;
use nexus_persistence::hm::{HmTx, LabelRow, NewMemoryRow, NewTaskRow, StrategyContribution};
use nexus_persistence::{PersistenceError, Result as StoreResult};
use uuid::Uuid;

use super::context::{DerivedItem, DispatchContext, Principal, RecordRef};
use super::issuer::{decode_policy, DispatchIssuer, ProbePoint};
use super::{evaluate_in, migrated, HmError, OriginLabel, RecordTable};

/// The closed set of persisted `source_kind` values (§6.5). The role of a
/// record comes from this type, never from a substring (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    MemoryStoreAction,
    LoopEpisodic,
    EvolutionProcedural,
    StrategyScore,
    CrossAgentLearning,
    GoalRecord,
    OptimizedPrompt,
    PlannerStrategyInjection,
    CognitiveParam,
    ModelMapping,
    DbMemoryStore,
    KnowledgeGraphReplace,
}

impl SourceKind {
    pub fn name(self) -> &'static str {
        match self {
            SourceKind::MemoryStoreAction => "memory_store_action",
            SourceKind::LoopEpisodic => "loop_episodic",
            SourceKind::EvolutionProcedural => "evolution_procedural",
            SourceKind::StrategyScore => "strategy_score",
            SourceKind::CrossAgentLearning => "cross_agent_learning",
            SourceKind::GoalRecord => "goal_record",
            SourceKind::OptimizedPrompt => "optimized_prompt",
            SourceKind::PlannerStrategyInjection => "planner_strategy_injection",
            SourceKind::CognitiveParam => "cognitive_param",
            SourceKind::ModelMapping => "model_mapping",
            SourceKind::DbMemoryStore => "db_memory_store",
            SourceKind::KnowledgeGraphReplace => "knowledge_graph_replace",
        }
    }
}

/// An explicit parent of a write (§6.3): a record (its evaluated label
/// joins and a derivation edge is written) or a derived item (its fixed
/// label joins).
#[derive(Debug, Clone, Copy)]
pub enum Parent<'a> {
    Record(&'a RecordRef),
    Derived(&'a DerivedItem),
}

/// A new `agent_memory` record.
#[derive(Debug, Clone, Copy)]
pub struct NewMemory<'a> {
    pub source_kind: SourceKind,
    pub memory_type: &'a str,
    pub key: &'a str,
    pub value_json: &'a str,
    pub relevance_score: f64,
}

/// A `task_history` start or progress write (`goal_record`). The row is
/// created on first write; later writes change its progress columns.
#[derive(Debug, Clone, Copy)]
pub struct TaskWrite<'a> {
    pub task_id: Uuid,
    pub goal: &'a str,
    pub status: &'a str,
    pub steps_json: &'a str,
    pub result_json: Option<&'a str>,
    pub completed: bool,
    pub success: bool,
}

/// One contribution to the `strategy_scores` aggregate of
/// `(agent, strategy_hash)` (`strategy_score`).
#[derive(Debug, Clone, Copy)]
pub struct StrategyWrite<'a> {
    pub strategy_hash: &'a str,
    pub goal_type: &'a str,
    pub success: bool,
    pub fuel: f64,
    pub duration_secs: f64,
}

/// A committed labelled write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReceipt {
    reference: RecordRef,
    label: OriginLabel,
}

impl WriteReceipt {
    pub fn record_ref(&self) -> &RecordRef {
        &self.reference
    }

    /// The record's label as committed.
    pub fn label(&self) -> OriginLabel {
        self.label
    }
}

/// A refused write: nothing was committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteRefused {
    /// `hm_migration` is not `COMPLETE` (§9.4).
    NotMigrated,
    WrongIssuer,
    /// Only an agent context can write an agent's record.
    NotAnAgent,
    /// The record to update or copy does not exist.
    UnknownRecord,
    /// The record belongs to another agent.
    WrongAgent,
    /// Cross-agent copy of a table other than `agent_memory`.
    UnsupportedSource,
    /// §14 rule 3: the copy is restricted and the target is not local-only.
    CrossAgentRestricted,
    /// The target's policy row is missing or malformed.
    TargetPolicyUnresolved,
    Store,
}

impl fmt::Display for WriteRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "labelled write refused: {self:?}")
    }
}

impl std::error::Error for WriteRefused {}

type Outcome = Result<WriteReceipt, WriteRefused>;

/// Any failure inside a write transaction is returned as a store error, so
/// the transaction rolls back: a record is never committed without its
/// label (§6.2). Refusals are decided before the first write.
pub(in crate::hm) fn abort(e: HmError) -> PersistenceError {
    match e {
        HmError::Store(p) => p,
        other => PersistenceError::NotFound(other.to_string()),
    }
}

pub(in crate::hm) fn is_migrated(tx: &HmTx<'_>) -> StoreResult<bool> {
    migrated(tx).map_err(abort)
}

/// The current evaluated label of a record, inside the transaction.
pub(in crate::hm) fn evaluate_ref(tx: &HmTx<'_>, r: &RecordRef) -> StoreResult<OriginLabel> {
    let current = tx.current_row(r.table.table_name(), &r.record_id)?;
    Ok(evaluate_in(tx, r.table, &r.record_id, current.as_ref())
        .map_err(abort)?
        .effective())
}

/// The write label (§6.3) and the record parents with their labels. A
/// verbatim item of the working set contributes the label evaluated when it
/// entered ⊔ the label evaluated now (mission R1 (i)).
fn write_label(
    tx: &HmTx<'_>,
    ctx: &DispatchContext,
    parents: &[Parent<'_>],
) -> StoreResult<(OriginLabel, Vec<(RecordRef, OriginLabel)>)> {
    let mut label = ctx.taint;
    for (r, at_entry) in &ctx.working_set.verbatim {
        label = label.join(at_entry.effective()).join(evaluate_ref(tx, r)?);
    }
    let mut edges: Vec<(RecordRef, OriginLabel)> = Vec::new();
    for p in parents {
        match p {
            Parent::Record(r) => {
                let l = evaluate_ref(tx, r)?;
                label = label.join(l);
                if !edges.iter().any(|(e, _)| e == *r) {
                    edges.push(((*r).clone(), l));
                }
            }
            Parent::Derived(d) => label = label.join(d.label),
        }
    }
    Ok((label, edges))
}

fn insert_edges(
    tx: &HmTx<'_>,
    child: &RecordRef,
    edges: &[(RecordRef, OriginLabel)],
) -> StoreResult<()> {
    for (parent, l) in edges {
        tx.insert_derivation(
            child.table.table_name(),
            &child.record_id,
            parent.table.table_name(),
            &parent.record_id,
            l.class_name(),
        )?;
    }
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl DispatchIssuer {
    fn writer_agent(&self, ctx: &DispatchContext) -> Result<Uuid, WriteRefused> {
        if !self.issued(ctx) {
            return Err(WriteRefused::WrongIssuer);
        }
        match ctx.principal {
            Principal::Agent(a) => Ok(a),
            _ => Err(WriteRefused::NotAnAgent),
        }
    }

    /// Labels and inserts a new record row in the open transaction.
    fn label_new(
        &self,
        tx: &HmTx<'_>,
        ctx: &DispatchContext,
        reference: RecordRef,
        source_kind: SourceKind,
        label: OriginLabel,
        edges: &[(RecordRef, OriginLabel)],
    ) -> StoreResult<WriteReceipt> {
        let row = tx
            .current_row(reference.table.table_name(), &reference.record_id)?
            .ok_or_else(|| PersistenceError::NotFound("written row".into()))?;
        tx.insert_label(&LabelRow {
            record_table: reference.table.table_name().to_string(),
            record_id: reference.record_id.clone(),
            agent_id: row.agent_id,
            origin: label.class_name().to_string(),
            source_kind: source_kind.name().to_string(),
            content_sha256: row.content_sha256,
            policy_generation: ctx.policy.policy_generation,
        })?;
        self.hit(ProbePoint::WriteLabelWritten);
        insert_edges(tx, &reference, edges)?;
        self.hit(ProbePoint::WriteEdgesWritten);
        Ok(WriteReceipt { reference, label })
    }

    /// After a content update in the open transaction: the label becomes
    /// `evaluated label before the update ⊔ write label` (never lower) and
    /// binds the new digest. The T₀ ledger is not touched (§6.7).
    #[allow(clippy::too_many_arguments)]
    fn label_update(
        &self,
        tx: &HmTx<'_>,
        ctx: &DispatchContext,
        reference: RecordRef,
        before: OriginLabel,
        source_kind: SourceKind,
        label: OriginLabel,
        edges: &[(RecordRef, OriginLabel)],
    ) -> StoreResult<WriteReceipt> {
        let table = reference.table.table_name();
        let row = tx
            .current_row(table, &reference.record_id)?
            .ok_or_else(|| PersistenceError::NotFound("updated row".into()))?;
        let origin = before.join(label);
        if tx.label_row(table, &reference.record_id)?.is_some() {
            tx.update_label(
                table,
                &reference.record_id,
                origin.class_name(),
                &row.content_sha256,
            )?;
        } else {
            tx.insert_label(&LabelRow {
                record_table: table.to_string(),
                record_id: reference.record_id.clone(),
                agent_id: row.agent_id,
                origin: origin.class_name().to_string(),
                source_kind: source_kind.name().to_string(),
                content_sha256: row.content_sha256,
                policy_generation: ctx.policy.policy_generation,
            })?;
        }
        self.hit(ProbePoint::WriteLabelWritten);
        insert_edges(tx, &reference, edges)?;
        self.hit(ProbePoint::WriteEdgesWritten);
        Ok(WriteReceipt {
            reference,
            label: origin,
        })
    }

    /// Writes a new `agent_memory` record with its label and edges.
    pub fn write_memory(
        &self,
        ctx: &DispatchContext,
        memory: &NewMemory<'_>,
        parents: &[Parent<'_>],
    ) -> Outcome {
        let agent_id = self.writer_agent(ctx)?;
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                let (label, edges) = write_label(tx, ctx, parents)?;
                let id = tx.insert_agent_memory(&NewMemoryRow {
                    agent_id: agent_id.to_string(),
                    memory_type: memory.memory_type.to_string(),
                    key: memory.key.to_string(),
                    value_json: memory.value_json.to_string(),
                    relevance_score: memory.relevance_score,
                    created_at: now(),
                })?;
                self.hit(ProbePoint::WriteRecordWritten);
                let reference = RecordRef {
                    table: RecordTable::AgentMemory,
                    record_id: id.to_string(),
                };
                Ok(Ok(self.label_new(
                    tx,
                    ctx,
                    reference,
                    memory.source_kind,
                    label,
                    &edges,
                )?))
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// Creates the task row, or updates its progress columns as a content
    /// update (`goal_record`, §6.7).
    pub fn save_task(
        &self,
        ctx: &DispatchContext,
        task: &TaskWrite<'_>,
        parents: &[Parent<'_>],
    ) -> Outcome {
        let agent_id = self.writer_agent(ctx)?;
        let id = task.task_id.to_string();
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                let (label, edges) = write_label(tx, ctx, parents)?;
                let reference = RecordRef {
                    table: RecordTable::TaskHistory,
                    record_id: id.clone(),
                };
                let completed_at = task.completed.then(now);
                match tx.current_row("task_history", &id)? {
                    None => {
                        tx.insert_task(&NewTaskRow {
                            id: id.clone(),
                            agent_id: agent_id.to_string(),
                            goal: task.goal.to_string(),
                            status: task.status.to_string(),
                            steps_json: task.steps_json.to_string(),
                            result_json: task.result_json.map(str::to_string),
                            started_at: now(),
                            completed_at,
                            success: task.success,
                        })?;
                        self.hit(ProbePoint::WriteRecordWritten);
                        Ok(Ok(self.label_new(
                            tx,
                            ctx,
                            reference,
                            SourceKind::GoalRecord,
                            label,
                            &edges,
                        )?))
                    }
                    Some(row) if row.agent_id != agent_id.to_string() => {
                        Ok(Err(WriteRefused::WrongAgent))
                    }
                    Some(row) => {
                        let before = evaluate_in(tx, reference.table, &id, Some(&row))
                            .map_err(abort)?
                            .effective();
                        tx.update_task_progress(
                            &id,
                            task.status,
                            task.steps_json,
                            task.result_json,
                            completed_at.as_deref(),
                            task.success,
                        )?;
                        self.hit(ProbePoint::WriteRecordWritten);
                        Ok(Ok(self.label_update(
                            tx,
                            ctx,
                            reference,
                            before,
                            SourceKind::GoalRecord,
                            label,
                            &edges,
                        )?))
                    }
                }
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// Adds one contribution to the agent's strategy aggregate. Every
    /// contribution joins the aggregate's label (§6.7); the last writer
    /// never decides it.
    pub fn upsert_strategy(
        &self,
        ctx: &DispatchContext,
        s: &StrategyWrite<'_>,
        parents: &[Parent<'_>],
    ) -> Outcome {
        let agent_id = self.writer_agent(ctx)?;
        let agent = agent_id.to_string();
        let contribution = StrategyContribution {
            agent_id: agent.clone(),
            strategy_hash: s.strategy_hash.to_string(),
            goal_type: s.goal_type.to_string(),
            success: s.success,
            fuel: s.fuel,
            duration_secs: s.duration_secs,
            updated_at: now(),
        };
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                let (label, edges) = write_label(tx, ctx, parents)?;
                match tx.strategy_id(&agent, s.strategy_hash)? {
                    None => {
                        let id = tx.insert_strategy(&contribution)?;
                        self.hit(ProbePoint::WriteRecordWritten);
                        let reference = RecordRef {
                            table: RecordTable::StrategyScores,
                            record_id: id.to_string(),
                        };
                        Ok(Ok(self.label_new(
                            tx,
                            ctx,
                            reference,
                            SourceKind::StrategyScore,
                            label,
                            &edges,
                        )?))
                    }
                    Some(id) => {
                        let reference = RecordRef {
                            table: RecordTable::StrategyScores,
                            record_id: id.to_string(),
                        };
                        let before = evaluate_ref(tx, &reference)?;
                        tx.add_strategy_contribution(id, &contribution)?;
                        self.hit(ProbePoint::WriteRecordWritten);
                        Ok(Ok(self.label_update(
                            tx,
                            ctx,
                            reference,
                            before,
                            SourceKind::StrategyScore,
                            label,
                            &edges,
                        )?))
                    }
                }
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// Cross-agent copy (§14 rule 3): a new `agent_memory` record of the
    /// target agent carrying the source's evaluated label ⊔ the target
    /// unit's label at write, with a derivation edge to the source. Refused
    /// when that label is not `UNRESTRICTED_ORIGIN` and the target agent is
    /// not local-only in the store.
    pub fn copy_to_agent(&self, target: &DispatchContext, source: &RecordRef) -> Outcome {
        let agent_id = self.writer_agent(target)?;
        if source.table != RecordTable::AgentMemory {
            return Err(WriteRefused::UnsupportedSource);
        }
        let Ok(source_id) = source.record_id.parse::<i64>() else {
            return Err(WriteRefused::UnknownRecord);
        };
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                let Some(content) = tx.memory_content(source_id)? else {
                    return Ok(Err(WriteRefused::UnknownRecord));
                };
                let (label, edges) = write_label(tx, target, &[Parent::Record(source)])?;
                if label != OriginLabel::UnrestrictedOrigin {
                    let target_local = tx
                        .policy_row_raw(&agent_id.to_string())?
                        .map(|raw| decode_policy(agent_id, &raw).map(|p| p.local_only));
                    match target_local {
                        Some(Ok(true)) => {}
                        Some(Ok(false)) => return Ok(Err(WriteRefused::CrossAgentRestricted)),
                        _ => return Ok(Err(WriteRefused::TargetPolicyUnresolved)),
                    }
                }
                let id = tx.insert_agent_memory(&NewMemoryRow {
                    agent_id: agent_id.to_string(),
                    memory_type: content.memory_type,
                    key: content.key,
                    value_json: content.value_json,
                    relevance_score: 1.0,
                    created_at: now(),
                })?;
                self.hit(ProbePoint::WriteRecordWritten);
                let reference = RecordRef {
                    table: RecordTable::AgentMemory,
                    record_id: id.to_string(),
                };
                Ok(Ok(self.label_new(
                    tx,
                    target,
                    reference,
                    SourceKind::CrossAgentLearning,
                    label,
                    &edges,
                )?))
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// A new `agent_memory` record of the target agent whose content is
    /// derived from another agent's record (§14 rule 3, `share_learning`):
    /// labelled with the source's evaluated label ⊔ the target unit's label
    /// at write, with a derivation edge to the source. Refused when that
    /// label is not `UNRESTRICTED_ORIGIN` and the target agent is not
    /// local-only in the store. A grant on the source never counts.
    pub fn share_to_agent(
        &self,
        target: &DispatchContext,
        memory: &NewMemory<'_>,
        source: &RecordRef,
    ) -> Outcome {
        let agent_id = self.writer_agent(target)?;
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                if tx
                    .current_row(source.table.table_name(), &source.record_id)?
                    .is_none()
                {
                    return Ok(Err(WriteRefused::UnknownRecord));
                }
                let (label, edges) = write_label(tx, target, &[Parent::Record(source)])?;
                if label != OriginLabel::UnrestrictedOrigin {
                    let target_local = tx
                        .policy_row_raw(&agent_id.to_string())?
                        .map(|raw| decode_policy(agent_id, &raw).map(|p| p.local_only));
                    match target_local {
                        Some(Ok(true)) => {}
                        Some(Ok(false)) => return Ok(Err(WriteRefused::CrossAgentRestricted)),
                        _ => return Ok(Err(WriteRefused::TargetPolicyUnresolved)),
                    }
                }
                let id = tx.insert_agent_memory(&NewMemoryRow {
                    agent_id: agent_id.to_string(),
                    memory_type: memory.memory_type.to_string(),
                    key: memory.key.to_string(),
                    value_json: memory.value_json.to_string(),
                    relevance_score: memory.relevance_score,
                    created_at: now(),
                })?;
                self.hit(ProbePoint::WriteRecordWritten);
                let reference = RecordRef {
                    table: RecordTable::AgentMemory,
                    record_id: id.to_string(),
                };
                Ok(Ok(self.label_new(
                    tx,
                    target,
                    reference,
                    SourceKind::CrossAgentLearning,
                    label,
                    &edges,
                )?))
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// The knowledge-graph replace (`knowledge_graph_replace`, §6.5, §6.8):
    /// in one `BEGIN IMMEDIATE`, the agent's entity and relationship rows
    /// are deleted (the §5.1 delete triggers fire: labels and edges go, T₀
    /// members are tombstoned) and the new rows are inserted, each with its
    /// label and edges. Either all of it commits or none of it.
    pub fn replace_world_model(
        &self,
        ctx: &DispatchContext,
        entities: &[String],
        relationships: &[String],
        parents: &[Parent<'_>],
    ) -> Result<Vec<WriteReceipt>, WriteRefused> {
        let agent_id = self.writer_agent(ctx)?;
        let agent = agent_id.to_string();
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                // The label is decided before the delete: a parent deleted
                // by this replace still contributes its label.
                let (label, edges) = write_label(tx, ctx, parents)?;
                tx.delete_world_model(&agent)?;
                let created_at = now();
                let mut receipts = Vec::with_capacity(entities.len() + relationships.len());
                let rows = entities
                    .iter()
                    .map(|e| (RecordTable::WorldModelEntities, e))
                    .chain(
                        relationships
                            .iter()
                            .map(|r| (RecordTable::WorldModelRelationships, r)),
                    );
                for (table, json) in rows {
                    let id = match table {
                        RecordTable::WorldModelEntities => {
                            tx.insert_world_model_entity(&agent, json, &created_at)?
                        }
                        _ => tx.insert_world_model_relationship(&agent, json, &created_at)?,
                    };
                    self.hit(ProbePoint::WriteRecordWritten);
                    let reference = RecordRef {
                        table,
                        record_id: id.to_string(),
                    };
                    receipts.push(self.label_new(
                        tx,
                        ctx,
                        reference,
                        SourceKind::KnowledgeGraphReplace,
                        label,
                        &edges,
                    )?);
                }
                Ok(Ok(receipts))
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// The completion of a task (`goal_record`, §6.7): `status`,
    /// `result_json`, `completed_at` and `success` as a content update, in
    /// one transaction with the label update. `goal` and `steps_json` are
    /// unchanged. The task row must exist and belong to the context's agent.
    pub fn complete_task(
        &self,
        ctx: &DispatchContext,
        task_id: Uuid,
        status: &str,
        result_json: Option<&str>,
        success: bool,
        parents: &[Parent<'_>],
    ) -> Outcome {
        let agent_id = self.writer_agent(ctx)?;
        let id = task_id.to_string();
        self.db()
            .hm()
            .immediate(|tx| {
                if !is_migrated(tx)? {
                    return Ok(Err(WriteRefused::NotMigrated));
                }
                let reference = RecordRef {
                    table: RecordTable::TaskHistory,
                    record_id: id.clone(),
                };
                let Some(row) = tx.current_row("task_history", &id)? else {
                    return Ok(Err(WriteRefused::UnknownRecord));
                };
                if row.agent_id != agent_id.to_string() {
                    return Ok(Err(WriteRefused::WrongAgent));
                }
                let (label, edges) = write_label(tx, ctx, parents)?;
                let before = evaluate_in(tx, reference.table, &id, Some(&row))
                    .map_err(abort)?
                    .effective();
                tx.update_task_completion(&id, status, result_json, &now(), success)?;
                self.hit(ProbePoint::WriteRecordWritten);
                Ok(Ok(self.label_update(
                    tx,
                    ctx,
                    reference,
                    before,
                    SourceKind::GoalRecord,
                    label,
                    &edges,
                )?))
            })
            .map_err(|_| WriteRefused::Store)?
    }

    /// Bookkeeping of a task row (§6.7): status, fuel, completion time and
    /// success, none of them covered by the §5.2 digest. It changes no
    /// label and needs no context.
    pub fn task_bookkeeping(
        &self,
        task_id: &str,
        b: &TaskBookkeeping<'_>,
    ) -> Result<(), WriteRefused> {
        self.db()
            .hm()
            .immediate(|tx| tx.update_task_bookkeeping(task_id, b))
            .map(|_| ())
            .map_err(|_| WriteRefused::Store)
    }

    /// [`DispatchIssuer::share_to_agent`] whose source is the
    /// `strategy_scores` aggregate of `(source_agent, strategy_hash)`,
    /// resolved inside the transaction (`share_learning`, §14 rule 3).
    pub fn share_strategy_to_agent(
        &self,
        target: &DispatchContext,
        source_agent: Uuid,
        strategy_hash: &str,
        memory: &NewMemory<'_>,
    ) -> Outcome {
        let source_id = self
            .db()
            .hm()
            .read(|tx| tx.strategy_id(&source_agent.to_string(), strategy_hash))
            .map_err(|_| WriteRefused::Store)?
            .ok_or(WriteRefused::UnknownRecord)?;
        let source = RecordRef {
            table: RecordTable::StrategyScores,
            record_id: source_id.to_string(),
        };
        self.share_to_agent(target, memory, &source)
    }
}
