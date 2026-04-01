use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::NxError;

/// Summary of fuel budget status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelBudget {
    /// Total fuel units for this session.
    pub total: u64,
    /// Fuel consumed so far.
    pub consumed: u64,
    /// Fuel reserved for in-flight operations.
    pub reserved: u64,
    /// Estimated USD cost so far.
    pub cost_usd: f64,
}

/// Cost breakdown for a single LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelCost {
    /// Input tokens used.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Normalized fuel units (1 fuel ~ 1 token).
    pub fuel_units: u64,
    /// Estimated USD cost.
    pub estimated_usd: f64,
}

/// Fuel metering and budget enforcement.
pub struct FuelMeter {
    budget: FuelBudget,
    cost_history: Vec<(String, FuelCost, DateTime<Utc>)>,
}

impl FuelMeter {
    /// Create a new fuel meter with the given total budget.
    pub fn new(total_budget: u64) -> Self {
        Self {
            budget: FuelBudget {
                total: total_budget,
                consumed: 0,
                reserved: 0,
                cost_usd: 0.0,
            },
            cost_history: Vec::new(),
        }
    }

    /// Reserve fuel before an LLM call. Returns Err(FuelExhausted) if insufficient.
    pub fn reserve(&mut self, estimated_units: u64) -> Result<(), NxError> {
        if self.remaining() < estimated_units {
            return Err(NxError::FuelExhausted {
                remaining: self.remaining(),
                required: estimated_units,
            });
        }
        self.budget.reserved += estimated_units;
        Ok(())
    }

    /// Consume fuel after an LLM call completes. Releases the reservation
    /// and records actual consumption.
    pub fn consume(&mut self, provider: &str, actual: FuelCost) {
        if self.budget.reserved >= actual.fuel_units {
            self.budget.reserved -= actual.fuel_units;
        } else {
            self.budget.reserved = 0;
        }
        self.budget.consumed += actual.fuel_units;
        self.budget.cost_usd += actual.estimated_usd;
        self.cost_history
            .push((provider.to_string(), actual, Utc::now()));
    }

    /// Release a reservation (e.g., if the call was cancelled).
    pub fn release_reservation(&mut self, units: u64) {
        self.budget.reserved = self.budget.reserved.saturating_sub(units);
    }

    /// Get remaining fuel (total - consumed - reserved).
    pub fn remaining(&self) -> u64 {
        self.budget
            .total
            .saturating_sub(self.budget.consumed + self.budget.reserved)
    }

    /// Get the budget summary.
    pub fn budget(&self) -> &FuelBudget {
        &self.budget
    }

    /// Get cost history.
    pub fn cost_history(&self) -> &[(String, FuelCost, DateTime<Utc>)] {
        &self.cost_history
    }

    /// Check if budget is exhausted (remaining == 0).
    pub fn is_exhausted(&self) -> bool {
        self.remaining() == 0
    }

    /// Get percentage consumed (0.0 to 100.0).
    pub fn usage_percentage(&self) -> f64 {
        if self.budget.total == 0 {
            return 100.0;
        }
        (self.budget.consumed as f64 / self.budget.total as f64) * 100.0
    }
}
