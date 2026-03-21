//! Core types for the background agent scheduler.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a schedule entry.
pub type ScheduleId = Uuid;

/// A registered schedule entry — binds a trigger to an agent task with governance parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub id: ScheduleId,
    pub agent_did: String,
    pub name: String,
    pub description: String,
    pub trigger: TriggerType,
    pub task: ScheduledTask,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub run_count: u64,
    /// `None` = unlimited runs.
    pub max_runs: Option<u64>,
    pub max_fuel_per_run: u64,
    /// Force HITL approval even if the agent would not normally require it.
    pub requires_hitl: bool,
    pub on_failure: FailurePolicy,
}

/// What causes a schedule to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    /// Standard five-field cron expression (e.g. `*/5 * * * *`).
    Cron {
        expression: String,
        timezone: String,
    },
    /// Incoming HTTP webhook.
    Webhook {
        path: String,
        secret: Option<String>,
        filter: Option<String>,
    },
    /// Internal kernel event.
    Event {
        event_kind: EventKind,
        filter: Option<String>,
    },
    /// Simple recurring interval.
    Interval { seconds: u64 },
    /// Run exactly once at a specific time.
    OneShot { at: DateTime<Utc> },
}

/// Internal events that can trigger a scheduled task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventKind {
    FileChanged { path: String },
    FuelBelowThreshold { threshold_percent: f64 },
    AuditAnomaly,
    AgentCompleted { agent_did: String },
    IntegrationReceived { provider: String },
    GenomeEvolved { genome_id: String },
    Custom { name: String },
}

/// The task to execute when a trigger fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// One of: `run_agent`, `send_notification`, `execute_command`.
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: u64,
}

/// What to do when a scheduled execution fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailurePolicy {
    Ignore,
    Retry {
        max_attempts: u32,
        backoff_seconds: u64,
    },
    Disable,
    Alert {
        channel: String,
    },
}

impl ScheduleEntry {
    /// Create a new schedule entry with sensible defaults.
    pub fn new(agent_did: String, name: String, trigger: TriggerType, task: ScheduledTask) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_did,
            name,
            description: String::new(),
            trigger,
            task,
            enabled: true,
            created_at: Utc::now(),
            last_run: None,
            next_run: None,
            run_count: 0,
            max_runs: None,
            max_fuel_per_run: 5_000,
            requires_hitl: false,
            on_failure: FailurePolicy::Ignore,
        }
    }
}
