//! Core types for the actuator subsystem — governed real-world action execution.

use crate::autonomy::AutonomyLevel;
use crate::cognitive::types::PlannedAction;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// SideEffect — observable consequences of an action
// ---------------------------------------------------------------------------

/// Observable side effects produced by an actuator execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SideEffect {
    FileCreated { path: PathBuf },
    FileModified { path: PathBuf },
    FileDeleted { path: PathBuf },
    CommandExecuted { command: String },
    HttpRequest { url: String },
    MessageSent { target: String },
}

// ---------------------------------------------------------------------------
// ActionResult — outcome of an actuator execution
// ---------------------------------------------------------------------------

/// Result returned by an actuator after executing an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub output: String,
    pub fuel_cost: f64,
    pub side_effects: Vec<SideEffect>,
}

// ---------------------------------------------------------------------------
// ActuatorContext — per-invocation context
// ---------------------------------------------------------------------------

/// Context provided to an actuator for each action execution.
#[derive(Clone)]
pub struct ActuatorContext {
    pub agent_id: String,
    pub agent_name: String,
    pub working_dir: PathBuf,
    pub autonomy_level: AutonomyLevel,
    pub capabilities: HashSet<String>,
    pub fuel_remaining: f64,
    pub egress_allowlist: Vec<String>,
    pub action_review_engine: Option<Arc<dyn ActionReviewEngine>>,
}

impl std::fmt::Debug for ActuatorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActuatorContext")
            .field("agent_id", &self.agent_id)
            .field("agent_name", &self.agent_name)
            .field("working_dir", &self.working_dir)
            .field("autonomy_level", &self.autonomy_level)
            .field("capabilities", &self.capabilities)
            .field("fuel_remaining", &self.fuel_remaining)
            .field("egress_allowlist", &self.egress_allowlist)
            .field(
                "action_review_engine",
                &self
                    .action_review_engine
                    .as_ref()
                    .map(|_| "<dyn ActionReviewEngine>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionReviewDecision {
    Allow { reason: String },
    Deny { reason: String },
}

pub trait ActionReviewEngine: Send + Sync {
    fn review(
        &self,
        actor_agent_id: &str,
        actor_name: &str,
        action: &PlannedAction,
    ) -> Result<ActionReviewDecision, String>;
}

// ---------------------------------------------------------------------------
// Actuator trait
// ---------------------------------------------------------------------------

/// Trait that all governed actuators must implement.
///
/// Each actuator handles one category of real-world actions (filesystem, shell,
/// web, API) and enforces security policies specific to that domain.
pub trait Actuator: Send + Sync {
    /// Human-readable name for audit/logging.
    fn name(&self) -> &str;

    /// Capability strings required to use this actuator.
    fn required_capabilities(&self) -> Vec<String>;

    /// Execute the given action within the provided governance context.
    fn execute(
        &self,
        action: &crate::cognitive::types::PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError>;
}

// ---------------------------------------------------------------------------
// ActuatorError
// ---------------------------------------------------------------------------

/// Errors that can occur during actuator execution.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ActuatorError {
    #[error("capability denied: '{0}'")]
    CapabilityDenied(String),

    #[error("path traversal blocked: {0}")]
    PathTraversal(String),

    #[error("blocked file extension: {0}")]
    BlockedExtension(String),

    #[error("file too large: {size} bytes (max {max} bytes)")]
    FileTooLarge { size: u64, max: u64 },

    #[error("command blocked: {0}")]
    CommandBlocked(String),

    #[error("command timed out after {seconds}s")]
    CommandTimeout { seconds: u64 },

    #[error("egress denied: {0}")]
    EgressDenied(String),

    #[error("invalid method: {0}")]
    InvalidMethod(String),

    #[error("body too large: {size} bytes (max {max} bytes)")]
    BodyTooLarge { size: u64, max: u64 },

    #[error("human approval required: {0}")]
    HumanApprovalRequired(String),

    #[error("governance review failed: {0}")]
    GovernanceReviewFailed(String),

    #[error("action not handled by this actuator")]
    ActionNotHandled,

    #[error("io error: {0}")]
    IoError(String),

    #[error("insufficient fuel: need {needed}, have {available}")]
    InsufficientFuel { needed: f64, available: f64 },
}
