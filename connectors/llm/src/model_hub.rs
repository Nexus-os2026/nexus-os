//! HuggingFace model hub integration — search, download, and compatibility checking.
//!
//! Uses `curl` via `std::process::Command` for HTTP requests (no extra deps).
//! Model files are downloaded to `~/.nexus/models/{name}/` with a generated
//! `nexus-model.toml` metadata file for discovery by `ModelRegistry`.

use crate::model_registry::{ModelConfig, Quantization};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

// ─── HuggingFace API types ───────────────────────────────────────────────────

/// Information about a model from HuggingFace Hub.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HfModelInfo {
    pub model_id: String,
    pub author: String,
    pub name: String,
    pub description: String,
    pub downloads: u64,
    pub likes: u64,
    pub tags: Vec<String>,
    pub last_modified: String,
    pub files: Vec<HfModelFile>,
}

/// A downloadable file within a HuggingFace model repository.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HfModelFile {
    pub filename: String,
    pub size_bytes: u64,
    pub quantization: Option<String>,
}

/// Result of searching HuggingFace Hub.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelSearchResult {
    pub models: Vec<HfModelInfo>,
    pub total_count: usize,
    pub query: String,
}

// ─── Download types ──────────────────────────────────────────────────────────

/// Progress update during a model file download.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub status: DownloadStatus,
}

/// Status of a download operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Completed,
    Failed(String),
}

// ─── System compatibility types ──────────────────────────────────────────────

/// System compatibility assessment for running a model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemCompatibility {
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub can_run: bool,
    pub recommended_quantization: String,
    pub warning: Option<String>,
}

// ─── HTTP helper ─────────────────────────────────────────────────────────────

/// Perform an HTTP GET request using curl and return the response body.
fn http_get(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args(["-sS", "-L", "-m", "30"])
        .arg(url)
        .output()
        .map_err(|e| format!("curl execution failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl request failed: {stderr}"));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("response not utf-8: {e}"))
}

// ─── Quantization parsing ────────────────────────────────────────────────────

/// Extract a quantization tag from a filename (e.g. "Q4_K_M" from "llama-7b.Q4_K_M.gguf").
pub fn parse_quantization_from_filename(filename: &str) -> Option<String> {
    let upper = filename.to_uppercase();

    // Check for common quantization patterns in order of specificity.
    let patterns = [
        "Q2_K_S", "Q2_K_M", "Q2_K_L", "Q2_K", "Q3_K_S", "Q3_K_M", "Q3_K_L", "Q3_K", "Q4_K_S",
        "Q4_K_M", "Q4_K_L", "Q4_K", "Q4_0", "Q4_1", "Q5_K_S", "Q5_K_M", "Q5_K_L", "Q5_K", "Q5_0",
        "Q5_1", "Q6_K", "Q8_0", "Q8_1", "F16", "F32", "IQ1_S", "IQ1_M", "IQ2_XXS", "IQ2_XS",
        "IQ2_S", "IQ2_M", "IQ3_XXS", "IQ3_XS", "IQ3_S", "IQ4_XS", "IQ4_NL",
    ];

    for pattern in &patterns {
        if upper.contains(pattern) {
            return Some(pattern.to_string());
        }
    }

    None
}

/// Map a quantization string to the `Quantization` enum.
fn quantization_from_tag(tag: &str) -> Quantization {
    let upper = tag.to_uppercase();
    if upper.starts_with("Q2")
        || upper.starts_with("Q3")
        || upper.starts_with("Q4")
        || upper.starts_with("IQ")
    {
        Quantization::Q4
    } else if upper.starts_with("Q5") || upper.starts_with("Q6") || upper.starts_with("Q8") {
        Quantization::Q8
    } else if upper.contains("F16") {
        Quantization::F16
    } else if upper.contains("F32") {
        Quantization::F32
    } else {
        Quantization::Q4
    }
}

// ─── JSON parsing helpers ────────────────────────────────────────────────────

/// Parse a HuggingFace API model list JSON response into `Vec<HfModelInfo>`.
pub fn parse_hf_model_list(json_str: &str) -> Result<Vec<HfModelInfo>, String> {
    let array: Vec<serde_json::Value> =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    let mut models = Vec::new();
    for obj in &array {
        if let Some(info) = parse_hf_model_object(obj) {
            models.push(info);
        }
    }
    Ok(models)
}

