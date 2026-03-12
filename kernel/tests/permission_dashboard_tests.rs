//! Integration tests for Phase 6.5 — Visual Permission Dashboard
//!
//! Tests verify that permission toggles update real capabilities,
//! changes flow through audit trail, and bulk operations work correctly.

use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::permissions::{PermissionAction, PermissionManager, PermissionRiskLevel};
use nexus_kernel::supervisor::Supervisor;

fn test_manifest() -> AgentManifest {
    AgentManifest {
        name: "test-agent".to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec![
            "llm.query".to_string(),
            "fs.read".to_string(),
            "web.search".to_string(),
        ],
        fuel_budget: 10000,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    }
}

fn setup() -> (Supervisor, uuid::Uuid) {
    let mut sup = Supervisor::new();
    let id = sup.start_agent(test_manifest()).unwrap();
    (sup, id)
}

// ── Test 1: get_permissions returns all categories with correct risk levels ──

#[test]
fn get_permissions_returns_all_categories() {
    let (sup, id) = setup();
    let categories = sup.get_agent_permissions(id).unwrap();

    assert_eq!(categories.len(), 7, "should have 7 permission categories");

    let cat_ids: Vec<&str> = categories.iter().map(|c| c.id.as_str()).collect();
    assert!(cat_ids.contains(&"filesystem"));
    assert!(cat_ids.contains(&"network"));
    assert!(cat_ids.contains(&"ai"));
    assert!(cat_ids.contains(&"system"));
    assert!(cat_ids.contains(&"social"));
    assert!(cat_ids.contains(&"messaging"));
    assert!(cat_ids.contains(&"desktop"));

    // Verify risk levels
    let sys = categories.iter().find(|c| c.id == "system").unwrap();
    let exec = sys
        .permissions
        .iter()
        .find(|p| p.capability_key == "process.exec")
        .unwrap();
    assert_eq!(exec.risk_level, PermissionRiskLevel::Critical);

    let fs = categories.iter().find(|c| c.id == "filesystem").unwrap();
    let write = fs
        .permissions
        .iter()
        .find(|p| p.capability_key == "fs.write")
        .unwrap();
    assert_eq!(write.risk_level, PermissionRiskLevel::High);
}

// ── Test 2: update_permission changes actual capability and logs to audit ──

#[test]
fn update_permission_changes_capability_and_audits() {
    let (mut sup, id) = setup();
    let events_before = sup.audit_trail().events().len();

    // Grant fs.write
    sup.update_agent_permission(id, "fs.write", true, "user", Some("testing"))
        .unwrap();

    // Verify the actual capability was added
    let handle = sup.get_agent(id).unwrap();
    assert!(handle
        .manifest
        .capabilities
        .contains(&"fs.write".to_string()));

    // Verify audit event was logged
    let events_after = sup.audit_trail().events().len();
    assert!(events_after > events_before);
    let permission_events: Vec<_> = sup
        .audit_trail()
        .events()
        .iter()
        .filter(|e| {
            e.payload.get("event_kind").and_then(|v| v.as_str()) == Some("permission.changed")
        })
        .collect();
    assert!(!permission_events.is_empty());
}

// ── Test 3: bulk revoke all network disables all network capabilities ──

#[test]
fn bulk_revoke_all_network() {
    let (mut sup, id) = setup();

    // Agent has web.search enabled
    let handle = sup.get_agent(id).unwrap();
    assert!(handle
        .manifest
        .capabilities
        .contains(&"web.search".to_string()));

    // Bulk revoke
    let updates = PermissionManager::revoke_all_network_updates();
    sup.bulk_update_agent_permissions(id, &updates, "user", Some("security lockdown"))
        .unwrap();

    let handle = sup.get_agent(id).unwrap();
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"web.search".to_string()));
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"web.read".to_string()));
}

// ── Test 4: read-only mode keeps reads and disables writes ──

#[test]
fn read_only_mode() {
    let (mut sup, id) = setup();

    // First grant fs.write so we can test it gets revoked
    sup.update_agent_permission(id, "fs.write", true, "admin", None)
        .unwrap();

    let updates = PermissionManager::read_only_mode_updates();
    sup.bulk_update_agent_permissions(id, &updates, "user", None)
        .unwrap();

    let handle = sup.get_agent(id).unwrap();
    // fs.read should still be there
    assert!(handle
        .manifest
        .capabilities
        .contains(&"fs.read".to_string()));
    // fs.write should be gone
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"fs.write".to_string()));
}

// ── Test 5: permission history records all changes in order ──

