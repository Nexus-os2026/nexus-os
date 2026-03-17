//! Civilization Economy — reputation-based token system.
//!
//! Agents earn tokens for successful tasks, spend them on resources, and
//! transfer them to other agents. Token balance affects voting power.
//! Bankruptcy is detected when balance reaches zero.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::log::{CivilizationLog, GovernanceEventType};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Token balance for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub agent_id: String,
    pub balance: f64,
    pub earned_total: f64,
    pub spent_total: f64,
}

/// A recorded transaction between agents or the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub reason: String,
    pub timestamp: u64,
}

/// Economy error.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EconomyError {
    #[error("agent {0} not found in economy")]
    AgentNotFound(String),
    #[error("insufficient balance: {agent_id} has {balance}, needs {required}")]
    InsufficientBalance {
        agent_id: String,
        balance: f64,
        required: f64,
    },
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
}

/// The civilization economy managing token balances and transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CivilizationEconomy {
    balances: HashMap<String, TokenBalance>,
    transactions: Vec<Transaction>,
}

impl CivilizationEconomy {
    /// Create a new empty economy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure an agent exists in the economy, creating with zero balance if not.
    fn ensure_agent(&mut self, agent_id: &str) {
        self.balances
            .entry(agent_id.to_string())
            .or_insert_with(|| TokenBalance {
                agent_id: agent_id.to_string(),
                balance: 0.0,
                earned_total: 0.0,
                spent_total: 0.0,
            });
    }

    /// Earn tokens (e.g., from completing a task). Returns updated balance.
    pub fn earn_tokens(
        &mut self,
        agent_id: &str,
        amount: f64,
        reason: &str,
        log: &mut CivilizationLog,
    ) -> Result<f64, EconomyError> {
        if amount <= 0.0 {
            return Err(EconomyError::InvalidAmount(format!(
                "earn amount must be positive, got {amount}"
            )));
        }

        self.ensure_agent(agent_id);
        let bal = self.balances.get_mut(agent_id).expect("just ensured");
        bal.balance += amount;
        bal.earned_total += amount;

        self.transactions.push(Transaction {
            from: "system".to_string(),
            to: agent_id.to_string(),
            amount,
            reason: reason.to_string(),
            timestamp: now_secs(),
        });

        let _ = log.append_event(
            GovernanceEventType::TokensEarned,
            &format!("{agent_id} earned {amount:.2} tokens: {reason}"),
        );

        Ok(bal.balance)
    }

    /// Spend tokens. Returns updated balance or error if insufficient.
    pub fn spend_tokens(
        &mut self,
        agent_id: &str,
        amount: f64,
        reason: &str,
        log: &mut CivilizationLog,
    ) -> Result<f64, EconomyError> {
        if amount <= 0.0 {
            return Err(EconomyError::InvalidAmount(format!(
                "spend amount must be positive, got {amount}"
            )));
        }

        self.ensure_agent(agent_id);
        let bal = self.balances.get_mut(agent_id).expect("just ensured");

        if bal.balance < amount {
            return Err(EconomyError::InsufficientBalance {
                agent_id: agent_id.to_string(),
                balance: bal.balance,
                required: amount,
            });
        }

        bal.balance -= amount;
        bal.spent_total += amount;

        self.transactions.push(Transaction {
            from: agent_id.to_string(),
            to: "system".to_string(),
            amount,
            reason: reason.to_string(),
            timestamp: now_secs(),
        });

        let _ = log.append_event(
            GovernanceEventType::TokensSpent,
            &format!("{agent_id} spent {amount:.2} tokens: {reason}"),
        );

        // Detect bankruptcy.
        if bal.balance <= 0.0 {
            let _ = log.append_event(
                GovernanceEventType::Bankruptcy,
                &format!("{agent_id} is bankrupt (balance: {:.2})", bal.balance),
            );
        }

        Ok(bal.balance)
    }

