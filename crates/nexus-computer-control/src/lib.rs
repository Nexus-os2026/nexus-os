pub mod actions;
pub mod audit;
pub mod engine;
pub mod governance;
pub mod tauri_commands;

pub use actions::{ComputerAction, MouseButton, ScreenRegion};
pub use audit::{ActionAuditEntry, ControlAuditTrail};
pub use engine::{
    ActionResult, ComputerControlBudget, GovernedControlEngine, ScreenContext, VerificationResult,
};
pub use governance::{
    check_governance, is_command_allowed, minimum_autonomy_level, required_capability, token_cost,
};
pub use tauri_commands::ControlState;

/// Errors for the computer control engine.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("Governance denied: {0}")]
    GovernanceDenied(String),
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Timeout: action took longer than {0}ms")]
    Timeout(u64),
}
