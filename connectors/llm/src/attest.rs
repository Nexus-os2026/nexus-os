//! Transport attestation (P3-HM-CONTRACT-02 §2.4, §11.0, §12.4 step 2).
//!
//! [`attest_route`] is the only producer of `TransportEvidence`. It looks at
//! the provider object a dispatch actually selected and recognises concrete
//! types of this crate only, by their exact type: there is no trait method a
//! third party could implement to claim a local route.
//!
//! - **(L1)** a [`VerifiedLocalOllama`]: the direct-loopback Ollama provider
//!   together with the model that passed the D-2 R2 verification. It can only
//!   be obtained from [`verify_local_ollama`].
//! - **(L2)** a [`FlashProvider`] built with the `flash-infer` feature
//!   (in-process inference through `nexus-llama-bridge`) whose model file is a
//!   regular file under the backend-owned model directory, after
//!   canonicalisation. Taking the provider from the backend-owned provider
//!   cache and returning a Flash error as an error (no remote fallback) are
//!   the dispatch path's duties (§2.4 L2 items 2 and 3); the build guard
//!   A-F2 covers item 4.
//!
//! Everything else, including a plain `OllamaProvider` (configured or
//! loopback) whose model was not verified, is non-local.

use std::any::Any;
use std::path::Path;

use nexus_kernel::errors::AgentError;
use nexus_kernel::hm::{RouteAttestation, TransportEvidence};

use crate::providers::{FlashProvider, LlmProvider, LlmResponse, OllamaProvider};

/// (L1) proof: the direct-loopback Ollama provider and the model the D-2 R2
/// verification accepted. Its fields are private; [`verify_local_ollama`]
/// is its only constructor. Every query goes to the verified model.
#[derive(Debug)]
pub struct VerifiedLocalOllama {
    provider: OllamaProvider,
    model: String,
}

impl VerifiedLocalOllama {
    /// The verified model.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The loopback address of the verified runtime.
    pub fn base_url(&self) -> &str {
        self.provider.base_url()
    }
}

/// The D-2 R2 verification of `model` at the loopback Ollama address `base`
/// (`gateway::verify_local_ollama_model`: a loopback address, not a cloud
/// model, installed there and not relayed elsewhere), as an (L1) proof.
pub fn verify_local_ollama(base: &str, model: &str) -> Result<VerifiedLocalOllama, AgentError> {
    crate::gateway::verify_local_ollama_model(base, model)?;
    Ok(VerifiedLocalOllama {
        provider: OllamaProvider::local_only(base)?,
        model: model.to_string(),
    })
}

impl LlmProvider for VerifiedLocalOllama {
    /// Sends to the verified model, whatever `model` names.
    fn query(
        &self,
        prompt: &str,
        max_tokens: u32,
        _model: &str,
    ) -> Result<LlmResponse, AgentError> {
        self.provider.query(prompt, max_tokens, &self.model)
    }

    fn name(&self) -> &str {
        self.provider.name()
    }

    fn cost_per_token(&self) -> f64 {
        self.provider.cost_per_token()
    }

    fn endpoint_url(&self) -> String {
        self.provider.endpoint_url()
    }
}

/// The attested route class of the provider a dispatch selected (§2.4).
/// `provider` is the concrete provider object (`&VerifiedLocalOllama`,
/// `&FlashProvider`, `&Arc<FlashProvider>`, or any other value, which is
/// non-local).
pub fn attest_route(provider: &dyn Any) -> RouteAttestation {
    attest_route_in(provider, flash_model_dir().as_deref())
}

/// [`attest_route`] with the Flash model directory given: `None` when the
/// backend-owned model directory cannot be established (then no Flash
/// provider is local). Used by `attest_route` and by the tests.
#[doc(hidden)]
pub fn attest_route_in(provider: &dyn Any, flash_model_dir: Option<&Path>) -> RouteAttestation {
    RouteAttestation::from_transport_evidence(evidence(provider, flash_model_dir))
}

fn evidence(provider: &dyn Any, flash_model_dir: Option<&Path>) -> TransportEvidence {
    if let Some(local) = provider.downcast_ref::<VerifiedLocalOllama>() {
        return if local.provider.is_local_only() && !local.model.trim().is_empty() {
            TransportEvidence::VerifiedLoopbackOllama
        } else {
            TransportEvidence::Unattested
        };
    }
    let flash = provider.downcast_ref::<FlashProvider>().or_else(|| {
        provider
            .downcast_ref::<std::sync::Arc<FlashProvider>>()
            .map(|arc| arc.as_ref())
    });
    match flash {
        Some(flash) if flash_in_process(flash, flash_model_dir) => {
            TransportEvidence::VerifiedInProcessFlash
        }
        _ => TransportEvidence::Unattested,
    }
}

/// §2.4 (L2) items 1 and 5: in-process inference is compiled in, and the
/// model file is a regular file under the model directory, both
/// canonicalised (a component-wise prefix, never a string prefix).
fn flash_in_process(flash: &FlashProvider, model_dir: Option<&Path>) -> bool {
    if !cfg!(feature = "flash-infer") {
        return false;
    }
    let Some(dir) = model_dir else {
        return false;
    };
    let (Ok(dir), Ok(model)) = (
        std::fs::canonicalize(dir),
        std::fs::canonicalize(flash.model_path()),
    ) else {
        return false;
    };
    let regular = std::fs::metadata(&model).is_ok_and(|m| m.file_type().is_file());
    regular && model != dir && model.starts_with(&dir)
}

/// The backend-owned Flash model directory: the model storage of the
/// identity home, as `nexus_flash_infer::ModelStorage::for_home` defines it
/// (its `download` feature is not enabled for this crate, so the path is
/// derived here; the desktop's tests compare the two). `None` without a
/// valid identity home. Nothing is created.
fn flash_model_dir() -> Option<std::path::PathBuf> {
    let home = nexus_kernel::identity_home::identity_home().ok()?;
    flash_model_dir_for_home(&home)
}

/// `ModelStorage::for_home(home).base_dir()`, computed without creating it.
#[doc(hidden)]
pub fn flash_model_dir_for_home(home: &Path) -> Option<std::path::PathBuf> {
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return None;
    }
    let data = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        home.join("AppData").join("Roaming")
    } else {
        home.join(".local").join("share")
    };
    Some(data.join("nexus-os").join("models"))
}
