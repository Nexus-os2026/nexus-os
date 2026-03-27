//! Background Agent Scheduler — agents that work while you sleep.
//!
//! Supports cron, interval, one-shot, webhook, and event triggers.
//! Every scheduled execution goes through the full governance pipeline:
//! capability check → fuel reservation → adversarial arena → HITL → execute → audit.

pub mod cron_trigger;
pub mod error;
pub mod event;
pub mod executor;
pub mod runner;
pub mod store;
pub mod trigger;
pub mod webhook;

pub use cron_trigger::CronTrigger;
pub use error::SchedulerError;
pub use event::EventTrigger;
pub use executor::{ExecutionResult, ScheduledExecutor};
pub use runner::{RunningScheduleStatus, ScheduleGoalCallback, ScheduleRunner};
pub use store::ScheduleStore;
pub use trigger::{
    EventKind, FailurePolicy, ScheduleEntry, ScheduleId, ScheduledTask, TriggerType,
};
pub use webhook::WebhookTrigger;
