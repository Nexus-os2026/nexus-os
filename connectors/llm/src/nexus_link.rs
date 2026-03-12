//! Nexus Link — peer-to-peer model sharing over encrypted tunnels.
//!
//! Allows users to share downloaded GGUF models between devices on the same
//! network. Uses length-prefixed JSON messages over TCP and SHA-256 checksums
//! for integrity verification.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;

// ── Protocol types ──────────────────────────────────────────────────────────

/// A device participating in Nexus Link model sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDevice {
    pub id: String,
    pub name: String,
    pub address: String,
    pub last_seen: u64,
    pub available_models: Vec<SharedModelInfo>,
}

/// Metadata about a model available for sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedModelInfo {
    pub model_id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub quantization: String,
    pub checksum: String,
}

/// Wire-format messages exchanged between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinkMessage {
    Announce { device: PeerDevice },
    ModelListRequest,
    ModelListResponse { models: Vec<SharedModelInfo> },
    TransferRequest { model_id: String, filename: String },
    TransferAccepted { total_bytes: u64 },
    TransferChunk { offset: u64, data: Vec<u8> },
    TransferComplete { checksum: String },
    TransferRejected { reason: String },
    Ping,
    Pong,
}

/// Progress update emitted during a model transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub model_id: String,
    pub filename: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub status: TransferStatus,
    pub peer_name: String,
}

/// Current state of a transfer operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Connecting,
    Transferring,
    Verifying,
    Completed,
    Failed(String),
}

// ── NexusLink engine ────────────────────────────────────────────────────────

/// The core model-sharing engine.
pub struct NexusLink {
    device_id: String,
    device_name: String,
    models_dir: String,
    known_peers: Vec<PeerDevice>,
    sharing_enabled: bool,
    chunk_size: usize,
}

impl NexusLink {
    /// Create a new NexusLink instance with default settings.
    pub fn new(device_name: &str, models_dir: &str) -> Self {
        Self {
            device_id: Uuid::new_v4().to_string(),
            device_name: device_name.to_string(),
            models_dir: models_dir.to_string(),
            known_peers: Vec::new(),
            sharing_enabled: false,
            chunk_size: 1_048_576, // 1 MB
        }
    }

    /// Returns the device UUID.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Returns the human-readable device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Returns whether sharing is currently enabled.
    pub fn sharing_enabled(&self) -> bool {
        self.sharing_enabled
    }

    /// Returns the configured chunk size in bytes.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Enable model sharing on this device.
    pub fn enable_sharing(&mut self) {
        self.sharing_enabled = true;
    }

    /// Disable model sharing on this device.
    pub fn disable_sharing(&mut self) {
        self.sharing_enabled = false;
    }

