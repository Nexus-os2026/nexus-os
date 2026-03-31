use serde::{Deserialize, Serialize};
use std::sync::RwLock;

use crate::coin::{NexusCoin, TransactionType};
use crate::compute_pricing::ComputePricingTable;
use crate::delegation::DelegationManager;
use crate::genesis_economy::GenesisEconomy;
use crate::ledger::EconomyLedger;
use crate::rewards::RewardEngine;
use crate::supply::SupplyMetrics;
use crate::wallet::AgentWallet;

pub struct EconomyState {
    pub wallets: RwLock<Vec<AgentWallet>>,
    pub ledger: RwLock<EconomyLedger>,
    pub delegations: RwLock<DelegationManager>,
    pub pricing: ComputePricingTable,
    pub rewards: RewardEngine,
    pub genesis: GenesisEconomy,
    pub supply: RwLock<SupplyMetrics>,
    pub identity: nexus_crypto::CryptoIdentity,
}

impl EconomyState {
    pub fn new() -> Self {
        let identity =
            nexus_crypto::CryptoIdentity::generate(nexus_crypto::SignatureAlgorithm::Ed25519)
                .expect("Ed25519 key generation should never fail");
        Self {
            wallets: RwLock::new(Vec::new()),
            ledger: RwLock::new(EconomyLedger::new()),
            delegations: RwLock::new(DelegationManager::new()),
            pricing: ComputePricingTable::default_pricing(),
            rewards: RewardEngine::default_config(),
            genesis: GenesisEconomy::default_config(),
            supply: RwLock::new(SupplyMetrics::new()),
            identity,
        }
    }
}

impl Default for EconomyState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSummary {
    pub agent_id: String,
    pub balance: f64,
    pub available_balance: f64,
    pub lifetime_earned: f64,
    pub lifetime_burned: f64,
    pub lifetime_transferred: f64,
    pub lifetime_received: f64,
    pub escrowed: f64,
    pub burn_rate: f64,
    pub autonomy_level: u8,
    pub version: u64,
}

