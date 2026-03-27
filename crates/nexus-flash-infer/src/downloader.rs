//! Model download, storage, and management.
//!
//! Gated behind the `download` feature to keep the core crate network-free.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::FlashError;

// ── Types ──────────────────────────────────────────────────────────

/// Progress update during download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_name: String,
    pub file_index: u32,
    pub file_count: u32,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f64,
    pub speed_mb_per_sec: f64,
    pub eta_seconds: u64,
    pub status: DownloadStatus,
}

/// Status of a download operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Verifying,
    Complete,
    Failed(String),
    Cancelled,
}

/// A downloaded model on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModel {
    pub name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub file_size_display: String,
    pub quant_type: String,
    pub downloaded_at: String,
    pub sha256: Option<String>,
    pub verified: bool,
}

// ── ModelStorage ───────────────────────────────────────────────────

/// Manages local model file storage.
pub struct ModelStorage {
    base_dir: PathBuf,
    /// When true, `list_models` also scans well-known directories like
    /// `~/.nexus/models/` and `~/models/` in addition to `base_dir`.
    scan_extra_dirs: bool,
}

impl ModelStorage {
    /// Create storage, ensuring the base directory exists.
    /// Scans extra well-known directories when listing models.
    pub fn new() -> Result<Self, FlashError> {
        let base_dir = Self::default_model_dir()?;
        std::fs::create_dir_all(&base_dir)
            .map_err(|e| FlashError::DownloadError(format!("cannot create model dir: {e}")))?;
        Ok(Self {
            base_dir,
            scan_extra_dirs: true,
        })
    }

    /// Create storage at a specific path (for testing).
    /// Only scans the given directory — no extra dirs.
    pub fn with_dir(base_dir: PathBuf) -> Result<Self, FlashError> {
        std::fs::create_dir_all(&base_dir)
            .map_err(|e| FlashError::DownloadError(format!("cannot create model dir: {e}")))?;
        Ok(Self {
            base_dir,
            scan_extra_dirs: false,
        })
    }

    /// List all downloaded `.gguf` models.
    ///
    /// Scans the primary model directory **and** common user directories
    /// (`~/.nexus/models/`, `~/models/`) recursively so models inside
    /// sub-folders (e.g. `bartowski__gemma-2-2b-it-GGUF/`) are discovered.
    ///
    /// Split models (e.g. 4 shard files) are collapsed into a single entry
    /// pointing at the first shard (`-00001-of-`). The displayed size is
    /// the sum of all parts. llama.cpp auto-discovers remaining shards.
    pub fn list_models(&self) -> Result<Vec<LocalModel>, FlashError> {
        let mut models = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Collect all directories to scan.
        let mut scan_dirs: Vec<PathBuf> = vec![self.base_dir.clone()];

        // Also scan ~/.nexus/models/ and ~/models/ if they exist.
        if self.scan_extra_dirs {
            if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
                let home = PathBuf::from(home);
                for extra in &[home.join(".nexus").join("models"), home.join("models")] {
                    if extra.is_dir() && !scan_dirs.contains(extra) {
                        scan_dirs.push(extra.clone());
                    }
                }
            }
        }

        for dir in &scan_dirs {
            Self::scan_dir_recursive(dir, &mut models, &mut seen, 3);
        }

