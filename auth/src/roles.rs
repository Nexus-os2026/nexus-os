//! User roles and role-based access control for human users.
//!
//! These roles govern what a human user can do in the Nexus OS UI and API.
//! Agent-level capabilities are orthogonal — a user's role determines which
//! agents and operations they can access, while agent manifests determine
//! what the agents themselves are allowed to do.

use serde::{Deserialize, Serialize};

/// Human user role within Nexus OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserRole {
    /// Full system access: user management, policy editing, all operations.
    Admin,
    /// Agent deployment, configuration, HITL approval rights.
    Operator,
    /// Read-only access to dashboards and agent status.
    Viewer,
    /// Read-only access to audit trails and compliance reports.
    Auditor,
}

impl UserRole {
    /// Returns the privilege level (higher = more access).
    pub fn privilege_level(&self) -> u8 {
        match self {
            Self::Admin => 100,
            Self::Operator => 75,
            Self::Auditor => 25,
            Self::Viewer => 10,
        }
    }

    /// Check if this role has at least the privileges of `required`.
    pub fn satisfies(&self, required: &UserRole) -> bool {
        self.privilege_level() >= required.privilege_level()
    }

    /// Map from a string (IdP group name) using the configured role mapping.
    pub fn from_idp_group(
        group: &str,
        mapping: &std::collections::HashMap<String, UserRole>,
    ) -> Option<UserRole> {
        mapping.get(group).copied()
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "Admin"),
            Self::Operator => write!(f, "Operator"),
            Self::Viewer => write!(f, "Viewer"),
            Self::Auditor => write!(f, "Auditor"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn admin_satisfies_all_roles() {
        assert!(UserRole::Admin.satisfies(&UserRole::Admin));
        assert!(UserRole::Admin.satisfies(&UserRole::Operator));
        assert!(UserRole::Admin.satisfies(&UserRole::Viewer));
        assert!(UserRole::Admin.satisfies(&UserRole::Auditor));
    }

    #[test]
    fn viewer_does_not_satisfy_operator() {
        assert!(!UserRole::Viewer.satisfies(&UserRole::Operator));
    }

    #[test]
    fn operator_satisfies_viewer_and_auditor() {
        assert!(UserRole::Operator.satisfies(&UserRole::Viewer));
        assert!(UserRole::Operator.satisfies(&UserRole::Auditor));
    }

    #[test]
    fn idp_group_mapping() {
        let mut mapping = HashMap::new();
        mapping.insert("nexus-admin".to_string(), UserRole::Admin);
        mapping.insert("nexus-operator".to_string(), UserRole::Operator);
        mapping.insert("nexus-viewer".to_string(), UserRole::Viewer);

        assert_eq!(
            UserRole::from_idp_group("nexus-admin", &mapping),
            Some(UserRole::Admin)
        );
        assert_eq!(UserRole::from_idp_group("unknown-group", &mapping), None);
    }

    #[test]
    fn display_roles() {
        assert_eq!(format!("{}", UserRole::Admin), "Admin");
        assert_eq!(format!("{}", UserRole::Auditor), "Auditor");
    }
}
