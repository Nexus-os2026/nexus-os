//! Visual Permission Dashboard backend — Phase 6.5
//!
//! Maps agent capabilities to human-readable permission categories with
//! risk levels, display names, and change history. Every toggle maps to
//! a real kernel capability grant. Every change is audited.

use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use crate::manifest::AgentManifest;
use crate::supervisor::AgentId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// TODO: Replace magic string with proper RBAC check against role registry
const ADMIN_ROLE: &str = "admin";

// ---------------------------------------------------------------------------
// Risk Level (re-uses concept from speculative.rs but scoped for permissions)
// ---------------------------------------------------------------------------

/// Risk level for a permission toggle — advisory, not blocking (except Critical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl PermissionRiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ---------------------------------------------------------------------------
// Permission + Category
// ---------------------------------------------------------------------------

/// A single toggleable permission mapped to a real capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub capability_key: String,
    pub display_name: String,
    pub description: String,
    pub risk_level: PermissionRiskLevel,
    pub enabled: bool,
    pub granted_by: String,
    pub granted_at: u64,
    pub can_user_toggle: bool,
}

/// A group of related permissions (e.g. "Filesystem", "Network").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCategory {
    pub id: String,
    pub display_name: String,
    pub icon: String,
    pub permissions: Vec<Permission>,
}

// ---------------------------------------------------------------------------
// Permission History
// ---------------------------------------------------------------------------

/// Action taken on a permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    Granted,
    Revoked,
    Escalated,
    Downgraded,
    LockedByAdmin,
    UnlockedByAdmin,
}

/// A single entry in the permission change history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionHistoryEntry {
    pub capability_key: String,
    pub action: PermissionAction,
    pub changed_by: String,
    pub timestamp: u64,
    pub reason: Option<String>,
}

/// A request from an agent to acquire a new capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub agent_id: String,
    pub requested_capability: String,
    pub reason: String,
    pub risk_level: PermissionRiskLevel,
    pub current_capabilities: Vec<String>,
    pub requested_capabilities: Vec<String>,
}

// ---------------------------------------------------------------------------
// Capability metadata registry
// ---------------------------------------------------------------------------

/// Static metadata about each known capability.
struct CapabilityMeta {
    display_name: &'static str,
    description: &'static str,
    risk_level: PermissionRiskLevel,
    category_id: &'static str,
}

