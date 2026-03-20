//! Event types that trigger integrations and shared message structs.

use serde::{Deserialize, Serialize};

/// Events emitted by Nexus OS that can be routed to integrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NexusEvent {
    AgentStarted {
        did: String,
        workspace: String,
    },
    AgentCompleted {
        did: String,
        result: String,
    },
    AgentError {
        did: String,
        error: String,
    },
    HitlRequired {
        did: String,
        action: String,
        context: String,
    },
    HitlDecision {
        did: String,
        decision: String,
    },
    SecurityEvent {
        event_type: String,
        details: String,
    },
    FuelExhausted {
        did: String,
    },
    GenomeEvolved {
        genome_id: String,
        generation: u32,
    },
    AuditChainBreak {
        details: String,
    },
    BackupCompleted {
        path: String,
        size_bytes: u64,
    },
    SystemAlert {
        severity: String,
        message: String,
    },
}

impl NexusEvent {
    /// Returns the event kind string used for matching against config filters.
    pub fn kind(&self) -> &str {
        match self {
            Self::AgentStarted { .. } => "agent_started",
            Self::AgentCompleted { .. } => "agent_completed",
            Self::AgentError { .. } => "agent_error",
            Self::HitlRequired { .. } => "hitl_required",
            Self::HitlDecision { .. } => "hitl_decision",
            Self::SecurityEvent { .. } => "security_event",
            Self::FuelExhausted { .. } => "fuel_exhausted",
            Self::GenomeEvolved { .. } => "genome_evolved",
            Self::AuditChainBreak { .. } => "audit_chain_break",
            Self::BackupCompleted { .. } => "backup_completed",
            Self::SystemAlert { .. } => "system_alert",
        }
    }

    /// Human-readable summary for notification bodies.
    pub fn summary(&self) -> String {
        match self {
            Self::AgentStarted { did, workspace } => {
                format!("Agent {did} started in workspace {workspace}")
            }
            Self::AgentCompleted { did, result } => {
                format!("Agent {did} completed: {result}")
            }
            Self::AgentError { did, error } => {
                format!("Agent {did} error: {error}")
            }
            Self::HitlRequired { did, action, .. } => {
                format!("HITL approval required: agent {did} wants to {action}")
            }
            Self::HitlDecision { did, decision } => {
                format!("HITL decision for {did}: {decision}")
            }
            Self::SecurityEvent {
                event_type,
                details,
            } => {
                format!("Security event [{event_type}]: {details}")
            }
            Self::FuelExhausted { did } => {
                format!("Agent {did} fuel exhausted")
            }
            Self::GenomeEvolved {
                genome_id,
                generation,
            } => {
                format!("Genome {genome_id} evolved to generation {generation}")
            }
            Self::AuditChainBreak { details } => {
                format!("AUDIT CHAIN BREAK: {details}")
            }
            Self::BackupCompleted { path, size_bytes } => {
                format!("Backup completed: {path} ({size_bytes} bytes)")
            }
            Self::SystemAlert { severity, message } => {
                format!("[{severity}] {message}")
            }
        }
    }

    /// Severity level for color-coding notifications.
    pub fn severity(&self) -> Severity {
        match self {
            Self::AgentError { .. } | Self::SecurityEvent { .. } | Self::AuditChainBreak { .. } => {
                Severity::Critical
            }
            Self::HitlRequired { .. } | Self::FuelExhausted { .. } => Severity::Warning,
            _ => Severity::Info,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// A notification message destined for a messaging platform (Slack, Teams, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub severity: Severity,
    pub channel: Option<String>,
    pub source_event: String,
}

/// Request to create a ticket in a project tracker (Jira, ServiceNow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketRequest {
    pub title: String,
    pub description: String,
    pub project: String,
    pub issue_type: String,
    pub priority: String,
    pub labels: Vec<String>,
}

/// Response from a ticket creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketResponse {
    pub ticket_id: String,
    pub url: String,
    pub status: String,
}

/// Status update to push to integrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub resource_id: String,
    pub status: String,
    pub message: String,
}
