//! Compatibility data shared with the retained error API.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuelViolation {
    OverMonthlyCap,
    AnomalyDetected,
    ProviderTokenMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    Created,
    Starting,
    Running,
    Paused,
    Stopping,
    Stopped,
    Destroyed,
}

impl Display for AgentState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Created => write!(f, "Created"),
            AgentState::Starting => write!(f, "Starting"),
            AgentState::Running => write!(f, "Running"),
            AgentState::Paused => write!(f, "Paused"),
            AgentState::Stopping => write!(f, "Stopping"),
            AgentState::Stopped => write!(f, "Stopped"),
            AgentState::Destroyed => write!(f, "Destroyed"),
        }
    }
}
