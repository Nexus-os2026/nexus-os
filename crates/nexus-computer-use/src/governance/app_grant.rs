use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::app_registry::{AppCategory, AppInfo};
use crate::agent::action::AgentAction;
use crate::error::ComputerUseError;

/// Level of access granted to an app
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GrantLevel {
    /// Can screenshot but not interact
    ReadOnly,
    /// Can click and scroll, no typing
    Click,
    /// Can click, type, keyboard shortcuts
    Full,
    /// Specific permissions only
    Restricted,
}

impl std::fmt::Display for GrantLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::Click => write!(f, "Click"),
            Self::Full => write!(f, "Full"),
            Self::Restricted => write!(f, "Restricted"),
        }
    }
}

/// Specific permission for an app
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppPermission {
    Screenshot,
    MouseClick,
    MouseScroll,
    KeyboardType,
    KeyboardShortcut,
    DragDrop,
    /// Requires explicit grant
    CloseWindow,
    /// Requires explicit grant
    LaunchApp,
}

/// A cryptographic grant for app access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppGrant {
    /// Unique identifier
    pub id: String,
    /// Which app this grant applies to (WM_CLASS pattern)
    pub app_wm_class: String,
    /// Category of the app
    pub app_category: AppCategory,
    /// Level of access
    pub grant_level: GrantLevel,
    /// Specific permissions (used when grant_level is Restricted)
    pub permissions: Vec<AppPermission>,
    /// When the grant was created
    pub granted_at: DateTime<Utc>,
    /// Who granted access ("user" or session ID)
    pub granted_by: String,
    /// SHA-256 hash of grant details for audit trail
    pub audit_hash: String,
    /// Optional expiration
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the grant has been revoked
    pub revoked: bool,
}

impl AppGrant {
    /// Create a new grant with computed audit hash
    pub fn new(
        app_wm_class: &str,
        app_category: AppCategory,
        grant_level: GrantLevel,
        permissions: Vec<AppPermission>,
        granted_by: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        let granted_at = Utc::now();
        let audit_hash =
            compute_grant_hash(&id, app_wm_class, &grant_level, &granted_at, granted_by);

        Self {
            id,
            app_wm_class: app_wm_class.to_string(),
            app_category,
            grant_level,
            permissions,
            granted_at,
            granted_by: granted_by.to_string(),
            audit_hash,
            expires_at,
            revoked: false,
        }
    }

    /// Check if this grant is currently valid (not expired, not revoked)
    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(expires) = self.expires_at {
            if Utc::now() > expires {
                return false;
            }
        }
        true
    }

    /// Check if this grant allows a specific permission
    pub fn allows(&self, permission: &AppPermission) -> bool {
        if !self.is_valid() {
            return false;
        }
        match &self.grant_level {
            GrantLevel::Full => true,
            GrantLevel::Click => matches!(
                permission,
                AppPermission::Screenshot
                    | AppPermission::MouseClick
                    | AppPermission::MouseScroll
                    | AppPermission::DragDrop
            ),
            GrantLevel::ReadOnly => matches!(permission, AppPermission::Screenshot),
            GrantLevel::Restricted => self.permissions.contains(permission),
        }
    }
}

/// Compute a deterministic audit hash for a grant
pub fn compute_grant_hash(
    id: &str,
    wm_class: &str,
    level: &GrantLevel,
    timestamp: &DateTime<Utc>,
    granted_by: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update(wm_class.as_bytes());
    hasher.update(format!("{level}").as_bytes());
    hasher.update(timestamp.to_rfc3339().as_bytes());
    hasher.update(granted_by.as_bytes());
    hex::encode(hasher.finalize())
}

/// Manager for app grants — validates actions against active grants
pub struct AppGrantManager {
    grants: Vec<AppGrant>,
    /// Default grant level for unknown apps
    default_level: GrantLevel,
    /// Categories that get auto-granted
    auto_grant_categories: HashMap<AppCategory, GrantLevel>,
    /// Per-app action counters for session limits
    action_counts: HashMap<String, u32>,
    /// Max actions per app per session
    max_actions_per_app: u32,
}

