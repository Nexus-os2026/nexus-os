use std::path::Path;
use std::sync::Mutex;

use nexus_llama_bridge::{
    ControlFlow, GenerationConfig, LlamaContext, LlamaModel, ModelLoadConfig, ModelMetadata,
    PerfStats, TokenEvent,
};

use crate::backend::{InferenceBackend, LoadConfig, MemoryUsage, ModelFormat, ModelHandle};
use crate::error::FlashError;
use crate::types::HardwareInfo;

/// llama.cpp backend implementation via nexus-llama-bridge.
pub struct LlamaBackend {
    hw: HardwareInfo,
}

impl LlamaBackend {
    /// Create a new llama.cpp backend.
    pub fn new(hw: HardwareInfo) -> Self {
        nexus_llama_bridge::init();
        Self { hw }
    }
}

impl InferenceBackend for LlamaBackend {
    fn name(&self) -> &str {
        "llama.cpp"
    }

    fn supported_formats(&self) -> Vec<ModelFormat> {
        vec![ModelFormat::GGUF]
    }

    fn hardware_capabilities(&self) -> HardwareInfo {
        self.hw.clone()
    }

    fn probe_model(&self, path: &Path) -> Result<ModelMetadata, FlashError> {
        let config = ModelLoadConfig {
            model_path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let model = LlamaModel::load(&config).map_err(FlashError::LlamaBridge)?;
        Ok(model.metadata().clone())
    }

    fn load_model(
        &self,
        path: &Path,
        config: &LoadConfig,
    ) -> Result<Box<dyn ModelHandle>, FlashError> {
        let load_config = ModelLoadConfig {
            model_path: path.to_string_lossy().into_owned(),
            n_gpu_layers: config.n_gpu_layers,
            use_mmap: config.use_mmap,
            use_mlock: config.use_mlock,
            cpu_moe: config.cpu_moe,
            n_threads: config.n_threads,
            numa_strategy: config.numa_strategy,
        };

        let model = LlamaModel::load(&load_config).map_err(FlashError::LlamaBridge)?;

        let gen_config = GenerationConfig {
            n_ctx: config.n_ctx,
            n_batch: config.n_batch,
            ..Default::default()
        };

        let context = LlamaContext::new(&model, &gen_config).map_err(FlashError::LlamaBridge)?;

        let metadata = model.metadata().clone();
        let memory = model.estimate_memory(config.n_ctx);

        Ok(Box::new(LlamaModelHandle {
            model,
            context: Mutex::new(Some(context)),
            metadata,
            memory_usage: MemoryUsage::from(memory),
        }))
    }
}

/// A loaded llama.cpp model handle.
struct LlamaModelHandle {
    #[allow(dead_code)]
    model: LlamaModel,
    context: Mutex<Option<LlamaContext>>,
    metadata: ModelMetadata,
    memory_usage: MemoryUsage,
}

impl ModelHandle for LlamaModelHandle {
    fn generate(
        &self,
        prompt: &str,
        config: &GenerationConfig,
        callback: Box<dyn FnMut(TokenEvent) -> ControlFlow + Send>,
    ) -> Result<PerfStats, FlashError> {
        let mut guard = self
            .context
            .lock()
            .map_err(|e| FlashError::BackendError(format!("Lock poisoned: {}", e)))?;

        let ctx = guard
            .as_mut()
            .ok_or_else(|| FlashError::BackendError("Model already unloaded".into()))?;

        ctx.generate_sync(prompt, config, callback)
            .map_err(FlashError::LlamaBridge)
    }

    fn memory_usage(&self) -> MemoryUsage {
        self.memory_usage.clone()
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn unload(&mut self) -> Result<(), FlashError> {
        let mut guard = self
            .context
            .lock()
            .map_err(|e| FlashError::BackendError(format!("Lock poisoned: {}", e)))?;
        *guard = None;
        Ok(())
    }
}
