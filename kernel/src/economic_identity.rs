//! Autonomous Economic Identity — governed crypto-wallet-style budget system
//! for agents.
//!
//! Each agent has a wallet with credits, spending/daily limits, and full
//! transaction history.  Transactions above a configurable threshold require
//! HITL approval.  No real blockchain — this is a governance layer that could
//! later bridge to on-chain settlement.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An agent's governed credit wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWallet {
    pub agent_id: String,
    pub balance: f64,
    pub total_earned: f64,
    pub total_spent: f64,
    /// Maximum amount per single transaction.
    pub spending_limit: f64,
    /// Maximum aggregate spend per day.
    pub daily_limit: f64,
    /// Amount already spent today.
    pub daily_spent: f64,
    /// Timestamp of the last daily-counter reset.
    pub last_reset: u64,
    pub transaction_history: Vec<Transaction>,
    pub frozen: bool,
}

/// A single ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub wallet_id: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub description: String,
    pub timestamp: u64,
    pub approved: bool,
    /// The other agent or external service involved, if any.
    pub counterparty: Option<String>,
}

/// Classification of a transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionType {
    ApiCall,
    ServicePurchase,
    DataPurchase,
    Reward,
    Transfer,
    Refund,
    TopUp,
}

/// Global economic-engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicConfig {
    pub enabled: bool,
    pub default_balance: f64,
    pub default_spending_limit: f64,
    pub default_daily_limit: f64,
    /// Transactions above this amount require HITL approval.
    pub require_approval_above: f64,
}

impl Default for EconomicConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_balance: 100.0,
            default_spending_limit: 10.0,
            default_daily_limit: 100.0,
            require_approval_above: 50.0,
        }
    }
}

/// Aggregate economy statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyStats {
    pub total_wallets: usize,
    pub total_balance: f64,
    pub total_transactions: usize,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Governed budget engine managing agent wallets and transactions.
pub struct EconomicEngine {
    config: EconomicConfig,
    wallets: HashMap<String, AgentWallet>,
}

impl EconomicEngine {
    pub fn new(config: EconomicConfig) -> Self {
        Self {
            config,
            wallets: HashMap::new(),
        }
    }

    /// Create a new wallet for `agent_id` with default balances.
    pub fn create_wallet(&mut self, agent_id: &str) -> AgentWallet {
        let now = now_secs();
        let wallet = AgentWallet {
            agent_id: agent_id.to_string(),
            balance: self.config.default_balance,
            total_earned: 0.0,
            total_spent: 0.0,
            spending_limit: self.config.default_spending_limit,
            daily_limit: self.config.default_daily_limit,
            daily_spent: 0.0,
            last_reset: now,
            transaction_history: Vec::new(),
            frozen: false,
        };
        self.wallets.insert(agent_id.to_string(), wallet.clone());
        wallet
    }

    pub fn get_wallet(&self, agent_id: &str) -> Option<&AgentWallet> {
        self.wallets.get(agent_id)
    }

    /// Spend credits from an agent's wallet.
    pub fn spend(
        &mut self,
        agent_id: &str,
        amount: f64,
        tx_type: TransactionType,
        description: &str,
        counterparty: Option<String>,
    ) -> Result<Transaction, String> {
        let wallet = self
            .wallets
            .get_mut(agent_id)
            .ok_or_else(|| format!("wallet not found: {agent_id}"))?;

        if wallet.frozen {
            return Err("wallet is frozen".to_string());
        }
        if amount <= 0.0 {
            return Err("amount must be positive".to_string());
        }
        if amount > wallet.spending_limit {
            return Err(format!(
                "amount {amount} exceeds spending limit {}",
                wallet.spending_limit
            ));
        }
        if wallet.daily_spent + amount > wallet.daily_limit {
            return Err(format!(
                "amount {amount} would exceed daily limit {} (already spent {})",
                wallet.daily_limit, wallet.daily_spent
            ));
        }
        if amount > wallet.balance {
            return Err(format!(
                "insufficient balance: have {}, need {amount}",
                wallet.balance
            ));
        }

        let needs_approval = amount > self.config.require_approval_above;

        let tx = Transaction {
            id: Uuid::new_v4().to_string(),
            wallet_id: agent_id.to_string(),
            amount,
            transaction_type: tx_type,
            description: description.to_string(),
            timestamp: now_secs(),
            approved: !needs_approval,
            counterparty,
        };

        if tx.approved {
            wallet.balance -= amount;
            wallet.total_spent += amount;
            wallet.daily_spent += amount;
        }

        wallet.transaction_history.push(tx.clone());
        Ok(tx)
    }