impl AppGrantManager {
    /// Create a new grant manager with safe defaults
    pub fn new() -> Self {
        let mut auto_grant = HashMap::new();
        auto_grant.insert(AppCategory::Terminal, GrantLevel::Full);
        auto_grant.insert(AppCategory::Editor, GrantLevel::Full);
        auto_grant.insert(AppCategory::NexusOS, GrantLevel::Full);
        auto_grant.insert(AppCategory::Browser, GrantLevel::Click);
        auto_grant.insert(AppCategory::FileManager, GrantLevel::Click);
        auto_grant.insert(AppCategory::Communication, GrantLevel::ReadOnly);
        auto_grant.insert(AppCategory::System, GrantLevel::ReadOnly);
        auto_grant.insert(AppCategory::Unknown, GrantLevel::ReadOnly);

        Self {
            grants: Vec::new(),
            default_level: GrantLevel::ReadOnly,
            auto_grant_categories: auto_grant,
            action_counts: HashMap::new(),
            max_actions_per_app: 500,
        }
    }

    /// Create with custom max actions per app
    pub fn with_max_actions(mut self, max: u32) -> Self {
        self.max_actions_per_app = max;
        self
    }

    /// Add a grant
    pub fn add_grant(&mut self, grant: AppGrant) {
        self.grants.push(grant);
    }

    /// Revoke a grant by ID
    pub fn revoke_grant(&mut self, grant_id: &str) -> bool {
        for grant in &mut self.grants {
            if grant.id == grant_id {
                grant.revoked = true;
                return true;
            }
        }
        false
    }

    /// Get all active (non-revoked, non-expired) grants
    pub fn active_grants(&self) -> Vec<&AppGrant> {
        self.grants.iter().filter(|g| g.is_valid()).collect()
    }

    /// Find the grant for a given app (by WM_CLASS substring match)
    fn find_grant(&self, wm_class: &str) -> Option<&AppGrant> {
        let lower = wm_class.to_lowercase();
        self.grants
            .iter()
            .filter(|g| g.is_valid())
            .find(|g| lower.contains(&g.app_wm_class.to_lowercase()))
    }

    /// Get the effective grant level for an app (explicit grant > auto-grant > default)
    pub fn effective_level(&self, app: &AppInfo) -> GrantLevel {
        // Check explicit grants first
        if let Some(grant) = self.find_grant(&app.wm_class) {
            return grant.grant_level.clone();
        }
        // Check auto-grant by category
        if let Some(level) = self.auto_grant_categories.get(&app.category) {
            return level.clone();
        }
        // Default
        self.default_level.clone()
    }

    /// Determine what permission an AgentAction requires
    pub fn required_permission(action: &AgentAction) -> AppPermission {
        match action {
            AgentAction::Click { .. } | AgentAction::DoubleClick { .. } => {
                AppPermission::MouseClick
            }
            AgentAction::Scroll { .. } => AppPermission::MouseScroll,
            AgentAction::Type { .. } => AppPermission::KeyboardType,
            AgentAction::KeyPress { .. } => AppPermission::KeyboardShortcut,
            AgentAction::Drag { .. } => AppPermission::DragDrop,
            AgentAction::Screenshot | AgentAction::Wait { .. } | AgentAction::Done { .. } => {
                AppPermission::Screenshot
            }
        }
    }

