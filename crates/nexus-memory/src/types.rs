//! Core types for the Nexus OS Agent Memory Subsystem.
//!
//! Every type here derives `Debug, Clone, Serialize, Deserialize` so it can be
//! persisted, sent over IPC, and inspected in audit logs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a memory entry.
pub type MemoryId = Uuid;

// ── Memory classification ───────────────────────────────────────────────────

/// The four memory types forming the agent's cognitive architecture.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Short-lived scratch space for the current task context.
    Working,
    /// Append-only chronicle of events and actions.
    Episodic,
    /// Structured knowledge (triples, assertions, entity records).
    Semantic,
    /// Learned procedures and workflows.
    Procedural,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Working => write!(f, "Working"),
            Self::Episodic => write!(f, "Episodic"),
            Self::Semantic => write!(f, "Semantic"),
            Self::Procedural => write!(f, "Procedural"),
        }
    }
}

// ── Epistemic classification ────────────────────────────────────────────────

/// Epistemic classification — determines trust, retrieval priority, and GC
/// behaviour.  Every memory entry carries exactly one class.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpistemicClass {
    /// Directly observed by the agent during execution (highest trust).
    Observation,
    /// Explicitly stated by a human user.
    UserAssertion,
    /// Inferred by the agent from other data.
    Inference { derived_from: Vec<MemoryId> },
    /// Retrieved from an external source, not independently validated.
    CachedRetrieval {
        source_url: Option<String>,
        retrieved_at: DateTime<Utc>,
    },
    /// Compressed summary of multiple entries.
    Summary { summarizes: Vec<MemoryId> },
    /// Promoted from repeated successful execution (procedural only).
    LearnedBehavior {
        evidence_task_ids: Vec<String>,
        success_rate: f32,
    },
    /// Received from another agent.
    SharedKnowledge {
        source_agent_id: String,
        original_class: Box<EpistemicClass>,
    },
    /// Imported from an external system.
    Imported { source_system: String },
    /// System-generated (governance events, audit markers).
    SystemGenerated,
}

impl EpistemicClass {
    /// Returns the simplified filter variant that matches this class.
    pub fn to_filter(&self) -> EpistemicClassFilter {
        match self {
            Self::Observation => EpistemicClassFilter::Observation,
            Self::UserAssertion => EpistemicClassFilter::UserAssertion,
            Self::Inference { .. } => EpistemicClassFilter::Inference,
            Self::CachedRetrieval { .. } => EpistemicClassFilter::CachedRetrieval,
            Self::Summary { .. } => EpistemicClassFilter::Summary,
            Self::LearnedBehavior { .. } => EpistemicClassFilter::LearnedBehavior,
            Self::SharedKnowledge { .. } => EpistemicClassFilter::SharedKnowledge,
            Self::Imported { .. } => EpistemicClassFilter::Imported,
            Self::SystemGenerated => EpistemicClassFilter::SystemGenerated,
        }
    }

    /// Default trust score for this epistemic class.
    pub fn default_trust(&self) -> f32 {
        match self {
            Self::Observation => 0.95,
            Self::UserAssertion => 0.90,
            Self::Inference { .. } => 0.60,
            Self::CachedRetrieval { .. } => 0.40,
            Self::Summary { .. } => 0.70,
            Self::LearnedBehavior { success_rate, .. } => *success_rate,
            Self::SharedKnowledge { .. } => 0.50,
            Self::Imported { .. } => 0.45,
            Self::SystemGenerated => 0.85,
        }
    }
}

// ── Validation state ────────────────────────────────────────────────────────

/// Validation state of a memory entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationState {
    /// Not yet verified by any means.
    Unverified,
    /// Supported by additional evidence.
    Corroborated,
    /// Another entry contradicts this one.
    Contested,
    /// Superseded by a newer entry but kept for history.
    Deprecated,
    /// Explicitly invalidated (by human, governance, or regression).
    Revoked,
}

