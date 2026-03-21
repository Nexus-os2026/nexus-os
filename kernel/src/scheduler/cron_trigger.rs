//! Cron-based trigger engine.
//!
//! Uses the `cron` crate to parse expressions and compute next-fire times.
//! Runs a background tokio task that sleeps until the earliest next-fire.

use super::error::SchedulerError;
use super::trigger::ScheduleId;
use chrono::{DateTime, Utc};
use cron::Schedule;
use std::str::FromStr;
use tokio::sync::mpsc;

/// Manages cron-based schedule triggers.
pub struct CronTrigger {
    entries: Vec<CronEntry>,
    tx: mpsc::Sender<ScheduleId>,
}

struct CronEntry {
    id: ScheduleId,
    schedule: Schedule,
    next_fire: DateTime<Utc>,
}

impl CronTrigger {
    pub fn new(tx: mpsc::Sender<ScheduleId>) -> Self {
        Self {
            entries: Vec::new(),
            tx,
        }
    }

    /// Register a cron schedule. Returns the computed next-fire time.
    pub fn add_schedule(
        &mut self,
        id: ScheduleId,
        expression: &str,
        _timezone: &str,
    ) -> Result<DateTime<Utc>, SchedulerError> {
        let normalized = normalize_cron(expression)?;
        let schedule = Schedule::from_str(&normalized)
            .map_err(|e| SchedulerError::InvalidCron(format!("{expression}: {e}")))?;

        let next = schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| SchedulerError::NoNextFire(expression.to_string()))?;

        self.entries.push(CronEntry {
            id,
            schedule,
            next_fire: next,
        });

        Ok(next)
    }

    /// Remove a schedule by id.
    pub fn remove_schedule(&mut self, id: &ScheduleId) {
        self.entries.retain(|e| e.id != *id);
    }

    /// Compute the next fire time for a schedule entry without registering it.
    pub fn next_fire_time(expression: &str) -> Result<DateTime<Utc>, SchedulerError> {
        let normalized = normalize_cron(expression)?;
        let schedule = Schedule::from_str(&normalized)
            .map_err(|e| SchedulerError::InvalidCron(format!("{expression}: {e}")))?;
        schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| SchedulerError::NoNextFire(expression.to_string()))
    }

    /// Main loop — runs in a background tokio task.
    /// Fires schedule IDs through the channel when their cron time arrives.
    pub async fn run(&mut self) {
        loop {
            let now = Utc::now();

            for entry in &mut self.entries {
                if entry.next_fire <= now {
                    if let Err(e) = self.tx.send(entry.id).await {
                        eprintln!("[scheduler] cron send failed: {e}");
                    }
                    // Advance to next fire time
                    if let Some(next) = entry.schedule.upcoming(Utc).next() {
                        entry.next_fire = next;
                    }
                }
            }

            // Sleep until the earliest next fire, clamped to [1s, 60s]
            let sleep_dur = self
                .entries
                .iter()
                .map(|e| e.next_fire)
                .min()
                .map(|t| {
                    (t - Utc::now())
                        .to_std()
                        .unwrap_or(std::time::Duration::from_secs(1))
                })
                .unwrap_or(std::time::Duration::from_secs(60))
                .min(std::time::Duration::from_secs(60))
                .max(std::time::Duration::from_secs(1));

            tokio::time::sleep(sleep_dur).await;
        }
    }

    /// Returns the number of registered cron entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if there are no registered cron entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Normalize a 5-field cron expression to the 6/7-field format expected by the `cron` crate.
fn normalize_cron(expression: &str) -> Result<String, SchedulerError> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return Err(SchedulerError::InvalidCron("empty expression".to_string()));
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    match parts.len() {
        5 => Ok(format!("0 {trimmed}")),
        6 | 7 => Ok(trimmed.to_string()),
        n => Err(SchedulerError::InvalidCron(format!(
            "expected 5, 6, or 7 fields, got {n}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_5_field_cron() {
        let result = normalize_cron("*/5 * * * *").unwrap();
        assert_eq!(result, "0 */5 * * * *");
    }

    #[test]
    fn normalize_6_field_passthrough() {
        let result = normalize_cron("0 */5 * * * *").unwrap();
        assert_eq!(result, "0 */5 * * * *");
    }

    #[test]
    fn normalize_empty_fails() {
        assert!(normalize_cron("").is_err());
    }

    #[test]
    fn normalize_bad_field_count_fails() {
        assert!(normalize_cron("a b c").is_err());
    }

    #[test]
    fn next_fire_time_computes_future() {
        let next = CronTrigger::next_fire_time("* * * * *").unwrap();
        assert!(next > Utc::now());
    }

    #[test]
    fn add_schedule_returns_future_time() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = CronTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        let next = trigger.add_schedule(id, "*/10 * * * *", "UTC").unwrap();
        assert!(next > Utc::now());
        assert_eq!(trigger.len(), 1);
    }

    #[test]
    fn remove_schedule_works() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = CronTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.add_schedule(id, "*/10 * * * *", "UTC").unwrap();
        assert_eq!(trigger.len(), 1);
        trigger.remove_schedule(&id);
        assert!(trigger.is_empty());
    }

    #[test]
    fn invalid_cron_expression_rejected() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = CronTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        assert!(trigger.add_schedule(id, "not valid cron", "UTC").is_err());
    }
}
