//! Self-improvement layer for governed strategy adaptation.

pub mod adapter;
pub mod authority;
pub mod preferences;
pub mod suggestions;
pub mod tracker;

use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyDocument {
    pub posting_times: Vec<String>,
    pub content_style: String,
    pub hashtags: Vec<String>,
    pub platforms: Vec<String>,
    pub weekly_budget: u64,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    pub audit_level: String,
}

impl StrategyDocument {
    pub fn normalize(&mut self) {
        normalize_preserve_order(&mut self.posting_times);
        normalize_sorted(&mut self.hashtags);
        normalize_sorted(&mut self.platforms);
        normalize_sorted(&mut self.capabilities);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaptationError {
    RequiresApproval(String),
    NeverAllowed(String),
    TrackerError(String),
    PreferencesError(String),
    KernelError(String),
}

impl Display for AdaptationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AdaptationError::RequiresApproval(reason) => {
                write!(f, "authority change requires user approval: {reason}")
            }
            AdaptationError::NeverAllowed(reason) => {
                write!(f, "never-allowed change rejected: {reason}")
            }
            AdaptationError::TrackerError(reason) => write!(f, "change tracker error: {reason}"),
            AdaptationError::PreferencesError(reason) => {
                write!(f, "preferences error: {reason}")
            }
            AdaptationError::KernelError(reason) => write!(f, "kernel error: {reason}"),
        }
    }
}

impl Error for AdaptationError {}

impl From<AgentError> for AdaptationError {
    fn from(value: AgentError) -> Self {
        AdaptationError::KernelError(value.to_string())
    }
}

fn normalize_preserve_order(values: &mut Vec<String>) {
    for value in values.iter_mut() {
        *value = value.trim().to_string();
    }
    values.retain(|value| !value.is_empty());
    let mut deduped = Vec::new();
    for value in values.iter() {
        if !deduped.contains(value) {
            deduped.push(value.clone());
        }
    }
    *values = deduped;
}

fn normalize_sorted(values: &mut Vec<String>) {
    normalize_preserve_order(values);
    values.sort();
}
