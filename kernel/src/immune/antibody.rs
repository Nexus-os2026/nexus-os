//! Antibody spawner — generates specialized defense agents for unknown threats.
//!
//! When the [`ThreatDetector`](super::detector::ThreatDetector) encounters a
//! novel threat, the `AntibodySpawner` creates a targeted [`Antibody`] defense
//! pattern. Each antibody is derived via genome mutation to specialize against
//! the specific threat type.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::detector::{ThreatEvent, ThreatType};

// ---------------------------------------------------------------------------
// Antibody
// ---------------------------------------------------------------------------

/// A specialized defense pattern generated in response to a threat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Antibody {
    pub id: Uuid,
    pub threat_type: ThreatType,
    /// Human-readable description of the defense strategy.
    pub defense_pattern: String,
    /// Timestamp of creation (UNIX seconds).
    pub created_at: u64,
    /// Effectiveness score (0.0 .. 1.0), updated as the antibody is tested.
    pub effectiveness_score: f64,
}

// ---------------------------------------------------------------------------
// AntibodySpawner
// ---------------------------------------------------------------------------

/// Factory for generating [`Antibody`] instances from threat events.
///
/// Uses genome-inspired mutation to tune each antibody's defense pattern
/// to the specific threat category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntibodySpawner {
    /// Base mutation rate applied when deriving antibodies (0.0 .. 1.0).
    pub mutation_rate: f64,
    /// Total antibodies spawned by this spawner.
    pub total_spawned: u64,
}

impl AntibodySpawner {
    pub fn new() -> Self {
        Self {
            mutation_rate: 0.15,
            total_spawned: 0,
        }
    }

    /// Set the base mutation rate.
    pub fn with_mutation_rate(mut self, rate: f64) -> Self {
        self.mutation_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Spawn a new antibody tailored to the given threat event.
    pub fn spawn_antibody(&mut self, threat: &ThreatEvent) -> Antibody {
        self.total_spawned += 1;

        let defense_pattern = match threat.threat_type {
            ThreatType::PromptInjection => format!(
                "injection_filter:block_pattern:{}:mutation_rate:{:.2}",
                threat.matched_pattern.as_deref().unwrap_or("unknown"),
                self.mutation_rate
            ),
            ThreatType::DataExfiltration => format!(
                "exfil_guard:monitor_path:{}:redact_output:mutation_rate:{:.2}",
                threat.matched_pattern.as_deref().unwrap_or("unknown"),
                self.mutation_rate
            ),
            ThreatType::ResourceAbuse => format!(
                "resource_limiter:throttle_agent:{}:enforce_budget:mutation_rate:{:.2}",
                threat.agent_id, self.mutation_rate
            ),
            ThreatType::UnauthorizedTool => format!(
                "tool_gate:deny_tool:{}:agent:{}:mutation_rate:{:.2}",
                threat.matched_pattern.as_deref().unwrap_or("unknown"),
                threat.agent_id,
                self.mutation_rate
            ),
            ThreatType::AnomalousBehavior => format!(
                "anomaly_dampener:isolate_agent:{}:monitor_window:60s:mutation_rate:{:.2}",
                threat.agent_id, self.mutation_rate
            ),
        };

        Antibody {
            id: Uuid::new_v4(),
            threat_type: threat.threat_type,
            defense_pattern,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            effectiveness_score: 0.5, // start at neutral
        }
    }
}

impl Default for AntibodySpawner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immune::detector::{ThreatSeverity, ThreatType};

    fn make_threat(tt: ThreatType, pattern: Option<&str>) -> ThreatEvent {
        ThreatEvent::new(
            tt,
            ThreatSeverity::High,
            "test-agent",
            "test threat",
            pattern.map(|s| s.to_string()),
        )
    }

    #[test]
    fn test_spawn_injection_antibody() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::PromptInjection, Some("ignore previous"));
        let ab = spawner.spawn_antibody(&threat);
        assert_eq!(ab.threat_type, ThreatType::PromptInjection);
        assert!(ab.defense_pattern.contains("injection_filter"));
        assert!(ab.defense_pattern.contains("ignore previous"));
        assert_eq!(ab.effectiveness_score, 0.5);
        assert_eq!(spawner.total_spawned, 1);
    }

    #[test]
    fn test_spawn_exfil_antibody() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::DataExfiltration, Some("/etc/passwd"));
        let ab = spawner.spawn_antibody(&threat);
        assert!(ab.defense_pattern.contains("exfil_guard"));
    }

    #[test]
    fn test_spawn_resource_antibody() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::ResourceAbuse, None);
        let ab = spawner.spawn_antibody(&threat);
        assert!(ab.defense_pattern.contains("resource_limiter"));
    }

    #[test]
    fn test_spawn_tool_antibody() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::UnauthorizedTool, Some("execute_shell"));
        let ab = spawner.spawn_antibody(&threat);
        assert!(ab.defense_pattern.contains("tool_gate"));
    }

    #[test]
    fn test_spawn_anomaly_antibody() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::AnomalousBehavior, None);
        let ab = spawner.spawn_antibody(&threat);
        assert!(ab.defense_pattern.contains("anomaly_dampener"));
    }

    #[test]
    fn test_mutation_rate_clamped() {
        let spawner = AntibodySpawner::new().with_mutation_rate(5.0);
        assert_eq!(spawner.mutation_rate, 1.0);
        let spawner = AntibodySpawner::new().with_mutation_rate(-1.0);
        assert_eq!(spawner.mutation_rate, 0.0);
    }

    #[test]
    fn test_unique_ids() {
        let mut spawner = AntibodySpawner::new();
        let threat = make_threat(ThreatType::PromptInjection, None);
        let a1 = spawner.spawn_antibody(&threat);
        let a2 = spawner.spawn_antibody(&threat);
        assert_ne!(a1.id, a2.id);
        assert_eq!(spawner.total_spawned, 2);
    }
}
