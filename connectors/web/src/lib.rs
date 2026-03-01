//! Governed web intelligence connectors for search, content extraction, and X integration.

pub mod reader;
pub mod search;
pub mod twitter;

use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WebAgentContext {
    pub agent_id: Uuid,
    pub capabilities: HashSet<String>,
    pub fuel_remaining: u64,
}

impl WebAgentContext {
    pub fn new(agent_id: Uuid, capabilities: HashSet<String>, fuel_remaining: u64) -> Self {
        Self {
            agent_id,
            capabilities,
            fuel_remaining,
        }
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn consume_fuel(&mut self, amount: u64) -> bool {
        if self.fuel_remaining < amount {
            return false;
        }
        self.fuel_remaining -= amount;
        true
    }
}
