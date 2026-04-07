use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use super::app_grant::{AppGrant, AppGrantManager, GrantLevel};
use super::app_registry::{AppInfo, AppRegistry};
use crate::agent::action::AgentAction;
use crate::error::ComputerUseError;

/// Configuration for a governed session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Auto-grant full access to terminal apps
    pub auto_grant_terminals: bool,
    /// Auto-grant full access to editor apps
    pub auto_grant_editors: bool,
    /// Auto-grant full access to Nexus OS app
    pub auto_grant_nexus: bool,
    /// Require explicit grant for unknown apps
    pub require_grant_for_unknown: bool,
    /// Maximum actions per app per session
    pub max_actions_per_app: u32,
    /// Session timeout in minutes
    pub session_timeout_minutes: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_grant_terminals: true,
            auto_grant_editors: true,
            auto_grant_nexus: true,
            require_grant_for_unknown: true,
            max_actions_per_app: 500,
            session_timeout_minutes: 60,
        }
    }
}

/// Every action in a governed session is logged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedAction {
    /// Step number in the session
    pub step: u32,
    /// The app where the action occurred
    pub app: AppInfo,
    /// Description of the action
    pub action: String,
    /// Which grant authorized this action
    pub grant_id: String,
    /// The grant level that was applied
    pub grant_level: GrantLevel,
    /// When the action occurred
    pub timestamp: DateTime<Utc>,
    /// Audit hash for this action
    pub audit_hash: String,
}

impl GovernedAction {
    /// Create a new governed action log entry
    pub fn new(
        step: u32,
        app: &AppInfo,
        action: &str,
        grant_id: &str,
        grant_level: GrantLevel,
    ) -> Self {
        let timestamp = Utc::now();
        let mut hasher = Sha256::new();
        hasher.update(step.to_le_bytes());
        hasher.update(app.wm_class.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(grant_id.as_bytes());
        hasher.update(timestamp.to_rfc3339().as_bytes());
        let audit_hash = hex::encode(hasher.finalize());

        Self {
            step,
            app: app.clone(),
            action: action.to_string(),
            grant_id: grant_id.to_string(),
            grant_level,
            timestamp,
            audit_hash,
        }
    }
}

/// A governed computer-use session with full audit trail
///
/// Note: Does not implement Clone because sessions are unique and should not be duplicated.
/// AgentConfig uses Option<GovernedSession> and is no longer Clone when a session is attached.
pub struct GovernedSession {
    /// Unique session identifier
    pub id: String,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// App registry for window detection
    pub app_registry: AppRegistry,
    /// Grant manager for permission validation
    pub grant_manager: AppGrantManager,
    /// Log of all governed actions
    pub action_log: Vec<GovernedAction>,
    /// Session configuration
    pub config: SessionConfig,
    /// Current step counter
    step_counter: u32,
}

impl GovernedSession {
    /// Create a new governed session
    pub fn new(config: SessionConfig) -> Self {
        let grant_manager = AppGrantManager::new().with_max_actions(config.max_actions_per_app);

        Self {
            id: Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            app_registry: AppRegistry::new(),
            grant_manager,
            action_log: Vec::new(),
            config,
            step_counter: 0,
        }
    }

    /// Create a session with default config
    pub fn with_defaults() -> Self {
        Self::new(SessionConfig::default())
    }

    /// Check if the session has timed out
    pub fn is_timed_out(&self) -> bool {
        let elapsed = Utc::now() - self.started_at;
        let timeout = chrono::Duration::minutes(i64::from(self.config.session_timeout_minutes));
        elapsed > timeout
    }

    /// Validate an action against the governance rules for the focused app
    pub fn validate_action(
        &mut self,
        focused_app: &AppInfo,
        action: &AgentAction,
    ) -> Result<String, ComputerUseError> {
        // Check session timeout
        if self.is_timed_out() {
            return Err(ComputerUseError::CapabilityDenied {
                capability: format!(
                    "Session timed out after {} minutes",
                    self.config.session_timeout_minutes
                ),
            });
        }

        self.grant_manager.validate_action(focused_app, action)
    }

    /// Log an action that was validated and executed
    pub fn log_action(&mut self, app: &AppInfo, action: &AgentAction, grant_id: &str) {
        self.step_counter += 1;
        let level = self.grant_manager.effective_level(app);
        let governed =
            GovernedAction::new(self.step_counter, app, &action.to_string(), grant_id, level);
        info!(
            "Governed action #{}: {} on {} (grant: {})",
            governed.step, governed.action, app.name, grant_id
        );
        self.action_log.push(governed);
    }

    /// Request a new grant — returns the formatted request string for display
    pub fn format_grant_request(app: &AppInfo, level: &GrantLevel, reason: &str) -> String {
        format!(
            "nx wants [{level}] access to [{}] for: [{reason}]",
            app.name
        )
    }