        // Newest first
        models.sort_by(|a, b| b.downloaded_at.cmp(&a.downloaded_at));
        Ok(models)
    }

    /// Recursively scan a directory for `.gguf` files up to `max_depth` levels.
    ///
    /// Split models (`-00002-of-`, `-00003-of-`, etc.) are skipped — only the
    /// first shard (`-00001-of-`) or non-split files are included. The file
    /// size for split models is the sum of all parts.
    fn scan_dir_recursive(
        dir: &Path,
        models: &mut Vec<LocalModel>,
        seen: &mut std::collections::HashSet<String>,
        max_depth: u8,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        // First pass: collect all gguf entries in this directory so we can
        // sum split-model sizes before emitting the first-shard entry.
        let mut gguf_entries: Vec<(PathBuf, std::fs::Metadata)> = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();

            if path.is_dir() && max_depth > 0 {
                Self::scan_dir_recursive(&path, models, seen, max_depth - 1);
                continue;
            }

            if path.extension().and_then(|e| e.to_str()) != Some("gguf") {
                continue;
            }
            // Skip partial downloads.
            let name_str = path.to_string_lossy().to_string();
            if name_str.ends_with(".part.gguf") {
                continue;
            }

            if let Ok(meta) = std::fs::metadata(&path) {
                gguf_entries.push((path, meta));
            }
        }

        // Build a map: split prefix → total size of all parts.
        // For "Model-Q4-00001-of-00004.gguf" the prefix is "Model-Q4".
        let mut split_totals: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        for (path, meta) in &gguf_entries {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if is_split_shard(&filename) {
                let prefix = split_prefix(&filename);
                *split_totals.entry(prefix).or_insert(0) += meta.len();
            }
        }

        // Second pass: emit models, skipping non-first split parts.
        for (path, meta) in gguf_entries {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Skip split shards that aren't the first part.
            if is_split_shard(&filename) && !is_first_shard(&filename) {
                continue;
            }

            // Deduplicate by absolute path.
            let canonical = path
                .canonicalize()
                .unwrap_or_else(|_| path.clone())
                .to_string_lossy()
                .to_string();
            if !seen.insert(canonical) {
                continue;
            }

            // For split models, use the combined size of all parts.
            let total_size = if is_first_shard(&filename) {
                let prefix = split_prefix(&filename);
                split_totals.get(&prefix).copied().unwrap_or(meta.len())
            } else {
                meta.len()
            };

            models.push(LocalModel {
                name: filename.clone(),
                file_path: path.to_string_lossy().to_string(),
                file_size_bytes: total_size,
                file_size_display: format_bytes(total_size),
                quant_type: extract_quant_from_filename(&filename),
                downloaded_at: file_modified_iso(&meta),
                sha256: None,
                verified: false,
            });
        }
    }

    /// Path where a model would be stored.
    pub fn model_path(&self, filename: &str) -> PathBuf {
        self.base_dir.join(filename)
    }

    /// The base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Delete a downloaded model and any `.part` file.
    pub fn delete_model(&self, filename: &str) -> Result<(), FlashError> {
        let path = self.model_path(filename);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| FlashError::DownloadError(format!("delete failed: {e}")))?;
        }
        let part = self.base_dir.join(format!("{filename}.part"));
        if part.exists() {
            let _ = std::fs::remove_file(&part);
        }
        Ok(())
    }

    /// Available disk space in bytes on the volume hosting the model dir.
    pub fn available_disk_space(&self) -> Result<u64, FlashError> {
        Ok(available_space_bytes(&self.base_dir))
    }

    /// Total size of all downloaded models in bytes.
    pub fn total_models_size(&self) -> Result<u64, FlashError> {
        let models = self.list_models()?;
        Ok(models.iter().map(|m| m.file_size_bytes).sum())
    }

    fn default_model_dir() -> Result<PathBuf, FlashError> {
        // Try platform-appropriate data directory first.
        if let Some(data) = dirs_next(&["nexus-os", "models"]) {
            return Ok(data);
        }
        // Fallback: ~/.nexus-os/models/
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| FlashError::DownloadError("cannot determine home dir".into()))?;
        Ok(PathBuf::from(home).join(".nexus-os").join("models"))
    }
}

// ── ModelDownloader ────────────────────────────────────────────────

/// Downloads GGUF models from HuggingFace with resume support.
pub struct ModelDownloader {
    storage: ModelStorage,
    client: reqwest::Client,
}