    /// Credit an agent's wallet (rewards, refunds, etc.).
    pub fn earn(
        &mut self,
        agent_id: &str,
        amount: f64,
        description: &str,
    ) -> Result<Transaction, String> {
        let wallet = self
            .wallets
            .get_mut(agent_id)
            .ok_or_else(|| format!("wallet not found: {agent_id}"))?;

        if amount <= 0.0 {
            return Err("amount must be positive".to_string());
        }

        wallet.balance += amount;
        wallet.total_earned += amount;

        let tx = Transaction {
            id: Uuid::new_v4().to_string(),
            wallet_id: agent_id.to_string(),
            amount,
            transaction_type: TransactionType::Reward,
            description: description.to_string(),
            timestamp: now_secs(),
            approved: true,
            counterparty: None,
        };
        wallet.transaction_history.push(tx.clone());
        Ok(tx)
    }

    /// Transfer credits between two agent wallets.
    pub fn transfer(
        &mut self,
        from_agent: &str,
        to_agent: &str,
        amount: f64,
        description: &str,
    ) -> Result<(Transaction, Transaction), String> {
        if amount <= 0.0 {
            return Err("amount must be positive".to_string());
        }
        // Validate both wallets exist and sender has sufficient balance.
        {
            let from = self
                .wallets
                .get(from_agent)
                .ok_or_else(|| format!("sender wallet not found: {from_agent}"))?;
            if from.frozen {
                return Err("sender wallet is frozen".to_string());
            }
            if from.balance < amount {
                return Err(format!(
                    "insufficient balance: have {}, need {amount}",
                    from.balance
                ));
            }
            if !self.wallets.contains_key(to_agent) {
                return Err(format!("receiver wallet not found: {to_agent}"));
            }
        }

        let now = now_secs();

        // Debit sender
        let from_wallet = self.wallets.get_mut(from_agent).unwrap();
        from_wallet.balance -= amount;
        from_wallet.total_spent += amount;
        let debit_tx = Transaction {
            id: Uuid::new_v4().to_string(),
            wallet_id: from_agent.to_string(),
            amount,
            transaction_type: TransactionType::Transfer,
            description: description.to_string(),
            timestamp: now,
            approved: true,
            counterparty: Some(to_agent.to_string()),
        };
        from_wallet.transaction_history.push(debit_tx.clone());

        // Credit receiver
        let to_wallet = self.wallets.get_mut(to_agent).unwrap();
        to_wallet.balance += amount;
        to_wallet.total_earned += amount;
        let credit_tx = Transaction {
            id: Uuid::new_v4().to_string(),
            wallet_id: to_agent.to_string(),
            amount,
            transaction_type: TransactionType::Transfer,
            description: description.to_string(),
            timestamp: now,
            approved: true,
            counterparty: Some(from_agent.to_string()),
        };
        to_wallet.transaction_history.push(credit_tx.clone());

        Ok((debit_tx, credit_tx))
    }

    /// Approve a pending transaction and execute the deduction.
    pub fn approve_transaction(&mut self, agent_id: &str, tx_id: &str) -> Result<(), String> {
        let wallet = self
            .wallets
            .get_mut(agent_id)
            .ok_or_else(|| format!("wallet not found: {agent_id}"))?;

        let tx = wallet
            .transaction_history
            .iter_mut()
            .find(|t| t.id == tx_id)
            .ok_or_else(|| format!("transaction not found: {tx_id}"))?;

        if tx.approved {
            return Err("transaction already approved".to_string());
        }

        // Execute the deferred deduction.
        if wallet.balance < tx.amount {
            return Err(format!(
                "insufficient balance to approve: have {}, need {}",
                wallet.balance, tx.amount
            ));
        }

        tx.approved = true;
        wallet.balance -= tx.amount;
        wallet.total_spent += tx.amount;
        wallet.daily_spent += tx.amount;
        Ok(())
    }

