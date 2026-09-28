//! HuggingFace model hub integration — search, download, and compatibility checking.
//!
//! Uses `curl` via `std::process::Command` for HTTP requests (no extra deps).
//! Model files are downloaded to `~/.nexus/models/{name}/` with a generated
//! `nexus-model.toml` metadata file for discovery by `ModelRegistry`.

use crate::model_registry::{ModelConfig, Quantization};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── Identifiers (P0-002C5B) ─────────────────────────────────────────────────

/// Whether `segment` is a hub name segment: ASCII letters, digits, `.`, `_`
/// and `-` (and `+` for file names), starting with a letter or digit, no
/// trailing dot, not a DOS device name, at most `max` bytes. Such a segment is
/// safe both as a path component and as a URL path segment.
fn hub_segment(segment: &str, max: usize, plus: bool) -> bool {
    let bytes = segment.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= max
        && bytes[0].is_ascii_alphanumeric()
        && !segment.ends_with('.')
        && bytes.iter().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-') || (plus && *b == b'+')
        })
        && nexus_kernel::governed_path::validate_component(segment).is_ok()
}

/// A Hugging Face repository id: `name` or `owner/name`. It names a hub
/// resource and is never a filesystem path.
pub fn validate_hf_model_id(model_id: &str) -> Result<(), String> {
    let segments: Vec<&str> = model_id.split('/').collect();
    if segments.len() > 2 || !segments.iter().all(|s| hub_segment(s, 96, false)) {
        return Err("invalid Hugging Face model id".into());
    }
    Ok(())
}

/// A file inside a repository: at most four `/`-separated hub name segments.
pub fn validate_hf_filename(filename: &str) -> Result<(), String> {
    let segments: Vec<&str> = filename.split('/').collect();
    if segments.len() > 4 || !segments.iter().all(|s| hub_segment(s, 128, true)) {
        return Err("invalid Hugging Face file name".into());
    }
    Ok(())
}

/// The backend-owned directory holding a repository's files beneath the
/// models root: named by a digest of the id, never by the id itself, which
/// contains `/`.
pub fn model_storage_dir(models_root: &Path, model_id: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(model_id.as_bytes()));
    models_root.join(format!("hf-{}", &digest[..32]))
}

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
    // P0-002C5B: HTTPS only, including redirects, with the URL after `--`.
    let url = nexus_kernel::governed_http::http_url(url).map_err(|e| e.to_string())?;
    let output = Command::new("curl")
        .args(nexus_kernel::governed_http::CURL_HTTPS_ONLY)
        .args(["-sS", "-L", "-m", "30", "--max-filesize", "33554432", "--"])
        .arg(url.as_str())
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
    // P0-002C5B: the id and file name are validated grammars, the target root
    // is the backend's absolute models directory, and the file lands in a
    // digest-named directory beneath it.
    validate_hf_model_id(model_id)?;
    validate_hf_filename(filename)?;
    let target_root = Path::new(target_dir);
    if !target_root.is_absolute() {
        return Err("models directory is unavailable".into());
    }

    // Build the download URL.
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        model_id, filename
    );

    let model_dir = model_storage_dir(target_root, model_id);
    let file_path = nexus_kernel::governed_path::join_relative(&model_dir, filename)
        .map_err(|_| "invalid Hugging Face file name".to_string())?;
    // A name that differs from a stored file only by letter case would reach
    // that file on a case-insensitive filesystem: one spelling per name.
    nexus_kernel::governed_path::case_exact_relative(&model_dir, filename).map_err(|_| {
        "model file name differs from a stored file only by letter case".to_string()
    })?;
    let parent = file_path.parent().unwrap_or(&model_dir);
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create model directory: {e}"))?;
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

    // First, get the file size via a HEAD request. A declared size above the
    // maximum is refused at once; a smaller or missing one is not trusted.
    let total_bytes = get_content_length(&url).unwrap_or(0);
    if total_bytes > MAX_MODEL_FILE_BYTES {
        return Err(model_file_too_large());
    }

    // Start curl download in the background: HTTPS only, including redirects,
    // failing on HTTP errors, with the URL after `--`. A stalled transfer is
    // abandoned, and curl stops a transfer that reaches the backend maximum.
    // Final Gate item I: the in-flight registry starts the transfer and owns
    // it until the download ends, so the application ends it at exit.
    let max_filesize = MAX_MODEL_FILE_BYTES.to_string();
    let mut transfer = IN_FLIGHT_DOWNLOADS.start(file_path.clone(), || {
        Command::new("curl")
            .args(nexus_kernel::governed_http::CURL_HTTPS_ONLY)
            .args([
                "-sS",
                "-L",
                "--fail",
                "--connect-timeout",
                "30",
                "--speed-limit",
                "1",
                "--speed-time",
                "120",
                "--max-filesize",
                &max_filesize,
                "-o",
            ])
            .arg(&file_path_str)
            .arg("--")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("curl spawn failed: {e}"))
    })?;

    // Monitor file size growth while curl runs, whatever size was declared.
    let final_size = watch_download(
        &mut transfer,
        &file_path,
        MAX_MODEL_FILE_BYTES,
        std::time::Duration::from_millis(500),
        |current_size| {
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
        },
    )?;
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
    Ok(file_path_str)
}

