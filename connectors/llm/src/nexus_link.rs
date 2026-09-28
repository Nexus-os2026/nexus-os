//! Nexus Link — peer-to-peer model sharing over encrypted tunnels.
//!
//! Allows users to share downloaded GGUF models between devices on the same
//! network. Uses length-prefixed JSON messages over TCP and SHA-256 checksums
//! for integrity verification. Supports optional AES-256-GCM encryption and
//! HMAC-SHA256 challenge-response peer authentication.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Global nonce counter — ensures no two encryptions in the same process
/// ever reuse a nonce, even if the system clock is coarse.
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Wire-format flag: message payload is plaintext JSON.
const FLAG_PLAINTEXT: u8 = 0x00;
/// Wire-format flag: message payload is AES-256-GCM encrypted.
const FLAG_ENCRYPTED: u8 = 0x01;

/// Largest message a peer may announce (P0-002C5C). A transfer chunk is
/// `chunk_size` (1 MiB) of data plus framing; a longer length prefix is
/// refused before anything is allocated for it.
const MAX_LINK_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// How long connecting to a peer may take.
const PEER_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Connect to exactly `address`, the socket address the peer policy admitted
/// (Final Gate item F), within [`PEER_CONNECT_TIMEOUT`], with read and write
/// timeouts on the stream. No name is resolved here or anywhere on the way:
/// a DNS answer is not a peer grant.
fn connect_peer(
    address: std::net::SocketAddr,
    io_timeout: std::time::Duration,
) -> Result<TcpStream, String> {
    let stream = TcpStream::connect_timeout(&address, PEER_CONNECT_TIMEOUT)
        .map_err(|e| format!("Failed to connect to peer {address}: {e}"))?;
    stream
        .set_read_timeout(Some(io_timeout))
        .and_then(|()| stream.set_write_timeout(Some(io_timeout)))
        .map_err(|e| format!("Failed to set timeout: {e}"))?;
    Ok(stream)
}

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
    /// Optional AES-256-GCM encryption key for wire encryption.
    encryption_key: Option<[u8; 32]>,
    /// Optional shared secret for HMAC-SHA256 challenge-response authentication.
    shared_secret: Option<String>,
    /// The peer policy: the IP socket addresses (`ip:port`, `[ipv6]:port`) a
    /// transfer may reach. Empty admits no peer (Final Gate item F).
    allowed_peers: Vec<String>,
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
            encryption_key: None,
            shared_secret: None,
            allowed_peers: Vec::new(),
        }
    }

    /// Set the peer policy: the IP socket addresses a transfer may reach. An
    /// entry that is not an IP socket address admits nothing.
    pub fn set_allowed_peers(&mut self, peers: Vec<String>) {
        self.allowed_peers = peers;
    }

    /// Governance (Final Gate item F): the socket address a transfer may
    /// connect to. A peer address is not authority, so:
    /// - an empty peer policy admits no peer;
    /// - a peer is named by its IP socket address and must equal a policy
    ///   entry, compared as socket addresses (so `[::1]:9` and
    ///   `[0:0:0:0:0:0:0:1]:9` are one peer, and `127.0.0.1:9` is not
    ///   `[::ffff:127.0.0.1]:9`);
    /// - a host name is never resolved, and neither a host without its port
    ///   nor any other spelling is repaired into an address.
    ///
    /// The address returned is the only one the transfer connects to.
    fn check_peer_allowed(&self, address: &str) -> Result<std::net::SocketAddr, String> {
        if self.allowed_peers.is_empty() {
            return Err(
                "Governance denied: the peer policy is empty, so it admits no peer".to_string(),
            );
        }
        let requested: std::net::SocketAddr = address.parse().map_err(|_| {
            "Governance denied: a peer is named by its IP socket address; a host name is not resolved"
                .to_string()
        })?;
        let admitted = self
            .allowed_peers
            .iter()
            .filter_map(|entry| entry.parse::<std::net::SocketAddr>().ok())
            .any(|allowed| allowed == requested);
        if admitted {
            Ok(requested)
        } else {
            Err("Governance denied: the peer address is not in the peer policy".to_string())
        }
    }

    /// Governance (Final Gate item F): a transfer needs peer authentication
    /// and wire encryption both configured. Without a shared secret the
    /// handshake is skipped, and without a key every message is plaintext,
    /// so such a transfer is refused before any connection is made. Phase
    /// Zero builds no pairing: nothing in the desktop configures either.
    fn require_authenticated_transport(&self) -> Result<(), String> {
        if self.shared_secret.is_some() && self.encryption_key.is_some() {
            Ok(())
        } else {
            Err("Governance denied: unauthenticated Nexus Link transfer is unavailable; no shared secret and encryption key are configured".to_string())
        }
    }

    /// Derive and set an AES-256-GCM encryption key from a passphrase using SHA-256.
    /// When set, all wire messages will be encrypted.
    pub fn set_encryption_key(&mut self, passphrase: &str) {
        let mut hasher = Sha256::new();
        hasher.update(passphrase.as_bytes());
        self.encryption_key = Some(hasher.finalize().into());
    }

    /// Set the shared secret used for HMAC-SHA256 challenge-response peer authentication.
    /// When set, peers must authenticate before exchanging messages.
    pub fn set_shared_secret(&mut self, secret: &str) {
        self.shared_secret = Some(secret.to_string());
    }

    /// Perform challenge-response authentication as the initiator (client side).
    /// Sends a random 32-byte challenge, expects HMAC-SHA256(challenge, secret) back.
    pub fn authenticate_as_initiator(&self, stream: &mut TcpStream) -> Result<(), String> {
        let secret = match &self.shared_secret {
            Some(s) => s,
            None => return Ok(()), // No auth configured — skip
        };

        // Generate random 32-byte challenge using timestamp + counter + device_id
        let challenge = Self::generate_challenge();

        // Send the 32-byte challenge
        stream
            .write_all(&challenge)
            .map_err(|e| format!("Failed to send auth challenge: {e}"))?;

        // Read 32-byte HMAC response
        let mut response = [0u8; 32];
        stream
            .read_exact(&mut response)
            .map_err(|e| format!("Failed to read auth response: {e}"))?;

        // Verify HMAC
        let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("HMAC error: {e}"))?;
        mac.update(&challenge);
        mac.verify_slice(&response)
            .map_err(|_| "Peer authentication failed — HMAC mismatch".to_string())?;

        Ok(())
    }

    /// Perform challenge-response authentication as the responder (server side).
    /// Reads a 32-byte challenge, responds with HMAC-SHA256(challenge, secret).
    pub fn authenticate_as_responder(&self, stream: &mut TcpStream) -> Result<(), String> {
        let secret = match &self.shared_secret {
            Some(s) => s,
            None => return Ok(()), // No auth configured — skip
        };

        // Read 32-byte challenge
        let mut challenge = [0u8; 32];
        stream
            .read_exact(&mut challenge)
            .map_err(|e| format!("Failed to read auth challenge: {e}"))?;

        // Compute HMAC-SHA256(challenge, secret)
        let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("HMAC error: {e}"))?;
        mac.update(&challenge);
        let result = mac.finalize().into_bytes();

        // Send 32-byte HMAC response
        stream
            .write_all(&result)
            .map_err(|e| format!("Failed to send auth response: {e}"))?;

        Ok(())
    }

    /// Generate a 32-byte challenge using timestamp + atomic counter.
    fn generate_challenge() -> [u8; 32] {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let mut hasher = Sha256::new();
        hasher.update(ts.to_le_bytes());
        hasher.update(counter.to_le_bytes());
        hasher.update(b"nexus-link-challenge");
        hasher.finalize().into()
    }

    /// Generate a unique 96-bit (12-byte) nonce for AES-256-GCM.
    fn generate_nonce() -> [u8; 12] {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let mut hasher = Sha256::new();
        hasher.update(ts.to_le_bytes());
        hasher.update(counter.to_le_bytes());
        let hash = hasher.finalize();

        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&hash[..12]);
        nonce
    }

    /// Encrypt data with AES-256-GCM. Returns nonce (12 bytes) || ciphertext || tag.
    fn encrypt_payload(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(key.into());
        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = cipher
            .encrypt(&nonce, data)
            .map_err(|e| format!("AES-256-GCM encryption failed: {e}"))?;

        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt data (nonce || ciphertext || tag) with AES-256-GCM.
    fn decrypt_payload(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        if encrypted.len() < 28 {
            return Err(
                "Encrypted data too short — need at least nonce + tag (28 bytes)".to_string(),
            );
        }

        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce_arr: [u8; 12] = nonce_bytes
            .try_into()
            .map_err(|_| "invalid nonce length".to_string())?;
        let nonce = Nonce::from(nonce_arr);
        let cipher = Aes256Gcm::new(key.into());

        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| "AES-256-GCM decryption failed — wrong key or tampered data".to_string())
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
            eprintln!(
                "[nexus-link][governance] write_checksum path={}",
                checksum_path.display()
            );
            // Best-effort: cache checksum for faster future scans
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

    /// Query a peer for its available models over TCP: only a peer the
    /// policy admits, over an authenticated, encrypted link.
    pub fn discover_peer_models(&self, peer: &PeerDevice) -> Result<Vec<SharedModelInfo>, String> {
        let address = self.check_peer_allowed(&peer.address)?;
        self.require_authenticated_transport()?;
        eprintln!("[nexus-link][governance] tcp_connect peer={address} op=discover_models");
        let mut stream = connect_peer(address, std::time::Duration::from_secs(10))?;

        // Authenticate before exchanging messages
        self.authenticate_as_initiator(&mut stream)?;

        let request = self.serialize_message(&LinkMessage::ModelListRequest)?;
        stream
            .write_all(&request)
            .map_err(|e| format!("Failed to send model list request: {e}"))?;

        let response = self.read_message(&mut stream)?;
        match response {
            LinkMessage::ModelListResponse { models } => Ok(models),
            _ => Err("Unexpected response from peer".to_string()),
        }
    }

    /// Send a model file to a peer device: only a regular file beneath the
    /// models directory, only to a peer the policy admits, and only over an
    /// authenticated, encrypted link. Each check runs before any connection.
    pub fn send_model(
        &self,
        peer_address: &str,
        model_id: &str,
        filename: &str,
        progress_callback: impl Fn(TransferProgress),
    ) -> Result<(), String> {
        // P0-002C5B: the file is named only by a validated relative path
        // beneath the models directory (directly or inside the model's own
        // directory); every step must be a real directory and the target a
        // regular file, with no link followed anywhere.
        let root = std::path::Path::new(&self.models_dir);
        let file_path = nexus_kernel::governed_path::regular_file_beneath(root, filename)
            .or_else(|_| {
                nexus_kernel::governed_path::regular_file_beneath(
                    root,
                    &format!("{model_id}/{filename}"),
                )
            })
            .map_err(|_| "Model file not found beneath the models directory".to_string())?;
        // Final Gate item F: the peer policy and an authenticated transport.
        let address = self.check_peer_allowed(peer_address)?;
        self.require_authenticated_transport()?;
        self.send_model_from_path(&file_path, address, model_id, filename, progress_callback)
    }

    fn send_model_from_path(
        &self,
        file_path: &std::path::Path,
        address: std::net::SocketAddr,
        model_id: &str,
        filename: &str,
        progress_callback: impl Fn(TransferProgress),
    ) -> Result<(), String> {
        let peer_address = address.to_string();
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
            peer_name: peer_address.clone(),
        });

        eprintln!(
            "[nexus-link][governance] tcp_connect peer={peer_address} op=send_model model_id={model_id}"
        );
        let mut stream = connect_peer(address, std::time::Duration::from_secs(30))?;

        // Authenticate before exchanging messages
        self.authenticate_as_initiator(&mut stream)?;

        // Send transfer request
        let request = self.serialize_message(&LinkMessage::TransferRequest {
            model_id: model_id.to_string(),
            filename: filename.to_string(),
        })?;
        stream
            .write_all(&request)
            .map_err(|e| format!("Failed to send transfer request: {e}"))?;

        // Wait for acceptance
        let response = self.read_message(&mut stream)?;
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

            let chunk = self.serialize_message(&LinkMessage::TransferChunk {
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
        let complete = self.serialize_message(&LinkMessage::TransferComplete {
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
        let request = self.read_message(connection)?;
        let (model_id, filename) = match request {
            LinkMessage::TransferRequest { model_id, filename } => (model_id, filename),
            _ => return Err("Expected TransferRequest message".to_string()),
        };

        // Governance: reject filenames with path traversal components
        if filename.contains("..") || filename.starts_with('/') || filename.contains('\\') {
            let reject = self.serialize_message(&LinkMessage::TransferRejected {
                reason: "Filename contains path traversal characters".to_string(),
            })?;
            connection
                .write_all(&reject)
                .map_err(|e| format!("Failed to send rejection: {e}"))?;
            return Err(format!(
                "Governance denied: filename '{filename}' contains path traversal"
            ));
        }

        // Check if model already exists
        let target_path = std::path::Path::new(&self.models_dir).join(&filename);
        if target_path.exists() {
            let reject = self.serialize_message(&LinkMessage::TransferRejected {
                reason: "Model already exists locally".to_string(),
            })?;
            connection
                .write_all(&reject)
                .map_err(|e| format!("Failed to send rejection: {e}"))?;
            return Err("Model already exists locally".to_string());
        }

        // We don't know total_bytes yet — accept and start receiving
        let accept = self.serialize_message(&LinkMessage::TransferAccepted { total_bytes: 0 })?;
        connection
            .write_all(&accept)
            .map_err(|e| format!("Failed to send acceptance: {e}"))?;

        // Write to a temp file
        let temp_path = std::path::Path::new(&self.models_dir).join(format!(".{}.tmp", filename));
        eprintln!(
            "[nexus-link][governance] receive_model temp_file={} model_id={model_id}",
            temp_path.display()
        );
        let mut temp_file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {e}"))?;

        let mut bytes_received: u64 = 0;

        let expected_checksum = loop {
            let msg = self.read_message(connection)?;
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
            // Best-effort: clean up corrupted temp file after checksum mismatch
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
        eprintln!(
            "[nexus-link][governance] write_model_metadata path={}",
            toml_path.display()
        );
        // Best-effort: write model metadata for registry discovery
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

    /// Serialize a LinkMessage with a 1-byte encryption flag and 4-byte big-endian length prefix.
    ///
    /// Wire format: `flag (1 byte) | length (4 bytes BE) | payload`.
    /// - flag 0x00: payload is plaintext JSON
    /// - flag 0x01: payload is AES-256-GCM encrypted (nonce || ciphertext || tag)
    pub fn serialize_message(&self, msg: &LinkMessage) -> Result<Vec<u8>, String> {
        let json =
            serde_json::to_vec(msg).map_err(|e| format!("Failed to serialize message: {e}"))?;

        let (flag, payload) = if let Some(key) = &self.encryption_key {
            let encrypted = Self::encrypt_payload(&json, key)?;
            (FLAG_ENCRYPTED, encrypted)
        } else {
            (FLAG_PLAINTEXT, json)
        };

        let len = payload.len() as u32;
        let mut frame = Vec::with_capacity(1 + 4 + payload.len());
        frame.push(flag);
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    /// Serialize a LinkMessage without encryption (static method for backward compatibility).
    pub fn serialize_message_plaintext(msg: &LinkMessage) -> Result<Vec<u8>, String> {
        let json =
            serde_json::to_vec(msg).map_err(|e| format!("Failed to serialize message: {e}"))?;
        let len = json.len() as u32;
        let mut frame = Vec::with_capacity(1 + 4 + json.len());
        frame.push(FLAG_PLAINTEXT);
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&json);
        Ok(frame)
    }

    /// Deserialize a LinkMessage from a flag + length-prefixed byte buffer.
    pub fn deserialize_message(&self, data: &[u8]) -> Result<LinkMessage, String> {
        if data.len() < 5 {
            return Err("Buffer too short for flag + length prefix".to_string());
        }

        let flag = data[0];
        let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
        if data.len() < 5 + len {
            return Err(format!(
                "Buffer too short: expected {} bytes, got {}",
                5 + len,
                data.len()
            ));
        }

        let payload = &data[5..5 + len];

        let json_bytes = if flag == FLAG_ENCRYPTED {
            let key = self.encryption_key.as_ref().ok_or_else(|| {
                "Received encrypted message but no encryption key is set".to_string()
            })?;
            Self::decrypt_payload(payload, key)?
        } else {
            payload.to_vec()
        };

        serde_json::from_slice(&json_bytes)
            .map_err(|e| format!("Failed to deserialize message: {e}"))
    }

    /// Deserialize a LinkMessage without encryption (static method for backward compatibility).
    pub fn deserialize_message_plaintext(data: &[u8]) -> Result<LinkMessage, String> {
        if data.len() < 5 {
            return Err("Buffer too short for flag + length prefix".to_string());
        }

        let flag = data[0];
        if flag == FLAG_ENCRYPTED {
            return Err(
                "Message is encrypted but no key available (use instance method)".to_string(),
            );
        }

        let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
        if data.len() < 5 + len {
            return Err(format!(
                "Buffer too short: expected {} bytes, got {}",
                5 + len,
                data.len()
            ));
        }

        serde_json::from_slice(&data[5..5 + len])
            .map_err(|e| format!("Failed to deserialize message: {e}"))
    }

    /// Read a flag + length-prefixed message from a TCP stream.
    fn read_message(&self, stream: &mut TcpStream) -> Result<LinkMessage, String> {
        // Read the 1-byte flag
        let mut flag_buf = [0u8; 1];
        stream
            .read_exact(&mut flag_buf)
            .map_err(|e| format!("Failed to read message flag: {e}"))?;
        let flag = flag_buf[0];

        // Read 4-byte length
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .map_err(|e| format!("Failed to read message length: {e}"))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_LINK_MESSAGE_BYTES {
            return Err(format!(
                "Peer message of {len} bytes exceeds the {MAX_LINK_MESSAGE_BYTES}-byte limit"
            ));
        }

        // Read payload
        let mut payload = vec![0u8; len];
        stream
            .read_exact(&mut payload)
            .map_err(|e| format!("Failed to read message body: {e}"))?;

        let json_bytes = if flag == FLAG_ENCRYPTED {
            let key = self.encryption_key.as_ref().ok_or_else(|| {
                "Received encrypted message but no encryption key is set".to_string()
            })?;
            Self::decrypt_payload(&payload, key)?
        } else {
            payload
        };

        serde_json::from_slice(&json_bytes)
            .map_err(|e| format!("Failed to deserialize message: {e}"))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_002c5b_send_model_reads_only_regular_files_beneath_the_models_dir() {
        let base = std::env::temp_dir().join(format!("nexus-p0-002c5b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let root = base.canonicalize().unwrap();
        let models = root.join("models");
        std::fs::create_dir(&models).unwrap();
        std::fs::write(root.join("secret"), b"not a model").unwrap();
        let link = NexusLink::new("device", &models.to_string_lossy());
        for (model_id, filename) in [
            ("m", "../secret"),
            ("m", "/etc/passwd"),
            ("..", "secret"),
            ("m", "C:\\x"),
            ("../..", "etc/passwd"),
            ("m", ""),
        ] {
            let error = link
                .send_model("127.0.0.1:9", model_id, filename, |_| {})
                .unwrap_err();
            assert!(
                error.contains("not found beneath"),
                "{model_id:?} {filename:?}: {error}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("secret"), models.join("link.gguf")).unwrap();
            let error = link
                .send_model("127.0.0.1:9", "m", "link.gguf", |_| {})
                .unwrap_err();
            assert!(error.contains("not found beneath"), "{error}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

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
        let link = NexusLink::new("dev", "/tmp/models");
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
            let serialized = link.serialize_message(&msg).unwrap();
            let deserialized = link.deserialize_message(&serialized).unwrap();
            // Verify round-trip by re-serializing
            let re_serialized = link.serialize_message(&deserialized).unwrap();
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

    // ── AES-256-GCM encryption tests ────────────────────────────────────

    #[test]
    fn test_encryption_roundtrip() {
        let mut link = NexusLink::new("dev", "/tmp/models");
        link.set_encryption_key("my-secret-passphrase");

        let msg = LinkMessage::Ping;
        let serialized = link.serialize_message(&msg).unwrap();

        // First byte should be the encrypted flag
        assert_eq!(serialized[0], FLAG_ENCRYPTED);

        // Deserialize with same key
        let deserialized = link.deserialize_message(&serialized).unwrap();
        // Re-serialize plaintext to compare structure
        let plain_link = NexusLink::new("dev", "/tmp/models");
        let plain_ser = plain_link.serialize_message(&msg).unwrap();
        let plain_deser = plain_link.deserialize_message(&plain_ser).unwrap();

        // Both should deserialize to equivalent messages
        let json1 = serde_json::to_string(&deserialized).unwrap();
        let json2 = serde_json::to_string(&plain_deser).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_encrypted_serialize_deserialize_all_variants() {
        let mut link = NexusLink::new("dev", "/tmp/models");
        link.set_encryption_key("test-key-42");

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
            LinkMessage::TransferChunk {
                offset: 1024,
                data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            },
        ];

        for msg in variants {
            let serialized = link.serialize_message(&msg).unwrap();
            assert_eq!(serialized[0], FLAG_ENCRYPTED);
            let deserialized = link.deserialize_message(&serialized).unwrap();
            // Verify round-trip
            let json_orig = serde_json::to_string(&msg).unwrap();
            let json_rt = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json_orig, json_rt);
        }
    }

    #[test]
    fn test_encryption_wrong_key_fails() {
        let mut sender = NexusLink::new("sender", "/tmp/models");
        sender.set_encryption_key("correct-key");

        let mut receiver = NexusLink::new("receiver", "/tmp/models");
        receiver.set_encryption_key("wrong-key");

        let msg = LinkMessage::Ping;
        let serialized = sender.serialize_message(&msg).unwrap();

        // Should fail to decrypt with wrong key
        let result = receiver.deserialize_message(&serialized);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_message_differs_from_plaintext() {
        let plain_link = NexusLink::new("dev", "/tmp/models");
        let mut enc_link = NexusLink::new("dev", "/tmp/models");
        enc_link.set_encryption_key("secret");

        let msg = LinkMessage::Ping;
        let plain = plain_link.serialize_message(&msg).unwrap();
        let encrypted = enc_link.serialize_message(&msg).unwrap();

        // Flags differ
        assert_eq!(plain[0], FLAG_PLAINTEXT);
        assert_eq!(encrypted[0], FLAG_ENCRYPTED);

        // Payloads differ
        assert_ne!(plain[5..], encrypted[5..]);
    }

    #[test]
    fn test_nonce_uniqueness() {
        let mut link = NexusLink::new("dev", "/tmp/models");
        link.set_encryption_key("nonce-test");

        let msg = LinkMessage::Ping;
        let enc1 = link.serialize_message(&msg).unwrap();
        let enc2 = link.serialize_message(&msg).unwrap();

        // Different nonces should produce different ciphertext
        assert_ne!(enc1, enc2);

        // But both should decrypt to the same message
        let d1 = link.deserialize_message(&enc1).unwrap();
        let d2 = link.deserialize_message(&enc2).unwrap();
        assert_eq!(
            serde_json::to_string(&d1).unwrap(),
            serde_json::to_string(&d2).unwrap()
        );
    }

    #[test]
    fn test_no_encryption_key_receives_encrypted_message() {
        let mut sender = NexusLink::new("sender", "/tmp/models");
        sender.set_encryption_key("secret");

        let receiver = NexusLink::new("receiver", "/tmp/models");
        // receiver has no key

        let msg = LinkMessage::Ping;
        let serialized = sender.serialize_message(&msg).unwrap();

        let result = receiver.deserialize_message(&serialized);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no encryption key"));
    }

    #[test]
    fn test_plaintext_backward_compatibility() {
        // Plaintext serialization uses the static method
        let msg = LinkMessage::Ping;
        let serialized = NexusLink::serialize_message_plaintext(&msg).unwrap();

        // Should be readable by any instance (no key needed)
        let link = NexusLink::new("dev", "/tmp/models");
        let deserialized = link.deserialize_message(&serialized).unwrap();

        let json_orig = serde_json::to_string(&msg).unwrap();
        let json_rt = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json_orig, json_rt);

        // Also readable via static plaintext deserializer
        let deserialized2 = NexusLink::deserialize_message_plaintext(&serialized).unwrap();
        let json_rt2 = serde_json::to_string(&deserialized2).unwrap();
        assert_eq!(json_orig, json_rt2);
    }

    // ── P0-002C5C: bounded peer connections ─────────────────────────────

    #[test]
    fn p0_002c5c_peer_messages_are_bounded_before_allocation() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // A peer announcing a 4 GiB message and sending none of it.
            stream
                .write_all(&[FLAG_PLAINTEXT, 0xFF, 0xFF, 0xFF, 0xFF])
                .unwrap();
            let mut rest = [0u8; 16];
            while matches!(stream.read(&mut rest), Ok(n) if n > 0) {}
        });
        let link = NexusLink::new("client", "/tmp/models");
        let mut stream =
            connect_peer(address.parse().unwrap(), std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(
            stream.read_timeout().unwrap(),
            Some(std::time::Duration::from_secs(5))
        );
        assert_eq!(
            stream.write_timeout().unwrap(),
            Some(std::time::Duration::from_secs(5))
        );
        let error = match link.read_message(&mut stream) {
            Ok(_) => panic!("an oversized message was accepted"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            "Peer message of 4294967295 bytes exceeds the 16777216-byte limit"
        );
        drop(stream);
        peer.join().unwrap();

        // Final Gate item F: `connect_peer` takes only a socket address the
        // peer policy admitted, so text that is not one never reaches it.
        let mut link = NexusLink::new("client", "/tmp/models");
        link.set_allowed_peers(vec![address.clone()]);
        for malformed in ["", "no-port", "127.0.0.1"] {
            assert!(link.check_peer_allowed(malformed).is_err(), "{malformed:?}");
        }
    }

    // ── HMAC-SHA256 challenge-response auth tests ───────────────────────

    #[test]
    fn test_auth_challenge_response_over_tcp() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let mut server_link = NexusLink::new("server", "/tmp/models");
        server_link.set_shared_secret("shared-secret-xyz");

        let mut client_link = NexusLink::new("client", "/tmp/models");
        client_link.set_shared_secret("shared-secret-xyz");

        let server_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            server_link.authenticate_as_responder(&mut stream)
        });

        let mut client_stream = TcpStream::connect(addr).unwrap();
        client_stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let client_result = client_link.authenticate_as_initiator(&mut client_stream);

        let server_result = server_handle.join().unwrap();

        assert!(client_result.is_ok(), "Client auth should succeed");
        assert!(server_result.is_ok(), "Server auth should succeed");
    }

    #[test]
    fn test_auth_wrong_secret_fails() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let mut server_link = NexusLink::new("server", "/tmp/models");
        server_link.set_shared_secret("correct-secret");

        let mut client_link = NexusLink::new("client", "/tmp/models");
        client_link.set_shared_secret("wrong-secret");

        let server_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            // Server responds with its HMAC (using its own secret)
            server_link.authenticate_as_responder(&mut stream)
        });

        let mut client_stream = TcpStream::connect(addr).unwrap();
        client_stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let client_result = client_link.authenticate_as_initiator(&mut client_stream);

        let _ = server_handle.join().unwrap();

        // Client should fail HMAC verification since server used different secret
        assert!(client_result.is_err(), "Auth with wrong secret should fail");
        assert!(
            client_result.unwrap_err().contains("HMAC mismatch"),
            "Error should mention HMAC mismatch"
        );
    }

    #[test]
    fn test_auth_skipped_when_no_secret() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // No shared_secret set on either side
        let server_link = NexusLink::new("server", "/tmp/models");
        let client_link = NexusLink::new("client", "/tmp/models");

        let server_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            server_link.authenticate_as_responder(&mut stream)
        });

        let mut client_stream = TcpStream::connect(addr).unwrap();
        client_stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let client_result = client_link.authenticate_as_initiator(&mut client_stream);

        let server_result = server_handle.join().unwrap();

        // Both should succeed (no-op when no secret)
        assert!(client_result.is_ok());
        assert!(server_result.is_ok());
    }

    #[test]
    fn test_encrypted_message_over_tcp() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let mut server_link = NexusLink::new("server", "/tmp/models");
        server_link.set_encryption_key("tcp-enc-test");

        let mut client_link = NexusLink::new("client", "/tmp/models");
        client_link.set_encryption_key("tcp-enc-test");

        let server_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            server_link.read_message(&mut stream)
        });

        let mut client_stream = TcpStream::connect(addr).unwrap();
        let msg = LinkMessage::ModelListRequest;
        let serialized = client_link.serialize_message(&msg).unwrap();
        client_stream.write_all(&serialized).unwrap();

        let server_result = server_handle.join().unwrap();
        assert!(server_result.is_ok());
        let received = server_result.unwrap();
        let json = serde_json::to_string(&received).unwrap();
        assert_eq!(json, serde_json::to_string(&msg).unwrap());
    }

    #[test]
    fn test_set_encryption_key_derives_deterministic_key() {
        let mut link1 = NexusLink::new("dev1", "/tmp/models");
        let mut link2 = NexusLink::new("dev2", "/tmp/models");

        link1.set_encryption_key("same-passphrase");
        link2.set_encryption_key("same-passphrase");

        assert_eq!(link1.encryption_key, link2.encryption_key);
    }

    #[test]
    fn test_new_initializes_security_fields_as_none() {
        let link = NexusLink::new("dev", "/tmp/models");
        assert!(link.encryption_key.is_none());
        assert!(link.shared_secret.is_none());
    }

    #[test]
    fn p0_002c5b_send_model_names_a_stored_file_only_by_its_exact_spelling() {
        let base = std::env::temp_dir().join(format!("nexus-p0-002c5b-{}", uuid::Uuid::new_v4()));
        let models = base.join("models");
        std::fs::create_dir_all(models.join("m")).unwrap();
        std::fs::write(models.join("model.gguf"), b"x").unwrap();
        std::fs::write(models.join("m").join("q4.gguf"), b"x").unwrap();
        let link = NexusLink::new("device", &models.to_string_lossy());
        for (model_id, filename) in [
            ("m", "Model.gguf"),
            ("m", "MODEL.GGUF"),
            ("M", "q4.gguf"),
            ("m", "Q4.gguf"),
        ] {
            let error = link
                .send_model("127.0.0.1:9", model_id, filename, |_| {})
                .unwrap_err();
            assert!(
                error.contains("not found beneath"),
                "{model_id:?} {filename:?}: {error}"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    // ── Final Gate item F: the peer policy and an authenticated transport ──

    /// A models directory holding one regular model file, and its base.
    fn models_with_one_file() -> (std::path::PathBuf, String) {
        let base = std::env::temp_dir().join(format!("nexus-fg-link-{}", uuid::Uuid::new_v4()));
        let models = base.join("models");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("model.gguf"), b"weights").unwrap();
        let models = models.to_string_lossy().into_owned();
        (base, models)
    }

    /// A loopback listener that must never be contacted, and its address.
    fn quiet_peer() -> (std::net::TcpListener, String) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        (listener, address)
    }

    fn assert_never_contacted(listener: &std::net::TcpListener) {
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    fn authenticated_link(models: &str) -> NexusLink {
        let mut link = NexusLink::new("device", models);
        link.set_shared_secret("fg-shared-secret");
        link.set_encryption_key("fg-wire-key");
        link
    }

    /// An empty peer policy admits no peer, even over an authenticated
    /// link, and nothing is sent or discovered.
    #[test]
    fn p0_fg_an_empty_peer_policy_admits_no_peer() {
        let (base, models) = models_with_one_file();
        let (listener, address) = quiet_peer();
        let link = authenticated_link(&models);
        let error = link
            .send_model(&address, "m", "model.gguf", |_| {})
            .unwrap_err();
        assert!(error.contains("peer policy is empty"), "{error}");
        let peer = PeerDevice {
            id: "peer".into(),
            name: "peer".into(),
            address: address.clone(),
            last_seen: 0,
            available_models: Vec::new(),
        };
        let error = link.discover_peer_models(&peer).unwrap_err();
        assert!(error.contains("peer policy is empty"), "{error}");
        assert_never_contacted(&listener);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The policy compares IP socket addresses exactly. A host name, a
    /// host without its port, another port, a mapped or shorthand spelling
    /// and URL syntax name no admitted peer, and a policy entry that is not
    /// a socket address admits nothing. No name is resolved.
    #[test]
    fn p0_fg_the_peer_policy_admits_only_exact_socket_addresses() {
        let (base, models) = models_with_one_file();
        let (listener, address) = quiet_peer();
        let port = listener.local_addr().unwrap().port();
        let mut link = authenticated_link(&models);

        for policy in [
            format!("localhost:{port}"),
            format!("LOCALHOST:{port}"),
            "127.0.0.1".to_string(),
            format!("127.0.0.1:{}", port.wrapping_add(1)),
            format!("[::ffff:127.0.0.1]:{port}"),
            format!("127.0.0.1:{port} "),
            format!("http://127.0.0.1:{port}"),
            String::new(),
        ] {
            link.set_allowed_peers(vec![policy.clone()]);
            let error = link
                .send_model(&address, "m", "model.gguf", |_| {})
                .unwrap_err();
            assert!(error.contains("Governance denied"), "{policy:?}: {error}");
        }

        link.set_allowed_peers(vec![address.clone()]);
        for alias in [
            format!("localhost:{port}"),
            format!("127.1:{port}"),
            format!("[::ffff:127.0.0.1]:{port}"),
            "127.0.0.1".to_string(),
            format!("127.0.0.1:{port}/x"),
            format!("http://127.0.0.1:{port}"),
            format!(" 127.0.0.1:{port}"),
        ] {
            let error = link
                .send_model(&alias, "m", "model.gguf", |_| {})
                .unwrap_err();
            assert!(error.contains("Governance denied"), "{alias:?}: {error}");
        }
        assert_never_contacted(&listener);

        // Spellings of one socket address are one peer, and that address is
        // the one a transfer would connect to.
        link.set_allowed_peers(vec!["[::1]:9".to_string()]);
        assert_eq!(
            link.check_peer_allowed("[0:0:0:0:0:0:0:1]:9"),
            Ok("[::1]:9".parse().unwrap())
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Without both a shared secret and a wire key, an admitted peer is still
    /// refused, before any connection.
    #[test]
    fn p0_fg_an_unauthenticated_transfer_is_refused_before_connecting() {
        let (base, models) = models_with_one_file();
        let (listener, address) = quiet_peer();
        for (secret, key) in [(false, false), (true, false), (false, true)] {
            let mut link = NexusLink::new("device", &models);
            link.set_allowed_peers(vec![address.clone()]);
            if secret {
                link.set_shared_secret("fg-shared-secret");
            }
            if key {
                link.set_encryption_key("fg-wire-key");
            }
            let error = link
                .send_model(&address, "m", "model.gguf", |_| {})
                .unwrap_err();
            assert!(error.contains("unauthenticated"), "{secret} {key}: {error}");
            let peer = PeerDevice {
                id: "peer".into(),
                name: "peer".into(),
                address: address.clone(),
                last_seen: 0,
                available_models: Vec::new(),
            };
            let error = link.discover_peer_models(&peer).unwrap_err();
            assert!(error.contains("unauthenticated"), "{secret} {key}: {error}");
        }
        assert_never_contacted(&listener);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// An admitted peer over an authenticated, encrypted link is reached at
    /// exactly its policy address, and the transfer proceeds to the peer's
    /// answer.
    #[test]
    fn p0_fg_an_admitted_authenticated_peer_is_reached_at_its_policy_address() {
        let (base, models) = models_with_one_file();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let mut link = authenticated_link(&models);
        link.set_allowed_peers(vec![address.clone()]);
        let responder = authenticated_link(&models);
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            responder.authenticate_as_responder(&mut stream).unwrap();
            let request = responder.read_message(&mut stream).unwrap();
            let reply = responder
                .serialize_message(&LinkMessage::TransferRejected {
                    reason: "test peer".into(),
                })
                .unwrap();
            stream.write_all(&reply).unwrap();
            request
        });
        let error = link
            .send_model(&address, "m", "model.gguf", |_| {})
            .unwrap_err();
        assert_eq!(error, "Transfer rejected: test peer");
        let request = peer.join().unwrap();
        assert!(
            matches!(&request, LinkMessage::TransferRequest { filename, .. } if filename == "model.gguf"),
            "{request:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
