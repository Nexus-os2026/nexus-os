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
// Outcome-based billing
// ---------------------------------------------------------------------------

/// An outcome-based contract: agent gets paid when a task succeeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeContract {
    pub id: String,
    pub agent_id: String,
    /// Who is paying for results.
    pub client_id: String,
    pub task_description: String,
    pub success_criteria: SuccessCriteria,
    /// Credits transferred to agent on success.
    pub reward_amount: f64,
    /// Credits deducted from agent on failure (0 = no penalty).
    pub penalty_amount: f64,
    /// Optional deadline (unix timestamp).
    pub deadline: Option<u64>,
    pub status: ContractStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    /// Proof of completion or failure.
    pub evidence: Option<String>,
}

/// What constitutes "success" for an outcome contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuccessCriteria {
    /// Simple completion flag.
    TaskComplete,
    /// Code must pass at least `min_pass_rate`% of tests.
    TestsPassing { min_pass_rate: f64 },
    /// Output quality must be above threshold.
    QualityScore { min_score: f64 },
    /// Must complete within time limit (seconds from contract activation).
    DeliveryTime { max_seconds: u64 },
    /// Custom criteria with a human-readable description and verifier identity.
    Custom {
        description: String,
        verifier: String,
    },
}

/// Lifecycle status of an outcome contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractStatus {
    Proposed,
    Active,
    Completed { success: bool },
    Disputed,
    Expired,
    Cancelled,
}

/// Revenue statistics for outcome-based work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRevenue {
    pub total_earned_by_outcome: f64,
    pub total_contracts: usize,
    pub success_rate: f64,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Governed budget engine managing agent wallets and transactions.
pub struct EconomicEngine {
    config: EconomicConfig,
    wallets: HashMap<String, AgentWallet>,
    contracts: Vec<OutcomeContract>,
}

