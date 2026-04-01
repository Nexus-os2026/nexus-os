//! Cross-session memory with content hashes for integrity verification.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub category: String,
    pub content: String,
    pub source_session: String,
    pub content_hash: String,
}

/// Memory store with integrity verification.
pub struct MemoryStore {
    entries: Vec<MemoryEntry>,
    file_path: std::path::PathBuf,
}

impl MemoryStore {
    /// Load memory from disk (or create empty).
    pub fn load(path: std::path::PathBuf) -> Self {
        let entries = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            entries,
            file_path: path,
        }
    }

    /// Add a memory entry.
    pub fn add(&mut self, category: &str, content: &str, session_id: &str) {
        let hash = hex::encode(Sha256::digest(content.as_bytes()));
        self.entries.push(MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: chrono::Utc::now(),
            category: category.to_string(),
            content: content.to_string(),
            source_session: session_id.to_string(),
            content_hash: hash,
        });
    }

    /// Save memory to disk.
    pub fn save(&self) -> Result<(), crate::error::NxError> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| crate::error::NxError::ConfigError(e.to_string()))?;
        std::fs::write(&self.file_path, json)?;
        Ok(())
    }

    /// Verify all entries' content hashes. Returns IDs of corrupted entries.
    pub fn verify_integrity(&self) -> Vec<String> {
        let mut corrupted = Vec::new();
        for entry in &self.entries {
            let expected = hex::encode(Sha256::digest(entry.content.as_bytes()));
            if entry.content_hash != expected {
                corrupted.push(entry.id.clone());
            }
        }
        corrupted
    }

    /// Get all entries.
    pub fn entries(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Get mutable entries (for tampering tests).
    pub fn entries_mut(&mut self) -> &mut Vec<MemoryEntry> {
        &mut self.entries
    }

    /// Get entries by category.
    pub fn by_category(&self, category: &str) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Remove an entry by ID.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < before
    }

    /// Get entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
