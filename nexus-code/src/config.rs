use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::NxError;
use crate::llm::router::SlotConfig;

/// Application configuration, loaded from multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NxConfig {
    /// Fuel budget for this session (default: 50,000).
    pub fuel_budget: u64,
    /// Default LLM provider name.
    pub default_provider: String,
    /// Default model.
    pub default_model: String,
    /// Tools that are auto-approved (Tier1).
    pub auto_approve: Vec<String>,
    /// Optional glob pattern restricting file access.
    pub max_file_scope: Option<String>,
    /// Paths the agent cannot touch.
    pub blocked_paths: Vec<String>,
    /// Model slot configurations.
    pub slots: HashMap<String, SlotConfig>,
}

impl Default for NxConfig {
    fn default() -> Self {
        Self {
            fuel_budget: 50_000,
            default_provider: "anthropic".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            auto_approve: vec![
                "file_read".to_string(),
                "search".to_string(),
                "glob".to_string(),
                "lsp_query".to_string(),
            ],
            max_file_scope: None,
            blocked_paths: vec![],
            slots: HashMap::new(),
        }
    }
}

impl NxConfig {
    /// Load config with priority: env vars > .nxrc > ~/.config/nexus-code/config.toml > NEXUSCODE.md > defaults.
    pub fn load() -> Result<Self, NxError> {
        let mut config = NxConfig::default();

        // Try NEXUSCODE.md in current directory
        let nexuscode_path = Path::new("NEXUSCODE.md");
        if nexuscode_path.exists() {
            if let Ok(nc) = Self::parse_nexuscode_md(nexuscode_path) {
                config.merge_from(&nc);
            }
        }

        // Try ~/.config/nexus-code/config.toml
        if let Some(config_dir) = dirs::config_dir() {
            let global_config = config_dir.join("nexus-code").join("config.toml");
            if global_config.exists() {
                if let Ok(content) = std::fs::read_to_string(&global_config) {
                    if let Ok(file_config) = toml::from_str::<NxConfig>(&content) {
                        config.merge_from(&file_config);
                    }
                }
            }
        }

        // Try .nxrc in current directory
        let nxrc_path = Path::new(".nxrc");
        if nxrc_path.exists() {
            if let Ok(content) = std::fs::read_to_string(nxrc_path) {
                if let Ok(file_config) = toml::from_str::<NxConfig>(&content) {
                    config.merge_from(&file_config);
                }
            }
        }

        // Environment variable overrides
        if let Ok(provider) = std::env::var("NX_PROVIDER") {
            config.default_provider = provider;
        }
        if let Ok(model) = std::env::var("NX_MODEL") {
            config.default_model = model;
        }
        if let Ok(fuel) = std::env::var("NX_FUEL_BUDGET") {
            if let Ok(budget) = fuel.parse::<u64>() {
                config.fuel_budget = budget;
            }
        }

        Ok(config)
    }

    /// Parse a NEXUSCODE.md file for governance configuration.
    pub fn parse_nexuscode_md(path: &Path) -> Result<NxConfig, NxError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| NxError::ConfigError(e.to_string()))?;

        let mut config = NxConfig::default();

        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("provider:") {
                config.default_provider = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("model:") {
                config.default_model = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("fuel_budget:") {
                if let Ok(budget) = val.trim().parse::<u64>() {
                    config.fuel_budget = budget;
                }
            } else if let Some(val) = line.strip_prefix("blocked_paths:") {
                config.blocked_paths = val.split(',').map(|s| s.trim().to_string()).collect();
            }
        }

        Ok(config)
    }

    /// Get the project directory (current working directory).
    pub fn project_dir(&self) -> Option<std::path::PathBuf> {
        std::env::current_dir().ok()
    }

    /// Merge values from another config (non-default values override).
    fn merge_from(&mut self, other: &NxConfig) {
        if other.default_provider != "anthropic" {
            self.default_provider = other.default_provider.clone();
        }
        if other.default_model != "claude-sonnet-4-20250514" {
            self.default_model = other.default_model.clone();
        }
        if other.fuel_budget != 50_000 {
            self.fuel_budget = other.fuel_budget;
        }
        if !other.blocked_paths.is_empty() {
            self.blocked_paths = other.blocked_paths.clone();
        }
        for (k, v) in &other.slots {
            self.slots.insert(k.clone(), v.clone());
        }
    }
}
