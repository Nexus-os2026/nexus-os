//! Collaboration — multi-user real-time collaboration for Nexus Builder.
//!
//! Supports hosting and joining sessions over WebSocket (LAN-only).
//! Uses Ed25519-signed change attribution and role-based access control.
//! Solo mode is the default — collaboration is opt-in.

pub mod attribution;
pub mod comments;
pub mod presence;
pub mod roles;
pub mod sync_client;
pub mod sync_server;

use roles::CollaborationRole;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CollabError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("already hosting")]
    AlreadyHosting,
    #[error("already connected")]
    AlreadyConnected,
    #[error("not in a session")]
    NotInSession,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("server error: {0}")]
    ServerError(String),
    #[error("identity error: {0}")]
    IdentityError(String),
}

// ─── Collaborator Identity ────────────────────────────────────────────────

/// A collaborator's identity in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaboratorIdentity {
    pub public_key: String,
    pub display_name: String,
    pub color: String,
    pub role: CollaborationRole,
}

impl CollaboratorIdentity {
    pub fn new(public_key: String, display_name: String, role: CollaborationRole) -> Self {
        let color = assign_color(&public_key);
        Self {
            public_key,
            display_name,
            color,
            role,
        }
    }
}

/// Assign a deterministic color based on public key hash.
fn assign_color(public_key: &str) -> String {
    const PALETTE: &[&str] = &[
        "#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#8b5cf6", "#ec4899", "#06b6d4", "#f97316",
        "#14b8a6", "#a855f7", "#6366f1", "#10b981",
    ];
    let hash: usize = public_key.bytes().map(|b| b as usize).sum();
    PALETTE[hash % PALETTE.len()].to_string()
}

// ─── Collaboration Mode ───────────────────────────────────────────────────

/// Current collaboration mode for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum CollabMode {
    /// No collaboration (default, current behavior)
    #[default]
    Solo,
    /// This machine is the sync server
    Hosting { port: u16 },
    /// Connected to another machine's server
    Connected { server_url: String },
}

// ─── Session ──────────────────────────────────────────────────────────────

/// A collaboration session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabSession {
    pub session_id: String,
    pub project_id: String,
    pub host_address: String,
    pub session_token: String,
    pub participants: Vec<CollaboratorIdentity>,
    pub created_at: String,
}

/// Default collaboration port.
pub const DEFAULT_COLLAB_PORT: u16 = 15200;

/// Create a new hosting session.
pub fn start_hosting(
    project_id: &str,
    port: u16,
    owner: &CollaboratorIdentity,
) -> Result<CollabSession, CollabError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let session_token = generate_session_token();
    let host_address = format!("ws://127.0.0.1:{port}");

    Ok(CollabSession {
        session_id,
        project_id: project_id.to_string(),
        host_address,
        session_token,
        participants: vec![owner.clone()],
        created_at: crate::deploy::now_iso8601(),
    })
}

/// Add a participant to a session.
pub fn add_participant(
    session: &mut CollabSession,
    participant: CollaboratorIdentity,
) -> Result<(), CollabError> {
    // Check for duplicate
    if session
        .participants
        .iter()
        .any(|p| p.public_key == participant.public_key)
    {
        return Ok(()); // already in session
    }
    session.participants.push(participant);
    Ok(())
}

/// Remove a participant from a session.
pub fn remove_participant(session: &mut CollabSession, public_key: &str) {
    session.participants.retain(|p| p.public_key != public_key);
}

/// Generate an invite URL containing the session token and host address.
pub fn generate_invite(session: &CollabSession, role: CollaborationRole) -> String {
    // Format: nexus-collab://<host>?token=<token>&role=<role>
    format!(
        "nexus-collab://{}?token={}&role={}",
        session.host_address.trim_start_matches("ws://"),
        session.session_token,
        serde_json::to_string(&role)
            .unwrap_or_default()
            .trim_matches('"'),
    )
}

/// Generate a cryptographically random session token.
fn generate_session_token() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

/// Save session state to project directory.
pub fn save_session(project_dir: &std::path::Path, session: &CollabSession) -> Result<(), String> {
    let path = project_dir.join("collab_session.json");
    let json =
        serde_json::to_string_pretty(session).map_err(|e| format!("serialize session: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write session: {e}"))
}

/// Load session state from project directory.
pub fn load_session(project_dir: &std::path::Path) -> Option<CollabSession> {
    let path = project_dir.join("collab_session.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_owner() -> CollaboratorIdentity {
        CollaboratorIdentity::new("abcd1234".into(), "Alice".into(), CollaborationRole::Owner)
    }

    fn test_editor() -> CollaboratorIdentity {
        CollaboratorIdentity::new("ef567890".into(), "Bob".into(), CollaborationRole::Editor)
    }

    #[test]
    fn test_create_session() {
        let owner = test_owner();
        let session = start_hosting("proj-001", 15200, &owner).unwrap();
        assert!(!session.session_id.is_empty());
        assert_eq!(session.project_id, "proj-001");
        assert!(session.host_address.contains("15200"));
        assert_eq!(session.participants.len(), 1);
        assert_eq!(session.participants[0].display_name, "Alice");
    }

    #[test]
    fn test_session_tracks_participants() {
        let owner = test_owner();
        let mut session = start_hosting("proj-001", 15200, &owner).unwrap();
        assert_eq!(session.participants.len(), 1);

        let editor = test_editor();
        add_participant(&mut session, editor).unwrap();
        assert_eq!(session.participants.len(), 2);
    }

    #[test]
    fn test_leave_removes_participant() {
        let owner = test_owner();
        let mut session = start_hosting("proj-001", 15200, &owner).unwrap();
        let editor = test_editor();
        add_participant(&mut session, editor).unwrap();
        assert_eq!(session.participants.len(), 2);

        remove_participant(&mut session, "ef567890");
        assert_eq!(session.participants.len(), 1);
        assert_eq!(session.participants[0].public_key, "abcd1234");
    }

    #[test]
    fn test_duplicate_participant_ignored() {
        let owner = test_owner();
        let mut session = start_hosting("proj-001", 15200, &owner).unwrap();
        add_participant(&mut session, test_owner()).unwrap();
        assert_eq!(session.participants.len(), 1, "duplicate should be ignored");
    }

    #[test]
    fn test_generate_invite() {
        let owner = test_owner();
        let session = start_hosting("proj-001", 15200, &owner).unwrap();
        let invite = generate_invite(&session, CollaborationRole::Editor);
        assert!(invite.starts_with("nexus-collab://"));
        assert!(invite.contains("token="));
        assert!(invite.contains("role="));
    }

    #[test]
    fn test_assign_color_deterministic() {
        let c1 = assign_color("key123");
        let c2 = assign_color("key123");
        assert_eq!(c1, c2, "same key should get same color");
    }

    #[test]
    fn test_collab_mode_default_is_solo() {
        let mode = CollabMode::default();
        assert!(matches!(mode, CollabMode::Solo));
    }

    #[test]
    fn test_session_persistence() {
        let dir = std::env::temp_dir().join(format!("nexus-collab-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let owner = test_owner();
        let session = start_hosting("proj-001", 15200, &owner).unwrap();
        save_session(&dir, &session).unwrap();

        let loaded = load_session(&dir);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.participants.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
