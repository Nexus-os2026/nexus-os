//! Replay recorder — captures execution state into evidence bundles.

use super::evidence::{hash_json, GovernanceCheck, ReplayBundle, StateSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// In-progress bundle being recorded before finalization.
struct InProgressBundle {
    id: String,
    agent_id: String,
    action_type: String,
    pre_state: StateSnapshot,
    governance_checks: Vec<GovernanceCheck>,
    audit_events: Vec<serde_json::Value>,
    input: serde_json::Value,
    created_at: u64,
}

/// Summary of a replay bundle for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSummary {
    pub id: String,
    pub agent_id: String,
    pub action_type: String,
    pub created_at: u64,
    pub governance_passed: bool,
    pub bundle_hash: String,
}

/// Records agent actions into replay evidence bundles.
pub struct ReplayRecorder {
    bundles: Vec<ReplayBundle>,
    in_progress: HashMap<String, InProgressBundle>,
    recording: bool,
    max_bundles: usize,
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self::new(500)
    }
}

impl ReplayRecorder {
    pub fn new(max_bundles: usize) -> Self {
        Self {
            bundles: Vec::new(),
            in_progress: HashMap::new(),
            recording: false,
            max_bundles,
        }
    }

    pub fn start_recording(&mut self) {
        self.recording = true;
    }

    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Begin capturing a new action. Returns the bundle ID.
    ///
    /// Call this before the action executes to capture pre-state.
    #[allow(clippy::too_many_arguments)]
    pub fn capture_pre_state(
        &mut self,
        agent_id: &str,
        action_type: &str,
        capabilities: Vec<String>,
        fuel_remaining: u64,
        permissions: Vec<serde_json::Value>,
        active_model: Option<String>,
        input: serde_json::Value,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_secs();

        let pre_state = StateSnapshot {
            agent_capabilities: capabilities,
            fuel_remaining,
            filesystem_permissions: permissions,
            active_model,
            timestamp: now,
            custom_state: serde_json::Value::Null,
        };

        let bundle = InProgressBundle {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            action_type: action_type.to_string(),
            pre_state,
            governance_checks: Vec::new(),
            audit_events: Vec::new(),
            input,
            created_at: now,
        };

        self.in_progress.insert(id.clone(), bundle);
        id
    }

    /// Record a governance check for an in-progress bundle.
    pub fn record_governance_check(
        &mut self,
        bundle_id: &str,
        check_type: &str,
        passed: bool,
        details: &str,
    ) {
        if let Some(bundle) = self.in_progress.get_mut(bundle_id) {
            bundle.governance_checks.push(GovernanceCheck {
                check_type: check_type.to_string(),
                passed,
                details: details.to_string(),
                timestamp: now_secs(),
            });
        }
    }

    /// Record an audit event for an in-progress bundle.
    pub fn record_audit_event(&mut self, bundle_id: &str, event: serde_json::Value) {
        if let Some(bundle) = self.in_progress.get_mut(bundle_id) {
            bundle.audit_events.push(event);
        }
    }

    /// Finalize a bundle with post-action state. Returns the completed bundle.
    pub fn capture_post_state(
        &mut self,
        bundle_id: &str,
        capabilities: Vec<String>,
        fuel_remaining: u64,
        permissions: Vec<serde_json::Value>,
        output: serde_json::Value,
    ) -> Result<ReplayBundle, String> {
        let in_progress = self
            .in_progress
            .remove(bundle_id)
            .ok_or_else(|| format!("no in-progress bundle with id '{bundle_id}'"))?;

        let now = now_secs();

        let post_state = StateSnapshot {
            agent_capabilities: capabilities,
            fuel_remaining,
            filesystem_permissions: permissions,
            active_model: None,
            timestamp: now,
            custom_state: serde_json::Value::Null,
        };

        let input_hash = hash_json(&in_progress.input);
        let output_hash = hash_json(&output);

        let bundle_hash = ReplayBundle::compute_hash(
            &in_progress.id,
            &in_progress.agent_id,
            &in_progress.action_type,
            &in_progress.pre_state,
            &post_state,
            &in_progress.governance_checks,
            &in_progress.audit_events,
            &input_hash,
            &output_hash,
        );

        let bundle = ReplayBundle {
            id: in_progress.id,
            created_at: in_progress.created_at,
            agent_id: in_progress.agent_id,
            action_type: in_progress.action_type,
            pre_state: in_progress.pre_state,
            post_state,
            governance_checks: in_progress.governance_checks,
            audit_events: in_progress.audit_events,
            input_hash,
            output_hash,
            bundle_hash,
            replay_verdict: None,
        };

        // Evict oldest if over limit
        if self.bundles.len() >= self.max_bundles {
            self.bundles.remove(0);
        }

        self.bundles.push(bundle.clone());
        Ok(bundle)
    }

