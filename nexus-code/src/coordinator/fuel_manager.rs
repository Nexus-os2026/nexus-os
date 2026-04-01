//! Hierarchical fuel management for the coordinator.
//!
//! INVARIANT: coordinator_consumed + coordinator_reserved + total_allocated_to_children <= session_budget
//! Checked atomically on every allocation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A fuel slice allocated to a child agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelSlice {
    pub child_session_id: String,
    pub allocated: u64,
    pub consumed: u64,
    pub terminated: bool,
    pub termination_reason: Option<String>,
    pub successful_tool_count: u32,
    pub allocated_at: chrono::DateTime<chrono::Utc>,
}

impl FuelSlice {
    pub fn remaining(&self) -> u64 {
        self.allocated.saturating_sub(self.consumed)
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.allocated == 0 {
            return 100.0;
        }
        (self.consumed as f64 / self.allocated as f64) * 100.0
    }

    /// Runaway: >80% fuel consumed with 0 successful tools.
    pub fn is_runaway(&self) -> bool {
        self.usage_percentage() > 80.0 && self.successful_tool_count == 0
    }
}

/// Manages fuel allocation across coordinator and all child agents.
pub struct CoordinatorFuelManager {
    session_budget: u64,
    coordinator_consumed: u64,
    coordinator_reserved: u64,
    slices: HashMap<String, FuelSlice>,
}

impl CoordinatorFuelManager {
    pub fn new(session_budget: u64) -> Self {
        Self {
            session_budget,
            coordinator_consumed: 0,
            coordinator_reserved: 0,
            slices: HashMap::new(),
        }
    }

    /// Total fuel allocated to all active (non-terminated) children.
    pub fn total_allocated_to_children(&self) -> u64 {
        self.slices
            .values()
            .filter(|s| !s.terminated)
            .map(|s| s.allocated)
            .sum()
    }

    /// Total fuel consumed across all agents.
    pub fn total_consumed(&self) -> u64 {
        let children: u64 = self.slices.values().map(|s| s.consumed).sum();
        self.coordinator_consumed + children
    }

    /// Fuel available for new allocations.
    pub fn available_for_allocation(&self) -> u64 {
        let used = self.coordinator_consumed
            + self.coordinator_reserved
            + self.total_allocated_to_children();
        self.session_budget.saturating_sub(used)
    }

    /// Allocate a fuel slice to a child. Atomic: full amount or nothing.
    pub fn allocate(
        &mut self,
        child_session_id: &str,
        amount: u64,
    ) -> Result<(), crate::error::NxError> {
        if amount > self.available_for_allocation() {
            return Err(crate::error::NxError::FuelExhausted {
                remaining: self.available_for_allocation(),
                required: amount,
            });
        }

        if self.slices.contains_key(child_session_id) {
            return Err(crate::error::NxError::ConfigError(format!(
                "Fuel already allocated to child {}",
                child_session_id
            )));
        }

        self.slices.insert(
            child_session_id.to_string(),
            FuelSlice {
                child_session_id: child_session_id.to_string(),
                allocated: amount,
                consumed: 0,
                terminated: false,
                termination_reason: None,
                successful_tool_count: 0,
                allocated_at: chrono::Utc::now(),
            },
        );

        Ok(())
    }

    /// Update a child's consumed fuel and tool count.
    pub fn update_child_consumption(
        &mut self,
        child_session_id: &str,
        consumed: u64,
        successful_tools: u32,
    ) {
        if let Some(slice) = self.slices.get_mut(child_session_id) {
            slice.consumed = consumed;
            slice.successful_tool_count = successful_tools;
        }
    }

    /// Terminate a child agent.
    pub fn terminate_child(&mut self, child_session_id: &str, reason: &str) {
        if let Some(slice) = self.slices.get_mut(child_session_id) {
            slice.terminated = true;
            slice.termination_reason = Some(reason.to_string());
        }
    }

    /// Reclaim unused fuel from a completed/terminated child.
    pub fn reclaim_fuel(&mut self, child_session_id: &str) -> u64 {
        if let Some(slice) = self.slices.get_mut(child_session_id) {
            let unused = slice.remaining();
            slice.allocated = slice.consumed; // Shrink to actual
            slice.terminated = true;
            unused
        } else {
            0
        }
    }

    /// Record fuel consumed directly by the coordinator.
    pub fn record_coordinator_consumption(&mut self, amount: u64) {
        self.coordinator_consumed += amount;
    }

    /// Detect runaway children (>80% fuel, 0 successful tools, not terminated).
    pub fn detect_runaways(&self) -> Vec<String> {
        self.slices
            .values()
            .filter(|s| !s.terminated && s.is_runaway())
            .map(|s| s.child_session_id.clone())
            .collect()
    }

    pub fn slices(&self) -> &HashMap<String, FuelSlice> {
        &self.slices
    }

    pub fn session_budget(&self) -> u64 {
        self.session_budget
    }

    pub fn coordinator_consumed(&self) -> u64 {
        self.coordinator_consumed
    }

    pub fn summary(&self) -> String {
        let active = self.slices.values().filter(|s| !s.terminated).count();
        let child_consumed: u64 = self.slices.values().map(|s| s.consumed).sum();
        format!(
            "Budget: {}/{} | Coord: {} | Children: {} active, {}fu consumed | Available: {}",
            self.total_consumed(),
            self.session_budget,
            self.coordinator_consumed,
            active,
            child_consumed,
            self.available_for_allocation(),
        )
    }
}