    /// Create and add a grant after user approval
    pub fn approve_grant(
        &mut self,
        app: &AppInfo,
        level: GrantLevel,
        expires_at: Option<DateTime<Utc>>,
    ) -> AppGrant {
        let grant = AppGrant::new(
            &app.wm_class,
            app.category.clone(),
            level,
            Vec::new(),
            "user",
            expires_at,
        );
        self.grant_manager.add_grant(grant.clone());
        info!(
            "Grant approved: {} -> {} for '{}'",
            grant.id, grant.grant_level, app.name
        );
        grant
    }

    /// Get the total number of governed actions in this session
    pub fn total_actions(&self) -> u32 {
        self.step_counter
    }

    /// Get the session audit summary
    pub fn audit_summary(&self) -> String {
        let mut hasher = Sha256::new();
        for action in &self.action_log {
            hasher.update(action.audit_hash.as_bytes());
        }
        let session_hash = hex::encode(hasher.finalize());
        format!(
            "Session {} | {} actions | started {} | hash: {}",
            self.id,
            self.step_counter,
            self.started_at.to_rfc3339(),
            session_hash
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::AppCategory;

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
    fn test_session_creation() {
        let session = GovernedSession::with_defaults();
        assert!(!session.id.is_empty());
        assert!(session.action_log.is_empty());
        assert_eq!(session.step_counter, 0);
    }

    #[test]
    fn test_session_config_defaults() {
        let config = SessionConfig::default();
        assert!(config.auto_grant_terminals);
        assert!(config.auto_grant_editors);
        assert!(config.auto_grant_nexus);
        assert!(config.require_grant_for_unknown);
        assert_eq!(config.max_actions_per_app, 500);
        assert_eq!(config.session_timeout_minutes, 60);
    }

    #[test]
    fn test_governed_action_logging() {
        let mut session = GovernedSession::with_defaults();
        let app = make_app("Terminal", "kitty", AppCategory::Terminal);
        let action = AgentAction::Type {
            text: "ls".to_string(),
        };

        // Validate first
        let grant_id = session
            .validate_action(&app, &action)
            .expect("terminal typing should be auto-granted");

        // Log it
        session.log_action(&app, &action, &grant_id);
        assert_eq!(session.action_log.len(), 1);
        assert_eq!(session.step_counter, 1);

        let logged = &session.action_log[0];
        assert_eq!(logged.step, 1);
        assert_eq!(logged.app.name, "Terminal");
        assert!(!logged.audit_hash.is_empty());
    }

    #[test]
    fn test_session_max_actions_per_app() {
        let config = SessionConfig {
            max_actions_per_app: 2,
            ..Default::default()
        };
        let mut session = GovernedSession::new(config);
        let app = make_app("Terminal", "kitty", AppCategory::Terminal);
        let action = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };

        assert!(session.validate_action(&app, &action).is_ok());
        assert!(session.validate_action(&app, &action).is_ok());
        // Third should fail
        assert!(session.validate_action(&app, &action).is_err());
    }

    #[test]
    fn test_session_timeout() {
        let config = SessionConfig {
            session_timeout_minutes: 0, // immediate timeout
            ..Default::default()
        };
        let mut session = GovernedSession::new(config);
        // Set started_at to the past to ensure timeout
        session.started_at = Utc::now() - chrono::Duration::minutes(1);

        let app = make_app("Terminal", "kitty", AppCategory::Terminal);
        let action = AgentAction::Click {
            x: 10,
            y: 20,
            button: "left".to_string(),
        };
        let result = session.validate_action(&app, &action);
        assert!(result.is_err());
    }

    #[test]
    fn test_request_grant_format() {
        let app = make_app("VS Code", "code", AppCategory::Editor);
        let msg =
            GovernedSession::format_grant_request(&app, &GrantLevel::Full, "editing source files");
        assert!(msg.contains("[Full]"));
        assert!(msg.contains("[VS Code]"));
        assert!(msg.contains("editing source files"));
    }

    #[test]
    fn test_approve_grant() {
        let mut session = GovernedSession::with_defaults();
        let app = make_app("Discord", "discord", AppCategory::Communication);

        // Before grant: typing denied
        let action = AgentAction::Type {
            text: "hello".to_string(),
        };
        assert!(session.validate_action(&app, &action).is_err());

        // Approve grant
        let grant = session.approve_grant(&app, GrantLevel::Full, None);
        assert!(!grant.id.is_empty());
        assert_eq!(grant.grant_level, GrantLevel::Full);

        // After grant: typing allowed
        assert!(session.validate_action(&app, &action).is_ok());
    }

    #[test]
    fn test_audit_summary() {
        let session = GovernedSession::with_defaults();
        let summary = session.audit_summary();
        assert!(summary.contains(&session.id));
        assert!(summary.contains("0 actions"));
    }

    #[test]
    fn test_session_not_timed_out() {
        let session = GovernedSession::with_defaults();
        assert!(!session.is_timed_out());
    }

    #[test]
    fn test_governed_action_audit_hash() {
        let app = make_app("Term", "kitty", AppCategory::Terminal);
        let action = GovernedAction::new(1, &app, "type(ls)", "grant-123", GrantLevel::Full);
        assert_eq!(action.audit_hash.len(), 64);
        assert_eq!(action.step, 1);
        assert_eq!(action.grant_id, "grant-123");
    }
}
