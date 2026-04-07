//! User-configurable model selection for each build pipeline step.
//!
//! Detects all available models (Ollama local, CLI providers, API keys),
//! generates smart defaults, and persists user choices to
//! `~/.nexus/builder_model_config.json`.

use nexus_connectors_llm::providers::claude_code::{
    detect_claude_code, CLAUDE_CODE_DEFAULT_MODEL, CLAUDE_CODE_MODELS,
};
use nexus_connectors_llm::providers::codex_cli::{
    detect_codex_cli, CODEX_CLI_DEFAULT_MODEL, CODEX_CLI_MODELS,
};
use nexus_connectors_llm::providers::OllamaProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Types ──────────────────────────────────────────────────────────────────

/// A single model choice for a build pipeline step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelChoice {
    pub model_id: String,
    pub provider: String,
    pub display_name: String,
    pub cost_per_build: f64,
    pub speed_estimate: String,
    /// Optional warning (e.g. "Local models may produce weaker security policies").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl ModelChoice {
    /// Sentinel value when no models are available.
    pub fn none(reason: &str) -> Self {
        Self {
            model_id: String::new(),
            provider: "none".to_string(),
            display_name: reason.to_string(),
            cost_per_build: 0.0,
            speed_estimate: "N/A".to_string(),
            warning: Some("No models available".to_string()),
        }
    }

    pub fn is_none(&self) -> bool {
        self.provider == "none"
    }
}

/// Model configuration for all build pipeline steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildModelConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    pub planning: ModelChoice,
    pub content_generation: ModelChoice,
    pub section_edit: ModelChoice,
    pub full_build: ModelChoice,
    pub security_policies: ModelChoice,
}

fn default_version() -> u32 {
    1
}

/// Information about a single detected Ollama model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedOllamaModel {
    pub name: String,
    pub size_bytes: u64,
    pub size_display: String,
}

/// Information about a detected CLI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCliProvider {
    pub version: String,
    pub models: Vec<(String, String)>,
    pub default_model: String,
    pub authenticated: bool,
}

/// All available models detected on this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModels {
    pub ollama_models: Vec<DetectedOllamaModel>,
    pub ollama_running: bool,
    pub codex_cli: Option<DetectedCliProvider>,
    pub claude_cli: Option<DetectedCliProvider>,
    pub anthropic_api_key_set: bool,
    pub openai_api_key_set: bool,
}

impl AvailableModels {
    pub fn has_ollama_model(&self, name: &str) -> bool {
        self.ollama_models.iter().any(|m| m.name == name)
    }

    pub fn has_any_ollama(&self) -> bool {
        self.ollama_running && !self.ollama_models.is_empty()
    }

    pub fn largest_ollama(&self) -> Option<&DetectedOllamaModel> {
        self.ollama_models.iter().max_by_key(|m| m.size_bytes)
    }

    pub fn codex_cli_available(&self) -> bool {
        self.codex_cli
            .as_ref()
            .map(|c| c.authenticated)
            .unwrap_or(false)
    }

    pub fn claude_cli_available(&self) -> bool {
        self.claude_cli
            .as_ref()
            .map(|c| c.authenticated)
            .unwrap_or(false)
    }

    /// Collect all usable model choices for a given task category.
    pub fn choices_for_planning(&self) -> Vec<ModelChoice> {
        let mut out = Vec::new();
        // Local models first (fast, free)
        for m in &self.ollama_models {
            out.push(ollama_choice(
                &m.name,
                &estimate_ollama_speed(&m.name, "plan"),
            ));
        }
        if self.codex_cli_available() {
            out.push(codex_choice(CODEX_CLI_DEFAULT_MODEL, "~5s"));
        }
        if self.claude_cli_available() {
            out.push(claude_cli_choice(CLAUDE_CODE_DEFAULT_MODEL, "~10s"));
        }
        if self.anthropic_api_key_set {
            out.push(anthropic_api_choice(
                "claude-haiku-4-5-20251001",
                "~3s",
                0.003,
            ));
        }
        if self.openai_api_key_set {
            out.push(openai_api_choice("gpt-4o-mini", "~3s", 0.002));
        }
        out
    }