impl std::fmt::Display for ValidationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => write!(f, "Unverified"),
            Self::Corroborated => write!(f, "Corroborated"),
            Self::Contested => write!(f, "Contested"),
            Self::Deprecated => write!(f, "Deprecated"),
            Self::Revoked => write!(f, "Revoked"),
        }
    }
}

// ── Scope & sensitivity ─────────────────────────────────────────────────────

/// Visibility boundary for memory entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryScope {
    /// Visible only within a specific task.
    Task(String),
    /// Visible only to this agent.
    Agent,
    /// Shared with specifically granted agents.
    Shared,
    /// Visible to all agents in the deployment.
    Organization,
}

/// Sensitivity classification for compliance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SensitivityClass {
    /// No restrictions.
    Public,
    /// Internal use only.
    Internal,
    /// Requires access control.
    Sensitive,
    /// Highest protection — PII, credentials, financial.
    Restricted,
}

impl std::fmt::Display for SensitivityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "Public"),
            Self::Internal => write!(f, "Internal"),
            Self::Sensitive => write!(f, "Sensitive"),
            Self::Restricted => write!(f, "Restricted"),
        }
    }
}

// ── Memory content ──────────────────────────────────────────────────────────

/// Memory content variants — one per memory type, plus semantic sub-variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryContent {
    // === Working Memory ===
    /// Key-value context for the current task.
    Context {
        key: String,
        value: serde_json::Value,
    },

    // === Episodic Memory ===
    /// A recorded event or action.
    Episode {
        event_type: EpisodeType,
        summary: String,
        details: serde_json::Value,
        outcome: Option<Outcome>,
        duration_ms: Option<u64>,
    },

    // === Semantic Memory (multiple forms) ===
    /// Subject-predicate-object triple.
    Triple {
        subject: String,
        predicate: String,
        object: String,
    },
    /// A natural-language assertion with citations.
    Assertion {
        statement: String,
        citations: Vec<String>,
    },
    /// An entity with typed attributes.
    EntityRecord {
        name: String,
        entity_type: String,
        attributes: HashMap<String, serde_json::Value>,
    },
    /// A fact with temporal validity.
    TemporalFact {
        statement: String,
        effective_from: DateTime<Utc>,
        effective_to: Option<DateTime<Utc>>,
        context: String,
    },

    // === Procedural Memory ===
    /// A learned procedure or workflow.
    Procedure {
        name: String,
        description: String,
        trigger_condition: String,
        steps: Vec<ProcedureStep>,
    },
}

impl MemoryContent {
    /// Returns the expected `MemoryType` for this content variant.
    pub fn expected_memory_type(&self) -> MemoryType {
        match self {
            Self::Context { .. } => MemoryType::Working,
            Self::Episode { .. } => MemoryType::Episodic,
            Self::Triple { .. }
            | Self::Assertion { .. }
            | Self::EntityRecord { .. }
            | Self::TemporalFact { .. } => MemoryType::Semantic,
            Self::Procedure { .. } => MemoryType::Procedural,
        }
    }

    /// Returns the context key if this is a `Context` variant.
    pub fn context_key(&self) -> Option<&str> {
        match self {
            Self::Context { key, .. } => Some(key),
            _ => None,
        }
    }

    /// Returns the episode type if this is an `Episode` variant.
    pub fn episode_type(&self) -> Option<&EpisodeType> {
        match self {
            Self::Episode { event_type, .. } => Some(event_type),
            _ => None,
        }
    }
}

/// Types of episodic events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EpisodeType {
    Conversation,
    ActionExecuted,
    ActionBlocked,
    ObservationMade,
    ErrorEncountered,
    GoalAchieved,
    GoalFailed,
    HitlDecision,
    RollbackOccurred,
    MemoryPromoted,
    MemoryDemoted,
}

impl std::fmt::Display for EpisodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Outcome of an action or goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Outcome {
    Success {
        details: String,
    },
    Failure {
        reason: String,
    },
    Partial {
        completed: String,
        remaining: String,
    },
    Blocked {
        by: String,
    },
}