/// Parse a single HuggingFace API model JSON object into `HfModelInfo`.
pub fn parse_hf_model_object(obj: &serde_json::Value) -> Option<HfModelInfo> {
    let model_id = obj.get("modelId")?.as_str()?.to_string();
    let author = obj
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let downloads = obj.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);
    let likes = obj.get("likes").and_then(|v| v.as_u64()).unwrap_or(0);
    let last_modified = obj
        .get("lastModified")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tags: Vec<String> = obj
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Parse files from the "siblings" array.
    let files: Vec<HfModelFile> = obj
        .get("siblings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let fname = s.get("rfilename")?.as_str()?.to_string();
                    let size = s.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    // Only include GGUF files.
                    if !fname.to_lowercase().ends_with(".gguf") {
                        return None;
                    }
                    let quantization = parse_quantization_from_filename(&fname);
                    Some(HfModelFile {
                        filename: fname,
                        size_bytes: size,
                        quantization,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Use the part after '/' as the display name.
    let name = model_id
        .split('/')
        .next_back()
        .unwrap_or(&model_id)
        .to_string();

    // Description from tags since the list endpoint doesn't include model card text.
    let description = if tags.is_empty() {
        String::new()
    } else {
        tags.iter().take(10).cloned().collect::<Vec<_>>().join(", ")
    };

    Some(HfModelInfo {
        model_id,
        author,
        name,
        description,
        downloads,
        likes,
        tags,
        last_modified,
        files,
    })
}

// ─── HuggingFace API functions ───────────────────────────────────────────────

/// Search HuggingFace Hub for GGUF models.
pub fn search_huggingface(query: &str, limit: usize) -> Result<ModelSearchResult, String> {
    let encoded_query = query.replace(' ', "+");
    let url = format!(
        "https://huggingface.co/api/models?search={}&filter=gguf&sort=downloads&direction=-1&limit={}",
        encoded_query, limit
    );

    let body = http_get(&url)?;
    let models = parse_hf_model_list(&body)?;
    let total_count = models.len();

    Ok(ModelSearchResult {
        models,
        total_count,
        query: query.to_string(),
    })
}

/// Fetch detailed information about a specific model from HuggingFace Hub.
pub fn get_model_details(model_id: &str) -> Result<HfModelInfo, String> {
    let url = format!("https://huggingface.co/api/models/{model_id}");
    let body = http_get(&url)?;

    let obj: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

    // For the detail endpoint, try to extract description from the model card.
    let mut info = parse_hf_model_object(&obj)
        .ok_or_else(|| format!("failed to parse model details for '{model_id}'"))?;

    // The detail endpoint sometimes includes cardData.description or a text field.
    if let Some(card) = obj.get("cardData") {
        if let Some(desc) = card.get("description").and_then(|v| v.as_str()) {
            let truncated: String = desc.chars().take(200).collect();
            info.description = truncated;
        }
    }

    Ok(info)
}

// ─── Download functions ──────────────────────────────────────────────────────

/// Download a model file from HuggingFace Hub with progress updates.
///
/// Downloads to `{target_dir}/{sanitized_model_name}/{filename}`.
/// Calls `progress_callback` with status updates during the download.
/// Returns the full path of the downloaded file on success.
pub fn download_model_file(
    model_id: &str,
    filename: &str,
    target_dir: &str,
    progress_callback: impl Fn(DownloadProgress),
) -> Result<String, String> {
    // Build the download URL.
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        model_id, filename
    );

    // Create the target directory.
    let model_name = model_id.replace('/', "__");
    let model_dir = PathBuf::from(target_dir).join(&model_name);
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| format!("failed to create model directory: {e}"))?;

    let file_path = model_dir.join(filename);
    let file_path_str = file_path.to_string_lossy().to_string();

    // Emit starting status.
    progress_callback(DownloadProgress {
        model_id: model_id.to_string(),
        filename: filename.to_string(),
        bytes_downloaded: 0,
        total_bytes: 0,
        percent: 0.0,
        status: DownloadStatus::Starting,
    });

    // First, get the file size via a HEAD request.
    let total_bytes = get_content_length(&url).unwrap_or(0);

    // Start curl download in the background.
    let mut child = Command::new("curl")
        .args(["-sS", "-L", "-o"])
        .arg(&file_path_str)
        .arg(&url)
        .spawn()
        .map_err(|e| format!("curl spawn failed: {e}"))?;

    // Monitor file size growth while curl runs.
    let poll_interval = std::time::Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                if exit_status.success() && file_path.exists() {
                    let final_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                    progress_callback(DownloadProgress {
                        model_id: model_id.to_string(),
                        filename: filename.to_string(),
                        bytes_downloaded: final_size,
                        total_bytes: if total_bytes > 0 {
                            total_bytes
                        } else {
                            final_size
                        },
                        percent: 100.0,
                        status: DownloadStatus::Completed,
                    });
                    return Ok(file_path_str);
                } else {
                    // Clean up partial file.
                    // Best-effort: clean up partial download on curl failure
                    let _ = std::fs::remove_file(&file_path);
                    return Err("download failed: curl exited with error".to_string());
                }
            }
            Ok(None) => {
                // Still running — emit progress.
                let current_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                let percent = if total_bytes > 0 {
                    (current_size as f32 / total_bytes as f32 * 100.0).min(99.9)
                } else {
                    0.0
                };
                progress_callback(DownloadProgress {
                    model_id: model_id.to_string(),
                    filename: filename.to_string(),
                    bytes_downloaded: current_size,
                    total_bytes,
                    percent,
                    status: DownloadStatus::Downloading,
                });
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                // Best-effort: clean up partial download on wait error
                let _ = std::fs::remove_file(&file_path);
                return Err(format!("error waiting for curl: {e}"));
            }
        }
    }
}