impl ModelDownloader {
    pub fn new(storage: ModelStorage) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("NexusOS/9.3.0")
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { storage, client }
    }

    /// Access the underlying storage.
    pub fn storage(&self) -> &ModelStorage {
        &self.storage
    }

    /// Download a model file from HuggingFace.
    ///
    /// `hf_repo`: e.g. `"unsloth/Qwen3.5-397B-A17B-GGUF"`
    /// `filename`: e.g. `"model-Q4_K_M.gguf"`
    /// `progress_tx`: channel to send progress updates (best-effort).
    pub async fn download(
        &self,
        hf_repo: &str,
        filename: &str,
        progress_tx: mpsc::Sender<DownloadProgress>,
    ) -> Result<LocalModel, FlashError> {
        self.download_single(hf_repo, filename, 1, 1, &progress_tx)
            .await
    }

    /// Download a multi-part split model (e.g. 5 shard files).
    ///
    /// `filenames`: ordered list of shard filenames.
    pub async fn download_multi(
        &self,
        hf_repo: &str,
        filenames: &[String],
        progress_tx: mpsc::Sender<DownloadProgress>,
    ) -> Result<LocalModel, FlashError> {
        if filenames.is_empty() {
            return Err(FlashError::DownloadError("no filenames provided".into()));
        }
        let count = filenames.len() as u32;
        for (i, filename) in filenames.iter().enumerate() {
            self.download_single(hf_repo, filename, (i as u32) + 1, count, &progress_tx)
                .await?;
        }
        // Return the first shard — llama.cpp loads from the first file.
        let first = &filenames[0];
        let path = self.storage.model_path(first);
        build_local_model(first, &path)
    }

    async fn download_single(
        &self,
        hf_repo: &str,
        filename: &str,
        file_index: u32,
        file_count: u32,
        progress_tx: &mpsc::Sender<DownloadProgress>,
    ) -> Result<LocalModel, FlashError> {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            hf_repo, filename
        );
        let dest = self.storage.model_path(filename);
        let part_path = self.storage.base_dir().join(format!("{filename}.part"));

        // Already fully downloaded?
        if dest.exists() {
            let meta = std::fs::metadata(&dest)
                .map_err(|e| FlashError::DownloadError(format!("metadata: {e}")))?;
            let _ = progress_tx
                .send(DownloadProgress {
                    model_name: filename.to_string(),
                    file_index,
                    file_count,
                    bytes_downloaded: meta.len(),
                    total_bytes: meta.len(),
                    percent: 100.0,
                    speed_mb_per_sec: 0.0,
                    eta_seconds: 0,
                    status: DownloadStatus::Complete,
                })
                .await;
            return build_local_model(filename, &dest);
        }

        // Resume: check .part file
        let mut downloaded_bytes: u64 = 0;
        if part_path.exists() {
            downloaded_bytes = std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
        }

        let _ = progress_tx
            .send(DownloadProgress {
                model_name: filename.to_string(),
                file_index,
                file_count,
                bytes_downloaded: downloaded_bytes,
                total_bytes: 0,
                percent: 0.0,
                speed_mb_per_sec: 0.0,
                eta_seconds: 0,
                status: DownloadStatus::Starting,
            })
            .await;

        // Build request with optional Range header for resume.
        let mut request = self.client.get(&url);
        if downloaded_bytes > 0 {
            request = request.header("Range", format!("bytes={downloaded_bytes}-"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| FlashError::DownloadError(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(FlashError::DownloadError(format!(
                "HTTP {}: {}",
                response.status(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("unknown error")
            )));
        }

        // Total size from Content-Range or Content-Length.
        let total_bytes = if downloaded_bytes > 0 {
            response
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').next_back())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        } else {
            response.content_length().unwrap_or(0)
        };

        // Stream to .part file.
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
            .await
            .map_err(|e| FlashError::DownloadError(format!("open part file: {e}")))?;

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();
        let mut last_report = start_time;

        while let Some(chunk_result) = stream.next().await {
            let chunk =
                chunk_result.map_err(|e| FlashError::DownloadError(format!("stream read: {e}")))?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|e| FlashError::DownloadError(format!("write: {e}")))?;
            downloaded_bytes += chunk.len() as u64;

            // Report progress every 500ms.
            if last_report.elapsed() > std::time::Duration::from_millis(500) {
                let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                let speed = downloaded_bytes as f64 / elapsed / 1_048_576.0;
                let remaining = if speed > 0.0 && total_bytes > downloaded_bytes {
                    ((total_bytes - downloaded_bytes) as f64 / (speed * 1_048_576.0)) as u64
                } else {
                    0
                };

                let _ = progress_tx
                    .send(DownloadProgress {
                        model_name: filename.to_string(),
                        file_index,
                        file_count,
                        bytes_downloaded: downloaded_bytes,
                        total_bytes,
                        percent: if total_bytes > 0 {
                            (downloaded_bytes as f64 / total_bytes as f64) * 100.0
                        } else {
                            0.0
                        },
                        speed_mb_per_sec: speed,
                        eta_seconds: remaining,
                        status: DownloadStatus::Downloading,
                    })
                    .await;

                last_report = std::time::Instant::now();
            }
        }

        // Flush before rename.
        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| FlashError::DownloadError(format!("flush: {e}")))?;
        drop(file);

        // Rename .part → final.
        tokio::fs::rename(&part_path, &dest)
            .await
            .map_err(|e| FlashError::DownloadError(format!("rename: {e}")))?;

        let _ = progress_tx
            .send(DownloadProgress {
                model_name: filename.to_string(),
                file_index,
                file_count,
                bytes_downloaded: total_bytes,
                total_bytes,
                percent: 100.0,
                speed_mb_per_sec: 0.0,
                eta_seconds: 0,
                status: DownloadStatus::Complete,
            })
            .await;

        build_local_model(filename, &dest)
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn build_local_model(filename: &str, path: &Path) -> Result<LocalModel, FlashError> {
    let meta =
        std::fs::metadata(path).map_err(|e| FlashError::DownloadError(format!("metadata: {e}")))?;
    Ok(LocalModel {
        name: filename.to_string(),
        file_path: path.to_string_lossy().to_string(),
        file_size_bytes: meta.len(),
        file_size_display: format_bytes(meta.len()),
        quant_type: extract_quant_from_filename(filename),
        downloaded_at: chrono::Utc::now().to_rfc3339(),
        sha256: None,
        verified: false,
    })
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Extract quantization type from GGUF filename.
///
/// Looks for patterns like `Q4_K_M`, `Q8_0`, `IQ4_XS`, `MXFP4_MOE`, etc.
pub fn extract_quant_from_filename(filename: &str) -> String {
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);
    // Strip multi-part suffix like "-00001-of-00005"
    let stem = strip_shard_suffix(stem);

    // Try known quant prefixes from the end of the stem, separated by `-`
    let parts: Vec<&str> = stem.split('-').collect();
    for i in (0..parts.len()).rev() {
        let candidate = parts[i];
        if is_quant_token(candidate) {
            // Check if preceding parts are also quant tokens (e.g. Q4_K_XL → might be split)
            let mut quant = candidate.to_string();
            // Some quant names span two dash-parts: "UD-Q4_K_XL"
            if i > 0 && (parts[i - 1] == "UD" || parts[i - 1] == "BF16") {
                quant = format!("{}-{}", parts[i - 1], candidate);
            }
            return quant;
        }
    }

    // Also try underscore-separated from the end
    let uparts: Vec<&str> = stem.split('_').collect();
    for i in (0..uparts.len()).rev() {
        let window = uparts[i..].join("_");
        if is_quant_token(&window) {
            return window;
        }
    }

    "Unknown".to_string()
}

/// Returns true if the filename is any part of a split GGUF model
/// (contains `-NNNNN-of-NNNNN`).
fn is_split_shard(filename: &str) -> bool {
    // Match pattern: -DIGITS-of-DIGITS  (e.g. -00001-of-00004)
    if let Some(of_idx) = filename.find("-of-") {
        let before = &filename[..of_idx];
        if let Some(dash_idx) = before.rfind('-') {
            let shard_num = &before[dash_idx + 1..];
            return shard_num.chars().all(|c| c.is_ascii_digit()) && !shard_num.is_empty();
        }
    }
    false
}

/// Returns true if the filename is the first shard (`-00001-of-`).
fn is_first_shard(filename: &str) -> bool {
    if let Some(of_idx) = filename.find("-of-") {
        let before = &filename[..of_idx];
        if let Some(dash_idx) = before.rfind('-') {
            let shard_num = &before[dash_idx + 1..];
            // The first shard number is all zeros except the last digit is 1
            return shard_num.chars().all(|c| c.is_ascii_digit())
                && !shard_num.is_empty()
                && shard_num.parse::<u64>().ok() == Some(1);
        }
    }
    false
}

/// Extract the prefix of a split filename (everything before `-NNNNN-of-`).
/// Used as a grouping key to sum sizes across all parts.
fn split_prefix(filename: &str) -> String {
    if let Some(of_idx) = filename.find("-of-") {
        let before = &filename[..of_idx];
        if let Some(dash_idx) = before.rfind('-') {
            return before[..dash_idx].to_string();
        }
    }
    filename.to_string()
}

fn strip_shard_suffix(stem: &str) -> &str {
    // Remove "-00001-of-00005" style suffixes
    if let Some(idx) = stem.rfind("-of-") {
        // Walk back to find the shard number part
        let before_of = &stem[..idx];
        if let Some(dash_idx) = before_of.rfind('-') {
            let shard_num = &before_of[dash_idx + 1..];
            if shard_num.chars().all(|c| c.is_ascii_digit()) {
                return &stem[..dash_idx];
            }
        }
    }
    stem
}

fn is_quant_token(s: &str) -> bool {
    let upper = s.to_uppercase();
    // Standard GGUF quant names
    let prefixes = [
        "Q2_K", "Q3_K", "Q4_K", "Q4_0", "Q4_1", "Q5_K", "Q5_0", "Q5_1", "Q6_K", "Q8_0", "Q8_1",
        "IQ1", "IQ2", "IQ3", "IQ4", "F16", "F32", "BF16", "MXFP4", "MXFP6",
    ];
    for prefix in &prefixes {
        if upper.starts_with(prefix) {
            return true;
        }
    }
    false
}

fn file_modified_iso(meta: &std::fs::Metadata) -> String {
    meta.modified()
        .ok()
        .and_then(|t| {
            let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
            let dt = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)?;
            Some(dt.to_rfc3339())
        })
        .unwrap_or_default()
}

/// Platform-appropriate data directory.
fn dirs_next(components: &[&str]) -> Option<PathBuf> {
    // Linux: ~/.local/share/  macOS: ~/Library/Application Support/  Windows: %APPDATA%
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    } else {
        // XDG on Linux/BSD
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local").join("share"))
            })
    };

    base.map(|mut p| {
        for c in components {
            p = p.join(c);
        }
        p
    })
}

