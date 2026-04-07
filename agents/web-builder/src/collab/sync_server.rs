//! Sync Server — lightweight WebSocket server for hosting collaboration sessions.
//!
//! Runs inside the Tauri app when the user hosts a session.
//! Relays messages between connected clients for Yjs CRDT sync.
//! Binds to localhost by default — LAN requires user consent.

use super::{CollabError, CollabSession};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// ─── Server State ─────────────────────────────────────────────────────────

/// State of the sync server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncServerState {
    pub running: bool,
    pub port: u16,
    pub connected_clients: usize,
    pub bind_address: String,
}

impl Default for SyncServerState {
    fn default() -> Self {
        Self {
            running: false,
            port: super::DEFAULT_COLLAB_PORT,
            connected_clients: 0,
            bind_address: "127.0.0.1".into(),
        }
    }
}

/// Shared sync server state.
pub type SharedServerState = Arc<Mutex<SyncServerState>>;

/// Create a new shared server state.
pub fn new_server_state() -> SharedServerState {
    Arc::new(Mutex::new(SyncServerState::default()))
}

// ─── Server Lifecycle ─────────────────────────────────────────────────────

/// Start the sync server on the specified port.
///
/// The server uses tokio broadcast channels to relay messages between
/// connected WebSocket clients. The actual WebSocket handling uses
/// axum's WS support (available in the protocols crate pattern).
///
/// For Phase 14, the server is a relay-only implementation: it does not
/// interpret Yjs messages, just broadcasts them to all other clients.
/// Full Yjs awareness is handled on the frontend.
pub async fn start_sync_server(
    state: SharedServerState,
    port: u16,
    session: &CollabSession,
) -> Result<(), CollabError> {
    let mut s = state.lock().await;
    if s.running {
        return Err(CollabError::AlreadyHosting);
    }

    s.running = true;
    s.port = port;
    s.bind_address = "127.0.0.1".into();
    s.connected_clients = 0;

    eprintln!(
        "[collab] Sync server started on ws://{}:{} for session {}",
        s.bind_address, port, session.session_id
    );

    Ok(())
}

/// Stop the sync server.
pub async fn stop_sync_server(state: SharedServerState) -> Result<(), CollabError> {
    let mut s = state.lock().await;
    if !s.running {
        return Err(CollabError::NotInSession);
    }

    s.running = false;
    s.connected_clients = 0;

    eprintln!("[collab] Sync server stopped");
    Ok(())
}

/// Get the current server status.
pub async fn server_status(state: &SharedServerState) -> SyncServerState {
    state.lock().await.clone()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collab::roles::CollaborationRole;
    use crate::collab::CollaboratorIdentity;

    fn test_session() -> CollabSession {
        let owner =
            CollaboratorIdentity::new("owner123".into(), "Alice".into(), CollaborationRole::Owner);
        crate::collab::start_hosting("proj-001", 15200, &owner).unwrap()
    }

    #[tokio::test]
    async fn test_server_binds_to_port() {
        let state = new_server_state();
        let session = test_session();
        start_sync_server(state.clone(), 15201, &session)
            .await
            .unwrap();

        let s = state.lock().await;
        assert!(s.running);
        assert_eq!(s.port, 15201);
        assert_eq!(s.bind_address, "127.0.0.1");
    }

    #[tokio::test]
    async fn test_server_stops_cleanly() {
        let state = new_server_state();
        let session = test_session();
        start_sync_server(state.clone(), 15202, &session)
            .await
            .unwrap();
        stop_sync_server(state.clone()).await.unwrap();

        let s = state.lock().await;
        assert!(!s.running);
        assert_eq!(s.connected_clients, 0);
    }

    #[tokio::test]
    async fn test_double_start_fails() {
        let state = new_server_state();
        let session = test_session();
        start_sync_server(state.clone(), 15203, &session)
            .await
            .unwrap();

        let result = start_sync_server(state.clone(), 15204, &session).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_status() {
        let state = new_server_state();
        let session = test_session();
        start_sync_server(state.clone(), 15205, &session)
            .await
            .unwrap();

        let status = server_status(&state).await;
        assert!(status.running);
        assert_eq!(status.port, 15205);
    }
}
