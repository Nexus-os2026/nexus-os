//! Cross-platform computer control foundation for governed capture and input automation.

pub mod action_log;
pub mod capture;
pub mod input;

use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlAgentContext {
    pub agent_id: Uuid,
    pub capabilities: HashSet<String>,
}

impl ControlAgentContext {
    pub fn new(agent_id: Uuid, capabilities: HashSet<String>) -> Self {
        Self {
            agent_id,
            capabilities,
        }
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }
}
