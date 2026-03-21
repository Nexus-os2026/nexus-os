//! Flash inference provider — runs GGUF models locally via nexus-flash-infer.

use nexus_kernel::errors::AgentError;

#[cfg(feature = "flash-infer")]
use nexus_flash_infer::InferenceBackend;

use super::{LlmProvider, LlmResponse};

/// LLM provider that uses the nexus-flash-infer engine for local GGUF inference.
///
/// Unlike cloud providers, FlashProvider runs models entirely on the local machine
/// using llama.cpp via `nexus-llama-bridge`. It supports MoE expert streaming,
/// mmap-based disk offloading, and automatic memory budgeting.
///
/// When compiled with the `flash-infer` feature, `query()` performs real inference.
/// Without it, `query()` returns an error directing users to the Flash Inference UI.
pub struct FlashProvider {
    model_path: String,
    #[cfg(feature = "flash-infer")]
    model_handle: std::sync::Mutex<Option<Box<dyn nexus_flash_infer::ModelHandle>>>,
}

impl FlashProvider {
    /// Create a provider for a specific model path.
    pub fn new(model_path: String) -> Self {
        Self {
            model_path,
            #[cfg(feature = "flash-infer")]
            model_handle: std::sync::Mutex::new(None),
        }
    }

    /// Load the model into memory if not already loaded.
    #[cfg(feature = "flash-infer")]
    fn ensure_loaded(&self) -> Result<(), AgentError> {
        let mut guard = self
            .model_handle
            .lock()
            .map_err(|e| AgentError::SupervisorError(format!("flash lock poisoned: {e}")))?;

        if guard.is_some() {
            return Ok(());
        }

        let hw = nexus_flash_infer::detect_hardware();
        let backend = nexus_flash_infer::LlamaBackend::new(hw);

        let path = std::path::Path::new(&self.model_path);
        let config = nexus_flash_infer::LoadConfig::default();

        let handle = backend
            .load_model(path, &config)
            .map_err(|e| AgentError::SupervisorError(format!("flash model load failed: {e}")))?;

        *guard = Some(handle);
        Ok(())
    }
}

impl LlmProvider for FlashProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let model_display = if model.is_empty() {
            &self.model_path
        } else {
            model
        };

        #[cfg(feature = "flash-infer")]
        {
            self.ensure_loaded()?;

            let guard = self
                .model_handle
                .lock()
                .map_err(|e| AgentError::SupervisorError(format!("flash lock poisoned: {e}")))?;

            let handle = guard
                .as_ref()
                .ok_or_else(|| AgentError::SupervisorError("flash model not loaded".to_string()))?;

            let gen_config = nexus_llama_bridge::GenerationConfig {
                max_tokens,
                ..Default::default()
            };

            let output = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let token_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

            let out_clone = output.clone();
            let count_clone = token_count.clone();

            let callback = Box::new(
                move |event: nexus_llama_bridge::TokenEvent| -> nexus_llama_bridge::ControlFlow {
                    if let nexus_llama_bridge::TokenEvent::Token { text, .. } = &event {
                        if let Ok(mut buf) = out_clone.lock() {
                            buf.push_str(text);
                        }
                        count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    nexus_llama_bridge::ControlFlow::Continue
                },
            );

            let _stats = handle
                .generate(prompt, &gen_config, callback)
                .map_err(|e| AgentError::SupervisorError(format!("flash inference failed: {e}")))?;

            let output_text = output
                .lock()
                .map_err(|e| AgentError::SupervisorError(format!("flash output lock: {e}")))?
                .clone();
            let tokens = token_count.load(std::sync::atomic::Ordering::Relaxed);

            Ok(LlmResponse {
                output_text,
                token_count: tokens,
                model_name: model_display.to_string(),
                tool_calls: Vec::new(),
            })
        }

        #[cfg(not(feature = "flash-infer"))]
        Err(AgentError::SupervisorError(format!(
            "Flash provider '{model_display}': direct query not supported — \
             compile with `flash-infer` feature or use the Flash Inference UI. \
             Prompt length: {} chars, max_tokens: {max_tokens}",
            prompt.len()
        )))
    }

    fn name(&self) -> &str {
        "flash"
    }

    fn cost_per_token(&self) -> f64 {
        0.0 // Local inference — no API cost
    }

    fn endpoint_url(&self) -> String {
        "local://flash-infer".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_provider_name() {
        let provider = FlashProvider::new("test-model".into());
        assert_eq!(provider.name(), "flash");
    }

    #[test]
    fn test_flash_provider_free() {
        let provider = FlashProvider::new("test-model".into());
        assert_eq!(provider.cost_per_token(), 0.0);
        assert!(!provider.is_paid());
    }

    #[test]
    fn test_flash_provider_endpoint() {
        let provider = FlashProvider::new("test-model".into());
        assert_eq!(provider.endpoint_url(), "local://flash-infer");
    }

    #[test]
    fn test_flash_provider_query_without_feature() {
        let provider = FlashProvider::new("qwen3.5-moe.gguf".into());
        // Without flash-infer feature, query returns an error
        #[cfg(not(feature = "flash-infer"))]
        {
            let result = provider.query("Hello", 100, "");
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("Flash provider"));
        }
        // With flash-infer feature, query would attempt real inference
        // (which fails without a real model file — tested in integration tests)
        #[cfg(feature = "flash-infer")]
        {
            let _ = provider; // suppress unused warning
        }
    }
}