impl EconomicEngine {
    pub fn new(config: EconomicConfig) -> Self {
        Self {
            config,
            wallets: HashMap::new(),
            contracts: Vec::new(),
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

    // ── Outcome-based billing ──────────────────────────────────────────

    /// Create a new outcome contract. The client must have sufficient balance
    /// to cover the reward (escrow check).
    #[allow(clippy::too_many_arguments)]
    pub fn create_contract(
        &mut self,
        agent_id: &str,
        client_id: &str,
        description: &str,
        criteria: SuccessCriteria,
        reward_amount: f64,
        penalty_amount: f64,
        deadline: Option<u64>,
    ) -> Result<OutcomeContract, String> {
        if reward_amount <= 0.0 {
            return Err("reward_amount must be positive".to_string());
        }
        if penalty_amount < 0.0 {
            return Err("penalty_amount cannot be negative".to_string());
        }
        // Verify client wallet exists and has sufficient balance for escrow.
        let client_wallet = self
            .wallets
            .get(client_id)
            .ok_or_else(|| format!("client wallet not found: {client_id}"))?;
        if client_wallet.balance < reward_amount {
            return Err(format!(
                "client has insufficient balance for escrow: have {}, need {reward_amount}",
                client_wallet.balance
            ));
        }
        // Verify agent wallet exists.
        if !self.wallets.contains_key(agent_id) {
            return Err(format!("agent wallet not found: {agent_id}"));
        }

        // Escrow: deduct reward_amount from client wallet on contract creation.
        let client_wallet = self
            .wallets
            .get_mut(client_id)
            .ok_or_else(|| format!("client wallet not found: {client_id}"))?;
        client_wallet.balance -= reward_amount;
        client_wallet.total_spent += reward_amount;

        let contract = OutcomeContract {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            client_id: client_id.to_string(),
            task_description: description.to_string(),
            success_criteria: criteria,
            reward_amount,
            penalty_amount,
            deadline,
            status: ContractStatus::Proposed,
            created_at: now_secs(),
            completed_at: None,
            evidence: None,
        };
        self.contracts.push(contract.clone());
        Ok(contract)
    }

    /// Activate a proposed contract, transitioning from Proposed to Active.
    pub fn activate_contract(&mut self, contract_id: &str) -> Result<(), String> {
        let contract = self
            .contracts
            .iter_mut()
            .find(|c| c.id == contract_id)
            .ok_or_else(|| format!("contract not found: {contract_id}"))?;
        match &contract.status {
            ContractStatus::Proposed => {
                contract.status = ContractStatus::Active;
                Ok(())
            }
            other => Err(format!(
                "cannot activate contract in status {:?}, must be Proposed",
                other
            )),
        }
    }

    /// Complete a contract. On success: transfer reward from client to agent.
    /// On failure with penalty > 0: deduct penalty from agent.
    pub fn complete_contract(
        &mut self,
        contract_id: &str,
        success: bool,
        evidence: Option<String>,
    ) -> Result<Transaction, String> {
        // Find the contract and validate state.
        let contract = self
            .contracts
            .iter()
            .find(|c| c.id == contract_id)
            .ok_or_else(|| format!("contract not found: {contract_id}"))?;

        match &contract.status {
            ContractStatus::Active => {}
            ContractStatus::Expired => {
                return Err("cannot complete an expired contract".to_string())
            }
            _ => {
                return Err(format!(
                    "contract is not active (status: {:?})",
                    contract.status
                ))
            }
        }

        let agent_id = contract.agent_id.clone();
        let client_id = contract.client_id.clone();
        let reward = contract.reward_amount;
        let penalty = contract.penalty_amount;

        // Update contract status.
        let contract = self
            .contracts
            .iter_mut()
            .find(|c| c.id == contract_id)
            .unwrap();
        contract.status = ContractStatus::Completed { success };
        contract.completed_at = Some(now_secs());
        contract.evidence = evidence.clone();

        let now = now_secs();

        if success {
            // Escrowed funds go to the agent (already deducted from client at creation).
            let agent_wallet = self
                .wallets
                .get_mut(&agent_id)
                .ok_or("agent wallet missing")?;
            agent_wallet.balance += reward;
            agent_wallet.total_earned += reward;

            let tx = Transaction {
                id: Uuid::new_v4().to_string(),
                wallet_id: agent_id,
                amount: reward,
                transaction_type: TransactionType::Reward,
                description: format!("outcome contract {contract_id} — success (escrow released)"),
                timestamp: now,
                approved: true,
                counterparty: Some(client_id),
            };
            self.wallets
                .get_mut(&tx.wallet_id)
                .unwrap()
                .transaction_history
                .push(tx.clone());
            Ok(tx)
        } else if penalty > 0.0 {
            // Failure with penalty: return escrowed amount minus penalty to client.
            let refund = reward - penalty.min(reward);
            let client_wallet = self
                .wallets
                .get_mut(&client_id)
                .ok_or("client wallet missing")?;
            client_wallet.balance += refund;
            if refund > 0.0 {
                client_wallet.total_earned += refund;
            }

            let tx = Transaction {
                id: Uuid::new_v4().to_string(),
                wallet_id: client_id.clone(),
                amount: penalty.min(reward),
                transaction_type: TransactionType::ServicePurchase,
                description: format!(
                    "outcome contract {contract_id} — failure penalty (escrow partial refund)"
                ),
                timestamp: now,
                approved: true,
                counterparty: Some(agent_id),
            };
            self.wallets
                .get_mut(&client_id)
                .unwrap()
                .transaction_history
                .push(tx.clone());
            Ok(tx)
        } else {
            // Failure with no penalty — return full escrow to client.
            let client_wallet = self
                .wallets
                .get_mut(&client_id)
                .ok_or("client wallet missing")?;
            client_wallet.balance += reward;
            client_wallet.total_earned += reward;

            let tx = Transaction {
                id: Uuid::new_v4().to_string(),
                wallet_id: client_id.clone(),
                amount: 0.0,
                transaction_type: TransactionType::Refund,
                description: format!("outcome contract {contract_id} — failure (escrow returned)"),
                timestamp: now,
                approved: true,
                counterparty: Some(agent_id),
            };
            self.wallets
                .get_mut(&client_id)
                .unwrap()
                .transaction_history
                .push(tx.clone());
            Ok(tx)
        }
    }

    /// Mark a contract as disputed and return escrowed funds to client.
    pub fn dispute_contract(&mut self, contract_id: &str, reason: &str) -> Result<(), String> {
        let contract = self
            .contracts
            .iter_mut()
            .find(|c| c.id == contract_id)
            .ok_or_else(|| format!("contract not found: {contract_id}"))?;
        let client_id = contract.client_id.clone();
        let reward = contract.reward_amount;
        contract.status = ContractStatus::Disputed;
        contract.evidence = Some(format!("DISPUTE: {reason}"));

        // Return escrowed funds to client on dispute.
        let client_wallet = self
            .wallets
            .get_mut(&client_id)
            .ok_or("client wallet missing")?;
        client_wallet.balance += reward;
        client_wallet.total_earned += reward;
        Ok(())
    }

    /// List all contracts for an agent (as agent or client).
    pub fn list_contracts(&self, agent_id: &str) -> Vec<&OutcomeContract> {
        self.contracts
            .iter()
            .filter(|c| c.agent_id == agent_id || c.client_id == agent_id)
            .collect()
    }

    /// Get a single contract by ID.
    pub fn get_contract(&self, id: &str) -> Option<&OutcomeContract> {
        self.contracts.iter().find(|c| c.id == id)
    }

    /// Expire all contracts past their deadline. Returns count of newly expired.
    pub fn expire_overdue_contracts(&mut self) -> usize {
        let now = now_secs();
        let mut expired_info: Vec<(String, f64)> = Vec::new();
        for contract in &mut self.contracts {
            let is_active_or_proposed = matches!(
                contract.status,
                ContractStatus::Active | ContractStatus::Proposed
            );
            if is_active_or_proposed {
                if let Some(deadline) = contract.deadline {
                    if now > deadline {
                        expired_info.push((contract.client_id.clone(), contract.reward_amount));
                        contract.status = ContractStatus::Expired;
                        contract.completed_at = Some(now);
                    }
                }
            }
        }
        // Return escrowed funds to clients for expired contracts.
        for (client_id, reward) in &expired_info {
            if let Some(wallet) = self.wallets.get_mut(client_id) {
                wallet.balance += reward;
                wallet.total_earned += reward;
            }
        }
        expired_info.len()
    }

    /// Success rate for an agent across all completed contracts.
    pub fn agent_success_rate(&self, agent_id: &str) -> f64 {
        let completed: Vec<&OutcomeContract> = self
            .contracts
            .iter()
            .filter(|c| c.agent_id == agent_id)
            .filter(|c| matches!(c.status, ContractStatus::Completed { .. }))
            .collect();

        if completed.is_empty() {
            return 0.0;
        }

        let successes = completed
            .iter()
            .filter(|c| matches!(c.status, ContractStatus::Completed { success: true }))
            .count();

        successes as f64 / completed.len() as f64
    }

    /// Revenue statistics for an agent's outcome-based work.
    pub fn revenue_by_outcome(&self, agent_id: &str) -> OutcomeRevenue {
        let agent_contracts: Vec<&OutcomeContract> = self
            .contracts
            .iter()
            .filter(|c| c.agent_id == agent_id)
            .collect();

        let total_contracts = agent_contracts.len();
        let success_rate = self.agent_success_rate(agent_id);

        let total_earned: f64 = agent_contracts
            .iter()
            .filter(|c| matches!(c.status, ContractStatus::Completed { success: true }))
            .map(|c| c.reward_amount)
            .sum();

        OutcomeRevenue {
            total_earned_by_outcome: total_earned,
            total_contracts,
            success_rate,
        }
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

    // ── Outcome contract tests ──────────────────────────────────────────

    fn engine_with_wallets() -> EconomicEngine {
        let mut engine = EconomicEngine::new(EconomicConfig {
            default_balance: 1000.0,
            default_spending_limit: 500.0,
            default_daily_limit: 1000.0,
            require_approval_above: 1000.0,
            ..EconomicConfig::default()
        });
        engine.create_wallet("agent-1");
        engine.create_wallet("client-1");
        engine
    }

    #[test]
    fn test_create_contract() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Build landing page",
                SuccessCriteria::TaskComplete,
                50.0,
                0.0,
                None,
            )
            .unwrap();
        assert_eq!(contract.agent_id, "agent-1");
        assert_eq!(contract.client_id, "client-1");
        assert_eq!(contract.reward_amount, 50.0);
        assert!(matches!(contract.status, ContractStatus::Proposed));
        // Escrow: client balance reduced at creation
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 950.0);
        assert!(engine.get_contract(&contract.id).is_some());
    }