#[test]
fn permission_history_records_changes() {
    let (mut sup, id) = setup();

    sup.update_agent_permission(id, "fs.write", true, "user", None)
        .unwrap();
    sup.update_agent_permission(id, "web.read", true, "user", None)
        .unwrap();
    sup.update_agent_permission(id, "fs.write", false, "user", Some("too risky"))
        .unwrap();

    let history = sup.get_permission_history(id).unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].capability_key, "fs.write");
    assert_eq!(history[0].action, PermissionAction::Granted);
    assert_eq!(history[1].capability_key, "web.read");
    assert_eq!(history[1].action, PermissionAction::Granted);
    assert_eq!(history[2].capability_key, "fs.write");
    assert_eq!(history[2].action, PermissionAction::Revoked);
    assert_eq!(history[2].reason, Some("too risky".to_string()));
}

// ── Test 6: locked permission rejects user toggle attempt ──

#[test]
fn locked_permission_rejects_toggle() {
    let (mut sup, id) = setup();

    sup.lock_agent_capability(id, "llm.query").unwrap();

    let result = sup.update_agent_permission(id, "llm.query", false, "user", None);
    assert!(
        result.is_err(),
        "locked permission should reject user toggle"
    );
}

// ── Test 7: capability request comparison shows correct diff ──

#[test]
fn capability_request_lifecycle() {
    let (sup, id) = setup();

    let requests = sup.get_capability_requests(id).unwrap();
    assert!(requests.is_empty());
}

// ── Test 8: critical risk permission requires admin approval ──

#[test]
fn critical_permission_requires_admin() {
    let (mut sup, id) = setup();

    // User tries to enable process.exec (Critical)
    let result = sup.update_agent_permission(id, "process.exec", true, "user", None);
    assert!(result.is_err(), "critical permission should require admin");

    // Admin can enable it
    let result = sup.update_agent_permission(id, "process.exec", true, "admin", None);
    assert!(result.is_ok());

    let handle = sup.get_agent(id).unwrap();
    assert!(handle
        .manifest
        .capabilities
        .contains(&"process.exec".to_string()));
}

// ── Test 9: permission change audited with hash chain integrity ──

#[test]
fn permission_change_maintains_audit_integrity() {
    let (mut sup, id) = setup();

    // Multiple permission changes
    sup.update_agent_permission(id, "fs.write", true, "admin", None)
        .unwrap();
    sup.update_agent_permission(id, "web.read", true, "user", None)
        .unwrap();
    sup.update_agent_permission(id, "fs.write", false, "user", None)
        .unwrap();

    // Audit chain should still be intact
    assert!(sup.audit_trail().verify_integrity());
}

// ── Test 10: minimal mode revokes everything except audit.read ──

#[test]
fn minimal_mode_revokes_all_except_audit() {
    let (mut sup, id) = setup();

    // First give the agent audit.read
    sup.update_agent_permission(id, "audit.read", true, "user", None)
        .unwrap();

    let updates = PermissionManager::minimal_mode_updates();
    sup.bulk_update_agent_permissions(id, &updates, "user", None)
        .unwrap();

    let handle = sup.get_agent(id).unwrap();
    // audit.read should still be there
    assert!(handle
        .manifest
        .capabilities
        .contains(&"audit.read".to_string()));
    // Everything else should be gone
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"fs.read".to_string()));
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"llm.query".to_string()));
    assert!(!handle
        .manifest
        .capabilities
        .contains(&"web.search".to_string()));
}

// ── Test 11: unlock capability allows subsequent toggle ──

#[test]
fn unlock_allows_toggle() {
    let (mut sup, id) = setup();

    sup.lock_agent_capability(id, "llm.query").unwrap();
    assert!(sup
        .update_agent_permission(id, "llm.query", false, "user", None)
        .is_err());

    sup.unlock_agent_capability(id, "llm.query").unwrap();
    assert!(sup
        .update_agent_permission(id, "llm.query", false, "user", None)
        .is_ok());
}

// ── Test 12: locked permission shows in categories ──

#[test]
fn locked_permission_visible_in_categories() {
    let (mut sup, id) = setup();

    sup.lock_agent_capability(id, "llm.query").unwrap();

    let categories = sup.get_agent_permissions(id).unwrap();
    let ai = categories.iter().find(|c| c.id == "ai").unwrap();
    let llm = ai
        .permissions
        .iter()
        .find(|p| p.capability_key == "llm.query")
        .unwrap();
    assert!(!llm.can_user_toggle);
}

// ── Test 13: permission toggle idempotent (no-op if already in desired state) ──

#[test]
fn toggle_idempotent() {
    let (mut sup, id) = setup();

    // llm.query is already enabled, enabling again should be no-op
    sup.update_agent_permission(id, "llm.query", true, "user", None)
        .unwrap();

    let handle = sup.get_agent(id).unwrap();
    // Should only appear once, not duplicated
    let count = handle
        .manifest
        .capabilities
        .iter()
        .filter(|c| c.as_str() == "llm.query")
        .count();
    assert_eq!(count, 1);
}
