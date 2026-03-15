//! Model discovery and management for local SLM inference.
//!
//! `ModelRegistry` scans `~/.nexus/models/` for available models, each stored
//! in its own directory with `config.json`, `tokenizer.json`,
//! `model.safetensors`, and a `nexus-model.toml` metadata file.
//!
//! The data structures (`ModelConfig`, `Quantization`, `LoadedModel`) are
//! always available regardless of the `local-slm` feature flag. The runtime
//! loading logic that depends on candle is gated behind
//! `#[cfg(feature = "local-slm")]`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "local-slm")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "local-slm")]
use tokenizers::Tokenizer;

/// Quantization level for a local model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantization {
    /// 4-bit quantized — smallest footprint, fastest, least accurate.
    Q4,
    /// 8-bit quantized — good balance of size, speed, and accuracy.
    Q8,
    /// 16-bit floating point — high quality, moderate RAM.
    F16,
    /// 32-bit floating point — full precision, highest RAM usage.
    F32,
}

impl std::fmt::Display for Quantization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Q4 => write!(f, "Q4"),
            Self::Q8 => write!(f, "Q8"),
            Self::F16 => write!(f, "F16"),
            Self::F32 => write!(f, "F32"),
        }
    }
}

impl Quantization {
    /// Candle DType corresponding to this quantization level.
    #[cfg(feature = "local-slm")]
    pub fn to_dtype(self) -> DType {
        match self {
            Self::Q4 | Self::Q8 => DType::F32, // quantized weights dequantized to f32 at load
            Self::F16 => DType::F16,
            Self::F32 => DType::F32,
        }
    }
}

/// Configuration describing a locally-available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// HuggingFace-style model identifier, e.g. `"microsoft/phi-4"`.
    pub model_id: String,
    /// Local directory containing model weights and tokenizer.
    pub model_path: PathBuf,
    /// Quantization level of the stored weights.
    pub quantization: Quantization,
    /// Maximum context length in tokens.
    pub max_context_length: usize,
    /// Governance task types this model is recommended for.
    pub recommended_tasks: Vec<String>,
    /// Minimum RAM in megabytes required to load this model.
    pub min_ram_mb: usize,
}

/// A model loaded into memory, ready for inference.
///
/// Wrapped in `Arc` so multiple governance tasks can share the same loaded
/// model without duplicating weights in memory.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    /// The configuration for this model.
    pub config: ModelConfig,
    /// Candle tensor weights (feature-gated).
    #[cfg(feature = "local-slm")]
    pub weights: Arc<Vec<(String, Tensor)>>,
    /// Tokenizer for encoding/decoding text.
    #[cfg(feature = "local-slm")]
    pub tokenizer: Arc<Tokenizer>,
    /// Device the model is loaded on (CPU by default).
    #[cfg(feature = "local-slm")]
    pub device: Device,
}

/// Registry for discovering and managing local models.
///
/// Scans a models directory (default `~/.nexus/models/`) for subdirectories
/// containing a `nexus-model.toml` metadata file. Each subdirectory is one
/// model variant.
#[derive(Debug)]
pub struct ModelRegistry {
    /// Root directory to scan for models.
    models_dir: PathBuf,
    /// Discovered model configurations.
    available_models: Vec<ModelConfig>,
    /// Currently loaded models, keyed by model_id.
    loaded_models: HashMap<String, Arc<LoadedModel>>,
}

impl Clone for ModelRegistry {
    fn clone(&self) -> Self {
        Self {
            models_dir: self.models_dir.clone(),
            available_models: self.available_models.clone(),
            loaded_models: self.loaded_models.clone(),
        }
    }
}