/// The largest model file the backend downloads (P0-002C5C). Hugging Face
/// stores a single file of at most 50 GB, so a larger transfer, whatever
/// length the server declares, is stopped and its file removed.
pub const MAX_MODEL_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

fn model_file_too_large() -> String {
    format!("download refused: the model file exceeds the {MAX_MODEL_FILE_BYTES}-byte maximum")
}

/// A running transfer that writes the download file: the curl child.
trait Transfer {
    /// `Some(success)` once the transfer has ended, `None` while it runs.
    fn finished(&mut self) -> std::io::Result<Option<bool>>;
    /// Stop the transfer and reap it.
    fn stop(&mut self);
}

// ─── In-flight downloads (Final Gate item I) ─────────────────────────────────
//
// A model download runs a curl child for as long as the transfer takes. It
// used to be owned only by the thread watching it, so it outlived the
// application: nothing stopped it at exit. Each child is now started and
// owned by an in-flight registry until its download ends, and
// `terminate_in_flight_downloads` ends every one: through the owned handle
// only (kill, then reap), never by process id, name or port, within a bound,
// reporting truthfully what it could not confirm. No download starts after it.

/// The total time `terminate_in_flight_downloads` waits for killed transfers
/// to be reaped.
pub const DOWNLOAD_TERMINATION_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// How often a killed transfer is checked for its exit.
const REAP_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// The downloads this process runs, for the application's exit.
static IN_FLIGHT_DOWNLOADS: InFlightDownloads = InFlightDownloads::new();

/// End every in-flight model download for the application's exit, within
/// [`DOWNLOAD_TERMINATION_WAIT`]; no download starts afterwards. Each
/// transfer is killed and reaped through its owned handle only, and its
/// partial file removed. A transfer that had already ended is reaped, not an
/// error, and keeps its file only if it completed. `Ok` counts the transfers
/// confirmed ended. The error counts those whose exit could not be confirmed;
/// they stay registered, so a later call tries again. A second call after
/// success finds nothing to do.
pub fn terminate_in_flight_downloads() -> Result<usize, DownloadTermination> {
    IN_FLIGHT_DOWNLOADS.terminate_all(std::time::Instant::now() + DOWNLOAD_TERMINATION_WAIT)
}

/// Why `terminate_in_flight_downloads` could not confirm every transfer
/// ended. It carries counts only: no path, URL or process id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadTermination {
    /// Transfers confirmed ended.
    pub ended: usize,
    /// Transfers whose exit could not be confirmed.
    pub not_confirmed: usize,
}

impl std::fmt::Display for DownloadTermination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "model download cleanup not confirmed: {} of {} in-flight downloads may still be running",
            self.not_confirmed,
            self.ended + self.not_confirmed
        )
    }
}

/// The process behind an owned transfer: the curl child.
trait TransferProcess: Send {
    /// `Some(success)` once it has exited; a killed process never succeeds.
    fn exit(&mut self) -> std::io::Result<Option<bool>>;
    /// Send it the kill signal.
    fn kill(&mut self) -> std::io::Result<()>;
}

