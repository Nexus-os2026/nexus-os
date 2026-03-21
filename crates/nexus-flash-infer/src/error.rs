use thiserror::Error;

/// Errors from the flash inference engine.
#[derive(Debug, Error)]
pub enum FlashError {
    #[error(
        "model too large: needs {model_min_mb}MB, only {available_mb}MB available. {suggestion}"
    )]
    ModelTooLarge {
        model_min_mb: u64,
        available_mb: u64,
        suggestion: String,
    },

    #[error("memory budget exceeded: requested {requested_mb}MB, remaining {remaining_mb}MB")]
    BudgetExceeded {
        requested_mb: u64,
        remaining_mb: u64,
    },

    #[error("backend not found for format {0:?}")]
    NoBackendForFormat(crate::backend::ModelFormat),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session limit reached: max {max} concurrent sessions")]
    SessionLimitReached { max: usize },

    #[error("model not found in catalog: {0}")]
    ModelNotInCatalog(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("backend error: {0}")]
    BackendError(String),

    #[error("llama bridge error: {0}")]
    LlamaBridge(#[from] nexus_llama_bridge::LlamaError),

    #[error("download error: {0}")]
    DownloadError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
