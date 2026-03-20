//! `nexus-auth` — OIDC/SAML authentication, session management, and role-based
//! user administration for Nexus OS.
//!
//! This crate provides the **human user authentication layer** that sits on top
//! of the existing agent capability ACL. The architecture is two-layered:
//!
//! ```text
//! User (OIDC token) → nexus-auth → Session → UserRole → Agent Scope → Capability ACL
//! ```
//!
//! # Modules
//!
//! - [`oidc`] — OIDC authorization code flow with PKCE
//! - [`session`] — In-memory session management with automatic expiry
//! - [`roles`] — User roles and privilege hierarchy
//! - [`config`] — Authentication provider configuration
//! - [`error`] — Error types

pub mod config;
pub mod error;
pub mod oidc;
pub mod roles;
pub mod session;

// Re-exports for convenience.
pub use config::{AuthConfig, AuthProvider};
pub use error::AuthError;
pub use oidc::{AuthRedirectUrl, OidcClient};
pub use roles::UserRole;
pub use session::{AuthenticatedUser, NewSessionRequest, SessionManager, UserSummary};

/// Create a local-mode (desktop) session without OIDC.
///
/// In desktop mode, the OS-level user is trusted and gets an Admin session.
/// This bypasses OIDC entirely — useful for single-user desktop deployments.
pub async fn create_local_session(session_mgr: &SessionManager) -> AuthenticatedUser {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local-user".to_string());

    session_mgr
        .create_session(NewSessionRequest {
            id: format!("local:{username}"),
            email: format!("{username}@localhost"),
            name: username,
            role: UserRole::Admin,
            provider: "local".to_string(),
            refresh_token: None,
            workspace_id: None,
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_session_is_admin() {
        let mgr = SessionManager::new(8);
        let user = create_local_session(&mgr).await;
        assert_eq!(user.role, UserRole::Admin);
        assert_eq!(user.provider, "local");
        assert!(user.id.starts_with("local:"));
    }

    #[tokio::test]
    async fn full_session_lifecycle() {
        let mgr = SessionManager::new(8);

        // Create
        let user = mgr
            .create_session(NewSessionRequest {
                id: "oidc-user-1".into(),
                email: "admin@company.com".into(),
                name: "Admin User".into(),
                role: UserRole::Admin,
                provider: "oidc".into(),
                refresh_token: Some("refresh-tok".into()),
                workspace_id: Some("workspace-1".into()),
            })
            .await;
        assert_eq!(mgr.session_count().await, 1);

        // Retrieve
        let session = mgr.get_session(user.session_id).await.unwrap();
        assert_eq!(session.workspace_id, Some("workspace-1".to_string()));

        // List
        let active = mgr.list_active_sessions().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].email, "admin@company.com");

        // Logout
        mgr.remove_session(user.session_id).await;
        assert_eq!(mgr.session_count().await, 0);
    }

    #[test]
    fn role_hierarchy() {
        assert!(UserRole::Admin.satisfies(&UserRole::Operator));
        assert!(UserRole::Operator.satisfies(&UserRole::Viewer));
        assert!(!UserRole::Viewer.satisfies(&UserRole::Admin));
        assert!(!UserRole::Auditor.satisfies(&UserRole::Operator));
    }

    #[test]
    fn config_defaults() {
        let cfg = AuthConfig::default();
        assert_eq!(cfg.provider, AuthProvider::Local);
        assert_eq!(cfg.session_duration_hours, 8);
        assert_eq!(cfg.redirect_uri, "http://localhost:1420/auth/callback");
    }

    #[test]
    fn user_role_ordering_all_pairs() {
        // Admin > Operator > Auditor > Viewer (by privilege level)
        assert!(UserRole::Admin.privilege_level() > UserRole::Operator.privilege_level());
        assert!(UserRole::Operator.privilege_level() > UserRole::Auditor.privilege_level());
        assert!(UserRole::Auditor.privilege_level() > UserRole::Viewer.privilege_level());

        // Self-satisfaction
        assert!(UserRole::Viewer.satisfies(&UserRole::Viewer));
        assert!(UserRole::Auditor.satisfies(&UserRole::Auditor));
    }

    #[test]
    fn role_mapping_from_string_unknown_returns_none() {
        let mapping = std::collections::HashMap::new();
        assert_eq!(UserRole::from_idp_group("anything", &mapping), None);
    }

    #[test]
    fn auth_config_resolve_no_secret() {
        let cfg = AuthConfig::default();
        assert_eq!(cfg.resolve_client_secret(), None);
    }

    #[test]
    fn auth_config_serde_roundtrip_preserves_all_fields() {
        let mut mapping = std::collections::HashMap::new();
        mapping.insert("admins".to_string(), UserRole::Admin);
        mapping.insert("ops".to_string(), UserRole::Operator);
        let cfg = AuthConfig {
            provider: AuthProvider::Oidc,
            issuer_url: "https://keycloak.test/realms/nexus".to_string(),
            client_id: "nexus-client".to_string(),
            client_secret: Some("secret123".to_string()),
            role_mapping: mapping,
            roles_claim: "realm_roles".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.issuer_url, "https://keycloak.test/realms/nexus");
        assert_eq!(parsed.client_id, "nexus-client");
        assert_eq!(parsed.role_mapping.len(), 2);
        assert_eq!(parsed.roles_claim, "realm_roles");
    }

    #[tokio::test]
    async fn session_not_found_returns_error() {
        let mgr = SessionManager::new(8);
        let result = mgr.get_session(uuid::Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn multiple_sessions_same_user() {
        let mgr = SessionManager::new(8);
        let req = NewSessionRequest {
            id: "user-multi".into(),
            email: "multi@test.com".into(),
            name: "Multi".into(),
            role: UserRole::Operator,
            provider: "oidc".into(),
            refresh_token: None,
            workspace_id: None,
        };
        let s1 = mgr.create_session(req.clone()).await;
        let s2 = mgr.create_session(req).await;
        // Different session IDs even for same user
        assert_ne!(s1.session_id, s2.session_id);
        assert_eq!(mgr.session_count().await, 2);
    }
}
