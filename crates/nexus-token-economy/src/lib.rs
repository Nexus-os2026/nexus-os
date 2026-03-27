pub mod coin;
pub mod compute_pricing;
pub mod delegation;
pub mod fiat_interface;
pub mod gating;
pub mod genesis_economy;
pub mod ledger;
pub mod rewards;
pub mod supply;
pub mod tauri_commands;
pub mod wallet;

pub use coin::{NexusCoin, TransactionType};
pub use compute_pricing::ComputePricingTable;
pub use delegation::{Delegation, DelegationManager, DelegationOutcome, DelegationStatus};
pub use gating::ExecutionGate;
pub use genesis_economy::{GenesisEconomy, SpawnCalculation};
pub use ledger::{EconomyLedger, LedgerEntry};
pub use rewards::{RewardCalculation, RewardEngine};
pub use supply::{SupplyMetrics, SupplySnapshot};
pub use tauri_commands::EconomyState;
pub use wallet::AgentWallet;

/// Errors for the token economy
#[derive(Debug, thiserror::Error)]
pub enum EconomyError {
    #[error("Insufficient balance: requested {requested}, available {available}")]
    InsufficientBalance {
        requested: NexusCoin,
        available: NexusCoin,
    },
    #[error("Arithmetic overflow")]
    Overflow,
    #[error("Escrow error: {0}")]
    EscrowError(String),
    #[error("Integrity violation: {0}")]
    IntegrityViolation(String),
    #[error("Gating denied: {0}")]
    GatingDenied(String),
    #[error("Delegation error: {0}")]
    DelegationError(String),
}