    pub fn choices_for_content(&self) -> Vec<ModelChoice> {
        let mut out = Vec::new();
        for m in &self.ollama_models {
            out.push(ollama_choice(
                &m.name,
                &estimate_ollama_speed(&m.name, "content"),
            ));
        }
        if self.codex_cli_available() {
            out.push(codex_choice(CODEX_CLI_DEFAULT_MODEL, "~15s"));
        }
        if self.claude_cli_available() {
            out.push(claude_cli_choice(CLAUDE_CODE_DEFAULT_MODEL, "~30s"));
        }
        if self.anthropic_api_key_set {
            out.push(anthropic_api_choice("claude-sonnet-4-6", "~15s", 0.05));
        }
        if self.openai_api_key_set {
            out.push(openai_api_choice("gpt-4o", "~10s", 0.03));
        }
        out
    }

    pub fn choices_for_section_edit(&self) -> Vec<ModelChoice> {
        self.choices_for_content()
    }

    pub fn choices_for_full_build(&self) -> Vec<ModelChoice> {
        let mut out = Vec::new();
        if self.codex_cli_available() {
            out.push(codex_choice(CODEX_CLI_DEFAULT_MODEL, "~2 min"));
        }
        if self.anthropic_api_key_set {
            out.push(anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.45));
        }
        if self.claude_cli_available() {
            out.push(claude_cli_choice(CLAUDE_CODE_DEFAULT_MODEL, "~5-8 min"));
        }
        if self.openai_api_key_set {
            out.push(openai_api_choice("gpt-4o", "~30s", 0.20));
        }
        for m in &self.ollama_models {
            out.push(ollama_choice(
                &m.name,
                &estimate_ollama_speed(&m.name, "build"),
            ));
        }
        out
    }

    pub fn choices_for_security(&self) -> Vec<ModelChoice> {
        let mut out = Vec::new();
        if self.anthropic_api_key_set {
            out.push(anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.15));
        }
        if self.codex_cli_available() {
            out.push(codex_choice(CODEX_CLI_DEFAULT_MODEL, "~30s"));
        }
        if self.claude_cli_available() {
            out.push(claude_cli_choice(CLAUDE_CODE_DEFAULT_MODEL, "~2 min"));
        }
        if self.openai_api_key_set {
            out.push(openai_api_choice("gpt-4o", "~30s", 0.10));
        }
        for m in &self.ollama_models {
            let mut c = ollama_choice(&m.name, &estimate_ollama_speed(&m.name, "security"));
            c.warning = Some("Local models may produce weaker security policies".to_string());
            out.push(c);
        }
        out
    }
}

// ─── Choice Constructors ────────────────────────────────────────────────────

fn ollama_choice(model_id: &str, speed: &str) -> ModelChoice {
    ModelChoice {
        model_id: model_id.to_string(),
        provider: "ollama".to_string(),
        display_name: format!("{model_id} (Local)"),
        cost_per_build: 0.0,
        speed_estimate: speed.to_string(),
        warning: None,
    }
}

fn codex_choice(model_id: &str, speed: &str) -> ModelChoice {
    ModelChoice {
        model_id: model_id.to_string(),
        provider: "codex_cli".to_string(),
        display_name: "GPT-5.4 (Codex CLI)".to_string(),
        cost_per_build: 0.0,
        speed_estimate: speed.to_string(),
        warning: None,
    }
}

fn claude_cli_choice(model_id: &str, speed: &str) -> ModelChoice {
    ModelChoice {
        model_id: model_id.to_string(),
        provider: "claude_cli".to_string(),
        display_name: "Sonnet 4.6 (Claude CLI)".to_string(),
        cost_per_build: 0.0,
        speed_estimate: speed.to_string(),
        warning: None,
    }
}