    /// Transfer tokens from one agent to another.
    pub fn transfer_tokens(
        &mut self,
        from: &str,
        to: &str,
        amount: f64,
        reason: &str,
        log: &mut CivilizationLog,
    ) -> Result<(), EconomyError> {
        if amount <= 0.0 {
            return Err(EconomyError::InvalidAmount(format!(
                "transfer amount must be positive, got {amount}"
            )));
        }

        self.ensure_agent(from);
        self.ensure_agent(to);

        let from_bal = self.balances.get(from).expect("just ensured").balance;
        if from_bal < amount {
            return Err(EconomyError::InsufficientBalance {
                agent_id: from.to_string(),
                balance: from_bal,
                required: amount,
            });
        }

        // Debit sender.
        let sender = self.balances.get_mut(from).expect("just ensured");
        sender.balance -= amount;
        sender.spent_total += amount;

        // Credit receiver.
        let receiver = self.balances.get_mut(to).expect("just ensured");
        receiver.balance += amount;
        receiver.earned_total += amount;

        self.transactions.push(Transaction {
            from: from.to_string(),
            to: to.to_string(),
            amount,
            reason: reason.to_string(),
            timestamp: now_secs(),
        });

        let _ = log.append_event(
            GovernanceEventType::TokensSpent,
            &format!("{from} transferred {amount:.2} tokens to {to}: {reason}"),
        );

        // Check sender bankruptcy.
        let sender_balance = self.balances.get(from).expect("exists").balance;
        if sender_balance <= 0.0 {
            let _ = log.append_event(
                GovernanceEventType::Bankruptcy,
                &format!("{from} is bankrupt after transfer (balance: {sender_balance:.2})"),
            );
        }

        Ok(())
    }

    /// Get an agent's current balance.
    pub fn get_balance(&self, agent_id: &str) -> Option<&TokenBalance> {
        self.balances.get(agent_id)
    }

    /// Get all agent balances.
    pub fn get_all_balances(&self) -> Vec<&TokenBalance> {
        self.balances.values().collect()
    }

    /// Check if an agent is bankrupt (balance <= 0).
    pub fn is_bankrupt(&self, agent_id: &str) -> bool {
        self.balances
            .get(agent_id)
            .map(|b| b.balance <= 0.0)
            .unwrap_or(false)
    }

    /// Get transaction history.
    pub fn get_transactions(&self) -> &[Transaction] {
        &self.transactions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earn_and_spend() {
        let mut economy = CivilizationEconomy::new();
        let mut log = CivilizationLog::new();

        economy
            .earn_tokens("agent-1", 100.0, "task completed", &mut log)
            .unwrap();
        assert_eq!(economy.get_balance("agent-1").unwrap().balance, 100.0);

        economy
            .spend_tokens("agent-1", 30.0, "resource access", &mut log)
            .unwrap();
        assert_eq!(economy.get_balance("agent-1").unwrap().balance, 70.0);
    }

    #[test]
    fn insufficient_balance() {
        let mut economy = CivilizationEconomy::new();
        let mut log = CivilizationLog::new();

        economy
            .earn_tokens("agent-1", 10.0, "small task", &mut log)
            .unwrap();
        let err = economy
            .spend_tokens("agent-1", 50.0, "expensive op", &mut log)
            .unwrap_err();
        assert!(matches!(err, EconomyError::InsufficientBalance { .. }));
    }

    #[test]
    fn transfer_tokens_works() {
        let mut economy = CivilizationEconomy::new();
        let mut log = CivilizationLog::new();

        economy
            .earn_tokens("agent-1", 100.0, "work", &mut log)
            .unwrap();
        economy
            .transfer_tokens("agent-1", "agent-2", 40.0, "payment", &mut log)
            .unwrap();

        assert_eq!(economy.get_balance("agent-1").unwrap().balance, 60.0);
        assert_eq!(economy.get_balance("agent-2").unwrap().balance, 40.0);
    }

    #[test]
    fn bankruptcy_detected() {
        let mut economy = CivilizationEconomy::new();
        let mut log = CivilizationLog::new();

        economy
            .earn_tokens("agent-1", 10.0, "work", &mut log)
            .unwrap();
        economy
            .spend_tokens("agent-1", 10.0, "all-in", &mut log)
            .unwrap();

        assert!(economy.is_bankrupt("agent-1"));
    }

    #[test]
    fn invalid_amounts_rejected() {
        let mut economy = CivilizationEconomy::new();
        let mut log = CivilizationLog::new();

        assert!(economy.earn_tokens("a", -5.0, "bad", &mut log).is_err());
        assert!(economy.spend_tokens("a", 0.0, "bad", &mut log).is_err());
        assert!(economy
            .transfer_tokens("a", "b", -1.0, "bad", &mut log)
            .is_err());
    }
}
