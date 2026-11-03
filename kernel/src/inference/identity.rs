//! The identity of the one qualified local model.

use sha2::{Digest, Sha256};

use super::request::MIN_NUM_CTX;
use super::ModelError;

/// The only provider of the local model transport.
pub const LOCAL_PROVIDER: &str = "ollama";

const MAX_MODEL_NAME_BYTES: usize = 128;
const MAX_RUNTIME_VERSION_BYTES: usize = 32;
const MAX_NUM_CTX: u32 = 262_144;
const MAX_TEMPERATURE_MILLI: u16 = 2_000;

/// The thinking setting sent with every request. Only `Off` is qualified:
/// `think: false` is sent explicitly, and the model must list the
/// `thinking` capability, so that switching it off is meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Think {
    Off,
}

/// The fixed request configuration of a pinned model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub num_ctx: u32,
    pub temperature_milli: u16,
    pub seed: u32,
    pub think: Think,
    pub truncate: bool,
}

impl RuntimeConfig {
    /// The canonical text of the configuration (the digests' input).
    fn canonical(&self) -> String {
        let think = match self.think {
            Think::Off => "off",
        };
        format!(
            "num_ctx={};temperature_milli={};seed={};think={think};truncate={}",
            self.num_ctx, self.temperature_milli, self.seed, self.truncate
        )
    }

    /// SHA-256 over `nexus.inference.config.v1` and the canonical text.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"nexus.inference.config.v1");
        length_prefixed(&mut hash, self.canonical().as_bytes());
        hash.finalize().into()
    }

    /// A configuration the transport can keep its bounds under: the
    /// runtime's truncation is off, and the context is large enough that the
    /// byte bound keeps the prompt under half of it.
    fn check(&self) -> Result<(), ModelError> {
        if self.truncate
            || self.num_ctx < MIN_NUM_CTX
            || self.num_ctx > MAX_NUM_CTX
            || self.temperature_milli > MAX_TEMPERATURE_MILLI
        {
            return Err(ModelError::ConfigMismatch);
        }
        Ok(())
    }
}

/// What a caller pins: the model's name and manifest digest (64 lowercase
/// hex characters, as `/api/tags` lists it), the runtime's version and the
/// configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPinSpec {
    pub name: String,
    pub digest: String,
    pub runtime_version: String,
    pub config: RuntimeConfig,
}

/// A checked model identity. [`ModelIdentity::from_spec`] checks only its
/// spelling; only [`super::VerifiedOllama::verify`] establishes that the
/// local runtime serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    name: String,
    model_digest: String,
    runtime_version: String,
    config: RuntimeConfig,
}

impl ModelIdentity {
    /// Check a pin's spelling: the name grammar, no cloud name, a digest of
    /// 64 lowercase hex characters, a plain version and a configuration the
    /// transport can bound. No request is made.
    pub fn from_spec(spec: &ModelPinSpec) -> Result<Self, ModelError> {
        let name = checked_model_name(&spec.name)?;
        if super::ollama::name_is_cloud(name) {
            return Err(ModelError::CloudName);
        }
        if spec.digest.len() != 64
            || !spec
                .digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ModelError::DigestMismatch);
        }
        let version = &spec.runtime_version;
        if version.is_empty()
            || version.len() > MAX_RUNTIME_VERSION_BYTES
            || !version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
        {
            return Err(ModelError::RuntimeMismatch);
        }
        spec.config.check()?;
        Ok(Self {
            name: name.to_string(),
            model_digest: spec.digest.clone(),
            runtime_version: version.clone(),
            config: spec.config,
        })
    }

    pub fn provider(&self) -> &'static str {
        LOCAL_PROVIDER
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The model's manifest digest (hex).
    pub fn model_digest(&self) -> &str {
        &self.model_digest
    }

    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// SHA-256 over `nexus.inference.identity.v1` and the length-prefixed
    /// provider, name, model digest, runtime version and canonical
    /// configuration.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"nexus.inference.identity.v1");
        for field in [
            LOCAL_PROVIDER,
            &self.name,
            &self.model_digest,
            &self.runtime_version,
            &self.config.canonical(),
        ] {
            length_prefixed(&mut hash, field.as_bytes());
        }
        hash.finalize().into()
    }
}

fn length_prefixed(hash: &mut Sha256, field: &[u8]) {
    hash.update(u32::try_from(field.len()).unwrap_or(u32::MAX).to_be_bytes());
    hash.update(field);
}

/// A model name: 1–128 bytes of ASCII letters, digits and `._:-/`, starting
/// with a letter or digit.
pub(super) fn checked_model_name(name: &str) -> Result<&str, ModelError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_MODEL_NAME_BYTES
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-' | '/'));
    if valid {
        Ok(name)
    } else {
        Err(ModelError::InvalidModelName)
    }
}
