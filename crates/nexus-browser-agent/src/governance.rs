//! Browser capability governance — controls which agents can use browser automation.

use serde::{Deserialize, Serialize};

/// The capability name for browser automation.
pub const BROWSER_CAPABILITY: &str = "browser_automation";

/// Browser-specific governance policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPolicy {
    pub min_autonomy_level: u8,
    pub max_sessions_per_agent: u32,
    pub max_steps_per_task: u32,
    pub url_allowlist: Vec<String>,
    pub url_denylist: Vec<String>,
    pub allow_headful: bool,
    pub max_task_duration_secs: u64,
}

impl Default for BrowserPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 3,
            max_sessions_per_agent: 2,
            max_steps_per_task: 50,
            url_allowlist: Vec::new(),
            url_denylist: vec![
                "file://".into(),
                "chrome://".into(),
                "about:".into(),
                "javascript:".into(),
                "data:text/html".into(),
            ],
            allow_headful: false,
            max_task_duration_secs: 300,
        }
    }
}

/// Governance error.
#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("Agent {agent_id} requires L{required}+ for browser automation (has L{actual})")]
    InsufficientAutonomy {
        agent_id: String,
        required: u8,
        actual: u8,
    },
    #[error("URL denied: {url} — {reason}")]
    UrlDenied { url: String, reason: String },
    #[error("Step limit exceeded: requested {requested}, maximum {maximum}")]
    StepLimitExceeded { requested: u32, maximum: u32 },
}

/// Check if an agent is authorized for browser automation.
pub fn check_authorization(
    agent_id: &str,
    autonomy_level: u8,
    policy: &BrowserPolicy,
) -> Result<(), GovernanceError> {
    if autonomy_level < policy.min_autonomy_level {
        return Err(GovernanceError::InsufficientAutonomy {
            agent_id: agent_id.into(),
            required: policy.min_autonomy_level,
            actual: autonomy_level,
        });
    }
    Ok(())
}

/// Check if a URL is allowed.
pub fn check_url(url: &str, policy: &BrowserPolicy) -> Result<(), GovernanceError> {
    for denied in &policy.url_denylist {
        if url.starts_with(denied) || url.contains(denied) {
            return Err(GovernanceError::UrlDenied {
                url: url.into(),
                reason: format!("Matches denylist pattern: {denied}"),
            });
        }
    }

    if !policy.url_allowlist.is_empty()
        && !policy
            .url_allowlist
            .iter()
            .any(|p| url.starts_with(p) || url.contains(p))
    {
        return Err(GovernanceError::UrlDenied {
            url: url.into(),
            reason: "Not in URL allowlist".into(),
        });
    }

    Ok(())
}

/// Check step count against policy.
pub fn check_steps(steps: u32, policy: &BrowserPolicy) -> Result<(), GovernanceError> {
    if steps > policy.max_steps_per_task {
        return Err(GovernanceError::StepLimitExceeded {
            requested: steps,
            maximum: policy.max_steps_per_task,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_min_autonomy() {
        let policy = BrowserPolicy::default();
        assert!(check_authorization("agent-l2", 2, &policy).is_err());
        assert!(check_authorization("agent-l3", 3, &policy).is_ok());
        assert!(check_authorization("agent-l5", 5, &policy).is_ok());
    }

    #[test]
    fn test_governance_url_denylist() {
        let policy = BrowserPolicy::default();
        assert!(check_url("file:///etc/passwd", &policy).is_err());
        assert!(check_url("javascript:alert(1)", &policy).is_err());
        assert!(check_url("chrome://settings", &policy).is_err());
        assert!(check_url("https://example.com", &policy).is_ok());
    }

    #[test]
    fn test_governance_url_allowlist() {
        let policy = BrowserPolicy {
            url_allowlist: vec!["https://example.com".into()],
            ..BrowserPolicy::default()
        };
        assert!(check_url("https://example.com/page", &policy).is_ok());
        assert!(check_url("https://evil.com", &policy).is_err());
    }

    #[test]
    fn test_governance_step_limit() {
        let policy = BrowserPolicy {
            max_steps_per_task: 50,
            ..BrowserPolicy::default()
        };
        assert!(check_steps(30, &policy).is_ok());
        assert!(check_steps(100, &policy).is_err());
    }

    #[test]
    fn test_policy_defaults() {
        let policy = BrowserPolicy::default();
        assert_eq!(policy.min_autonomy_level, 3);
        assert_eq!(policy.max_sessions_per_agent, 2);
        assert_eq!(policy.max_steps_per_task, 50);
        assert!(!policy.allow_headful);
        assert_eq!(policy.max_task_duration_secs, 300);
        assert!(!policy.url_denylist.is_empty());
    }
}