    #[test]
    fn test_complete_contract_success() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Write tests",
                SuccessCriteria::TestsPassing { min_pass_rate: 0.9 },
                100.0,
                10.0,
                None,
            )
            .unwrap();

        // Client escrowed 100 at creation.
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 900.0);

        engine.activate_contract(&contract.id).unwrap();

        let tx = engine
            .complete_contract(&contract.id, true, Some("all tests pass".to_string()))
            .unwrap();
        assert_eq!(tx.amount, 100.0);
        assert_eq!(tx.transaction_type, TransactionType::Reward);

        // Agent earned 100 (escrowed funds released to agent), client stays at 900.
        assert_eq!(engine.get_wallet("agent-1").unwrap().balance, 1100.0);
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 900.0);
    }

    #[test]
    fn test_complete_contract_failure_with_penalty() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Deploy service",
                SuccessCriteria::TaskComplete,
                100.0,
                25.0,
                None,
            )
            .unwrap();

        // Client escrowed 100 at creation.
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 900.0);

        engine.activate_contract(&contract.id).unwrap();

        let tx = engine
            .complete_contract(&contract.id, false, Some("deployment failed".to_string()))
            .unwrap();
        assert_eq!(tx.amount, 25.0);
        // Client gets back escrow minus penalty (100 - 25 = 75), agent unchanged.
        assert_eq!(engine.get_wallet("agent-1").unwrap().balance, 1000.0);
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 975.0);
    }

    #[test]
    fn test_complete_contract_failure_no_penalty() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Try something",
                SuccessCriteria::TaskComplete,
                50.0,
                0.0,
                None,
            )
            .unwrap();

        // Client escrowed 50 at creation.
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 950.0);

        engine.activate_contract(&contract.id).unwrap();

        let tx = engine.complete_contract(&contract.id, false, None).unwrap();
        assert_eq!(tx.amount, 0.0);
        // Full escrow returned to client, agent unchanged.
        assert_eq!(engine.get_wallet("agent-1").unwrap().balance, 1000.0);
        assert_eq!(engine.get_wallet("client-1").unwrap().balance, 1000.0);
    }

    #[test]
    fn test_expire_overdue() {
        let mut engine = engine_with_wallets();
        // Create a contract with a deadline in the past.
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Urgent task",
                SuccessCriteria::DeliveryTime { max_seconds: 60 },
                50.0,
                0.0,
                Some(1), // deadline = 1 second after epoch (already expired)
            )
            .unwrap();

        let expired = engine.expire_overdue_contracts();
        assert_eq!(expired, 1);

        let c = engine.get_contract(&contract.id).unwrap();
        assert!(matches!(c.status, ContractStatus::Expired));
    }

    #[test]
    fn test_dispute_contract() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Ambiguous task",
                SuccessCriteria::Custom {
                    description: "client satisfaction".to_string(),
                    verifier: "human-reviewer".to_string(),
                },
                75.0,
                0.0,
                None,
            )
            .unwrap();

        engine
            .dispute_contract(&contract.id, "Quality not acceptable")
            .unwrap();

        let c = engine.get_contract(&contract.id).unwrap();
        assert!(matches!(c.status, ContractStatus::Disputed));
        assert!(c.evidence.as_ref().unwrap().contains("DISPUTE"));
    }

    #[test]
    fn test_success_rate_calculation() {
        let mut engine = engine_with_wallets();
        // 3 contracts: 2 succeed, 1 fails → 66.7% rate.
        for i in 0..3 {
            let c = engine
                .create_contract(
                    "agent-1",
                    "client-1",
                    &format!("task-{i}"),
                    SuccessCriteria::TaskComplete,
                    10.0,
                    0.0,
                    None,
                )
                .unwrap();
            engine.activate_contract(&c.id).unwrap();
            engine.complete_contract(&c.id, i < 2, None).unwrap();
        }

        let rate = engine.agent_success_rate("agent-1");
        assert!((rate - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_revenue_by_outcome() {
        let mut engine = engine_with_wallets();
        for i in 0..4 {
            let c = engine
                .create_contract(
                    "agent-1",
                    "client-1",
                    &format!("task-{i}"),
                    SuccessCriteria::TaskComplete,
                    25.0,
                    0.0,
                    None,
                )
                .unwrap();
            engine.activate_contract(&c.id).unwrap();
            engine.complete_contract(&c.id, i < 3, None).unwrap();
        }

        let rev = engine.revenue_by_outcome("agent-1");
        assert_eq!(rev.total_contracts, 4);
        assert_eq!(rev.total_earned_by_outcome, 75.0); // 3 × 25
        assert!((rev.success_rate - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_contract_requires_sufficient_client_balance() {
        let mut engine = EconomicEngine::new(EconomicConfig {
            default_balance: 10.0,
            ..EconomicConfig::default()
        });
        engine.create_wallet("agent-1");
        engine.create_wallet("client-1"); // balance = 10

        let result = engine.create_contract(
            "agent-1",
            "client-1",
            "Expensive task",
            SuccessCriteria::TaskComplete,
            999.0,
            0.0,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient balance"));
    }

    #[test]
    fn test_list_contracts_by_agent() {
        let mut engine = engine_with_wallets();
        engine.create_wallet("agent-2");

        engine
            .create_contract(
                "agent-1",
                "client-1",
                "task A",
                SuccessCriteria::TaskComplete,
                10.0,
                0.0,
                None,
            )
            .unwrap();
        engine
            .create_contract(
                "agent-2",
                "client-1",
                "task B",
                SuccessCriteria::TaskComplete,
                10.0,
                0.0,
                None,
            )
            .unwrap();
        engine
            .create_contract(
                "agent-1",
                "client-1",
                "task C",
                SuccessCriteria::TaskComplete,
                10.0,
                0.0,
                None,
            )
            .unwrap();

        assert_eq!(engine.list_contracts("agent-1").len(), 2);
        assert_eq!(engine.list_contracts("agent-2").len(), 1);
        // client-1 appears in all 3 contracts.
        assert_eq!(engine.list_contracts("client-1").len(), 3);
    }

    #[test]
    fn test_cannot_complete_expired_contract() {
        let mut engine = engine_with_wallets();
        let contract = engine
            .create_contract(
                "agent-1",
                "client-1",
                "Expired task",
                SuccessCriteria::TaskComplete,
                50.0,
                0.0,
                Some(1), // already expired
            )
            .unwrap();

        engine.expire_overdue_contracts();

        let result = engine.complete_contract(&contract.id, true, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
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