fn capability_metadata() -> HashMap<&'static str, CapabilityMeta> {
    let mut map = HashMap::new();

    // Filesystem
    map.insert(
        "fs.read",
        CapabilityMeta {
            display_name: "Read files",
            description: "Allows the agent to read files from the filesystem",
            risk_level: PermissionRiskLevel::Medium,
            category_id: "filesystem",
        },
    );
    map.insert(
        "fs.write",
        CapabilityMeta {
            display_name: "Write files",
            description: "Allows the agent to create or modify files on the filesystem. Changes can be destructive.",
            risk_level: PermissionRiskLevel::High,
            category_id: "filesystem",
        },
    );

    // Network
    map.insert(
        "web.search",
        CapabilityMeta {
            display_name: "Web search",
            description: "Allows the agent to search the web for information",
            risk_level: PermissionRiskLevel::Medium,
            category_id: "network",
        },
    );
    map.insert(
        "web.read",
        CapabilityMeta {
            display_name: "Read web pages",
            description: "Allows the agent to fetch and read web page content",
            risk_level: PermissionRiskLevel::Medium,
            category_id: "network",
        },
    );

    // AI / LLM
    map.insert(
        "llm.query",
        CapabilityMeta {
            display_name: "Query AI model",
            description: "Allows the agent to send prompts to an AI language model. May incur costs and expose data.",
            risk_level: PermissionRiskLevel::Medium,
            category_id: "ai",
        },
    );

    // System
    map.insert(
        "process.exec",
        CapabilityMeta {
            display_name: "Execute processes",
            description: "Allows the agent to run system commands and processes. This is a powerful capability.",
            risk_level: PermissionRiskLevel::Critical,
            category_id: "system",
        },
    );
    map.insert(
        "audit.read",
        CapabilityMeta {
            display_name: "Read audit logs",
            description: "Allows the agent to read the audit trail and event history",
            risk_level: PermissionRiskLevel::Low,
            category_id: "system",
        },
    );

    // Social
    map.insert(
        "social.post",
        CapabilityMeta {
            display_name: "Post to social media",
            description: "Allows the agent to publish posts on social media platforms",
            risk_level: PermissionRiskLevel::High,
            category_id: "social",
        },
    );
    map.insert(
        "social.x.post",
        CapabilityMeta {
            display_name: "Post to X (Twitter)",
            description: "Allows the agent to publish posts on X (formerly Twitter)",
            risk_level: PermissionRiskLevel::High,
            category_id: "social",
        },
    );
    map.insert(
        "social.x.read",
        CapabilityMeta {
            display_name: "Read X (Twitter)",
            description: "Allows the agent to read posts and timelines on X",
            risk_level: PermissionRiskLevel::Low,
            category_id: "social",
        },
    );

    // Messaging
    map.insert(
        "messaging.send",
        CapabilityMeta {
            display_name: "Send messages",
            description:
                "Allows the agent to send messages via Telegram, WhatsApp, Discord, or Slack",
            risk_level: PermissionRiskLevel::High,
            category_id: "messaging",
        },
    );

    // RAG (Retrieval-Augmented Generation)
    map.insert(
        "rag.ingest",
        CapabilityMeta {
            display_name: "Ingest documents",
            description:
                "Allows the agent to ingest and index documents into the RAG knowledge base",
            risk_level: PermissionRiskLevel::Medium,
            category_id: "ai",
        },
    );
    map.insert(
        "rag.query",
        CapabilityMeta {
            display_name: "Query knowledge base",
            description: "Allows the agent to query the RAG knowledge base for relevant context",
            risk_level: PermissionRiskLevel::Low,
            category_id: "ai",
        },
    );

    // MCP (Model Context Protocol)
    map.insert(
        "mcp.call",
        CapabilityMeta {
            display_name: "MCP tool calls",
            description: "Allows the agent to invoke tools via the Model Context Protocol. MCP tools may access external services.",
            risk_level: PermissionRiskLevel::High,
            category_id: "system",
        },
    );

    // Desktop control
    map.insert(
        "computer.control",
        CapabilityMeta {
            display_name: "Desktop control",
            description: "Allows the agent to capture screenshots and simulate keyboard/mouse input on the desktop. Extremely powerful.",
            risk_level: PermissionRiskLevel::Critical,
            category_id: "desktop",
        },
    );

    map
}

/// Category display metadata.
struct CategoryMeta {
    display_name: &'static str,
    icon: &'static str,
    order: u8,
}

fn category_metadata() -> HashMap<&'static str, CategoryMeta> {
    let mut map = HashMap::new();
    map.insert(
        "filesystem",
        CategoryMeta {
            display_name: "Filesystem",
            icon: "folder",
            order: 0,
        },
    );
    map.insert(
        "network",
        CategoryMeta {
            display_name: "Network",
            icon: "globe",
            order: 1,
        },
    );
    map.insert(
        "ai",
        CategoryMeta {
            display_name: "AI / LLM",
            icon: "brain",
            order: 2,
        },
    );
    map.insert(
        "system",
        CategoryMeta {
            display_name: "System",
            icon: "shield",
            order: 3,
        },
    );
    map.insert(
        "social",
        CategoryMeta {
            display_name: "Social Media",
            icon: "share",
            order: 4,
        },
    );
    map.insert(
        "messaging",
        CategoryMeta {
            display_name: "Messaging",
            icon: "chat",
            order: 5,
        },
    );
    map.insert(
        "desktop",
        CategoryMeta {
            display_name: "Desktop Control",
            icon: "monitor",
            order: 6,
        },
    );
    map
}