/// A single step in a learned procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureStep {
    /// Execution order (1-based).
    pub order: u32,
    /// Human-readable description of the step.
    pub description: String,
    /// Optional tool identifier to invoke.
    pub tool: Option<String>,
    /// Expected outcome description.
    pub expected_outcome: Option<String>,
}

// ── The complete memory entry ───────────────────────────────────────────────

/// The complete memory entry — the atomic unit of the memory subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // ── Identity ──
    /// Unique identifier.
    pub id: MemoryId,
    /// Schema version for forward-compatible deserialization.
    pub schema_version: u32,
    /// Owning agent.
    pub agent_id: String,
    /// Which memory store this belongs to.
    pub memory_type: MemoryType,
    /// Trust classification.
    pub epistemic_class: EpistemicClass,
    /// Current validation state.
    pub validation_state: ValidationState,

    // ── Content ──
    /// The actual memory payload.
    pub content: MemoryContent,
    /// Optional embedding vector for semantic search.
    pub embedding: Option<Vec<f32>>,

    // ── Temporal validity ──
    /// When this entry was created.
    pub created_at: DateTime<Utc>,
    /// When this entry was last modified.
    pub updated_at: DateTime<Utc>,
    /// Earliest time this fact is considered valid.
    pub valid_from: DateTime<Utc>,
    /// Latest time this fact is considered valid (`None` = forever).
    pub valid_to: Option<DateTime<Utc>>,

    // ── Trust and importance ──
    /// Trust score in `[0.0, 1.0]`.
    pub trust_score: f32,
    /// Importance score in `[0.0, 1.0]`.
    pub importance: f32,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f32,

    // ── Lineage ──
    /// Entry this one supersedes (if any).
    pub supersedes: Option<MemoryId>,
    /// Entries this one was derived from.
    pub derived_from: Vec<MemoryId>,
    /// Task that produced this entry.
    pub source_task_id: Option<String>,
    /// Conversation that produced this entry.
    pub source_conversation_id: Option<String>,

    // ── Scope and sensitivity ──
    /// Visibility boundary.
    pub scope: MemoryScope,
    /// Compliance classification.
    pub sensitivity: SensitivityClass,

    // ── Usage tracking ──
    /// How many times this entry has been read.
    pub access_count: u64,
    /// Last time this entry was accessed.
    pub last_accessed: DateTime<Utc>,
    /// Content version (incremented on update).
    pub version: u32,
    /// Time-to-live in seconds (`None` = no expiry).
    pub ttl: Option<i64>,

    // ── Tags ──
    /// Free-form tags for filtering.
    pub tags: Vec<String>,
}

impl MemoryEntry {
    /// Returns `true` if this entry has expired according to its TTL.
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let expires_at = self.created_at + chrono::Duration::seconds(ttl);
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Returns `true` if this entry is within its temporal validity window.
    pub fn is_temporally_valid(&self) -> bool {
        let now = Utc::now();
        if now < self.valid_from {
            return false;
        }
        if let Some(ref to) = self.valid_to {
            if now > *to {
                return false;
            }
        }
        true
    }
}

// ── Query ───────────────────────────────────────────────────────────────────

/// Query for retrieving memories.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryQuery {
    /// Filter by memory types.
    pub memory_types: Option<Vec<MemoryType>>,
    /// Filter by epistemic class.
    pub epistemic_filter: Option<Vec<EpistemicClassFilter>>,
    /// Minimum trust score.
    pub min_trust: Option<f32>,
    /// Minimum confidence score.
    pub min_confidence: Option<f32>,
    /// Filter by tags (any match).
    pub tags: Option<Vec<String>>,
    /// Filter by scope.
    pub scope: Option<MemoryScope>,
    /// Only entries created after this time.
    pub since: Option<DateTime<Utc>>,
    /// Maximum number of results.
    pub limit: Option<usize>,
    /// Whether to include expired entries.
    pub include_expired: bool,
    /// Filter by validation states.
    pub validation_states: Option<Vec<ValidationState>>,
}

