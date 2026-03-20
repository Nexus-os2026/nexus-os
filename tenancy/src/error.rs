//! Multi-tenancy error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TenancyError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("workspace already exists: {0}")]
    WorkspaceAlreadyExists(String),

    #[error("member not found: user '{user_id}' in workspace '{workspace_id}'")]
    MemberNotFound {
        workspace_id: String,
        user_id: String,
    },

    #[error("member already exists: user '{user_id}' in workspace '{workspace_id}'")]
    MemberAlreadyExists {
        workspace_id: String,
        user_id: String,
    },

    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("agent limit reached: workspace '{workspace_id}' has {current}/{limit} agents")]
    AgentLimitReached {
        workspace_id: String,
        current: u32,
        limit: u32,
    },

    #[error("fuel budget exhausted: workspace '{workspace_id}' used {used}/{budget} today")]
    FuelBudgetExhausted {
        workspace_id: String,
        used: u64,
        budget: u64,
    },

    #[error(
        "autonomy level {requested} exceeds workspace max {max} for workspace '{workspace_id}'"
    )]
    AutonomyLevelExceeded {
        workspace_id: String,
        requested: u8,
        max: u8,
    },

    #[error("provider '{provider}' not allowed in workspace '{workspace_id}'")]
    ProviderNotAllowed {
        workspace_id: String,
        provider: String,
    },

    #[error("cannot remove last admin from workspace '{0}'")]
    LastAdmin(String),

    #[error("invalid policy: {0}")]
    InvalidPolicy(String),
}
