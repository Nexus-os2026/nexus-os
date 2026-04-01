//! Governed self-improvement — prompt versioning with hard invariants.
//!
//! The self-improvement engine can modify system prompts but CANNOT modify:
//! 1. Consent tier classifications
//! 2. Capability ACL grants
//! 3. Fuel budget limits
//! 4. Audit trail behavior
//! 5. Behavioral envelope thresholds

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A versioned system prompt with governance metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVersion {
    pub version: u32,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub reason: String,
    /// SHA-256 of content.
    pub content_hash: String,
    /// Hash chain back to v0.
    pub previous_hash: String,
}

/// The self-improvement engine.
pub struct SelfImproveEngine {
    versions: Vec<PromptVersion>,
    current_version: u32,
}

impl SelfImproveEngine {
    pub fn new(initial_prompt: &str) -> Self {
        let hash = hex::encode(Sha256::digest(initial_prompt.as_bytes()));
        let genesis = PromptVersion {
            version: 0,
            content: initial_prompt.to_string(),
            created_at: chrono::Utc::now(),
            reason: "Initial system prompt".to_string(),
            content_hash: hash,
            previous_hash: "0".repeat(64),
        };
        Self {
            versions: vec![genesis],
            current_version: 0,
        }
    }

    /// Propose a prompt modification. Not applied until `apply()` is called.
    pub fn propose(
        &self,
        new_content: &str,
        reason: &str,
    ) -> Result<PromptVersion, crate::error::NxError> {
        self.validate_invariants(new_content)?;

        let hash = hex::encode(Sha256::digest(new_content.as_bytes()));
        let previous_hash = self
            .versions
            .last()
            .map(|v| v.content_hash.clone())
            .unwrap_or_else(|| "0".repeat(64));

        Ok(PromptVersion {
            version: self.current_version + 1,
            content: new_content.to_string(),
            created_at: chrono::Utc::now(),
            reason: reason.to_string(),
            content_hash: hash,
            previous_hash,
        })
    }

    /// Apply a proposed version.
    pub fn apply(&mut self, version: PromptVersion) {
        self.current_version = version.version;
        self.versions.push(version);
    }

    /// Validate that a prompt doesn't violate hard invariants.
    fn validate_invariants(&self, new_content: &str) -> Result<(), crate::error::NxError> {
        let lower = new_content.to_lowercase();

        let bypass_patterns = [
            "skip consent",
            "bypass consent",
            "auto-approve all",
            "ignore permissions",
            "disable governance",
            "skip approval",
            "no consent needed",
            "approve everything",
        ];
        for pattern in &bypass_patterns {
            if lower.contains(pattern) {
                return Err(crate::error::NxError::ConfigError(format!(
                    "Self-improvement invariant violation: '{}'",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Get the current prompt content.
    pub fn current_prompt(&self) -> &str {
        self.versions
            .last()
            .map(|v| v.content.as_str())
            .unwrap_or("")
    }

    /// Get version history.
    pub fn versions(&self) -> &[PromptVersion] {
        &self.versions
    }

    /// Get current version number.
    pub fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Verify the version chain integrity.
    pub fn verify_chain(&self) -> bool {
        for i in 1..self.versions.len() {
            if self.versions[i].previous_hash != self.versions[i - 1].content_hash {
                return false;
            }
        }
        true
    }
}