/// Simplified filter for epistemic classes (since some variants carry data).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EpistemicClassFilter {
    Observation,
    UserAssertion,
    Inference,
    CachedRetrieval,
    Summary,
    LearnedBehavior,
    SharedKnowledge,
    Imported,
    SystemGenerated,
}

// ── Access permissions ──────────────────────────────────────────────────────

/// Memory access permissions for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccess {
    /// Memory types this agent may read.
    pub read: Vec<MemoryType>,
    /// Memory types this agent may write.
    pub write: Vec<MemoryType>,
    /// Whether this agent can search memory.
    pub search: bool,
    /// Whether this agent can share entries with others.
    pub share: bool,
}

// ── Statistics ───────────────────────────────────────────────────────────────

/// Memory usage statistics for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Agent identifier.
    pub agent_id: String,
    /// Working memory entry count.
    pub working_count: u64,
    /// Episodic memory entry count.
    pub episodic_count: u64,
    /// Semantic memory entry count.
    pub semantic_count: u64,
    /// Procedural memory entry count.
    pub procedural_count: u64,
    /// Approximate total size in bytes.
    pub total_size_bytes: u64,
}

// ── Audit ───────────────────────────────────────────────────────────────────

/// Audit log entry for memory operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAuditEntry {
    /// Unique audit entry identifier.
    pub id: Uuid,
    /// Agent whose memory was accessed.
    pub agent_id: String,
    /// Who performed the operation.
    pub accessor_id: String,
    /// What operation was performed.
    pub operation: MemoryOperation,
    /// Which memory type was affected.
    pub memory_type: MemoryType,
    /// Specific entry affected (if applicable).
    pub entry_id: Option<MemoryId>,
    /// When the operation occurred.
    pub timestamp: DateTime<Utc>,
    /// Additional human-readable details.
    pub details: Option<String>,
    /// SHA-256 hash chaining to previous entry.
    pub hash: String,
    /// Hash of the previous audit entry.
    pub previous_hash: Option<String>,
}

/// Operations that can be performed on memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryOperation {
    Read,
    Write,
    Update,
    SoftDelete,
    Search,
    Share,
    Rollback,
    GarbageCollect,
}

impl std::fmt::Display for MemoryOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for a memory space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum working-memory entries per agent.
    pub max_working_entries: usize,
    /// Maximum episodic-memory entries per agent.
    pub max_episodic_entries: usize,
    /// Maximum semantic-memory entries per agent.
    pub max_semantic_entries: usize,
    /// Maximum procedural-memory entries per agent.
    pub max_procedural_entries: usize,
    /// Default TTL for working memory in seconds.
    pub default_ttl_working_secs: Option<i64>,
    /// Whether audit logging is enabled (should always be `true` in production).
    pub enable_audit: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_working_entries: 1000,
            max_episodic_entries: 50_000,
            max_semantic_entries: 10_000,
            max_procedural_entries: 500,
            default_ttl_working_secs: Some(3600),
            enable_audit: true,
        }
    }
}

// ── Procedural memory types ──────────────────────────────────────────────────

/// Evidence required before a behavior becomes a reusable procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionEvidence {
    /// Task IDs where this behavior was observed succeeding.
    pub evidence_task_ids: Vec<String>,
    /// Number of successful executions observed.
    pub success_count: u32,
    /// Total executions observed (successes + failures).
    pub total_count: u32,
    /// Computed success rate (`success_count / total_count`).
    pub success_rate: f32,
    /// Start of the evidence collection window.
    pub evidence_window_start: DateTime<Utc>,
    /// End of the evidence collection window.
    pub evidence_window_end: DateTime<Utc>,
    /// Who approved the promotion.
    pub approved_by: PromotionApprover,
    /// When promotion was approved.
    pub promoted_at: DateTime<Utc>,
}

