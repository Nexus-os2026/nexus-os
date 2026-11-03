//! The identity of the one qualified local model.
//!
//! M2e-A1 interface stub: the inherited behaviour. The coding-run pin is a
//! provider, an endpoint and a model name (F2); this stub checks only the
//! name's grammar. The digests are the frozen encodings (no check is made
//! with them here).

use sha2::{Digest, Sha256};

use super::ModelError;

/// The only provider of the local model transport.
pub const LOCAL_PROVIDER: &str = "ollama";

const MAX_MODEL_NAME_BYTES: usize = 128;

/// The thinking setting sent with every request.
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
}

/// What a caller pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPinSpec {
    pub name: String,
    pub digest: String,
    pub runtime_version: String,
    pub config: RuntimeConfig,
}

/// A model identity (stub: only the name is checked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    name: String,
    model_digest: String,
    runtime_version: String,
    config: RuntimeConfig,
}

impl ModelIdentity {
    pub fn from_spec(spec: &ModelPinSpec) -> Result<Self, ModelError> {
        Ok(Self {
            name: checked_model_name(&spec.name)?.to_string(),
            model_digest: spec.digest.clone(),
            runtime_version: spec.runtime_version.clone(),
            config: spec.config,
        })
    }

    pub fn provider(&self) -> &'static str {
        LOCAL_PROVIDER
    }

    pub fn name(&self) -> &str {
        &self.name
    }

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

/// The inherited name grammar.
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