impl ModelRegistry {
    /// Create a new registry pointing at the given models directory.
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            available_models: Vec::new(),
            loaded_models: HashMap::new(),
        }
    }

    /// Create a registry using the default `~/.nexus/models/` directory.
    pub fn default_dir() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::new(PathBuf::from(home).join(".nexus").join("models"))
    }

    /// Root directory this registry scans.
    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }

    /// List all discovered models.
    pub fn available_models(&self) -> &[ModelConfig] {
        &self.available_models
    }

    /// Scan the models directory for available models.
    ///
    /// Each subdirectory with a `nexus-model.toml` is treated as a model.
    /// Returns the number of models discovered.
    pub fn discover(&mut self) -> usize {
        self.available_models.clear();

        let entries = match std::fs::read_dir(&self.models_dir) {
            Ok(entries) => entries,
            Err(_) => return 0,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let toml_path = path.join("nexus-model.toml");
            if !toml_path.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&toml_path) {
                if let Some(config) = Self::parse_model_toml(&content, &path) {
                    self.available_models.push(config);
                }
            }
        }

        self.available_models.len()
    }

    /// Find a model by its model_id.
    pub fn find_model(&self, model_id: &str) -> Option<&ModelConfig> {
        self.available_models
            .iter()
            .find(|m| m.model_id == model_id)
    }

    /// Get the recommended model for a given task type string.
    ///
    /// Prefers models that are already loaded. Among unloaded candidates,
    /// picks the first whose `recommended_tasks` includes the given type
    /// and whose RAM requirement is satisfiable.
    pub fn recommend_for_task(&self, task_type: &str) -> Option<&ModelConfig> {
        // Prefer an already-loaded model that supports this task.
        for (model_id, loaded) in &self.loaded_models {
            if loaded
                .config
                .recommended_tasks
                .iter()
                .any(|t| t == task_type)
            {
                return self.find_model(model_id);
            }
        }
        // Fall back to any discovered model that supports it and can be loaded.
        self.available_models
            .iter()
            .find(|m| m.recommended_tasks.iter().any(|t| t == task_type) && Self::can_load(m))
    }

    /// Check if the system has enough available RAM to load a model.
    ///
    /// On Linux, reads `/proc/meminfo` for the `MemAvailable` field.
    /// On other platforms, falls back to assuming 8 GB available.
    pub fn can_load(config: &ModelConfig) -> bool {
        let available_mb = Self::available_ram_mb();
        config.min_ram_mb <= available_mb
    }

    /// Query available system RAM in megabytes.
    ///
    /// Reads `/proc/meminfo` on Linux. Falls back to 8 GB on other platforms.
    pub fn available_ram_mb() -> usize {
        Self::read_available_ram_mb().unwrap_or(8 * 1024)
    }

    fn read_available_ram_mb() -> Option<usize> {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                let kb_str = rest.trim().trim_end_matches("kB").trim();
                let kb: usize = kb_str.parse().ok()?;
                return Some(kb / 1024);
            }
        }
        None
    }

    /// Load a model's weights and tokenizer into memory.
    ///
    /// Requires the `local-slm` feature. Without it, returns an error.
    /// The model must have been discovered first via `discover()`.
    ///
    /// Loads safetensors weights onto CPU device by default.
    /// Returns an `Arc<LoadedModel>` for thread-safe sharing.
    #[cfg(feature = "local-slm")]
    pub fn load(&mut self, model_id: &str) -> Result<Arc<LoadedModel>, String> {
        // Already loaded?
        if let Some(loaded) = self.loaded_models.get(model_id) {
            return Ok(Arc::clone(loaded));
        }

        let config = self
            .find_model(model_id)
            .ok_or_else(|| format!("model '{model_id}' not found in registry"))?
            .clone();

        if !Self::can_load(&config) {
            return Err(format!(
                "insufficient RAM to load '{}': requires {}MB, available {}MB",
                model_id,
                config.min_ram_mb,
                Self::available_ram_mb()
            ));
        }

        let device = Device::Cpu;

        // Load tokenizer
        let tokenizer_path = config.model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            format!(
                "failed to load tokenizer from '{}': {e}",
                tokenizer_path.display()
            )
        })?;

        // Load safetensors weights
        let weights_path = config.model_path.join("model.safetensors");
        let weights_bytes = std::fs::read(&weights_path).map_err(|e| {
            format!(
                "failed to read weights from '{}': {e}",
                weights_path.display()
            )
        })?;
        let weights_data =
            safetensors::tensor::SafeTensors::deserialize(&weights_bytes).map_err(|e| {
                format!(
                    "failed to deserialize safetensors from '{}': {e}",
                    weights_path.display()
                )
            })?;

        let dtype = config.quantization.to_dtype();
        let mut tensors = Vec::new();
        for (name, view) in weights_data.tensors() {
            let tensor = Tensor::from_raw_buffer(
                view.data(),
                view.dtype().try_into().map_err(|e: candle_core::Error| {
                    format!("unsupported dtype for tensor '{name}': {e}")
                })?,
                view.shape(),
                &device,
            )
            .map_err(|e| format!("failed to load tensor '{name}': {e}"))?;

            // Cast to target dtype if needed
            let tensor = if tensor.dtype() != dtype {
                tensor
                    .to_dtype(dtype)
                    .map_err(|e| format!("failed to cast tensor '{name}' to {dtype:?}: {e}"))?
            } else {
                tensor
            };

            tensors.push((name.to_string(), tensor));
        }

        let loaded = Arc::new(LoadedModel {
            config,
            weights: Arc::new(tensors),
            tokenizer: Arc::new(tokenizer),
            device,
        });

        self.loaded_models
            .insert(model_id.to_string(), Arc::clone(&loaded));
        Ok(loaded)
    }

    /// Load a model (stub when `local-slm` feature is disabled).
    #[cfg(not(feature = "local-slm"))]
    pub fn load(&mut self, model_id: &str) -> Result<Arc<LoadedModel>, String> {
        let config = self
            .find_model(model_id)
            .ok_or_else(|| format!("model '{model_id}' not found in registry"))?
            .clone();

        if !Self::can_load(&config) {
            return Err(format!(
                "insufficient RAM to load '{}': requires {}MB, available {}MB",
                model_id,
                config.min_ram_mb,
                Self::available_ram_mb()
            ));
        }

        Err(format!(
            "cannot load model '{model_id}': compile with `local-slm` feature to enable candle inference"
        ))
    }

    /// Unload a model, freeing its memory.
    ///
    /// Returns `true` if the model was loaded and is now unloaded,
    /// `false` if it was not loaded.
    pub fn unload(&mut self, model_id: &str) -> bool {
        self.loaded_models.remove(model_id).is_some()
    }

    /// Unload all loaded models.
    pub fn unload_all(&mut self) {
        self.loaded_models.clear();
    }

    /// Get a reference to a loaded model by its model_id.
    pub fn get_loaded(&self, model_id: &str) -> Option<Arc<LoadedModel>> {
        self.loaded_models.get(model_id).cloned()
    }

    /// List all currently loaded model IDs.
    pub fn loaded_model_ids(&self) -> Vec<String> {
        self.loaded_models.keys().cloned().collect()
    }

    /// Whether any model is currently loaded.
    pub fn has_loaded_models(&self) -> bool {
        !self.loaded_models.is_empty()
    }

    /// Number of models currently loaded.
    pub fn loaded_count(&self) -> usize {
        self.loaded_models.len()
    }

    /// Parse a `nexus-model.toml` into a `ModelConfig`.
    ///
    /// Expected format:
    /// ```toml
    /// model_id = "microsoft/phi-4"
    /// quantization = "Q4"
    /// max_context_length = 4096
    /// recommended_tasks = ["pii_detection", "prompt_safety"]
    /// min_ram_mb = 2048
    /// ```
    fn parse_model_toml(content: &str, model_path: &std::path::Path) -> Option<ModelConfig> {
        // Minimal TOML parsing without adding a toml dep — key = value lines.
        let mut model_id = None;
        let mut quantization = Quantization::Q4;
        let mut max_context_length = 4096usize;
        let mut recommended_tasks = Vec::new();
        let mut min_ram_mb = 2048usize;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match key {
                    "model_id" => model_id = Some(value.to_string()),
                    "quantization" => {
                        quantization = match value {
                            "Q4" => Quantization::Q4,
                            "Q8" => Quantization::Q8,
                            "F16" => Quantization::F16,
                            "F32" => Quantization::F32,
                            _ => Quantization::Q4,
                        };
                    }
                    "max_context_length" => {
                        max_context_length = value.parse().unwrap_or(4096);
                    }
                    "min_ram_mb" => {
                        min_ram_mb = value.parse().unwrap_or(2048);
                    }
                    "recommended_tasks" => {
                        // Parse simple array: ["a", "b", "c"]
                        let inner = value.trim_start_matches('[').trim_end_matches(']');
                        recommended_tasks = inner
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    _ => {}
                }
            }
        }

        let model_id = model_id?;
        Some(ModelConfig {
            model_id,
            model_path: model_path.to_path_buf(),
            quantization,
            max_context_length,
            recommended_tasks,
            min_ram_mb,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("nexus_model_registry_tests_{}", std::process::id()))
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_model_toml(model_dir: &std::path::Path, toml: &str) {
        fs::create_dir_all(model_dir).unwrap();
        fs::write(model_dir.join("nexus-model.toml"), toml).unwrap();
    }

    #[test]
    fn parse_model_toml_basic() {
        let toml = r#"
model_id = "microsoft/phi-4"
quantization = "Q4"
max_context_length = 4096
recommended_tasks = ["pii_detection", "prompt_safety"]
min_ram_mb = 2048
"#;
        let config =
            ModelRegistry::parse_model_toml(toml, &PathBuf::from("/tmp/models/phi-4")).unwrap();
        assert_eq!(config.model_id, "microsoft/phi-4");
        assert_eq!(config.quantization, Quantization::Q4);
        assert_eq!(config.max_context_length, 4096);
        assert_eq!(
            config.recommended_tasks,
            vec!["pii_detection", "prompt_safety"]
        );
        assert_eq!(config.min_ram_mb, 2048);
    }

    #[test]
    fn parse_model_toml_missing_id_returns_none() {
        let toml = r#"
quantization = "Q8"
max_context_length = 2048
"#;
        assert!(ModelRegistry::parse_model_toml(toml, &PathBuf::from("/tmp")).is_none());
    }

    #[test]
    fn parse_model_toml_f32_quantization() {
        let toml = r#"model_id = "test/model"
quantization = "F32"
"#;
        let config = ModelRegistry::parse_model_toml(toml, &PathBuf::from("/tmp")).unwrap();
        assert_eq!(config.quantization, Quantization::F32);
    }

    #[test]
    fn discover_empty_directory() {
        let dir = make_test_dir("discover_empty");
        let mut registry = ModelRegistry::new(dir.clone());
        assert_eq!(registry.discover(), 0);
        assert!(registry.available_models().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_finds_model_with_toml() {
        let dir = make_test_dir("discover_toml");
        write_model_toml(
            &dir.join("phi-4-q4"),
            "model_id = \"microsoft/phi-4\"\nquantization = \"Q4\"\nrecommended_tasks = [\"pii_detection\"]\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        assert_eq!(registry.discover(), 1);
        assert_eq!(registry.available_models()[0].model_id, "microsoft/phi-4");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_model_by_id() {
        let dir = make_test_dir("find_model");
        write_model_toml(
            &dir.join("test-model"),
            "model_id = \"test/model\"\nquantization = \"Q8\"\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();
        assert!(registry.find_model("test/model").is_some());
        assert!(registry.find_model("nonexistent").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recommend_for_task_finds_match() {
        let dir = make_test_dir("recommend_task");
        write_model_toml(
            &dir.join("safety-model"),
            "model_id = \"safety/v1\"\nrecommended_tasks = [\"prompt_safety\", \"content_classification\"]\nmin_ram_mb = 512\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();
        assert!(registry.recommend_for_task("prompt_safety").is_some());
        assert!(registry.recommend_for_task("unknown_task").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn can_load_checks_ram() {
        let config = ModelConfig {
            model_id: "test".into(),
            model_path: PathBuf::from("/tmp"),
            quantization: Quantization::Q4,
            max_context_length: 4096,
            recommended_tasks: vec![],
            min_ram_mb: 512,
        };
        assert!(ModelRegistry::can_load(&config));

        let huge = ModelConfig {
            min_ram_mb: 999_999,
            ..config
        };
        assert!(!ModelRegistry::can_load(&huge));
    }

    #[test]
    fn quantization_display() {
        assert_eq!(format!("{}", Quantization::Q4), "Q4");
        assert_eq!(format!("{}", Quantization::Q8), "Q8");
        assert_eq!(format!("{}", Quantization::F16), "F16");
        assert_eq!(format!("{}", Quantization::F32), "F32");
    }

    #[test]
    fn nonexistent_directory_returns_zero() {
        let mut registry = ModelRegistry::new(PathBuf::from("/tmp/nexus_nonexistent_dir_12345"));
        assert_eq!(registry.discover(), 0);
    }

    #[test]
    fn default_dir_uses_home() {
        let registry = ModelRegistry::default_dir();
        let path_str = registry.models_dir().to_string_lossy();
        assert!(path_str.contains(".nexus") && path_str.contains("models"));
    }

    #[test]
    fn available_ram_mb_returns_positive() {
        let ram = ModelRegistry::available_ram_mb();
        assert!(ram > 0);
    }

    // -----------------------------------------------------------------------
    // Load / unload tests (without local-slm feature, tests stub behavior)
    // -----------------------------------------------------------------------

    #[test]
    fn load_nonexistent_model_fails() {
        let dir = make_test_dir("load_nonexistent");
        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        let result = registry.load("nonexistent/model");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_model_insufficient_ram_fails() {
        let dir = make_test_dir("load_no_ram");
        write_model_toml(
            &dir.join("huge-model"),
            "model_id = \"huge/model\"\nmin_ram_mb = 999999\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        let result = registry.load("huge/model");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient RAM"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(not(feature = "local-slm"))]
    #[test]
    fn load_without_feature_returns_feature_error() {
        let dir = make_test_dir("load_no_feature");
        write_model_toml(
            &dir.join("small-model"),
            "model_id = \"small/model\"\nmin_ram_mb = 1\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        let result = registry.load("small/model");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("local-slm"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unload_returns_false_if_not_loaded() {
        let dir = make_test_dir("unload_not_loaded");
        let mut registry = ModelRegistry::new(dir.clone());
        assert!(!registry.unload("anything"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unload_all_clears_loaded() {
        let dir = make_test_dir("unload_all");
        let mut registry = ModelRegistry::new(dir.clone());
        assert!(!registry.has_loaded_models());
        registry.unload_all();
        assert_eq!(registry.loaded_count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loaded_model_ids_empty_by_default() {
        let dir = make_test_dir("loaded_ids_empty");
        let registry = ModelRegistry::new(dir.clone());
        assert!(registry.loaded_model_ids().is_empty());
        assert!(!registry.has_loaded_models());
        assert_eq!(registry.loaded_count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_loaded_returns_none_when_not_loaded() {
        let dir = make_test_dir("get_loaded_none");
        let registry = ModelRegistry::new(dir.clone());
        assert!(registry.get_loaded("some/model").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_skips_files_not_dirs() {
        let dir = make_test_dir("discover_skip_files");
        // Create a regular file (not a directory) at top level
        fs::write(dir.join("not-a-dir.txt"), "just a file").unwrap();

        let mut registry = ModelRegistry::new(dir.clone());
        assert_eq!(registry.discover(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_skips_dirs_without_toml() {
        let dir = make_test_dir("discover_no_toml");
        fs::create_dir_all(dir.join("empty-model-dir")).unwrap();

        let mut registry = ModelRegistry::new(dir.clone());
        assert_eq!(registry.discover(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_multiple_models() {
        let dir = make_test_dir("discover_multi");
        write_model_toml(
            &dir.join("model-a"),
            "model_id = \"vendor/model-a\"\nmin_ram_mb = 100\nrecommended_tasks = [\"pii_detection\"]\n",
        );
        write_model_toml(
            &dir.join("model-b"),
            "model_id = \"vendor/model-b\"\nmin_ram_mb = 200\nrecommended_tasks = [\"prompt_safety\"]\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        let count = registry.discover();
        assert_eq!(count, 2);
        assert!(registry.find_model("vendor/model-a").is_some());
        assert!(registry.find_model("vendor/model-b").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recommend_prefers_loaded_model() {
        // Without the local-slm feature we can't actually load, but we
        // can test the fallback path: recommend_for_task finds unloaded
        // models that support the task and pass can_load.
        let dir = make_test_dir("recommend_prefer");
        write_model_toml(
            &dir.join("pii-model"),
            "model_id = \"pii/v1\"\nmin_ram_mb = 100\nrecommended_tasks = [\"pii_detection\"]\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        let rec = registry.recommend_for_task("pii_detection");
        assert!(rec.is_some());
        assert_eq!(rec.unwrap().model_id, "pii/v1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recommend_skips_model_too_large_to_load() {
        let dir = make_test_dir("recommend_skip_large");
        write_model_toml(
            &dir.join("huge-pii"),
            "model_id = \"huge/pii\"\nmin_ram_mb = 999999\nrecommended_tasks = [\"pii_detection\"]\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        // The model supports pii_detection but is too large to load
        let rec = registry.recommend_for_task("pii_detection");
        assert!(rec.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clone_registry_shares_state() {
        let dir = make_test_dir("clone_registry");
        write_model_toml(
            &dir.join("clone-model"),
            "model_id = \"clone/test\"\nmin_ram_mb = 100\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        registry.discover();

        let cloned = registry.clone();
        assert_eq!(cloned.available_models().len(), 1);
        assert_eq!(cloned.available_models()[0].model_id, "clone/test");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn re_discover_clears_old_models() {
        let dir = make_test_dir("rediscover");
        write_model_toml(
            &dir.join("model-1"),
            "model_id = \"first/model\"\nmin_ram_mb = 100\n",
        );

        let mut registry = ModelRegistry::new(dir.clone());
        assert_eq!(registry.discover(), 1);

        // Remove old, add new
        fs::remove_dir_all(dir.join("model-1")).unwrap();
        write_model_toml(
            &dir.join("model-2"),
            "model_id = \"second/model\"\nmin_ram_mb = 100\n",
        );

        assert_eq!(registry.discover(), 1);
        assert!(registry.find_model("first/model").is_none());
        assert!(registry.find_model("second/model").is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
