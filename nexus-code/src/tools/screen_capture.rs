//! ScreenCaptureTool — governed screenshot capture with audit trail.
//! Requires ComputerUse capability (not granted by default).

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Capture a screenshot of the screen or a specific window.
/// Returns base64-encoded PNG and SHA-256 audit hash.
pub struct ScreenCaptureTool;

#[async_trait]
impl NxTool for ScreenCaptureTool {
    fn name(&self) -> &str {
        "screen_capture"
    }

    fn description(&self) -> &str {
        "Capture a screenshot of the screen or a specific window. Returns base64 PNG \
         for vision analysis. The screenshot is audit-logged with a SHA-256 content hash. \
         Requires ComputerUse capability."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "window": {
                    "type": "string",
                    "description": "Optional window title to focus before capture"
                }
            },
            "required": []
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        1
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::ComputerUse)
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let window = input.get("window").and_then(|v| v.as_str());

        // Focus specific window if requested
        if let Some(win_title) = window {
            let find = match std::process::Command::new("xdotool")
                .args(["search", "--name", win_title])
                .output()
            {
                Ok(o) => o,
                Err(e) => {
                    return ToolResult::error(format!("xdotool search failed: {}", e));
                }
            };

            let stdout = String::from_utf8_lossy(&find.stdout);
            let window_id = match stdout.trim().lines().next() {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return ToolResult::error(format!("Window '{}' not found", win_title));
                }
            };

            if let Err(e) = std::process::Command::new("xdotool")
                .args(["windowactivate", "--sync", &window_id])
                .output()
            {
                return ToolResult::error(format!("Failed to activate window: {}", e));
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        // Build screenshot path
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let screenshot_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".nexus")
            .join("screenshots");
        if let Err(e) = std::fs::create_dir_all(&screenshot_dir) {
            return ToolResult::error(format!("Failed to create screenshot dir: {}", e));
        }
        let path = screenshot_dir.join(format!("nx_{}.png", timestamp));

        // Capture via scrot
        let scrot_result = std::process::Command::new("scrot")
            .args(["-o", &path.to_string_lossy()])
            .output();

        match scrot_result {
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return ToolResult::error(format!("scrot failed: {}", stderr));
            }
            Err(e) => {
                return ToolResult::error(format!(
                    "scrot not found or failed: {}. Install with: sudo apt install scrot",
                    e
                ));
            }
            _ => {}
        }

        // Read file bytes
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                return ToolResult::error(format!("Failed to read screenshot: {}", e));
            }
        };

        // Base64 encode
        use sha2::{Digest, Sha256};
        let base64_str = base64_encode(&bytes);

        // SHA-256 hash for audit
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());

        let file_size = bytes.len();

        ToolResult::success(
            serde_json::to_string(&json!({
                "image_base64": base64_str,
                "path": path.to_string_lossy(),
                "hash": hash,
                "size_bytes": file_size,
                "window": window,
            }))
            .unwrap_or_default(),
        )
    }
}

/// Public wrapper for use by screen_analyze.
pub fn base64_encode_bytes(data: &[u8]) -> String {
    base64_encode(data)
}

/// Simple base64 encoder (no external crate needed — we encode PNG bytes).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
