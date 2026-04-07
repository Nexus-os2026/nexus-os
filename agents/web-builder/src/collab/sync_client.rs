//! Sync Client — WebSocket client for joining collaboration sessions.
//!
//! Connects to the host's sync server for Yjs CRDT sync.
//! The actual Yjs sync protocol is handled on the frontend (TypeScript).
//! This module manages the connection lifecycle from the Rust/Tauri side.

use super::CollabError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// ─── Client State ─────────────────────────────────────────────────────────

/// State of the sync client connection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncClientState {
    pub connected: bool,
    pub server_url: Option<String>,
    pub session_token: Option<String>,
}

/// Shared sync client state.
pub type SharedClientState = Arc<Mutex<SyncClientState>>;

/// Create a new shared client state.
pub fn new_client_state() -> SharedClientState {
    Arc::new(Mutex::new(SyncClientState::default()))
}

// ─── Client Lifecycle ─────────────────────────────────────────────────────

/// Connect to a collaboration session.
///
/// The actual WebSocket connection and Yjs sync happen on the frontend
/// via y-websocket. This sets the Rust-side state so Tauri commands
/// know we're in a collaborative session.
pub async fn connect(
    state: SharedClientState,
    server_url: &str,
    session_token: &str,
) -> Result<(), CollabError> {
    let mut s = state.lock().await;
    if s.connected {
        return Err(CollabError::AlreadyConnected);
    }

    s.connected = true;
    s.server_url = Some(server_url.to_string());
    s.session_token = Some(session_token.to_string());

    eprintln!("[collab] Connected to {server_url}");
    Ok(())
}

/// Disconnect from the collaboration session.
pub async fn disconnect(state: SharedClientState) -> Result<(), CollabError> {
    let mut s = state.lock().await;
    if !s.connected {
        return Err(CollabError::NotInSession);
    }

    s.connected = false;
    s.server_url = None;
    s.session_token = None;

    eprintln!("[collab] Disconnected");
    Ok(())
}

/// Get the current client status.
pub async fn client_status(state: &SharedClientState) -> SyncClientState {
    state.lock().await.clone()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect() {
        let state = new_client_state();
        connect(state.clone(), "ws://192.168.1.5:15200", "token123")
            .await
            .unwrap();

        let s = state.lock().await;
        assert!(s.connected);
        assert_eq!(s.server_url.as_deref(), Some("ws://192.168.1.5:15200"));
    }

    #[tokio::test]
    async fn test_disconnect() {
        let state = new_client_state();
        connect(state.clone(), "ws://localhost:15200", "token123")
            .await
            .unwrap();
        disconnect(state.clone()).await.unwrap();

        let s = state.lock().await;
        assert!(!s.connected);
        assert!(s.server_url.is_none());
    }

    #[tokio::test]
    async fn test_double_connect_fails() {
        let state = new_client_state();
        connect(state.clone(), "ws://localhost:15200", "token123")
            .await
            .unwrap();

        let result = connect(state.clone(), "ws://localhost:15200", "token456").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected_fails() {
        let state = new_client_state();
        let result = disconnect(state).await;
        assert!(result.is_err());
    }
}
