use thiserror::Error;

/// Errors from the flash inference engine.
#[derive(Debug, Error)]
pub enum FlashError {
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
