//! ScreenAnalyzeTool — vision analysis of screenshots via Claude CLI.
//! Requires ComputerUse capability (not granted by default).

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Dead man switch timeout for vision analysis (60 seconds).
const VISION_TIMEOUT_SECS: u64 = 60;

/// Analyze a screenshot with Claude Opus 4.6 vision.
/// Takes an image (base64 or from last capture) and sends it to Claude
/// with a specific analysis prompt.
pub struct ScreenAnalyzeTool;

#[async_trait]
impl NxTool for ScreenAnalyzeTool {
    fn name(&self) -> &str {
        "screen_analyze"
    }

    fn description(&self) -> &str {
        "Analyze a screenshot with Claude Opus 4.6 vision. Ask questions about what's \
         visible on screen. Pass image as base64 or use the last captured screenshot. \
         Requires ComputerUse capability. Costs 5 fuel (vision is expensive)."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "What to analyze in the screenshot"
                },
                "image": {
                    "type": "string",
                    "description": "Base64-encoded PNG image (if omitted, uses last screenshot from ~/.nexus/screenshots/)"
                }
            },
            "required": ["question"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        5
    }

    fn required_capability(
        &self,
        _input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        Some(crate::governance::Capability::ComputerUse)
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let question = match input.get("question").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::error("Missing required parameter: question"),
        };

        // Get image: either from input or find latest screenshot
        let image_base64 = if let Some(img) = input.get("image").and_then(|v| v.as_str()) {
            img.to_string()
        } else {
            match find_latest_screenshot() {
                Some(path) => match std::fs::read(&path) {
                    Ok(bytes) => crate::tools::screen_capture::base64_encode_bytes(&bytes),
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Failed to read latest screenshot: {}",
                            e
                        ));
                    }
                },
                None => {
                    return ToolResult::error("No screenshot available. Use screen_capture first.");
                }
            }
        };

        // Decode base64 to write temp file for Claude CLI
        let bytes = match base64_decode(&image_base64) {
            Ok(b) => b,
            Err(e) => {
                return ToolResult::error(format!("Invalid base64 image data: {}", e));
            }
        };

        let temp_path = std::env::temp_dir().join("nx_vision_input.png");
        if let Err(e) = std::fs::write(&temp_path, &bytes) {
            return ToolResult::error(format!("Failed to write temp image: {}", e));
        }

        // SHA-256 hash of image for audit
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let image_hash = format!("{:x}", hasher.finalize());

        // Call Claude CLI with vision (with dead man switch timeout)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(VISION_TIMEOUT_SECS),
            run_claude_vision(&temp_path, question),
        )
        .await;

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        match result {
            Ok(Ok(response)) => ToolResult::success(
                serde_json::to_string(&json!({
                    "analysis": response,
                    "model": "claude-opus-4-6",
                    "image_hash": image_hash,
                }))
                .unwrap_or_default(),
            ),
            Ok(Err(e)) => ToolResult::error(format!("Vision analysis failed: {}", e)),
            Err(_) => ToolResult::error(format!(
                "Dead man switch: vision analysis timed out after {}s — aborted",
                VISION_TIMEOUT_SECS
            )),
        }
    }
}

/// Find the most recent screenshot in ~/.nexus/screenshots/.
fn find_latest_screenshot() -> Option<std::path::PathBuf> {
    let dir = dirs::home_dir()?.join(".nexus").join("screenshots");
    if !dir.exists() {
        return None;
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "png")
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| {
        e.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    entries.last().map(|e| e.path())
}

/// Run Claude CLI with an image for vision analysis.
async fn run_claude_vision(image_path: &std::path::Path, question: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("claude")
        .args([
            "--model",
            "claude-opus-4-6",
            "--image",
            &image_path.to_string_lossy(),
            "-p",
            question,
        ])
        .output()
        .await
        .map_err(|e| {
            format!(
                "Claude CLI not found or failed: {}. Install from https://claude.ai/cli",
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Claude CLI exited with error: {}", stderr));
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(response)
}

/// Simple base64 decoder.
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const DECODE: [u8; 128] = {
        let mut table = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i;
            table[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            table[(b'0' + i) as usize] = i + 52;
            i += 1;
        }
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let input = input.trim();
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect();
    let mut result = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return Err("Invalid base64 input".to_string());
        }
        let mut buf = [0u32; 4];
        let mut pad = 0;
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                pad += 1;
                buf[i] = 0;
            } else if b < 128 && DECODE[b as usize] != 255 {
                buf[i] = DECODE[b as usize] as u32;
            } else {
                return Err(format!("Invalid base64 character: {}", b as char));
            }
        }
        // Fill missing positions (short final chunk)
        for item in buf.iter_mut().skip(chunk.len()) {
            *item = 0;
            pad += 1;
        }
        let triple = (buf[0] << 18) | (buf[1] << 12) | (buf[2] << 6) | buf[3];
        result.push((triple >> 16) as u8);
        if pad < 2 {
            result.push((triple >> 8) as u8);
        }
        if pad < 1 {
            result.push(triple as u8);
        }
    }

    Ok(result)
}