/// Get Content-Length of a URL via a HEAD request.
fn get_content_length(url: &str) -> Option<u64> {
    let output = Command::new("curl")
        .args(["-sS", "-L", "-I", "-m", "10"])
        .arg(url)
        .output()
        // Optional: curl may not be installed or HEAD request may fail
        .ok()?;

    let headers = String::from_utf8_lossy(&output.stdout);
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            // Optional: header value may not be a valid u64
            return rest.trim().parse().ok();
        }
    }
    None
}

/// Generate a `nexus-model.toml` config file after downloading a model.
///
/// Writes the TOML to `{model_dir}/nexus-model.toml` and returns the parsed config.
pub fn generate_model_config(
    model_id: &str,
    filename: &str,
    model_dir: &str,
) -> Result<ModelConfig, String> {
    let dir = PathBuf::from(model_dir);

    // Determine file size for RAM estimate.
    let file_path = dir.join(filename);
    let file_size_bytes = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
    let file_size_mb = (file_size_bytes / (1024 * 1024)) as usize;

    // Parse quantization from filename.
    let quant_tag = parse_quantization_from_filename(filename);
    let quantization = quant_tag
        .as_deref()
        .map(quantization_from_tag)
        .unwrap_or(Quantization::Q4);

    // Estimate minimum RAM: file size * multiplier based on quantization.
    let ram_multiplier: f64 = match quantization {
        Quantization::Q4 => 1.2,
        Quantization::Q8 => 1.1,
        Quantization::F16 => 1.05,
        Quantization::F32 => 1.05,
    };
    let min_ram_mb = ((file_size_mb as f64) * ram_multiplier).ceil() as usize;

    let config = ModelConfig {
        model_id: model_id.to_string(),
        model_path: dir.clone(),
        quantization,
        max_context_length: 4096,
        recommended_tasks: vec!["general".to_string()],
        min_ram_mb,
    };

    // Write nexus-model.toml.
    let toml_content = format!(
        r#"model_id = "{}"
quantization = "{}"
max_context_length = {}
recommended_tasks = ["general"]
min_ram_mb = {}
"#,
        model_id, quantization, config.max_context_length, min_ram_mb
    );

    std::fs::write(dir.join("nexus-model.toml"), toml_content)
        .map_err(|e| format!("failed to write nexus-model.toml: {e}"))?;

    Ok(config)
}

// ─── System compatibility ────────────────────────────────────────────────────

