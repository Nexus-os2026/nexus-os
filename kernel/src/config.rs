use crate::errors::AgentError;
use crate::privacy::{EncryptedField, PrivacyManager, UserKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NexusConfig {
    pub llm: LlmConfig,
    pub search: SearchConfig,
    pub social: SocialConfig,
    pub messaging: MessagingConfig,
    pub voice: VoiceConfig,
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub governance: GovernanceConfig,
    #[serde(default)]
    pub kill_gates: KillGatesConfig,
    #[serde(default)]
    pub hardware: HardwareConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub models: ModelsConfig,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentLlmConfig>,
    #[serde(default)]
    pub agent_llm_assignments: BTreeMap<String, AgentLlmAssignment>,
    #[serde(default)]
    pub security: crate::crypto::EncryptionConfig,
    #[serde(default)]
    pub backup: crate::backup::BackupScheduleConfig,
    #[serde(default)]
    pub rate_limiting: crate::rate_limit::RateLimitConfig,
    #[serde(default)]
    pub api: crate::rate_limit::ApiHardeningConfig,
    /// Bug AK: SecretsFacade behavior knobs. See ADR 0004.
    #[serde(default)]
    pub credential_facade: CredentialFacadeConfig,
}

/// Bug AK: SecretsFacade configuration. Controls per-scope lookup
/// order. Scopes listed in `env_override_providers` are env-first
/// (`env -> keyring -> sqlite -> memory`); all other scopes are
/// keyring-first (`keyring -> env -> sqlite -> memory`). Default
/// list is `["codex_cli", "ollama"]` because those tools are
/// conventionally env-driven; the four LLM providers
/// (anthropic / openai / openrouter / huggingface) stay
/// keyring-first by default to neutralize the documented stale-env-var
/// risk on paid-API keys (ADR 0004 Consequences).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialFacadeConfig {
    #[serde(default = "default_env_override_providers")]
    pub env_override_providers: Vec<String>,
}

fn default_env_override_providers() -> Vec<String> {
    vec!["codex_cli".to_string(), "ollama".to_string()]
}

impl Default for CredentialFacadeConfig {
    fn default() -> Self {
        Self {
            env_override_providers: default_env_override_providers(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmConfig {
    pub default_model: String,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default)]
    pub nvidia_api_key: String,
    #[serde(default)]
    pub openrouter_api_key: String,
    pub ollama_url: String,
    #[serde(default)]
    pub routing_strategy: String,
    #[serde(default)]
    pub providers: Vec<LlmProviderEntry>,
    /// Persisted CLI provider states (enabled/disabled, last detection results).
    #[serde(default)]
    pub cli_providers: Vec<CliProviderEntry>,
}

/// Persisted state for a CLI-based LLM provider (e.g. Claude Code, Codex CLI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliProviderEntry {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_detected: String,
}

/// A user-configured LLM provider entry for the priority list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmProviderEntry {
    pub id: String,
    pub provider_type: String,
    pub display_name: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: u32,
}

fn default_true() -> bool {
    true
}