/// Who approved a procedure promotion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromotionApprover {
    /// Met evidence threshold automatically.
    Automatic { autonomy_level: u8 },
    /// Human approved via HITL.
    HumanApproved { approver_id: String },
    /// Darwin Core promoted during evolution.
    EvolutionPromoted { generation: u32, fitness_score: f32 },
    /// Shipped with agent genome.
    SystemDefault,
}

/// Promotion thresholds per autonomy level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionThresholds {
    pub min_successes: u32,
    pub min_success_rate: f32,
    pub min_evidence_window_hours: u64,
}

impl PromotionThresholds {
    /// Returns thresholds for the given autonomy level.
    /// L0–L2 cannot self-promote (returns `None`).
    pub fn for_autonomy_level(level: u8) -> Option<Self> {
        match level {
            0..=2 => None,
            3 => Some(Self {
                min_successes: 10,
                min_success_rate: 0.9,
                min_evidence_window_hours: 48,
            }),
            4..=5 => Some(Self {
                min_successes: 5,
                min_success_rate: 0.8,
                min_evidence_window_hours: 24,
            }),
            6.. => Some(Self {
                min_successes: 3,
                min_success_rate: 0.7,
                min_evidence_window_hours: 12,
            }),
        }
    }
}

/// Tracking execution outcomes for a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureExecution {
    /// The procedure that was executed.
    pub procedure_id: MemoryId,
    /// The task in which execution occurred.
    pub task_id: String,
    /// When execution happened.
    pub executed_at: DateTime<Utc>,
    /// What happened.
    pub outcome: Outcome,
    /// How long it took.
    pub duration_ms: u64,
}

/// State machine for procedural memory entries.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcedureState {
    /// Observed pattern, not yet promoted.
    Candidate,
    /// Met evidence threshold, promoted to active procedure.
    Promoted,
    /// Currently in use by agent.
    Active,
    /// Success rate dropped, flagged for review.
    Flagged,
    /// Demoted back to episodic memory.
    Demoted,
    /// Archived (no longer used but preserved for history).
    Archived,
}

impl std::fmt::Display for ProcedureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A candidate procedure being tracked before promotion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureCandidate {
    /// Unique identifier.
    pub id: Uuid,
    /// Owning agent.
    pub agent_id: String,
    /// Procedure name.
    pub name: String,
    /// What the procedure does.
    pub description: String,
    /// When to trigger this procedure.
    pub trigger_condition: String,
    /// The steps.
    pub steps: Vec<ProcedureStep>,
    /// Recorded executions.
    pub executions: Vec<ProcedureExecution>,
    /// When the candidate was first observed.
    pub created_at: DateTime<Utc>,
    /// Current state.
    pub state: ProcedureState,
}

/// Regression event — when a procedure starts failing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionEvent {
    /// The failing procedure.
    pub procedure_id: MemoryId,
    /// Owning agent.
    pub agent_id: String,
    /// What triggered the regression check.
    pub trigger: RegressionTrigger,
    /// Success rate when triggered.
    pub success_rate_at_trigger: f32,
    /// Recent execution evidence.
    pub recent_executions: Vec<ProcedureExecution>,
    /// What action was taken.
    pub action_taken: RegressionAction,
    /// When this was detected.
    pub timestamp: DateTime<Utc>,
}

/// What triggered a regression check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegressionTrigger {
    /// Success rate dropped below the flag threshold (60%).
    SuccessRateDroppedBelowFlag,
    /// Success rate dropped below the demote threshold (40%).
    SuccessRateDroppedBelowDemote,
    /// Not used for an extended period.
    StaleUnused { days_since_last_use: u64 },
}