/// Check system compatibility for running a model of the given file size.
pub fn check_compatibility(model_file_size_bytes: u64) -> SystemCompatibility {
    let total_ram_mb = read_total_ram_mb().unwrap_or(8 * 1024) as u64;
    let available_ram_mb = read_available_ram_mb().unwrap_or(8 * 1024) as u64;
    let file_size_mb = model_file_size_bytes / (1024 * 1024);

    let threshold_comfortable = (file_size_mb as f64 * 1.5) as u64;
    let threshold_tight = (file_size_mb as f64 * 1.1) as u64;

    let (can_run, warning) = if available_ram_mb >= threshold_comfortable {
        (true, None)
    } else if available_ram_mb >= threshold_tight {
        (
            true,
            Some(format!(
                "Model may be slow with only {}GB available RAM",
                available_ram_mb / 1024
            )),
        )
    } else {
        (
            false,
            Some("Insufficient RAM — try a smaller quantization".to_string()),
        )
    };

    let recommended_quantization = recommend_quantization(total_ram_mb);

    SystemCompatibility {
        total_ram_mb,
        available_ram_mb,
        can_run,
        recommended_quantization,
        warning,
    }
}

/// Check compatibility with explicit RAM values (for testing).
pub fn check_compatibility_with_ram(
    model_file_size_bytes: u64,
    total_ram_mb: u64,
    available_ram_mb: u64,
) -> SystemCompatibility {
    let file_size_mb = model_file_size_bytes / (1024 * 1024);

    let threshold_comfortable = (file_size_mb as f64 * 1.5) as u64;
    let threshold_tight = (file_size_mb as f64 * 1.1) as u64;

    let (can_run, warning) = if available_ram_mb >= threshold_comfortable {
        (true, None)
    } else if available_ram_mb >= threshold_tight {
        (
            true,
            Some(format!(
                "Model may be slow with only {}GB available RAM",
                available_ram_mb / 1024
            )),
        )
    } else {
        (
            false,
            Some("Insufficient RAM — try a smaller quantization".to_string()),
        )
    };

    let recommended_quantization = recommend_quantization(total_ram_mb);

    SystemCompatibility {
        total_ram_mb,
        available_ram_mb,
        can_run,
        recommended_quantization,
        warning,
    }
}

/// Recommend a quantization level based on total system RAM.
fn recommend_quantization(total_ram_mb: u64) -> String {
    if total_ram_mb < 8 * 1024 {
        "Q4_K_S".to_string()
    } else if total_ram_mb < 16 * 1024 {
        "Q4_K_M".to_string()
    } else if total_ram_mb < 32 * 1024 {
        "Q5_K_M".to_string()
    } else {
        "F16".to_string()
    }
}

