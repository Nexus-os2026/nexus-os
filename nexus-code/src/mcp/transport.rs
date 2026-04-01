//! MCP transport layer — stdio and SSE transports.

use super::jsonrpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;

/// MCP transport abstraction.
#[async_trait]
pub trait McpTransportTrait: Send + Sync {
    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, crate::error::NxError>;

    /// Send a notification (no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), crate::error::NxError>;

    /// Close the transport.
    async fn close(&mut self) -> Result<(), crate::error::NxError>;
}

/// Stdio transport: spawns an MCP server process, communicates via stdin/stdout.
pub struct StdioTransport {
    child: tokio::process::Child,
    stdin: tokio::io::BufWriter<tokio::process::ChildStdin>,
    reader: tokio::io::BufReader<tokio::process::ChildStdout>,
}

impl StdioTransport {
    /// Spawn an MCP server process.
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self, crate::error::NxError> {
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                crate::error::NxError::ConfigError(format!(
                    "Failed to spawn MCP server '{}': {}",
                    command, e
                ))
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            crate::error::NxError::ConfigError("Failed to capture MCP server stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            crate::error::NxError::ConfigError("Failed to capture MCP server stdout".to_string())
        })?;

        Ok(Self {
            child,
            stdin: tokio::io::BufWriter::new(stdin),
            reader: tokio::io::BufReader::new(stdout),
        })
    }
}

#[async_trait]
impl McpTransportTrait for StdioTransport {
    async fn send_request(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, crate::error::NxError> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let json = serde_json::to_string(&request)
            .map_err(|e| crate::error::NxError::ConfigError(format!("Serialize: {}", e)))?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        let mut line = String::new();
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.reader.read_line(&mut line),
        )
        .await;

        match timeout {
            Ok(Ok(0)) => Err(crate::error::NxError::ConfigError(
                "MCP server closed connection".to_string(),
            )),
            Ok(Ok(_)) => serde_json::from_str(line.trim()).map_err(|e| {
                crate::error::NxError::ConfigError(format!("Parse MCP response: {}", e))
            }),
            Ok(Err(e)) => Err(crate::error::NxError::Io(e)),
            Err(_) => Err(crate::error::NxError::ConfigError(
                "MCP server response timed out (30s)".to_string(),
            )),
        }
    }

    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), crate::error::NxError> {
        use tokio::io::AsyncWriteExt;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        let json = serde_json::to_string(&notification)
            .map_err(|e| crate::error::NxError::ConfigError(format!("Serialize: {}", e)))?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), crate::error::NxError> {
        self.child.kill().await.ok();
        Ok(())
    }
}

/// SSE transport: communicates via HTTP POST requests.
pub struct SseTransport {
    base_url: String,
    client: reqwest::Client,
    session_id: Option<String>,
}

impl SseTransport {
    pub fn new(url: &str) -> Self {
        Self {
            base_url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            session_id: None,
        }
    }
}

#[async_trait]
impl McpTransportTrait for SseTransport {
    async fn send_request(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, crate::error::NxError> {
        let url = format!("{}/message", self.base_url);
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref sid) = self.session_id {
            req = req.header("X-Session-Id", sid);
        }

        let response = req.json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NxError::ConfigError(format!(
                "MCP server HTTP {}: {}",
                status, body
            )));
        }

        if let Some(sid) = response.headers().get("x-session-id") {
            self.session_id = sid.to_str().ok().map(String::from);
        }

        let text = response.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| crate::error::NxError::ConfigError(format!("Parse MCP response: {}", e)))
    }

    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), crate::error::NxError> {
        let url = format!("{}/message", self.base_url);
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&notification)
            .send()
            .await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), crate::error::NxError> {
        Ok(())
    }
}
