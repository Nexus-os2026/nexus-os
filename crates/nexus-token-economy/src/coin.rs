use serde::{Deserialize, Serialize};

/// Nexus coin amount — the atomic unit of the economy.
/// Stored as u64 micronexus (1 coin = 1,000,000 micronexus) for precision
/// without floating point errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NexusCoin(u64);

impl NexusCoin {
    pub const ZERO: Self = Self(0);

    /// Create from whole coins
    pub fn from_coins(coins: u64) -> Self {
        Self(coins.saturating_mul(1_000_000))
    }

    /// Create from micronexus (1 coin = 1,000,000 micro)
    pub fn from_micro(micro: u64) -> Self {
        Self(micro)
    }

    /// Get the value in whole coins (truncated)
    pub fn coins(&self) -> u64 {
        self.0 / 1_000_000
    }

    /// Get the value in micronexus
    pub fn micro(&self) -> u64 {
        self.0
    }

    /// Get as f64 coins (for display)
    pub fn as_f64(&self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    /// Checked addition — returns None on overflow
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    /// Checked subtraction — returns None if insufficient
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    /// Multiply by a fraction (for child allocation)
    pub fn fraction(self, numerator: u64, denominator: u64) -> Self {
        if denominator == 0 {
            return Self::ZERO;
        }
        Self((self.0 as u128 * numerator as u128 / denominator as u128) as u64)
    }
}

impl std::fmt::Display for NexusCoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:06} NXC", self.0 / 1_000_000, self.0 % 1_000_000)
    }
}

/// Types of transactions in the economy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    /// Governance-approved allocation (coins enter the system)
    GovernanceMint { reason: String },
    /// Task completion reward (coins enter the system)
    TaskReward {
        task_id: String,
        quality_score: f64,
        difficulty: f64,
    },
    /// Compute cost burn (coins leave the system permanently)
    ComputeBurn { model_id: String, tokens_used: u64 },
    /// Genesis spawn cost burn (coins leave the system permanently)
    SpawnBurn { child_agent_id: String },
    /// Child allocation (transfer from parent to child wallet)
    ChildAllocation {
        parent_id: String,
        child_id: String,
        fraction: f64,
    },
    /// Delegation escrow lock (coins held pending task completion)
    DelegationLock {
        delegation_id: String,
        provider_id: String,
    },
    /// Delegation escrow release (coins transferred to provider)
    DelegationRelease {
        delegation_id: String,
        provider_id: String,
    },
    /// Delegation escrow refund (coins returned to requester)
    DelegationRefund { delegation_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coin_arithmetic() {
        let a = NexusCoin::from_coins(10);
        let b = NexusCoin::from_coins(5);
        assert_eq!(a.checked_add(b).unwrap(), NexusCoin::from_coins(15));
        assert_eq!(a.checked_sub(b).unwrap(), NexusCoin::from_coins(5));
        assert!(b.checked_sub(a).is_none());
    }

    #[test]
    fn test_coin_display() {
        let c = NexusCoin::from_coins(42);
        assert_eq!(format!("{c}"), "42.000000 NXC");

        let c2 = NexusCoin::from_micro(1_500_000);
        assert_eq!(format!("{c2}"), "1.500000 NXC");
    }

    #[test]
    fn test_coin_overflow_protection() {
        let max = NexusCoin::from_micro(u64::MAX);
        assert!(max.checked_add(NexusCoin::from_micro(1)).is_none());
    }

    #[test]
    fn test_coin_fraction() {
        let c = NexusCoin::from_coins(100);
        let quarter = c.fraction(1, 4);
        assert_eq!(quarter, NexusCoin::from_coins(25));

        // Zero denominator returns ZERO
        assert_eq!(c.fraction(1, 0), NexusCoin::ZERO);
    }
}
