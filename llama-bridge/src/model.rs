//! Safe wrapper around llama.cpp model loading and metadata extraction.

use std::ffi::{CStr, CString};
use std::ptr;

use tracing::info;

use crate::error::LlamaError;
use crate::ffi;
use crate::types::{MemoryUsage, ModelLoadConfig, ModelMetadata};

/// A loaded GGUF model. Owns the underlying `llama_model` pointer and frees
/// it on drop.
pub struct LlamaModel {
    ptr: *mut ffi::LlamaModel,
    metadata: ModelMetadata,
}

// llama_model is thread-safe for concurrent read access (multiple contexts
// can share one model). Mutation only occurs through llama_model_free.
unsafe impl Send for LlamaModel {}
unsafe impl Sync for LlamaModel {}

impl LlamaModel {
    /// Load a GGUF model from disk.
    pub fn load(config: &ModelLoadConfig) -> Result<Self, LlamaError> {
        let c_path =
            CString::new(config.model_path.as_str()).map_err(|_| LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "path contains null byte".into(),
            })?;

        let params = unsafe { ffi::nexus_model_params_create() };
        if params.is_null() {
            return Err(LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "failed to allocate model params".into(),
            });
        }
        unsafe {
            ffi::nexus_model_params_set_n_gpu_layers(params, config.n_gpu_layers);
            ffi::nexus_model_params_set_use_mmap(params, config.use_mmap);
            ffi::nexus_model_params_set_use_mlock(params, config.use_mlock);
        }

        let ptr = unsafe { ffi::nexus_model_load_from_file(c_path.as_ptr(), params) };
        unsafe { ffi::nexus_model_params_free(params) };
        if ptr.is_null() {
            return Err(LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "llama_model_load_from_file returned null".into(),
            });
        }

        let metadata = Self::extract_metadata(ptr);

        info!(
            arch = %metadata.architecture,
            params = metadata.total_params,
            ctx = metadata.context_length,
            moe = metadata.is_moe,
            "model loaded"
        );

        Ok(Self { ptr, metadata })
    }

    /// Cached model metadata.
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Estimate memory usage for a given context size.
    pub fn estimate_memory(&self, ctx_size: u32) -> MemoryUsage {
        let model_size_mb = self.metadata.file_size_bytes / (1024 * 1024);
        // Rough estimate: 2 bytes per token per layer for KV cache
        let kv_bytes = ctx_size as u64
            * self.metadata.num_layers as u64
            * self.metadata.embedding_size as u64
            * 2;
        let context_size_mb = kv_bytes / (1024 * 1024);
        MemoryUsage {
            model_size_mb,
            context_size_mb,
            total_mb: model_size_mb + context_size_mb,
        }
    }

    /// Raw pointer for use by [`LlamaContext`](crate::context::LlamaContext).
    pub(crate) fn as_mut_ptr(&self) -> *mut ffi::LlamaModel {
        self.ptr
    }

    /// Get the vocab pointer from this model.
    pub(crate) fn vocab(&self) -> *const ffi::LlamaVocab {
        unsafe { ffi::llama_model_get_vocab(self.ptr) }
    }

    fn extract_metadata(ptr: *mut ffi::LlamaModel) -> ModelMetadata {
        let total_params = unsafe { ffi::llama_model_n_params(ptr) };
        let file_size_bytes = unsafe { ffi::llama_model_size(ptr) };
        let context_length = unsafe { ffi::llama_model_n_ctx_train(ptr) } as u32;

        let vocab_ptr = unsafe { ffi::llama_model_get_vocab(ptr) };
        let vocab_size = if vocab_ptr.is_null() {
            0u32
        } else {
            (unsafe { ffi::llama_vocab_n_tokens(vocab_ptr) }) as u32
        };

        let architecture = Self::read_meta_str(ptr, "general.architecture");
        let quantization = Self::read_meta_str(ptr, "general.quantization_version");

        let expert_count_str = Self::read_meta_str(ptr, "llama.expert_count");
        let expert_used_str = Self::read_meta_str(ptr, "llama.expert_used_count");
        let num_experts = expert_count_str.parse::<u32>().ok();
        let num_active_experts = expert_used_str.parse::<u32>().ok();
        let is_moe = num_experts.is_some_and(|n| n > 1);

        let num_layers_str = Self::read_meta_str(ptr, "llama.block_count");
        let num_layers = num_layers_str.parse::<u32>().unwrap_or(0);

        let embd_str = Self::read_meta_str(ptr, "llama.embedding_length");
        let embedding_size = embd_str.parse::<u32>().unwrap_or(0);

        ModelMetadata {
            architecture,
            total_params,
            file_size_bytes,
            context_length,
            vocab_size,
            quantization,
            is_moe,
            num_experts,
            num_active_experts,
            num_layers,
            embedding_size,
        }
    }

    fn read_meta_str(model: *mut ffi::LlamaModel, key: &str) -> String {
        let c_key = match CString::new(key) {
            Ok(k) => k,
            Err(_) => return String::new(),
        };
        let mut buf = [0i8; 256];
        let ret = unsafe {
            ffi::llama_model_meta_val_str(
                model,
                c_key.as_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
            )
        };
        if ret < 0 {
            return String::new();
        }
        let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
        c_str.to_string_lossy().into_owned()
    }
}

impl Drop for LlamaModel {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::llama_model_free(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}
