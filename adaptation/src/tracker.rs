use crate::{AdaptationError, StrategyDocument};
use nexus_kernel::audit::{AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyVersion {
    pub version: u32,
    pub strategy: StrategyDocument,
    pub summary: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyDiff {
    pub from_version: u32,
    pub to_version: u32,
    pub changes: Vec<String>,
}

pub struct ChangeTracker {
    agent_id: Uuid,
    versions: Vec<StrategyVersion>,
    audit_trail: AuditTrail,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl ChangeTracker {
    pub fn new(agent_id: Uuid, initial_strategy: StrategyDocument) -> Self {
        Self::with_clock(agent_id, initial_strategy, Box::new(current_unix_timestamp))
    }

    pub fn with_clock(
        agent_id: Uuid,
        mut initial_strategy: StrategyDocument,
        clock: Box<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        initial_strategy.normalize();
        let mut tracker = Self {
            agent_id,
            versions: Vec::new(),
            audit_trail: AuditTrail::new(),
            clock,
        };

        let initial_version = StrategyVersion {
            version: 1,
            strategy: initial_strategy,
            summary: "initial_strategy".to_string(),
            timestamp: (tracker.clock)(),
        };
        tracker.log_version_event(&initial_version, "created");
        tracker.versions.push(initial_version);
        tracker
    }

    pub fn current_version(&self) -> u32 {
        self.versions
            .last()
            .map(|version| version.version)
            .unwrap_or(0)
    }

    pub fn current_strategy(&self) -> Option<&StrategyDocument> {
        self.versions.last().map(|version| &version.strategy)
    }

    pub fn record_change(
        &mut self,
        mut strategy: StrategyDocument,
        summary: &str,
    ) -> Result<u32, AdaptationError> {
        strategy.normalize();
        let next_version = self.current_version().saturating_add(1);
        if next_version == 0 {
            return Err(AdaptationError::TrackerError(
                "version counter overflow".to_string(),
            ));
        }

        let version = StrategyVersion {
            version: next_version,
            strategy,
            summary: summary.to_string(),
            timestamp: (self.clock)(),
        };
        self.log_version_event(&version, "updated");
        self.versions.push(version);
        Ok(next_version)
    }

    pub fn rollback(&mut self, version: u32) -> Result<StrategyDocument, AdaptationError> {
        let snapshot = self
            .versions
            .iter()
            .find(|entry| entry.version == version)
            .map(|entry| entry.strategy.clone())
            .ok_or_else(|| {
                AdaptationError::TrackerError(format!("strategy version {version} not found"))
            })?;

        let summary = format!("rollback_to_v{version}");
        let _ = self.record_change(snapshot.clone(), summary.as_str())?;

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "strategy_rollback",
                "target_version": version,
                "new_version": self.current_version()
            }),
        )?;

        Ok(snapshot)
    }

    pub fn diff(
        &self,
        from_version: u32,
        to_version: u32,
    ) -> Result<StrategyDiff, AdaptationError> {
        let left = self
            .versions
            .iter()
            .find(|entry| entry.version == from_version)
            .ok_or_else(|| {
                AdaptationError::TrackerError(format!("strategy version {from_version} not found"))
            })?;
        let right = self
            .versions
            .iter()
            .find(|entry| entry.version == to_version)
            .ok_or_else(|| {
                AdaptationError::TrackerError(format!("strategy version {to_version} not found"))
            })?;

        let mut changes = Vec::new();
        if left.strategy.posting_times != right.strategy.posting_times {
            changes.push(format!(
                "posting_times: {:?} -> {:?}",
                left.strategy.posting_times, right.strategy.posting_times
            ));
        }
        if left.strategy.content_style != right.strategy.content_style {
            changes.push(format!(
                "content_style: {} -> {}",
                left.strategy.content_style, right.strategy.content_style
            ));
        }
        if left.strategy.hashtags != right.strategy.hashtags {
            changes.push(format!(
                "hashtags: {:?} -> {:?}",
                left.strategy.hashtags, right.strategy.hashtags
            ));
        }
        if left.strategy.platforms != right.strategy.platforms {
            changes.push(format!(
                "platforms: {:?} -> {:?}",
                left.strategy.platforms, right.strategy.platforms
            ));
        }
        if left.strategy.weekly_budget != right.strategy.weekly_budget {
            changes.push(format!(
                "weekly_budget: {} -> {}",
                left.strategy.weekly_budget, right.strategy.weekly_budget
            ));
        }
        if left.strategy.capabilities != right.strategy.capabilities {
            changes.push(format!(
                "capabilities: {:?} -> {:?}",
                left.strategy.capabilities, right.strategy.capabilities
            ));
        }
        if left.strategy.fuel_budget != right.strategy.fuel_budget {
            changes.push(format!(
                "fuel_budget: {} -> {}",
                left.strategy.fuel_budget, right.strategy.fuel_budget
            ));
        }
        if left.strategy.audit_level != right.strategy.audit_level {
            changes.push(format!(
                "audit_level: {} -> {}",
                left.strategy.audit_level, right.strategy.audit_level
            ));
        }

        Ok(StrategyDiff {
            from_version,
            to_version,
            changes,
        })
    }

    pub fn versions(&self) -> &[StrategyVersion] {
        &self.versions
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    fn log_version_event(&mut self, version: &StrategyVersion, action: &str) {
        if let Err(e) = self.audit_trail
            .append_event(
                self.agent_id,
                EventType::StateChange,
                json!({
                    "event": "strategy_version_changed",
                    "action": action,
                    "version": version.version,
                    "summary": version.summary,
                    "timestamp": version.timestamp
                }),
            ) {
            tracing::error!("Audit append failed: {e}");
        }
    }
}

fn current_unix_timestamp() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::ChangeTracker;
    use crate::StrategyDocument;
    use uuid::Uuid;

    fn strategy(time: &str, style: &str) -> StrategyDocument {
        StrategyDocument {
            posting_times: vec![time.to_string()],
            content_style: style.to_string(),
            hashtags: vec!["#nexusos".to_string()],
            platforms: vec!["x".to_string()],
            weekly_budget: 1_000,
            capabilities: vec!["social.x.post".to_string()],
            fuel_budget: 5_000,
            audit_level: "strict".to_string(),
        }
    }

    #[test]
    fn test_strategy_rollback() {
        let agent_id = Uuid::new_v4();
        let initial = strategy("9am", "tutorial");
        let mut tracker = ChangeTracker::new(agent_id, initial.clone());

        let v2 = tracker.record_change(strategy("12pm", "tutorial"), "adjust slot");
        assert!(v2.is_ok());
        let v3 = tracker.record_change(strategy("12pm", "thread"), "change style");
        assert!(v3.is_ok());
        let v4 = tracker.record_change(strategy("5pm", "thread"), "retime");
        assert!(v4.is_ok());

        let rolled_back = tracker.rollback(1);
        assert!(rolled_back.is_ok());
        if let Ok(rolled_back) = rolled_back {
            assert_eq!(rolled_back, initial);
        }

        assert!(tracker.current_version() >= 5);
        let current = tracker.current_strategy();
        assert!(current.is_some());
        if let Some(current) = current {
            assert_eq!(current, &initial);
        }
    }

    #[test]
    fn test_diff_between_versions() {
        let agent_id = Uuid::new_v4();
        let initial = strategy("9am", "tutorial");
        let mut tracker = ChangeTracker::new(agent_id, initial);
        let version_two = tracker.record_change(strategy("2pm", "thread"), "shift");
        assert!(version_two.is_ok());

        let diff = tracker.diff(1, 2);
        assert!(diff.is_ok());
        if let Ok(diff) = diff {
            assert!(diff
                .changes
                .iter()
                .any(|line| line.contains("posting_times")));
            assert!(diff
                .changes
                .iter()
                .any(|line| line.contains("content_style")));
        }
    }
}
