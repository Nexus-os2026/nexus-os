use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::NexusCoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCapability {
    pub domain: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemCapability {
    pub path: String,
    pub permission: FilesystemPermission,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemPermission {
    ReadOnly,
    ReadWrite,
    WriteOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySet {
    pub network: Vec<NetworkCapability>,
    pub filesystem: Vec<FilesystemCapability>,
    pub spawn_agents: bool,
    pub messaging: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub agent: AgentMetadata,
    pub capabilities: CapabilitySet,
    pub fuel: FuelPolicy,
    pub audit: AuditPolicy,
    pub privacy: PrivacyPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelPolicy {
    pub max_llm_calls: u64,
    pub max_tool_calls: u64,
    pub max_wall_clock_seconds: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    pub log_llm_inputs: bool,
    pub log_llm_outputs: bool,
    pub log_tool_calls: bool,
    pub encrypt_at_rest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub data_classification: DataClassification,
    pub erasure_strategy: ErasureStrategy,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataClassification {
    Personal,
    Anonymous,
    Sensitive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErasureStrategy {
    KeyDeletion,
    SecureOverwrite,
    LogicalDelete,
}

impl AgentManifest {
    pub fn from_file(path: &Path) -> Result<Self, NexusCoreError> {
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|e| NexusCoreError::ManifestParseError {
            path: path.display().to_string(),
            source: e,
        })
    }

    pub fn permits_network(&self, domain: &str) -> bool {
        self.capabilities.network.iter().any(|n| n.domain == domain)
    }

    pub fn permits_filesystem(&self, path: &str, write: bool) -> bool {
        self.capabilities.filesystem.iter().any(|f| {
            path.starts_with(&f.path)
                && if write {
                    f.permission == FilesystemPermission::ReadWrite
                        || f.permission == FilesystemPermission::WriteOnly
                } else {
                    true
                }
        })
    }
}
