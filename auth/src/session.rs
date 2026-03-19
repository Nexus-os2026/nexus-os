//! In-memory session management with automatic expiry cleanup.

use crate::error::AuthError;
use crate::roles::UserRole;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// An authenticated user session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    /// Unique user identifier (from IdP `sub` claim).
    pub id: String,
    /// User email address.
    pub email: String,
    /// Display name.
    pub name: String,
    /// Assigned role within Nexus OS.
    pub role: UserRole,
    /// Workspace/tenant ID for multi-tenancy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Unique session identifier.
    pub session_id: Uuid,
    /// When the user authenticated.
    pub authenticated_at: DateTime<Utc>,
    /// When this session expires.
    pub expires_at: DateTime<Utc>,
    /// IdP that issued the identity.
    pub provider: String,
    /// OIDC refresh token (not serialized to frontend).
    #[serde(skip)]
    pub refresh_token: Option<String>,
}

/// Summary of a user session (safe to expose to admin UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub session_id: Uuid,
    pub authenticated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl From<&AuthenticatedUser> for UserSummary {
    fn from(user: &AuthenticatedUser) -> Self {
        Self {
            id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            role: user.role,
            session_id: user.session_id,
            authenticated_at: user.authenticated_at,
            expires_at: user.expires_at,
        }
    }
}

/// Parameters for creating a new user session.
#[derive(Debug, Clone)]
pub struct NewSessionRequest {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub provider: String,
    pub refresh_token: Option<String>,
    pub workspace_id: Option<String>,
}

/// Manages active user sessions with automatic cleanup.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, AuthenticatedUser>>>,
    max_session_duration: Duration,
}

impl SessionManager {
    /// Create a new session manager with the given max session duration in hours.
    pub fn new(max_session_hours: u64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_session_duration: Duration::hours(max_session_hours as i64),
        }
    }

    /// Create a new session for an authenticated user.
    pub async fn create_session(&self, req: NewSessionRequest) -> AuthenticatedUser {
        let now = Utc::now();
        let session = AuthenticatedUser {
            id: req.id,
            email: req.email,
            name: req.name,
            role: req.role,
            workspace_id: req.workspace_id,
            session_id: Uuid::new_v4(),
            authenticated_at: now,
            expires_at: now + self.max_session_duration,
            provider: req.provider,
            refresh_token: req.refresh_token,
        };
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_id, session.clone());
        session
    }

    /// Retrieve and validate a session by ID.
    pub async fn get_session(&self, session_id: Uuid) -> Result<AuthenticatedUser, AuthError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| AuthError::SessionNotFound(session_id.to_string()))?;

        if Utc::now() > session.expires_at {
            drop(sessions);
            self.remove_session(session_id).await;
            return Err(AuthError::SessionExpired);
        }

        Ok(session.clone())
    }

    /// Remove a session (logout).
    pub async fn remove_session(&self, session_id: Uuid) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&session_id).is_some()
    }

    /// List all active (non-expired) sessions as summaries.
    pub async fn list_active_sessions(&self) -> Vec<UserSummary> {
        let sessions = self.sessions.read().await;
        let now = Utc::now();
        sessions
            .values()
            .filter(|s| s.expires_at > now)
            .map(UserSummary::from)
            .collect()
    }

    /// Remove all expired sessions. Returns count of removed sessions.
    pub async fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let now = Utc::now();
        let before = sessions.len();
        sessions.retain(|_, s| s.expires_at > now);
        before - sessions.len()
    }

    /// Spawn a background task that periodically cleans up expired sessions.
    pub fn spawn_cleanup_task(self, interval_secs: u64) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let removed = self.cleanup_expired().await;
                if removed > 0 {
                    tracing::info!("Session cleanup: removed {} expired sessions", removed);
                }
            }
        })
    }

    /// Total number of sessions (including possibly expired).
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_retrieve_session() {
        let mgr = SessionManager::new(8);
        let user = mgr
            .create_session(NewSessionRequest {
                id: "user-1".into(),
                email: "user@example.com".into(),
                name: "Test User".into(),
                role: UserRole::Operator,
                provider: "oidc".into(),
                refresh_token: None,
                workspace_id: None,
            })
            .await;

        let retrieved = mgr.get_session(user.session_id).await.unwrap();
        assert_eq!(retrieved.email, "user@example.com");
        assert_eq!(retrieved.role, UserRole::Operator);
    }

    #[tokio::test]
    async fn expired_session_returns_error() {
        let mgr = SessionManager::new(0); // 0 hours = expires immediately
        let user = mgr
            .create_session(NewSessionRequest {
                id: "user-2".into(),
                email: "expired@example.com".into(),
                name: "Expired User".into(),
                role: UserRole::Viewer,
                provider: "oidc".into(),
                refresh_token: None,
                workspace_id: None,
            })
            .await;

        // Session was created with 0-hour duration, so it's already expired
        let result = mgr.get_session(user.session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn logout_removes_session() {
        let mgr = SessionManager::new(8);
        let user = mgr
            .create_session(NewSessionRequest {
                id: "user-3".into(),
                email: "logout@example.com".into(),
                name: "Logout User".into(),
                role: UserRole::Admin,
                provider: "local".into(),
                refresh_token: None,
                workspace_id: None,
            })
            .await;

        assert!(mgr.remove_session(user.session_id).await);
        assert!(mgr.get_session(user.session_id).await.is_err());
    }

    #[tokio::test]
    async fn list_active_sessions() {
        let mgr = SessionManager::new(8);
        mgr.create_session(NewSessionRequest {
            id: "u1".into(),
            email: "a@b.com".into(),
            name: "A".into(),
            role: UserRole::Admin,
            provider: "oidc".into(),
            refresh_token: None,
            workspace_id: None,
        })
        .await;
        mgr.create_session(NewSessionRequest {
            id: "u2".into(),
            email: "c@d.com".into(),
            name: "B".into(),
            role: UserRole::Viewer,
            provider: "oidc".into(),
            refresh_token: None,
            workspace_id: None,
        })
        .await;

        let active = mgr.list_active_sessions().await;
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn cleanup_expired_sessions() {
        let mgr = SessionManager::new(0); // all sessions expire immediately
        mgr.create_session(NewSessionRequest {
            id: "u1".into(),
            email: "a@b.com".into(),
            name: "A".into(),
            role: UserRole::Admin,
            provider: "oidc".into(),
            refresh_token: None,
            workspace_id: None,
        })
        .await;

        let removed = mgr.cleanup_expired().await;
        assert_eq!(removed, 1);
        assert_eq!(mgr.session_count().await, 0);
    }
}