impl TransferProcess for std::process::Child {
    fn exit(&mut self) -> std::io::Result<Option<bool>> {
        Ok(self.try_wait()?.map(|status| status.success()))
    }

    fn kill(&mut self) -> std::io::Result<()> {
        std::process::Child::kill(self)
    }
}

/// An owned transfer: its process and the file it writes.
struct OwnedTransfer {
    process: Box<dyn TransferProcess>,
    file: PathBuf,
}

type SharedTransfer = std::sync::Arc<std::sync::Mutex<OwnedTransfer>>;

/// The in-flight transfers, keyed by a private sequence number.
struct InFlightDownloads {
    state: std::sync::Mutex<InFlight>,
}

struct InFlight {
    /// Set by the exit cleanup: nothing starts afterwards.
    closed: bool,
    next: u64,
    transfers: std::collections::BTreeMap<u64, SharedTransfer>,
}

fn locked<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where stopping an owned transfer stands.
enum Stopping {
    /// Its exit is confirmed: `true` when the transfer completed by itself.
    Ended(bool),
    /// It was sent the kill; its exit is not confirmed yet.
    Killed,
    /// It could not be killed, and its exit is not confirmed.
    Failed,
}

/// Kill an owned transfer unless it has already ended. Its lock is held for
/// the exit check and the kill only.
fn kill_unless_ended(transfer: &std::sync::Mutex<OwnedTransfer>) -> Stopping {
    let mut transfer = locked(transfer);
    if let Ok(Some(success)) = transfer.process.exit() {
        return Stopping::Ended(success);
    }
    match transfer.process.kill() {
        Ok(()) => Stopping::Killed,
        // It may have exited between the check and the kill: not a failure.
        Err(_) => match transfer.process.exit() {
            Ok(Some(success)) => Stopping::Ended(success),
            _ => Stopping::Failed,
        },
    }
}

/// Wait until `deadline` for a killed transfer's exit: `Some(success)` once
/// it is confirmed. Its lock is never held while waiting.
fn reap_by(
    transfer: &std::sync::Mutex<OwnedTransfer>,
    deadline: std::time::Instant,
) -> Option<bool> {
    loop {
        let exit = locked(transfer).process.exit();
        match exit {
            Ok(Some(success)) => return Some(success),
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(REAP_POLL),
            _ => return None,
        }
    }
}

impl InFlightDownloads {
    const fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(InFlight {
                closed: false,
                next: 0,
                transfers: std::collections::BTreeMap::new(),
            }),
        }
    }

    /// Start a transfer with `spawn` and own it until the returned handle is
    /// dropped. `spawn` runs under the registry lock, so a transfer is either
    /// started and registered before the exit cleanup looks, or never
    /// started: nothing starts once the cleanup ran.
    fn start<P: TransferProcess + 'static>(
        &self,
        file: PathBuf,
        spawn: impl FnOnce() -> Result<P, String>,
    ) -> Result<RegisteredTransfer<'_>, String> {
        let mut state = locked(&self.state);
        if state.closed {
            return Err("download refused: the application is exiting".to_string());
        }
        let process: Box<dyn TransferProcess> = Box::new(spawn()?);
        let transfer = std::sync::Arc::new(std::sync::Mutex::new(OwnedTransfer { process, file }));
        let id = state.next;
        state.next += 1;
        state.transfers.insert(id, std::sync::Arc::clone(&transfer));
        Ok(RegisteredTransfer {
            registry: self,
            id,
            transfer,
        })
    }

    /// End every registered transfer by `deadline` and refuse new ones (see
    /// [`terminate_in_flight_downloads`]). All are killed first, then reaped.
    /// The registry lock is held only to close it and to copy or remove
    /// entries, never while a transfer is stopped or a file removed.
    fn terminate_all(&self, deadline: std::time::Instant) -> Result<usize, DownloadTermination> {
        let transfers: Vec<(u64, SharedTransfer)> = {
            let mut state = locked(&self.state);
            state.closed = true;
            state
                .transfers
                .iter()
                .map(|(id, transfer)| (*id, std::sync::Arc::clone(transfer)))
                .collect()
        };
        let (mut ended, mut not_confirmed) = (0, 0);
        let mut killed = Vec::new();
        for (id, transfer) in transfers {
            match kill_unless_ended(&transfer) {
                Stopping::Ended(completed) => {
                    self.release(id, &transfer, completed);
                    ended += 1;
                }
                Stopping::Killed => killed.push((id, transfer)),
                Stopping::Failed => not_confirmed += 1,
            }
        }
        for (id, transfer) in killed {
            match reap_by(&transfer, deadline) {
                Some(completed) => {
                    self.release(id, &transfer, completed);
                    ended += 1;
                }
                None => not_confirmed += 1,
            }
        }
        if not_confirmed == 0 {
            Ok(ended)
        } else {
            Err(DownloadTermination {
                ended,
                not_confirmed,
            })
        }
    }

    /// Forget a transfer whose exit is confirmed, removing its file unless it
    /// completed: a stopped or failed transfer leaves only a partial file.
    /// Best-effort; its watcher, if it still runs, removes it as well.
    fn release(&self, id: u64, transfer: &std::sync::Mutex<OwnedTransfer>, completed: bool) {
        if !completed {
            let file = locked(transfer).file.clone();
            let _ = std::fs::remove_file(file);
        }
        locked(&self.state).transfers.remove(&id);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        locked(&self.state).transfers.len()
    }
}

