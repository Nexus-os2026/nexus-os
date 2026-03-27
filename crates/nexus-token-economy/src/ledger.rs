use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::coin::{NexusCoin, TransactionType};

/// The economy ledger — hash-chained record of all transactions.
/// Same pattern as DecisionAuditLog in the Governance Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyLedger {
    entries: Vec<LedgerEntry>,
    latest_hash: String,
    /// Running total supply (all minted minus all burned)
    total_supply: NexusCoin,
    /// Running total burned
    total_burned: NexusCoin,
    /// Running total minted
    total_minted: NexusCoin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub entry_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub transaction_type: TransactionType,
    pub amount: NexusCoin,
    /// For burns: true. For mints: false. For transfers: N/A
    pub is_burn: bool,
    /// Balance after this transaction
    pub balance_after: NexusCoin,
    pub previous_hash: String,
    pub entry_hash: String,
}

impl EconomyLedger {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest_hash: "genesis-economy".to_string(),
            total_supply: NexusCoin::ZERO,
            total_burned: NexusCoin::ZERO,
            total_minted: NexusCoin::ZERO,
        }
    }

    /// Record a transaction
    pub fn record(
        &mut self,
        agent_id: &str,
        transaction_type: TransactionType,
        amount: NexusCoin,
        is_burn: bool,
        balance_after: NexusCoin,
    ) {
        let mut hasher = Sha256::new();
        hasher.update(self.latest_hash.as_bytes());
        hasher.update(agent_id.as_bytes());
        hasher.update(amount.micro().to_le_bytes());
        hasher.update(if is_burn {
            b"burn".as_slice()
        } else {
            b"credit".as_slice()
        });
        let entry_hash = format!("{:x}", hasher.finalize());

        // Update supply tracking
        if is_burn {
            self.total_burned = self
                .total_burned
                .checked_add(amount)
                .unwrap_or(self.total_burned);
            self.total_supply = self
                .total_supply
                .checked_sub(amount)
                .unwrap_or(NexusCoin::ZERO);
        } else {
            match &transaction_type {
                TransactionType::GovernanceMint { .. } | TransactionType::TaskReward { .. } => {
                    self.total_minted = self
                        .total_minted
                        .checked_add(amount)
                        .unwrap_or(self.total_minted);
                    self.total_supply = self
                        .total_supply
                        .checked_add(amount)
                        .unwrap_or(self.total_supply);
                }
                // Transfers don't change total supply
                _ => {}
            }
        }

        let entry = LedgerEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            agent_id: agent_id.to_string(),
            transaction_type,
            amount,
            is_burn,
            balance_after,
            previous_hash: self.latest_hash.clone(),
            entry_hash: entry_hash.clone(),
        };

        self.latest_hash = entry_hash;
        self.entries.push(entry);
    }

    /// Verify the entire chain integrity
    pub fn verify_chain(&self) -> Result<(), String> {
        let mut expected_prev = "genesis-economy".to_string();

        for entry in &self.entries {
            if entry.previous_hash != expected_prev {
                return Err(format!("Chain broken at {}", entry.entry_id));
            }
            let mut hasher = Sha256::new();
            hasher.update(entry.previous_hash.as_bytes());
            hasher.update(entry.agent_id.as_bytes());
            hasher.update(entry.amount.micro().to_le_bytes());
            hasher.update(if entry.is_burn {
                b"burn".as_slice()
            } else {
                b"credit".as_slice()
            });
            let computed = format!("{:x}", hasher.finalize());
            if computed != entry.entry_hash {
                return Err(format!("Hash mismatch at {}", entry.entry_id));
            }
            expected_prev = entry.entry_hash.clone();
        }
        Ok(())
    }

    pub fn total_supply(&self) -> NexusCoin {
        self.total_supply
    }
    pub fn total_burned(&self) -> NexusCoin {
        self.total_burned
    }
    pub fn total_minted(&self) -> NexusCoin {
        self.total_minted
    }
    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for EconomyLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ledger_chain_integrity() {
        let mut ledger = EconomyLedger::new();

        ledger.record(
            "agent-1",
            TransactionType::GovernanceMint {
                reason: "initial".into(),
            },
            NexusCoin::from_coins(100),
            false,
            NexusCoin::from_coins(100),
        );
        ledger.record(
            "agent-1",
            TransactionType::ComputeBurn {
                model_id: "flash-2b".into(),
                tokens_used: 1000,
            },
            NexusCoin::from_coins(1),
            true,
            NexusCoin::from_coins(99),
        );
        ledger.record(
            "agent-2",
            TransactionType::GovernanceMint {
                reason: "spawn".into(),
            },
            NexusCoin::from_coins(50),
            false,
            NexusCoin::from_coins(50),
        );

        assert!(ledger.verify_chain().is_ok());
        assert_eq!(ledger.len(), 3);
    }

    #[test]
    fn test_ledger_tamper_detection() {
        let mut ledger = EconomyLedger::new();

        ledger.record(
            "agent-1",
            TransactionType::GovernanceMint {
                reason: "initial".into(),
            },
            NexusCoin::from_coins(100),
            false,
            NexusCoin::from_coins(100),
        );
        ledger.record(
            "agent-1",
            TransactionType::ComputeBurn {
                model_id: "flash-2b".into(),
                tokens_used: 1000,
            },
            NexusCoin::from_coins(1),
            true,
            NexusCoin::from_coins(99),
        );

        // Tamper with the first entry
        ledger.entries[0].amount = NexusCoin::from_coins(999);

        assert!(ledger.verify_chain().is_err());
    }

    #[test]
    fn test_ledger_supply_tracking() {
        let mut ledger = EconomyLedger::new();

        ledger.record(
            "agent-1",
            TransactionType::GovernanceMint {
                reason: "initial".into(),
            },
            NexusCoin::from_coins(100),
            false,
            NexusCoin::from_coins(100),
        );

        assert_eq!(ledger.total_minted(), NexusCoin::from_coins(100));
        assert_eq!(ledger.total_supply(), NexusCoin::from_coins(100));

        ledger.record(
            "agent-1",
            TransactionType::ComputeBurn {
                model_id: "flash-2b".into(),
                tokens_used: 1000,
            },
            NexusCoin::from_coins(10),
            true,
            NexusCoin::from_coins(90),
        );

        assert_eq!(ledger.total_burned(), NexusCoin::from_coins(10));
        assert_eq!(ledger.total_supply(), NexusCoin::from_coins(90));
    }
}
