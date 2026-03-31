use std::path::Path;
use std::sync::Mutex;

use nexus_llama_bridge::{
    ControlFlow, GenerationConfig, LlamaContext, LlamaModel, ModelLoadConfig, ModelMetadata,
    PerfStats, TokenEvent,
};

use crate::backend::{InferenceBackend, LoadConfig, MemoryUsage, ModelFormat, ModelHandle};
use crate::error::FlashError;
use crate::types::HardwareInfo;

/// Pre-warm the page cache by reading model files with MADV_SEQUENTIAL.
///
/// For MoE models, the model is mmap'd and expert weights are loaded on
/// demand. This causes cold page faults that hit NVMe (~3.5 GB/s) instead
/// of RAM (~11 GB/s DDR5). By sequentially reading the files once, we
/// populate the page cache so subsequent random expert access hits RAM.
///
/// Spawns a background thread so it doesn't block model loading.
fn warm_page_cache(model_path: &Path) {
    let path = model_path.to_path_buf();
    std::thread::Builder::new()
        .name("page-cache-warm".into())
        .spawn(move || {
            let paths = collect_model_files(&path);
            let total_bytes: u64 = paths
                .iter()
                // Optional: skip files whose metadata can't be read
                .filter_map(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
                .sum();

            tracing::info!(
                files = paths.len(),
                total_gb = format_args!("{:.1}", total_bytes as f64 / 1024.0 / 1024.0 / 1024.0),
                "starting page cache warmup"
            );

            for file_path in &paths {
                if let Err(e) = warm_single_file(file_path) {
                    tracing::warn!(path = %file_path.display(), error = %e, "warmup failed");
                }
            }

            tracing::info!("page cache warmup complete");
        })
        // Optional: page cache warmup is non-critical, don't fail model loading
        .ok();
}

/// Collect all GGUF split files for a model path.
///
/// For split models like `model-00001-of-00004.gguf`, returns all 4 files.
/// For single files, returns just that file.
fn collect_model_files(path: &Path) -> Vec<std::path::PathBuf> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();

    // Check for split pattern: *-NNNNN-of-NNNNN.gguf
    if let Some(of_pos) = name.find("-of-") {
        if let Some(dash_pos) = name[..of_pos].rfind('-') {
            let prefix = &name[..dash_pos + 1];
            let suffix_part = &name[of_pos + 4..];
            if let Some(total_str) = suffix_part.strip_suffix(".gguf") {
                if let Ok(total) = total_str.parse::<u32>() {
                    let parent = path.parent().unwrap_or(Path::new("."));
                    let mut files = Vec::with_capacity(total as usize);
                    for i in 1..=total {
                        let split_name = format!("{prefix}{i:05}-of-{total:05}.gguf");
                        let split_path = parent.join(&split_name);
                        if split_path.exists() {
                            files.push(split_path);
                        }
                    }
                    if !files.is_empty() {
                        return files;
                    }
                }
            }
        }
    }

    vec![path.to_path_buf()]
}

/// Hint the OS to start pre-reading a file into the page cache.
///
/// Uses `posix_fadvise(POSIX_FADV_WILLNEED)` which is **non-blocking** —
/// the kernel starts async readahead in the background without consuming
/// user CPU or blocking the calling thread. This is much gentler than
/// sequentially reading the file ourselves, which would saturate NVMe
/// bandwidth and freeze the system.
fn warm_single_file(path: &Path) -> Result<(), std::io::Error> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len() as usize;
    if len == 0 {
        return Ok(());
    }

    // POSIX_FADV_WILLNEED: async kernel readahead — non-blocking, gentle on I/O
    nexus_llama_bridge::fadvise_willneed(&file, len);

    tracing::debug!(
        path = %path.display(),
        size_gb = format_args!("{:.1}", len as f64 / 1024.0 / 1024.0 / 1024.0),
        "hinted page cache warmup (fadvise WILLNEED)"
    );

    Ok(())
}

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

        // Start background page cache warmup for MoE models.
        // This pre-reads model files into the page cache so expert page faults
        // hit RAM (~11 GB/s DDR5) instead of NVMe (~3.5 GB/s).
        if config.cpu_moe {
            warm_page_cache(path);
        }

        // Compute n_ubatch from n_batch: for MoE, use smaller ubatch to reduce
        // peak memory pressure. The autoconfig sets n_batch=512 for MoE and we
        // want n_ubatch <= n_batch. Default::default() would use 512 which is
        // too large for huge MoE models.
        let n_ubatch = if config.cpu_moe {
            (config.n_batch / 2).clamp(64, 256) // MoE: half of batch, clamped
        } else {
            config.n_batch.min(512)
        };
        let gen_config = GenerationConfig {
            n_ctx: config.n_ctx,
            n_batch: config.n_batch,
            n_ubatch,
            n_threads: config.n_threads,
            flash_attn: true, // Always enable — reduces KV memory and speeds attention
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
    // Owns the loaded model; must stay alive while context is in use.
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