/// Per-agent LLM assignment stored in config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLlmAssignment {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub local_only: bool,
    #[serde(default)]
    pub budget_dollars: u32,
    #[serde(default)]
    pub budget_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchConfig {
    pub brave_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialConfig {
    pub x_api_key: String,
    pub x_api_secret: String,
    pub x_access_token: String,
    pub x_access_secret: String,
    pub facebook_page_token: String,
    pub instagram_access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessagingConfig {
    pub telegram_bot_token: String,
    pub whatsapp_business_id: String,
    pub whatsapp_api_token: String,
    pub discord_bot_token: String,
    pub slack_bot_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceConfig {
    pub whisper_model: String,
    pub wake_word: String,
    pub tts_voice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyConfig {
    pub telemetry: bool,
    pub audit_retention_days: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceConfig {
    #[serde(default)]
    pub enable_warden_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KillGatesConfig {
    pub screen_poster_freeze_bps: u32,
    pub screen_poster_halt_bps: u32,
    pub mutation_freeze_signal: u32,
    pub mutation_halt_signal: u32,
    pub cluster_freeze_signal: u32,
    pub cluster_halt_signal: u32,
    pub bft_freeze_signal: u32,
    pub bft_halt_signal: u32,
}

impl Default for KillGatesConfig {
    fn default() -> Self {
        Self {
            screen_poster_freeze_bps: 200,
            screen_poster_halt_bps: 500,
            mutation_freeze_signal: 1,
            mutation_halt_signal: u32::MAX,
            cluster_freeze_signal: 1,
            cluster_halt_signal: u32::MAX,
            bft_freeze_signal: u32::MAX,
            bft_halt_signal: 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareConfig {
    #[serde(default)]
    pub gpu: String,
    #[serde(default)]
    pub vram_mb: u64,
    #[serde(default)]
    pub ram_mb: u64,
    #[serde(default)]
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
    #[serde(default)]
    pub status: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_url(),
            status: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelsConfig {
    #[serde(default)]
    pub primary: String,
    #[serde(default)]
    pub fast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentLlmConfig {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct EncryptedConfigEnvelope {
    version: u8,
    key_id: String,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

impl Default for NexusConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                default_model: "claude-sonnet-4-5".to_string(),
                anthropic_api_key: String::new(),
                openai_api_key: String::new(),
                deepseek_api_key: String::new(),
                gemini_api_key: String::new(),
                nvidia_api_key: String::new(),
                openrouter_api_key: String::new(),
                ollama_url: "http://localhost:11434".to_string(),
                routing_strategy: String::new(),
                providers: Vec::new(),
                cli_providers: Vec::new(),
            },
            search: SearchConfig {
                brave_api_key: String::new(),
            },
            social: SocialConfig {
                x_api_key: String::new(),
                x_api_secret: String::new(),
                x_access_token: String::new(),
                x_access_secret: String::new(),
                facebook_page_token: String::new(),
                instagram_access_token: String::new(),
            },
            messaging: MessagingConfig {
                telegram_bot_token: String::new(),
                whatsapp_business_id: String::new(),
                whatsapp_api_token: String::new(),
                discord_bot_token: String::new(),
                slack_bot_token: String::new(),
            },
            voice: VoiceConfig {
                whisper_model: "auto".to_string(),
                wake_word: "hey nexus".to_string(),
                tts_voice: "default".to_string(),
            },
            privacy: PrivacyConfig {
                telemetry: false,
                audit_retention_days: 365,
            },
            governance: GovernanceConfig::default(),
            kill_gates: KillGatesConfig::default(),
            hardware: HardwareConfig::default(),
            ollama: OllamaConfig::default(),
            models: ModelsConfig::default(),
            agents: BTreeMap::new(),
            agent_llm_assignments: BTreeMap::new(),
            security: crate::crypto::EncryptionConfig::default(),
            backup: crate::backup::BackupScheduleConfig::default(),
            rate_limiting: crate::rate_limit::RateLimitConfig::default(),
            api: crate::rate_limit::ApiHardeningConfig::default(),
            credential_facade: CredentialFacadeConfig::default(),
        }
    }
}

/// The Nexus config file. `NEXUS_CONFIG_PATH` remains the recorded operator
/// state-location override (see the C5 authority inventory); otherwise the
/// file lives under the validated identity home, with no fallback to the
/// working directory (P0-002C5B).
pub fn config_path() -> Result<PathBuf, AgentError> {
    if let Some(path) = env::var_os("NEXUS_CONFIG_PATH") {
        return crate::identity_home::operator_override(path).map_err(|_| {
            AgentError::SupervisorError("NEXUS_CONFIG_PATH must be an absolute path".into())
        });
    }
    crate::identity_home::nexus_state_path("config.toml")
        .map_err(|error| AgentError::SupervisorError(error.to_string()))
}

pub fn load_config() -> Result<NexusConfig, AgentError> {
    load_config_from_path(config_path()?.as_path())
}

/// Writes the configuration file under the launch environment's key material
/// (see [`save_config_checked_to_path`]).
pub fn save_config(config: &NexusConfig) -> Result<(), AgentError> {
    save_config_to_path(config_path()?.as_path(), config)
}

/// [`save_config`] with its typed refusal and outcome.
pub fn save_config_checked(config: &NexusConfig) -> Result<SaveOutcome, ConfigSaveError> {
    let path = config_path().map_err(ConfigSaveError::Failed)?;
    save_config_checked_to_path(&path, config, &ConfigKeyMaterial::from_launch_environment())
}

pub fn load_config_from_path(path: &Path) -> Result<NexusConfig, AgentError> {
    load_config_from_path_with(path, &ConfigKeyMaterial::from_launch_environment())
}

/// Reads a configuration file with the given key material (Final Gate item
/// A).
///
/// A missing file is the first run: the default configuration, which holds no
/// credential, is written and returned. An existing file is only read, never
/// rewritten: a legacy plaintext file stays as it is until an explicit save,
/// and an empty or whitespace-only file is refused rather than replaced.
pub fn load_config_from_path_with(
    path: &Path,
    keys: &ConfigKeyMaterial,
) -> Result<NexusConfig, AgentError> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => Zeroizing::new(raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let default_config = NexusConfig::default();
            save_config_checked_to_path(path, &default_config, keys)?;
            return Ok(default_config);
        }
        Err(error) => return Err(to_io_error(error)),
    };
    if raw.trim().is_empty() {
        return Err(AgentError::SupervisorError(EMPTY_CONFIGURATION.into()));
    }
    open_config_text(&raw, keys).map(|opened| opened.config)
}

/// Why an interface update has no current security baseline. It carries no
/// path, configuration text, key source, key file, or parse or decryption
/// detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("current security settings are unavailable")]
pub struct SecurityBaselineUnavailable;

/// The encryption-at-rest section of the current configuration: the baseline
/// an interface update is compared against (P0-002C5B).
///
/// Backend bootstrap ([`load_config`]) creates the first-run default when the
/// file is missing; it never rewrites an existing file. An interface update is
/// not a bootstrap authority, so this never creates, rewrites or migrates
/// anything. A missing, empty or whitespace-only, unreadable, undecryptable or
/// unparsable configuration yields no baseline. A valid historical plaintext
/// configuration is read, not migrated.
pub fn load_current_security_baseline(
) -> Result<crate::crypto::EncryptionConfig, SecurityBaselineUnavailable> {
    let path = config_path().map_err(|_| SecurityBaselineUnavailable)?;
    load_security_baseline_from_path(&path)
}

/// [`load_current_security_baseline`] for an explicit configuration file.
pub fn load_security_baseline_from_path(
    path: &Path,
) -> Result<crate::crypto::EncryptionConfig, SecurityBaselineUnavailable> {
    let raw = fs::read_to_string(path).map_err(|_| SecurityBaselineUnavailable)?;
    if raw.trim().is_empty() {
        return Err(SecurityBaselineUnavailable);
    }
    security_baseline_from_text(&raw, &ConfigKeyMaterial::from_launch_environment())
}

/// [`load_security_baseline_from_path`] with the given key material.
pub fn load_security_baseline_from_path_with(
    path: &Path,
    keys: &ConfigKeyMaterial,
) -> Result<crate::crypto::EncryptionConfig, SecurityBaselineUnavailable> {
    let raw = fs::read_to_string(path).map_err(|_| SecurityBaselineUnavailable)?;
    if raw.trim().is_empty() {
        return Err(SecurityBaselineUnavailable);
    }
    security_baseline_from_text(&raw, keys)
}

fn security_baseline_from_text(
    raw: &str,
    keys: &ConfigKeyMaterial,
) -> Result<crate::crypto::EncryptionConfig, SecurityBaselineUnavailable> {
    open_config_text(raw, keys)
        .map(|opened| opened.config.security)
        .map_err(|_| SecurityBaselineUnavailable)
}

pub fn save_config_to_path(path: &Path, config: &NexusConfig) -> Result<(), AgentError> {
    save_config_checked_to_path(path, config, &ConfigKeyMaterial::from_launch_environment())
        .map(|_| ())
        .map_err(AgentError::from)
}

/// Writes `config` to `path` (Final Gate item A).
///
/// - The configuration already at `path` must open with `keys`. When it does
///   not (unreadable, empty, malformed, undecryptable or in an unsupported
///   envelope), nothing is written: legacy ciphertext is never replaced by a
///   configuration built without it.
/// - A new or changed credential is written only under the operator key
///   (`NEXUS_CONFIG_KEY`). Without one the save is refused and nothing is
///   written. Clearing a credential is not a new or changed credential.
/// - Otherwise the file keeps the key that opened it: a legacy file stays
///   under its legacy key. When a credential save moves a file to the operator
///   key, or an explicit save first encrypts a legacy plaintext file, the
///   outcome says so; loading never does either.
pub fn save_config_checked_to_path(
    path: &Path,
    config: &NexusConfig,
    keys: &ConfigKeyMaterial,
) -> Result<SaveOutcome, ConfigSaveError> {
    let stored = read_stored_for_write(path, keys)?;
    let operator = keys.operator_key();
    let (key, outcome) = if introduces_credentials(stored.as_ref().map(|s| &s.config), config) {
        let key = operator.ok_or(ConfigSaveError::Refused(
            ConfigWriteRefusal::OperatorKeyRequired,
        ))?;
        let rekeyed = stored.as_ref().is_some_and(|stored| {
            stored.key.as_ref().map(|opened| opened.bytes) != Some(key.bytes)
        });
        let outcome = if rekeyed {
            SaveOutcome::RekeyedToOperatorKey
        } else {
            SaveOutcome::Written
        };
        (key, outcome)
    } else {
        match stored {
            Some(OpenedConfig {
                key: Some(opened), ..
            }) => (opened, SaveOutcome::Written),
            Some(OpenedConfig { key: None, .. }) => match operator {
                Some(key) => (key, SaveOutcome::RekeyedToOperatorKey),
                None => (keys.ambient_key(), SaveOutcome::EncryptedLegacyPlaintext),
            },
            None => match operator {
                Some(key) => (key, SaveOutcome::Written),
                None => (keys.ambient_key(), SaveOutcome::Written),
            },
        }
    };
    write_envelope(path, config, &key).map_err(ConfigSaveError::Failed)?;
    Ok(outcome)
}

/// How a configuration save protected the file (Final Gate item A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Written under the key that opened the existing file, or a first write.
    Written,
    /// The file was under a legacy key (or was legacy plaintext) and is now
    /// under the operator key: a credential save needed it, or an explicit
    /// save first encrypted a plaintext file while the operator key was set.
    RekeyedToOperatorKey,
    /// An explicit save first encrypted a legacy plaintext file. No operator
    /// key was set and no credential was added or changed, so the legacy
    /// ambient key was used.
    EncryptedLegacyPlaintext,
}

/// Why a configuration save wrote nothing (Final Gate item A). The reasons are
/// bounded: no path, key material or configuration text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigWriteRefusal {
    /// A new or changed credential, and no operator configuration key.
    OperatorKeyRequired,
    /// The configuration on disk cannot be read with the available key
    /// material, so it is not overwritten.
    ExistingUnreadable,
}

impl ConfigWriteRefusal {
    /// Audit reason class.
    pub const fn reason_class(self) -> &'static str {
        match self {
            Self::OperatorKeyRequired => "configuration_key_required",
            Self::ExistingUnreadable => "existing_configuration_unreadable",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::OperatorKeyRequired => {
                "configuration not saved: a new or changed credential needs the operator configuration key (NEXUS_CONFIG_KEY)"
            }
            Self::ExistingUnreadable => {
                "configuration not saved: the configuration on disk cannot be read, so it is not overwritten"
            }
        }
    }
}

/// A configuration save that wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigSaveError {
    #[error("{}", .0.message())]
    Refused(ConfigWriteRefusal),
    #[error(transparent)]
    Failed(AgentError),
}

impl From<AgentError> for ConfigSaveError {
    fn from(error: AgentError) -> Self {
        Self::Failed(error)
    }
}

impl From<ConfigSaveError> for AgentError {
    fn from(error: ConfigSaveError) -> Self {
        match error {
            ConfigSaveError::Refused(refusal) => {
                AgentError::SupervisorError(refusal.message().to_string())
            }
            ConfigSaveError::Failed(error) => error,
        }
    }
}

/// Key material for the configuration file (Final Gate item A).
///
/// - The operator key is `NEXUS_CONFIG_KEY`, a secret the operator sets in the
///   launch environment. It keys a new or changed credential only when it is
///   valid UTF-8 and neither empty nor whitespace-only. Nothing else about it
///   is checked: it is hashed once with SHA-256 under a fixed label, with no
///   salt and no stretching, so a short or guessable value gives a
///   correspondingly weak key.
/// - The two legacy derivations exist to read files written before the Final
///   Gate and to rewrite a file that gains no new or changed credential: the
///   legacy explicit key (`NEXUS_CONFIG_KEY` as any valid UTF-8 value, even an
///   empty one) and the ambient key (HOME, USER, USERNAME and HOSTNAME, each
///   when set). The ambient key is not a secret: anyone who knows the account
///   and host names can derive it.
#[derive(Clone)]
pub struct ConfigKeyMaterial {
    legacy_explicit: Option<Zeroizing<String>>,
    ambient: [Option<String>; 4],
}

impl ConfigKeyMaterial {
    /// The key material of the launch environment. The desktop and the CLI use
    /// only this.
    pub fn from_launch_environment() -> Self {
        let lossy = |value: Option<std::ffi::OsString>| {
            value.map(|value| value.to_string_lossy().into_owned())
        };
        Self {
            // Exactly as the legacy reader: any valid UTF-8 value, even an
            // empty one. A value that is not UTF-8 is no key material.
            legacy_explicit: env::var_os("NEXUS_CONFIG_KEY")
                .and_then(|value| value.into_string().ok())
                .map(Zeroizing::new),
            ambient: [
                lossy(env::var_os("HOME")),
                lossy(env::var_os("USER")),
                lossy(env::var_os("USERNAME")),
                lossy(env::var_os("HOSTNAME")),
            ],
        }
    }

    /// Key material the caller already holds, for tests and tools. Nothing is
    /// read from the environment. `ambient` is HOME, USER, USERNAME and
    /// HOSTNAME, in that order.
    pub fn from_values(operator_key: Option<&str>, ambient: [Option<&str>; 4]) -> Self {
        Self {
            legacy_explicit: operator_key.map(|value| Zeroizing::new(value.to_owned())),
            ambient: ambient.map(|part| part.map(str::to_owned)),
        }
    }

    /// Whether an operator key is available for new or changed credentials.
    pub fn has_operator_key(&self) -> bool {
        self.operator_key().is_some()
    }

    fn operator_key(&self) -> Option<UserKey> {
        self.legacy_explicit
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| derive_config_key(&[value.as_bytes()]))
    }

    /// The explicit legacy read path: a legacy (v1) file was written under
    /// exactly one of these two derivations, and the AES-GCM tag decides
    /// which. No other key is ever tried.
    fn legacy_read_keys(&self) -> Vec<UserKey> {
        let mut keys = Vec::with_capacity(2);
        if let Some(value) = &self.legacy_explicit {
            keys.push(derive_config_key(&[value.as_bytes()]));
        }
        keys.push(self.ambient_key());
        keys
    }

    fn ambient_key(&self) -> UserKey {
        let parts: Vec<&[u8]> = self
            .ambient
            .iter()
            .flatten()
            .map(|part| part.as_bytes())
            .collect();
        derive_config_key(&parts)
    }
}

/// Label that starts every configuration key derivation, legacy or operator.
const CONFIG_KEY_LABEL: &[u8] = b"nexus-config-key-v1";
/// The envelope's key identifier. Legacy and operator-keyed files share the
/// version 1 envelope, so an earlier build can still read a file written here
/// when the same `NEXUS_CONFIG_KEY` is set.
const CONFIG_KEY_ID: &str = "nexus-config-v1";
const ENVELOPE_VERSION: u8 = 1;

const EMPTY_CONFIGURATION: &str =
    "the configuration file is empty; it is not replaced (remove it to start from the default configuration)";
const INVALID_CONFIGURATION: &str = "the configuration file is not a valid Nexus configuration";
const UNSUPPORTED_ENVELOPE: &str = "the configuration file uses an unsupported encryption envelope";
const UNDECRYPTABLE_CONFIGURATION: &str =
    "the configuration file cannot be decrypted with the available key material";

fn derive_config_key(parts: &[&[u8]]) -> UserKey {
    let mut hasher = Sha256::new();
    hasher.update(CONFIG_KEY_LABEL);
    for part in parts {
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    UserKey {
        id: CONFIG_KEY_ID.to_string(),
        bytes,
    }
}

/// The configuration's credential fields other than per-provider API keys, in
/// a fixed order (Final Gate item A). `whatsapp_business_id` is an account
/// identifier, not a credential.
pub fn credential_fields(config: &NexusConfig) -> [&String; 17] {
    [
        &config.llm.anthropic_api_key,
        &config.llm.openai_api_key,
        &config.llm.deepseek_api_key,
        &config.llm.gemini_api_key,
        &config.llm.nvidia_api_key,
        &config.llm.openrouter_api_key,
        &config.search.brave_api_key,
        &config.social.x_api_key,
        &config.social.x_api_secret,
        &config.social.x_access_token,
        &config.social.x_access_secret,
        &config.social.facebook_page_token,
        &config.social.instagram_access_token,
        &config.messaging.telegram_bot_token,
        &config.messaging.whatsapp_api_token,
        &config.messaging.discord_bot_token,
        &config.messaging.slack_bot_token,
    ]
}

/// [`credential_fields`], mutable, in the same order.
pub fn credential_fields_mut(config: &mut NexusConfig) -> [&mut String; 17] {
    [
        &mut config.llm.anthropic_api_key,
        &mut config.llm.openai_api_key,
        &mut config.llm.deepseek_api_key,
        &mut config.llm.gemini_api_key,
        &mut config.llm.nvidia_api_key,
        &mut config.llm.openrouter_api_key,
        &mut config.search.brave_api_key,
        &mut config.social.x_api_key,
        &mut config.social.x_api_secret,
        &mut config.social.x_access_token,
        &mut config.social.x_access_secret,
        &mut config.social.facebook_page_token,
        &mut config.social.instagram_access_token,
        &mut config.messaging.telegram_bot_token,
        &mut config.messaging.whatsapp_api_token,
        &mut config.messaging.discord_bot_token,
        &mut config.messaging.slack_bot_token,
    ]
}

/// Whether `next` holds a credential that `stored` does not hold in the same
/// place: a non-empty field that differs, or a provider API key not stored for
/// that provider id. An emptied field adds nothing.
fn introduces_credentials(stored: Option<&NexusConfig>, next: &NexusConfig) -> bool {
    let stored_fields = stored.map(credential_fields);
    let field_added = credential_fields(next)
        .iter()
        .enumerate()
        .any(|(index, value)| {
            !value.is_empty()
                && stored_fields
                    .as_ref()
                    .is_none_or(|fields| fields[index] != *value)
        });
    let provider_key_added = next.llm.providers.iter().any(|provider| {
        !provider.api_key.is_empty()
            && !stored.is_some_and(|stored| {
                stored
                    .llm
                    .providers
                    .iter()
                    .any(|kept| kept.id == provider.id && kept.api_key == provider.api_key)
            })
    });
    field_added || provider_key_added
}

/// A configuration that opened, and the key that opened it (`None`: legacy
/// plaintext).
struct OpenedConfig {
    config: NexusConfig,
    key: Option<UserKey>,
}

/// The configuration already at `path`, for a write. `None` only when there is
/// no file; anything that exists but does not open refuses the write.
fn read_stored_for_write(
    path: &Path,
    keys: &ConfigKeyMaterial,
) -> Result<Option<OpenedConfig>, ConfigSaveError> {
    let refused = || ConfigSaveError::Refused(ConfigWriteRefusal::ExistingUnreadable);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => Zeroizing::new(raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(refused()),
    };
    if raw.trim().is_empty() {
        return Err(refused());
    }
    open_config_text(&raw, keys)
        .map(Some)
        .map_err(|_| refused())
}

/// Opens configuration text: a version 1 envelope through the explicit legacy
/// read path, or legacy plaintext as it is. Errors carry no configuration
/// text.
fn open_config_text(raw: &str, keys: &ConfigKeyMaterial) -> Result<OpenedConfig, AgentError> {
    match toml::from_str::<EncryptedConfigEnvelope>(raw) {
        Ok(envelope) => open_envelope(&envelope, keys),
        Err(_) => toml::from_str::<NexusConfig>(raw)
            .map(|config| OpenedConfig { config, key: None })
            .map_err(|_| AgentError::SupervisorError(INVALID_CONFIGURATION.into())),
    }
}

fn open_envelope(
    envelope: &EncryptedConfigEnvelope,
    keys: &ConfigKeyMaterial,
) -> Result<OpenedConfig, AgentError> {
    if envelope.version != ENVELOPE_VERSION || envelope.key_id != CONFIG_KEY_ID {
        return Err(AgentError::SupervisorError(UNSUPPORTED_ENVELOPE.into()));
    }
    let field = EncryptedField {
        key_id: envelope.key_id.clone(),
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext.clone(),
    };
    let privacy = PrivacyManager::new();
    for key in keys.legacy_read_keys() {
        if let Ok(plaintext) = privacy.decrypt_field(&field, &key) {
            let config = decode_payload(Zeroizing::new(plaintext))?;
            return Ok(OpenedConfig {
                config,
                key: Some(key),
            });
        }
    }
    Err(AgentError::SupervisorError(
        UNDECRYPTABLE_CONFIGURATION.into(),
    ))
}

fn decode_payload(plaintext: Zeroizing<Vec<u8>>) -> Result<NexusConfig, AgentError> {
    let text = std::str::from_utf8(&plaintext)
        .map_err(|_| AgentError::SupervisorError(INVALID_CONFIGURATION.into()))?;
    toml::from_str::<NexusConfig>(text)
        .map_err(|_| AgentError::SupervisorError(INVALID_CONFIGURATION.into()))
}

fn encrypt_config(plaintext: &str, key: &UserKey) -> Result<EncryptedConfigEnvelope, AgentError> {
    let mut privacy = PrivacyManager::new();
    let encrypted = privacy.encrypt_field(plaintext.as_bytes(), key)?;
    Ok(EncryptedConfigEnvelope {
        version: ENVELOPE_VERSION,
        key_id: encrypted.key_id,
        nonce: encrypted.nonce,
        ciphertext: encrypted.ciphertext,
    })
}

/// Encrypts `config` under `key` and replaces `path` atomically. The
/// temporary file has a unique name and is created owner-only (0600 on Unix)
/// before any byte is written; Windows keeps the directory's inherited access.
fn write_envelope(path: &Path, config: &NexusConfig, key: &UserKey) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(to_io_error)?;
    }
    let plaintext = Zeroizing::new(toml::to_string(config).map_err(|error| {
        AgentError::SupervisorError(format!("unable to serialize config: {error}"))
    })?);
    let envelope = encrypt_config(&plaintext, key)?;
    let encoded = toml::to_string(&envelope).map_err(|error| {
        AgentError::SupervisorError(format!("unable to encode encrypted config: {error}"))
    })?;
    let tmp = path.with_extension(format!(
        "toml.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    write_new_private_file(&tmp, encoded.as_bytes())?;
    fs::rename(&tmp, path).map_err(|error| {
        // Best-effort: the unique temporary file is ours; the rename failure
        // is the error reported.
        let _ = fs::remove_file(&tmp);
        to_io_error(error)
    })
}

/// Creates `path`, which must not exist, owner-only on Unix, writes `bytes`
/// and syncs them. A partly written file is removed.
fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<(), AgentError> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(to_io_error)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        // Best-effort: the file was created by this call and is incomplete.
        let _ = fs::remove_file(path);
        return Err(to_io_error(error));
    }
    Ok(())
}

fn to_io_error(error: std::io::Error) -> AgentError {
    AgentError::SupervisorError(format!("config I/O error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        credential_fields, credential_fields_mut, derive_config_key, load_config_from_path,
        load_config_from_path_with, load_security_baseline_from_path,
        load_security_baseline_from_path_with, save_config_checked_to_path, save_config_to_path,
        ConfigKeyMaterial, ConfigSaveError, ConfigWriteRefusal, EncryptedConfigEnvelope,
        LlmProviderEntry, NexusConfig, SaveOutcome, SecurityBaselineUnavailable,
    };
    use crate::privacy::{EncryptedField, PrivacyManager, UserKey};
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    fn temp_config_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("nexus-config-test-{}", Uuid::new_v4()));
        base.join(".nexus").join("config.toml")
    }

    fn cleanup(path: &Path) {
        // Best-effort: remove the test's own temporary tree.
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    // Synthetic launch environments. No test here reads the process
    // environment or a real configuration.
    const HOME: &str = "/home/synthetic-nexus";
    const OPERATOR_KEY: &str = "synthetic-operator-key";

    fn ambient_only() -> ConfigKeyMaterial {
        ConfigKeyMaterial::from_values(
            None,
            [
                Some(HOME),
                Some("synthetic-user"),
                None,
                Some("synthetic-host"),
            ],
        )
    }

    fn with_operator_key() -> ConfigKeyMaterial {
        ConfigKeyMaterial::from_values(
            Some(OPERATOR_KEY),
            [
                Some(HOME),
                Some("synthetic-user"),
                None,
                Some("synthetic-host"),
            ],
        )
    }

    // Legacy key vectors computed independently (`printf ... | sha256sum`):
    // SHA-256("nexus-config-key-v1" ‖ inputs).
    /// HOME, USER and HOSTNAME above, USERNAME unset.
    const AMBIENT_VECTOR: &str = "10265430fed11d4a97abe76795007874532cb2ee7e77f01b04a9b4e4bd54f642";
    /// NEXUS_CONFIG_KEY = OPERATOR_KEY.
    const OPERATOR_VECTOR: &str =
        "2e58cf335a20c27407e059d61b4983eba70684ce8de0bd5f4de87be583fccf74";
    /// The label alone: NEXUS_CONFIG_KEY = "", or no ambient input at all.
    const LABEL_ONLY_VECTOR: &str =
        "e1a01b22bacda129785b641255014b348b3deb0407aea2db7f1d5b4f90fd1fd3";
    /// NEXUS_CONFIG_KEY = three spaces.
    const BLANK_VECTOR: &str = "731fc26e1b16176861fd7bd2d8058e9857e8ae8d71da5f2c0822ecbe88ebcba0";

    fn vector_key(digest: &str) -> UserKey {
        UserKey {
            id: "nexus-config-v1".into(),
            bytes: hex::decode(digest).unwrap().try_into().unwrap(),
        }
    }

    /// A configuration file exactly as the pre-Final-Gate writer produced it
    /// under `key`, built without the code under test.
    fn write_legacy_file(path: &Path, config: &NexusConfig, key: &UserKey) -> String {
        let plaintext = toml::to_string(config).unwrap();
        let mut privacy = PrivacyManager::new();
        let encrypted = privacy.encrypt_field(plaintext.as_bytes(), key).unwrap();
        let envelope = EncryptedConfigEnvelope {
            version: 1,
            key_id: encrypted.key_id,
            nonce: encrypted.nonce,
            ciphertext: encrypted.ciphertext,
        };
        let text = toml::to_string(&envelope).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &text).unwrap();
        text
    }

    fn opens_with(text: &str, key: &UserKey) -> bool {
        let envelope: EncryptedConfigEnvelope = toml::from_str(text).unwrap();
        let field = EncryptedField {
            key_id: envelope.key_id,
            nonce: envelope.nonce,
            ciphertext: envelope.ciphertext,
        };
        PrivacyManager::new().decrypt_field(&field, key).is_ok()
    }

    fn provider(id: &str, api_key: &str) -> LlmProviderEntry {
        LlmProviderEntry {
            id: id.into(),
            provider_type: "openai".into(),
            display_name: id.into(),
            api_key: api_key.into(),
            base_url: String::new(),
            enabled: true,
            priority: 0,
        }
    }

    fn with_credentials() -> NexusConfig {
        let mut config = NexusConfig::default();
        config.search.brave_api_key = "synthetic-brave".into();
        config.messaging.telegram_bot_token = "123:synthetic".into();
        config.llm.providers = vec![provider("p", "synthetic-provider-key")];
        config
    }

    #[test]
    fn test_config_create_and_load() {
        let path = temp_config_path();
        let keys = with_operator_key();
        let mut config = NexusConfig::default();
        config.llm.anthropic_api_key = "sk-ant-test".to_string();
        config.search.brave_api_key = "brave-key".to_string();
        config.messaging.telegram_bot_token = "123:abc".to_string();
        config.voice.wake_word = "hey nexus".to_string();

        let save = save_config_checked_to_path(path.as_path(), &config, &keys);
        assert!(save.is_ok());

        let loaded = load_config_from_path_with(path.as_path(), &keys);
        assert!(loaded.is_ok());
        let loaded_config = loaded.expect("load should succeed");
        assert_eq!(loaded_config.llm, config.llm);
        assert_eq!(loaded_config.search, config.search);
        assert_eq!(loaded_config.messaging, config.messaging);
        assert_eq!(loaded_config.voice, config.voice);
        cleanup(&path);
    }

    #[test]
    fn test_config_encrypted_at_rest() {
        let path = temp_config_path();
        let keys = with_operator_key();
        let mut config = NexusConfig::default();
        config.llm.anthropic_api_key = "sk-ant-plaintext-check".to_string();

        let save = save_config_checked_to_path(path.as_path(), &config, &keys);
        assert!(save.is_ok());

        let raw = fs::read_to_string(path.as_path()).unwrap_or_default();
        assert!(!raw.contains("sk-ant-plaintext-check"));
        assert!(!raw.contains("[llm]"));

        let loaded = load_config_from_path_with(path.as_path(), &keys);
        assert!(loaded.is_ok());
        cleanup(&path);
    }

    #[test]
    fn p0_002c5b_security_baselines_come_only_from_an_existing_loadable_config() {
        let path = temp_config_path();
        let none = Err(SecurityBaselineUnavailable);
        // Missing: no baseline, and nothing is created.
        assert_eq!(load_security_baseline_from_path(&path), none);
        assert!(!path.exists());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Zero-byte, whitespace-only and malformed: none, nothing rewritten.
        for content in ["", " \n\t\r\n", "[llm\nbroken = "] {
            fs::write(&path, content).unwrap();
            assert_eq!(load_security_baseline_from_path(&path), none, "{content:?}");
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
        // An encrypted configuration that no longer decrypts: none. (Final
        // Gate item A: a write never replaces a file it cannot read, so the
        // malformed file is removed first.)
        fs::remove_file(&path).unwrap();
        save_config_to_path(&path, &NexusConfig::default()).unwrap();
        let mut envelope: EncryptedConfigEnvelope =
            toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        envelope.ciphertext[0] ^= 1;
        let tampered = toml::to_string(&envelope).unwrap();
        fs::write(&path, &tampered).unwrap();
        assert_eq!(load_security_baseline_from_path(&path), none);
        assert_eq!(fs::read_to_string(&path).unwrap(), tampered);
        // An I/O failure (a directory is no configuration file): none.
        assert_eq!(
            load_security_baseline_from_path(path.parent().unwrap()),
            none
        );
        // A valid encrypted configuration yields its security section.
        fs::remove_file(&path).unwrap();
        let mut config = NexusConfig::default();
        config.security.key_env = "NEXUS_OPERATOR_KEY".into();
        save_config_to_path(&path, &config).unwrap();
        assert_eq!(
            load_security_baseline_from_path(&path),
            Ok(config.security.clone())
        );
        // A valid historical plaintext configuration is read, not migrated.
        let plaintext = toml::to_string(&config).unwrap();
        fs::write(&path, &plaintext).unwrap();
        assert_eq!(
            load_security_baseline_from_path(&path),
            Ok(config.security.clone())
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), plaintext);
        // Bootstrap stays separate: the ordinary loader still creates the
        // first-run configuration for backend startup.
        fs::remove_file(&path).unwrap();
        assert!(load_config_from_path(&path).is_ok());
        assert!(path.exists());
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_legacy_key_derivations_are_unchanged() {
        let digest = |key: UserKey| hex::encode(key.bytes);
        assert_eq!(digest(ambient_only().ambient_key()), AMBIENT_VECTOR);
        assert_eq!(
            digest(with_operator_key().operator_key().unwrap()),
            OPERATOR_VECTOR
        );
        let empty = ConfigKeyMaterial::from_values(Some(""), [None; 4]);
        assert_eq!(
            digest(empty.legacy_read_keys()[0].clone()),
            LABEL_ONLY_VECTOR
        );
        assert_eq!(digest(empty.ambient_key()), LABEL_ONLY_VECTOR);
        let blank = ConfigKeyMaterial::from_values(Some("   "), [None; 4]);
        assert_eq!(digest(blank.legacy_read_keys()[0].clone()), BLANK_VECTOR);
        assert_eq!(derive_config_key(&[]).id, "nexus-config-v1");
    }

    #[test]
    fn p0_fg_a_legacy_ciphertext_still_reads_and_is_never_rewritten() {
        let config = with_credentials();
        let operator_only = ConfigKeyMaterial::from_values(Some(OPERATOR_KEY), [None; 4]);
        let other_host = ConfigKeyMaterial::from_values(
            None,
            [
                Some(HOME),
                Some("synthetic-user"),
                None,
                Some("another-host"),
            ],
        );
        let cases = [
            (
                AMBIENT_VECTOR,
                vec![ambient_only(), with_operator_key()],
                vec![operator_only.clone(), other_host.clone()],
            ),
            (
                OPERATOR_VECTOR,
                vec![with_operator_key(), operator_only.clone()],
                vec![ambient_only(), other_host.clone()],
            ),
            (
                LABEL_ONLY_VECTOR,
                vec![
                    ConfigKeyMaterial::from_values(Some(""), [None; 4]),
                    ConfigKeyMaterial::from_values(None, [None; 4]),
                    // With no ambient input the ambient key is the label
                    // alone as well.
                    operator_only.clone(),
                ],
                vec![ambient_only(), with_operator_key()],
            ),
        ];
        for (vector, opens, fails) in cases {
            let path = temp_config_path();
            let text = write_legacy_file(&path, &config, &vector_key(vector));
            for keys in &opens {
                assert_eq!(
                    load_config_from_path_with(&path, keys).unwrap(),
                    config,
                    "{vector}"
                );
                assert_eq!(
                    load_security_baseline_from_path_with(&path, keys),
                    Ok(config.security.clone())
                );
            }
            for keys in &fails {
                assert!(load_config_from_path_with(&path, keys).is_err(), "{vector}");
            }
            // Reading never rewrites the file.
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
            cleanup(&path);
        }
        // Legacy plaintext is read as it is; a load never migrates it.
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let plaintext = toml::to_string(&config).unwrap();
        fs::write(&path, &plaintext).unwrap();
        for keys in [ambient_only(), with_operator_key()] {
            assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), config);
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), plaintext);
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_new_or_changed_credentials_need_the_operator_key() {
        let refused = Err(ConfigSaveError::Refused(
            ConfigWriteRefusal::OperatorKeyRequired,
        ));
        let keys = ambient_only();
        // A first write that would store a credential.
        let path = temp_config_path();
        assert_eq!(
            save_config_checked_to_path(&path, &with_credentials(), &keys),
            refused
        );
        assert!(!path.exists());
        // A legacy ambient file: every new or changed credential is refused
        // and the file is left exactly as it was.
        let stored = with_credentials();
        let text = write_legacy_file(&path, &stored, &vector_key(AMBIENT_VECTOR));
        let mut changed = stored.clone();
        changed.messaging.telegram_bot_token = "456:synthetic".into();
        let mut added = stored.clone();
        added.social.x_api_key = "synthetic-x".into();
        let mut new_provider = stored.clone();
        new_provider
            .llm
            .providers
            .push(provider("q", "synthetic-q"));
        let mut moved = stored.clone();
        moved.llm.providers[0].id = "other".into();
        let mut blank = stored.clone();
        blank.search.brave_api_key = " ".into();
        for next in [changed, added, new_provider, moved, blank] {
            assert_eq!(save_config_checked_to_path(&path, &next, &keys), refused);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        // A save that keeps every credential, and one that clears
        // credentials, are written under the legacy key that opened the file.
        let mut settings = stored.clone();
        settings.llm.default_model = "synthetic-model".into();
        assert_eq!(
            save_config_checked_to_path(&path, &settings, &keys),
            Ok(SaveOutcome::Written)
        );
        assert!(opens_with(
            &fs::read_to_string(&path).unwrap(),
            &vector_key(AMBIENT_VECTOR)
        ));
        assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), settings);
        let mut cleared = settings.clone();
        cleared.search.brave_api_key.clear();
        cleared.llm.providers.clear();
        assert_eq!(
            save_config_checked_to_path(&path, &cleared, &keys),
            Ok(SaveOutcome::Written)
        );
        assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), cleared);
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_operator_key_material_must_be_present_and_not_blank() {
        for operator in [None, Some(""), Some(" "), Some(" \t\r\n")] {
            let keys = ConfigKeyMaterial::from_values(operator, [Some(HOME), None, None, None]);
            assert!(!keys.has_operator_key(), "{operator:?}");
            let path = temp_config_path();
            assert_eq!(
                save_config_checked_to_path(&path, &with_credentials(), &keys),
                Err(ConfigSaveError::Refused(
                    ConfigWriteRefusal::OperatorKeyRequired
                )),
                "{operator:?}"
            );
            assert!(!path.exists());
        }
        assert!(with_operator_key().has_operator_key());
    }

    #[test]
    fn p0_fg_a_operator_keyed_credentials_open_only_with_the_operator_key() {
        let path = temp_config_path();
        let config = with_credentials();
        assert_eq!(
            save_config_checked_to_path(&path, &config, &with_operator_key()),
            Ok(SaveOutcome::Written)
        );
        let text = fs::read_to_string(&path).unwrap();
        for secret in [
            "synthetic-brave",
            "123:synthetic",
            "synthetic-provider-key",
            "[llm]",
        ] {
            assert!(!text.contains(secret), "{secret}");
        }
        assert!(opens_with(&text, &vector_key(OPERATOR_VECTOR)));
        assert!(!opens_with(&text, &vector_key(AMBIENT_VECTOR)));
        assert_eq!(
            load_config_from_path_with(&path, &with_operator_key()).unwrap(),
            config
        );
        // Without the operator key it does not open, and nothing overwrites it.
        let keys = ambient_only();
        assert!(load_config_from_path_with(&path, &keys).is_err());
        assert_eq!(
            load_security_baseline_from_path_with(&path, &keys),
            Err(SecurityBaselineUnavailable)
        );
        let mut settings = config.clone();
        settings.llm.default_model = "synthetic-model".into();
        assert_eq!(
            save_config_checked_to_path(&path, &settings, &keys),
            Err(ConfigSaveError::Refused(
                ConfigWriteRefusal::ExistingUnreadable
            ))
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_only_a_credential_save_moves_an_ambient_file_to_the_operator_key() {
        let path = temp_config_path();
        let stored = with_credentials();
        write_legacy_file(&path, &stored, &vector_key(AMBIENT_VECTOR));
        let keys = with_operator_key();
        // Readable through the ambient legacy derivation while the operator
        // key is set.
        assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), stored);
        // A save without a new or changed credential keeps the file ambient.
        let mut settings = stored.clone();
        settings.llm.default_model = "synthetic-model".into();
        assert_eq!(
            save_config_checked_to_path(&path, &settings, &keys),
            Ok(SaveOutcome::Written)
        );
        assert!(opens_with(
            &fs::read_to_string(&path).unwrap(),
            &vector_key(AMBIENT_VECTOR)
        ));
        // A credential save moves the whole file to the operator key, and the
        // outcome says so.
        let mut credential = settings.clone();
        credential.social.x_api_key = "synthetic-x".into();
        assert_eq!(
            save_config_checked_to_path(&path, &credential, &keys),
            Ok(SaveOutcome::RekeyedToOperatorKey)
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(opens_with(&text, &vector_key(OPERATOR_VECTOR)));
        assert!(load_config_from_path_with(&path, &ambient_only()).is_err());
        assert_eq!(
            load_config_from_path_with(&path, &keys).unwrap(),
            credential
        );
        // Later credential saves under the same key are ordinary writes.
        let mut again = credential.clone();
        again.social.x_api_secret = "synthetic-secret".into();
        assert_eq!(
            save_config_checked_to_path(&path, &again, &keys),
            Ok(SaveOutcome::Written)
        );
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_only_an_explicit_save_encrypts_a_legacy_plaintext_file() {
        let config = with_credentials();
        let cases = [
            (
                ambient_only(),
                SaveOutcome::EncryptedLegacyPlaintext,
                AMBIENT_VECTOR,
            ),
            (
                with_operator_key(),
                SaveOutcome::RekeyedToOperatorKey,
                OPERATOR_VECTOR,
            ),
        ];
        for (keys, outcome, vector) in cases {
            let path = temp_config_path();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let plaintext = toml::to_string(&config).unwrap();
            fs::write(&path, &plaintext).unwrap();
            assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), config);
            assert_eq!(fs::read_to_string(&path).unwrap(), plaintext);
            let mut settings = config.clone();
            settings.llm.default_model = "synthetic-model".into();
            assert_eq!(
                save_config_checked_to_path(&path, &settings, &keys),
                Ok(outcome)
            );
            let text = fs::read_to_string(&path).unwrap();
            assert!(!text.contains("synthetic-brave"));
            assert!(opens_with(&text, &vector_key(vector)));
            assert_eq!(load_config_from_path_with(&path, &keys).unwrap(), settings);
            cleanup(&path);
        }
    }

    #[test]
    fn p0_fg_a_an_unreadable_configuration_is_never_overwritten() {
        let refused = Err(ConfigSaveError::Refused(
            ConfigWriteRefusal::ExistingUnreadable,
        ));
        let path = temp_config_path();
        write_legacy_file(&path, &NexusConfig::default(), &vector_key(AMBIENT_VECTOR));
        let mut envelope: EncryptedConfigEnvelope =
            toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        envelope.ciphertext[0] ^= 1;
        let tampered = toml::to_string(&envelope).unwrap();
        envelope.ciphertext[0] ^= 1;
        envelope.version = 2;
        let unsupported = toml::to_string(&envelope).unwrap();
        for content in [
            "",
            " \n\t",
            "[llm\nbroken = ",
            tampered.as_str(),
            unsupported.as_str(),
        ] {
            fs::write(&path, content).unwrap();
            for keys in [ambient_only(), with_operator_key()] {
                assert!(
                    load_config_from_path_with(&path, &keys).is_err(),
                    "{content:?}"
                );
                for next in [NexusConfig::default(), with_credentials()] {
                    assert_eq!(
                        save_config_checked_to_path(&path, &next, &keys),
                        refused,
                        "{content:?}"
                    );
                    assert_eq!(fs::read_to_string(&path).unwrap(), content);
                }
            }
        }
        // A directory is no configuration to overwrite either.
        assert_eq!(
            save_config_checked_to_path(
                path.parent().unwrap(),
                &NexusConfig::default(),
                &ambient_only()
            ),
            refused
        );
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_loading_creates_only_a_missing_file() {
        let path = temp_config_path();
        let keys = ambient_only();
        // Missing: the first-run default, which holds no credential.
        assert_eq!(
            load_config_from_path_with(&path, &keys).unwrap(),
            NexusConfig::default()
        );
        assert!(path.exists());
        assert_eq!(
            load_config_from_path_with(&path, &keys).unwrap(),
            NexusConfig::default()
        );
        // Empty or whitespace-only: refused, never replaced.
        for content in ["", "  \n"] {
            fs::write(&path, content).unwrap();
            let error = load_config_from_path_with(&path, &keys).unwrap_err();
            assert!(error.to_string().contains("empty"), "{error}");
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_errors_never_carry_configuration_text() {
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Malformed legacy plaintext holding a credential.
        fs::write(
            &path,
            "[search]\nbrave_api_key = \"marker-secret-value\"\n[llm\n",
        )
        .unwrap();
        let error = load_config_from_path_with(&path, &ambient_only())
            .unwrap_err()
            .to_string();
        assert!(!error.contains("marker-secret-value"), "{error}");
        // A payload that decrypts but is not a configuration.
        let mut privacy = PrivacyManager::new();
        let encrypted = privacy
            .encrypt_field(
                b"brave_api_key = \"marker-secret-value\"\n[[",
                &vector_key(AMBIENT_VECTOR),
            )
            .unwrap();
        let envelope = EncryptedConfigEnvelope {
            version: 1,
            key_id: encrypted.key_id,
            nonce: encrypted.nonce,
            ciphertext: encrypted.ciphertext,
        };
        fs::write(&path, toml::to_string(&envelope).unwrap()).unwrap();
        let error = load_config_from_path_with(&path, &ambient_only())
            .unwrap_err()
            .to_string();
        assert!(!error.contains("marker-secret-value"), "{error}");
        cleanup(&path);
    }

    #[test]
    fn p0_fg_a_credential_field_lists_agree() {
        let mut config = NexusConfig::default();
        for (index, field) in credential_fields_mut(&mut config).into_iter().enumerate() {
            *field = format!("marker-{index}");
        }
        let seen: Vec<String> = credential_fields(&config)
            .iter()
            .map(|field| field.to_string())
            .collect();
        let expected: Vec<String> = (0..17).map(|index| format!("marker-{index}")).collect();
        assert_eq!(seen, expected);
        // An account identifier is not a credential.
        assert!(config.messaging.whatsapp_business_id.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn p0_fg_a_written_configuration_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_config_path();
        save_config_checked_to_path(&path, &with_credentials(), &with_operator_key()).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let entries = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(entries, 1, "no temporary file is left behind");
        cleanup(&path);
    }
}
