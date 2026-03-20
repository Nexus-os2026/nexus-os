//! `nexus-metering` — Billing & usage metering for Nexus OS.
//!
//! Tracks resource consumption by agent, workspace, user, and time period.
//! Supports cost estimation, aggregation, budget alerts, and CSV export.

pub mod aggregator;
pub mod collector;
pub mod cost;
pub mod error;
pub mod store;
pub mod types;

// Re-export key types at crate root.
pub use aggregator::{export_csv, MeteringAggregator};
pub use collector::UsageCollector;
pub use cost::CostRates;
pub use error::MeteringError;
pub use store::MeteringStore;
pub use types::{
    AgentUsageSummary, BudgetAlert, CostLineItem, GroupBy, ResourceType, TimePeriod, UsageRecord,
    UsageReport, UsageTrend,
};
