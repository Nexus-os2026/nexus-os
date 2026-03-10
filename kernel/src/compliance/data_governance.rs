//! GDPR Article 17 agent data erasure and retention policy enforcement.
//!
//! Provides:
//! - [`AgentDataEraser`] — complete agent-level cryptographic erasure
//! - [`RetentionPolicy`] — per-data-class retention periods with legal hold

use crate::audit::{AuditTrail, EventType};
use crate::identity::agent_identity::IdentityManager;
use crate::permissions::PermissionManager;
use crate::privacy::PrivacyManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Erasure
// ---------------------------------------------------------------------------

/// Outcome of an agent data erasure operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureReceipt {
    /// Agent whose data was erased.
    pub agent_id: Uuid,
    /// Number of audit events redacted.
    pub events_redacted: usize,
    /// Encryption keys destroyed (cryptographic erasure).
    pub keys_destroyed: Vec<String>,
    /// Whether the agent identity was purged.
    pub identity_purged: bool,
    /// Whether permission history was purged.
    pub permissions_purged: bool,
    /// UUID of the compliance-proof audit event.
    pub proof_event_id: Uuid,
    /// Unix timestamp of the erasure.
    pub erased_at: u64,
}

/// Error returned when erasure cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErasureError {
    #[error("agent {0} is under legal hold — erasure prohibited")]
    LegalHold(Uuid),
    #[error("audit trail error: {0}")]
    AuditError(String),
    #[error("identity error: {0}")]
    IdentityError(String),
}

/// Performs complete agent-level data erasure per GDPR Article 17.
#[derive(Debug, Default)]
pub struct AgentDataEraser {
    /// Agent IDs under legal hold — erasure is prohibited.
    legal_holds: HashSet<Uuid>,
}

