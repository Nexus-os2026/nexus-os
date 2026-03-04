use crate::errors::AgentError;
use crate::privacy::{EncryptedField, PrivacyManager, UserKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NexusConfig {
    pub llm: LlmConfig,
    pub search: SearchConfig,
    pub social: SocialConfig,
    pub messaging: MessagingConfig,
    pub voice: VoiceConfig,
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub kill_gates: KillGatesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmConfig {
    pub default_model: String,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub ollama_url: String,
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
                ollama_url: "http://localhost:11434".to_string(),
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
            kill_gates: KillGatesConfig::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Some(path) = env::var_os("NEXUS_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".nexus").join("config.toml")
}

pub fn load_config() -> Result<NexusConfig, AgentError> {
    load_config_from_path(config_path().as_path())
}

pub fn save_config(config: &NexusConfig) -> Result<(), AgentError> {
    save_config_to_path(config_path().as_path(), config)
}

pub fn load_config_from_path(path: &Path) -> Result<NexusConfig, AgentError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(to_io_error)?;
        }
        let default_config = NexusConfig::default();
        save_config_to_path(path, &default_config)?;
        return Ok(default_config);
    }

    let raw = fs::read_to_string(path).map_err(to_io_error)?;
    if raw.trim().is_empty() {
        let default_config = NexusConfig::default();
        save_config_to_path(path, &default_config)?;
        return Ok(default_config);
    }

    if let Ok(envelope) = toml::from_str::<EncryptedConfigEnvelope>(&raw) {
        decrypt_envelope(&envelope)
    } else {
        // Backward-compatible migration path for plaintext config files.
        let plaintext = toml::from_str::<NexusConfig>(&raw).map_err(|error| {
            AgentError::SupervisorError(format!("invalid config format: {error}"))
        })?;
        save_config_to_path(path, &plaintext)?;
        Ok(plaintext)
    }
}

pub fn save_config_to_path(path: &Path, config: &NexusConfig) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(to_io_error)?;
    }

    let plaintext = toml::to_string(config).map_err(|error| {
        AgentError::SupervisorError(format!("unable to serialize config: {error}"))
    })?;
    let envelope = encrypt_config(&plaintext)?;
    let encoded = toml::to_string(&envelope).map_err(|error| {
        AgentError::SupervisorError(format!("unable to encode encrypted config: {error}"))
    })?;

    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, encoded).map_err(to_io_error)?;
    set_restrictive_permissions(&tmp)?;
    fs::rename(&tmp, path).map_err(to_io_error)?;
    Ok(())
}

fn encrypt_config(plaintext: &str) -> Result<EncryptedConfigEnvelope, AgentError> {
    let mut privacy = PrivacyManager::new();
    let user_key = config_user_key();
    let encrypted = privacy.encrypt_field(plaintext.as_bytes(), &user_key)?;
    Ok(EncryptedConfigEnvelope {
        version: 1,
        key_id: encrypted.key_id,
        nonce: encrypted.nonce,
        ciphertext: encrypted.ciphertext,
    })
}

fn decrypt_envelope(envelope: &EncryptedConfigEnvelope) -> Result<NexusConfig, AgentError> {
    let privacy = PrivacyManager::new();
    let user_key = config_user_key();
    let encrypted = EncryptedField {
        key_id: envelope.key_id.clone(),
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext.clone(),
    };
    let plaintext = privacy.decrypt_field(&encrypted, &user_key)?;
    let decoded = String::from_utf8(plaintext).map_err(|error| {
        AgentError::SupervisorError(format!("config payload is not utf-8: {error}"))
    })?;
    toml::from_str::<NexusConfig>(&decoded).map_err(|error| {
        AgentError::SupervisorError(format!("unable to decode config payload: {error}"))
    })
}

fn config_user_key() -> UserKey {
    let mut hasher = Sha256::new();
    hasher.update(b"nexus-config-key-v1");

    if let Ok(explicit) = env::var("NEXUS_CONFIG_KEY") {
        hasher.update(explicit.as_bytes());
    } else {
        if let Some(home) = env::var_os("HOME") {
            hasher.update(home.to_string_lossy().as_bytes());
        }
        if let Some(user) = env::var_os("USER") {
            hasher.update(user.to_string_lossy().as_bytes());
        }
        if let Some(username) = env::var_os("USERNAME") {
            hasher.update(username.to_string_lossy().as_bytes());
        }
        if let Some(hostname) = env::var_os("HOSTNAME") {
            hasher.update(hostname.to_string_lossy().as_bytes());
        }
    }

    let digest = hasher.finalize();
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest[..32]);
    UserKey {
        id: "nexus-config-v1".to_string(),
        bytes: key,
    }
}

fn to_io_error(error: std::io::Error) -> AgentError {
    AgentError::SupervisorError(format!("config I/O error: {error}"))
}

#[cfg(unix)]
fn set_restrictive_permissions(path: &Path) -> Result<(), AgentError> {
    use std::os::unix::fs::PermissionsExt;

    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms).map_err(to_io_error)
}

#[cfg(not(unix))]
fn set_restrictive_permissions(_path: &Path) -> Result<(), AgentError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_config_from_path, save_config_to_path, NexusConfig};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_config_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("nexus-config-test-{}", Uuid::new_v4()));
        base.join(".nexus").join("config.toml")
    }

    #[test]
    fn test_config_create_and_load() {
        let path = temp_config_path();
        let mut config = NexusConfig::default();
        config.llm.anthropic_api_key = "sk-ant-test".to_string();
        config.search.brave_api_key = "brave-key".to_string();
        config.messaging.telegram_bot_token = "123:abc".to_string();
        config.voice.wake_word = "hey nexus".to_string();

        let save = save_config_to_path(path.as_path(), &config);
        assert!(save.is_ok());

        let loaded = load_config_from_path(path.as_path());
        assert!(loaded.is_ok());
        assert_eq!(loaded.unwrap_or_default(), config);
    }

    #[test]
    fn test_config_encrypted_at_rest() {
        let path = temp_config_path();
        let mut config = NexusConfig::default();
        config.llm.anthropic_api_key = "sk-ant-plaintext-check".to_string();

        let save = save_config_to_path(path.as_path(), &config);
        assert!(save.is_ok());

        let raw = fs::read_to_string(path.as_path()).unwrap_or_default();
        assert!(!raw.contains("sk-ant-plaintext-check"));
        assert!(!raw.contains("[llm]"));

        let loaded = load_config_from_path(path.as_path());
        assert!(loaded.is_ok());
        let _ = fs::remove_file(path.as_path());
    }
}