impl From<&AgentWallet> for WalletSummary {
    fn from(w: &AgentWallet) -> Self {
        Self {
            agent_id: w.agent_id.clone(),
            balance: w.balance.as_f64(),
            available_balance: w.available_balance().as_f64(),
            lifetime_earned: w.lifetime_earned.as_f64(),
            lifetime_burned: w.lifetime_burned.as_f64(),
            lifetime_transferred: w.lifetime_transferred.as_f64(),
            lifetime_received: w.lifetime_received.as_f64(),
            escrowed: w.escrowed.as_f64(),
            burn_rate: w.burn_rate(),
            autonomy_level: w.autonomy_level,
            version: w.version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntrySummary {
    pub entry_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub transaction_type: String,
    pub amount: f64,
    pub is_burn: bool,
    pub balance_after: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplySummary {
    pub total_supply: f64,
    pub total_burned: f64,
    pub total_minted: f64,
    pub is_deflationary: bool,
    pub active_wallets: usize,
    pub active_delegations: usize,
    pub total_escrowed: f64,
    pub net_flow: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnEstimate {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_nxc: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardEstimate {
    pub base: f64,
    pub quality_multiplier: f64,
    pub difficulty_multiplier: f64,
    pub speed_multiplier: f64,
    pub final_reward: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnEstimate {
    pub spawn_cost: f64,
    pub child_allocation: f64,
    pub parent_remaining: f64,
    pub fraction_used: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationSummary {
    pub id: String,
    pub requester_id: String,
    pub provider_id: String,
    pub task_description: String,
    pub payment: f64,
    pub status: String,
    pub quality_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingSummary {
    pub model_id: String,
    pub size_class: String,
    pub is_local: bool,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

// ── Tauri Command Functions ──

pub fn get_wallet(state: &EconomyState, agent_id: &str) -> Result<WalletSummary, String> {
    let wallets = state.wallets.read().map_err(|e| e.to_string())?;
    wallets
        .iter()
        .find(|w| w.agent_id == agent_id)
        .map(WalletSummary::from)
        .ok_or_else(|| format!("Wallet not found for agent {}", agent_id))
}

pub fn get_all_wallets(state: &EconomyState) -> Result<Vec<WalletSummary>, String> {
    let wallets = state.wallets.read().map_err(|e| e.to_string())?;
    Ok(wallets.iter().map(WalletSummary::from).collect())
}

pub fn create_wallet(
    state: &EconomyState,
    agent_id: &str,
    initial_balance: f64,
    autonomy_level: u8,
) -> Result<WalletSummary, String> {
    let mut wallets = state.wallets.write().map_err(|e| e.to_string())?;
    if wallets.iter().any(|w| w.agent_id == agent_id) {
        return Err(format!("Wallet already exists for agent {}", agent_id));
    }

    let balance = NexusCoin::from_micro((initial_balance * 1_000_000.0) as u64);
    let wallet = AgentWallet::new(
        agent_id.to_string(),
        balance,
        autonomy_level,
        &state.identity,
    );

    // Record mint in ledger
    let mut ledger = state.ledger.write().map_err(|e| e.to_string())?;
    ledger.record(
        agent_id,
        TransactionType::GovernanceMint {
            reason: "initial allocation".into(),
        },
        balance,
        false,
        balance,
    );

    let summary = WalletSummary::from(&wallet);
    wallets.push(wallet);
    Ok(summary)
}

pub fn get_ledger(
    state: &EconomyState,
    agent_id: Option<&str>,
    limit: usize,
) -> Result<Vec<LedgerEntrySummary>, String> {
    let ledger = state.ledger.read().map_err(|e| e.to_string())?;
    let entries: Vec<LedgerEntrySummary> = ledger
        .entries()
        .iter()
        .rev()
        .filter(|e| agent_id.is_none_or(|id| e.agent_id == id))
        .take(limit)
        .map(|e| LedgerEntrySummary {
            entry_id: e.entry_id.clone(),
            timestamp: e.timestamp,
            agent_id: e.agent_id.clone(),
            transaction_type: format!("{:?}", e.transaction_type),
            amount: e.amount.as_f64(),
            is_burn: e.is_burn,
            balance_after: e.balance_after.as_f64(),
        })
        .collect();
    Ok(entries)
}

pub fn get_supply(state: &EconomyState) -> Result<SupplySummary, String> {
    let ledger = state.ledger.read().map_err(|e| e.to_string())?;
    let supply = state.supply.read().map_err(|e| e.to_string())?;
    let wallets = state.wallets.read().map_err(|e| e.to_string())?;
    let delegations = state.delegations.read().map_err(|e| e.to_string())?;

    let total_escrowed: u64 = wallets.iter().map(|w| w.escrowed.micro()).sum();
    let active_delegations = delegations
        .delegations()
        .iter()
        .filter(|d| {
            d.status == crate::delegation::DelegationStatus::InProgress
                || d.status == crate::delegation::DelegationStatus::Pending
        })
        .count();

    Ok(SupplySummary {
        total_supply: ledger.total_supply().as_f64(),
        total_burned: ledger.total_burned().as_f64(),
        total_minted: ledger.total_minted().as_f64(),
        is_deflationary: supply.is_deflationary(),
        active_wallets: wallets.len(),
        active_delegations,
        total_escrowed: NexusCoin::from_micro(total_escrowed).as_f64(),
        net_flow: supply.snapshots.last().map(|s| s.net_flow).unwrap_or(0),
    })
}

pub fn calculate_burn(
    state: &EconomyState,
    model_id: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> BurnEstimate {
    let cost = state
        .pricing
        .calculate_burn(model_id, input_tokens, output_tokens);
    BurnEstimate {
        model_id: model_id.to_string(),
        input_tokens,
        output_tokens,
        cost_nxc: cost.as_f64(),
    }
}

pub fn calculate_reward(
    state: &EconomyState,
    quality: f64,
    difficulty: f64,
    completion_secs: u64,
) -> RewardEstimate {
    let calc = state
        .rewards
        .calculate_reward(quality, difficulty, completion_secs);
    RewardEstimate {
        base: calc.base.as_f64(),
        quality_multiplier: calc.quality_multiplier,
        difficulty_multiplier: calc.difficulty_multiplier,
        speed_multiplier: calc.speed_multiplier,
        final_reward: calc.final_reward.as_f64(),
    }
}

pub fn calculate_spawn(
    state: &EconomyState,
    parent_id: &str,
    fraction: Option<f64>,
) -> Result<SpawnEstimate, String> {
    let wallets = state.wallets.read().map_err(|e| e.to_string())?;
    let parent = wallets
        .iter()
        .find(|w| w.agent_id == parent_id)
        .ok_or_else(|| format!("Parent wallet not found: {}", parent_id))?;

    let calc = state
        .genesis
        .calculate_spawn(parent.balance, fraction)
        .map_err(|e| e.to_string())?;

    Ok(SpawnEstimate {
        spawn_cost: calc.spawn_cost.as_f64(),
        child_allocation: calc.child_allocation.as_f64(),
        parent_remaining: calc.parent_remaining.as_f64(),
        fraction_used: calc.fraction_used,
    })
}

pub fn create_delegation(
    state: &EconomyState,
    requester_id: &str,
    provider_id: &str,
    task: &str,
    payment: f64,
    threshold: f64,
    timeout: u64,
) -> Result<DelegationSummary, String> {
    let payment_coins = NexusCoin::from_micro((payment * 1_000_000.0) as u64);

    // Lock escrow on requester wallet
    {
        let mut wallets = state.wallets.write().map_err(|e| e.to_string())?;
        let requester = wallets
            .iter_mut()
            .find(|w| w.agent_id == requester_id)
            .ok_or_else(|| format!("Requester wallet not found: {}", requester_id))?;
        requester
            .lock_escrow(payment_coins, &state.identity)
            .map_err(|e| e.to_string())?;
    }

    let mut delegations = state.delegations.write().map_err(|e| e.to_string())?;
    let d = delegations.create(
        requester_id.to_string(),
        provider_id.to_string(),
        task.to_string(),
        payment_coins,
        threshold,
        timeout,
    );

    // Record in ledger
    let mut ledger = state.ledger.write().map_err(|e| e.to_string())?;
    ledger.record(
        requester_id,
        TransactionType::DelegationLock {
            delegation_id: d.id.clone(),
            provider_id: provider_id.to_string(),
        },
        payment_coins,
        false,
        NexusCoin::ZERO, // We'd need the actual balance here in production
    );

    Ok(DelegationSummary {
        id: d.id,
        requester_id: d.requester_id,
        provider_id: d.provider_id,
        task_description: d.task_description,
        payment: d.payment.as_f64(),
        status: format!("{:?}", d.status),
        quality_threshold: d.quality_threshold,
    })
}

pub fn get_delegations(
    state: &EconomyState,
    agent_id: &str,
) -> Result<Vec<DelegationSummary>, String> {
    let delegations = state.delegations.read().map_err(|e| e.to_string())?;
    let results: Vec<DelegationSummary> = delegations
        .delegations()
        .iter()
        .filter(|d| d.requester_id == agent_id || d.provider_id == agent_id)
        .map(|d| DelegationSummary {
            id: d.id.clone(),
            requester_id: d.requester_id.clone(),
            provider_id: d.provider_id.clone(),
            task_description: d.task_description.clone(),
            payment: d.payment.as_f64(),
            status: format!("{:?}", d.status),
            quality_threshold: d.quality_threshold,
        })
        .collect();
    Ok(results)
}

pub fn get_pricing(state: &EconomyState) -> Vec<PricingSummary> {
    state
        .pricing
        .prices
        .iter()
        .map(|p| PricingSummary {
            model_id: p.model_id.clone(),
            size_class: p.size_class.clone(),
            is_local: p.is_local,
            input_cost_per_1k: NexusCoin::from_micro(p.input_cost_per_1k).as_f64(),
            output_cost_per_1k: NexusCoin::from_micro(p.output_cost_per_1k).as_f64(),
        })
        .collect()
}
