//! Budget accounting with saturating arithmetic.
//!
//! Every swarm run is bounded by a [`Budget`]. Provider invocations call
//! [`Budget::try_consume`] before executing; if any field would be exhausted,
//! the call is rejected with a typed error and the provider is never
//! contacted.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Upper bound on a swarm run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    /// Total tokens allowed across all provider calls.
    pub tokens: u64,
    /// Total cost in US cents allowed.
    pub cost_cents: u32,
    /// Wall-clock ceiling in milliseconds.
    pub wall_ms: u64,
    /// Maximum allowed sub-agent nesting depth. Phase 1 rejects `> 0`.
    pub subagent_depth: u8,
}

impl Budget {
    pub fn new(tokens: u64, cost_cents: u32, wall_ms: u64) -> Self {
        Self {
            tokens,
            cost_cents,
            wall_ms,
            subagent_depth: 0,
        }
    }

    /// Generous default useful for tests.
    pub fn unlimited_for_tests() -> Self {
        Self {
            tokens: u64::MAX,
            cost_cents: u32::MAX,
            wall_ms: u64::MAX,
            subagent_depth: 0,
        }
    }

    /// Attempt to deduct a cost from the budget.
    ///
    /// On success the budget is mutated in place. On failure nothing is
    /// mutated and the appropriate [`BudgetError`] is returned.
    pub fn try_consume(&mut self, cost: BudgetCost) -> Result<(), BudgetError> {
        if cost.tokens > self.tokens {
            return Err(BudgetError::TokensExhausted {
                requested: cost.tokens,
                remaining: self.tokens,
            });
        }
        if cost.cost_cents > self.cost_cents {
            return Err(BudgetError::CostExhausted {
                requested: cost.cost_cents,
                remaining: self.cost_cents,
            });
        }
        if cost.wall_ms > self.wall_ms {
            return Err(BudgetError::WallClockExhausted {
                requested: cost.wall_ms,
                remaining: self.wall_ms,
            });
        }
        // All checks passed — commit (saturating just in case).
        self.tokens = self.tokens.saturating_sub(cost.tokens);
        self.cost_cents = self.cost_cents.saturating_sub(cost.cost_cents);
        self.wall_ms = self.wall_ms.saturating_sub(cost.wall_ms);
        Ok(())
    }

    /// True when any single axis is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.tokens == 0 || self.cost_cents == 0 || self.wall_ms == 0
    }
}

/// A prospective deduction against a [`Budget`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BudgetCost {
    pub tokens: u64,
    pub cost_cents: u32,
    pub wall_ms: u64,
}

impl BudgetCost {
    pub fn tokens(n: u64) -> Self {
        Self {
            tokens: n,
            ..Self::default()
        }
    }
    pub fn cents(c: u32) -> Self {
        Self {
            cost_cents: c,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BudgetError {
    #[error("tokens exhausted: requested {requested}, remaining {remaining}")]
    TokensExhausted { requested: u64, remaining: u64 },
    #[error("cost exhausted: requested {requested}¢, remaining {remaining}¢")]
    CostExhausted { requested: u32, remaining: u32 },
    #[error("wall clock exhausted: requested {requested}ms, remaining {remaining}ms")]
    WallClockExhausted { requested: u64, remaining: u64 },
    #[error("sub-agent depth {requested} exceeds max {max}")]
    SubagentDepthExceeded { requested: u8, max: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_within_budget_succeeds() {
        let mut b = Budget::new(100, 50, 1000);
        assert!(b.try_consume(BudgetCost::tokens(30)).is_ok());
        assert_eq!(b.tokens, 70);
    }

    #[test]
    fn consume_over_tokens_fails_without_mutation() {
        let mut b = Budget::new(10, 50, 1000);
        let err = b.try_consume(BudgetCost::tokens(100)).unwrap_err();
        assert!(matches!(err, BudgetError::TokensExhausted { .. }));
        assert_eq!(b.tokens, 10, "budget must not mutate on failure");
    }

    #[test]
    fn consume_over_cost_fails_without_mutation() {
        let mut b = Budget::new(100, 5, 1000);
        let err = b.try_consume(BudgetCost::cents(10)).unwrap_err();
        assert!(matches!(err, BudgetError::CostExhausted { .. }));
        assert_eq!(b.cost_cents, 5);
    }

    #[test]
    fn exhausted_when_any_axis_zero() {
        let b = Budget::new(0, 100, 1000);
        assert!(b.is_exhausted());
        let b = Budget::new(100, 0, 1000);
        assert!(b.is_exhausted());
        let b = Budget::new(100, 100, 0);
        assert!(b.is_exhausted());
    }
}
