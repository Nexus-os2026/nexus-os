use serde::{Deserialize, Serialize};

use crate::coin::NexusCoin;
use crate::delegation::{DelegationManager, DelegationStatus};
use crate::ledger::EconomyLedger;
use crate::wallet::AgentWallet;

/// Tracks the overall economy health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyMetrics {
    /// Snapshots taken at regular intervals
    pub snapshots: Vec<SupplySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplySnapshot {
    pub timestamp: u64,
    pub total_supply: NexusCoin,
    pub total_burned: NexusCoin,
    pub total_minted: NexusCoin,
    pub active_wallets: usize,
    pub active_delegations: usize,
    pub total_escrowed: NexusCoin,
    /// Net flow: minted - burned since last snapshot
    pub net_flow: i64, // Can be negative (deflationary)
}

impl SupplyMetrics {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    pub fn record_snapshot(
        &mut self,
        ledger: &EconomyLedger,
        wallets: &[AgentWallet],
        delegations: &DelegationManager,
    ) {
        let total_escrowed =
            NexusCoin::from_micro(wallets.iter().map(|w| w.escrowed.micro()).sum());
        let active_delegations = delegations
            .delegations()
            .iter()
            .filter(|d| {
                d.status == DelegationStatus::InProgress || d.status == DelegationStatus::Pending
            })
            .count();

        let net_flow = if let Some(last) = self.snapshots.last() {
            ledger.total_minted().micro() as i64
                - last.total_minted.micro() as i64
                - (ledger.total_burned().micro() as i64 - last.total_burned.micro() as i64)
        } else {
            ledger.total_minted().micro() as i64 - ledger.total_burned().micro() as i64
        };

        self.snapshots.push(SupplySnapshot {
            timestamp: epoch_now(),
            total_supply: ledger.total_supply(),
            total_burned: ledger.total_burned(),
            total_minted: ledger.total_minted(),
            active_wallets: wallets.len(),
            active_delegations,
            total_escrowed,
            net_flow,
        });
    }

    /// Is the economy deflationary? (burning more than minting)
    pub fn is_deflationary(&self) -> bool {
        self.snapshots
            .last()
            .map(|s| s.net_flow < 0)
            .unwrap_or(false)
    }
}

impl Default for SupplyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