    /// Get a stored bundle by ID.
    pub fn get_bundle(&self, id: &str) -> Option<&ReplayBundle> {
        self.bundles.iter().find(|b| b.id == id)
    }

    /// List bundles, optionally filtered by agent_id.
    pub fn list_bundles(&self, agent_id: Option<&str>, limit: usize) -> Vec<BundleSummary> {
        self.bundles
            .iter()
            .rev()
            .filter(|b| agent_id.is_none_or(|a| b.agent_id == a))
            .take(limit)
            .map(|b| BundleSummary {
                id: b.id.clone(),
                agent_id: b.agent_id.clone(),
                action_type: b.action_type.clone(),
                created_at: b.created_at,
                governance_passed: b.governance_checks.iter().all(|c| c.passed),
                bundle_hash: b.bundle_hash.clone(),
            })
            .collect()
    }

    /// Export a bundle as JSON for external audit.
    pub fn export_bundle(&self, id: &str) -> Result<String, String> {
        let bundle = self
            .get_bundle(id)
            .ok_or_else(|| format!("bundle '{id}' not found"))?;
        serde_json::to_string_pretty(bundle).map_err(|e| e.to_string())
    }

    /// Verify a stored bundle's integrity by recomputing its hash.
    pub fn verify_bundle_integrity(&self, id: &str) -> Result<bool, String> {
        let bundle = self
            .get_bundle(id)
            .ok_or_else(|| format!("bundle '{id}' not found"))?;
        Ok(bundle.verify_integrity())
    }