    /// Check if an action is allowed for the currently focused app
    pub fn validate_action(
        &mut self,
        focused_app: &AppInfo,
        action: &AgentAction,
    ) -> Result<String, ComputerUseError> {
        let required = Self::required_permission(action);

        // Screenshot, Wait, and Done are always allowed
        if matches!(
            action,
            AgentAction::Screenshot | AgentAction::Wait { .. } | AgentAction::Done { .. }
        ) {
            return Ok("implicit".to_string());
        }

        // Check per-app action limit
        let count = self
            .action_counts
            .get(&focused_app.wm_class)
            .copied()
            .unwrap_or(0);
        if count >= self.max_actions_per_app {
            return Err(ComputerUseError::CapabilityDenied {
                capability: format!(
                    "Max actions ({}) reached for app '{}'",
                    self.max_actions_per_app, focused_app.name
                ),
            });
        }

        // 1. Check explicit grant
        let explicit_result = self.find_grant(&focused_app.wm_class).map(|grant| {
            (
                grant.allows(&required),
                grant.id.clone(),
                grant.grant_level.clone(),
            )
        });
        if let Some((allowed, grant_id, grant_level)) = explicit_result {
            if allowed {
                *self
                    .action_counts
                    .entry(focused_app.wm_class.clone())
                    .or_insert(0) += 1;
                return Ok(grant_id);
            }
            return Err(ComputerUseError::CapabilityDenied {
                capability: format!(
                    "{:?} not allowed by grant '{}' (level: {}) for app '{}'",
                    required, grant_id, grant_level, focused_app.name
                ),
            });
        }

        // 2. Check auto-grant by category
        if let Some(level) = self.auto_grant_categories.get(&focused_app.category) {
            let auto_grant = AppGrant::new(
                &focused_app.wm_class,
                focused_app.category.clone(),
                level.clone(),
                Vec::new(),
                "auto",
                None,
            );
            if auto_grant.allows(&required) {
                *self
                    .action_counts
                    .entry(focused_app.wm_class.clone())
                    .or_insert(0) += 1;
                return Ok(format!("auto:{}", focused_app.category));
            }
            return Err(ComputerUseError::CapabilityDenied {
                capability: format!(
                    "{:?} not allowed by auto-grant (level: {}) for {} app '{}'",
                    required, level, focused_app.category, focused_app.name
                ),
            });
        }

        // 3. No grant at all — deny
        Err(ComputerUseError::CapabilityDenied {
            capability: format!(
                "No grant for app '{}' ({}). Request a grant first.",
                focused_app.name, focused_app.wm_class
            ),
        })
    }

    /// Reset action counters (e.g., at session start)
    pub fn reset_counters(&mut self) {
        self.action_counts.clear();
    }

    /// Get action count for an app
    pub fn action_count(&self, wm_class: &str) -> u32 {
        self.action_counts.get(wm_class).copied().unwrap_or(0)
    }
}

