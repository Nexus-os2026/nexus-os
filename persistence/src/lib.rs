use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Error Type ──────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

// ── Row Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRow {
    pub id: String,
    pub manifest_json: String,
    pub state: String,
    pub was_running: bool,
    pub autonomy_level: u8,
    pub execution_mode: String,
    pub parent_agent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            manifest_json: row.get(1)?,
            state: row.get(2)?,
            was_running: row.get::<_, i64>(3)? != 0,
            autonomy_level: row.get::<_, u8>(4)?,
            execution_mode: row.get(5)?,
            parent_agent_id: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventRow {
    pub id: i64,
    pub agent_id: String,
    pub event_type: String,
    pub detail_json: String,
    pub previous_hash: String,
    pub current_hash: String,
    pub sequence: i64,
    pub created_at: String,
}

impl AuditEventRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            event_type: row.get(2)?,
            detail_json: row.get(3)?,
            previous_hash: row.get(4)?,
            current_hash: row.get(5)?,
            sequence: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelLedgerRow {
    pub agent_id: String,
    pub budget_total: f64,
    pub budget_consumed: f64,
    pub period_start: String,
    pub period_end: String,
    pub anomaly_count: i64,
    pub ledger_json: String,
    pub updated_at: String,
}

impl FuelLedgerRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            agent_id: row.get(0)?,
            budget_total: row.get(1)?,
            budget_consumed: row.get(2)?,
            period_start: row.get(3)?,
            period_end: row.get(4)?,
            anomaly_count: row.get(5)?,
            ledger_json: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRow {
    pub id: i64,
    pub agent_id: String,
    pub capability: String,
    pub granted: bool,
    pub risk_level: String,
    pub granted_at: String,
    pub revoked_at: Option<String>,
}

impl PermissionRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            capability: row.get(2)?,
            granted: row.get::<_, i32>(3)? != 0,
            risk_level: row.get(4)?,
            granted_at: row.get(5)?,
            revoked_at: row.get(6)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRow {
    pub id: String,
    pub agent_id: String,
    pub operation_type: String,
    pub operation_json: String,
    pub hitl_tier: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
}

impl ConsentRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            operation_type: row.get(2)?,
            operation_json: row.get(3)?,
            hitl_tier: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            resolved_at: row.get(7)?,
            resolved_by: row.get(8)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRow {
    pub id: String,
    pub agent_id: String,
    pub state_json: String,
    pub description: Option<String>,
    pub created_at: String,
}

impl CheckpointRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            state_json: row.get(2)?,
            description: row.get(3)?,
            created_at: row.get(4)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRow {
    pub id: String,
    pub agent_id: String,
    pub content_hash: String,
    pub chunk_text: String,
    pub vector_json: String,
    pub metadata_json: String,
    pub created_at: String,
}

impl EmbeddingRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            content_hash: row.get(2)?,
            chunk_text: row.get(3)?,
            vector_json: row.get(4)?,
            metadata_json: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRow {
    pub id: i64,
    pub agent_id: String,
    pub memory_type: String,
    pub key: String,
    pub value_json: String,
    pub relevance_score: f64,
    pub access_count: i64,
    pub created_at: String,
    pub last_accessed: String,
    pub expires_at: Option<String>,
}

impl MemoryRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            memory_type: row.get(2)?,
            key: row.get(3)?,
            value_json: row.get(4)?,
            relevance_score: row.get(5)?,
            access_count: row.get(6)?,
            created_at: row.get(7)?,
            last_accessed: row.get(8)?,
            expires_at: row.get(9)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L6CooldownTrackerRow {
    pub agent_id: String,
    pub cycle_count: i64,
    pub last_cooldown: Option<String>,
    pub total_cooldowns: i64,
}

impl L6CooldownTrackerRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            agent_id: row.get(0)?,
            cycle_count: row.get(1)?,
            last_cooldown: row.get(2)?,
            total_cooldowns: row.get(3)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmSelectionRow {
    pub id: i64,
    pub agent_id: String,
    pub task_id: String,
    pub algorithm: String,
    pub config_json: String,
    pub outcome_score: Option<f64>,
    pub created_at: String,
}

impl AlgorithmSelectionRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            task_id: row.get(2)?,
            algorithm: row.get(3)?,
            config_json: row.get(4)?,
            outcome_score: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEcosystemRow {
    pub id: String,
    pub creator_agent_id: String,
    pub ecosystem_json: String,
    pub agent_count: i64,
    pub total_fuel_allocated: f64,
    pub status: String,
    pub created_at: String,
}

impl AgentEcosystemRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            creator_agent_id: row.get(1)?,
            ecosystem_json: row.get(2)?,
            agent_count: row.get(3)?,
            total_fuel_allocated: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

// ── Governance Persistence Row Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlDecisionRow {
    pub id: String,
    pub agent_id: String,
    pub action: String,
    pub context_json: Option<String>,
    pub decision: String,
    pub decided_by: Option<String>,
    pub decided_at: String,
    pub response_time_ms: i64,
    pub metadata_json: Option<String>,
}

impl HitlDecisionRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            action: row.get(2)?,
            context_json: row.get(3)?,
            decision: row.get(4)?,
            decided_by: row.get(5)?,
            decided_at: row.get(6)?,
            response_time_ms: row.get(7)?,
            metadata_json: row.get(8)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelTransactionRow {
    pub id: String,
    pub agent_id: String,
    pub operation: String,
    pub amount: i64,
    pub balance_after: i64,
    pub reservation_id: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

impl FuelTransactionRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            operation: row.get(2)?,
            amount: row.get(3)?,
            balance_after: row.get(4)?,
            reservation_id: row.get(5)?,
            metadata_json: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelBalanceRow {
    pub agent_id: String,
    pub balance: i64,
    pub total_allocated: i64,
    pub total_consumed: i64,
    pub last_updated: String,
}

impl FuelBalanceRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            agent_id: row.get(0)?,
            balance: row.get(1)?,
            total_allocated: row.get(2)?,
            total_consumed: row.get(3)?,
            last_updated: row.get(4)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityHistoryRow {
    pub id: String,
    pub agent_id: String,
    pub capability: String,
    pub action: String,
    pub resource: Option<String>,
    pub performed_by: Option<String>,
    pub created_at: String,
    pub metadata_json: Option<String>,
}

impl CapabilityHistoryRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            capability: row.get(2)?,
            action: row.get(3)?,
            resource: row.get(4)?,
            performed_by: row.get(5)?,
            created_at: row.get(6)?,
            metadata_json: row.get(7)?,
        })
    }
}

/// Result of verifying the audit hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerifyResult {
    pub verified: bool,
    pub chain_length: u64,
    pub break_at_sequence: Option<i64>,
    pub verification_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRow {
    pub id: String,
    pub agent_id: String,
    pub goal: String,
    pub status: String,
    pub steps_json: String,
    pub result_json: Option<String>,
    pub fuel_consumed: f64,
    pub fuel_budget: Option<f64>,
    pub estimated_time_secs: Option<f64>,
    pub actual_time_secs: Option<f64>,
    pub quality_score: Option<f64>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub success: bool,
}

impl TaskRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            goal: row.get(2)?,
            status: row.get(3)?,
            steps_json: row.get(4)?,
            result_json: row.get(5)?,
            fuel_consumed: row.get(6)?,
            fuel_budget: row.get(7)?,
            estimated_time_secs: row.get(8)?,
            actual_time_secs: row.get(9)?,
            quality_score: row.get(10)?,
            started_at: row.get(11)?,
            completed_at: row.get(12)?,
            success: row.get::<_, i32>(13)? != 0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HivemindSessionRow {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub sub_tasks_json: String,
    pub assignments_json: String,
    pub results_json: String,
    pub fuel_consumed: f64,
    pub started_at: String,
    pub completed_at: Option<String>,
}

impl HivemindSessionRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            goal: row.get(1)?,
            status: row.get(2)?,
            sub_tasks_json: row.get(3)?,
            assignments_json: row.get(4)?,
            results_json: row.get(5)?,
            fuel_consumed: row.get(6)?,
            started_at: row.get(7)?,
            completed_at: row.get(8)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyScoreRow {
    pub id: i64,
    pub agent_id: String,
    pub strategy_hash: String,
    pub goal_type: String,
    pub uses: i64,
    pub successes: i64,
    pub total_fuel: f64,
    pub total_duration_secs: f64,
    pub composite_score: f64,
    pub updated_at: String,
}

impl StrategyScoreRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            strategy_hash: row.get(2)?,
            goal_type: row.get(3)?,
            uses: row.get(4)?,
            successes: row.get(5)?,
            total_fuel: row.get(6)?,
            total_duration_secs: row.get(7)?,
            composite_score: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionArchiveRow {
    pub id: i64,
    pub engine_id: String,
    pub generation: u32,
    pub variant_json: String,
    pub fitness: f64,
    pub created_at: String,
}

impl EvolutionArchiveRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            engine_id: row.get(1)?,
            generation: row.get(2)?,
            variant_json: row.get(3)?,
            fitness: row.get(4)?,
            created_at: row.get(5)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionHistoryRow {
    pub id: i64,
    pub agent_id: String,
    pub version: i64,
    pub description_before: String,
    pub description_after: String,
    pub trigger: String,
    pub performance_before: f64,
    pub performance_after: f64,
    pub kept: bool,
    pub created_at: String,
}

impl EvolutionHistoryRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            version: row.get(2)?,
            description_before: row.get(3)?,
            description_after: row.get(4)?,
            trigger: row.get(5)?,
            performance_before: row.get(6)?,
            performance_after: row.get(7)?,
            kept: row.get::<_, i32>(8)? != 0,
            created_at: row.get(9)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStateRow {
    pub id: i64,
    pub coordinator_id: String,
    pub iteration: u32,
    pub particles_json: String,
    pub global_best_json: Option<String>,
    pub created_at: String,
}

impl SwarmStateRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            coordinator_id: row.get(1)?,
            iteration: row.get(2)?,
            particles_json: row.get(3)?,
            global_best_json: row.get(4)?,
            created_at: row.get(5)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldModelEntityRow {
    pub id: i64,
    pub agent_id: String,
    pub entity_json: String,
    pub created_at: String,
}

impl WorldModelEntityRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            entity_json: row.get(2)?,
            created_at: row.get(3)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldModelRelationshipRow {
    pub id: i64,
    pub agent_id: String,
    pub relationship_json: String,
    pub created_at: String,
}

impl WorldModelRelationshipRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            relationship_json: row.get(2)?,
            created_at: row.get(3)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdversarialMatchRow {
    pub id: i64,
    pub arena_id: String,
    pub attacker_id: String,
    pub defender_id: String,
    pub succeeded: bool,
    pub severity: Option<String>,
    pub created_at: String,
}