/// What action was taken in response to a regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegressionAction {
    /// Flagged for review — still usable but monitored.
    Flagged,
    /// Demoted — no longer auto-triggered.
    Demoted { reason: String },
    /// No action — regression within acceptable bounds.
    NoAction { reason: String },
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Error types for the memory subsystem.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    /// Agent memory space does not exist.
    #[error("Agent memory space not found: {0}")]
    SpaceNotFound(String),

    /// A specific memory entry does not exist.
    #[error("Memory entry not found: {0}")]
    EntryNotFound(MemoryId),

    /// The caller lacks the required permission.
    #[error("Access denied: {agent_id} cannot {operation} on {memory_type:?}")]
    AccessDenied {
        agent_id: String,
        operation: String,
        memory_type: MemoryType,
    },

    /// The agent has exceeded its memory quota.
    #[error("Quota exceeded for {agent_id}: {memory_type:?} has {current}/{max} entries")]
    QuotaExceeded {
        agent_id: String,
        memory_type: MemoryType,
        current: usize,
        max: usize,
    },

    /// An error in the persistence layer.
    #[error("Persistence error: {0}")]
    PersistenceError(String),

    /// A validation error in the memory entry.
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// A sensitivity/compliance violation.
    #[error("Sensitivity violation: {0}")]
    SensitivityViolation(String),

    /// Content type does not match declared memory type.
    #[error("Type mismatch: content is {content_type} but memory_type is {declared_type}")]
    TypeMismatch {
        content_type: MemoryType,
        declared_type: MemoryType,
    },

    /// A procedure candidate was not found.
    #[error("Procedure candidate not found: {0}")]
    CandidateNotFound(Uuid),

    /// A memory checkpoint was not found.
    #[error("Memory checkpoint not found: {0}")]
    CheckpointNotFound(Uuid),
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_type_display() {
        assert_eq!(MemoryType::Working.to_string(), "Working");
        assert_eq!(MemoryType::Episodic.to_string(), "Episodic");
        assert_eq!(MemoryType::Semantic.to_string(), "Semantic");
        assert_eq!(MemoryType::Procedural.to_string(), "Procedural");
    }

    #[test]
    fn epistemic_class_default_trust_ordering() {
        let obs = EpistemicClass::Observation.default_trust();
        let user = EpistemicClass::UserAssertion.default_trust();
        let sys = EpistemicClass::SystemGenerated.default_trust();
        let inf = EpistemicClass::Inference {
            derived_from: vec![],
        }
        .default_trust();
        let cached = EpistemicClass::CachedRetrieval {
            source_url: None,
            retrieved_at: Utc::now(),
        }
        .default_trust();

        assert!(obs > user, "Observation should be highest trust");
        assert!(user > sys, "UserAssertion > SystemGenerated");
        assert!(sys > inf, "SystemGenerated > Inference");
        assert!(inf > cached, "Inference > CachedRetrieval");
    }

    #[test]
    fn epistemic_class_to_filter_roundtrip() {
        let classes = vec![
            EpistemicClass::Observation,
            EpistemicClass::UserAssertion,
            EpistemicClass::Inference {
                derived_from: vec![],
            },
            EpistemicClass::SystemGenerated,
        ];
        let expected = vec![
            EpistemicClassFilter::Observation,
            EpistemicClassFilter::UserAssertion,
            EpistemicClassFilter::Inference,
            EpistemicClassFilter::SystemGenerated,
        ];
        for (cls, exp) in classes.iter().zip(expected.iter()) {
            assert_eq!(&cls.to_filter(), exp);
        }
    }

    #[test]
    fn memory_content_expected_type() {
        assert_eq!(
            MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null
            }
            .expected_memory_type(),
            MemoryType::Working
        );
        assert_eq!(
            MemoryContent::Episode {
                event_type: EpisodeType::Conversation,
                summary: String::new(),
                details: serde_json::Value::Null,
                outcome: None,
                duration_ms: None,
            }
            .expected_memory_type(),
            MemoryType::Episodic
        );
        assert_eq!(
            MemoryContent::Triple {
                subject: String::new(),
                predicate: String::new(),
                object: String::new(),
            }
            .expected_memory_type(),
            MemoryType::Semantic
        );
        assert_eq!(
            MemoryContent::Procedure {
                name: String::new(),
                description: String::new(),
                trigger_condition: String::new(),
                steps: vec![],
            }
            .expected_memory_type(),
            MemoryType::Procedural
        );
    }

    #[test]
    fn serde_roundtrip_memory_entry() {
        let entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Working,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: "task_goal".into(),
                value: serde_json::json!("build something"),
            },
            embedding: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            valid_from: Utc::now(),
            valid_to: None,
            trust_score: 0.95,
            importance: 0.8,
            confidence: 0.9,
            supersedes: None,
            derived_from: vec![],
            source_task_id: Some("task-1".into()),
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: Utc::now(),
            version: 1,
            ttl: Some(3600),
            tags: vec!["test".into()],
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        let deser: MemoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, entry.id);
        assert_eq!(deser.agent_id, entry.agent_id);
        assert_eq!(deser.memory_type, entry.memory_type);
        assert_eq!(deser.trust_score, entry.trust_score);
    }

    #[test]
    fn serde_roundtrip_epistemic_class_shared_knowledge() {
        let cls = EpistemicClass::SharedKnowledge {
            source_agent_id: "agent-2".into(),
            original_class: Box::new(EpistemicClass::Observation),
        };
        let json = serde_json::to_string(&cls).expect("serialize");
        let deser: EpistemicClass = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cls, deser);
    }

    #[test]
    fn serde_roundtrip_memory_query() {
        let q = MemoryQuery {
            memory_types: Some(vec![MemoryType::Working, MemoryType::Episodic]),
            min_trust: Some(0.5),
            limit: Some(10),
            include_expired: false,
            ..Default::default()
        };
        let json = serde_json::to_string(&q).expect("serialize");
        let deser: MemoryQuery = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.min_trust, Some(0.5));
        assert_eq!(deser.limit, Some(10));
    }

    #[test]
    fn serde_roundtrip_audit_entry() {
        let entry = MemoryAuditEntry {
            id: Uuid::new_v4(),
            agent_id: "agent-1".into(),
            accessor_id: "system".into(),
            operation: MemoryOperation::Write,
            memory_type: MemoryType::Episodic,
            entry_id: Some(Uuid::new_v4()),
            timestamp: Utc::now(),
            details: Some("test write".into()),
            hash: "abc123".into(),
            previous_hash: None,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let deser: MemoryAuditEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, entry.id);
        assert_eq!(deser.operation, MemoryOperation::Write);
    }

    #[test]
    fn memory_config_defaults() {
        let cfg = MemoryConfig::default();
        assert_eq!(cfg.max_working_entries, 1000);
        assert_eq!(cfg.max_episodic_entries, 50_000);
        assert!(cfg.enable_audit);
    }

    #[test]
    fn sensitivity_ordering() {
        assert!(SensitivityClass::Public < SensitivityClass::Internal);
        assert!(SensitivityClass::Internal < SensitivityClass::Sensitive);
        assert!(SensitivityClass::Sensitive < SensitivityClass::Restricted);
    }

    #[test]
    fn memory_entry_expiry() {
        let mut entry = MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "a".into(),
            memory_type: MemoryType::Working,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Context {
                key: "k".into(),
                value: serde_json::Value::Null,
            },
            embedding: None,
            created_at: Utc::now() - chrono::Duration::seconds(7200),
            updated_at: Utc::now(),
            valid_from: Utc::now() - chrono::Duration::seconds(7200),
            valid_to: None,
            trust_score: 0.5,
            importance: 0.5,
            confidence: 0.5,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: Utc::now(),
            version: 1,
            ttl: Some(3600), // 1 hour TTL, created 2 hours ago
            tags: vec![],
        };
        assert!(
            entry.is_expired(),
            "Entry with 1h TTL created 2h ago should be expired"
        );

        entry.ttl = None;
        assert!(!entry.is_expired(), "Entry with no TTL should not expire");
    }
}
