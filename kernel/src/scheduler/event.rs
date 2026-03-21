//! Internal event trigger — fires schedules in response to kernel events.

use super::error::SchedulerError;
use super::trigger::{EventKind, ScheduleId};
use tokio::sync::mpsc;

/// Manages event subscriptions and dispatches matching events to scheduled tasks.
pub struct EventTrigger {
    subscriptions: Vec<EventSubscription>,
    tx: mpsc::Sender<(ScheduleId, serde_json::Value)>,
}

struct EventSubscription {
    schedule_id: ScheduleId,
    event_kind: EventKind,
    /// Reserved for future JSONPath filtering on event data.
    #[allow(dead_code)]
    filter: Option<String>,
}

impl EventTrigger {
    pub fn new(tx: mpsc::Sender<(ScheduleId, serde_json::Value)>) -> Self {
        Self {
            subscriptions: Vec::new(),
            tx,
        }
    }

    /// Subscribe a schedule to an event kind.
    pub fn subscribe(
        &mut self,
        schedule_id: ScheduleId,
        event_kind: EventKind,
        filter: Option<String>,
    ) {
        self.subscriptions.push(EventSubscription {
            schedule_id,
            event_kind,
            filter,
        });
    }

    /// Remove all subscriptions for a given schedule.
    pub fn unsubscribe(&mut self, schedule_id: &ScheduleId) {
        self.subscriptions.retain(|s| s.schedule_id != *schedule_id);
    }

    /// Called by other kernel systems when events occur.
    /// Dispatches to all matching subscriptions.
    pub async fn emit(
        &self,
        event_kind: &EventKind,
        data: serde_json::Value,
    ) -> Result<u32, SchedulerError> {
        let mut dispatched = 0u32;
        for sub in &self.subscriptions {
            if matches_event(&sub.event_kind, event_kind) {
                self.tx
                    .send((sub.schedule_id, data.clone()))
                    .await
                    .map_err(|e| SchedulerError::ChannelClosed(e.to_string()))?;
                dispatched += 1;
            }
        }
        Ok(dispatched)
    }

    /// Returns the number of active subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Returns true if no subscriptions exist.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

/// Determine if a subscribed event kind matches an actual emitted event.
fn matches_event(subscribed: &EventKind, actual: &EventKind) -> bool {
    match (subscribed, actual) {
        (EventKind::FileChanged { path: p1 }, EventKind::FileChanged { path: p2 }) => {
            // Prefix match: subscribing to "/src" matches "/src/main.rs"
            p2.starts_with(p1.as_str())
        }
        (
            EventKind::FuelBelowThreshold {
                threshold_percent: t1,
            },
            EventKind::FuelBelowThreshold {
                threshold_percent: t2,
            },
        ) => t2 <= t1,
        (EventKind::AuditAnomaly, EventKind::AuditAnomaly) => true,
        (
            EventKind::AgentCompleted { agent_did: d1 },
            EventKind::AgentCompleted { agent_did: d2 },
        ) => d1 == d2 || d1 == "*",
        (
            EventKind::IntegrationReceived { provider: p1 },
            EventKind::IntegrationReceived { provider: p2 },
        ) => p1 == p2 || p1 == "*",
        (
            EventKind::GenomeEvolved { genome_id: g1 },
            EventKind::GenomeEvolved { genome_id: g2 },
        ) => g1 == g2 || g1 == "*",
        (EventKind::Custom { name: n1 }, EventKind::Custom { name: n2 }) => n1 == n2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_changed_prefix_match() {
        let sub = EventKind::FileChanged {
            path: "/src".to_string(),
        };
        let actual = EventKind::FileChanged {
            path: "/src/main.rs".to_string(),
        };
        assert!(matches_event(&sub, &actual));
    }

    #[test]
    fn file_changed_no_match() {
        let sub = EventKind::FileChanged {
            path: "/src".to_string(),
        };
        let actual = EventKind::FileChanged {
            path: "/lib/utils.rs".to_string(),
        };
        assert!(!matches_event(&sub, &actual));
    }

    #[test]
    fn fuel_threshold_match() {
        let sub = EventKind::FuelBelowThreshold {
            threshold_percent: 20.0,
        };
        let actual = EventKind::FuelBelowThreshold {
            threshold_percent: 15.0,
        };
        assert!(matches_event(&sub, &actual));
    }

    #[test]
    fn fuel_threshold_no_match() {
        let sub = EventKind::FuelBelowThreshold {
            threshold_percent: 10.0,
        };
        let actual = EventKind::FuelBelowThreshold {
            threshold_percent: 50.0,
        };
        assert!(!matches_event(&sub, &actual));
    }

    #[test]
    fn wildcard_agent_completed() {
        let sub = EventKind::AgentCompleted {
            agent_did: "*".to_string(),
        };
        let actual = EventKind::AgentCompleted {
            agent_did: "agent-123".to_string(),
        };
        assert!(matches_event(&sub, &actual));
    }

    #[test]
    fn exact_agent_completed() {
        let sub = EventKind::AgentCompleted {
            agent_did: "agent-123".to_string(),
        };
        let actual = EventKind::AgentCompleted {
            agent_did: "agent-456".to_string(),
        };
        assert!(!matches_event(&sub, &actual));
    }

    #[test]
    fn audit_anomaly_matches() {
        assert!(matches_event(
            &EventKind::AuditAnomaly,
            &EventKind::AuditAnomaly
        ));
    }

    #[test]
    fn custom_event_match() {
        let sub = EventKind::Custom {
            name: "deploy".to_string(),
        };
        let actual = EventKind::Custom {
            name: "deploy".to_string(),
        };
        assert!(matches_event(&sub, &actual));
    }

    #[test]
    fn custom_event_no_match() {
        let sub = EventKind::Custom {
            name: "deploy".to_string(),
        };
        let actual = EventKind::Custom {
            name: "rollback".to_string(),
        };
        assert!(!matches_event(&sub, &actual));
    }

    #[test]
    fn different_kinds_no_match() {
        let sub = EventKind::AuditAnomaly;
        let actual = EventKind::Custom {
            name: "test".to_string(),
        };
        assert!(!matches_event(&sub, &actual));
    }

    #[tokio::test]
    async fn emit_dispatches_to_matching_subs() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut trigger = EventTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.subscribe(id, EventKind::AuditAnomaly, None);

        let count = trigger
            .emit(
                &EventKind::AuditAnomaly,
                serde_json::json!({"detail": "test"}),
            )
            .await
            .unwrap();
        assert_eq!(count, 1);

        let (recv_id, _data) = rx.recv().await.unwrap();
        assert_eq!(recv_id, id);
    }

    #[tokio::test]
    async fn emit_skips_non_matching_subs() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = EventTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.subscribe(id, EventKind::AuditAnomaly, None);

        let count = trigger
            .emit(
                &EventKind::Custom {
                    name: "other".to_string(),
                },
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn subscribe_unsubscribe() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = EventTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.subscribe(id, EventKind::AuditAnomaly, None);
        assert_eq!(trigger.len(), 1);
        trigger.unsubscribe(&id);
        assert!(trigger.is_empty());
    }
}
