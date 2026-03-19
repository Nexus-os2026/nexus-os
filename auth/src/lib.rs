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
}