impl AgentDataEraser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Place an agent under legal hold, preventing any data erasure.
    pub fn set_legal_hold(&mut self, agent_id: Uuid) {
        self.legal_holds.insert(agent_id);
    }

    /// Remove a legal hold, allowing erasure to proceed.
    pub fn release_legal_hold(&mut self, agent_id: &Uuid) {
        self.legal_holds.remove(agent_id);
    }

    /// Check whether an agent is under legal hold.
    pub fn is_under_legal_hold(&self, agent_id: &Uuid) -> bool {
        self.legal_holds.contains(agent_id)
    }

    /// Erase all data for an agent.
    ///
    /// Steps:
    /// 1. Check legal hold — refuse if held
    /// 2. Redact agent audit events (replace payloads with erasure marker)
    /// 3. Destroy encryption keys via `PrivacyManager::erase_key()`
    /// 4. Purge agent identity from `IdentityManager`
    /// 5. Remove permission history from `PermissionManager`
    /// 6. Log a non-erasable compliance proof event
    ///
    /// The compliance proof event is logged under the system agent UUID
    /// (`Uuid::nil()`) so it is never itself subject to agent erasure.
    pub fn erase_agent_data(
        &self,
        agent_id: Uuid,
        encryption_key_ids: &[String],
        audit_trail: &mut AuditTrail,
        privacy_manager: &mut PrivacyManager,
        identity_manager: &mut IdentityManager,
        permission_manager: &mut PermissionManager,
    ) -> Result<ErasureReceipt, ErasureError> {
        // 1. Legal hold check
        if self.legal_holds.contains(&agent_id) {
            return Err(ErasureError::LegalHold(agent_id));
        }

        let erased_at = current_timestamp();

        // 2. Redact audit events for this agent
        let events_redacted = self.redact_agent_events(agent_id, audit_trail);

        // 3. Cryptographic erasure — destroy encryption keys
        let mut keys_destroyed = Vec::new();
        for key_id in encryption_key_ids {
            privacy_manager.erase_key(key_id);
            keys_destroyed.push(key_id.clone());
        }

        // 4. Purge agent identity
        let identity_purged = identity_manager
            .remove(&agent_id)
            .map_err(|e| ErasureError::IdentityError(e.to_string()))?;

        // 5. Remove permission history
        permission_manager.remove_agent(&agent_id);
        let permissions_purged = true;

        // 6. Log compliance proof event (under system UUID, not the erased agent)
        let system_id = Uuid::nil();
        let proof_event_id = audit_trail
            .append_event(
                system_id,
                EventType::StateChange,
                json!({
                    "event": "gdpr.article17.erasure_completed",
                    "erased_agent_id": agent_id.to_string(),
                    "events_redacted": events_redacted,
                    "keys_destroyed": keys_destroyed.len(),
                    "identity_purged": identity_purged,
                    "permissions_purged": permissions_purged,
                    "erased_at": erased_at,
                }),
            )
            .map_err(|e| ErasureError::AuditError(e.to_string()))?;

        Ok(ErasureReceipt {
            agent_id,
            events_redacted,
            keys_destroyed,
            identity_purged,
            permissions_purged,
            proof_event_id,
            erased_at,
        })
    }

    /// Replace all audit event payloads for a given agent with a redaction marker.
    /// The hash chain is intentionally preserved (events stay but content is gone).
    fn redact_agent_events(&self, agent_id: Uuid, audit_trail: &mut AuditTrail) -> usize {
        let mut count = 0;
        for event in audit_trail.events_mut() {
            if event.agent_id == agent_id {
                event.payload = json!({
                    "redacted": true,
                    "reason": "GDPR Article 17 erasure",
                });
                count += 1;
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------

/// Data class identifier for retention policies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataClass {
    /// Agent audit events
    AuditEvents,
    /// Evidence bundles
    EvidenceBundles,
    /// Agent identity records
    AgentIdentity,
    /// Permission change history
    PermissionHistory,
    /// Custom data class
    Custom(String),
}

/// A single retention rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionRule {
    /// Data class this rule applies to.
    pub data_class: DataClass,
    /// Maximum age in seconds before data is eligible for purge.
    pub max_age_secs: u64,
}

/// Result of a retention enforcement check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionResult {
    /// Number of audit events purged.
    pub events_purged: usize,
    /// Agents skipped due to legal hold.
    pub agents_held: Vec<Uuid>,
    /// Timestamp of the check.
    pub checked_at: u64,
}

/// Configurable retention policy with per-data-class periods and legal hold.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    rules: HashMap<DataClass, u64>,
    /// Agent IDs under legal hold — their data is exempt from retention purge.
    legal_holds: HashSet<Uuid>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        let mut rules = HashMap::new();
        // Default: 365 days for audit events (matches config.rs)
        rules.insert(DataClass::AuditEvents, 365 * 24 * 3600);
        Self {
            rules,
            legal_holds: HashSet::new(),
        }
    }
}