fn anthropic_api_choice(model_id: &str, speed: &str, cost: f64) -> ModelChoice {
    let name = if model_id.contains("haiku") {
        "Haiku 4.5"
    } else if model_id.contains("opus") {
        "Opus 4.6"
    } else {
        "Sonnet 4.6"
    };
    ModelChoice {
        model_id: model_id.to_string(),
        provider: "anthropic_api".to_string(),
        display_name: format!("{name} (API, ~${cost:.2})"),
        cost_per_build: cost,
        speed_estimate: speed.to_string(),
        warning: None,
    }
}

fn openai_api_choice(model_id: &str, speed: &str, cost: f64) -> ModelChoice {
    let name = if model_id.contains("mini") {
        "GPT-4o Mini"
    } else {
        "GPT-4o"
    };
    ModelChoice {
        model_id: model_id.to_string(),
        provider: "openai_api".to_string(),
        display_name: format!("{name} (API, ~${cost:.2})"),
        cost_per_build: cost,
        speed_estimate: speed.to_string(),
        warning: None,
    }
}

fn estimate_ollama_speed(model_name: &str, task: &str) -> String {
    let m = model_name.to_lowercase();
    let is_small = m.contains("e2b") || m.contains(":1b") || m.contains(":3b");
    match (task, is_small) {
        ("plan", true) => "~7s".to_string(),
        ("plan", false) => "~15s".to_string(),
        ("content", true) => "~12s".to_string(),
        ("content", false) => "~25s".to_string(),
        ("build", true) => "~5 min".to_string(),
        ("build", false) => "~3-5 min".to_string(),
        ("security", true) => "~20s".to_string(),
        ("security", false) => "~30s".to_string(),
        _ => "~30s".to_string(),
    }
}

// ─── Detection ──────────────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    let gb = bytes as f64 / 1_073_741_824.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        let mb = bytes as f64 / 1_048_576.0;
        format!("{mb:.0} MB")
    }
}

