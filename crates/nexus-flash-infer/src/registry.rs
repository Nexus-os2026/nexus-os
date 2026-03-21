use std::collections::HashMap;
use std::path::Path;

use crate::backend::{InferenceBackend, ModelFormat};
use crate::error::FlashError;

/// Registry of available inference backends.
/// Selects the best backend for a given model format.
pub struct BackendRegistry {
    backends: Vec<Box<dyn InferenceBackend>>,
    format_priority: HashMap<ModelFormat, Vec<usize>>,
}

impl BackendRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            format_priority: HashMap::new(),
        }
    }

    /// Register a backend. The first registered backend for a format has highest priority.
    pub fn register(&mut self, backend: Box<dyn InferenceBackend>) {
        let idx = self.backends.len();
        for format in backend.supported_formats() {
            self.format_priority.entry(format).or_default().push(idx);
        }
        self.backends.push(backend);
    }

    /// Get the best backend for a model file.
    /// Detects format from file extension, then returns the highest-priority backend.
    pub fn select_backend(&self, model_path: &Path) -> Result<&dyn InferenceBackend, FlashError> {
        let format = detect_format(model_path)?;
        self.select_for_format(&format)
    }

    /// Get the best backend for a specific format.
    pub fn select_for_format(
        &self,
        format: &ModelFormat,
    ) -> Result<&dyn InferenceBackend, FlashError> {
        let indices = self
            .format_priority
            .get(format)
            .ok_or(FlashError::NoBackendForFormat(*format))?;

        let idx = indices
            .first()
            .ok_or(FlashError::NoBackendForFormat(*format))?;

        Ok(self.backends[*idx].as_ref())
    }

    /// List all registered backends.
    pub fn list_backends(&self) -> Vec<&str> {
        self.backends.iter().map(|b| b.name()).collect()
    }

    /// List all supported formats.
    pub fn supported_formats(&self) -> Vec<ModelFormat> {
        self.format_priority.keys().copied().collect()
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect model format from file extension.
pub fn detect_format(path: &Path) -> Result<ModelFormat, FlashError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "gguf" => Ok(ModelFormat::GGUF),
        "safetensors" => Ok(ModelFormat::SafeTensors),
        "mlx" => Ok(ModelFormat::MLX),
        "onnx" => Ok(ModelFormat::ONNX),
        _ => Err(FlashError::InvalidConfig(format!(
            "Unknown model format for extension '.{}'. Supported: .gguf, .safetensors, .mlx, .onnx",
            ext
        ))),
    }
}
