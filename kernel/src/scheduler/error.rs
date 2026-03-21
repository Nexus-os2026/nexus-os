//! Scheduler error types.

/// Errors that can occur during schedule management or execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SchedulerError {
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("no next fire time for expression: {0}")]
    NoNextFire(String),

    #[error("unknown webhook path: {0}")]
    UnknownWebhook(String),

    #[error("missing HMAC signature")]
    MissingSignature,

    #[error("invalid HMAC signature")]
    InvalidSignature,

    #[error("channel closed: {0}")]
    ChannelClosed(String),

    #[error("capability denied for agent: {0}")]
    CapabilityDenied(String),

    #[error("insufficient fuel: {0}")]
    InsufficientFuel(String),

    #[error("adversarial check blocked: {0}")]
    AdversarialBlock(String),

    #[error("missing parameter: {0}")]
    MissingParam(String),

    #[error("unknown task type: {0}")]
    UnknownTaskType(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("execution timeout after {0}s")]
    Timeout(u64),

    #[error("schedule not found: {0}")]
    NotFound(String),

    #[error("HITL approval required for scheduled task: {0}")]
    HitlRequired(String),
}