/// A download's transfer while its download runs. Dropping it releases the
/// transfer from the registry once its exit is confirmed; a transfer that may
/// still run stays registered, so `terminate_in_flight_downloads` still ends
/// it.
struct RegisteredTransfer<'a> {
    registry: &'a InFlightDownloads,
    id: u64,
    transfer: SharedTransfer,
}

impl Transfer for RegisteredTransfer<'_> {
    fn finished(&mut self) -> std::io::Result<Option<bool>> {
        locked(&self.transfer).process.exit()
    }

    fn stop(&mut self) {
        // A transfer not confirmed stopped stays registered (see Drop).
        if let Stopping::Killed = kill_unless_ended(&self.transfer) {
            let _ = reap_by(
                &self.transfer,
                std::time::Instant::now() + DOWNLOAD_TERMINATION_WAIT,
            );
        }
    }
}

impl Drop for RegisteredTransfer<'_> {
    fn drop(&mut self) {
        let ended = matches!(locked(&self.transfer).process.exit(), Ok(Some(_)));
        if ended {
            locked(&self.registry.state).transfers.remove(&self.id);
        }
    }
}

/// Watch `transfer` write `file_path` until it ends, reporting the size so
/// far. A file larger than `max_bytes` stops the transfer and is removed,
/// whether it is still growing or complete, so the bound never depends on a
/// declared length. Any failure also removes the partial file.
fn watch_download(
    transfer: &mut dyn Transfer,
    file_path: &Path,
    max_bytes: u64,
    poll_interval: std::time::Duration,
    mut progress: impl FnMut(u64),
) -> Result<u64, String> {
    loop {
        let finished = transfer.finished();
        let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        if size > max_bytes {
            transfer.stop();
            // Best-effort: the oversized file is removed.
            let _ = std::fs::remove_file(file_path);
            return Err(model_file_too_large());
        }
        match finished {
            Ok(Some(true)) if file_path.exists() => return Ok(size),
            Ok(Some(_)) => {
                // Best-effort: clean up partial download on curl failure
                let _ = std::fs::remove_file(file_path);
                return Err("download failed: curl exited with error".to_string());
            }
            Ok(None) => {
                progress(size);
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                transfer.stop();
                // Best-effort: clean up partial download on wait error
                let _ = std::fs::remove_file(file_path);
                return Err(format!("error waiting for curl: {e}"));
            }
        }
    }
}