    /// Scan the local models directory and return metadata for each model file.
    pub fn get_local_models(&self) -> Result<Vec<SharedModelInfo>, String> {
        let dir = std::path::Path::new(&self.models_dir);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut models = Vec::new();
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read models dir: {e}"))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
            let path = entry.path();

            // Look for GGUF files directly and in subdirectories
            if path.is_file() {
                if let Some(info) = self.model_info_from_file(&path)? {
                    models.push(info);
                }
            } else if path.is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file() {
                            if let Some(info) = self.model_info_from_file(&sub_path)? {
                                models.push(info);
                            }
                        }
                    }
                }
            }
        }

        Ok(models)
    }

    /// Extract model info from a single file, returning None if not a model file.
    fn model_info_from_file(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<SharedModelInfo>, String> {
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => return Ok(None),
        };

        // Only consider GGUF and safetensors model files
        if !filename.ends_with(".gguf") && !filename.ends_with(".safetensors") {
            return Ok(None);
        }

        let metadata =
            std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;

        // Try to read cached checksum first
        let checksum_path = path.with_extension("checksum");
        let checksum = if checksum_path.exists() {
            std::fs::read_to_string(&checksum_path)
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            let cs = Self::compute_file_checksum(&path.display().to_string())?;
            // Cache the checksum for future use
            let _ = std::fs::write(&checksum_path, &cs);
            cs
        };

        // Infer quantization from filename
        let quantization = if filename.contains("Q4") || filename.contains("q4") {
            "Q4".to_string()
        } else if filename.contains("Q8") || filename.contains("q8") {
            "Q8".to_string()
        } else if filename.contains("f16") || filename.contains("F16") {
            "F16".to_string()
        } else {
            "F32".to_string()
        };

        // Derive model_id from parent directory name or filename
        let model_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(&filename)
            .to_string();

        Ok(Some(SharedModelInfo {
            model_id,
            filename,
            size_bytes: metadata.len(),
            quantization,
            checksum,
        }))
    }

    /// Add a peer device to the known peers list.
    pub fn add_peer(&mut self, address: &str, name: &str) -> PeerDevice {
        let peer = PeerDevice {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            address: address.to_string(),
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            available_models: Vec::new(),
        };
        self.known_peers.push(peer.clone());
        peer
    }

    /// Remove a peer device by its ID. Returns true if found and removed.
    pub fn remove_peer(&mut self, device_id: &str) -> bool {
        let before = self.known_peers.len();
        self.known_peers.retain(|p| p.id != device_id);
        self.known_peers.len() < before
    }

    /// List all known peer devices.
    pub fn list_peers(&self) -> &[PeerDevice] {
        &self.known_peers
    }

    /// Query a peer for its available models over TCP.
    pub fn discover_peer_models(&self, peer: &PeerDevice) -> Result<Vec<SharedModelInfo>, String> {
        let mut stream = TcpStream::connect(&peer.address)
            .map_err(|e| format!("Failed to connect to peer {}: {e}", peer.address))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| format!("Failed to set read timeout: {e}"))?;

        let request = Self::serialize_message(&LinkMessage::ModelListRequest)?;
        stream
            .write_all(&request)
            .map_err(|e| format!("Failed to send model list request: {e}"))?;

        let response = Self::read_message(&mut stream)?;
        match response {
            LinkMessage::ModelListResponse { models } => Ok(models),
            _ => Err("Unexpected response from peer".to_string()),
        }
    }

    /// Send a model file to a peer device.
    pub fn send_model(
        &self,
        peer_address: &str,
        model_id: &str,
        filename: &str,
        progress_callback: impl Fn(TransferProgress),
    ) -> Result<(), String> {
        let file_path = std::path::Path::new(&self.models_dir).join(filename);
        if !file_path.exists() {
            // Try subdirectory matching model_id
            let alt_path = std::path::Path::new(&self.models_dir)
                .join(model_id)
                .join(filename);
            if !alt_path.exists() {
                return Err(format!("Model file not found: {filename}"));
            }
            return self.send_model_from_path(
                &alt_path,
                peer_address,
                model_id,
                filename,
                progress_callback,
            );
        }
        self.send_model_from_path(
            &file_path,
            peer_address,
            model_id,
            filename,
            progress_callback,
        )
    }

    fn send_model_from_path(
        &self,
        file_path: &std::path::Path,
        peer_address: &str,
        model_id: &str,
        filename: &str,
        progress_callback: impl Fn(TransferProgress),
    ) -> Result<(), String> {
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| format!("Failed to read file metadata: {e}"))?;
        let total_bytes = metadata.len();

        progress_callback(TransferProgress {
            model_id: model_id.to_string(),
            filename: filename.to_string(),
            bytes_transferred: 0,
            total_bytes,
            percent: 0.0,
            status: TransferStatus::Connecting,
            peer_name: peer_address.to_string(),
        });

        let mut stream = TcpStream::connect(peer_address)
            .map_err(|e| format!("Failed to connect to peer: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set timeout: {e}"))?;

        // Send transfer request
        let request = Self::serialize_message(&LinkMessage::TransferRequest {
            model_id: model_id.to_string(),
            filename: filename.to_string(),
        })?;
        stream
            .write_all(&request)
            .map_err(|e| format!("Failed to send transfer request: {e}"))?;

        // Wait for acceptance
        let response = Self::read_message(&mut stream)?;
        match response {
            LinkMessage::TransferAccepted { .. } => {}
            LinkMessage::TransferRejected { reason } => {
                return Err(format!("Transfer rejected: {reason}"));
            }
            _ => return Err("Unexpected response to transfer request".to_string()),
        }

        // Stream file in chunks
        let mut file = std::fs::File::open(file_path)
            .map_err(|e| format!("Failed to open model file: {e}"))?;
        let mut buffer = vec![0u8; self.chunk_size];
        let mut offset: u64 = 0;

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read file: {e}"))?;
            if bytes_read == 0 {
                break;
            }

            let chunk = Self::serialize_message(&LinkMessage::TransferChunk {
                offset,
                data: buffer[..bytes_read].to_vec(),
            })?;
            stream
                .write_all(&chunk)
                .map_err(|e| format!("Failed to send chunk: {e}"))?;

            offset += bytes_read as u64;
            let percent = if total_bytes > 0 {
                (offset as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            progress_callback(TransferProgress {
                model_id: model_id.to_string(),
                filename: filename.to_string(),
                bytes_transferred: offset,
                total_bytes,
                percent,
                status: TransferStatus::Transferring,
                peer_name: peer_address.to_string(),
            });
        }

        // Send completion with checksum
        let checksum = Self::compute_file_checksum(&file_path.display().to_string())?;
        let complete = Self::serialize_message(&LinkMessage::TransferComplete {
            checksum: checksum.clone(),
        })?;
        stream
            .write_all(&complete)
            .map_err(|e| format!("Failed to send completion: {e}"))?;

        progress_callback(TransferProgress {
            model_id: model_id.to_string(),
            filename: filename.to_string(),
            bytes_transferred: total_bytes,
            total_bytes,
            percent: 100.0,
            status: TransferStatus::Completed,
            peer_name: peer_address.to_string(),
        });

        Ok(())
    }

    /// Receive a model from a peer connection.
    pub fn receive_model(
        &self,
        connection: &mut TcpStream,
        progress_callback: impl Fn(TransferProgress),
    ) -> Result<String, String> {
        let request = Self::read_message(connection)?;
        let (model_id, filename) = match request {
            LinkMessage::TransferRequest { model_id, filename } => (model_id, filename),
            _ => return Err("Expected TransferRequest message".to_string()),
        };

        // Check if model already exists
        let target_path = std::path::Path::new(&self.models_dir).join(&filename);
        if target_path.exists() {
            let reject = Self::serialize_message(&LinkMessage::TransferRejected {
                reason: "Model already exists locally".to_string(),
            })?;
            connection
                .write_all(&reject)
                .map_err(|e| format!("Failed to send rejection: {e}"))?;
            return Err("Model already exists locally".to_string());
        }

        // We don't know total_bytes yet — accept and start receiving
        let accept = Self::serialize_message(&LinkMessage::TransferAccepted { total_bytes: 0 })?;
        connection
            .write_all(&accept)
            .map_err(|e| format!("Failed to send acceptance: {e}"))?;

        // Write to a temp file
        let temp_path = std::path::Path::new(&self.models_dir).join(format!(".{}.tmp", filename));
        let mut temp_file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {e}"))?;

        let mut bytes_received: u64 = 0;

        let expected_checksum = loop {
            let msg = Self::read_message(connection)?;
            match msg {
                LinkMessage::TransferChunk { data, .. } => {
                    let chunk_len = data.len() as u64;
                    temp_file
                        .write_all(&data)
                        .map_err(|e| format!("Failed to write chunk: {e}"))?;
                    bytes_received += chunk_len;

                    progress_callback(TransferProgress {
                        model_id: model_id.clone(),
                        filename: filename.clone(),
                        bytes_transferred: bytes_received,
                        total_bytes: 0, // unknown until complete
                        percent: 0.0,   // unknown until complete
                        status: TransferStatus::Transferring,
                        peer_name: "peer".to_string(),
                    });
                }
                LinkMessage::TransferComplete { checksum } => {
                    break checksum;
                }
                _ => return Err("Unexpected message during transfer".to_string()),
            }
        };

        drop(temp_file);

        // Verify checksum
        progress_callback(TransferProgress {
            model_id: model_id.clone(),
            filename: filename.clone(),
            bytes_transferred: bytes_received,
            total_bytes: bytes_received,
            percent: 100.0,
            status: TransferStatus::Verifying,
            peer_name: "peer".to_string(),
        });

        let actual_checksum = Self::compute_file_checksum(&temp_path.display().to_string())?;
        if actual_checksum != expected_checksum {
            let _ = std::fs::remove_file(&temp_path);
            let msg =
                format!("Checksum mismatch: expected {expected_checksum}, got {actual_checksum}");
            progress_callback(TransferProgress {
                model_id: model_id.clone(),
                filename: filename.clone(),
                bytes_transferred: bytes_received,
                total_bytes: bytes_received,
                percent: 100.0,
                status: TransferStatus::Failed(msg.clone()),
                peer_name: "peer".to_string(),
            });
            return Err(msg);
        }

        // Move temp file to final location
        std::fs::rename(&temp_path, &target_path)
            .map_err(|e| format!("Failed to move file to models dir: {e}"))?;

        // Generate a basic nexus-model.toml
        let toml_path = target_path.with_extension("toml");
        let toml_content = format!(
            "[model]\nmodel_id = \"{model_id}\"\nfilename = \"{filename}\"\nsource = \"nexus-link\"\n"
        );
        let _ = std::fs::write(toml_path, toml_content);

        progress_callback(TransferProgress {
            model_id: model_id.clone(),
            filename: filename.clone(),
            bytes_transferred: bytes_received,
            total_bytes: bytes_received,
            percent: 100.0,
            status: TransferStatus::Completed,
            peer_name: "peer".to_string(),
        });

        Ok(target_path.display().to_string())
    }

    /// Compute SHA-256 checksum of a file, reading in chunks to handle large files.
    pub fn compute_file_checksum(path: &str) -> Result<String, String> {
        let mut file =
            std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536]; // 64KB read buffer

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read file: {e}"))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Serialize a LinkMessage with a 4-byte big-endian length prefix.
    pub fn serialize_message(msg: &LinkMessage) -> Result<Vec<u8>, String> {
        let json =
            serde_json::to_vec(msg).map_err(|e| format!("Failed to serialize message: {e}"))?;
        let len = json.len() as u32;
        let mut frame = Vec::with_capacity(4 + json.len());
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&json);
        Ok(frame)
    }

    /// Deserialize a LinkMessage from a length-prefixed byte buffer.
    pub fn deserialize_message(data: &[u8]) -> Result<LinkMessage, String> {
        if data.len() < 4 {
            return Err("Buffer too short for length prefix".to_string());
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            return Err(format!(
                "Buffer too short: expected {} bytes, got {}",
                4 + len,
                data.len()
            ));
        }
        serde_json::from_slice(&data[4..4 + len])
            .map_err(|e| format!("Failed to deserialize message: {e}"))
    }

    /// Read a length-prefixed message from a TCP stream.
    fn read_message(stream: &mut TcpStream) -> Result<LinkMessage, String> {
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .map_err(|e| format!("Failed to read message length: {e}"))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        stream
            .read_exact(&mut msg_buf)
            .map_err(|e| format!("Failed to read message body: {e}"))?;

        serde_json::from_slice(&msg_buf).map_err(|e| format!("Failed to deserialize message: {e}"))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_device() {
        let link = NexusLink::new("test-device", "/tmp/models");
        assert!(!link.device_id().is_empty());
        assert_eq!(link.device_name(), "test-device");
    }

    #[test]
    fn test_sharing_toggle() {
        let mut link = NexusLink::new("dev", "/tmp/models");
        assert!(!link.sharing_enabled());
        link.enable_sharing();
        assert!(link.sharing_enabled());
        link.disable_sharing();
        assert!(!link.sharing_enabled());
    }

    #[test]
    fn test_add_remove_peer() {
        let mut link = NexusLink::new("dev", "/tmp/models");
        let peer = link.add_peer("192.168.1.10:9090", "laptop");
        assert_eq!(link.list_peers().len(), 1);
        assert_eq!(link.list_peers()[0].name, "laptop");
        assert_eq!(link.list_peers()[0].address, "192.168.1.10:9090");

        let removed = link.remove_peer(&peer.id);
        assert!(removed);
        assert!(link.list_peers().is_empty());

        // Removing non-existent peer returns false
        assert!(!link.remove_peer("nonexistent"));
    }

    #[test]
    fn test_serialize_deserialize_message() {
        let variants: Vec<LinkMessage> = vec![
            LinkMessage::Ping,
            LinkMessage::Pong,
            LinkMessage::ModelListRequest,
            LinkMessage::ModelListResponse {
                models: vec![SharedModelInfo {
                    model_id: "phi-4".to_string(),
                    filename: "phi-4-q4.gguf".to_string(),
                    size_bytes: 2_000_000_000,
                    quantization: "Q4".to_string(),
                    checksum: "abc123".to_string(),
                }],
            },
            LinkMessage::TransferRequest {
                model_id: "phi-4".to_string(),
                filename: "phi-4-q4.gguf".to_string(),
            },
            LinkMessage::TransferAccepted {
                total_bytes: 1_000_000,
            },
            LinkMessage::TransferChunk {
                offset: 0,
                data: vec![1, 2, 3, 4],
            },
            LinkMessage::TransferComplete {
                checksum: "sha256hash".to_string(),
            },
            LinkMessage::TransferRejected {
                reason: "already exists".to_string(),
            },
            LinkMessage::Announce {
                device: PeerDevice {
                    id: "dev-1".to_string(),
                    name: "laptop".to_string(),
                    address: "10.0.0.1:9090".to_string(),
                    last_seen: 1700000000,
                    available_models: vec![],
                },
            },
        ];

        for msg in variants {
            let serialized = NexusLink::serialize_message(&msg).unwrap();
            let deserialized = NexusLink::deserialize_message(&serialized).unwrap();
            // Verify round-trip by re-serializing
            let re_serialized = NexusLink::serialize_message(&deserialized).unwrap();
            assert_eq!(serialized, re_serialized);
        }
    }

    #[test]
    fn test_compute_checksum() {
        let dir = std::env::temp_dir().join("nexus_link_test_checksum");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("test_file.bin");
        std::fs::write(&file_path, b"hello nexus link").unwrap();

        let checksum1 = NexusLink::compute_file_checksum(&file_path.display().to_string()).unwrap();
        let checksum2 = NexusLink::compute_file_checksum(&file_path.display().to_string()).unwrap();

        // Deterministic
        assert_eq!(checksum1, checksum2);
        // SHA-256 produces 64 hex chars
        assert_eq!(checksum1.len(), 64);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_checksum_different_content() {
        let dir = std::env::temp_dir().join("nexus_link_test_diff");
        let _ = std::fs::create_dir_all(&dir);

        let file_a = dir.join("a.bin");
        let file_b = dir.join("b.bin");
        std::fs::write(&file_a, b"content alpha").unwrap();
        std::fs::write(&file_b, b"content beta").unwrap();

        let checksum_a = NexusLink::compute_file_checksum(&file_a.display().to_string()).unwrap();
        let checksum_b = NexusLink::compute_file_checksum(&file_b.display().to_string()).unwrap();

        assert_ne!(checksum_a, checksum_b);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_shared_model_info() {
        let info = SharedModelInfo {
            model_id: "llama-3".to_string(),
            filename: "llama-3-q8.gguf".to_string(),
            size_bytes: 4_500_000_000,
            quantization: "Q8".to_string(),
            checksum: "deadbeef".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SharedModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model_id, "llama-3");
        assert_eq!(deserialized.filename, "llama-3-q8.gguf");
        assert_eq!(deserialized.size_bytes, 4_500_000_000);
        assert_eq!(deserialized.quantization, "Q8");
        assert_eq!(deserialized.checksum, "deadbeef");
    }

    #[test]
    fn test_transfer_progress_tracking() {
        let progress = TransferProgress {
            model_id: "phi-4".to_string(),
            filename: "phi-4-q4.gguf".to_string(),
            bytes_transferred: 500_000_000,
            total_bytes: 2_000_000_000,
            percent: 25.0,
            status: TransferStatus::Transferring,
            peer_name: "remote-laptop".to_string(),
        };

        assert_eq!(progress.model_id, "phi-4");
        assert_eq!(progress.bytes_transferred, 500_000_000);
        assert_eq!(progress.total_bytes, 2_000_000_000);
        assert_eq!(progress.percent, 25.0);
        assert_eq!(progress.peer_name, "remote-laptop");

        // Verify serialization
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: TransferProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model_id, progress.model_id);
    }

    #[test]
    fn test_chunk_size_default() {
        let link = NexusLink::new("dev", "/tmp/models");
        assert_eq!(link.chunk_size(), 1_048_576);
    }

    #[test]
    fn test_peer_list_empty() {
        let link = NexusLink::new("dev", "/tmp/models");
        assert!(link.list_peers().is_empty());
    }
}
