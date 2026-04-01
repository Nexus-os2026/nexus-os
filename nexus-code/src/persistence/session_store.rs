//! Signed session persistence with Ed25519 signatures.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A saved session with cryptographic signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub session_id: String,
    pub saved_at: chrono::DateTime<chrono::Utc>,
    pub provider: String,
    pub model: String,
    pub fuel_total: u64,
    pub fuel_consumed: u64,
    pub audit_entry_count: usize,
    pub message_count: usize,
    pub messages: Vec<crate::llm::types::Message>,
    /// SHA-256 hash of the serialized session data (excluding signature).
    pub content_hash: String,
    /// Ed25519 signature of content_hash by the session identity.
    pub signature: String,
}

impl SavedSession {
    /// Create a saved session from the current app state.
    pub fn from_app(app: &crate::app::App, messages: &[crate::llm::types::Message]) -> Self {
        let mut session = Self {
            session_id: app.governance.identity.session_id().to_string(),
            saved_at: chrono::Utc::now(),
            provider: app.config.default_provider.clone(),
            model: app.config.default_model.clone(),
            fuel_total: app.governance.fuel.budget().total,
            fuel_consumed: app.governance.fuel.budget().consumed,
            audit_entry_count: app.governance.audit.len(),
            message_count: messages.len(),
            messages: messages.to_vec(),
            content_hash: String::new(),
            signature: String::new(),
        };

        // Compute hash (excluding hash and signature fields)
        session.content_hash = session.compute_content_hash();

        // Sign with session identity
        let hash_bytes = hex::decode(&session.content_hash).unwrap_or_default();
        let sig = app.governance.identity.sign(&hash_bytes);
        session.signature = hex::encode(sig.to_bytes());

        session
    }

    /// Compute the content hash for integrity verification.
    fn compute_content_hash(&self) -> String {
        let hash_data = serde_json::json!({
            "session_id": self.session_id,
            "saved_at": self.saved_at.to_rfc3339(),
            "messages": self.messages,
            "fuel_consumed": self.fuel_consumed,
            "audit_entry_count": self.audit_entry_count,
        });
        hex::encode(Sha256::digest(
            serde_json::to_string(&hash_data)
                .unwrap_or_default()
                .as_bytes(),
        ))
    }

    /// Save to disk.
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::error::NxError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::NxError::ConfigError(format!("Serialize error: {}", e)))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load from disk.
    pub fn load(path: &std::path::Path) -> Result<Self, crate::error::NxError> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| crate::error::NxError::ConfigError(format!("Parse error: {}", e)))
    }

    /// Verify the session file's integrity (hash matches content).
    pub fn verify_integrity(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }
}
