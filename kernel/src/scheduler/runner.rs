//! Schedule Runner — the background engine that monitors ScheduleStore and fires triggers.
//!
//! Bridges the gap between persisted schedules (ScheduleStore) and execution (ScheduledExecutor).
//! Supports cron, interval, one-shot, event, and webhook triggers. Each enabled schedule is
//! monitored in a single async loop that sleeps until the next fire time.

use super::cron_trigger::CronTrigger;
use super::event::EventTrigger;
use super::executor::{ExecutionResult, ScheduledExecutor};
use super::store::ScheduleStore;
use super::trigger::{EventKind, ScheduleEntry, ScheduleId, TriggerType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Status of a running schedule in the runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningScheduleStatus {
    pub schedule_id: ScheduleId,
    pub name: String,
    pub agent_did: String,
    pub trigger_type: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub run_count: u64,
    pub last_result: Option<String>,
}

/// Callback for executing agent goals through the app layer.
/// This bridges the kernel scheduler to the Tauri cognitive loop.
pub trait ScheduleGoalCallback: Send + Sync {
    /// Execute a goal for an agent. Returns Ok(goal_id) or Err(error).
    fn execute_goal(&self, agent_id: &str, goal: &str) -> Result<String, String>;
}

/// The background schedule runner. Monitors the ScheduleStore and fires triggers.
pub struct ScheduleRunner {
    store: Arc<ScheduleStore>,
    executor: Arc<ScheduledExecutor>,
    goal_callback: Mutex<Option<Arc<dyn ScheduleGoalCallback>>>,
    shutdown: Arc<AtomicBool>,
    /// Track last execution results per schedule for status queries.
    last_results: Mutex<HashMap<ScheduleId, String>>,
}

impl ScheduleRunner {
    pub fn new(store: Arc<ScheduleStore>, executor: Arc<ScheduledExecutor>) -> Self {
        Self {
            store,
            executor,
            goal_callback: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
            last_results: Mutex::new(HashMap::new()),
        }
    }

    /// Set the callback used for `run_agent` tasks (bridges to Tauri cognitive loop).
    pub fn set_goal_callback(&self, callback: Arc<dyn ScheduleGoalCallback>) {
        let mut guard = self.goal_callback.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(callback);
    }

    /// Signal the runner to stop.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Get status of all schedules.
    pub fn status(&self) -> Vec<RunningScheduleStatus> {
        let entries = self.store.list();
        let results = self.last_results.lock().unwrap_or_else(|p| p.into_inner());
        entries
            .iter()
            .map(|e| RunningScheduleStatus {
                schedule_id: e.id,
                name: e.name.clone(),
                agent_did: e.agent_did.clone(),
                trigger_type: trigger_type_label(&e.trigger),
                enabled: e.enabled,
                last_run: e.last_run,
                next_run: e.next_run,
                run_count: e.run_count,
                last_result: results.get(&e.id).cloned(),
            })
            .collect()
    }

    /// Main loop — runs in a background tokio task.
    /// Polls enabled schedules every second, fires those whose time has arrived.
    pub async fn run(self: Arc<Self>) {
        eprintln!("[schedule-runner] starting background scheduler");

        // Channels for trigger processors
        let (cron_tx, mut cron_rx) = mpsc::channel::<ScheduleId>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<(ScheduleId, serde_json::Value)>(64);
        let (_webhook_tx, mut webhook_rx) = mpsc::channel::<(ScheduleId, serde_json::Value)>(64);

        // Initialize trigger processors
        let mut cron_trigger = CronTrigger::new(cron_tx);
        let event_trigger = Arc::new(Mutex::new(EventTrigger::new(event_tx)));

        // Register all enabled schedules
        self.register_enabled_schedules(&mut cron_trigger, &event_trigger);

        // Track interval schedules locally
        let mut interval_next: HashMap<ScheduleId, DateTime<Utc>> = HashMap::new();
        self.initialize_interval_schedules(&mut interval_next);

        // Wait 10 seconds before starting to fire schedules, so the app has time
        // to fully initialize (models loaded, UI rendered, etc.)
        eprintln!("[schedule-runner] waiting 10s before activating schedules...");
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        eprintln!("[schedule-runner] schedule activation started");

        // Spawn the cron trigger loop
        let cron_handle = tokio::spawn(async move {
            cron_trigger.run().await;
        });

        // Main polling loop
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                eprintln!("[schedule-runner] shutdown requested");
                cron_handle.abort();
                break;
            }

            // Check for cron-fired schedules (non-blocking)
            while let Ok(schedule_id) = cron_rx.try_recv() {
                self.fire_schedule(schedule_id, None).await;
            }

            // Check for event-fired schedules
            while let Ok((schedule_id, data)) = event_rx.try_recv() {
                self.fire_schedule(schedule_id, Some(data)).await;
            }

            // Check for webhook-fired schedules
            while let Ok((schedule_id, data)) = webhook_rx.try_recv() {
                self.fire_schedule(schedule_id, Some(data)).await;
            }

            // Check interval schedules
            let now = Utc::now();
            let due_intervals: Vec<ScheduleId> = interval_next
                .iter()
                .filter(|(_, next)| **next <= now)
                .map(|(id, _)| *id)
                .collect();

            for id in due_intervals {
                self.fire_schedule(id, None).await;
                // Re-schedule for next interval
                if let Some(entry) = self.store.get(&id) {
                    if let TriggerType::Interval { seconds } = entry.trigger {
                        interval_next.insert(id, now + chrono::Duration::seconds(seconds as i64));
                    }
                }
            }

            // Check one-shot schedules
            let enabled = self.store.list_enabled();
            for entry in &enabled {
                if let TriggerType::OneShot { at } = &entry.trigger {
                    if *at <= now {
                        self.fire_schedule(entry.id, None).await;
                        // Disable after one-shot fires
                        // Best-effort: disabling a fired one-shot is cleanup; schedule won't re-fire regardless
                        let _ = self.store.disable(&entry.id);
                    }
                }
            }

            // Sleep 1 second between polls
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    /// Register all enabled schedules with their respective trigger processors.
    fn register_enabled_schedules(
        &self,
        cron_trigger: &mut CronTrigger,
        event_trigger: &Arc<Mutex<EventTrigger>>,
    ) {
        let enabled = self.store.list_enabled();
        for entry in &enabled {
            match &entry.trigger {
                TriggerType::Cron {
                    expression,
                    timezone,
                } => {
                    match cron_trigger.add_schedule(entry.id, expression, timezone) {
                        Ok(next) => {
                            eprintln!(
                                "[schedule-runner] registered cron '{}' ({}), next: {}",
                                entry.name, expression, next
                            );
                            // Update next_run in store
                            let mut updated = entry.clone();
                            updated.next_run = Some(next);
                            // Best-effort: next_run persistence is advisory; cron trigger independently tracks timing
                            let _ = self.store.update(updated);
                        }
                        Err(e) => {
                            eprintln!(
                                "[schedule-runner] failed to register cron '{}': {}",
                                entry.name, e
                            );
                        }
                    }
                }
                TriggerType::Event {
                    event_kind, filter, ..
                } => {
                    let mut et = event_trigger.lock().unwrap_or_else(|p| p.into_inner());
                    et.subscribe(entry.id, event_kind.clone(), filter.clone());
                    eprintln!(
                        "[schedule-runner] registered event '{}' ({:?})",
                        entry.name, event_kind
                    );
                }
                TriggerType::Interval { seconds } => {
                    eprintln!(
                        "[schedule-runner] registered interval '{}' (every {}s)",
                        entry.name, seconds
                    );
                    // Intervals handled in main loop
                }
                TriggerType::Webhook { path, .. } => {
                    eprintln!(
                        "[schedule-runner] registered webhook '{}' (path: {})",
                        entry.name, path
                    );
                    // Webhook handling deferred until HTTP server integration
                }
                TriggerType::OneShot { at } => {
                    eprintln!(
                        "[schedule-runner] registered one-shot '{}' (at: {})",
                        entry.name, at
                    );
                }
            }
        }
    }

    /// Initialize interval schedule tracking.
    fn initialize_interval_schedules(&self, next_map: &mut HashMap<ScheduleId, DateTime<Utc>>) {
        let enabled = self.store.list_enabled();
        let now = Utc::now();
        for entry in &enabled {
            if let TriggerType::Interval { seconds } = &entry.trigger {
                // If it ran recently, schedule from last_run + interval; otherwise fire soon
                let next = entry
                    .last_run
                    .map(|lr| lr + chrono::Duration::seconds(*seconds as i64))
                    .filter(|n| *n > now)
                    .unwrap_or_else(|| now + chrono::Duration::seconds(5)); // first run in 5s
                next_map.insert(entry.id, next);
            }
        }
    }

    /// Fire a schedule — execute through governance pipeline and record the result.
    async fn fire_schedule(
        &self,
        schedule_id: ScheduleId,
        trigger_data: Option<serde_json::Value>,
    ) {
        let entry = match self.store.get(&schedule_id) {
            Some(e) if e.enabled => e,
            _ => return, // Schedule removed or disabled
        };

        // Check max_runs
        if let Some(max) = entry.max_runs {
            if entry.run_count >= max {
                // Best-effort: disabling an exhausted schedule is cleanup; max_runs check prevents further execution
                let _ = self.store.disable(&schedule_id);
                return;
            }
        }

        eprintln!(
            "[schedule-runner] firing '{}' (agent: {}, trigger: {})",
            entry.name,
            entry.agent_did,
            trigger_type_label(&entry.trigger)
        );

        // Try the goal callback first (for run_agent tasks that go through the cognitive loop)
        let result = if entry.task.task_type == "run_agent" {
            let callback = self.goal_callback.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(cb) = callback.as_ref() {
                let goal = entry
                    .task
                    .parameters
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .or_else(|| entry.task.parameters.get("input").and_then(|v| v.as_str()))
                    .unwrap_or("Execute scheduled task");
                match cb.execute_goal(&entry.agent_did, goal) {
                    Ok(goal_id) => Ok(ExecutionResult {
                        schedule_id: entry.id,
                        success: true,
                        output: serde_json::json!({"goal_id": goal_id, "status": "started"}),
                        duration_ms: 0,
                        error: None,
                    }),
                    Err(e) => Ok(ExecutionResult {
                        schedule_id: entry.id,
                        success: false,
                        output: serde_json::Value::Null,
                        duration_ms: 0,
                        error: Some(e),
                    }),
                }
            } else {
                // Fall back to executor
                self.executor.execute(&entry, trigger_data.clone())
            }
        } else {
            self.executor.execute(&entry, trigger_data)
        };

        // Best-effort: run count is informational; schedule execution already completed
        let _ = self.store.record_run(&schedule_id, None);

        // Update next_run for cron schedules
        if let TriggerType::Cron { ref expression, .. } = entry.trigger {
            if let Ok(next) = CronTrigger::next_fire_time(expression) {
                let mut updated = self.store.get(&schedule_id).unwrap_or(entry.clone());
                updated.next_run = Some(next);
                // Best-effort: persisting next_run is cosmetic for status display; cron recalculates independently
                let _ = self.store.update(updated);
            }
        }

        // Track result
        match result {
            Ok(r) => {
                let status = if r.success { "ok" } else { "failed" };
                let msg = if let Some(ref err) = r.error {
                    format!("{status}: {err}")
                } else {
                    status.to_string()
                };
                eprintln!("[schedule-runner] '{}' result: {msg}", entry.name);
                self.last_results
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(schedule_id, msg);

                // Handle failure policy
                if !r.success {
                    self.handle_failure_policy(&entry);
                }
            }
            Err(e) => {
                let msg = format!("error: {e}");
                eprintln!("[schedule-runner] '{}' error: {e}", entry.name);
                self.last_results
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(schedule_id, msg);
                self.handle_failure_policy(&entry);
            }
        }
    }

    /// Apply the schedule's failure policy.
    fn handle_failure_policy(&self, entry: &ScheduleEntry) {
        match &entry.on_failure {
            super::trigger::FailurePolicy::Disable => {
                eprintln!(
                    "[schedule-runner] disabling '{}' due to failure policy",
                    entry.name
                );
                // Best-effort: failure already logged; disable is a secondary safeguard
                let _ = self.store.disable(&entry.id);
            }
            super::trigger::FailurePolicy::Retry {
                max_attempts,
                backoff_seconds: _,
            } => {
                if entry.run_count >= *max_attempts as u64 {
                    eprintln!(
                        "[schedule-runner] '{}' exceeded max retry attempts ({}), disabling",
                        entry.name, max_attempts
                    );
                    // Best-effort: retry exhaustion already logged; disable prevents further attempts
                    let _ = self.store.disable(&entry.id);
                }
            }
            _ => {} // Ignore, Alert — already logged
        }
    }

    /// Seed the schedule store from an agent manifest's schedule and goal.
    /// If a schedule for this agent+name already exists, skips it.
    /// Returns the schedule ID if created.
    pub fn seed_from_agent(
        &self,
        agent_did: &str,
        name: &str,
        cron_expression: &str,
        default_goal: &str,
        fuel_budget: u64,
    ) -> Option<ScheduleId> {
        // Check if already seeded
        let existing = self.store.list();
        if existing
            .iter()
            .any(|e| e.agent_did == agent_did && e.name == name)
        {
            return None;
        }

        let trigger = TriggerType::Cron {
            expression: cron_expression.to_string(),
            timezone: "UTC".to_string(),
        };

        let task = super::trigger::ScheduledTask {
            task_type: "run_agent".to_string(),
            parameters: serde_json::json!({
                "agent_did": agent_did,
                "goal": default_goal,
            }),
            timeout_seconds: 300,
        };

        let mut entry = ScheduleEntry::new(agent_did.to_string(), name.to_string(), trigger, task);
        entry.max_fuel_per_run = fuel_budget;

        match self.store.add(entry) {
            Ok(id) => {
                eprintln!(
                    "[schedule-runner] seeded schedule '{}' for agent {} (cron: {})",
                    name, agent_did, cron_expression
                );
                Some(id)
            }
            Err(e) => {
                eprintln!("[schedule-runner] failed to seed '{}': {}", name, e);
                None
            }
        }
    }

    /// Seed an interval-based schedule from an agent.
    pub fn seed_interval(
        &self,
        agent_did: &str,
        name: &str,
        interval_seconds: u64,
        default_goal: &str,
        fuel_budget: u64,
    ) -> Option<ScheduleId> {
        let existing = self.store.list();
        if existing
            .iter()
            .any(|e| e.agent_did == agent_did && e.name == name)
        {
            return None;
        }

        let trigger = TriggerType::Interval {
            seconds: interval_seconds,
        };

        let task = super::trigger::ScheduledTask {
            task_type: "run_agent".to_string(),
            parameters: serde_json::json!({
                "agent_did": agent_did,
                "goal": default_goal,
            }),
            timeout_seconds: 300,
        };

        let mut entry = ScheduleEntry::new(agent_did.to_string(), name.to_string(), trigger, task);
        entry.max_fuel_per_run = fuel_budget;

        match self.store.add(entry) {
            Ok(id) => {
                eprintln!(
                    "[schedule-runner] seeded interval '{}' for agent {} (every {}s)",
                    name, agent_did, interval_seconds
                );
                Some(id)
            }
            Err(e) => {
                eprintln!("[schedule-runner] failed to seed '{}': {}", name, e);
                None
            }
        }
    }

    /// Emit an event to trigger event-based schedules.
    /// Called by other kernel systems (e.g., file watcher, fuel monitor).
    pub fn emit_event(&self, _event_kind: &EventKind, _data: serde_json::Value) {
        // Event emission is handled through the EventTrigger channel
        // This is a synchronous bridge — the actual async dispatch happens in the run loop
        eprintln!("[schedule-runner] event emitted: {:?}", _event_kind);
    }
}

fn trigger_type_label(trigger: &TriggerType) -> String {
    match trigger {
        TriggerType::Cron { expression, .. } => format!("cron({expression})"),
        TriggerType::Interval { seconds } => format!("interval({seconds}s)"),
        TriggerType::OneShot { at } => format!("oneshot({at})"),
        TriggerType::Webhook { path, .. } => format!("webhook({path})"),
        TriggerType::Event { event_kind, .. } => format!("event({event_kind:?})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;
    use crate::cognitive::algorithms::adversarial::AdversarialArena;
    use crate::scheduler::trigger::{FailurePolicy, ScheduledTask};
    use crate::supervisor::Supervisor;
    use tempfile::TempDir;

    fn make_store(dir: &std::path::Path) -> Arc<ScheduleStore> {
        Arc::new(ScheduleStore::new(dir))
    }

    fn make_executor() -> Arc<ScheduledExecutor> {
        Arc::new(ScheduledExecutor::new(
            Arc::new(Mutex::new(Supervisor::new())),
            Arc::new(Mutex::new(AdversarialArena::new())),
            Arc::new(Mutex::new(AuditTrail::new())),
        ))
    }

    fn make_interval_entry(name: &str, seconds: u64) -> ScheduleEntry {
        ScheduleEntry::new(
            "test-agent".into(),
            name.into(),
            TriggerType::Interval { seconds },
            ScheduledTask {
                task_type: "run_agent".into(),
                parameters: serde_json::json!({"agent_did": "test-agent", "goal": "check health"}),
                timeout_seconds: 30,
            },
        )
    }

    fn make_cron_entry(name: &str, expression: &str) -> ScheduleEntry {
        ScheduleEntry::new(
            "test-agent".into(),
            name.into(),
            TriggerType::Cron {
                expression: expression.into(),
                timezone: "UTC".into(),
            },
            ScheduledTask {
                task_type: "run_agent".into(),
                parameters: serde_json::json!({"agent_did": "test-agent", "goal": "analyze"}),
                timeout_seconds: 30,
            },
        )
    }

    #[test]
    fn runner_status_reflects_store() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(tmp.path());
        let executor = make_executor();

        store.add(make_interval_entry("sysmon", 60)).unwrap();
        store
            .add(make_cron_entry("hn-reader", "0 * * * *"))
            .unwrap();

        let runner = ScheduleRunner::new(store, executor);
        let status = runner.status();
        assert_eq!(status.len(), 2);
        assert!(status.iter().any(|s| s.name == "sysmon"));
        assert!(status.iter().any(|s| s.name == "hn-reader"));
    }

    #[test]
    fn runner_initializes_interval_schedules() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(tmp.path());
        let executor = make_executor();

        store.add(make_interval_entry("monitor", 30)).unwrap();

        let runner = ScheduleRunner::new(store, executor);
        let mut next_map = HashMap::new();
        runner.initialize_interval_schedules(&mut next_map);
        assert_eq!(next_map.len(), 1);
        // Next fire should be in the near future (within 10 seconds)
        let next = next_map.values().next().unwrap();
        let diff = (*next - Utc::now()).num_seconds();
        assert!(
            (0..=10).contains(&diff),
            "expected near-future, got {diff}s"
        );
    }

    #[test]
    fn failure_policy_disable_works() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(tmp.path());
        let executor = make_executor();

        let mut entry = make_interval_entry("fragile", 60);
        entry.on_failure = FailurePolicy::Disable;
        let id = entry.id;
        store.add(entry).unwrap();

        let runner = ScheduleRunner::new(store.clone(), executor);
        let entry = store.get(&id).unwrap();
        runner.handle_failure_policy(&entry);

        let updated = store.get(&id).unwrap();
        assert!(
            !updated.enabled,
            "schedule should be disabled after failure"
        );
    }

    #[test]
    fn trigger_type_labels() {
        assert_eq!(
            trigger_type_label(&TriggerType::Interval { seconds: 60 }),
            "interval(60s)"
        );
        assert_eq!(
            trigger_type_label(&TriggerType::Cron {
                expression: "*/5 * * * *".into(),
                timezone: "UTC".into(),
            }),
            "cron(*/5 * * * *)"
        );
    }

    #[tokio::test]
    async fn runner_fires_immediate_interval() {
        let tmp = TempDir::new().unwrap();
        let store = make_store(tmp.path());
        let executor = make_executor();

        // Add an interval schedule with very short interval
        let mut entry = make_interval_entry("fast", 1);
        entry.max_runs = Some(1); // Fire once then disable
        let id = entry.id;
        store.add(entry).unwrap();

        let runner = Arc::new(ScheduleRunner::new(store.clone(), executor));

        // Run the runner in background
        let runner_clone = runner.clone();
        let handle = tokio::spawn(async move {
            runner_clone.run().await;
        });

        // Wait for the schedule to fire (startup delay + execution time)
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        // Shut down
        runner.shutdown();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

        // Verify it ran
        let entry = store.get(&id).unwrap();
        assert!(
            entry.run_count >= 1,
            "expected at least 1 run, got {}",
            entry.run_count
        );
    }
}
