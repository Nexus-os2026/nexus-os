//! Hash-chained, append-only session audit log.
//!
//! Phase 1.1 ships the file format and the chain. Real signing with the
//! session keypair lands in Phase 1.2; for now `prev_hash` and `hash`
//! cover I-5 (replayability) by themselves.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One entry in the audit log.
///
/// `prev_hash` is the `hash` of the previous entry; the genesis entry
/// uses an all-zero hash. `hash` is `sha256(prev_hash || body)` where
/// `body` is the JSON serialization of every field except `hash`
/// itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub state: String,
    pub action: String,
    pub specialist: Option<String>,
    pub inputs: serde_json::Value,
    pub output: serde_json::Value,
    pub prev_hash: String,
    pub hash: String,
}

/// The fields of an [`AuditEntry`] that participate in the hash, i.e.
/// every field except `hash`. Kept as a private mirror struct so the
/// hash computation is one `serde_json::to_vec` call away.
#[derive(Serialize)]
struct AuditEntryBody<'a> {
    timestamp: &'a str,
    state: &'a str,
    action: &'a str,
    specialist: Option<&'a str>,
    inputs: &'a serde_json::Value,
    output: &'a serde_json::Value,
    prev_hash: &'a str,
}

/// Append-only audit log writer.
pub struct AuditLog {
    path: PathBuf,
    last_hash: String,
}

impl AuditLog {
    /// Open (or prepare to open) an audit log at `path`. The file is
    /// not created until the first append. The starting `last_hash` is
    /// the all-zero genesis hash.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_hash: "0".repeat(64),
        }
    }

    /// Append a new entry. The caller fills in `timestamp`, `state`,
    /// `action`, `specialist`, `inputs`, and `output`; the `prev_hash`
    /// and `hash` fields are computed here and overwritten. The entry
    /// is then serialized as a single JSON line and appended to the
    /// log file.
    pub fn append(&mut self, mut entry: AuditEntry) -> crate::Result<()> {
        entry.prev_hash = self.last_hash.clone();
        let body = AuditEntryBody {
            timestamp: &entry.timestamp,
            state: &entry.state,
            action: &entry.action,
            specialist: entry.specialist.as_deref(),
            inputs: &entry.inputs,
            output: &entry.output,
            prev_hash: &entry.prev_hash,
        };
        let body_bytes = serde_json::to_vec(&body)?;
        let mut hasher = Sha256::new();
        hasher.update(entry.prev_hash.as_bytes());
        hasher.update(&body_bytes);
        let digest = hasher.finalize();
        let mut hash_hex = String::with_capacity(64);
        for byte in digest.iter() {
            hash_hex.push_str(&format!("{:02x}", byte));
        }
        entry.hash = hash_hex.clone();

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(&entry)?;
        writeln!(file, "{}", line)?;

        self.last_hash = hash_hex;
        Ok(())
    }

    /// The hash of the most recently appended entry, or the genesis
    /// hash if nothing has been appended yet.
    pub fn last_hash(&self) -> &str {
        &self.last_hash
    }

    /// The path of the underlying log file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn empty_entry() -> AuditEntry {
        AuditEntry {
            timestamp: "2026-04-08T14:23:00Z".to_string(),
            state: "Enumerate".to_string(),
            action: "begin".to_string(),
            specialist: None,
            inputs: serde_json::json!({}),
            output: serde_json::json!({}),
            prev_hash: String::new(),
            hash: String::new(),
        }
    }

    #[test]
    fn second_entry_links_to_first() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("audit.jsonl");
        let mut log = AuditLog::new(path.clone());

        log.append(empty_entry()).expect("append 1");
        let first_hash = log.last_hash().to_string();
        assert_ne!(first_hash, "0".repeat(64));

        let mut second = empty_entry();
        second.action = "step".to_string();
        log.append(second).expect("append 2");

        let contents = std::fs::read_to_string(&path).expect("read log");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        let second_parsed: AuditEntry = serde_json::from_str(lines[1]).expect("parse second entry");
        assert_eq!(second_parsed.prev_hash, first_hash);
    }
}