/// Get available disk space for a path (best-effort, returns 0 on failure).
///
/// Uses `df` on Unix-like systems as a safe alternative to `statvfs`.
fn available_space_bytes(path: &Path) -> u64 {
    let output = std::process::Command::new("df")
        .arg("--output=avail")
        .arg("-B1")
        .arg(path)
        .output()
        .ok();

    if let Some(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            // Skip the header line, parse the second line.
            if let Some(line) = text.lines().nth(1) {
                return line.trim().parse::<u64>().unwrap_or(0);
            }
        }
    }

    // Fallback: try POSIX df without --output (macOS)
    let output = std::process::Command::new("df")
        .arg("-k")
        .arg(path)
        .output()
        .ok();

    if let Some(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = text.lines().nth(1) {
                // df -k columns: Filesystem 1K-blocks Used Available ...
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    return parts[3].parse::<u64>().unwrap_or(0) * 1024;
                }
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(5_242_880), "5.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(225_485_783_040), "210.0 GB");
    }

    #[test]
    fn test_extract_quant_from_filename() {
        assert_eq!(
            extract_quant_from_filename("Qwen3.5-397B-A17B-UD-Q4_K_XL.gguf"),
            "UD-Q4_K_XL"
        );
        assert_eq!(
            extract_quant_from_filename("Llama-3.3-70B-Q4_K_M.gguf"),
            "Q4_K_M"
        );
        assert_eq!(extract_quant_from_filename("model-Q8_0.gguf"), "Q8_0");
        assert_eq!(
            extract_quant_from_filename("Qwen3.5-397B-A17B-UD-Q4_K_XL-00001-of-00005.gguf"),
            "UD-Q4_K_XL"
        );
        assert_eq!(
            extract_quant_from_filename("phi-4-mini-IQ4_XS.gguf"),
            "IQ4_XS"
        );
    }

    #[test]
    fn test_strip_shard_suffix() {
        assert_eq!(
            strip_shard_suffix("Model-Q4_K_M-00001-of-00005"),
            "Model-Q4_K_M"
        );
        assert_eq!(strip_shard_suffix("Model-Q4_K_M"), "Model-Q4_K_M");
    }

    #[test]
    fn test_is_quant_token() {
        assert!(is_quant_token("Q4_K_M"));
        assert!(is_quant_token("Q4_K_XL"));
        assert!(is_quant_token("Q8_0"));
        assert!(is_quant_token("IQ4_XS"));
        assert!(is_quant_token("F16"));
        assert!(is_quant_token("MXFP4_MOE"));
        assert!(!is_quant_token("70B"));
        assert!(!is_quant_token("A17B"));
    }

    #[test]
    fn test_is_split_shard() {
        assert!(is_split_shard(
            "Qwen3.5-397B-A17B-UD-IQ3_XXS-00001-of-00004.gguf"
        ));
        assert!(is_split_shard(
            "Qwen3.5-397B-A17B-UD-IQ3_XXS-00003-of-00004.gguf"
        ));
        assert!(!is_split_shard("Llama-3.3-70B-Q4_K_M.gguf"));
        assert!(!is_split_shard("model-Q8_0.gguf"));
    }

    #[test]
    fn test_is_first_shard() {
        assert!(is_first_shard(
            "Qwen3.5-397B-A17B-UD-IQ3_XXS-00001-of-00004.gguf"
        ));
        assert!(!is_first_shard(
            "Qwen3.5-397B-A17B-UD-IQ3_XXS-00002-of-00004.gguf"
        ));
        assert!(!is_first_shard("Llama-3.3-70B-Q4_K_M.gguf"));
    }

    #[test]
    fn test_split_prefix() {
        assert_eq!(
            split_prefix("Qwen3.5-397B-A17B-UD-IQ3_XXS-00001-of-00004.gguf"),
            "Qwen3.5-397B-A17B-UD-IQ3_XXS"
        );
        assert_eq!(
            split_prefix("Qwen3.5-397B-A17B-UD-IQ3_XXS-00003-of-00004.gguf"),
            "Qwen3.5-397B-A17B-UD-IQ3_XXS"
        );
    }

    #[test]
    fn test_list_models_collapses_split_files() {
        let tmp = std::env::temp_dir().join("nexus-flash-test-split");
        let _ = std::fs::remove_dir_all(&tmp);
        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();

        // Create 4 fake split shard files (33 GB each → 132 GB total)
        let shard_size = 33 * 1024; // small fake size in bytes
        for i in 1..=4 {
            let name = format!("Qwen3.5-397B-A17B-UD-IQ3_XXS-{:05}-of-00004.gguf", i);
            let data = vec![0u8; shard_size];
            std::fs::write(storage.model_path(&name), &data).unwrap();
        }

        // Also create a non-split model
        std::fs::write(storage.model_path("Llama-3.3-70B-Q4_K_M.gguf"), b"single").unwrap();

        let models = storage.list_models().unwrap();

        // Should see exactly 2 models: the first shard and the single model
        assert_eq!(models.len(), 2, "expected 2 models, got: {models:?}");

        // Find the split model entry
        let split = models
            .iter()
            .find(|m| m.name.contains("IQ3_XXS"))
            .expect("should find split model");
        assert!(
            split.name.contains("-00001-of-"),
            "split model should show first shard"
        );
        // Size should be sum of all 4 parts
        let expected_total = (shard_size * 4) as u64;
        assert_eq!(
            split.file_size_bytes, expected_total,
            "split model size should be sum of all parts"
        );

        // Non-split model should be present with its own size
        let single = models
            .iter()
            .find(|m| m.name.contains("Llama"))
            .expect("should find single model");
        assert_eq!(single.file_size_bytes, 6); // b"single".len()

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_model_storage_with_dir() {
        let tmp = std::env::temp_dir().join("nexus-flash-test-storage");
        let _ = std::fs::remove_dir_all(&tmp);
        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        assert!(tmp.exists());
        assert!(storage.list_models().unwrap().is_empty());
        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let tmp = std::env::temp_dir().join("nexus-flash-test-del");
        let _ = std::fs::remove_dir_all(&tmp);
        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        // Deleting a file that doesn't exist should succeed.
        assert!(storage.delete_model("nonexistent.gguf").is_ok());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_list_models_finds_gguf() {
        let tmp = std::env::temp_dir().join("nexus-flash-test-list");
        let _ = std::fs::remove_dir_all(&tmp);
        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        // Create a fake .gguf file
        std::fs::write(storage.model_path("test-Q4_K_M.gguf"), b"fake").unwrap();
        // Create a non-gguf file (should be ignored)
        std::fs::write(tmp.join("readme.txt"), b"ignore").unwrap();

        let models = storage.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "test-Q4_K_M.gguf");
        assert_eq!(models[0].quant_type, "Q4_K_M");
        assert_eq!(models[0].file_size_bytes, 4);

        // Delete it
        storage.delete_model("test-Q4_K_M.gguf").unwrap();
        assert!(storage.list_models().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_available_disk_space() {
        let tmp = std::env::temp_dir().join("nexus-flash-test-space");
        let _ = std::fs::create_dir_all(&tmp);
        let storage = ModelStorage::with_dir(tmp.clone()).unwrap();
        // Should return some positive value on any real filesystem.
        let space = storage.available_disk_space().unwrap();
        assert!(space > 0, "expected positive disk space, got {space}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