// ---------------------------------------------------------------------------
// PermissionManager
// ---------------------------------------------------------------------------

/// Manages the permission view layer over kernel capabilities.
///
/// Builds human-readable permission categories from agent manifests,
/// tracks change history, and applies updates to the real capability set.
#[derive(Debug, Clone, Default)]
pub struct PermissionManager {
    /// Permission change history per agent.
    history: HashMap<AgentId, Vec<PermissionHistoryEntry>>,
    /// Locked capabilities per agent (cannot be toggled by user).
    locked: HashMap<AgentId, Vec<String>>,
    /// Pending capability requests per agent.
    pending_requests: HashMap<AgentId, Vec<CapabilityRequest>>,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl PermissionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build permission categories from an agent's current manifest.
    pub fn get_permissions(
        &self,
        agent_id: AgentId,
        manifest: &AgentManifest,
    ) -> Vec<PermissionCategory> {
        let cap_meta = capability_metadata();
        let cat_meta = category_metadata();

        // Group capabilities by category
        let mut categories: HashMap<&str, Vec<Permission>> = HashMap::new();

        // Add ALL known capabilities, marking enabled/disabled based on manifest
        for (cap_key, meta) in &cap_meta {
            let enabled = manifest.capabilities.contains(&cap_key.to_string());
            let locked_caps = self.locked.get(&agent_id);
            let is_locked = locked_caps.is_some_and(|caps| caps.contains(&cap_key.to_string()));

            let perm = Permission {
                capability_key: cap_key.to_string(),
                display_name: meta.display_name.to_string(),
                description: meta.description.to_string(),
                risk_level: meta.risk_level,
                enabled,
                granted_by: if enabled {
                    "manifest".to_string()
                } else {
                    String::new()
                },
                granted_at: current_timestamp(),
                can_user_toggle: !is_locked,
            };

            categories.entry(meta.category_id).or_default().push(perm);
        }

        // Sort permissions within each category by capability_key for deterministic output
        for perms in categories.values_mut() {
            perms.sort_by(|a, b| a.capability_key.cmp(&b.capability_key));
        }

        // Build category structs, sorted by order
        let mut result: Vec<PermissionCategory> = categories
            .into_iter()
            .map(|(cat_id, permissions)| {
                let meta = cat_meta.get(cat_id);
                PermissionCategory {
                    id: cat_id.to_string(),
                    display_name: meta.map_or(cat_id, |m| m.display_name).to_string(),
                    icon: meta.map_or("circle", |m| m.icon).to_string(),
                    permissions,
                }
            })
            .collect();

        result.sort_by_key(|c| cat_meta.get(c.id.as_str()).map_or(99, |m| m.order));

        result
    }