/// Read total system RAM in megabytes from `/proc/meminfo`.
fn read_total_ram_mb() -> Option<usize> {
    // Optional: /proc/meminfo not available on non-Linux platforms
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb_str = rest.trim().trim_end_matches("kB").trim();
            // Optional: parse failure means malformed meminfo line
            let kb: usize = kb_str.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Read available system RAM in megabytes from `/proc/meminfo`.
fn read_available_ram_mb() -> Option<usize> {
    // Optional: /proc/meminfo not available on non-Linux platforms
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb_str = rest.trim().trim_end_matches("kB").trim();
            // Optional: parse failure means malformed meminfo line
            let kb: usize = kb_str.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

// ─── Ollama registration ──────────────────────────────────────────────────

/// Register a downloaded GGUF model with Ollama so it appears in model lists.
///
/// Creates a Modelfile pointing at the downloaded GGUF and calls `POST /api/create`
/// on the local Ollama server. This bridges ModelHub downloads to Chat.
pub fn register_downloaded_model_with_ollama(
    model_path: &std::path::Path,
    model_name: &str,
) -> Result<(), String> {
    let modelfile_content = format!(
        "FROM {}\n\nPARAMETER temperature 0.7\nPARAMETER top_p 0.9\n",
        model_path.display()
    );

    // Write Modelfile next to the model
    let modelfile_path = model_path.with_extension("Modelfile");
    std::fs::write(&modelfile_path, &modelfile_content)
        .map_err(|e| format!("failed to write Modelfile: {e}"))?;

    // Sanitize model name for Ollama (lowercase, no special chars)
    let ollama_name = model_name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.' || *c == ':')
        .collect::<String>();

    // Call Ollama API to create the model
    let payload = serde_json::json!({
        "name": ollama_name,
        "modelfile": modelfile_content,
    });

    let result = Command::new("curl")
        .args([
            "-sS",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload.to_string(),
            "http://localhost:11434/api/create",
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Ollama registration failed: {stderr}"))
        }
        Err(_) => {
            // Ollama not running — not an error, model is still saved locally
            Ok(())
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Quantization parsing ──

    #[test]
    fn test_parse_quantization_from_filename() {
        assert_eq!(
            parse_quantization_from_filename("llama-2-7b.Q4_K_M.gguf"),
            Some("Q4_K_M".to_string())
        );
    }

    #[test]
    fn test_parse_quantization_q8() {
        assert_eq!(
            parse_quantization_from_filename("model.Q8_0.gguf"),
            Some("Q8_0".to_string())
        );
    }

    #[test]
    fn test_parse_quantization_f16() {
        assert_eq!(
            parse_quantization_from_filename("model-f16.gguf"),
            Some("F16".to_string())
        );
    }

    #[test]
    fn test_parse_quantization_q5_k_s() {
        assert_eq!(
            parse_quantization_from_filename("phi-3-mini-Q5_K_S.gguf"),
            Some("Q5_K_S".to_string())
        );
    }

    #[test]
    fn test_parse_quantization_none() {
        assert_eq!(parse_quantization_from_filename("readme.md"), None);
    }

    #[test]
    fn test_parse_quantization_case_insensitive() {
        assert_eq!(
            parse_quantization_from_filename("model.q4_k_m.gguf"),
            Some("Q4_K_M".to_string())
        );
    }

    // ── Compatibility checks ──

    #[test]
    fn test_compatibility_high_ram() {
        // 32GB total, 30GB available, 2GB model
        let compat = check_compatibility_with_ram(
            2 * 1024 * 1024 * 1024, // 2GB file
            32 * 1024,              // 32GB total
            30 * 1024,              // 30GB available
        );
        assert!(compat.can_run);
        assert!(compat.warning.is_none());
    }

    #[test]
    fn test_compatibility_low_ram() {
        // 4GB total, 3GB available, 8GB model
        let compat = check_compatibility_with_ram(
            8u64 * 1024 * 1024 * 1024, // 8GB file
            4 * 1024,                  // 4GB total
            3 * 1024,                  // 3GB available
        );
        assert!(!compat.can_run);
        assert!(compat.warning.is_some());
    }

    #[test]
    fn test_compatibility_tight_ram() {
        // 8GB total, 7GB available, 6GB model → can run but tight
        let compat = check_compatibility_with_ram(
            6u64 * 1024 * 1024 * 1024, // 6GB file
            8 * 1024,                  // 8GB total
            7 * 1024,                  // 7GB available
        );
        assert!(compat.can_run);
        assert!(compat.warning.is_some());
    }

    // ── Quantization recommendations ──

    #[test]
    fn test_recommended_quantization_low() {
        let q = recommend_quantization(6 * 1024); // 6GB
        assert_eq!(q, "Q4_K_S");
    }

    #[test]
    fn test_recommended_quantization_medium() {
        let q = recommend_quantization(16 * 1024); // 16GB
        assert_eq!(q, "Q5_K_M");
    }

    #[test]
    fn test_recommended_quantization_8gb() {
        let q = recommend_quantization(8 * 1024); // 8GB
        assert_eq!(q, "Q4_K_M");
    }

    #[test]
    fn test_recommended_quantization_high() {
        let q = recommend_quantization(64 * 1024); // 64GB
        assert_eq!(q, "F16");
    }

    // ── Generate model config ──

    #[test]
    fn test_generate_model_config() {
        let dir = std::env::temp_dir().join("nexus_model_hub_test_gen_config");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a fake model file.
        let fake_data = vec![0u8; 4 * 1024 * 1024]; // 4MB
        std::fs::write(dir.join("test-model.Q4_K_M.gguf"), &fake_data).unwrap();

        let config = generate_model_config(
            "test/model",
            "test-model.Q4_K_M.gguf",
            dir.to_str().unwrap(),
        )
        .expect("generate config");

        assert_eq!(config.model_id, "test/model");
        assert_eq!(config.quantization, Quantization::Q4);
        assert!(config.min_ram_mb > 0);
        assert_eq!(config.max_context_length, 4096);
        assert!(config.recommended_tasks.contains(&"general".to_string()));

        // Verify TOML was written.
        let toml_path = dir.join("nexus-model.toml");
        assert!(toml_path.exists());
        let toml_content = std::fs::read_to_string(toml_path).unwrap();
        assert!(toml_content.contains("test/model"));
        assert!(toml_content.contains("Q4"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── JSON parsing ──

    #[test]
    fn test_search_result_parsing() {
        let json = r#"[
            {
                "modelId": "TheBloke/Llama-2-7B-GGUF",
                "author": "TheBloke",
                "downloads": 500000,
                "likes": 1200,
                "tags": ["gguf", "llama", "7b"],
                "lastModified": "2024-01-15T10:00:00.000Z",
                "siblings": [
                    {"rfilename": "llama-2-7b.Q4_K_M.gguf", "size": 4370000000},
                    {"rfilename": "llama-2-7b.Q8_0.gguf", "size": 7160000000},
                    {"rfilename": "README.md", "size": 1024}
                ]
            },
            {
                "modelId": "second/model",
                "author": "someone",
                "downloads": 100,
                "likes": 5,
                "tags": ["gguf"],
                "lastModified": "2024-06-01T00:00:00.000Z",
                "siblings": []
            }
        ]"#;

        let models = parse_hf_model_list(json).expect("parse");
        assert_eq!(models.len(), 2);

        let m0 = &models[0];
        assert_eq!(m0.model_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(m0.author, "TheBloke");
        assert_eq!(m0.downloads, 500000);
        assert_eq!(m0.likes, 1200);
        assert_eq!(m0.name, "Llama-2-7B-GGUF");
        assert_eq!(m0.tags, vec!["gguf", "llama", "7b"]);

        // Only GGUF files should be parsed (README.md excluded).
        assert_eq!(m0.files.len(), 2);
        assert_eq!(m0.files[0].filename, "llama-2-7b.Q4_K_M.gguf");
        assert_eq!(m0.files[0].size_bytes, 4370000000);
        assert_eq!(m0.files[0].quantization, Some("Q4_K_M".to_string()));
        assert_eq!(m0.files[1].filename, "llama-2-7b.Q8_0.gguf");
        assert_eq!(m0.files[1].quantization, Some("Q8_0".to_string()));

        let m1 = &models[1];
        assert_eq!(m1.model_id, "second/model");
        assert!(m1.files.is_empty());
    }

    #[test]
    fn test_parse_single_model_object() {
        let json = r#"{
            "modelId": "user/test-model",
            "author": "user",
            "downloads": 42,
            "likes": 3,
            "tags": ["gguf", "test"],
            "lastModified": "2025-01-01T00:00:00.000Z",
            "siblings": [
                {"rfilename": "test.F16.gguf", "size": 1000000}
            ]
        }"#;

        let obj: serde_json::Value = serde_json::from_str(json).unwrap();
        let info = parse_hf_model_object(&obj).expect("parse model");
        assert_eq!(info.model_id, "user/test-model");
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].quantization, Some("F16".to_string()));
    }

    #[test]
    fn test_parse_empty_array() {
        let models = parse_hf_model_list("[]").expect("parse empty");
        assert!(models.is_empty());
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        assert!(parse_hf_model_list("not json").is_err());
    }

    #[test]
    fn test_quantization_from_tag_mapping() {
        assert_eq!(quantization_from_tag("Q4_K_M"), Quantization::Q4);
        assert_eq!(quantization_from_tag("Q8_0"), Quantization::Q8);
        assert_eq!(quantization_from_tag("F16"), Quantization::F16);
        assert_eq!(quantization_from_tag("F32"), Quantization::F32);
        assert_eq!(quantization_from_tag("Q5_K_S"), Quantization::Q8);
        assert_eq!(quantization_from_tag("Q3_K_M"), Quantization::Q4);
    }

    #[test]
    fn test_download_progress_serialization() {
        let progress = DownloadProgress {
            model_id: "test/model".to_string(),
            filename: "model.gguf".to_string(),
            bytes_downloaded: 1024,
            total_bytes: 2048,
            percent: 50.0,
            status: DownloadStatus::Downloading,
        };
        let json = serde_json::to_string(&progress).expect("serialize");
        assert!(json.contains("test/model"));
        assert!(json.contains("Downloading"));
    }

    #[test]
    fn test_system_compatibility_serialization() {
        let compat = SystemCompatibility {
            total_ram_mb: 16384,
            available_ram_mb: 12000,
            can_run: true,
            recommended_quantization: "Q4_K_M".to_string(),
            warning: None,
        };
        let json = serde_json::to_string(&compat).expect("serialize");
        assert!(json.contains("Q4_K_M"));
        assert!(json.contains("16384"));
    }
}
