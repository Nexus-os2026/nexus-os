//! Immune system status dashboard — aggregated health at a glance.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ThreatLevel
// ---------------------------------------------------------------------------

/// Overall threat level of the system, inspired by DEFCON / biological
/// inflammation states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreatLevel {
    /// No active threats.
    Green,
    /// Minor threats detected, monitoring.
    Yellow,
    /// Significant threats, active countermeasures.
    Orange,
    /// Critical threats, emergency lockdown.
    Red,
}

// ---------------------------------------------------------------------------
// ImmuneStatus
// ---------------------------------------------------------------------------

/// Snapshot of the immune system's overall health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmuneStatus {
    /// Current threat level.
    pub threat_level: ThreatLevel,
    /// Number of active antibody defense patterns.
    pub active_antibodies: usize,
    /// Cumulative threats blocked since boot.
    pub threats_blocked: u64,
    /// UNIX timestamp of the last full scan.
    pub last_scan: u64,
    /// Cumulative privacy violations blocked since boot.
    pub privacy_violations_blocked: u64,
}

impl ImmuneStatus {
    /// Create a fresh status with no threats.
    pub fn new() -> Self {
        Self {
            threat_level: ThreatLevel::Green,
            active_antibodies: 0,
            threats_blocked: 0,
            last_scan: 0,
            privacy_violations_blocked: 0,
        }
    }

    /// Recompute the threat level based on recent activity.
    ///
    /// Heuristic:
    /// - Green: 0 threats blocked in the last window
    /// - Yellow: 1–5 threats
    /// - Orange: 6–20 threats
    /// - Red: >20 threats
    pub fn recompute_level(&mut self, recent_threats: u64) {
        self.threat_level = match recent_threats {
            0 => ThreatLevel::Green,
            1..=5 => ThreatLevel::Yellow,
            6..=20 => ThreatLevel::Orange,
            _ => ThreatLevel::Red,
        };
    }

    /// Record a blocked threat and update counters.
    pub fn record_threat_blocked(&mut self) {
        self.threats_blocked += 1;
    }

    /// Record a blocked privacy violation and update counters.
    pub fn record_privacy_blocked(&mut self, count: u64) {
        self.privacy_violations_blocked += count;
    }

    /// Update the last-scan timestamp to now.
    pub fn mark_scanned(&mut self) {
        self.last_scan = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

impl Default for ImmuneStatus {
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

    #[test]
    fn test_new_status_is_green() {
        let status = ImmuneStatus::new();
        assert_eq!(status.threat_level, ThreatLevel::Green);
        assert_eq!(status.active_antibodies, 0);
        assert_eq!(status.threats_blocked, 0);
    }

    #[test]
    fn test_recompute_levels() {
        let mut status = ImmuneStatus::new();

        status.recompute_level(0);
        assert_eq!(status.threat_level, ThreatLevel::Green);

        status.recompute_level(3);
        assert_eq!(status.threat_level, ThreatLevel::Yellow);

        status.recompute_level(15);
        assert_eq!(status.threat_level, ThreatLevel::Orange);

        status.recompute_level(50);
        assert_eq!(status.threat_level, ThreatLevel::Red);
    }

    #[test]
    fn test_record_threats() {
        let mut status = ImmuneStatus::new();
        status.record_threat_blocked();
        status.record_threat_blocked();
        assert_eq!(status.threats_blocked, 2);
    }

    #[test]
    fn test_record_privacy() {
        let mut status = ImmuneStatus::new();
        status.record_privacy_blocked(5);
        status.record_privacy_blocked(3);
        assert_eq!(status.privacy_violations_blocked, 8);
    }

    #[test]
    fn test_mark_scanned() {
        let mut status = ImmuneStatus::new();
        assert_eq!(status.last_scan, 0);
        status.mark_scanned();
        assert!(status.last_scan > 0);
    }

    #[test]
    fn test_default() {
        let status = ImmuneStatus::default();
        assert_eq!(status.threat_level, ThreatLevel::Green);
    }
}
