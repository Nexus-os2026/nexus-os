use serde::{Deserialize, Serialize};

use crate::coin::NexusCoin;
use crate::EconomyError;

/// Genesis economy — economic rules for agent spawning.
/// Spawn costs burn coins. Child allocation transfers from parent.
/// Child NEVER receives more than parent's remaining balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisEconomy {
    /// Base cost to spawn a child agent (burned, not transferred)
    pub spawn_cost: NexusCoin,
    /// Maximum fraction of parent's remaining balance to allocate to child
    pub max_child_fraction: f64,
    /// Default fraction if not specified
    pub default_child_fraction: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnCalculation {
    /// Coins burned for the spawn (leaves supply)
    pub spawn_cost: NexusCoin,
    /// Coins allocated to child wallet (transfer, not burn)
    pub child_allocation: NexusCoin,
    /// Parent's balance after spawn + allocation
    pub parent_remaining: NexusCoin,
    /// Actual fraction used
    pub fraction_used: f64,
}

impl GenesisEconomy {
    pub fn default_config() -> Self {
        Self {
            spawn_cost: NexusCoin::from_coins(10), // 10 NXC to spawn
            max_child_fraction: 0.5,               // Child gets at most 50% of remaining
            default_child_fraction: 0.25,          // Default: 25% of remaining
        }
    }

    /// Calculate the cost and allocation for spawning a child
    pub fn calculate_spawn(
        &self,
        parent_balance: NexusCoin,
        requested_fraction: Option<f64>,
    ) -> Result<SpawnCalculation, EconomyError> {
        // Check parent can afford the spawn cost
        if parent_balance < self.spawn_cost {
            return Err(EconomyError::InsufficientBalance {
                requested: self.spawn_cost,
                available: parent_balance,
            });
        }

        // Balance after spawn cost burn
        let after_burn = parent_balance.checked_sub(self.spawn_cost).ok_or(
            EconomyError::InsufficientBalance {
                requested: self.spawn_cost,
                available: parent_balance,
            },
        )?;

        // Child allocation
        let fraction = requested_fraction
            .unwrap_or(self.default_child_fraction)
            .clamp(0.0, self.max_child_fraction);

        let child_allocation = NexusCoin::from_micro((after_burn.micro() as f64 * fraction) as u64);

        // Parent remaining after spawn cost + child allocation
        let parent_remaining = after_burn
            .checked_sub(child_allocation)
            .unwrap_or(NexusCoin::ZERO);

        Ok(SpawnCalculation {
            spawn_cost: self.spawn_cost,
            child_allocation,
            parent_remaining,
            fraction_used: fraction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_cost_burns() {
        let genesis = GenesisEconomy::default_config();
        let calc = genesis
            .calculate_spawn(NexusCoin::from_coins(100), None)
            .unwrap();

        // 10 NXC burned for spawn
        assert_eq!(calc.spawn_cost, NexusCoin::from_coins(10));
        // Remaining: 90 NXC, child gets 25% = 22.5 NXC
        assert_eq!(calc.child_allocation.coins(), 22);
        // Parent remaining ≈ 67.5 NXC
        assert!(calc.parent_remaining.coins() >= 67);
    }

    #[test]
    fn test_child_never_exceeds_parent() {
        let genesis = GenesisEconomy::default_config();
        // Even requesting 100% fraction, it caps at 50%
        let calc = genesis
            .calculate_spawn(NexusCoin::from_coins(100), Some(1.0))
            .unwrap();

        assert!(calc.child_allocation <= NexusCoin::from_coins(45)); // 50% of 90
        assert!(calc.parent_remaining >= NexusCoin::from_coins(45));
    }

    #[test]
    fn test_insufficient_spawn_balance() {
        let genesis = GenesisEconomy::default_config();
        let result = genesis.calculate_spawn(NexusCoin::from_coins(5), None);
        assert!(result.is_err());
    }
}