    pub fn freeze_wallet(&mut self, agent_id: &str) -> Result<(), String> {
        let wallet = self
            .wallets
            .get_mut(agent_id)
            .ok_or_else(|| format!("wallet not found: {agent_id}"))?;
        wallet.frozen = true;
        Ok(())
    }

    pub fn unfreeze_wallet(&mut self, agent_id: &str) -> Result<(), String> {
        let wallet = self
            .wallets
            .get_mut(agent_id)
            .ok_or_else(|| format!("wallet not found: {agent_id}"))?;
        wallet.frozen = false;
        Ok(())
    }

    pub fn get_transaction_history(&self, agent_id: &str) -> Vec<&Transaction> {
        self.wallets
            .get(agent_id)
            .map(|w| w.transaction_history.iter().collect())
            .unwrap_or_default()
    }

    /// Reset the daily spending counter if a new day has started.
    pub fn daily_reset(&mut self, agent_id: &str) {
        let now = now_secs();
        if let Some(wallet) = self.wallets.get_mut(agent_id) {
            // A "day" = 86400 seconds since last reset.
            if now - wallet.last_reset >= 86400 {
                wallet.daily_spent = 0.0;
                wallet.last_reset = now;
            }
        }
    }

    /// Aggregate statistics across all wallets.
    pub fn total_economy_stats(&self) -> EconomyStats {
        let total_balance: f64 = self.wallets.values().map(|w| w.balance).sum();
        let total_transactions: usize = self
            .wallets
            .values()
            .map(|w| w.transaction_history.len())
            .sum();
        EconomyStats {
            total_wallets: self.wallets.len(),
            total_balance,
            total_transactions,
        }
    }

    /// Read-only access to the config.
    pub fn config(&self) -> &EconomicConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_engine() -> EconomicEngine {
        EconomicEngine::new(EconomicConfig::default())
    }

