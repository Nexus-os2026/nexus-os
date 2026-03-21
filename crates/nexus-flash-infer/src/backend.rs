use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::FlashError;
use crate::types::HardwareInfo;
use nexus_llama_bridge::{ControlFlow, GenerationConfig, ModelMetadata, PerfStats, TokenEvent};

/// Supported model file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelFormat {
    /// GGUF format — 60+ architectures via llama.cpp.
    GGUF,
    /// SafeTensors — auto-convert to GGUF for inference.
    SafeTensors,
    /// Apple MLX format (future).
    MLX,
    /// ONNX Runtime format (future).
    ONNX,
}

/// Memory usage breakdown for a loaded model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub model_size_mb: u64,
    pub context_size_mb: u64,
    pub total_mb: u64,
}

impl From<nexus_llama_bridge::MemoryUsage> for MemoryUsage {
    fn from(m: nexus_llama_bridge::MemoryUsage) -> Self {
        Self {
            model_size_mb: m.model_size_mb,
            context_size_mb: m.context_size_mb,
            total_mb: m.total_mb,
        }
    }
}

/// Load configuration for a model backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfig {
    pub model_path: String,
    pub n_gpu_layers: i32,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub cpu_moe: bool,
    pub n_threads: Option<u32>,
    pub numa_strategy: nexus_llama_bridge::NumaStrategy,
    pub n_ctx: u32,
    pub n_batch: u32,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            n_gpu_layers: 0,
            use_mmap: true,
            use_mlock: false,
            cpu_moe: true,
            n_threads: None,
            numa_strategy: nexus_llama_bridge::NumaStrategy::Disabled,
            n_ctx: 4096,
            n_batch: 512,
        }
    }
}

/// Universal inference backend trait — any engine implements this.
pub trait InferenceBackend: Send + Sync {
    /// Backend name (e.g. "llama.cpp").
    fn name(&self) -> &str;

    /// Model formats this backend can load.
    fn supported_formats(&self) -> Vec<ModelFormat>;

    /// Hardware capabilities detected by this backend.
    fn hardware_capabilities(&self) -> HardwareInfo;

    /// Probe a model file to extract metadata without fully loading it.
    fn probe_model(&self, path: &Path) -> Result<ModelMetadata, FlashError>;

    /// Load a model with the given configuration.
    fn load_model(
        &self,
        path: &Path,
        config: &LoadConfig,
    ) -> Result<Box<dyn ModelHandle>, FlashError>;
}

/// A loaded model — uniform interface regardless of backend.
pub trait ModelHandle: Send + Sync {
    /// Run generation with streaming callback.
    fn generate(
        &self,
        prompt: &str,
        config: &GenerationConfig,
        callback: Box<dyn FnMut(TokenEvent) -> ControlFlow + Send>,
    ) -> Result<PerfStats, FlashError>;

    /// Current memory usage of this loaded model.
    fn memory_usage(&self) -> MemoryUsage;

    /// Metadata extracted from the model.
    fn metadata(&self) -> &ModelMetadata;

    /// Unload the model and free resources.
    fn unload(&mut self) -> Result<(), FlashError>;
}
