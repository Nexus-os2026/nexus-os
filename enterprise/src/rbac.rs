//! Role-Based Access Control with glob-matched resource permissions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Admin,
    Operator,
    Developer,
    Viewer,
    Auditor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    pub resource: String,
    pub actions: Vec<String>,
}

impl Permission {
    pub fn new(resource: &str, actions: &[&str]) -> Self {
        Self {
            resource: resource.to_string(),
            actions: actions.iter().map(|a| a.to_string()).collect(),
        }
    }

    /// Check if this permission grants the given action on the given resource.
    /// Supports glob matching: "agent:*" matches "agent:coder", "agent:designer", etc.
    pub fn matches(&self, resource: &str, action: &str) -> bool {
        if !self.actions.contains(&action.to_string()) && !self.actions.contains(&"*".to_string())
        {
            return false;
        }
        glob_match(&self.resource, resource)
    }
}

/// Simple glob matching: "*" in pattern matches any suffix segment.
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

#[derive(Debug)]
pub struct RbacEngine {
    role_permissions: HashMap<Role, Vec<Permission>>,
    user_roles: HashMap<Uuid, Vec<Role>>,
}

impl RbacEngine {
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();

        role_permissions.insert(
            Role::Owner,
            vec![Permission::new("*", &["*"])],
        );

        role_permissions.insert(
            Role::Admin,
            vec![
                Permission::new("agent:*", &["read", "write", "execute", "approve", "delete"]),
                Permission::new("audit:*", &["read"]),
                Permission::new("config:*", &["read", "write"]),
                Permission::new("user:*", &["read", "write"]),
            ],
        );

        role_permissions.insert(
            Role::Operator,
            vec![
                Permission::new("agent:*", &["read", "execute", "approve"]),
                Permission::new("audit:*", &["read"]),
                Permission::new("config:*", &["read"]),
            ],
        );

        role_permissions.insert(
            Role::Developer,
            vec![
                Permission::new("agent:*", &["read", "write", "execute"]),
                Permission::new("audit:*", &["read"]),
                Permission::new("config:*", &["read"]),
            ],
        );

        role_permissions.insert(
            Role::Viewer,
            vec![
                Permission::new("agent:*", &["read"]),
                Permission::new("audit:*", &["read"]),
                Permission::new("config:*", &["read"]),
            ],
        );

        role_permissions.insert(
            Role::Auditor,
            vec![
                Permission::new("audit:*", &["read"]),
            ],
        );

        Self {
            role_permissions,
            user_roles: HashMap::new(),
        }
    }

    pub fn assign_role(&mut self, user_id: Uuid, role: Role) {
        let roles = self.user_roles.entry(user_id).or_default();
        if !roles.contains(&role) {
            roles.push(role);
        }
    }

    pub fn revoke_role(&mut self, user_id: Uuid, role: Role) {
        if let Some(roles) = self.user_roles.get_mut(&user_id) {
            roles.retain(|r| *r != role);
        }
    }

    pub fn check(&self, user_id: Uuid, resource: &str, action: &str) -> bool {
        let Some(roles) = self.user_roles.get(&user_id) else {
            return false;
        };

        for role in roles {
            if let Some(perms) = self.role_permissions.get(role) {
                for perm in perms {
                    if perm.matches(resource, action) {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn list_user_roles(&self, user_id: Uuid) -> Vec<Role> {
        self.user_roles.get(&user_id).cloned().unwrap_or_default()
    }

    pub fn list_role_permissions(&self, role: Role) -> Vec<Permission> {
        self.role_permissions.get(&role).cloned().unwrap_or_default()
    }
}

impl Default for RbacEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_can_do_everything() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Owner);

        assert!(engine.check(user, "agent:coder", "read"));
        assert!(engine.check(user, "agent:coder", "write"));
        assert!(engine.check(user, "agent:coder", "execute"));
        assert!(engine.check(user, "agent:coder", "delete"));
        assert!(engine.check(user, "audit:events", "read"));
        assert!(engine.check(user, "config:global", "write"));
        assert!(engine.check(user, "anything:at-all", "whatever"));
    }

    #[test]
    fn viewer_can_only_read() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Viewer);

        assert!(engine.check(user, "agent:coder", "read"));
        assert!(engine.check(user, "audit:events", "read"));
        assert!(engine.check(user, "config:global", "read"));

        assert!(!engine.check(user, "agent:coder", "write"));
        assert!(!engine.check(user, "agent:coder", "execute"));
        assert!(!engine.check(user, "agent:coder", "delete"));
        assert!(!engine.check(user, "config:global", "write"));
    }

    #[test]
    fn assign_and_revoke_changes_permissions() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();

        engine.assign_role(user, Role::Developer);
        assert!(engine.check(user, "agent:coder", "write"));

        engine.revoke_role(user, Role::Developer);
        assert!(!engine.check(user, "agent:coder", "write"));
        assert!(!engine.check(user, "agent:coder", "read"));
    }

    #[test]
    fn glob_matching_works_for_resources() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Developer);

        // "agent:*" should match any agent
        assert!(engine.check(user, "agent:coder", "read"));
        assert!(engine.check(user, "agent:designer", "read"));
        assert!(engine.check(user, "agent:social-poster", "execute"));

        // "audit:*" should match any audit resource
        assert!(engine.check(user, "audit:events", "read"));
        assert!(engine.check(user, "audit:federation", "read"));

        // Should not match unrelated resources
        assert!(!engine.check(user, "user:admin", "read"));
    }

    #[test]
    fn unassigned_user_has_no_permissions() {
        let engine = RbacEngine::new();
        let user = Uuid::new_v4();

        assert!(!engine.check(user, "agent:coder", "read"));
        assert!(!engine.check(user, "audit:events", "read"));
        assert!(!engine.check(user, "config:global", "read"));
        assert!(engine.list_user_roles(user).is_empty());
    }

    #[test]
    fn auditor_only_reads_audit() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Auditor);

        assert!(engine.check(user, "audit:events", "read"));
        assert!(engine.check(user, "audit:federation", "read"));

        assert!(!engine.check(user, "agent:coder", "read"));
        assert!(!engine.check(user, "config:global", "read"));
        assert!(!engine.check(user, "audit:events", "write"));
    }

    #[test]
    fn multiple_roles_combine_permissions() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Viewer);
        engine.assign_role(user, Role::Auditor);

        // Viewer gives agent:read, Auditor gives audit:read
        assert!(engine.check(user, "agent:coder", "read"));
        assert!(engine.check(user, "audit:events", "read"));
        assert!(!engine.check(user, "agent:coder", "write"));

        assert_eq!(engine.list_user_roles(user).len(), 2);
    }

    #[test]
    fn list_role_permissions_returns_correct_perms() {
        let engine = RbacEngine::new();
        let owner_perms = engine.list_role_permissions(Role::Owner);
        assert_eq!(owner_perms.len(), 1);
        assert_eq!(owner_perms[0].resource, "*");

        let auditor_perms = engine.list_role_permissions(Role::Auditor);
        assert_eq!(auditor_perms.len(), 1);
        assert_eq!(auditor_perms[0].resource, "audit:*");
    }

    #[test]
    fn duplicate_assign_is_idempotent() {
        let mut engine = RbacEngine::new();
        let user = Uuid::new_v4();
        engine.assign_role(user, Role::Viewer);
        engine.assign_role(user, Role::Viewer);
        assert_eq!(engine.list_user_roles(user).len(), 1);
    }
}