    /// Total number of stored bundles.
    pub fn bundle_count(&self) -> usize {
        self.bundles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_recorder(max: usize) -> ReplayRecorder {
        ReplayRecorder::new(max)
    }

    fn record_full_bundle(recorder: &mut ReplayRecorder, agent_id: &str) -> ReplayBundle {
        let bid = recorder.capture_pre_state(
            agent_id,
            "tool_call",
            vec!["fs.read".into(), "process.exec".into()],
            1000,
            vec![json!({"path": "/tmp", "mode": "read"})],
            Some("mock-llm".into()),
            json!({"command": "ls"}),
        );

        recorder.record_governance_check(&bid, "capability", true, "fs.read allowed");
        recorder.record_governance_check(&bid, "fuel", true, "1000 >= 2 (cost)");
        recorder.record_audit_event(&bid, json!({"event": "tool_call", "tool": "ls"}));

        recorder
            .capture_post_state(
                &bid,
                vec!["fs.read".into(), "process.exec".into()],
                998,
                vec![json!({"path": "/tmp", "mode": "read"})],
                json!({"stdout": "file1.txt\nfile2.txt", "exit_code": 0}),
            )
            .expect("capture_post_state should succeed")
    }

    #[test]
    fn test_record_full_bundle() {
        let mut recorder = make_recorder(100);
        recorder.start_recording();

        let bundle = record_full_bundle(&mut recorder, "agent-1");

        assert!(!bundle.id.is_empty());
        assert_eq!(bundle.agent_id, "agent-1");
        assert_eq!(bundle.action_type, "tool_call");
        assert_eq!(bundle.pre_state.fuel_remaining, 1000);
        assert_eq!(bundle.post_state.fuel_remaining, 998);
        assert_eq!(bundle.governance_checks.len(), 2);
        assert!(bundle.governance_checks.iter().all(|c| c.passed));
        assert_eq!(bundle.audit_events.len(), 1);
        assert!(!bundle.input_hash.is_empty());
        assert!(!bundle.output_hash.is_empty());
        assert!(!bundle.bundle_hash.is_empty());
        assert!(bundle.verify_integrity());
        assert_eq!(recorder.bundle_count(), 1);
    }

    #[test]
    fn test_bundle_hash_integrity() {
        let mut recorder = make_recorder(100);
        let bundle = record_full_bundle(&mut recorder, "agent-1");

        // Original should verify
        assert!(bundle.verify_integrity());

        // Tamper and verify fails
        let mut tampered = bundle.clone();
        tampered.output_hash = "tampered_hash".into();
        assert!(!tampered.verify_integrity());
    }

    #[test]
    fn test_bundle_hash_deterministic() {
        let mut recorder1 = make_recorder(100);
        let mut recorder2 = make_recorder(100);

        // Use fixed IDs for deterministic hashing
        let bid1 = recorder1.capture_pre_state(
            "a1",
            "test",
            vec!["cap1".into()],
            100,
            vec![],
            None,
            json!({"x": 1}),
        );
        recorder1
            .capture_post_state(&bid1, vec!["cap1".into()], 90, vec![], json!({"y": 2}))
            .unwrap();

        let bid2 = recorder2.capture_pre_state(
            "a1",
            "test",
            vec!["cap1".into()],
            100,
            vec![],
            None,
            json!({"x": 1}),
        );
        recorder2
            .capture_post_state(&bid2, vec!["cap1".into()], 90, vec![], json!({"y": 2}))
            .unwrap();

        let b1 = recorder1.get_bundle(&bid1).unwrap();
        let b2 = recorder2.get_bundle(&bid2).unwrap();

        // Input and output hashes should be deterministic
        assert_eq!(b1.input_hash, b2.input_hash);
        assert_eq!(b1.output_hash, b2.output_hash);
        // Bundle hashes differ because IDs are random UUIDs, but
        // the input/output hashes are the same for same data
    }

    #[test]
    fn test_list_filter_by_agent() {
        let mut recorder = make_recorder(100);

        record_full_bundle(&mut recorder, "agent-1");
        record_full_bundle(&mut recorder, "agent-2");
        record_full_bundle(&mut recorder, "agent-1");
        record_full_bundle(&mut recorder, "agent-3");

        let all = recorder.list_bundles(None, 100);
        assert_eq!(all.len(), 4);

        let agent1 = recorder.list_bundles(Some("agent-1"), 100);
        assert_eq!(agent1.len(), 2);
        assert!(agent1.iter().all(|b| b.agent_id == "agent-1"));

        let agent2 = recorder.list_bundles(Some("agent-2"), 100);
        assert_eq!(agent2.len(), 1);

        let none = recorder.list_bundles(Some("agent-99"), 100);
        assert!(none.is_empty());
    }

    #[test]
    fn test_export_bundle_json() {
        let mut recorder = make_recorder(100);
        let bundle = record_full_bundle(&mut recorder, "agent-1");

        let json = recorder.export_bundle(&bundle.id).unwrap();
        assert!(!json.is_empty());

        // Should be valid JSON that round-trips
        let parsed: ReplayBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, bundle.id);
        assert_eq!(parsed.bundle_hash, bundle.bundle_hash);
    }

    #[test]
    fn test_max_bundles_eviction() {
        let mut recorder = make_recorder(3);

        let b1 = record_full_bundle(&mut recorder, "a1");
        let _b2 = record_full_bundle(&mut recorder, "a2");
        let _b3 = record_full_bundle(&mut recorder, "a3");
        assert_eq!(recorder.bundle_count(), 3);

        // Adding a 4th should evict the oldest (b1)
        let _b4 = record_full_bundle(&mut recorder, "a4");
        assert_eq!(recorder.bundle_count(), 3);
        assert!(recorder.get_bundle(&b1.id).is_none());
    }

    #[test]
    fn test_recording_toggle() {
        let mut recorder = make_recorder(100);
        assert!(!recorder.is_recording());

        recorder.start_recording();
        assert!(recorder.is_recording());

        recorder.stop_recording();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_cannot_finalize_nonexistent() {
        let mut recorder = make_recorder(100);
        let result = recorder.capture_post_state("nonexistent", vec![], 0, vec![], json!(null));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no in-progress bundle"));
    }

    #[test]
    fn test_get_nonexistent_bundle() {
        let recorder = make_recorder(100);
        assert!(recorder.get_bundle("nonexistent").is_none());
    }

    #[test]
    fn test_export_nonexistent_bundle() {
        let recorder = make_recorder(100);
        let result = recorder.export_bundle("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_stored_bundle() {
        let mut recorder = make_recorder(100);
        let bundle = record_full_bundle(&mut recorder, "agent-1");
        assert!(recorder.verify_bundle_integrity(&bundle.id).unwrap());
    }
}
