use crate::coin::NexusCoin;
use crate::wallet::AgentWallet;
use crate::EconomyError;

/// Execution gating based on autonomy level and balance.
/// L0-L3: coins tracked but never gate execution.
/// L4-L6: coins gate execution — insufficient balance blocks the action.
pub struct ExecutionGate;

impl ExecutionGate {
    /// Check if an agent can execute a compute action.
    /// Returns Ok(cost) if allowed, Err if gated.
    pub fn check_compute(
        wallet: &AgentWallet,
        compute_cost: NexusCoin,
    ) -> Result<NexusCoin, EconomyError> {
        if wallet.autonomy_level >= 4 {
            // L4-L6: hard gate — must have sufficient balance
            if wallet.available_balance() < compute_cost {
                return Err(EconomyError::GatingDenied(format!(
                    "Agent {} (L{}) has insufficient balance: {} available, {} required",
                    wallet.agent_id,
                    wallet.autonomy_level,
                    wallet.available_balance(),
                    compute_cost,
                )));
            }
        }
        // L0-L3: always allowed, cost is tracked but not enforced
        Ok(compute_cost)
    }

    /// Check if an agent can spawn a child
    pub fn check_spawn(
        wallet: &AgentWallet,
        spawn_cost: NexusCoin,
    ) -> Result<NexusCoin, EconomyError> {
        // Spawning always requires sufficient balance regardless of level
        // (you can't create economic actors without economic backing)
        if wallet.available_balance() < spawn_cost {
            return Err(EconomyError::GatingDenied(format!(
                "Agent {} has insufficient balance to spawn: {} available, {} required",
                wallet.agent_id,
                wallet.available_balance(),
                spawn_cost,
            )));
        }
        Ok(spawn_cost)
    }

    /// Check if an agent can create a delegation
    pub fn check_delegation(
        wallet: &AgentWallet,
        payment: NexusCoin,
    ) -> Result<NexusCoin, EconomyError> {
        if wallet.available_balance() < payment {
            return Err(EconomyError::GatingDenied(format!(
                "Agent {} has insufficient balance for delegation: {} available, {} required",
                wallet.agent_id,
                wallet.available_balance(),
                payment,
            )));
        }
        Ok(payment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signing_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[42u8; 32])
    }

    #[test]
    fn test_l3_not_gated() {
        let sk = test_signing_key();
        let wallet = AgentWallet::new("agent-l3".into(), NexusCoin::ZERO, 3, &sk);
        // L3 with zero balance can still compute
        let result = ExecutionGate::check_compute(&wallet, NexusCoin::from_coins(100));
        assert!(result.is_ok());
    }

    #[test]
    fn test_l4_gated() {
        let sk = test_signing_key();
        let wallet = AgentWallet::new("agent-l4".into(), NexusCoin::ZERO, 4, &sk);
        // L4 with zero balance is blocked
        let result = ExecutionGate::check_compute(&wallet, NexusCoin::from_coins(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_l4_sufficient_balance_allowed() {
        let sk = test_signing_key();
        let wallet = AgentWallet::new("agent-l4".into(), NexusCoin::from_coins(100), 4, &sk);
        let result = ExecutionGate::check_compute(&wallet, NexusCoin::from_coins(10));
        assert!(result.is_ok());
    }

    #[test]
    fn test_spawn_always_gated() {
        let sk = test_signing_key();
        // Even L0 must have balance to spawn
        let wallet = AgentWallet::new("agent-l0".into(), NexusCoin::ZERO, 0, &sk);
        let result = ExecutionGate::check_spawn(&wallet, NexusCoin::from_coins(10));
        assert!(result.is_err());
    }
}