impl AdversarialMatchRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            arena_id: row.get(1)?,
            attacker_id: row.get(2)?,
            defender_id: row.get(3)?,
            succeeded: row.get::<_, i32>(4)? != 0,
            severity: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationWorldRow {
    pub id: String,
    pub name: String,
    pub seed_text: String,
    pub status: String,
    pub tick_count: i64,
    pub persona_count: i64,
    pub config_json: String,
    pub report_json: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl SimulationWorldRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            seed_text: row.get(2)?,
            status: row.get(3)?,
            tick_count: row.get(4)?,
            persona_count: row.get(5)?,
            config_json: row.get(6)?,
            report_json: row.get(7)?,
            created_at: row.get(8)?,
            completed_at: row.get(9)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationPersonaRow {
    pub id: String,
    pub world_id: String,
    pub name: String,
    pub role: String,
    pub personality_json: String,
    pub beliefs_json: String,
    pub memories_json: String,
    pub relationships_json: String,
    pub created_at: String,
}

impl SimulationPersonaRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            world_id: row.get(1)?,
            name: row.get(2)?,
            role: row.get(3)?,
            personality_json: row.get(4)?,
            beliefs_json: row.get(5)?,
            memories_json: row.get(6)?,
            relationships_json: row.get(7)?,
            created_at: row.get(8)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationEventRow {
    pub id: i64,
    pub world_id: String,
    pub tick: i64,
    pub actor_id: String,
    pub action_type: String,
    pub content: Option<String>,
    pub target_id: Option<String>,
    pub impact: f64,
    pub created_at: String,
}

impl SimulationEventRow {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            world_id: row.get(1)?,
            tick: row.get(2)?,
            actor_id: row.get(3)?,
            action_type: row.get(4)?,
            content: row.get(5)?,
            target_id: row.get(6)?,
            impact: row.get(7)?,
            created_at: row.get(8)?,
        })
    }
}

// ── StateStore Trait ────────────────────────────────────────────────────────

pub trait StateStore {
    // Agent methods
    fn save_agent(
        &self,
        id: &str,
        manifest_json: &str,
        state: &str,
        autonomy_level: u8,
        execution_mode: &str,
    ) -> Result<()>;
    fn load_agent(&self, id: &str) -> Result<Option<AgentRow>>;
    fn list_agents(&self) -> Result<Vec<AgentRow>>;
    fn update_agent_state(&self, id: &str, state: &str) -> Result<()>;
    fn delete_agent(&self, id: &str) -> Result<()>;
    fn clear_all_agents(&self) -> Result<usize>;

