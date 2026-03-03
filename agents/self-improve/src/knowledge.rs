use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeCategory {
    CodingPatterns,
    PostingStrategies,
    DesignPrinciples,
    WorkflowOptimizations,
    ErrorSolutions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: u64,
    pub agent_id: String,
    pub category: KnowledgeCategory,
    pub strategy: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalHit {
    pub entry: KnowledgeEntry,
    pub similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeDb {
    entries: Vec<KnowledgeEntry>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    storage_path: Option<PathBuf>,
    key_seed: String,
    entries: Vec<KnowledgeEntry>,
    next_id: u64,
}

impl KnowledgeBase {
    pub fn new_in_memory(agent_scope_key: &str) -> Self {
        Self {
            storage_path: None,
            key_seed: agent_scope_key.to_string(),
            entries: Vec::new(),
            next_id: 1,
        }
    }

    pub fn new_with_file(
        path: impl AsRef<Path>,
        agent_scope_key: &str,
    ) -> Result<Self, AgentError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Ok(Self {
                storage_path: Some(path),
                key_seed: agent_scope_key.to_string(),
                entries: Vec::new(),
                next_id: 1,
            });
        }

        let encrypted_hex = fs::read_to_string(path.as_path()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to read knowledge db '{}': {error}",
                path.display()
            ))
        })?;
        let cipher = hex_to_bytes(encrypted_hex.trim())?;
        let plain = xor_cipher(cipher.as_slice(), key(agent_scope_key).as_slice());
        let db = serde_json::from_slice::<KnowledgeDb>(plain.as_slice()).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to parse decrypted knowledge db '{}': {error}",
                path.display()
            ))
        })?;

        let next_id = db.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
        Ok(Self {
            storage_path: Some(path),
            key_seed: agent_scope_key.to_string(),
            entries: db.entries,
            next_id,
        })
    }

    pub fn store_strategy(
        &mut self,
        agent_id: &str,
        category: KnowledgeCategory,
        strategy: &str,
        tags: &[&str],
    ) -> Result<KnowledgeEntry, AgentError> {
        let entry = KnowledgeEntry {
            id: self.next_id,
            agent_id: agent_id.to_string(),
            category,
            strategy: strategy.to_string(),
            tags: tags.iter().map(|value| value.to_string()).collect(),
        };
        self.next_id = self.next_id.saturating_add(1);
        self.entries.push(entry.clone());
        self.persist()?;
        Ok(entry)
    }

    pub fn retrieve(&self, agent_id: &str, query: &str, limit: usize) -> Vec<RetrievalHit> {
        let query_tokens = tokens(query);
        let mut hits = self
            .entries
            .iter()
            .filter(|entry| entry.agent_id == agent_id)
            .map(|entry| RetrievalHit {
                entry: entry.clone(),
                similarity: similarity(
                    query_tokens.as_slice(),
                    tokens(format!("{} {}", entry.strategy, entry.tags.join(" ")).as_str())
                        .as_slice(),
                ),
            })
            .filter(|hit| hit.similarity > 0.0)
            .collect::<Vec<_>>();

        hits.sort_by(|left, right| {
            right
                .similarity
                .partial_cmp(&left.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit.max(1));
        hits
    }

    pub fn entries(&self) -> &[KnowledgeEntry] {
        &self.entries
    }

    fn persist(&self) -> Result<(), AgentError> {
        let Some(path) = &self.storage_path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed to create knowledge db parent '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let db = KnowledgeDb {
            entries: self.entries.clone(),
        };
        let plain = serde_json::to_vec(&db).map_err(|error| {
            AgentError::SupervisorError(format!("failed to serialize knowledge db: {error}"))
        })?;
        let cipher = xor_cipher(plain.as_slice(), key(self.key_seed.as_str()).as_slice());
        let encrypted_hex = bytes_to_hex(cipher.as_slice());
        fs::write(path, encrypted_hex).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to write encrypted knowledge db '{}': {error}",
                path.display()
            ))
        })
    }
}

fn tokens(input: &str) -> Vec<String> {
    input
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_string())
        .collect()
}

fn similarity(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let left_set = left.iter().cloned().collect::<HashSet<_>>();
    let right_set = right.iter().cloned().collect::<HashSet<_>>();
    let intersection = left_set.intersection(&right_set).count() as f64;
    let union = left_set.union(&right_set).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn key(seed: &str) -> Vec<u8> {
    let mut bytes = seed.as_bytes().to_vec();
    if bytes.is_empty() {
        bytes.push(0x5a);
    }
    bytes
}

fn xor_cipher(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect()
}

fn bytes_to_hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for byte in data {
        out.push_str(format!("{:02x}", byte).as_str());
    }
    out
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, AgentError> {
    if !hex.len().is_multiple_of(2) {
        return Err(AgentError::SupervisorError(
            "invalid encrypted knowledge db length".to_string(),
        ));
    }

    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let chars = hex.as_bytes();
    for index in (0..chars.len()).step_by(2) {
        let pair = std::str::from_utf8(&chars[index..index + 2]).map_err(|error| {
            AgentError::SupervisorError(format!("invalid encrypted knowledge data: {error}"))
        })?;
        let value = u8::from_str_radix(pair, 16).map_err(|error| {
            AgentError::SupervisorError(format!("invalid encrypted byte '{pair}': {error}"))
        })?;
        bytes.push(value);
    }
    Ok(bytes)
}