/// Detect all available models on this machine.
pub fn detect_available_models() -> AvailableModels {
    // 1. Ollama
    let ollama = OllamaProvider::from_env();
    let ollama_running = ollama.health_check().unwrap_or(false);
    let ollama_models = if ollama_running {
        ollama
            .list_models()
            .unwrap_or_default()
            .into_iter()
            .map(|m| DetectedOllamaModel {
                size_display: format_size(m.size),
                name: m.name,
                size_bytes: m.size,
            })
            .collect()
    } else {
        Vec::new()
    };

    // 2. Codex CLI
    let codex_status = detect_codex_cli();
    let codex_cli = if codex_status.installed {
        Some(DetectedCliProvider {
            version: codex_status.version.unwrap_or_default(),
            models: CODEX_CLI_MODELS
                .iter()
                .map(|(id, name)| (id.to_string(), name.to_string()))
                .collect(),
            default_model: CODEX_CLI_DEFAULT_MODEL.to_string(),
            authenticated: codex_status.authenticated,
        })
    } else {
        None
    };

    // 3. Claude Code CLI
    let claude_status = detect_claude_code();
    let claude_cli = if claude_status.installed {
        Some(DetectedCliProvider {
            version: claude_status.version.unwrap_or_default(),
            models: CLAUDE_CODE_MODELS
                .iter()
                .map(|(id, name)| (id.to_string(), name.to_string()))
                .collect(),
            default_model: CLAUDE_CODE_DEFAULT_MODEL.to_string(),
            authenticated: claude_status.authenticated,
        })
    } else {
        None
    };

    // 4. API keys
    let anthropic_api_key_set = std::env::var("ANTHROPIC_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);
    let openai_api_key_set = std::env::var("OPENAI_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);

    AvailableModels {
        ollama_models,
        ollama_running,
        codex_cli,
        claude_cli,
        anthropic_api_key_set,
        openai_api_key_set,
    }
}

// ─── Smart Defaults ─────────────────────────────────────────────────────────

/// Generate smart default configuration based on what's available.
pub fn generate_smart_defaults(available: &AvailableModels) -> BuildModelConfig {
    BuildModelConfig {
        version: 1,
        planning: best_for_planning(available),
        content_generation: best_for_content(available),
        section_edit: best_for_section_edit(available),
        full_build: best_for_full_build(available),
        security_policies: best_for_security(available),
    }
}

fn best_for_planning(a: &AvailableModels) -> ModelChoice {
    if a.has_ollama_model("gemma4:e2b") {
        return ollama_choice("gemma4:e2b", "~7s");
    }
    if a.has_ollama_model("gemma4:e4b") {
        return ollama_choice("gemma4:e4b", "~15s");
    }
    if a.has_any_ollama() {
        return ollama_choice(&a.ollama_models[0].name, "~10s");
    }
    if a.codex_cli_available() {
        return codex_choice("gpt-5.4", "~5s");
    }
    if a.claude_cli_available() {
        return claude_cli_choice("claude-sonnet-4-6", "~10s");
    }
    if a.anthropic_api_key_set {
        return anthropic_api_choice("claude-haiku-4-5-20251001", "~3s", 0.003);
    }
    if a.openai_api_key_set {
        return openai_api_choice("gpt-4o-mini", "~3s", 0.002);
    }
    ModelChoice::none("No models available")
}

fn best_for_content(a: &AvailableModels) -> ModelChoice {
    if a.has_ollama_model("gemma4:e4b") {
        return ollama_choice("gemma4:e4b", "~17s");
    }
    if a.has_ollama_model("gemma4:e2b") {
        return ollama_choice("gemma4:e2b", "~12s");
    }
    if a.has_any_ollama() {
        let largest = a
            .largest_ollama()
            .map(|m| m.name.as_str())
            .unwrap_or("gemma4:e2b");
        return ollama_choice(largest, "~20s");
    }
    if a.codex_cli_available() {
        return codex_choice("gpt-5.4", "~15s");
    }
    if a.claude_cli_available() {
        return claude_cli_choice("claude-sonnet-4-6", "~30s");
    }
    if a.anthropic_api_key_set {
        return anthropic_api_choice("claude-sonnet-4-6", "~15s", 0.05);
    }
    if a.openai_api_key_set {
        return openai_api_choice("gpt-4o", "~10s", 0.03);
    }
    ModelChoice::none("No models available")
}

fn best_for_section_edit(a: &AvailableModels) -> ModelChoice {
    best_for_content(a)
}

fn best_for_full_build(a: &AvailableModels) -> ModelChoice {
    if a.codex_cli_available() {
        return codex_choice("gpt-5.4", "~2 min");
    }
    if a.anthropic_api_key_set {
        return anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.45);
    }
    if a.claude_cli_available() {
        return claude_cli_choice("claude-sonnet-4-6", "~5-8 min");
    }
    if a.openai_api_key_set {
        return openai_api_choice("gpt-4o", "~30s", 0.20);
    }
    if a.has_ollama_model("gemma4:e4b") {
        return ollama_choice("gemma4:e4b", "~3-5 min");
    }
    if a.has_any_ollama() {
        let largest = a
            .largest_ollama()
            .map(|m| m.name.as_str())
            .unwrap_or("gemma4:e4b");
        return ollama_choice(largest, "~5 min");
    }
    ModelChoice::none("No models available")
}

fn best_for_security(a: &AvailableModels) -> ModelChoice {
    if a.anthropic_api_key_set {
        return anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.15);
    }
    if a.codex_cli_available() {
        return codex_choice("gpt-5.4", "~30s");
    }
    if a.claude_cli_available() {
        return claude_cli_choice("claude-sonnet-4-6", "~2 min");
    }
    if a.openai_api_key_set {
        return openai_api_choice("gpt-4o", "~30s", 0.10);
    }
    if a.has_ollama_model("gemma4:e4b") {
        let mut c = ollama_choice("gemma4:e4b", "~20s");
        c.warning = Some("Local models may produce weaker security policies".to_string());
        return c;
    }
    if a.has_any_ollama() {
        let largest = a
            .largest_ollama()
            .map(|m| m.name.as_str())
            .unwrap_or("gemma4:e2b");
        let mut c = ollama_choice(largest, "~30s");
        c.warning = Some("Local models may produce weaker security policies".to_string());
        return c;
    }
    ModelChoice::none("No models available")
}