    // Audit methods
    fn append_audit_event(
        &self,
        agent_id: &str,
        event_type: &str,
        detail_json: &str,
        previous_hash: &str,
        current_hash: &str,
        sequence: i64,
    ) -> Result<i64>;
    fn load_audit_events(
        &self,
        agent_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditEventRow>>;
    fn get_latest_audit_hash(&self) -> Result<Option<String>>;
    fn get_audit_count(&self) -> Result<i64>;

    // Fuel methods
    fn save_fuel_ledger(&self, agent_id: &str, ledger: &FuelLedgerRow) -> Result<()>;
    fn load_fuel_ledger(&self, agent_id: &str) -> Result<Option<FuelLedgerRow>>;

    // Permission methods
    fn grant_permission(&self, agent_id: &str, capability: &str, risk_level: &str) -> Result<()>;
    fn revoke_permission(&self, agent_id: &str, capability: &str) -> Result<()>;
    fn load_permissions(&self, agent_id: &str) -> Result<Vec<PermissionRow>>;

    // Consent methods
    fn enqueue_consent(&self, request: &ConsentRow) -> Result<()>;
    fn resolve_consent(&self, id: &str, status: &str, resolved_by: &str) -> Result<()>;
    fn load_pending_consent(&self) -> Result<Vec<ConsentRow>>;
    fn load_consent_by_agent(&self, agent_id: &str) -> Result<Vec<ConsentRow>>;
    fn load_all_consents(&self, limit: u32) -> Result<Vec<ConsentRow>>;

    // Time machine checkpoint methods
    fn save_checkpoint(&self, checkpoint: &CheckpointRow) -> Result<()>;
    fn load_checkpoint(&self, id: &str) -> Result<Option<CheckpointRow>>;
    fn list_checkpoints(&self, limit: usize) -> Result<Vec<CheckpointRow>>;

    // Embedding methods
    fn save_embedding(&self, embedding: &EmbeddingRow) -> Result<()>;
    fn load_embeddings_by_agent(&self, agent_id: &str) -> Result<Vec<EmbeddingRow>>;
    fn delete_embeddings_by_hash(&self, content_hash: &str) -> Result<()>;

    // Memory methods
    fn save_memory(
        &self,
        agent_id: &str,
        memory_type: &str,
        key: &str,
        value_json: &str,
    ) -> Result<()>;
    fn load_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryRow>>;
    fn delete_memories_by_agent(&self, agent_id: &str) -> Result<()>;
    fn touch_memory(&self, id: i64) -> Result<()>;
    fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<()>;

    // L6 coordination methods
    fn upsert_l6_cooldown(
        &self,
        agent_id: &str,
        cycle_count: i64,
        last_cooldown: Option<&str>,
        total_cooldowns: i64,
    ) -> Result<()>;
    fn load_l6_cooldown(&self, agent_id: &str) -> Result<Option<L6CooldownTrackerRow>>;
    fn save_algorithm_selection(
        &self,
        agent_id: &str,
        task_id: &str,
        algorithm: &str,
        config_json: &str,
        outcome_score: Option<f64>,
    ) -> Result<i64>;
    fn load_algorithm_selections(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AlgorithmSelectionRow>>;
    fn save_agent_ecosystem(
        &self,
        id: &str,
        creator_agent_id: &str,
        ecosystem_json: &str,
        agent_count: i64,
        total_fuel_allocated: f64,
        status: &str,
    ) -> Result<()>;
    fn load_agent_ecosystem(&self, id: &str) -> Result<Option<AgentEcosystemRow>>;
    fn list_agent_ecosystems(&self, creator_agent_id: &str) -> Result<Vec<AgentEcosystemRow>>;

    // Task history methods
    fn save_task(&self, task: &TaskRow) -> Result<()>;
    fn load_tasks_by_agent(&self, agent_id: &str, limit: usize) -> Result<Vec<TaskRow>>;
    fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_json: Option<&str>,
        fuel: f64,
        success: bool,
    ) -> Result<()>;

    // Hivemind session methods
    fn save_hivemind_session(&self, session: &HivemindSessionRow) -> Result<()>;
    fn load_hivemind_session(&self, id: &str) -> Result<Option<HivemindSessionRow>>;
    fn list_hivemind_sessions(&self) -> Result<Vec<HivemindSessionRow>>;
    fn update_hivemind_session_status(&self, session: &HivemindSessionRow) -> Result<()>;

    // Strategy score methods (evolution engine)
    fn upsert_strategy_score(
        &self,
        agent_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel: f64,
        duration: f64,
    ) -> Result<()>;
    fn load_top_strategies(
        &self,
        agent_id: &str,
        goal_type: &str,
        limit: usize,
    ) -> Result<Vec<StrategyScoreRow>>;
    fn load_strategy_history(&self, agent_id: &str, limit: usize) -> Result<Vec<StrategyScoreRow>>;

    // ── HITL Decision Methods ──────────────────────────────────────────
    fn record_hitl_decision(&self, decision: &HitlDecisionRow) -> Result<()>;
    fn load_hitl_decisions(
        &self,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HitlDecisionRow>>;
    fn hitl_approval_rate(&self, agent_id: Option<&str>) -> Result<f64>;

    // ── Fuel Transaction Methods ───────────────────────────────────────
    fn append_fuel_transaction(&self, tx: &FuelTransactionRow) -> Result<()>;
    fn load_fuel_transactions(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<FuelTransactionRow>>;
    fn upsert_fuel_balance(
        &self,
        agent_id: &str,
        balance: i64,
        total_allocated: i64,
        total_consumed: i64,
    ) -> Result<()>;
    fn load_fuel_balance(&self, agent_id: &str) -> Result<Option<FuelBalanceRow>>;

    // ── Capability History Methods ─────────────────────────────────────
    fn append_capability_history(&self, entry: &CapabilityHistoryRow) -> Result<()>;
    fn load_capability_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<CapabilityHistoryRow>>;

    // ── Audit Chain Verification ───────────────────────────────────────
    fn verify_audit_chain(&self) -> Result<ChainVerifyResult>;
}

// ── NexusDatabase ───────────────────────────────────────────────────────────

pub struct NexusDatabase {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for NexusDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusDatabase").finish()
    }
}

impl NexusDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        // WAL mode: concurrent reads + crash-safe writes
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn default_db_path() -> std::path::PathBuf {
        if let Ok(path) = std::env::var("NEXUS_DB_PATH") {
            return std::path::PathBuf::from(path);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home)
            .join(".nexus")
            .join("nexus.db")
    }

    fn migrate(&self) -> Result<()> {
        {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            conn.execute_batch(
                "BEGIN;

            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                manifest_json TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'created',
                was_running INTEGER NOT NULL DEFAULT 0,
                autonomy_level INTEGER NOT NULL DEFAULT 0,
                execution_mode TEXT NOT NULL DEFAULT 'native',
                parent_agent_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                detail_json TEXT NOT NULL,
                previous_hash TEXT NOT NULL,
                current_hash TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_agent ON audit_events(agent_id);
            CREATE INDEX IF NOT EXISTS idx_audit_sequence ON audit_events(sequence);

            CREATE TABLE IF NOT EXISTS fuel_ledgers (
                agent_id TEXT PRIMARY KEY,
                budget_total REAL NOT NULL,
                budget_consumed REAL NOT NULL,
                period_start TEXT NOT NULL,
                period_end TEXT NOT NULL,
                anomaly_count INTEGER NOT NULL DEFAULT 0,
                ledger_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                granted INTEGER NOT NULL DEFAULT 1,
                risk_level TEXT NOT NULL DEFAULT 'low',
                granted_at TEXT NOT NULL,
                revoked_at TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_perm_agent_cap
                ON permissions(agent_id, capability);

            CREATE TABLE IF NOT EXISTS consent_queue (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                operation_type TEXT NOT NULL,
                operation_json TEXT NOT NULL,
                hitl_tier TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by TEXT
            );

            CREATE TABLE IF NOT EXISTS embeddings (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                chunk_text TEXT NOT NULL,
                vector_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_embed_agent ON embeddings(agent_id);
            CREATE INDEX IF NOT EXISTS idx_embed_hash ON embeddings(content_hash);

            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                state_json TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_checkpoint_agent ON checkpoints(agent_id);

            CREATE TABLE IF NOT EXISTS agent_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                relevance_score REAL NOT NULL DEFAULT 1.0,
                access_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                expires_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_memory_agent_type
                ON agent_memory(agent_id, memory_type);
            CREATE INDEX IF NOT EXISTS idx_memory_relevance
                ON agent_memory(relevance_score DESC);

            CREATE TABLE IF NOT EXISTS task_history (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                steps_json TEXT NOT NULL,
                result_json TEXT,
                fuel_consumed REAL NOT NULL DEFAULT 0.0,
                fuel_budget REAL,
                estimated_time_secs REAL,
                actual_time_secs REAL,
                quality_score REAL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                success INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_task_agent ON task_history(agent_id);

            CREATE TABLE IF NOT EXISTS hivemind_sessions (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                sub_tasks_json TEXT NOT NULL,
                assignments_json TEXT NOT NULL,
                results_json TEXT NOT NULL,
                fuel_consumed REAL NOT NULL DEFAULT 0.0,
                started_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS strategy_scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                strategy_hash TEXT NOT NULL,
                goal_type TEXT NOT NULL,
                uses INTEGER NOT NULL DEFAULT 0,
                successes INTEGER NOT NULL DEFAULT 0,
                total_fuel REAL NOT NULL DEFAULT 0.0,
                total_duration_secs REAL NOT NULL DEFAULT 0.0,
                composite_score REAL NOT NULL DEFAULT 0.0,
                updated_at TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_strategy_agent_hash
                ON strategy_scores(agent_id, strategy_hash);

            CREATE TABLE IF NOT EXISTS evolution_archive (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                engine_id TEXT NOT NULL,
                generation INTEGER NOT NULL,
                variant_json TEXT NOT NULL,
                fitness REAL NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_evolution_engine_generation
                ON evolution_archive(engine_id, generation DESC);

            CREATE TABLE IF NOT EXISTS evolution_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                description_before TEXT NOT NULL,
                description_after TEXT NOT NULL,
                trigger TEXT NOT NULL,
                performance_before REAL NOT NULL,
                performance_after REAL NOT NULL,
                kept INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_evolution_history_agent_version
                ON evolution_history(agent_id, version DESC);

            CREATE TABLE IF NOT EXISTS swarm_state (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                coordinator_id TEXT NOT NULL,
                iteration INTEGER NOT NULL,
                particles_json TEXT NOT NULL,
                global_best_json TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_swarm_coordinator_iteration
                ON swarm_state(coordinator_id, iteration DESC);

            CREATE TABLE IF NOT EXISTS world_model_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                entity_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_world_entities_agent
                ON world_model_entities(agent_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS world_model_relationships (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                relationship_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_world_relationships_agent
                ON world_model_relationships(agent_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS adversarial_matches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                arena_id TEXT NOT NULL,
                attacker_id TEXT NOT NULL,
                defender_id TEXT NOT NULL,
                succeeded INTEGER NOT NULL,
                severity TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_adversarial_arena
                ON adversarial_matches(arena_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS l6_cooldown_tracker (
                agent_id TEXT PRIMARY KEY,
                cycle_count INTEGER NOT NULL DEFAULT 0,
                last_cooldown TEXT,
                total_cooldowns INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS algorithm_selections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                algorithm TEXT NOT NULL,
                config_json TEXT NOT NULL,
                outcome_score REAL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_algo_agent
                ON algorithm_selections(agent_id);

            CREATE TABLE IF NOT EXISTS agent_ecosystems (
                id TEXT PRIMARY KEY,
                creator_agent_id TEXT NOT NULL,
                ecosystem_json TEXT NOT NULL,
                agent_count INTEGER NOT NULL,
                total_fuel_allocated REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS simulation_worlds (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                seed_text TEXT NOT NULL,
                status TEXT NOT NULL,
                tick_count INTEGER NOT NULL DEFAULT 0,
                persona_count INTEGER NOT NULL,
                config_json TEXT NOT NULL,
                report_json TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS simulation_personas (
                id TEXT PRIMARY KEY,
                world_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                personality_json TEXT NOT NULL,
                beliefs_json TEXT NOT NULL,
                memories_json TEXT NOT NULL,
                relationships_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_persona_world ON simulation_personas(world_id);

            CREATE TABLE IF NOT EXISTS simulation_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                world_id TEXT NOT NULL,
                tick INTEGER NOT NULL,
                actor_id TEXT NOT NULL,
                action_type TEXT NOT NULL,
                content TEXT,
                target_id TEXT,
                impact REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_event_world_tick ON simulation_events(world_id, tick);

            -- Governance persistence tables (GAP 1 fix)

            CREATE TABLE IF NOT EXISTS hitl_decisions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                action TEXT NOT NULL,
                context_json TEXT,
                decision TEXT NOT NULL,
                decided_by TEXT,
                decided_at TEXT NOT NULL,
                response_time_ms INTEGER NOT NULL DEFAULT 0,
                metadata_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_hitl_agent ON hitl_decisions(agent_id);
            CREATE INDEX IF NOT EXISTS idx_hitl_time ON hitl_decisions(decided_at);

            CREATE TABLE IF NOT EXISTS fuel_transactions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                operation TEXT NOT NULL,
                amount INTEGER NOT NULL,
                balance_after INTEGER NOT NULL,
                reservation_id TEXT,
                metadata_json TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_fuel_tx_agent ON fuel_transactions(agent_id);
            CREATE INDEX IF NOT EXISTS idx_fuel_tx_time ON fuel_transactions(created_at);

            CREATE TABLE IF NOT EXISTS fuel_balances (
                agent_id TEXT PRIMARY KEY,
                balance INTEGER NOT NULL DEFAULT 0,
                total_allocated INTEGER NOT NULL DEFAULT 0,
                total_consumed INTEGER NOT NULL DEFAULT 0,
                last_updated TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS capability_history (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT,
                performed_by TEXT,
                created_at TEXT NOT NULL,
                metadata_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_cap_hist_agent ON capability_history(agent_id);
            CREATE INDEX IF NOT EXISTS idx_cap_hist_time ON capability_history(created_at);

            COMMIT;",
            )?;
        }
        self.add_column_if_missing("task_history", "fuel_budget", "REAL")?;
        self.add_column_if_missing("task_history", "estimated_time_secs", "REAL")?;
        self.add_column_if_missing("task_history", "actual_time_secs", "REAL")?;
        self.add_column_if_missing("task_history", "quality_score", "REAL")?;
        self.add_column_if_missing("agents", "parent_agent_id", "TEXT")?;
        self.add_column_if_missing("agents", "was_running", "INTEGER NOT NULL DEFAULT 0")?;
        Ok(())
    }

    fn add_column_if_missing(&self, table: &str, column: &str, definition: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
        match conn.execute(&sql, []) {
            Ok(_) => Ok(()),
            Err(err) if err.to_string().contains("duplicate column name") => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub fn save_evolution_variant(
        &self,
        engine_id: &str,
        generation: u32,
        variant_json: &str,
        fitness: f64,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO evolution_archive (engine_id, generation, variant_json, fitness, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![engine_id, generation, variant_json, fitness, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_evolution_archive(
        &self,
        engine_id: &str,
        limit: usize,
    ) -> Result<Vec<EvolutionArchiveRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, engine_id, generation, variant_json, fitness, created_at
             FROM evolution_archive
             WHERE engine_id = ?1
             ORDER BY generation DESC, fitness DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![engine_id, limit as i64],
            EvolutionArchiveRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_evolution_history(
        &self,
        agent_id: &str,
        version: i64,
        description_before: &str,
        description_after: &str,
        trigger: &str,
        performance_before: f64,
        performance_after: f64,
        kept: bool,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO evolution_history (
                agent_id, version, description_before, description_after, trigger,
                performance_before, performance_after, kept, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                agent_id,
                version,
                description_before,
                description_after,
                trigger,
                performance_before,
                performance_after,
                kept as i32,
                &now
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_evolution_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<EvolutionHistoryRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, version, description_before, description_after, trigger,
                    performance_before, performance_after, kept, created_at
             FROM evolution_history
             WHERE agent_id = ?1
             ORDER BY version DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![agent_id, limit as i64],
            EvolutionHistoryRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn save_agent_with_parent(
        &self,
        id: &str,
        manifest_json: &str,
        state: &str,
        autonomy_level: u8,
        execution_mode: &str,
        parent_agent_id: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let was_running = agent_state_was_running(state);
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO agents (
                id, manifest_json, state, was_running, autonomy_level, execution_mode,
                parent_agent_id, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                manifest_json = excluded.manifest_json,
                state = excluded.state,
                was_running = excluded.was_running,
                autonomy_level = excluded.autonomy_level,
                execution_mode = excluded.execution_mode,
                parent_agent_id = excluded.parent_agent_id,
                updated_at = excluded.updated_at",
            params![
                id,
                manifest_json,
                state,
                was_running as i64,
                autonomy_level,
                execution_mode,
                parent_agent_id,
                &now,
                &now
            ],
        )?;
        Ok(())
    }

    pub fn save_swarm_state(
        &self,
        coordinator_id: &str,
        iteration: u32,
        particles_json: &str,
        global_best_json: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO swarm_state (coordinator_id, iteration, particles_json, global_best_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![coordinator_id, iteration, particles_json, global_best_json, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_latest_swarm_state(&self, coordinator_id: &str) -> Result<Option<SwarmStateRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, coordinator_id, iteration, particles_json, global_best_json, created_at
             FROM swarm_state
             WHERE coordinator_id = ?1
             ORDER BY iteration DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![coordinator_id], SwarmStateRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn replace_world_model_state(
        &self,
        agent_id: &str,
        entities_json: &[String],
        relationships_json: &[String],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM world_model_entities WHERE agent_id = ?1",
            params![agent_id],
        )?;
        tx.execute(
            "DELETE FROM world_model_relationships WHERE agent_id = ?1",
            params![agent_id],
        )?;
        for entity_json in entities_json {
            tx.execute(
                "INSERT INTO world_model_entities (agent_id, entity_json, created_at)
                 VALUES (?1, ?2, ?3)",
                params![agent_id, entity_json, &now],
            )?;
        }
        for relationship_json in relationships_json {
            tx.execute(
                "INSERT INTO world_model_relationships (agent_id, relationship_json, created_at)
                 VALUES (?1, ?2, ?3)",
                params![agent_id, relationship_json, &now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_world_model_entities(&self, agent_id: &str) -> Result<Vec<WorldModelEntityRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_json, created_at
             FROM world_model_entities
             WHERE agent_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![agent_id], WorldModelEntityRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn load_world_model_relationships(
        &self,
        agent_id: &str,
    ) -> Result<Vec<WorldModelRelationshipRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, relationship_json, created_at
             FROM world_model_relationships
             WHERE agent_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![agent_id], WorldModelRelationshipRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn save_adversarial_match(
        &self,
        arena_id: &str,
        attacker_id: &str,
        defender_id: &str,
        succeeded: bool,
        severity: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO adversarial_matches (arena_id, attacker_id, defender_id, succeeded, severity, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                arena_id,
                attacker_id,
                defender_id,
                succeeded as i32,
                severity,
                &now
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_adversarial_matches(
        &self,
        arena_id: &str,
        limit: usize,
    ) -> Result<Vec<AdversarialMatchRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, arena_id, attacker_id, defender_id, succeeded, severity, created_at
             FROM adversarial_matches
             WHERE arena_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![arena_id, limit as i64],
            AdversarialMatchRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_simulation_world(
        &self,
        id: &str,
        name: &str,
        seed_text: &str,
        status: &str,
        tick_count: i64,
        persona_count: i64,
        config_json: &str,
        report_json: Option<&str>,
        completed_at: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO simulation_worlds (id, name, seed_text, status, tick_count, persona_count, config_json, report_json, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                seed_text = excluded.seed_text,
                status = excluded.status,
                tick_count = excluded.tick_count,
                persona_count = excluded.persona_count,
                config_json = excluded.config_json,
                report_json = excluded.report_json,
                completed_at = excluded.completed_at",
            params![
                id,
                name,
                seed_text,
                status,
                tick_count,
                persona_count,
                config_json,
                report_json,
                &now,
                completed_at
            ],
        )?;
        Ok(())
    }

    pub fn load_simulation_world(&self, id: &str) -> Result<Option<SimulationWorldRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, name, seed_text, status, tick_count, persona_count, config_json, report_json, created_at, completed_at
             FROM simulation_worlds
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], SimulationWorldRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_simulation_worlds(&self) -> Result<Vec<SimulationWorldRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, name, seed_text, status, tick_count, persona_count, config_json, report_json, created_at, completed_at
             FROM simulation_worlds
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], SimulationWorldRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn replace_simulation_personas(
        &self,
        world_id: &str,
        personas: &[(String, String, String, String, String, String, String)],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM simulation_personas WHERE world_id = ?1",
            params![world_id],
        )?;
        for (id, name, role, personality_json, beliefs_json, memories_json, relationships_json) in
            personas
        {
            tx.execute(
                "INSERT INTO simulation_personas (id, world_id, name, role, personality_json, beliefs_json, memories_json, relationships_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id,
                    world_id,
                    name,
                    role,
                    personality_json,
                    beliefs_json,
                    memories_json,
                    relationships_json,
                    &now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_simulation_personas(&self, world_id: &str) -> Result<Vec<SimulationPersonaRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, world_id, name, role, personality_json, beliefs_json, memories_json, relationships_json, created_at
             FROM simulation_personas
             WHERE world_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![world_id], SimulationPersonaRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_simulation_event(
        &self,
        world_id: &str,
        tick: i64,
        actor_id: &str,
        action_type: &str,
        content: Option<&str>,
        target_id: Option<&str>,
        impact: f64,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO simulation_events (world_id, tick, actor_id, action_type, content, target_id, impact, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![world_id, tick, actor_id, action_type, content, target_id, impact, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn load_simulation_events(
        &self,
        world_id: &str,
        tick: Option<i64>,
    ) -> Result<Vec<SimulationEventRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut result = Vec::new();
        match tick {
            Some(tick) => {
                let mut stmt = conn.prepare(
                    "SELECT id, world_id, tick, actor_id, action_type, content, target_id, impact, created_at
                     FROM simulation_events
                     WHERE world_id = ?1 AND tick = ?2
                     ORDER BY id ASC",
                )?;
                let rows = stmt.query_map(params![world_id, tick], SimulationEventRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, world_id, tick, actor_id, action_type, content, target_id, impact, created_at
                     FROM simulation_events
                     WHERE world_id = ?1
                     ORDER BY tick ASC, id ASC",
                )?;
                let rows = stmt.query_map(params![world_id], SimulationEventRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
        }
        Ok(result)
    }
}

// ── StateStore Implementation ───────────────────────────────────────────────

impl StateStore for NexusDatabase {
    // ── Agent Methods ───────────────────────────────────────────────────

    fn save_agent(
        &self,
        id: &str,
        manifest_json: &str,
        state: &str,
        autonomy_level: u8,
        execution_mode: &str,
    ) -> Result<()> {
        self.save_agent_with_parent(
            id,
            manifest_json,
            state,
            autonomy_level,
            execution_mode,
            None,
        )
    }

    fn load_agent(&self, id: &str) -> Result<Option<AgentRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, manifest_json, state, was_running, autonomy_level, execution_mode,
                    parent_agent_id, created_at, updated_at
             FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], AgentRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list_agents(&self) -> Result<Vec<AgentRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, manifest_json, state, was_running, autonomy_level, execution_mode,
                    parent_agent_id, created_at, updated_at
             FROM agents ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], AgentRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn update_agent_state(&self, id: &str, state: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let was_running = agent_state_was_running(state);
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let changed = conn.execute(
            "UPDATE agents SET state = ?1, was_running = ?2, updated_at = ?3 WHERE id = ?4",
            params![state, was_running as i64, &now, id],
        )?;
        if changed == 0 {
            return Err(PersistenceError::NotFound(format!("agent {id}")));
        }
        Ok(())
    }

    fn delete_agent(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute("DELETE FROM agents WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn clear_all_agents(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: usize = conn.query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))?;
        conn.execute_batch(
            "DELETE FROM agents;
             DELETE FROM fuel_ledgers;
             DELETE FROM permissions;
             DELETE FROM consent_queue;
             DELETE FROM task_history;
             DELETE FROM agent_memory;",
        )?;
        Ok(count)
    }

    // ── Audit Methods ───────────────────────────────────────────────────

    fn append_audit_event(
        &self,
        agent_id: &str,
        event_type: &str,
        detail_json: &str,
        previous_hash: &str,
        current_hash: &str,
        sequence: i64,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO audit_events (agent_id, event_type, detail_json, previous_hash, current_hash, sequence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![agent_id, event_type, detail_json, previous_hash, current_hash, sequence, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn load_audit_events(
        &self,
        agent_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditEventRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut result = Vec::new();
        match agent_id {
            Some(aid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, event_type, detail_json, previous_hash, current_hash, sequence, created_at
                     FROM audit_events WHERE agent_id = ?1
                     ORDER BY sequence ASC LIMIT ?2 OFFSET ?3",
                )?;
                let rows = stmt.query_map(
                    params![aid, limit as i64, offset as i64],
                    AuditEventRow::from_row,
                )?;
                for row in rows {
                    result.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, event_type, detail_json, previous_hash, current_hash, sequence, created_at
                     FROM audit_events ORDER BY sequence ASC LIMIT ?1 OFFSET ?2",
                )?;
                let rows = stmt.query_map(
                    params![limit as i64, offset as i64],
                    AuditEventRow::from_row,
                )?;
                for row in rows {
                    result.push(row?);
                }
            }
        }
        Ok(result)
    }

    fn get_latest_audit_hash(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt =
            conn.prepare("SELECT current_hash FROM audit_events ORDER BY sequence DESC LIMIT 1")?;
        let mut rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(hash) => Ok(Some(hash?)),
            None => Ok(None),
        }
    }

    fn get_audit_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))?;
        Ok(count)
    }

    // ── Fuel Methods ────────────────────────────────────────────────────

    fn save_fuel_ledger(&self, agent_id: &str, ledger: &FuelLedgerRow) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO fuel_ledgers (agent_id, budget_total, budget_consumed, period_start, period_end, anomaly_count, ledger_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(agent_id) DO UPDATE SET
                budget_total = excluded.budget_total,
                budget_consumed = excluded.budget_consumed,
                period_start = excluded.period_start,
                period_end = excluded.period_end,
                anomaly_count = excluded.anomaly_count,
                ledger_json = excluded.ledger_json,
                updated_at = excluded.updated_at",
            params![
                agent_id,
                ledger.budget_total,
                ledger.budget_consumed,
                &ledger.period_start,
                &ledger.period_end,
                ledger.anomaly_count,
                &ledger.ledger_json,
                &now,
            ],
        )?;
        Ok(())
    }

    fn load_fuel_ledger(&self, agent_id: &str) -> Result<Option<FuelLedgerRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT agent_id, budget_total, budget_consumed, period_start, period_end, anomaly_count, ledger_json, updated_at
             FROM fuel_ledgers WHERE agent_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![agent_id], FuelLedgerRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Permission Methods ──────────────────────────────────────────────

    fn grant_permission(&self, agent_id: &str, capability: &str, risk_level: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO permissions (agent_id, capability, granted, risk_level, granted_at)
             VALUES (?1, ?2, 1, ?3, ?4)
             ON CONFLICT(agent_id, capability) DO UPDATE SET
                granted = 1,
                risk_level = excluded.risk_level,
                granted_at = excluded.granted_at,
                revoked_at = NULL",
            params![agent_id, capability, risk_level, &now],
        )?;
        Ok(())
    }

    fn revoke_permission(&self, agent_id: &str, capability: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE permissions SET granted = 0, revoked_at = ?1
             WHERE agent_id = ?2 AND capability = ?3",
            params![&now, agent_id, capability],
        )?;
        Ok(())
    }

    fn load_permissions(&self, agent_id: &str) -> Result<Vec<PermissionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, capability, granted, risk_level, granted_at, revoked_at
             FROM permissions WHERE agent_id = ?1",
        )?;
        let rows = stmt.query_map(params![agent_id], PermissionRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Consent Methods ─────────────────────────────────────────────────

    fn enqueue_consent(&self, request: &ConsentRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO consent_queue (id, agent_id, operation_type, operation_json, hitl_tier, status, created_at, resolved_at, resolved_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &request.id,
                &request.agent_id,
                &request.operation_type,
                &request.operation_json,
                &request.hitl_tier,
                &request.status,
                &request.created_at,
                &request.resolved_at,
                &request.resolved_by,
            ],
        )?;
        Ok(())
    }

    fn resolve_consent(&self, id: &str, status: &str, resolved_by: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE consent_queue SET status = ?1, resolved_at = ?2, resolved_by = ?3 WHERE id = ?4",
            params![status, &now, resolved_by, id],
        )?;
        Ok(())
    }

    fn load_pending_consent(&self) -> Result<Vec<ConsentRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, operation_type, operation_json, hitl_tier, status, created_at, resolved_at, resolved_by
             FROM consent_queue WHERE status = 'pending' ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], ConsentRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn load_consent_by_agent(&self, agent_id: &str) -> Result<Vec<ConsentRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, operation_type, operation_json, hitl_tier, status, created_at, resolved_at, resolved_by
             FROM consent_queue WHERE agent_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![agent_id], ConsentRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn load_all_consents(&self, limit: u32) -> Result<Vec<ConsentRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, operation_type, operation_json, hitl_tier, status, created_at, resolved_at, resolved_by
             FROM consent_queue ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], ConsentRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn save_checkpoint(&self, checkpoint: &CheckpointRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO checkpoints (id, agent_id, state_json, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                agent_id = excluded.agent_id,
                state_json = excluded.state_json,
                description = excluded.description,
                created_at = excluded.created_at",
            params![
                &checkpoint.id,
                &checkpoint.agent_id,
                &checkpoint.state_json,
                &checkpoint.description,
                &checkpoint.created_at,
            ],
        )?;
        Ok(())
    }

    fn load_checkpoint(&self, id: &str) -> Result<Option<CheckpointRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, state_json, description, created_at
             FROM checkpoints WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], CheckpointRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list_checkpoints(&self, limit: usize) -> Result<Vec<CheckpointRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, state_json, description, created_at
             FROM checkpoints ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], CheckpointRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Embedding Methods ───────────────────────────────────────────────

    fn save_embedding(&self, embedding: &EmbeddingRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO embeddings (id, agent_id, content_hash, chunk_text, vector_json, metadata_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                vector_json = excluded.vector_json,
                metadata_json = excluded.metadata_json",
            params![
                &embedding.id,
                &embedding.agent_id,
                &embedding.content_hash,
                &embedding.chunk_text,
                &embedding.vector_json,
                &embedding.metadata_json,
                &embedding.created_at,
            ],
        )?;
        Ok(())
    }

    fn load_embeddings_by_agent(&self, agent_id: &str) -> Result<Vec<EmbeddingRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, content_hash, chunk_text, vector_json, metadata_json, created_at
             FROM embeddings WHERE agent_id = ?1",
        )?;
        let rows = stmt.query_map(params![agent_id], EmbeddingRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn delete_embeddings_by_hash(&self, content_hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "DELETE FROM embeddings WHERE content_hash = ?1",
            params![content_hash],
        )?;
        Ok(())
    }

    // ── Memory Methods ──────────────────────────────────────────────────

    fn save_memory(
        &self,
        agent_id: &str,
        memory_type: &str,
        key: &str,
        value_json: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO agent_memory (agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed)
             VALUES (?1, ?2, ?3, ?4, 1.0, 0, ?5, ?5)",
            params![agent_id, memory_type, key, value_json, &now],
        )?;
        Ok(())
    }

    fn load_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut result = Vec::new();
        match memory_type {
            Some(mt) => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at
                     FROM agent_memory WHERE agent_id = ?1 AND memory_type = ?2
                     ORDER BY relevance_score DESC LIMIT ?3",
                )?;
                let rows =
                    stmt.query_map(params![agent_id, mt, limit as i64], MemoryRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, memory_type, key, value_json, relevance_score, access_count, created_at, last_accessed, expires_at
                     FROM agent_memory WHERE agent_id = ?1
                     ORDER BY relevance_score DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![agent_id, limit as i64], MemoryRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
        }
        Ok(result)
    }

    fn delete_memories_by_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "DELETE FROM agent_memory WHERE agent_id = ?1",
            params![agent_id],
        )?;
        Ok(())
    }

    fn touch_memory(&self, id: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE agent_memory SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
            params![&now, id],
        )?;
        Ok(())
    }

    fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE agent_memory SET relevance_score = relevance_score * ?1
             WHERE agent_id = ?2 AND last_accessed < datetime('now', '-24 hours')",
            params![decay_factor, agent_id],
        )?;
        Ok(())
    }

    fn upsert_l6_cooldown(
        &self,
        agent_id: &str,
        cycle_count: i64,
        last_cooldown: Option<&str>,
        total_cooldowns: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO l6_cooldown_tracker (agent_id, cycle_count, last_cooldown, total_cooldowns)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(agent_id) DO UPDATE SET
                cycle_count = excluded.cycle_count,
                last_cooldown = excluded.last_cooldown,
                total_cooldowns = excluded.total_cooldowns",
            params![agent_id, cycle_count, last_cooldown, total_cooldowns],
        )?;
        Ok(())
    }

    fn load_l6_cooldown(&self, agent_id: &str) -> Result<Option<L6CooldownTrackerRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT agent_id, cycle_count, last_cooldown, total_cooldowns
             FROM l6_cooldown_tracker WHERE agent_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![agent_id], L6CooldownTrackerRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn save_algorithm_selection(
        &self,
        agent_id: &str,
        task_id: &str,
        algorithm: &str,
        config_json: &str,
        outcome_score: Option<f64>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO algorithm_selections (agent_id, task_id, algorithm, config_json, outcome_score, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![agent_id, task_id, algorithm, config_json, outcome_score, &now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn load_algorithm_selections(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AlgorithmSelectionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, task_id, algorithm, config_json, outcome_score, created_at
             FROM algorithm_selections
             WHERE agent_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![agent_id, limit as i64],
            AlgorithmSelectionRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn save_agent_ecosystem(
        &self,
        id: &str,
        creator_agent_id: &str,
        ecosystem_json: &str,
        agent_count: i64,
        total_fuel_allocated: f64,
        status: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO agent_ecosystems (id, creator_agent_id, ecosystem_json, agent_count, total_fuel_allocated, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                creator_agent_id = excluded.creator_agent_id,
                ecosystem_json = excluded.ecosystem_json,
                agent_count = excluded.agent_count,
                total_fuel_allocated = excluded.total_fuel_allocated,
                status = excluded.status",
            params![
                id,
                creator_agent_id,
                ecosystem_json,
                agent_count,
                total_fuel_allocated,
                status,
                &now
            ],
        )?;
        Ok(())
    }

    fn load_agent_ecosystem(&self, id: &str) -> Result<Option<AgentEcosystemRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, creator_agent_id, ecosystem_json, agent_count, total_fuel_allocated, status, created_at
             FROM agent_ecosystems WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], AgentEcosystemRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list_agent_ecosystems(&self, creator_agent_id: &str) -> Result<Vec<AgentEcosystemRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, creator_agent_id, ecosystem_json, agent_count, total_fuel_allocated, status, created_at
             FROM agent_ecosystems
             WHERE creator_agent_id = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![creator_agent_id], AgentEcosystemRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Task History Methods ────────────────────────────────────────────

    fn save_task(&self, task: &TaskRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, fuel_budget, estimated_time_secs, actual_time_secs, quality_score, started_at, completed_at, success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                steps_json = excluded.steps_json,
                result_json = excluded.result_json,
                fuel_consumed = excluded.fuel_consumed,
                fuel_budget = excluded.fuel_budget,
                estimated_time_secs = excluded.estimated_time_secs,
                actual_time_secs = excluded.actual_time_secs,
                quality_score = excluded.quality_score,
                completed_at = excluded.completed_at,
                success = excluded.success",
            params![
                &task.id,
                &task.agent_id,
                &task.goal,
                &task.status,
                &task.steps_json,
                &task.result_json,
                task.fuel_consumed,
                task.fuel_budget,
                task.estimated_time_secs,
                task.actual_time_secs,
                task.quality_score,
                &task.started_at,
                &task.completed_at,
                task.success as i32,
            ],
        )?;
        Ok(())
    }

    fn load_tasks_by_agent(&self, agent_id: &str, limit: usize) -> Result<Vec<TaskRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, goal, status, steps_json, result_json, fuel_consumed, fuel_budget, estimated_time_secs, actual_time_secs, quality_score, started_at, completed_at, success
             FROM task_history WHERE agent_id = ?1
             ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![agent_id, limit as i64], TaskRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_json: Option<&str>,
        fuel: f64,
        success: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE task_history SET status = ?1, result_json = ?2, fuel_consumed = ?3, completed_at = ?4, success = ?5
             WHERE id = ?6",
            params![status, result_json, fuel, &now, success as i32, id],
        )?;
        Ok(())
    }

    // ── Hivemind Session Methods ─────────────────────────────────────────

    fn save_hivemind_session(&self, session: &HivemindSessionRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO hivemind_sessions (id, goal, status, sub_tasks_json, assignments_json, results_json, fuel_consumed, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                sub_tasks_json = excluded.sub_tasks_json,
                assignments_json = excluded.assignments_json,
                results_json = excluded.results_json,
                fuel_consumed = excluded.fuel_consumed,
                completed_at = excluded.completed_at",
            params![
                &session.id,
                &session.goal,
                &session.status,
                &session.sub_tasks_json,
                &session.assignments_json,
                &session.results_json,
                session.fuel_consumed,
                &session.started_at,
                &session.completed_at,
            ],
        )?;
        Ok(())
    }

    fn load_hivemind_session(&self, id: &str) -> Result<Option<HivemindSessionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, goal, status, sub_tasks_json, assignments_json, results_json, fuel_consumed, started_at, completed_at
             FROM hivemind_sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], HivemindSessionRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list_hivemind_sessions(&self) -> Result<Vec<HivemindSessionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, goal, status, sub_tasks_json, assignments_json, results_json, fuel_consumed, started_at, completed_at
             FROM hivemind_sessions ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map([], HivemindSessionRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn update_hivemind_session_status(&self, session: &HivemindSessionRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE hivemind_sessions SET status = ?1, results_json = ?2, fuel_consumed = ?3, completed_at = ?4
             WHERE id = ?5",
            params![
                &session.status,
                &session.results_json,
                session.fuel_consumed,
                &session.completed_at,
                &session.id,
            ],
        )?;
        Ok(())
    }

    fn upsert_strategy_score(
        &self,
        agent_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel: f64,
        duration: f64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let now = Utc::now().to_rfc3339();
        let success_inc: i64 = if success { 1 } else { 0 };
        conn.execute(
            "INSERT INTO strategy_scores (agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, 0.0, ?7)
             ON CONFLICT(agent_id, strategy_hash) DO UPDATE SET
                uses = uses + 1,
                successes = successes + ?4,
                total_fuel = total_fuel + ?5,
                total_duration_secs = total_duration_secs + ?6,
                goal_type = ?3,
                updated_at = ?7",
            params![agent_id, strategy_hash, goal_type, success_inc, fuel, duration, now],
        )?;
        Ok(())
    }

    fn load_top_strategies(
        &self,
        agent_id: &str,
        goal_type: &str,
        limit: usize,
    ) -> Result<Vec<StrategyScoreRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at
             FROM strategy_scores
             WHERE agent_id = ?1 AND goal_type = ?2
             ORDER BY composite_score DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![agent_id, goal_type, limit as i64],
            StrategyScoreRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn load_strategy_history(&self, agent_id: &str, limit: usize) -> Result<Vec<StrategyScoreRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, strategy_hash, goal_type, uses, successes, total_fuel, total_duration_secs, composite_score, updated_at
             FROM strategy_scores
             WHERE agent_id = ?1
             ORDER BY composite_score DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![agent_id, limit as i64], StrategyScoreRow::from_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── HITL Decision Methods ───────────────────────────────────────────

    fn record_hitl_decision(&self, decision: &HitlDecisionRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO hitl_decisions (id, agent_id, action, context_json, decision, decided_by, decided_at, response_time_ms, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &decision.id,
                &decision.agent_id,
                &decision.action,
                &decision.context_json,
                &decision.decision,
                &decision.decided_by,
                &decision.decided_at,
                decision.response_time_ms,
                &decision.metadata_json,
            ],
        )?;
        Ok(())
    }

    fn load_hitl_decisions(
        &self,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HitlDecisionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut result = Vec::new();
        match agent_id {
            Some(aid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, action, context_json, decision, decided_by, decided_at, response_time_ms, metadata_json
                     FROM hitl_decisions WHERE agent_id = ?1
                     ORDER BY decided_at DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![aid, limit as i64], HitlDecisionRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, action, context_json, decision, decided_by, decided_at, response_time_ms, metadata_json
                     FROM hitl_decisions ORDER BY decided_at DESC LIMIT ?1",
                )?;
                let rows = stmt.query_map(params![limit as i64], HitlDecisionRow::from_row)?;
                for row in rows {
                    result.push(row?);
                }
            }
        }
        Ok(result)
    }

    fn hitl_approval_rate(&self, agent_id: Option<&str>) -> Result<f64> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let (total, approved): (i64, i64) = match agent_id {
            Some(did) => {
                let total: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM hitl_decisions WHERE agent_id = ?1",
                    params![did],
                    |r| r.get(0),
                )?;
                let approved: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM hitl_decisions WHERE agent_id = ?1 AND decision = 'approved'",
                    params![did],
                    |r| r.get(0),
                )?;
                (total, approved)
            }
            None => {
                let total: i64 =
                    conn.query_row("SELECT COUNT(*) FROM hitl_decisions", [], |r| r.get(0))?;
                let approved: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM hitl_decisions WHERE decision = 'approved'",
                    [],
                    |r| r.get(0),
                )?;
                (total, approved)
            }
        };
        if total == 0 {
            return Ok(1.0);
        }
        Ok(approved as f64 / total as f64)
    }

    // ── Fuel Transaction Methods ────────────────────────────────────────

    fn append_fuel_transaction(&self, tx: &FuelTransactionRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO fuel_transactions (id, agent_id, operation, amount, balance_after, reservation_id, metadata_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &tx.id,
                &tx.agent_id,
                &tx.operation,
                tx.amount,
                tx.balance_after,
                &tx.reservation_id,
                &tx.metadata_json,
                &tx.created_at,
            ],
        )?;
        Ok(())
    }

    fn load_fuel_transactions(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<FuelTransactionRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, operation, amount, balance_after, reservation_id, metadata_json, created_at
             FROM fuel_transactions WHERE agent_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![agent_id, limit as i64],
            FuelTransactionRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    fn upsert_fuel_balance(
        &self,
        agent_id: &str,
        balance: i64,
        total_allocated: i64,
        total_consumed: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO fuel_balances (agent_id, balance, total_allocated, total_consumed, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(agent_id) DO UPDATE SET
                balance = excluded.balance,
                total_allocated = excluded.total_allocated,
                total_consumed = excluded.total_consumed,
                last_updated = excluded.last_updated",
            params![agent_id, balance, total_allocated, total_consumed, &now],
        )?;
        Ok(())
    }

    fn load_fuel_balance(&self, agent_id: &str) -> Result<Option<FuelBalanceRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT agent_id, balance, total_allocated, total_consumed, last_updated
             FROM fuel_balances WHERE agent_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![agent_id], FuelBalanceRow::from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Capability History Methods ──────────────────────────────────────

    fn append_capability_history(&self, entry: &CapabilityHistoryRow) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO capability_history (id, agent_id, capability, action, resource, performed_by, created_at, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &entry.id,
                &entry.agent_id,
                &entry.capability,
                &entry.action,
                &entry.resource,
                &entry.performed_by,
                &entry.created_at,
                &entry.metadata_json,
            ],
        )?;
        Ok(())
    }

    fn load_capability_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<CapabilityHistoryRow>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, capability, action, resource, performed_by, created_at, metadata_json
             FROM capability_history WHERE agent_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![agent_id, limit as i64],
            CapabilityHistoryRow::from_row,
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ── Audit Chain Verification ────────────────────────────────────────

    fn verify_audit_chain(&self) -> Result<ChainVerifyResult> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT sequence, previous_hash, current_hash FROM audit_events ORDER BY sequence ASC",
        )?;

        let start = std::time::Instant::now();
        let mut expected_prev: Option<String> = None;
        let mut count = 0u64;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row_result in rows {
            let (seq, prev_hash, current_hash) = row_result?;

            if let Some(ref expected) = expected_prev {
                if prev_hash != *expected {
                    return Ok(ChainVerifyResult {
                        verified: false,
                        chain_length: count,
                        break_at_sequence: Some(seq),
                        verification_time: start.elapsed(),
                    });
                }
            }

            expected_prev = Some(current_hash);
            count += 1;
        }

        Ok(ChainVerifyResult {
            verified: true,
            chain_length: count,
            break_at_sequence: None,
            verification_time: start.elapsed(),
        })
    }
}

#[cfg(any(test, feature = "testing"))]
impl NexusDatabase {
    /// Execute raw SQL — **test-only** (e.g., simulating audit tampering).
    ///
    /// Gated behind `#[cfg(any(test, feature = "testing"))]` to prevent
    /// production code from bypassing governed `StateStore` trait methods.
    pub fn execute_raw(&self, sql: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(sql, [])?;
        Ok(())
    }
}

fn agent_state_was_running(state: &str) -> bool {
    matches!(
        state,
        "running" | "paused" | "starting" | "Running" | "Paused" | "Starting"
    )
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> NexusDatabase {
        NexusDatabase::in_memory().expect("failed to create in-memory db")
    }

    // ── Agent Tests ─────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_agent() {
        let db = test_db();
        db.save_agent("agent-1", r#"{"name":"test"}"#, "created", 0, "native")
            .unwrap();
        let agent = db.load_agent("agent-1").unwrap().unwrap();
        assert_eq!(agent.id, "agent-1");
        assert_eq!(agent.manifest_json, r#"{"name":"test"}"#);
        assert_eq!(agent.state, "created");
        assert_eq!(agent.autonomy_level, 0);
        assert_eq!(agent.execution_mode, "native");
        assert_eq!(agent.parent_agent_id, None);
        assert!(!agent.created_at.is_empty());
        assert!(!agent.updated_at.is_empty());
    }

    #[test]
    fn test_save_agent_with_parent_lineage() {
        let db = test_db();
        db.save_agent_with_parent(
            "child-1",
            r#"{"name":"child"}"#,
            "created",
            4,
            "native",
            Some("parent-1"),
        )
        .unwrap();
        let agent = db.load_agent("child-1").unwrap().unwrap();
        assert_eq!(agent.parent_agent_id.as_deref(), Some("parent-1"));
    }

    #[test]
    fn test_update_agent_state() {
        let db = test_db();
        db.save_agent("agent-1", "{}", "created", 0, "native")
            .unwrap();
        let before = db.load_agent("agent-1").unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        db.update_agent_state("agent-1", "running").unwrap();
        let after = db.load_agent("agent-1").unwrap().unwrap();
        assert_eq!(after.state, "running");
        assert!(after.updated_at >= before.updated_at);
    }

    #[test]
    fn test_delete_agent() {
        let db = test_db();
        db.save_agent("agent-1", "{}", "created", 0, "native")
            .unwrap();
        db.delete_agent("agent-1").unwrap();
        assert!(db.load_agent("agent-1").unwrap().is_none());
    }

    #[test]
    fn test_list_agents() {
        let db = test_db();
        db.save_agent("a1", "{}", "created", 0, "native").unwrap();
        db.save_agent("a2", "{}", "running", 2, "wasm").unwrap();
        db.save_agent("a3", "{}", "stopped", 1, "native").unwrap();
        let agents = db.list_agents().unwrap();
        assert_eq!(agents.len(), 3);
    }

    #[test]
    fn test_update_nonexistent_agent_errors() {
        let db = test_db();
        let result = db.update_agent_state("ghost", "running");
        assert!(result.is_err());
    }

    #[test]
    fn test_save_agent_upsert() {
        let db = test_db();
        db.save_agent("agent-1", r#"{"v":1}"#, "created", 0, "native")
            .unwrap();
        db.save_agent("agent-1", r#"{"v":2}"#, "running", 3, "wasm")
            .unwrap();
        let agent = db.load_agent("agent-1").unwrap().unwrap();
        assert_eq!(agent.manifest_json, r#"{"v":2}"#);
        assert_eq!(agent.state, "running");
        assert_eq!(agent.autonomy_level, 3);
    }

    // ── Audit Tests ─────────────────────────────────────────────────────

    #[test]
    fn test_append_and_load_audit_events() {
        let db = test_db();
        let mut prev_hash = "0000000000000000".to_string();
        for i in 0..100 {
            let hash = format!("hash_{i:04}");
            db.append_audit_event("agent-1", "ToolCall", "{}", &prev_hash, &hash, i)
                .unwrap();
            prev_hash = hash;
        }
        let count = db.get_audit_count().unwrap();
        assert_eq!(count, 100);

        // Verify hash chain
        let events = db.load_audit_events(None, 100, 0).unwrap();
        assert_eq!(events.len(), 100);
        for i in 1..events.len() {
            assert_eq!(events[i].previous_hash, events[i - 1].current_hash);
        }
    }

    #[test]
    fn test_audit_pagination() {
        let db = test_db();
        for i in 0..50 {
            db.append_audit_event("a1", "ToolCall", "{}", "prev", &format!("h{i}"), i)
                .unwrap();
        }
        let page1 = db.load_audit_events(None, 10, 0).unwrap();
        assert_eq!(page1.len(), 10);
        let page2 = db.load_audit_events(None, 10, 10).unwrap();
        assert_eq!(page2.len(), 10);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[test]
    fn test_audit_filter_by_agent() {
        let db = test_db();
        db.append_audit_event("a1", "ToolCall", "{}", "p", "h1", 0)
            .unwrap();
        db.append_audit_event("a2", "UserAction", "{}", "p", "h2", 1)
            .unwrap();
        db.append_audit_event("a1", "StateChange", "{}", "p", "h3", 2)
            .unwrap();
        let a1_events = db.load_audit_events(Some("a1"), 100, 0).unwrap();
        assert_eq!(a1_events.len(), 2);
        let a2_events = db.load_audit_events(Some("a2"), 100, 0).unwrap();
        assert_eq!(a2_events.len(), 1);
    }

    #[test]
    fn test_latest_audit_hash() {
        let db = test_db();
        assert!(db.get_latest_audit_hash().unwrap().is_none());
        db.append_audit_event("a1", "ToolCall", "{}", "prev", "hash_final", 0)
            .unwrap();
        let hash = db.get_latest_audit_hash().unwrap().unwrap();
        assert_eq!(hash, "hash_final");
    }

    #[test]
    fn test_audit_returns_autoincrement_id() {
        let db = test_db();
        let id1 = db
            .append_audit_event("a1", "ToolCall", "{}", "p", "h1", 0)
            .unwrap();
        let id2 = db
            .append_audit_event("a1", "ToolCall", "{}", "p", "h2", 1)
            .unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    // ── Fuel Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_fuel_ledger() {
        let db = test_db();
        let ledger = FuelLedgerRow {
            agent_id: "a1".to_string(),
            budget_total: 1000.0,
            budget_consumed: 250.5,
            period_start: "2026-01-01T00:00:00Z".to_string(),
            period_end: "2026-02-01T00:00:00Z".to_string(),
            anomaly_count: 3,
            ledger_json: r#"{"entries":[]}"#.to_string(),
            updated_at: String::new(),
        };
        db.save_fuel_ledger("a1", &ledger).unwrap();
        let loaded = db.load_fuel_ledger("a1").unwrap().unwrap();
        assert!((loaded.budget_total - 1000.0).abs() < f64::EPSILON);
        assert!((loaded.budget_consumed - 250.5).abs() < f64::EPSILON);
        assert_eq!(loaded.anomaly_count, 3);
        assert_eq!(loaded.period_start, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn test_fuel_ledger_upsert() {
        let db = test_db();
        let ledger1 = FuelLedgerRow {
            agent_id: "a1".to_string(),
            budget_total: 1000.0,
            budget_consumed: 0.0,
            period_start: "2026-01-01T00:00:00Z".to_string(),
            period_end: "2026-02-01T00:00:00Z".to_string(),
            anomaly_count: 0,
            ledger_json: "{}".to_string(),
            updated_at: String::new(),
        };
        db.save_fuel_ledger("a1", &ledger1).unwrap();
        let ledger2 = FuelLedgerRow {
            budget_consumed: 500.0,
            anomaly_count: 2,
            ..ledger1
        };
        db.save_fuel_ledger("a1", &ledger2).unwrap();
        let loaded = db.load_fuel_ledger("a1").unwrap().unwrap();
        assert!((loaded.budget_consumed - 500.0).abs() < f64::EPSILON);
        assert_eq!(loaded.anomaly_count, 2);
    }

    #[test]
    fn test_fuel_ledger_not_found() {
        let db = test_db();
        assert!(db.load_fuel_ledger("ghost").unwrap().is_none());
    }

    // ── Permission Tests ────────────────────────────────────────────────

    #[test]
    fn test_grant_and_load_permission() {
        let db = test_db();
        db.grant_permission("a1", "file.read", "low").unwrap();
        let perms = db.load_permissions("a1").unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].capability, "file.read");
        assert_eq!(perms[0].risk_level, "low");
        assert!(perms[0].granted);
        assert!(perms[0].revoked_at.is_none());
    }

    #[test]
    fn test_revoke_permission() {
        let db = test_db();
        db.grant_permission("a1", "file.write", "medium").unwrap();
        db.revoke_permission("a1", "file.write").unwrap();
        let perms = db.load_permissions("a1").unwrap();
        assert_eq!(perms.len(), 1);
        assert!(!perms[0].granted);
        assert!(perms[0].revoked_at.is_some());
    }

    #[test]
    fn test_grant_permission_upsert_no_duplicate() {
        let db = test_db();
        db.grant_permission("a1", "net.http", "high").unwrap();
        db.grant_permission("a1", "net.http", "critical").unwrap();
        let perms = db.load_permissions("a1").unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].risk_level, "critical");
    }

    #[test]
    fn test_revoke_and_regrant_permission() {
        let db = test_db();
        db.grant_permission("a1", "exec.shell", "high").unwrap();
        db.revoke_permission("a1", "exec.shell").unwrap();
        db.grant_permission("a1", "exec.shell", "high").unwrap();
        let perms = db.load_permissions("a1").unwrap();
        assert_eq!(perms.len(), 1);
        assert!(perms[0].granted);
        assert!(perms[0].revoked_at.is_none());
    }

    // ── Consent Tests ───────────────────────────────────────────────────

    #[test]
    fn test_enqueue_and_load_consent() {
        let db = test_db();
        let request = ConsentRow {
            id: "consent-1".to_string(),
            agent_id: "a1".to_string(),
            operation_type: "file.delete".to_string(),
            operation_json: r#"{"path":"/etc"}"#.to_string(),
            hitl_tier: "Tier2".to_string(),
            status: "pending".to_string(),
            created_at: Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        };
        db.enqueue_consent(&request).unwrap();
        let pending = db.load_pending_consent().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "consent-1");
        assert_eq!(pending[0].hitl_tier, "Tier2");
    }

    #[test]
    fn test_resolve_consent() {
        let db = test_db();
        let request = ConsentRow {
            id: "consent-2".to_string(),
            agent_id: "a1".to_string(),
            operation_type: "net.send".to_string(),
            operation_json: "{}".to_string(),
            hitl_tier: "Tier1".to_string(),
            status: "pending".to_string(),
            created_at: Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        };
        db.enqueue_consent(&request).unwrap();
        db.resolve_consent("consent-2", "approved", "admin")
            .unwrap();
        let pending = db.load_pending_consent().unwrap();
        assert!(pending.is_empty());
        let by_agent = db.load_consent_by_agent("a1").unwrap();
        assert_eq!(by_agent.len(), 1);
        assert_eq!(by_agent[0].status, "approved");
        assert_eq!(by_agent[0].resolved_by.as_deref(), Some("admin"));
        assert!(by_agent[0].resolved_at.is_some());
    }

    #[test]
    fn test_consent_filter_by_agent() {
        let db = test_db();
        for (i, agent) in ["a1", "a2", "a1"].iter().enumerate() {
            db.enqueue_consent(&ConsentRow {
                id: format!("c{i}"),
                agent_id: agent.to_string(),
                operation_type: "test".to_string(),
                operation_json: "{}".to_string(),
                hitl_tier: "Tier0".to_string(),
                status: "pending".to_string(),
                created_at: Utc::now().to_rfc3339(),
                resolved_at: None,
                resolved_by: None,
            })
            .unwrap();
        }
        assert_eq!(db.load_consent_by_agent("a1").unwrap().len(), 2);
        assert_eq!(db.load_consent_by_agent("a2").unwrap().len(), 1);
    }

    // ── Embedding Tests ─────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_embedding() {
        let db = test_db();
        let vec_json = serde_json::to_string(&vec![0.1_f64, 0.2, 0.3]).unwrap();
        let emb = EmbeddingRow {
            id: "emb-1".to_string(),
            agent_id: "a1".to_string(),
            content_hash: "sha256_abc".to_string(),
            chunk_text: "hello world".to_string(),
            vector_json: vec_json.clone(),
            metadata_json: r#"{"source":"doc1"}"#.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };
        db.save_embedding(&emb).unwrap();
        let loaded = db.load_embeddings_by_agent("a1").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].vector_json, vec_json);
        assert_eq!(loaded[0].chunk_text, "hello world");
    }

    #[test]
    fn test_delete_embeddings_by_hash() {
        let db = test_db();
        for i in 0..3 {
            db.save_embedding(&EmbeddingRow {
                id: format!("emb-{i}"),
                agent_id: "a1".to_string(),
                content_hash: "same_hash".to_string(),
                chunk_text: format!("chunk {i}"),
                vector_json: "[]".to_string(),
                metadata_json: "{}".to_string(),
                created_at: Utc::now().to_rfc3339(),
            })
            .unwrap();
        }
        assert_eq!(db.load_embeddings_by_agent("a1").unwrap().len(), 3);
        db.delete_embeddings_by_hash("same_hash").unwrap();
        assert!(db.load_embeddings_by_agent("a1").unwrap().is_empty());
    }

    // ── Memory Tests ────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_memory() {
        let db = test_db();
        db.save_memory("a1", "episodic", "task_completed", r#"{"task":"build"}"#)
            .unwrap();
        let memories = db.load_memories("a1", Some("episodic"), 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].key, "task_completed");
        assert!((memories[0].relevance_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(memories[0].access_count, 0);
    }

    #[test]
    fn test_load_memories_all_types() {
        let db = test_db();
        db.save_memory("a1", "episodic", "k1", "{}").unwrap();
        db.save_memory("a1", "semantic", "k2", "{}").unwrap();
        db.save_memory("a1", "procedural", "k3", "{}").unwrap();
        let all = db.load_memories("a1", None, 100).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_touch_memory() {
        let db = test_db();
        db.save_memory("a1", "episodic", "key1", "{}").unwrap();
        let memories = db.load_memories("a1", None, 10).unwrap();
        let mem_id = memories[0].id;
        db.touch_memory(mem_id).unwrap();
        db.touch_memory(mem_id).unwrap();
        let updated = db.load_memories("a1", None, 10).unwrap();
        assert_eq!(updated[0].access_count, 2);
    }

    #[test]
    fn test_decay_memories() {
        let db = test_db();
        db.save_memory("a1", "episodic", "old_memory", "{}")
            .unwrap();
        // Force last_accessed to be old so decay applies
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_memory SET last_accessed = datetime('now', '-48 hours') WHERE agent_id = 'a1'",
                [],
            )
            .unwrap();
        }
        db.decay_memories("a1", 0.5).unwrap();
        let memories = db.load_memories("a1", None, 10).unwrap();
        assert!(memories[0].relevance_score < 1.0);
        assert!((memories[0].relevance_score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_memory_limit() {
        let db = test_db();
        for i in 0..20 {
            db.save_memory("a1", "semantic", &format!("k{i}"), "{}")
                .unwrap();
        }
        let limited = db.load_memories("a1", None, 5).unwrap();
        assert_eq!(limited.len(), 5);
    }

    // ── Task Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_task() {
        let db = test_db();
        let task = TaskRow {
            id: "task-1".to_string(),
            agent_id: "a1".to_string(),
            goal: "build website".to_string(),
            status: "running".to_string(),
            steps_json: r#"["plan","code","deploy"]"#.to_string(),
            result_json: None,
            fuel_consumed: 0.0,
            fuel_budget: Some(100.0),
            estimated_time_secs: Some(60.0),
            actual_time_secs: Some(55.0),
            quality_score: Some(8.5),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            success: false,
        };
        db.save_task(&task).unwrap();
        let loaded = db.load_tasks_by_agent("a1", 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].goal, "build website");
        assert_eq!(loaded[0].status, "running");
        assert!(!loaded[0].success);
    }

    #[test]
    fn test_update_task_status() {
        let db = test_db();
        let task = TaskRow {
            id: "task-2".to_string(),
            agent_id: "a1".to_string(),
            goal: "analyze data".to_string(),
            status: "running".to_string(),
            steps_json: "[]".to_string(),
            result_json: None,
            fuel_consumed: 0.0,
            fuel_budget: Some(120.0),
            estimated_time_secs: Some(80.0),
            actual_time_secs: Some(70.0),
            quality_score: Some(9.0),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            success: false,
        };
        db.save_task(&task).unwrap();
        db.update_task_status("task-2", "completed", Some(r#"{"rows":42}"#), 150.5, true)
            .unwrap();
        let loaded = db.load_tasks_by_agent("a1", 10).unwrap();
        assert_eq!(loaded[0].status, "completed");
        assert_eq!(loaded[0].result_json.as_deref(), Some(r#"{"rows":42}"#));
        assert!((loaded[0].fuel_consumed - 150.5).abs() < f64::EPSILON);
        assert!(loaded[0].success);
        assert!(loaded[0].completed_at.is_some());
    }

    #[test]
    fn test_task_ordering_desc() {
        let db = test_db();
        for i in 0..5 {
            let task = TaskRow {
                id: format!("t{i}"),
                agent_id: "a1".to_string(),
                goal: format!("goal {i}"),
                status: "completed".to_string(),
                steps_json: "[]".to_string(),
                result_json: None,
                fuel_consumed: 0.0,
                fuel_budget: Some(50.0),
                estimated_time_secs: Some(30.0),
                actual_time_secs: Some(25.0),
                quality_score: Some(7.5),
                started_at: format!("2026-01-0{}T00:00:00Z", i + 1),
                completed_at: None,
                success: true,
            };
            db.save_task(&task).unwrap();
        }
        let tasks = db.load_tasks_by_agent("a1", 3).unwrap();
        assert_eq!(tasks.len(), 3);
        // Most recent first
        assert_eq!(tasks[0].id, "t4");
        assert_eq!(tasks[1].id, "t3");
        assert_eq!(tasks[2].id, "t2");
    }

    #[test]
    fn test_task_metrics_roundtrip() {
        let db = test_db();
        let task = TaskRow {
            id: "task-metrics".to_string(),
            agent_id: "a1".to_string(),
            goal: "measure".to_string(),
            status: "completed".to_string(),
            steps_json: "[]".to_string(),
            result_json: None,
            fuel_consumed: 45.0,
            fuel_budget: Some(60.0),
            estimated_time_secs: Some(30.0),
            actual_time_secs: Some(24.0),
            quality_score: Some(8.2),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            success: true,
        };
        db.save_task(&task).unwrap();
        let loaded = db.load_tasks_by_agent("a1", 10).unwrap();
        assert_eq!(loaded[0].fuel_budget, Some(60.0));
        assert_eq!(loaded[0].estimated_time_secs, Some(30.0));
        assert_eq!(loaded[0].actual_time_secs, Some(24.0));
        assert_eq!(loaded[0].quality_score, Some(8.2));
    }

    #[test]
    fn test_save_and_load_evolution_archive() {
        let db = test_db();
        db.save_evolution_variant("engine-1", 1, r#"{"id":"v1"}"#, 0.91)
            .unwrap();
        db.save_evolution_variant("engine-1", 2, r#"{"id":"v2"}"#, 0.95)
            .unwrap();
        let rows = db.load_evolution_archive("engine-1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].generation, 2);
        assert_eq!(rows[0].fitness, 0.95);
    }

    #[test]
    fn test_save_and_load_swarm_state() {
        let db = test_db();
        db.save_swarm_state("swarm-1", 3, r#"[{"agent":"a"}]"#, Some(r#"{"score":1.0}"#))
            .unwrap();
        let row = db.load_latest_swarm_state("swarm-1").unwrap().unwrap();
        assert_eq!(row.iteration, 3);
        assert_eq!(row.global_best_json.as_deref(), Some(r#"{"score":1.0}"#));
    }

    #[test]
    fn test_replace_and_load_world_model_state() {
        let db = test_db();
        db.replace_world_model_state(
            "agent-1",
            &[r#"{"id":"e1"}"#.to_string(), r#"{"id":"e2"}"#.to_string()],
            &[r#"{"from":"e1","to":"e2"}"#.to_string()],
        )
        .unwrap();
        let entities = db.load_world_model_entities("agent-1").unwrap();
        let relationships = db.load_world_model_relationships("agent-1").unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(relationships.len(), 1);

        db.replace_world_model_state("agent-1", &[r#"{"id":"e3"}"#.to_string()], &[])
            .unwrap();
        let entities = db.load_world_model_entities("agent-1").unwrap();
        let relationships = db.load_world_model_relationships("agent-1").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_json, r#"{"id":"e3"}"#);
        assert!(relationships.is_empty());
    }

    #[test]
    fn test_save_and_load_adversarial_matches() {
        let db = test_db();
        db.save_adversarial_match("arena-1", "att-1", "def-1", true, Some("high"))
            .unwrap();
        db.save_adversarial_match("arena-1", "att-2", "def-1", false, None)
            .unwrap();
        let rows = db.load_adversarial_matches("arena-1", 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].attacker_id, "att-2");
        assert!(!rows[0].succeeded);
    }

    #[test]
    fn test_save_and_load_evolution_history() {
        let db = test_db();
        let id = db
            .save_evolution_history(
                "agent-1",
                3,
                "before prompt",
                "after prompt",
                "self_modify_description",
                0.72,
                0.81,
                true,
            )
            .unwrap();
        assert!(id > 0);

        let history = db.load_evolution_history("agent-1", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version, 3);
        assert_eq!(history[0].trigger, "self_modify_description");
        assert!(history[0].kept);
    }

    // ── Edge Case Tests ─────────────────────────────────────────────────

    #[test]
    fn test_large_manifest_payload() {
        let db = test_db();
        let large_json = format!(r#"{{"data":"{}"}}"#, "x".repeat(100_000));
        db.save_agent("big", &large_json, "created", 0, "native")
            .unwrap();
        let loaded = db.load_agent("big").unwrap().unwrap();
        assert_eq!(loaded.manifest_json.len(), large_json.len());
    }

    #[test]
    fn test_empty_tables_return_empty_vecs() {
        let db = test_db();
        assert!(db.list_agents().unwrap().is_empty());
        assert!(db.load_audit_events(None, 100, 0).unwrap().is_empty());
        assert!(db.load_permissions("ghost").unwrap().is_empty());
        assert!(db.load_pending_consent().unwrap().is_empty());
        assert!(db.load_embeddings_by_agent("ghost").unwrap().is_empty());
        assert!(db.load_memories("ghost", None, 10).unwrap().is_empty());
        assert!(db.load_tasks_by_agent("ghost", 10).unwrap().is_empty());
        assert!(db.load_consent_by_agent("ghost").unwrap().is_empty());
    }

    #[test]
    fn test_load_nonexistent_agent_returns_none() {
        let db = test_db();
        assert!(db.load_agent("nope").unwrap().is_none());
    }

    #[test]
    fn test_load_nonexistent_fuel_returns_none() {
        let db = test_db();
        assert!(db.load_fuel_ledger("nope").unwrap().is_none());
    }

    #[test]
    fn test_save_and_load_simulation_world() {
        let db = test_db();
        db.save_simulation_world(
            "world-1",
            "Forecast",
            "seed text",
            "running",
            3,
            12,
            r#"{"name":"Forecast"}"#,
            None,
            None,
        )
        .unwrap();
        let loaded = db.load_simulation_world("world-1").unwrap().unwrap();
        assert_eq!(loaded.name, "Forecast");
        assert_eq!(loaded.tick_count, 3);
    }

    #[test]
    fn test_save_and_load_simulation_events_by_tick() {
        let db = test_db();
        db.append_simulation_event("world-1", 0, "p-1", "speak", Some("hello"), None, 0.5)
            .unwrap();
        db.append_simulation_event("world-1", 1, "p-2", "observe", None, None, 0.1)
            .unwrap();
        let tick_zero = db.load_simulation_events("world-1", Some(0)).unwrap();
        assert_eq!(tick_zero.len(), 1);
        assert_eq!(tick_zero[0].action_type, "speak");
    }

    #[test]
    fn test_save_and_load_simulation_personas() {
        let db = test_db();
        db.replace_simulation_personas(
            "world-1",
            &[(
                "p-1".to_string(),
                "Ada".to_string(),
                "analyst".to_string(),
                "{}".to_string(),
                r#"{"topic":0.4}"#.to_string(),
                "[]".to_string(),
                "{}".to_string(),
            )],
        )
        .unwrap();
        let loaded = db.load_simulation_personas("world-1").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Ada");
    }

    #[test]
    fn test_list_simulation_worlds_orders_latest_first() {
        let db = test_db();
        db.save_simulation_world(
            "world-a",
            "A",
            "seed",
            "completed",
            1,
            3,
            "{}",
            Some("{}"),
            Some("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        db.save_simulation_world("world-b", "B", "seed", "running", 2, 4, "{}", None, None)
            .unwrap();
        let worlds = db.list_simulation_worlds().unwrap();
        assert_eq!(worlds.len(), 2);
    }

    #[test]
    fn test_default_db_path() {
        let path = NexusDatabase::default_db_path();
        assert!(path.ends_with("nexus.db"));
        assert!(path.to_string_lossy().contains(".nexus"));
    }

    #[test]
    fn test_multiple_agents_multiple_permissions() {
        let db = test_db();
        db.grant_permission("a1", "file.read", "low").unwrap();
        db.grant_permission("a1", "file.write", "medium").unwrap();
        db.grant_permission("a2", "net.http", "high").unwrap();
        assert_eq!(db.load_permissions("a1").unwrap().len(), 2);
        assert_eq!(db.load_permissions("a2").unwrap().len(), 1);
    }

    #[test]
    fn test_embedding_upsert() {
        let db = test_db();
        db.save_embedding(&EmbeddingRow {
            id: "emb-1".to_string(),
            agent_id: "a1".to_string(),
            content_hash: "hash1".to_string(),
            chunk_text: "original".to_string(),
            vector_json: "[1.0]".to_string(),
            metadata_json: "{}".to_string(),
            created_at: Utc::now().to_rfc3339(),
        })
        .unwrap();
        db.save_embedding(&EmbeddingRow {
            id: "emb-1".to_string(),
            agent_id: "a1".to_string(),
            content_hash: "hash1".to_string(),
            chunk_text: "original".to_string(),
            vector_json: "[2.0]".to_string(),
            metadata_json: r#"{"updated":true}"#.to_string(),
            created_at: Utc::now().to_rfc3339(),
        })
        .unwrap();
        let loaded = db.load_embeddings_by_agent("a1").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].vector_json, "[2.0]");
    }

    #[test]
    fn test_consent_multiple_pending() {
        let db = test_db();
        for i in 0..5 {
            db.enqueue_consent(&ConsentRow {
                id: format!("c{i}"),
                agent_id: "a1".to_string(),
                operation_type: "test".to_string(),
                operation_json: "{}".to_string(),
                hitl_tier: "Tier1".to_string(),
                status: "pending".to_string(),
                created_at: Utc::now().to_rfc3339(),
                resolved_at: None,
                resolved_by: None,
            })
            .unwrap();
        }
        assert_eq!(db.load_pending_consent().unwrap().len(), 5);
        db.resolve_consent("c0", "approved", "admin").unwrap();
        db.resolve_consent("c1", "denied", "admin").unwrap();
        assert_eq!(db.load_pending_consent().unwrap().len(), 3);
    }

    #[test]
    fn test_load_all_consents() {
        let db = test_db();
        for i in 0..8 {
            db.enqueue_consent(&ConsentRow {
                id: format!("ca{i}"),
                agent_id: "a1".to_string(),
                operation_type: "test".to_string(),
                operation_json: "{}".to_string(),
                hitl_tier: "Tier1".to_string(),
                status: "pending".to_string(),
                created_at: Utc::now().to_rfc3339(),
                resolved_at: None,
                resolved_by: None,
            })
            .unwrap();
        }
        db.resolve_consent("ca0", "approved", "admin").unwrap();
        db.resolve_consent("ca1", "denied", "admin").unwrap();

        // load_all_consents returns all regardless of status
        let all = db.load_all_consents(100).unwrap();
        assert_eq!(all.len(), 8);

        // Respects limit
        let limited = db.load_all_consents(3).unwrap();
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn test_memory_types_filter() {
        let db = test_db();
        db.save_memory("a1", "episodic", "e1", "{}").unwrap();
        db.save_memory("a1", "semantic", "s1", "{}").unwrap();
        db.save_memory("a1", "procedural", "p1", "{}").unwrap();
        db.save_memory("a1", "preference", "pf1", "{}").unwrap();
        assert_eq!(
            db.load_memories("a1", Some("episodic"), 10).unwrap().len(),
            1
        );
        assert_eq!(
            db.load_memories("a1", Some("semantic"), 10).unwrap().len(),
            1
        );
        assert_eq!(
            db.load_memories("a1", Some("procedural"), 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.load_memories("a1", Some("preference"), 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(db.load_memories("a1", None, 10).unwrap().len(), 4);
    }

    #[test]
    fn test_task_upsert() {
        let db = test_db();
        let task = TaskRow {
            id: "task-u".to_string(),
            agent_id: "a1".to_string(),
            goal: "test".to_string(),
            status: "running".to_string(),
            steps_json: "[]".to_string(),
            result_json: None,
            fuel_consumed: 0.0,
            fuel_budget: Some(10.0),
            estimated_time_secs: Some(5.0),
            actual_time_secs: Some(4.0),
            quality_score: Some(6.0),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            success: false,
        };
        db.save_task(&task).unwrap();
        let updated = TaskRow {
            status: "completed".to_string(),
            fuel_consumed: 100.0,
            success: true,
            ..task
        };
        db.save_task(&updated).unwrap();
        let loaded = db.load_tasks_by_agent("a1", 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, "completed");
    }

    #[test]
    fn test_delete_agent_does_not_error_on_missing() {
        let db = test_db();
        db.delete_agent("nonexistent").unwrap();
    }

    #[test]
    fn test_audit_count_empty() {
        let db = test_db();
        assert_eq!(db.get_audit_count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_load_l6_cooldown() {
        let db = test_db();
        db.upsert_l6_cooldown("agent-l6", 42, Some("2026-03-14T00:00:00Z"), 2)
            .unwrap();
        let row = db.load_l6_cooldown("agent-l6").unwrap().unwrap();
        assert_eq!(row.cycle_count, 42);
        assert_eq!(row.total_cooldowns, 2);
        assert_eq!(row.last_cooldown.as_deref(), Some("2026-03-14T00:00:00Z"));
    }

    #[test]
    fn test_save_and_load_algorithm_selection() {
        let db = test_db();
        db.save_algorithm_selection(
            "agent-l6",
            "task-1",
            "world_model",
            r#"{"depth":2}"#,
            Some(0.9),
        )
        .unwrap();
        let rows = db.load_algorithm_selections("agent-l6", 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].algorithm, "world_model");
        assert_eq!(rows[0].task_id, "task-1");
    }

    #[test]
    fn test_save_and_list_agent_ecosystem() {
        let db = test_db();
        db.save_agent_ecosystem("eco-1", "creator-1", "[]", 3, 1500.0, "active")
            .unwrap();
        let ecosystem = db.load_agent_ecosystem("eco-1").unwrap().unwrap();
        assert_eq!(ecosystem.agent_count, 3);
        assert_eq!(ecosystem.total_fuel_allocated, 1500.0);

        let rows = db.list_agent_ecosystems("creator-1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "eco-1");
    }
}