/// Get Content-Length of a URL via a HEAD request.
fn get_content_length(url: &str) -> Option<u64> {
    let output = Command::new("curl")
        .args(nexus_kernel::governed_http::CURL_HTTPS_ONLY)
        .args([
            "-sS",
            "-L",
            "-I",
            "-m",
            "10",
            "--max-filesize",
            "1048576",
            "--",
        ])
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
    // P0-002C5B: both values are interpolated into TOML and a path.
    validate_hf_model_id(model_id)?;
    validate_hf_filename(filename)?;
    let dir = PathBuf::from(model_dir);
    if !dir.is_absolute() {
        return Err("model directory must be absolute".into());
    }

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
        .args(nexus_kernel::governed_http::CURL_HTTP_ONLY)
        .args([
            "-sS",
            "-m",
            "600",
            "--max-filesize",
            "16777216",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "--data-raw",
            &payload.to_string(),
            "--",
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

    const HOSTILE_IDS: &[&str] = &[
        "",
        "..",
        "../x",
        "a/../b",
        "/abs",
        "a//b",
        "a/b/c",
        "C:\\x",
        "a\\b",
        "a b",
        "a?b",
        "a#b",
        "a:b",
        "org/CON",
        "trailing.",
        "-flag",
    ];
    const HOSTILE_FILES: &[&str] = &[
        "",
        "..",
        "../../escape.gguf",
        "/etc/passwd",
        "a/../b",
        "a//b",
        "C:\\x",
        "a\\b",
        "x.gguf?x=1",
        "x#y",
        "a:ads",
        "CON",
        "a/b/c/d/e",
        "-o",
    ];

    /// A transfer that appends `chunk` bytes to the file each time it is
    /// polled, `polls` times, then ends with `success`.
    struct GrowingTransfer {
        path: PathBuf,
        chunk: usize,
        polls: usize,
        success: bool,
        stopped: bool,
    }

    impl Transfer for GrowingTransfer {
        fn finished(&mut self) -> std::io::Result<Option<bool>> {
            use std::io::Write;
            if self.stopped {
                return Ok(Some(false));
            }
            if self.polls == 0 {
                return Ok(Some(self.success));
            }
            self.polls -= 1;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?
                .write_all(&vec![0u8; self.chunk])?;
            Ok(None)
        }

        fn stop(&mut self) {
            self.stopped = true;
        }
    }

    // ── Final Gate item I: in-flight downloads are owned and ended ─────────

    /// Set only in the environment of a stand-in transfer child.
    const STAND_IN_ENV: &str = "NEXUS_FG_STAND_IN_TRANSFER";

    /// Not a check. Started by these tests as a child with `STAND_IN_ENV` set,
    /// this test binary stands in for a running transfer: it runs until it is
    /// killed, or until the test holding its stdin is gone. Run as a normal
    /// test, it returns at once.
    #[test]
    fn fg_stand_in_transfer() {
        if std::env::var_os(STAND_IN_ENV).is_some() {
            let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]);
        }
    }

    /// This test binary as a transfer process, with no network and no
    /// transfer program.
    fn stand_in(args: &[&str]) -> std::process::Command {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }

    /// A running transfer: it runs until it is killed.
    fn running() -> Result<std::process::Child, String> {
        stand_in(&[
            "--exact",
            "model_hub::tests::fg_stand_in_transfer",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(STAND_IN_ENV, "1")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())
    }

    /// A transfer that has already ended, successfully (the test list) or not
    /// (an unknown option).
    fn ended(success: bool) -> Result<std::process::Child, String> {
        let args: &[&str] = if success {
            &["--list"]
        } else {
            &["--no-such-option"]
        };
        let mut child = stand_in(args)
            .stdin(std::process::Stdio::null())
            .spawn()
            .unwrap();
        assert_eq!(child.wait().unwrap().success(), success);
        Ok(child)
    }

    /// A process whose exit is never confirmed; `killable` says whether it
    /// takes the kill.
    struct Unconfirmed {
        killable: bool,
    }

    impl TransferProcess for Unconfirmed {
        fn exit(&mut self) -> std::io::Result<Option<bool>> {
            Ok(None)
        }

        fn kill(&mut self) -> std::io::Result<()> {
            if self.killable {
                Ok(())
            } else {
                Err(std::io::Error::other("kill refused"))
            }
        }
    }

    /// A scratch directory with these partial download files.
    fn partial_files<const N: usize>(names: [&str; N]) -> (PathBuf, [PathBuf; N]) {
        let dir = std::env::temp_dir().join(format!("nexus-fg-download-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let files = names.map(|name| {
            let file = dir.join(name);
            std::fs::write(&file, b"partial").unwrap();
            file
        });
        (dir, files)
    }

    fn deadline() -> std::time::Instant {
        std::time::Instant::now() + std::time::Duration::from_secs(30)
    }

    /// Final Gate item I: ending the in-flight downloads kills and reaps
    /// every running transfer through its owned handle, removes its partial
    /// file, reports the count and empties the registry. A second call finds
    /// nothing to do.
    #[test]
    fn p0_fg_terminating_in_flight_downloads_reaps_each_running_transfer() {
        let registry = InFlightDownloads::new();
        let (dir, [first, second]) = partial_files(["first.gguf", "second.gguf"]);
        let mut a = registry.start(first.clone(), running).unwrap();
        let mut b = registry.start(second.clone(), running).unwrap();
        assert_eq!(registry.len(), 2);
        assert_eq!(a.finished().unwrap(), None);

        assert_eq!(registry.terminate_all(deadline()), Ok(2));
        // Each was killed, so it did not exit successfully, and was reaped.
        assert_eq!(a.finished().unwrap(), Some(false));
        assert_eq!(b.finished().unwrap(), Some(false));
        assert!(!first.exists() && !second.exists());
        assert_eq!(registry.len(), 0);

        assert_eq!(registry.terminate_all(deadline()), Ok(0));
        drop((a, b));
        assert_eq!(registry.len(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A transfer that had already ended is reaped and counted, not an
    /// error. Only a completed one keeps its file.
    #[test]
    fn p0_fg_a_transfer_that_already_ended_is_not_a_termination_error() {
        let registry = InFlightDownloads::new();
        let (dir, [complete, failed]) = partial_files(["complete.gguf", "failed.gguf"]);
        let handles = [
            registry.start(complete.clone(), || ended(true)).unwrap(),
            registry.start(failed.clone(), || ended(false)).unwrap(),
        ];
        assert_eq!(registry.terminate_all(deadline()), Ok(2));
        assert!(complete.exists() && !failed.exists());
        assert_eq!(registry.len(), 0);
        drop(handles);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An exit that cannot be confirmed is reported, never counted as ended:
    /// the transfer stays registered and a later call reports it again.
    #[test]
    fn p0_fg_an_unconfirmed_exit_is_reported_and_stays_registered() {
        let registry = InFlightDownloads::new();
        let (dir, [first, second]) = partial_files(["first.gguf", "second.gguf"]);
        let handles = [
            registry
                .start(first.clone(), || Ok(Unconfirmed { killable: true }))
                .unwrap(),
            registry
                .start(second.clone(), || Ok(Unconfirmed { killable: false }))
                .unwrap(),
        ];
        let unconfirmed = DownloadTermination {
            ended: 0,
            not_confirmed: 2,
        };
        // A deadline already reached: nothing is waited for.
        assert_eq!(
            registry.terminate_all(std::time::Instant::now()),
            Err(unconfirmed)
        );
        assert_eq!(registry.len(), 2);
        assert!(first.exists() && second.exists());
        drop(handles);
        assert_eq!(registry.len(), 2, "not released while they may run");
        assert_eq!(
            registry.terminate_all(std::time::Instant::now()),
            Err(unconfirmed)
        );
        assert_eq!(
            unconfirmed.to_string(),
            "model download cleanup not confirmed: 2 of 2 in-flight downloads may still be running"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The watcher of a transfer ended at exit sees it end and removes the
    /// partial file itself. (The registry removes its registered path too; a
    /// separate watched path shows the watcher's removal.)
    #[test]
    fn p0_fg_the_watcher_of_an_ended_transfer_removes_its_partial_file() {
        // Leaked, so a failing check never waits on a watcher that runs on.
        let registry: &'static InFlightDownloads = Box::leak(Box::new(InFlightDownloads::new()));
        let (dir, [registered, watched]) = partial_files(["registered.gguf", "watched.gguf"]);
        let mut transfer = registry.start(registered.clone(), running).unwrap();
        let watcher = std::thread::spawn({
            let watched = watched.clone();
            let tick = std::time::Duration::from_millis(1);
            move || watch_download(&mut transfer, &watched, u64::MAX, tick, |_| {})
        });
        assert_eq!(registry.terminate_all(deadline()), Ok(1));
        assert_eq!(
            watcher.join().unwrap(),
            Err("download failed: curl exited with error".to_string())
        );
        assert!(!watched.exists() && !registered.exists());
        assert_eq!(registry.len(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A download that ends normally leaves the registry when its handle
    /// goes, and keeps its file.
    #[test]
    fn p0_fg_a_download_that_ends_normally_leaves_the_registry() {
        let registry = InFlightDownloads::new();
        let (dir, [file]) = partial_files(["model.gguf"]);
        let tick = std::time::Duration::from_millis(1);
        let mut transfer = registry.start(file.clone(), || ended(true)).unwrap();
        assert_eq!(
            watch_download(&mut transfer, &file, u64::MAX, tick, |_| {}),
            Ok(7)
        );
        assert_eq!(registry.len(), 1, "registered while its handle lives");
        drop(transfer);
        assert_eq!(registry.len(), 0);
        assert!(file.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// No transfer starts once the downloads were ended for exit: its spawn
    /// is never run.
    #[test]
    fn p0_fg_no_download_starts_after_the_exit_cleanup() {
        let registry = InFlightDownloads::new();
        assert_eq!(registry.terminate_all(deadline()), Ok(0));
        let refused = registry.start(
            PathBuf::from("never.gguf"),
            || -> Result<Unconfirmed, String> {
                panic!("a transfer was started after the exit cleanup")
            },
        );
        assert_eq!(
            refused.err(),
            Some("download refused: the application is exiting".to_string())
        );
        assert_eq!(registry.len(), 0);
    }

    /// The application's exit cleanup, with nothing in flight, succeeds and
    /// does nothing, twice.
    #[test]
    fn p0_fg_the_exit_cleanup_is_a_no_op_with_nothing_in_flight() {
        assert_eq!(terminate_in_flight_downloads(), Ok(0));
        assert_eq!(terminate_in_flight_downloads(), Ok(0));
    }

    /// P0-002C5C: the backend's model-file maximum is enforced on the bytes
    /// written, while they are written, with no declared length involved.
    #[test]
    fn p0_002c5c_model_downloads_stop_and_are_removed_past_the_bound() {
        let dir = std::env::temp_dir().join(format!("nexus-c5c-hub-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.gguf");
        let transfer = |chunk, polls, success| GrowingTransfer {
            path: path.clone(),
            chunk,
            polls,
            success,
            stopped: false,
        };
        let zero = std::time::Duration::ZERO;

        // A transfer that keeps growing is stopped once past the bound, and
        // its file removed; no progress report exceeds the bound.
        let mut growing = transfer(400, 10, true);
        let mut reported = Vec::new();
        let result = watch_download(&mut growing, &path, 1000, zero, |size| reported.push(size));
        assert_eq!(result, Err(model_file_too_large()));
        assert!(growing.stopped);
        assert!(!path.exists());
        assert_eq!(reported, vec![400, 800]);

        // A transfer that ends with a file past the bound is refused too.
        std::fs::write(&path, vec![0u8; 1001]).unwrap();
        let mut ended = transfer(0, 0, true);
        assert_eq!(
            watch_download(&mut ended, &path, 1000, zero, |_| {}),
            Err(model_file_too_large())
        );
        assert!(!path.exists());

        // Within the bound the file is kept.
        let mut within = transfer(250, 4, true);
        assert_eq!(
            watch_download(&mut within, &path, 1000, zero, |_| {}),
            Ok(1000)
        );
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 1000);
        std::fs::remove_file(&path).unwrap();

        // A failed transfer leaves no partial file.
        let mut failed = transfer(100, 2, false);
        assert!(watch_download(&mut failed, &path, 1000, zero, |_| {}).is_err());
        assert!(!path.exists());

        // The production bound is finite and above the largest hub file.
        const { assert!(MAX_MODEL_FILE_BYTES > 50_000_000_000) };
        const { assert!(MAX_MODEL_FILE_BYTES < u64::MAX / 2) };
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn p0_002c5b_hub_identifiers_are_strict_grammars() {
        for id in ["gpt2", "org/model", "TheBloke/Llama-2-7B-GGUF", "a.b_c-d/e"] {
            assert_eq!(validate_hf_model_id(id), Ok(()), "{id:?}");
        }
        for id in HOSTILE_IDS {
            assert!(validate_hf_model_id(id).is_err(), "{id:?}");
        }
        for file in ["model.gguf", "onnx/model.onnx", "a/b/c/d.bin", "x+y.gguf"] {
            assert_eq!(validate_hf_filename(file), Ok(()), "{file:?}");
        }
        for file in HOSTILE_FILES {
            assert!(validate_hf_filename(file).is_err(), "{file:?}");
        }
    }

    #[test]
    fn p0_002c5b_model_storage_is_a_digest_never_the_repository_id() {
        let root = std::path::Path::new("/nexus/models");
        let dir = model_storage_dir(root, "org/model");
        assert_eq!(dir.parent(), Some(root));
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("hf-") && name.len() == 35, "{name}");
        assert!(!name.contains("org") && !name.contains("model"));
        assert_ne!(dir, model_storage_dir(root, "org/model2"));
        assert_eq!(dir, model_storage_dir(root, "org/model"));
    }

    #[test]
    fn p0_002c5b_hostile_downloads_are_refused_before_any_file_or_request() {
        let root = std::env::temp_dir().join(format!("nexus-p0-002c5b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let target = root.to_string_lossy().into_owned();
        for file in HOSTILE_FILES {
            let result = download_model_file("org/model", file, &target, |_| {});
            assert!(result.is_err(), "{file:?}");
        }
        for id in HOSTILE_IDS {
            let result = download_model_file(id, "model.gguf", &target, |_| {});
            assert!(result.is_err(), "{id:?}");
        }
        assert!(download_model_file("org/model", "model.gguf", "relative", |_| {}).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        assert!(generate_model_config("../x", "model.gguf", &target).is_err());
        assert!(generate_model_config("org/model", "../x.gguf", &target).is_err());
        assert!(generate_model_config("org/model", "model.gguf", "relative").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

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

    #[test]
    fn p0_002c5b_hub_requests_are_http_urls_before_curl_runs() {
        for url in [
            "file:///etc/passwd",
            "-K/etc/passwd",
            "@/etc/passwd",
            "https://a@huggingface.co/api/models",
        ] {
            let err = http_get(url).unwrap_err();
            assert!(
                err.contains("URL must be an http or https URL"),
                "{url:?}: {err}"
            );
        }
    }

    #[test]
    fn p0_002c5b_case_variants_of_a_stored_model_file_are_refused() {
        let root = std::env::temp_dir().join(format!("nexus-p0-002c5b-{}", uuid::Uuid::new_v4()));
        let model_dir = model_storage_dir(&root, "org/model");
        std::fs::create_dir_all(model_dir.join("gguf")).unwrap();
        std::fs::write(model_dir.join("model.Q4_K_M.gguf"), b"stored").unwrap();
        std::fs::write(model_dir.join("gguf").join("q4.gguf"), b"stored").unwrap();
        let target = root.to_string_lossy().into_owned();
        // Refused before any directory, file or request.
        for alias in [
            "Model.Q4_K_M.gguf",
            "model.q4_k_m.gguf",
            "GGUF/q4.gguf",
            "gguf/Q4.gguf",
        ] {
            let error = download_model_file("org/model", alias, &target, |_| {}).unwrap_err();
            assert!(error.contains("letter case"), "{alias}: {error}");
        }
        assert_eq!(
            std::fs::read(model_dir.join("model.Q4_K_M.gguf")).unwrap(),
            b"stored"
        );
        assert_eq!(std::fs::read_dir(&model_dir).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