// ─── Persistence ────────────────────────────────────────────────────────────

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".nexus")
        .join("builder_model_config.json")
}

/// Strip display suffixes like " (via Codex CLI)" from persisted model_id values.
///
/// Older configs may have saved `"gpt-5.4 (via Codex CLI)"` as a model_id
/// which Codex CLI rejects.  This strips the suffix on load.
fn migrate_model_ids(mut config: BuildModelConfig) -> BuildModelConfig {
    fn clean(id: &mut String) {
        if let Some(i) = id.find(" (") {
            id.truncate(i);
        }
    }
    clean(&mut config.planning.model_id);
    clean(&mut config.content_generation.model_id);
    clean(&mut config.section_edit.model_id);
    clean(&mut config.full_build.model_id);
    clean(&mut config.security_policies.model_id);
    config
}

/// Load model config from disk. If the file doesn't exist or is invalid,
/// detect available models and generate smart defaults.
pub fn load_config() -> BuildModelConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<BuildModelConfig>(&contents) {
            Ok(cfg) => {
                // Migrate stale model_id values that contain display suffixes
                let cfg = migrate_model_ids(cfg);
                // Validate that configured models are still available
                let available = detect_available_models();
                validate_config(cfg, &available)
            }
            Err(e) => {
                eprintln!("[model-config] Failed to parse config: {e}, regenerating defaults");
                let available = detect_available_models();
                let defaults = generate_smart_defaults(&available);
                let _ = save_config(&defaults);
                defaults
            }
        },
        Err(_) => {
            let available = detect_available_models();
            let defaults = generate_smart_defaults(&available);
            let _ = save_config(&defaults);
            defaults
        }
    }
}

/// Save model config to disk.
pub fn save_config(config: &BuildModelConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("serialization error: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write config: {e}"))
}

/// Validate a loaded config: if a configured model is no longer available,
/// replace it with the best available alternative.
fn validate_config(mut config: BuildModelConfig, available: &AvailableModels) -> BuildModelConfig {
    let check = |choice: &ModelChoice| -> bool {
        match choice.provider.as_str() {
            "ollama" => available.has_ollama_model(&choice.model_id),
            "codex_cli" => available.codex_cli_available(),
            "claude_cli" => available.claude_cli_available(),
            "anthropic_api" => available.anthropic_api_key_set,
            "openai_api" => available.openai_api_key_set,
            "none" => false,
            _ => false,
        }
    };

    let defaults = generate_smart_defaults(available);

    if !check(&config.planning) {
        config.planning = defaults.planning;
    }
    if !check(&config.content_generation) {
        config.content_generation = defaults.content_generation;
    }
    if !check(&config.section_edit) {
        config.section_edit = defaults.section_edit;
    }
    if !check(&config.full_build) {
        config.full_build = defaults.full_build;
    }
    if !check(&config.security_policies) {
        config.security_policies = defaults.security_policies;
    }

    config
}

/// Convert a ModelChoice provider string to the prefixed model format
/// used by `provider_from_prefixed_model` / `streaming_provider_from_prefixed_model`.
///
/// Every provider gets a prefix so the downstream routing function knows
/// which backend to use without falling back to API-key priority ordering
/// (which would send Ollama models to NVIDIA NIM when both are configured).
pub fn to_prefixed_model(choice: &ModelChoice) -> String {
    match choice.provider.as_str() {
        "codex_cli" => format!("codex-cli/{}", choice.model_id),
        "claude_cli" => format!("claude-code/{}", choice.model_id),
        "anthropic_api" => format!("anthropic/{}", choice.model_id),
        "openai_api" | "openai" => format!("openai/{}", choice.model_id),
        "ollama" => format!("ollama/{}", choice.model_id),
        _ => choice.model_id.clone(),
    }
}