impl Default for AppGrantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app(name: &str, wm_class: &str, category: AppCategory) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            wm_class: wm_class.to_string(),
            pid: 1000,
            window_id: 0x0100_0001,
            title: format!("{name} Window"),
            category,
            is_focused: true,
        }
    }

    #[test]
    fn test_grant_creation() {
        let grant = AppGrant::new(
            "kitty",
            AppCategory::Terminal,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        assert!(!grant.id.is_empty());
        assert_eq!(grant.app_wm_class, "kitty");
        assert_eq!(grant.grant_level, GrantLevel::Full);
        assert!(!grant.revoked);
        assert!(grant.expires_at.is_none());
        assert_eq!(grant.audit_hash.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_grant_audit_hash_deterministic() {
        let ts = Utc::now();
        let h1 = compute_grant_hash("id1", "kitty", &GrantLevel::Full, &ts, "user");
        let h2 = compute_grant_hash("id1", "kitty", &GrantLevel::Full, &ts, "user");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);

        // Different inputs produce different hashes
        let h3 = compute_grant_hash("id2", "kitty", &GrantLevel::Full, &ts, "user");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_grant_not_expired() {
        let grant = AppGrant::new(
            "kitty",
            AppCategory::Terminal,
            GrantLevel::Full,
            Vec::new(),
            "user",
            Some(Utc::now() + chrono::Duration::hours(1)),
        );
        assert!(grant.is_valid());
    }

    #[test]
    fn test_grant_expired() {
        let mut grant = AppGrant::new(
            "kitty",
            AppCategory::Terminal,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        // Manually set expiry in the past
        grant.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(!grant.is_valid());
    }

    #[test]
    fn test_grant_revoked() {
        let mut grant = AppGrant::new(
            "kitty",
            AppCategory::Terminal,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        assert!(grant.is_valid());
        grant.revoked = true;
        assert!(!grant.is_valid());
    }

    #[test]
    fn test_grant_level_full_allows_all() {
        let grant = AppGrant::new(
            "kitty",
            AppCategory::Terminal,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        assert!(grant.allows(&AppPermission::Screenshot));
        assert!(grant.allows(&AppPermission::MouseClick));
        assert!(grant.allows(&AppPermission::MouseScroll));
        assert!(grant.allows(&AppPermission::KeyboardType));
        assert!(grant.allows(&AppPermission::KeyboardShortcut));
        assert!(grant.allows(&AppPermission::DragDrop));
        assert!(grant.allows(&AppPermission::CloseWindow));
        assert!(grant.allows(&AppPermission::LaunchApp));
    }

    #[test]
    fn test_grant_level_click_blocks_typing() {
        let grant = AppGrant::new(
            "firefox",
            AppCategory::Browser,
            GrantLevel::Click,
            Vec::new(),
            "user",
            None,
        );
        assert!(grant.allows(&AppPermission::Screenshot));
        assert!(grant.allows(&AppPermission::MouseClick));
        assert!(grant.allows(&AppPermission::MouseScroll));
        assert!(grant.allows(&AppPermission::DragDrop));
        assert!(!grant.allows(&AppPermission::KeyboardType));
        assert!(!grant.allows(&AppPermission::KeyboardShortcut));
        assert!(!grant.allows(&AppPermission::CloseWindow));
        assert!(!grant.allows(&AppPermission::LaunchApp));
    }

    #[test]
    fn test_grant_level_readonly_blocks_click() {
        let grant = AppGrant::new(
            "slack",
            AppCategory::Communication,
            GrantLevel::ReadOnly,
            Vec::new(),
            "user",
            None,
        );
        assert!(grant.allows(&AppPermission::Screenshot));
        assert!(!grant.allows(&AppPermission::MouseClick));
        assert!(!grant.allows(&AppPermission::MouseScroll));
        assert!(!grant.allows(&AppPermission::KeyboardType));
        assert!(!grant.allows(&AppPermission::DragDrop));
    }

    #[test]
    fn test_grant_level_restricted_custom_perms() {
        let grant = AppGrant::new(
            "custom-app",
            AppCategory::Unknown,
            GrantLevel::Restricted,
            vec![AppPermission::MouseClick, AppPermission::Screenshot],
            "user",
            None,
        );
        assert!(grant.allows(&AppPermission::MouseClick));
        assert!(grant.allows(&AppPermission::Screenshot));
        assert!(!grant.allows(&AppPermission::KeyboardType));
        assert!(!grant.allows(&AppPermission::DragDrop));
    }

    #[test]
    fn test_validate_action_granted() {
        let mut manager = AppGrantManager::new();
        let app = make_app("Kitty", "kitty", AppCategory::Terminal);
        let action = AgentAction::Type {
            text: "ls -la".to_string(),
        };
        // Terminal auto-grants Full access
        let result = manager.validate_action(&app, &action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_action_denied() {
        let mut manager = AppGrantManager::new();
        let app = make_app("Slack", "slack", AppCategory::Communication);
        let action = AgentAction::Type {
            text: "hello".to_string(),
        };
        // Communication auto-grants ReadOnly — typing denied
        let result = manager.validate_action(&app, &action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::CapabilityDenied { .. }
        ));
    }

    #[test]
    fn test_validate_action_no_grant_unknown_app() {
        let mut manager = AppGrantManager::new();
        let app = make_app("RandomApp", "randomapp", AppCategory::Unknown);
        let action = AgentAction::Click {
            x: 100,
            y: 200,
            button: "left".to_string(),
        };
        // Unknown auto-grants ReadOnly — clicking denied
        let result = manager.validate_action(&app, &action);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_grant_terminal() {
        let mut manager = AppGrantManager::new();
        let app = make_app("Terminal", "gnome-terminal", AppCategory::Terminal);

        // All actions should be allowed for Terminal (Full access)
        let click = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };
        assert!(manager.validate_action(&app, &click).is_ok());

        let typing = AgentAction::Type {
            text: "test".to_string(),
        };
        assert!(manager.validate_action(&app, &typing).is_ok());
    }

    #[test]
    fn test_auto_grant_editor() {
        let mut manager = AppGrantManager::new();
        let app = make_app("VS Code", "code", AppCategory::Editor);

        let typing = AgentAction::Type {
            text: "fn main()".to_string(),
        };
        assert!(manager.validate_action(&app, &typing).is_ok());
    }

    #[test]
    fn test_no_auto_grant_communication() {
        let mut manager = AppGrantManager::new();
        let app = make_app("Discord", "discord", AppCategory::Communication);

        // Typing should be denied (ReadOnly for communication)
        let typing = AgentAction::Type {
            text: "hello".to_string(),
        };
        assert!(manager.validate_action(&app, &typing).is_err());

        // Clicking should also be denied (ReadOnly)
        let click = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };
        assert!(manager.validate_action(&app, &click).is_err());

        // Screenshot-type actions are always allowed
        let screenshot = AgentAction::Screenshot;
        assert!(manager.validate_action(&app, &screenshot).is_ok());
    }

    #[test]
    fn test_explicit_grant_overrides_auto() {
        let mut manager = AppGrantManager::new();
        // Give full access to Discord explicitly
        let grant = AppGrant::new(
            "discord",
            AppCategory::Communication,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        manager.add_grant(grant);

        let app = make_app("Discord", "discord", AppCategory::Communication);
        let typing = AgentAction::Type {
            text: "hello".to_string(),
        };
        assert!(manager.validate_action(&app, &typing).is_ok());
    }

    #[test]
    fn test_revoke_grant() {
        let mut manager = AppGrantManager::new();
        let grant = AppGrant::new(
            "custom",
            AppCategory::Unknown,
            GrantLevel::Full,
            Vec::new(),
            "user",
            None,
        );
        let grant_id = grant.id.clone();
        manager.add_grant(grant);

        assert!(manager.revoke_grant(&grant_id));
        assert_eq!(manager.active_grants().len(), 0);
    }

    #[test]
    fn test_max_actions_per_app() {
        let mut manager = AppGrantManager::new().with_max_actions(3);
        let app = make_app("Terminal", "kitty", AppCategory::Terminal);
        let action = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };

        // Should allow 3 actions
        assert!(manager.validate_action(&app, &action).is_ok());
        assert!(manager.validate_action(&app, &action).is_ok());
        assert!(manager.validate_action(&app, &action).is_ok());

        // 4th should be denied
        let result = manager.validate_action(&app, &action);
        assert!(result.is_err());
    }

    #[test]
    fn test_required_permission_mapping() {
        assert_eq!(
            AppGrantManager::required_permission(&AgentAction::Click {
                x: 0,
                y: 0,
                button: "left".to_string()
            }),
            AppPermission::MouseClick
        );
        assert_eq!(
            AppGrantManager::required_permission(&AgentAction::Type {
                text: "x".to_string()
            }),
            AppPermission::KeyboardType
        );
        assert_eq!(
            AppGrantManager::required_permission(&AgentAction::Scroll {
                x: 0,
                y: 0,
                direction: "down".to_string(),
                amount: 3
            }),
            AppPermission::MouseScroll
        );
        assert_eq!(
            AppGrantManager::required_permission(&AgentAction::Screenshot),
            AppPermission::Screenshot
        );
    }

    #[test]
    fn test_grant_level_display() {
        assert_eq!(GrantLevel::Full.to_string(), "Full");
        assert_eq!(GrantLevel::ReadOnly.to_string(), "ReadOnly");
        assert_eq!(GrantLevel::Click.to_string(), "Click");
        assert_eq!(GrantLevel::Restricted.to_string(), "Restricted");
    }

    #[test]
    fn test_effective_level() {
        let manager = AppGrantManager::new();
        let terminal = make_app("Term", "kitty", AppCategory::Terminal);
        assert_eq!(manager.effective_level(&terminal), GrantLevel::Full);

        let browser = make_app("FF", "firefox", AppCategory::Browser);
        assert_eq!(manager.effective_level(&browser), GrantLevel::Click);

        let comms = make_app("Slack", "slack", AppCategory::Communication);
        assert_eq!(manager.effective_level(&comms), GrantLevel::ReadOnly);
    }
}
