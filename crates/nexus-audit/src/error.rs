use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Failed to write audit entry: {0}")]
    WriteError(#[from] std::io::Error),
    #[error("Encryption failed: {0}")]
    EncryptionError(String),
    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),
}