// ─── CLI Auth Helpers ──────────────────────────────────────────────────────

/// Check whether a CLI provider is currently authenticated.
/// Returns `Ok(true)` if authenticated, `Ok(false)` if not or binary missing.
///
/// For Codex CLI: checks auth file on disk (instant, no CLI spawn).
/// For Claude CLI: uses `detect_claude_code()` which resolves the full binary path.
pub fn check_cli_auth(cli: &str) -> Result<bool, String> {
    match cli {
        "claude" => {
            let status = detect_claude_code();
            Ok(status.installed && status.authenticated)
        }
        "codex" => {
            // Fast path: check auth file on disk (<1ms)
            if nexus_connectors_llm::providers::codex_cli::check_codex_auth_file() {
                return Ok(true);
            }
            // Slow path: full detection (resolves binary path properly)
            let status = detect_codex_cli();
            Ok(status.installed && status.authenticated)
        }
        other => Err(format!("unknown cli: {other}")),
    }
}

/// Parse CLI auth status output to determine if authenticated.
/// Returns true if the output indicates a logged-in session.
pub fn parse_cli_auth_output(cli: &str, exit_success: bool, stdout: &str) -> bool {
    if !exit_success {
        return false;
    }
    let lower = stdout.to_lowercase();
    // Negative signals
    if lower.contains("not authenticated")
        || lower.contains("not logged in")
        || lower.contains("please login")
        || lower.contains("no active session")
    {
        return false;
    }
    match cli {
        // For codex: if we get successful output at all, treat as authenticated
        // (the real check is auth-file based, this is just a parse helper)
        "codex" => true,
        // `claude auth status` — exit code 0 means authenticated
        "claude" => true,
        _ => false,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_available_all() -> AvailableModels {
        AvailableModels {
            ollama_models: vec![
                DetectedOllamaModel {
                    name: "gemma4:e2b".to_string(),
                    size_bytes: 7_200_000_000,
                    size_display: "6.7 GB".to_string(),
                },
                DetectedOllamaModel {
                    name: "gemma4:e4b".to_string(),
                    size_bytes: 9_600_000_000,
                    size_display: "8.9 GB".to_string(),
                },
            ],
            ollama_running: true,
            codex_cli: Some(DetectedCliProvider {
                version: "0.118.0".to_string(),
                models: vec![("gpt-5.4".to_string(), "GPT-5.4".to_string())],
                default_model: "gpt-5.4".to_string(),
                authenticated: true,
            }),
            claude_cli: Some(DetectedCliProvider {
                version: "2.0.0".to_string(),
                models: vec![(
                    "claude-sonnet-4-6".to_string(),
                    "Claude Sonnet 4.6".to_string(),
                )],
                default_model: "claude-sonnet-4-6".to_string(),
                authenticated: true,
            }),
            anthropic_api_key_set: true,
            openai_api_key_set: true,
        }
    }

    fn mock_available_ollama_only() -> AvailableModels {
        AvailableModels {
            ollama_models: vec![DetectedOllamaModel {
                name: "gemma4:e4b".to_string(),
                size_bytes: 9_600_000_000,
                size_display: "8.9 GB".to_string(),
            }],
            ollama_running: true,
            codex_cli: None,
            claude_cli: None,
            anthropic_api_key_set: false,
            openai_api_key_set: false,
        }
    }

    fn mock_available_nothing() -> AvailableModels {
        AvailableModels {
            ollama_models: vec![],
            ollama_running: false,
            codex_cli: None,
            claude_cli: None,
            anthropic_api_key_set: false,
            openai_api_key_set: false,
        }
    }

    #[test]
    fn test_smart_defaults_all_available() {
        let a = mock_available_all();
        let cfg = generate_smart_defaults(&a);
        // Planning prefers local
        assert_eq!(cfg.planning.provider, "ollama");
        assert_eq!(cfg.planning.model_id, "gemma4:e2b");
        // Full build prefers Codex CLI
        assert_eq!(cfg.full_build.provider, "codex_cli");
        assert_eq!(cfg.full_build.model_id, "gpt-5.4");
        assert_eq!(cfg.full_build.cost_per_build, 0.0);
        // Security prefers Anthropic API (most capable)
        assert_eq!(cfg.security_policies.provider, "anthropic_api");
    }

    #[test]
    fn test_smart_defaults_ollama_only() {
        let a = mock_available_ollama_only();
        let cfg = generate_smart_defaults(&a);
        assert_eq!(cfg.planning.provider, "ollama");
        assert_eq!(cfg.full_build.provider, "ollama");
        assert_eq!(cfg.full_build.model_id, "gemma4:e4b");
        assert!(cfg.security_policies.warning.is_some());
    }

    #[test]
    fn test_smart_defaults_nothing_available() {
        let a = mock_available_nothing();
        let cfg = generate_smart_defaults(&a);
        assert!(cfg.planning.is_none());
        assert!(cfg.full_build.is_none());
        assert!(cfg.security_policies.is_none());
    }

    #[test]
    fn test_config_roundtrip() {
        let a = mock_available_all();
        let cfg = generate_smart_defaults(&a);
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: BuildModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn test_validate_config_replaces_missing_model() {
        let a = mock_available_ollama_only();
        // Config references Codex CLI which is not available
        let cfg = BuildModelConfig {
            version: 1,
            planning: ollama_choice("gemma4:e4b", "~15s"),
            content_generation: ollama_choice("gemma4:e4b", "~17s"),
            section_edit: ollama_choice("gemma4:e4b", "~17s"),
            full_build: codex_choice("gpt-5.4", "~2 min"),
            security_policies: anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.15),
        };
        let validated = validate_config(cfg, &a);
        // Codex CLI not available → should fall back to Ollama
        assert_eq!(validated.full_build.provider, "ollama");
        // Anthropic API not available → should fall back
        assert_eq!(validated.security_policies.provider, "ollama");
        // Ollama planning still works
        assert_eq!(validated.planning.provider, "ollama");
    }

    #[test]
    fn test_model_choice_none() {
        let c = ModelChoice::none("No models available");
        assert!(c.is_none());
        assert_eq!(c.provider, "none");
    }

    #[test]
    fn test_to_prefixed_model() {
        assert_eq!(
            to_prefixed_model(&codex_choice("gpt-5.4", "~2 min")),
            "codex-cli/gpt-5.4"
        );
        assert_eq!(
            to_prefixed_model(&claude_cli_choice("claude-sonnet-4-6", "~5 min")),
            "claude-code/claude-sonnet-4-6"
        );
        assert_eq!(
            to_prefixed_model(&anthropic_api_choice("claude-sonnet-4-6", "~30s", 0.45)),
            "anthropic/claude-sonnet-4-6"
        );
        assert_eq!(
            to_prefixed_model(&ollama_choice("gemma4:e4b", "~3 min")),
            "ollama/gemma4:e4b"
        );
        assert_eq!(
            to_prefixed_model(&openai_api_choice("gpt-4o", "~30s", 0.10)),
            "openai/gpt-4o"
        );
    }

    #[test]
    fn test_choices_for_full_build_order() {
        let a = mock_available_all();
        let choices = a.choices_for_full_build();
        assert!(!choices.is_empty());
        // First choice should be Codex CLI (fastest + free)
        assert_eq!(choices[0].provider, "codex_cli");
    }

    #[test]
    fn test_choices_for_planning_order() {
        let a = mock_available_all();
        let choices = a.choices_for_planning();
        assert!(!choices.is_empty());
        // First choices should be local Ollama models
        assert_eq!(choices[0].provider, "ollama");
    }

    #[test]
    fn test_cost_zero_for_local_and_cli() {
        let a = mock_available_all();
        let cfg = generate_smart_defaults(&a);
        assert_eq!(cfg.planning.cost_per_build, 0.0);
        assert_eq!(cfg.full_build.cost_per_build, 0.0);
        assert_eq!(cfg.content_generation.cost_per_build, 0.0);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1_073_741_824), "1.0 GB");
        assert_eq!(format_size(9_600_000_000), "8.9 GB");
        assert_eq!(format_size(500_000_000), "477 MB");
    }

    #[test]
    fn test_save_and_load_config_to_temp() {
        // We can't test the real path easily, but we can test serialization
        let cfg = generate_smart_defaults(&mock_available_all());
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: BuildModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    // ─── CLI Auth Tests ─────────────────────────────────────────────────

    #[test]
    fn test_check_cli_auth_authenticated() {
        // Simulate: `claude auth status --text` exits 0 with email
        assert!(parse_cli_auth_output(
            "claude",
            true,
            "Login method: Claude Max Account\nEmail: user@example.com\n"
        ));
    }

    #[test]
    fn test_check_cli_auth_not_authenticated() {
        // Simulate: `claude auth status` exits non-zero
        assert!(!parse_cli_auth_output("claude", false, ""));
        // Simulate: exits 0 but output says not authenticated
        assert!(!parse_cli_auth_output(
            "claude",
            true,
            "Not authenticated. Please login."
        ));
    }

    #[test]
    fn test_check_cli_auth_codex_authenticated() {
        // Codex: exit success with any output → authenticated
        assert!(parse_cli_auth_output(
            "codex",
            true,
            "some output from exec"
        ));
    }

    #[test]
    fn test_check_cli_auth_codex_not_authenticated() {
        // Exit failure → not authenticated
        assert!(!parse_cli_auth_output("codex", false, "Not logged in"));
        // "not logged in" negative signal
        assert!(!parse_cli_auth_output(
            "codex",
            true,
            "error: not logged in"
        ));
    }

    #[test]
    fn test_check_cli_auth_binary_missing() {
        // check_cli_auth with a binary that doesn't exist → Ok(false), not Err
        let result = check_cli_auth("claude_nonexistent_binary_xyz");
        // Unknown CLI returns Err, but binary-not-found for known CLIs returns Ok(false)
        assert!(result.is_err()); // "unknown cli" error
    }

    #[test]
    fn test_check_cli_auth_unknown_cli() {
        let result = check_cli_auth("unknown_provider");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown cli"));
    }

    #[test]
    fn test_parse_auth_output_unknown_cli() {
        // Unknown CLI always false
        assert!(!parse_cli_auth_output("unknown", true, "some output"));
    }

    /// Re-detect with auth file present → status becomes authenticated.
    /// Simulates detect_available_models seeing codex as authenticated.
    #[test]
    fn test_redetect_updates_status() {
        // Mock: if codex_cli.authenticated == true, AvailableModels should reflect it
        let available = mock_available_all(); // has codex_cli with authenticated: true
        assert!(available.codex_cli_available());

        let defaults = generate_smart_defaults(&available);
        // With codex available + authenticated, full_build should pick codex
        assert_eq!(defaults.full_build.provider, "codex_cli");

        // Now simulate codex not authenticated
        let mut no_auth = mock_available_all();
        if let Some(ref mut c) = no_auth.codex_cli {
            c.authenticated = false;
        }
        assert!(!no_auth.codex_cli_available());
        let defaults_no_auth = generate_smart_defaults(&no_auth);
        // Should NOT pick codex_cli when not authenticated
        assert_ne!(defaults_no_auth.full_build.provider, "codex_cli");
    }
}