    /// Update a single permission for an agent — modifies the manifest and logs audit.
    ///
    /// Returns the updated manifest on success.
    #[allow(clippy::too_many_arguments)]
    pub fn update_permission(
        &mut self,
        agent_id: AgentId,
        manifest: &AgentManifest,
        capability_key: &str,
        enabled: bool,
        changed_by: &str,
        reason: Option<&str>,
        audit_trail: &mut AuditTrail,
    ) -> Result<AgentManifest, AgentError> {
        let cap_meta = capability_metadata();

        // Validate capability key
        if !cap_meta.contains_key(capability_key) {
            return Err(AgentError::CapabilityDenied(format!(
                "unknown capability: {capability_key}"
            )));
        }

        // Check if locked
        if let Some(locked_caps) = self.locked.get(&agent_id) {
            if locked_caps.contains(&capability_key.to_string()) {
                return Err(AgentError::SupervisorError(format!(
                    "capability '{capability_key}' is locked by admin"
                )));
            }
        }

        // Build updated capabilities
        let mut new_caps = manifest.capabilities.clone();
        let currently_enabled = new_caps.contains(&capability_key.to_string());

        if enabled == currently_enabled {
            // No change needed
            return Ok(manifest.clone());
        }

        // Check if Critical risk requires admin — for BOTH grant and revoke.
        // Non-admins must not be able to modify critical capabilities at all.
        let meta = &cap_meta[capability_key];
        if meta.risk_level == PermissionRiskLevel::Critical && changed_by != ADMIN_ROLE {
            return Err(AgentError::SupervisorError(format!(
                "capability '{capability_key}' has critical risk and requires admin approval"
            )));
        }

        if enabled {
            new_caps.push(capability_key.to_string());
        } else {
            new_caps.retain(|c| c != capability_key);
        }

        // Record history
        let action = if enabled {
            PermissionAction::Granted
        } else {
            PermissionAction::Revoked
        };

        let entry = PermissionHistoryEntry {
            capability_key: capability_key.to_string(),
            action: action.clone(),
            changed_by: changed_by.to_string(),
            timestamp: current_timestamp(),
            reason: reason.map(|s| s.to_string()),
        };
        self.history.entry(agent_id).or_default().push(entry);

        // Audit log
        audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "permission.changed",
                "capability": capability_key,
                "action": format!("{action:?}"),
                "old_value": currently_enabled,
                "new_value": enabled,
                "changed_by": changed_by,
                "reason": reason,
            }),
        )?;

        // Flush to distributed audit immediately — permission changes are
        // high-value events that should not wait for the batch threshold.
        audit_trail
            .flush_batcher()
            .map_err(|e| AgentError::SupervisorError(format!("audit flush failed: {e}")))?;

        // Build updated manifest
        let mut updated = manifest.clone();
        updated.capabilities = new_caps;
        Ok(updated)
    }

    /// Bulk update: apply multiple permission changes at once.
    pub fn bulk_update_permissions(
        &mut self,
        agent_id: AgentId,
        manifest: &AgentManifest,
        updates: &[(String, bool)],
        changed_by: &str,
        reason: Option<&str>,
        audit_trail: &mut AuditTrail,
    ) -> Result<AgentManifest, AgentError> {
        let mut current = manifest.clone();
        for (capability_key, enabled) in updates {
            current = self.update_permission(
                agent_id,
                &current,
                capability_key,
                *enabled,
                changed_by,
                reason,
                audit_trail,
            )?;
        }
        Ok(current)
    }

    /// Get permission change history for an agent.
    pub fn get_history(&self, agent_id: AgentId) -> Vec<PermissionHistoryEntry> {
        self.history.get(&agent_id).cloned().unwrap_or_default()
    }

    /// Lock a capability so it cannot be toggled by users.
    pub fn lock_capability(
        &mut self,
        agent_id: AgentId,
        capability_key: &str,
        audit_trail: &mut AuditTrail,
    ) {
        let locked = self.locked.entry(agent_id).or_default();
        if !locked.contains(&capability_key.to_string()) {
            locked.push(capability_key.to_string());

            self.history
                .entry(agent_id)
                .or_default()
                .push(PermissionHistoryEntry {
                    capability_key: capability_key.to_string(),
                    action: PermissionAction::LockedByAdmin,
                    changed_by: "admin".to_string(),
                    timestamp: current_timestamp(),
                    reason: None,
                });

            let _ = audit_trail.append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "permission.locked",
                    "capability": capability_key,
                    "by": "admin",
                }),
            );
        }
    }

    /// Unlock a capability for user toggling.
    pub fn unlock_capability(
        &mut self,
        agent_id: AgentId,
        capability_key: &str,
        audit_trail: &mut AuditTrail,
    ) {
        if let Some(locked) = self.locked.get_mut(&agent_id) {
            locked.retain(|c| c != capability_key);

            self.history
                .entry(agent_id)
                .or_default()
                .push(PermissionHistoryEntry {
                    capability_key: capability_key.to_string(),
                    action: PermissionAction::UnlockedByAdmin,
                    changed_by: "admin".to_string(),
                    timestamp: current_timestamp(),
                    reason: None,
                });

            let _ = audit_trail.append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "permission.unlocked",
                    "capability": capability_key,
                    "by": "admin",
                }),
            );
        }
    }

    /// Add a pending capability request from an agent.
    /// Returns an error if the agent_id is not a valid UUID.
    pub fn add_capability_request(&mut self, request: CapabilityRequest) -> Result<(), AgentError> {
        let agent_id = uuid::Uuid::parse_str(&request.agent_id).map_err(|_| {
            AgentError::CapabilityDenied(format!("invalid agent UUID: '{}'", request.agent_id))
        })?;
        self.pending_requests
            .entry(agent_id)
            .or_default()
            .push(request);
        Ok(())
    }

    /// Get pending capability requests for an agent.
    pub fn get_capability_requests(&self, agent_id: AgentId) -> Vec<CapabilityRequest> {
        self.pending_requests
            .get(&agent_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Clear a pending capability request (after approval/denial).
    pub fn clear_capability_request(&mut self, agent_id: AgentId, capability_key: &str) {
        if let Some(requests) = self.pending_requests.get_mut(&agent_id) {
            requests.retain(|r| r.requested_capability != capability_key);
        }
    }

    /// Generate "revoke all network" bulk update list.
    pub fn revoke_all_network_updates() -> Vec<(String, bool)> {
        vec![
            ("web.search".to_string(), false),
            ("web.read".to_string(), false),
        ]
    }

    /// Generate "read-only mode" bulk update list — revoke all writes, keep reads.
    pub fn read_only_mode_updates() -> Vec<(String, bool)> {
        vec![
            ("fs.write".to_string(), false),
            ("social.post".to_string(), false),
            ("social.x.post".to_string(), false),
            ("messaging.send".to_string(), false),
            ("process.exec".to_string(), false),
        ]
    }

    /// Generate "minimal mode" bulk update list — revoke everything except logging/audit.
    pub fn minimal_mode_updates() -> Vec<(String, bool)> {
        vec![
            ("fs.read".to_string(), false),
            ("fs.write".to_string(), false),
            ("web.search".to_string(), false),
            ("web.read".to_string(), false),
            ("llm.query".to_string(), false),
            ("process.exec".to_string(), false),
            ("social.post".to_string(), false),
            ("social.x.post".to_string(), false),
            ("social.x.read".to_string(), false),
            ("messaging.send".to_string(), false),
            // audit.read stays enabled
        ]
    }

    /// Remove all data for an agent: history, locks, and pending requests.
    pub fn remove_agent(&mut self, agent_id: &AgentId) {
        self.history.remove(agent_id);
        self.locked.remove(agent_id);
        self.pending_requests.remove(agent_id);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;
    use crate::manifest::AgentManifest;
    use uuid::Uuid;

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

    #[test]
    fn get_permissions_returns_all_categories() {
        let mgr = PermissionManager::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let categories = mgr.get_permissions(agent_id, &manifest);

        // Should have 7 categories
        assert_eq!(categories.len(), 7);
        let cat_ids: Vec<&str> = categories.iter().map(|c| c.id.as_str()).collect();
        assert!(cat_ids.contains(&"filesystem"));
        assert!(cat_ids.contains(&"network"));
        assert!(cat_ids.contains(&"ai"));
        assert!(cat_ids.contains(&"system"));
        assert!(cat_ids.contains(&"social"));
        assert!(cat_ids.contains(&"messaging"));
        assert!(cat_ids.contains(&"desktop"));
    }

    #[test]
    fn get_permissions_reflects_enabled_state() {
        let mgr = PermissionManager::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let categories = mgr.get_permissions(agent_id, &manifest);

        // Find AI category
        let ai = categories.iter().find(|c| c.id == "ai").unwrap();
        let llm_perm = ai
            .permissions
            .iter()
            .find(|p| p.capability_key == "llm.query")
            .unwrap();
        assert!(llm_perm.enabled);

        // Find filesystem category
        let fs = categories.iter().find(|c| c.id == "filesystem").unwrap();
        let read_perm = fs
            .permissions
            .iter()
            .find(|p| p.capability_key == "fs.read")
            .unwrap();
        assert!(read_perm.enabled);
        let write_perm = fs
            .permissions
            .iter()
            .find(|p| p.capability_key == "fs.write")
            .unwrap();
        assert!(!write_perm.enabled);
    }

    #[test]
    fn get_permissions_correct_risk_levels() {
        let mgr = PermissionManager::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();
        let categories = mgr.get_permissions(agent_id, &manifest);

        let fs = categories.iter().find(|c| c.id == "filesystem").unwrap();
        let read_perm = fs
            .permissions
            .iter()
            .find(|p| p.capability_key == "fs.read")
            .unwrap();
        assert_eq!(read_perm.risk_level, PermissionRiskLevel::Medium);
        let write_perm = fs
            .permissions
            .iter()
            .find(|p| p.capability_key == "fs.write")
            .unwrap();
        assert_eq!(write_perm.risk_level, PermissionRiskLevel::High);

        let sys = categories.iter().find(|c| c.id == "system").unwrap();
        let exec_perm = sys
            .permissions
            .iter()
            .find(|p| p.capability_key == "process.exec")
            .unwrap();
        assert_eq!(exec_perm.risk_level, PermissionRiskLevel::Critical);
    }

    #[test]
    fn update_permission_grants_capability() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        // Grant fs.write
        let updated = mgr
            .update_permission(
                agent_id, &manifest, "fs.write", true, "user", None, &mut audit,
            )
            .unwrap();

        assert!(updated.capabilities.contains(&"fs.write".to_string()));
    }

    #[test]
    fn update_permission_revokes_capability() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        // Revoke llm.query
        let updated = mgr
            .update_permission(
                agent_id,
                &manifest,
                "llm.query",
                false,
                "user",
                None,
                &mut audit,
            )
            .unwrap();

        assert!(!updated.capabilities.contains(&"llm.query".to_string()));
    }

    #[test]
    fn update_permission_logs_audit_event() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        let events_before = audit.events().len();
        let _ = mgr.update_permission(
            agent_id,
            &manifest,
            "fs.write",
            true,
            "user",
            Some("testing"),
            &mut audit,
        );

        assert!(audit.events().len() > events_before);
        let last = audit.events().last().unwrap();
        assert_eq!(last.event_type, EventType::UserAction);
        assert_eq!(last.payload["event_kind"], "permission.changed");
        assert_eq!(last.payload["capability"], "fs.write");
        assert_eq!(last.payload["new_value"], true);
    }

    #[test]
    fn update_permission_records_history() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        let _ = mgr.update_permission(
            agent_id, &manifest, "fs.write", true, "user", None, &mut audit,
        );

        let history = mgr.get_history(agent_id);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].capability_key, "fs.write");
        assert_eq!(history[0].action, PermissionAction::Granted);
        assert_eq!(history[0].changed_by, "user");
    }

    #[test]
    fn locked_permission_rejects_user_toggle() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        mgr.lock_capability(agent_id, "llm.query", &mut audit);

        let result = mgr.update_permission(
            agent_id,
            &manifest,
            "llm.query",
            false,
            "user",
            None,
            &mut audit,
        );
        assert!(result.is_err());
    }

    #[test]
    fn critical_permission_requires_admin() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        // User tries to enable process.exec (Critical)
        let result = mgr.update_permission(
            agent_id,
            &manifest,
            "process.exec",
            true,
            "user",
            None,
            &mut audit,
        );
        assert!(result.is_err());

        // Admin can enable it
        let result = mgr.update_permission(
            agent_id,
            &manifest,
            "process.exec",
            true,
            "admin",
            None,
            &mut audit,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn bulk_update_revoke_all_network() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest(); // has web.search enabled

        let updates = PermissionManager::revoke_all_network_updates();
        let updated = mgr
            .bulk_update_permissions(
                agent_id,
                &manifest,
                &updates,
                "user",
                Some("security lockdown"),
                &mut audit,
            )
            .unwrap();

        assert!(!updated.capabilities.contains(&"web.search".to_string()));
        assert!(!updated.capabilities.contains(&"web.read".to_string()));
    }

    #[test]
    fn read_only_mode_keeps_reads() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let mut manifest = test_manifest();
        manifest.capabilities.push("fs.write".to_string());

        let updates = PermissionManager::read_only_mode_updates();
        let updated = mgr
            .bulk_update_permissions(agent_id, &manifest, &updates, "user", None, &mut audit)
            .unwrap();

        assert!(updated.capabilities.contains(&"fs.read".to_string()));
        assert!(!updated.capabilities.contains(&"fs.write".to_string()));
    }

    #[test]
    fn no_change_returns_same_manifest() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        // llm.query is already enabled
        let updated = mgr
            .update_permission(
                agent_id,
                &manifest,
                "llm.query",
                true,
                "user",
                None,
                &mut audit,
            )
            .unwrap();

        assert_eq!(updated, manifest);
    }

    #[test]
    fn unknown_capability_rejected() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        let result = mgr.update_permission(
            agent_id,
            &manifest,
            "quantum.compute",
            true,
            "user",
            None,
            &mut audit,
        );
        assert!(result.is_err());
    }

    #[test]
    fn unlock_capability_allows_toggle() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        mgr.lock_capability(agent_id, "llm.query", &mut audit);
        mgr.unlock_capability(agent_id, "llm.query", &mut audit);

        let result = mgr.update_permission(
            agent_id,
            &manifest,
            "llm.query",
            false,
            "user",
            None,
            &mut audit,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn capability_request_lifecycle() {
        let mut mgr = PermissionManager::new();
        let agent_id = Uuid::new_v4();

        let request = CapabilityRequest {
            agent_id: agent_id.to_string(),
            requested_capability: "fs.write".to_string(),
            reason: "Need to save analysis results".to_string(),
            risk_level: PermissionRiskLevel::High,
            current_capabilities: vec!["fs.read".to_string()],
            requested_capabilities: vec!["fs.read".to_string(), "fs.write".to_string()],
        };

        mgr.add_capability_request(request).unwrap();
        let requests = mgr.get_capability_requests(agent_id);
        assert_eq!(requests.len(), 1);

        mgr.clear_capability_request(agent_id, "fs.write");
        let requests = mgr.get_capability_requests(agent_id);
        assert!(requests.is_empty());
    }

    #[test]
    fn locked_permission_shows_in_categories() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        mgr.lock_capability(agent_id, "llm.query", &mut audit);

        let categories = mgr.get_permissions(agent_id, &manifest);
        let ai = categories.iter().find(|c| c.id == "ai").unwrap();
        let llm = ai
            .permissions
            .iter()
            .find(|p| p.capability_key == "llm.query")
            .unwrap();
        assert!(!llm.can_user_toggle);
    }

    #[test]
    fn audit_integrity_preserved_after_permission_changes() {
        let mut mgr = PermissionManager::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let manifest = test_manifest();

        // Multiple changes
        let m1 = mgr
            .update_permission(
                agent_id, &manifest, "fs.write", true, "user", None, &mut audit,
            )
            .unwrap();
        let _ = mgr
            .update_permission(agent_id, &m1, "web.read", true, "user", None, &mut audit)
            .unwrap();
        mgr.lock_capability(agent_id, "fs.write", &mut audit);

        // Audit chain should still be valid
        assert!(audit.verify_integrity());
    }
}