impl RetentionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a retention period for a data class.
    pub fn set_retention(&mut self, data_class: DataClass, max_age_secs: u64) {
        self.rules.insert(data_class, max_age_secs);
    }

    /// Get the retention period for a data class.
    pub fn get_retention(&self, data_class: &DataClass) -> Option<u64> {
        self.rules.get(data_class).copied()
    }

    /// Place an agent under legal hold.
    pub fn set_legal_hold(&mut self, agent_id: Uuid) {
        self.legal_holds.insert(agent_id);
    }

    /// Release a legal hold.
    pub fn release_legal_hold(&mut self, agent_id: &Uuid) {
        self.legal_holds.remove(agent_id);
    }

    /// Check whether an agent is under legal hold.
    pub fn is_under_legal_hold(&self, agent_id: &Uuid) -> bool {
        self.legal_holds.contains(agent_id)
    }

    /// Return all configured rules.
    pub fn rules(&self) -> Vec<RetentionRule> {
        self.rules
            .iter()
            .map(|(data_class, max_age_secs)| RetentionRule {
                data_class: data_class.clone(),
                max_age_secs: *max_age_secs,
            })
            .collect()
    }

    /// Enforce retention policy on the audit trail.
    ///
    /// Purges (redacts) events older than the configured retention period,
    /// except for events belonging to agents under legal hold.
    pub fn check_retention(&self, audit_trail: &mut AuditTrail) -> RetentionResult {
        let now = current_timestamp();
        let max_age = self
            .rules
            .get(&DataClass::AuditEvents)
            .copied()
            .unwrap_or(u64::MAX);
        let cutoff = now.saturating_sub(max_age);

        let mut events_purged = 0;
        let mut agents_held: HashSet<Uuid> = HashSet::new();

        for event in audit_trail.events_mut() {
            // Skip already-redacted events
            if event
                .payload
                .get("redacted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                continue;
            }

            // Skip system events (Uuid::nil)
            if event.agent_id == Uuid::nil() {
                continue;
            }

            if event.timestamp < cutoff {
                if self.legal_holds.contains(&event.agent_id) {
                    agents_held.insert(event.agent_id);
                    continue;
                }

                event.payload = json!({
                    "redacted": true,
                    "reason": "retention_policy_expired",
                });
                events_purged += 1;
            }
        }

        RetentionResult {
            events_purged,
            agents_held: agents_held.into_iter().collect(),
            checked_at: now,
        }
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditTrail, EventType};
    use crate::identity::agent_identity::IdentityManager;
    use crate::permissions::PermissionManager;
    use crate::privacy::PrivacyManager;
    use serde_json::json;
    use uuid::Uuid;

    fn populate_trail(agent_id: Uuid, trail: &mut AuditTrail, count: usize) {
        for i in 0..count {
            trail
                .append_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({"tool": "test", "index": i}),
                )
                .unwrap();
        }
    }

    #[test]
    fn erasure_removes_all_agent_data() {
        let agent_id = Uuid::new_v4();
        let other_agent = Uuid::new_v4();
        let mut trail = AuditTrail::new();

        // Populate events for both agents
        populate_trail(agent_id, &mut trail, 5);
        populate_trail(other_agent, &mut trail, 3);

        let mut privacy = PrivacyManager::new();
        let key_id = format!("agent-key-{}", agent_id);
        let mut identity_mgr = IdentityManager::in_memory();
        identity_mgr.get_or_create(agent_id).unwrap();
        identity_mgr.get_or_create(other_agent).unwrap();

        let mut perm_mgr = PermissionManager::new();

        let eraser = AgentDataEraser::new();
        let receipt = eraser
            .erase_agent_data(
                agent_id,
                std::slice::from_ref(&key_id),
                &mut trail,
                &mut privacy,
                &mut identity_mgr,
                &mut perm_mgr,
            )
            .unwrap();

        // All 5 agent events were redacted
        assert_eq!(receipt.events_redacted, 5);
        assert_eq!(receipt.keys_destroyed, vec![key_id]);
        assert!(receipt.identity_purged);
        assert!(receipt.permissions_purged);

        // Verify agent events are redacted
        for event in trail.events() {
            if event.agent_id == agent_id {
                assert_eq!(event.payload["redacted"], true);
                assert_eq!(event.payload["reason"], "GDPR Article 17 erasure");
            }
        }

        // Other agent's events are untouched
        for event in trail.events() {
            if event.agent_id == other_agent {
                assert!(event.payload.get("tool").is_some());
            }
        }

        // Agent identity is gone
        assert!(identity_mgr.get(&agent_id).is_none());
        // Other agent still exists
        assert!(identity_mgr.get(&other_agent).is_some());
    }

    #[test]
    fn erasure_proof_event_logged() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        populate_trail(agent_id, &mut trail, 2);

        let mut privacy = PrivacyManager::new();
        let mut identity_mgr = IdentityManager::in_memory();
        let mut perm_mgr = PermissionManager::new();

        let eraser = AgentDataEraser::new();
        let receipt = eraser
            .erase_agent_data(
                agent_id,
                &[],
                &mut trail,
                &mut privacy,
                &mut identity_mgr,
                &mut perm_mgr,
            )
            .unwrap();

        // Find the proof event
        let proof_event = trail
            .events()
            .iter()
            .find(|e| e.event_id == receipt.proof_event_id)
            .expect("proof event must exist");

        assert_eq!(proof_event.agent_id, Uuid::nil()); // system agent
        assert_eq!(
            proof_event.payload["event"],
            "gdpr.article17.erasure_completed"
        );
        assert_eq!(proof_event.payload["erased_agent_id"], agent_id.to_string());
        assert_eq!(proof_event.payload["events_redacted"], 2);
    }

    #[test]
    fn legal_hold_prevents_erasure() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        populate_trail(agent_id, &mut trail, 3);

        let mut privacy = PrivacyManager::new();
        let mut identity_mgr = IdentityManager::in_memory();
        identity_mgr.get_or_create(agent_id).unwrap();
        let mut perm_mgr = PermissionManager::new();

        let mut eraser = AgentDataEraser::new();
        eraser.set_legal_hold(agent_id);

        let result = eraser.erase_agent_data(
            agent_id,
            &[],
            &mut trail,
            &mut privacy,
            &mut identity_mgr,
            &mut perm_mgr,
        );

        assert!(matches!(result, Err(ErasureError::LegalHold(_))));

        // Data is still intact
        assert!(identity_mgr.get(&agent_id).is_some());
        for event in trail.events() {
            if event.agent_id == agent_id {
                assert!(event.payload.get("tool").is_some());
            }
        }
    }

    #[test]
    fn retention_purges_old_data() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();

        // Manually create events with old timestamps
        populate_trail(agent_id, &mut trail, 5);

        // Age all events by setting timestamps to 0 (very old)
        for event in trail.events_mut() {
            event.timestamp = 0;
        }

        // Add one recent event
        trail
            .append_event(
                agent_id,
                EventType::StateChange,
                json!({"status": "recent"}),
            )
            .unwrap();

        let mut policy = RetentionPolicy::new();
        // Set very short retention: 1 second
        policy.set_retention(DataClass::AuditEvents, 1);

        let result = policy.check_retention(&mut trail);

        // The 5 old events should be purged, the recent one kept
        assert_eq!(result.events_purged, 5);

        // Verify recent event is intact
        let recent = trail.events().last().unwrap();
        assert_eq!(recent.payload["status"], "recent");
    }

    #[test]
    fn retention_respects_legal_hold() {
        let held_agent = Uuid::new_v4();
        let free_agent = Uuid::new_v4();
        let mut trail = AuditTrail::new();

        // Create old events for both agents
        populate_trail(held_agent, &mut trail, 3);
        populate_trail(free_agent, &mut trail, 3);

        // Age all events
        for event in trail.events_mut() {
            event.timestamp = 0;
        }

        let mut policy = RetentionPolicy::new();
        policy.set_retention(DataClass::AuditEvents, 1);
        policy.set_legal_hold(held_agent);

        let result = policy.check_retention(&mut trail);

        // Only the free agent's events purged
        assert_eq!(result.events_purged, 3);
        assert!(result.agents_held.contains(&held_agent));

        // Held agent's events intact
        for event in trail.events() {
            if event.agent_id == held_agent {
                assert!(event.payload.get("tool").is_some());
            }
        }

        // Free agent's events redacted
        for event in trail.events() {
            if event.agent_id == free_agent {
                assert_eq!(event.payload["redacted"], true);
                assert_eq!(event.payload["reason"], "retention_policy_expired");
            }
        }
    }

    #[test]
    fn legal_hold_release_allows_erasure() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        populate_trail(agent_id, &mut trail, 2);

        let mut privacy = PrivacyManager::new();
        let mut identity_mgr = IdentityManager::in_memory();
        let mut perm_mgr = PermissionManager::new();

        let mut eraser = AgentDataEraser::new();
        eraser.set_legal_hold(agent_id);

        // Should fail
        assert!(eraser
            .erase_agent_data(
                agent_id,
                &[],
                &mut trail,
                &mut privacy,
                &mut identity_mgr,
                &mut perm_mgr,
            )
            .is_err());

        // Release and retry
        eraser.release_legal_hold(&agent_id);
        let receipt = eraser
            .erase_agent_data(
                agent_id,
                &[],
                &mut trail,
                &mut privacy,
                &mut identity_mgr,
                &mut perm_mgr,
            )
            .unwrap();

        assert_eq!(receipt.events_redacted, 2);
    }

    #[test]
    fn retention_skips_already_redacted_events() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();

        // Create events and mark them redacted
        populate_trail(agent_id, &mut trail, 3);
        for event in trail.events_mut() {
            event.timestamp = 0;
            event.payload = json!({"redacted": true, "reason": "prior_erasure"});
        }

        let mut policy = RetentionPolicy::new();
        policy.set_retention(DataClass::AuditEvents, 1);

        let result = policy.check_retention(&mut trail);
        assert_eq!(result.events_purged, 0);
    }

    #[test]
    fn retention_skips_system_events() {
        let mut trail = AuditTrail::new();

        // System events (Uuid::nil) should never be purged
        trail
            .append_event(
                Uuid::nil(),
                EventType::StateChange,
                json!({"event": "system.boot"}),
            )
            .unwrap();

        // Age the event
        for event in trail.events_mut() {
            event.timestamp = 0;
        }

        let mut policy = RetentionPolicy::new();
        policy.set_retention(DataClass::AuditEvents, 1);

        let result = policy.check_retention(&mut trail);
        assert_eq!(result.events_purged, 0);

        // System event intact
        assert_eq!(trail.events()[0].payload["event"], "system.boot");
    }

    #[test]
    fn erasure_with_encryption_keys() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        let mut privacy = PrivacyManager::new();
        let mut identity_mgr = IdentityManager::in_memory();
        let mut perm_mgr = PermissionManager::new();

        // Register encryption keys
        let key1 = crate::privacy::UserKey {
            id: "key-1".to_string(),
            bytes: [1u8; 32],
        };
        let key2 = crate::privacy::UserKey {
            id: "key-2".to_string(),
            bytes: [2u8; 32],
        };

        // Encrypt some data
        let enc1 = privacy.encrypt_field(b"data-1", &key1).unwrap();
        let enc2 = privacy.encrypt_field(b"data-2", &key2).unwrap();

        let eraser = AgentDataEraser::new();
        let receipt = eraser
            .erase_agent_data(
                agent_id,
                &["key-1".to_string(), "key-2".to_string()],
                &mut trail,
                &mut privacy,
                &mut identity_mgr,
                &mut perm_mgr,
            )
            .unwrap();

        assert_eq!(receipt.keys_destroyed.len(), 2);

        // Decryption should fail for both keys
        assert!(privacy.decrypt_field(&enc1, &key1).is_err());
        assert!(privacy.decrypt_field(&enc2, &key2).is_err());
    }

    #[test]
    fn default_retention_is_365_days() {
        let policy = RetentionPolicy::new();
        let audit_retention = policy.get_retention(&DataClass::AuditEvents);
        assert_eq!(audit_retention, Some(365 * 24 * 3600));
    }

    #[test]
    fn custom_data_class_retention() {
        let mut policy = RetentionPolicy::new();
        policy.set_retention(DataClass::Custom("metrics".to_string()), 30 * 24 * 3600);

        assert_eq!(
            policy.get_retention(&DataClass::Custom("metrics".to_string())),
            Some(30 * 24 * 3600)
        );
    }
}
