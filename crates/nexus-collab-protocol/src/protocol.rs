use crate::patterns::CollaborationPattern;
use crate::session::{CollabError, CollaborationSession};

/// Manages all active and completed collaboration sessions.
pub struct CollaborationProtocol {
    sessions: Vec<CollaborationSession>,
    history: Vec<CollaborationSession>,
    max_active_sessions: usize,
}

impl CollaborationProtocol {
    pub fn new(max_active_sessions: usize) -> Self {
        Self {
            sessions: Vec::new(),
            history: Vec::new(),
            max_active_sessions,
        }
    }

    pub fn create_session(
        &mut self,
        title: String,
        goal: String,
        pattern: CollaborationPattern,
        lead_agent: &str,
        lead_autonomy: u8,
    ) -> Result<String, CollabError> {
        if self.sessions.len() >= self.max_active_sessions {
            return Err(CollabError::InvalidState(format!(
                "Maximum {} active sessions",
                self.max_active_sessions,
            )));
        }
        let session = CollaborationSession::new(title, goal, pattern, lead_agent, lead_autonomy);
        let id = session.id.clone();
        self.sessions.push(session);
        Ok(id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut CollaborationSession> {
        self.sessions.iter_mut().find(|s| s.id == session_id)
    }

    pub fn get_session(&self, session_id: &str) -> Option<&CollaborationSession> {
        self.sessions
            .iter()
            .find(|s| s.id == session_id)
            .or_else(|| self.history.iter().find(|s| s.id == session_id))
    }

    pub fn complete_session(&mut self, session_id: &str) -> Result<(), CollabError> {
        if let Some(idx) = self.sessions.iter().position(|s| s.id == session_id) {
            let session = self.sessions.remove(idx);
            self.history.push(session);
            Ok(())
        } else {
            Err(CollabError::InvalidState("Session not found".into()))
        }
    }

    pub fn active_sessions(&self) -> &[CollaborationSession] {
        &self.sessions
    }

    pub fn completed_sessions(&self) -> &[CollaborationSession] {
        &self.history
    }

    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}
