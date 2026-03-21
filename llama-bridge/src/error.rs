/// Errors from the llama.cpp bridge layer.
#[derive(Debug, thiserror::Error)]
pub enum LlamaError {
    #[error("Failed to load model from {path}: {reason}")]
    ModelLoadFailed { path: String, reason: String },

    #[error("Failed to create context: {0}")]
    ContextCreationFailed(String),

    #[error("Tokenization failed: {0}")]
    TokenizationFailed(String),

    #[error("Decode failed with code {0}")]
    DecodeFailed(i32),

    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Context not initialized")]
    ContextNotInitialized,

    #[error("Memory budget exceeded: need {needed_mb}MB, have {available_mb}MB")]
    MemoryBudgetExceeded { needed_mb: u64, available_mb: u64 },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Generation cancelled")]
    Cancelled,

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),
}