    #[test]
    fn test_config_defaults() {
        let cfg = EconomicConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.default_balance, 100.0);
        assert_eq!(cfg.default_spending_limit, 10.0);
        assert_eq!(cfg.default_daily_limit, 100.0);
        assert_eq!(cfg.require_approval_above, 50.0);
    }

    #[test]
    fn test_create_wallet() {
        let mut engine = default_engine();
        let wallet = engine.create_wallet("agent-1");
        assert_eq!(wallet.agent_id, "agent-1");
        assert_eq!(wallet.balance, 100.0);
        assert!(!wallet.frozen);
        assert!(engine.get_wallet("agent-1").is_some());
    }

    #[test]
    fn test_spend_success() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        let tx = engine
            .spend(
                "agent-1",
                5.0,
                TransactionType::ApiCall,
                "openai call",
                None,
            )
            .unwrap();
        assert!(tx.approved);
        assert_eq!(tx.amount, 5.0);
        let wallet = engine.get_wallet("agent-1").unwrap();
        assert_eq!(wallet.balance, 95.0);
        assert_eq!(wallet.daily_spent, 5.0);
        assert_eq!(wallet.total_spent, 5.0);
    }

    #[test]
    fn test_spend_over_limit() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        // spending_limit is 10.0
        let result = engine.spend("agent-1", 15.0, TransactionType::ApiCall, "too much", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("spending limit"));
    }

    #[test]
    fn test_spend_over_daily_limit() {
        let mut engine = EconomicEngine::new(EconomicConfig {
            default_spending_limit: 60.0,
            default_daily_limit: 100.0,
            require_approval_above: 100.0, // disable approval for this test
            ..EconomicConfig::default()
        });
        engine.create_wallet("agent-1");
        // Spend 60, then try to spend 50 → over daily limit of 100
        engine
            .spend("agent-1", 60.0, TransactionType::ApiCall, "first", None)
            .unwrap();
        let result = engine.spend("agent-1", 50.0, TransactionType::ApiCall, "second", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("daily limit"));
    }

    #[test]
    fn test_spend_insufficient_balance() {
        let mut engine = EconomicEngine::new(EconomicConfig {
            default_balance: 5.0,
            default_spending_limit: 20.0,
            ..EconomicConfig::default()
        });
        engine.create_wallet("agent-1");
        let result = engine.spend(
            "agent-1",
            10.0,
            TransactionType::ApiCall,
            "too expensive",
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient balance"));
    }

    #[test]
    fn test_spend_frozen_wallet() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        engine.freeze_wallet("agent-1").unwrap();
        let result = engine.spend("agent-1", 1.0, TransactionType::ApiCall, "frozen", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("frozen"));
    }

    #[test]
    fn test_earn() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        let tx = engine.earn("agent-1", 25.0, "task reward").unwrap();
        assert!(tx.approved);
        assert_eq!(tx.transaction_type, TransactionType::Reward);
        let wallet = engine.get_wallet("agent-1").unwrap();
        assert_eq!(wallet.balance, 125.0);
        assert_eq!(wallet.total_earned, 25.0);
    }

    #[test]
    fn test_transfer() {
        let mut engine = default_engine();
        engine.create_wallet("agent-a");
        engine.create_wallet("agent-b");
        let (debit, credit) = engine
            .transfer("agent-a", "agent-b", 30.0, "data share")
            .unwrap();
        assert_eq!(debit.transaction_type, TransactionType::Transfer);
        assert_eq!(credit.counterparty.as_deref(), Some("agent-a"));
        assert_eq!(engine.get_wallet("agent-a").unwrap().balance, 70.0);
        assert_eq!(engine.get_wallet("agent-b").unwrap().balance, 130.0);
    }

    #[test]
    fn test_transfer_insufficient() {
        let mut engine = default_engine();
        engine.create_wallet("agent-a");
        engine.create_wallet("agent-b");
        let result = engine.transfer("agent-a", "agent-b", 200.0, "too much");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient"));
    }

    #[test]
    fn test_freeze_unfreeze() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        engine.freeze_wallet("agent-1").unwrap();
        assert!(engine.get_wallet("agent-1").unwrap().frozen);
        engine.unfreeze_wallet("agent-1").unwrap();
        assert!(!engine.get_wallet("agent-1").unwrap().frozen);
    }

    #[test]
    fn test_transaction_history() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        engine
            .spend("agent-1", 2.0, TransactionType::ApiCall, "call 1", None)
            .unwrap();
        engine
            .spend("agent-1", 3.0, TransactionType::DataPurchase, "data", None)
            .unwrap();
        engine.earn("agent-1", 10.0, "reward").unwrap();
        let history = engine.get_transaction_history("agent-1");
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_daily_reset() {
        let mut engine = default_engine();
        engine.create_wallet("agent-1");
        engine
            .spend("agent-1", 5.0, TransactionType::ApiCall, "call", None)
            .unwrap();
        assert_eq!(engine.get_wallet("agent-1").unwrap().daily_spent, 5.0);

        // Simulate that the last reset was >24h ago.
        let wallet = engine.wallets.get_mut("agent-1").unwrap();
        wallet.last_reset -= 86401;

        engine.daily_reset("agent-1");
        assert_eq!(engine.get_wallet("agent-1").unwrap().daily_spent, 0.0);
    }

    #[test]
    fn test_approval_required_above_threshold() {
        let mut engine = EconomicEngine::new(EconomicConfig {
            require_approval_above: 5.0,
            default_spending_limit: 20.0,
            ..EconomicConfig::default()
        });
        engine.create_wallet("agent-1");

        // Below threshold → auto-approved
        let tx_low = engine
            .spend("agent-1", 3.0, TransactionType::ApiCall, "small", None)
            .unwrap();
        assert!(tx_low.approved);

        // Above threshold → pending
        let tx_high = engine
            .spend("agent-1", 8.0, TransactionType::ApiCall, "big", None)
            .unwrap();
        assert!(!tx_high.approved);
        // Balance should NOT have been deducted yet
        assert_eq!(engine.get_wallet("agent-1").unwrap().balance, 97.0);

        // Approve it
        engine.approve_transaction("agent-1", &tx_high.id).unwrap();
        assert_eq!(engine.get_wallet("agent-1").unwrap().balance, 89.0);

        // Double-approve fails
        let result = engine.approve_transaction("agent-1", &tx_high.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_total_economy_stats() {
        let mut engine = default_engine();
        engine.create_wallet("a");
        engine.create_wallet("b");
        engine
            .spend("a", 5.0, TransactionType::ApiCall, "x", None)
            .unwrap();
        let stats = engine.total_economy_stats();
        assert_eq!(stats.total_wallets, 2);
        assert_eq!(stats.total_balance, 195.0); // 95 + 100
        assert_eq!(stats.total_transactions, 1);
    }
}
