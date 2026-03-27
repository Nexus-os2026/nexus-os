//! Frontend visibility into oracle status — operational monitoring only.
//!
//! These types NEVER expose governance decision logic, approval rates,
//! denial reasons, or timing distributions.

use std::collections::HashMap;

use serde::Serialize;

/// Operational status summary (no decision details).
#[derive(Debug, Clone, Serialize)]
pub struct OracleStatusSummary {
    pub queue_depth: usize,
    pub response_ceiling_ms: u64,
    pub requests_processed: u64,
    pub uptime_seconds: u64,
}

/// Token verification result (no decision details).
#[derive(Debug, Clone, Serialize)]
pub struct TokenVerification {
    pub valid: bool,
    pub token_id: String,
    pub timestamp: u64,
}

/// Budget summary visible to agents (they should know remaining budget).
#[derive(Debug, Clone, Serialize)]
pub struct BudgetSummary {
    pub agent_id: String,
    pub allocations: HashMap<String, u64>,
    pub version: u64,
}
