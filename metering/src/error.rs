//! Metering error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MeteringError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("budget exceeded: workspace={workspace_id} metric={metric} current={current} threshold={threshold}")]
    BudgetExceeded {
        workspace_id: String,
        metric: String,
        current: f64,
        threshold: f64,
    },
}
