//! Full NEXUSCODE.md parser — reads all configuration sections.

/// Parsed NEXUSCODE.md configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NexusCodeMd {
    // Project section
    pub project_name: Option<String>,
    pub language: Option<String>,
    pub build_command: Option<String>,
    pub test_command: Option<String>,
    pub lint_command: Option<String>,

    // Governance section
    pub max_file_scope: Option<String>,
    pub blocked_paths: Vec<String>,
    pub fuel_budget: Option<u64>,
    pub hitl_tier: Option<u8>,
    pub auto_approve: Vec<String>,

    // Models section
    pub execution_model: Option<String>,
    pub thinking_model: Option<String>,
    pub critique_model: Option<String>,
    pub compact_model: Option<String>,
    pub vision_model: Option<String>,

    // Memory section
    pub persist_across_sessions: bool,
    pub max_memory_entries: Option<u32>,

    // Style section
    pub prefer_short_responses: bool,
    pub show_diffs_inline: bool,
    pub auto_run_tests_after_edit: bool,
}

impl NexusCodeMd {
    /// Parse a NEXUSCODE.md file.
    /// Format is simple YAML-ish key: value under ## Section headers.
    pub fn parse(content: &str) -> Self {
        let mut config = Self::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Section headers
            if let Some(header) = trimmed.strip_prefix("## ") {
                current_section = header.trim().to_lowercase();
                continue;
            }

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            match current_section.as_str() {
                "project" => Self::parse_project(trimmed, &mut config),
                "governance" => Self::parse_governance(trimmed, &mut config),
                "models" => Self::parse_models(trimmed, &mut config),
                "memory" => Self::parse_memory(trimmed, &mut config),
                "style" => Self::parse_style(trimmed, &mut config),
                _ => {}
            }
        }

        config
    }

    fn parse_project(line: &str, config: &mut Self) {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => config.project_name = Some(value.to_string()),
                "language" => config.language = Some(value.to_string()),
                "build" => config.build_command = Some(value.to_string()),
                "test" => config.test_command = Some(value.to_string()),
                "lint" => config.lint_command = Some(value.to_string()),
                _ => {}
            }
        }
    }

    fn parse_governance(line: &str, config: &mut Self) {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "max_file_scope" => config.max_file_scope = Some(value.to_string()),
                "fuel_budget" => config.fuel_budget = value.parse().ok(),
                "hitl_tier" => config.hitl_tier = value.parse().ok(),
                "blocked_paths" => {
                    config.blocked_paths = value.split(',').map(|s| s.trim().to_string()).collect();
                }
                "auto_approve" => {
                    config.auto_approve = value.split(',').map(|s| s.trim().to_string()).collect();
                }
                _ => {}
            }
        }
    }

    fn parse_models(line: &str, config: &mut Self) {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "execution" => config.execution_model = Some(value.to_string()),
                "thinking" => config.thinking_model = Some(value.to_string()),
                "critique" => config.critique_model = Some(value.to_string()),
                "compact" => config.compact_model = Some(value.to_string()),
                "vision" => config.vision_model = Some(value.to_string()),
                _ => {}
            }
        }
    }

    fn parse_memory(line: &str, config: &mut Self) {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "persist_across_sessions" => {
                    config.persist_across_sessions = value.trim() == "true"
                }
                "max_memory_entries" => config.max_memory_entries = value.trim().parse().ok(),
                _ => {}
            }
        }
    }

    fn parse_style(line: &str, config: &mut Self) {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "prefer_short_responses" => config.prefer_short_responses = value.trim() == "true",
                "show_diffs_inline" => config.show_diffs_inline = value.trim() == "true",
                "auto_run_tests_after_edit" => {
                    config.auto_run_tests_after_edit = value.trim() == "true"
                }
                _ => {}
            }
        }
    }

    /// Load from a file path. Returns default if file doesn't exist.
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => Self::parse(&content),
            Err(_) => Self::default(),
        }
    }
}
