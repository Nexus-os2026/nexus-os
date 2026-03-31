//! Rust↔Python subprocess bridge for browser-use.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use serde::{Deserialize, Serialize};

/// Command sent to the Python subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeCommand {
    pub action: String,
    pub params: serde_json::Value,
}

/// Response from the Python subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub steps_taken: Option<usize>,
}

/// Bridge error.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Failed to spawn Python subprocess: {0}")]
    SpawnFailed(String),
    #[error("Bridge initialization failed: {0}")]
    InitFailed(String),
    #[error("Failed to write to subprocess: {0}")]
    WriteFailed(String),
    #[error("Failed to read from subprocess: {0}")]
    ReadFailed(String),
    #[error("Failed to parse response: {0}")]
    ParseFailed(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Subprocess not running")]
    NotRunning,
}

/// Bridge to the browser-use Python subprocess.
pub struct BrowserBridge {
    child: Option<Child>,
    stdout_reader: Option<BufReader<std::process::ChildStdout>>,
    python_path: String,
    script_path: String,
}

impl BrowserBridge {
    pub fn new(python_path: String, script_path: String) -> Self {
        Self {
            child: None,
            stdout_reader: None,
            python_path,
            script_path,
        }
    }

    /// Start the Python subprocess.
    pub fn start(&mut self) -> Result<(), BridgeError> {
        let mut child = Command::new(&self.python_path)
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BridgeError::SpawnFailed(e.to_string()))?;

        // Read the "ready" signal from stdout
        let stdout = child.stdout.take().ok_or(BridgeError::NotRunning)?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| BridgeError::ReadFailed(e.to_string()))?;

        let response: BridgeResponse = serde_json::from_str(line.trim())
            .map_err(|e| BridgeError::ParseFailed(e.to_string()))?;

        if response.status != "ready" {
            return Err(BridgeError::InitFailed(
                response.message.unwrap_or_else(|| "Not ready".into()),
            ));
        }

        self.stdout_reader = Some(reader);
        self.child = Some(child);
        Ok(())
    }

    /// Send a command and get the response.
    pub fn send_command(&mut self, command: BridgeCommand) -> Result<BridgeResponse, BridgeError> {
        let child = self.child.as_mut().ok_or(BridgeError::NotRunning)?;

        let json = serde_json::to_string(&command)
            .map_err(|e| BridgeError::SerializationError(e.to_string()))?;

        let stdin = child.stdin.as_mut().ok_or(BridgeError::NotRunning)?;
        writeln!(stdin, "{json}").map_err(|e| BridgeError::WriteFailed(e.to_string()))?;
        stdin
            .flush()
            .map_err(|e| BridgeError::WriteFailed(e.to_string()))?;

        // Read response line from subprocess stdout
        let reader = self.stdout_reader.as_mut().ok_or(BridgeError::NotRunning)?;
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| BridgeError::ReadFailed(e.to_string()))?;

        if line.trim().is_empty() {
            return Err(BridgeError::ReadFailed(
                "Empty response from subprocess".into(),
            ));
        }

        serde_json::from_str(line.trim()).map_err(|e| BridgeError::ParseFailed(e.to_string()))
    }

    /// Shutdown the subprocess.
    pub fn shutdown(&mut self) -> Result<(), BridgeError> {
        if let Some(mut child) = self.child.take() {
            // Try to send shutdown command
            if let Some(ref mut stdin) = child.stdin {
                let _ = writeln!(stdin, r#"{{"action":"shutdown","params":{{}}}}"#);
                let _ = stdin.flush();
            }
            let _ = child.wait();
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for BrowserBridge {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_command_serialization() {
        let cmd = BridgeCommand {
            action: "navigate".into(),
            params: serde_json::json!({"url": "https://example.com"}),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: BridgeCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.action, "navigate");
        assert_eq!(parsed.params["url"], "https://example.com");
    }

    #[test]
    fn test_bridge_response_deserialization() {
        let json = r#"{"status":"ok","url":"https://example.com","title":"Example"}"#;
        let resp: BridgeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.url.unwrap(), "https://example.com");
        assert_eq!(resp.title.unwrap(), "Example");
    }
}
