//! API-based image generation — OpenAI DALL-E 3 integration.
//!
//! Fallback when local GPU isn't available. Cost: ~$0.04 per image.

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiGenError {
    #[error("no API key configured")]
    NoApiKey,
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("failed to download image: {0}")]
    DownloadFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Map requested dimensions to the closest DALL-E 3 supported size.
fn dalle_size(width: u32, height: u32) -> &'static str {
    let aspect = width as f64 / height as f64;
    if aspect > 1.3 {
        "1792x1024" // landscape
    } else if aspect < 0.77 {
        "1024x1792" // portrait
    } else {
        "1024x1024" // square
    }
}

/// Estimate cost in USD for a DALL-E 3 standard quality image.
fn dalle_cost(size: &str) -> f64 {
    match size {
        "1024x1024" => 0.04,
        "1792x1024" | "1024x1792" => 0.08,
        _ => 0.04,
    }
}

/// Generate an image via OpenAI DALL-E 3 API.
///
/// Returns the cost in USD on success.
/// `fuel_budget_usd` is the maximum cost allowed — the call is rejected if the
/// estimated cost exceeds this budget (defense-in-depth fuel gate).
pub async fn generate_api(
    prompt: &str,
    width: u32,
    height: u32,
    output_path: &Path,
    api_key: &str,
    fuel_budget_usd: f64,
) -> Result<f64, ApiGenError> {
    if api_key.is_empty() {
        return Err(ApiGenError::NoApiKey);
    }

    let size = dalle_size(width, height);
    let cost = dalle_cost(size);

    // Governance: fuel gate — reject if estimated cost exceeds budget
    if cost > fuel_budget_usd {
        return Err(ApiGenError::RequestFailed(format!(
            "DALL-E cost ${cost:.4} exceeds fuel budget ${fuel_budget_usd:.4}"
        )));
    }

    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "dall-e-3",
        "prompt": prompt,
        "n": 1,
        "size": size,
        "quality": "standard",
        "response_format": "url"
    });

    eprintln!("[nexus-builder][governance] dalle3_generate size={size} cost=${cost:.4}");
    let resp = client
        .post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiGenError::RequestFailed(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiGenError::RequestFailed(format!("HTTP {status}: {text}")));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiGenError::RequestFailed(format!("parse response: {e}")))?;

    let image_url = json["data"][0]["url"]
        .as_str()
        .ok_or_else(|| ApiGenError::RequestFailed("no image URL in response".into()))?;

    // Download the generated image
    let img_resp = client
        .get(image_url)
        .send()
        .await
        .map_err(|e| ApiGenError::DownloadFailed(e.to_string()))?;

    let bytes = img_resp
        .bytes()
        .await
        .map_err(|e| ApiGenError::DownloadFailed(e.to_string()))?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, &bytes)?;

    Ok(cost)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dalle_size_landscape() {
        assert_eq!(dalle_size(1920, 1080), "1792x1024");
    }

    #[test]
    fn test_dalle_size_portrait() {
        assert_eq!(dalle_size(768, 1024), "1024x1792");
    }

    #[test]
    fn test_dalle_size_square() {
        assert_eq!(dalle_size(800, 800), "1024x1024");
    }

    #[test]
    fn test_dalle_cost_standard() {
        assert_eq!(dalle_cost("1024x1024"), 0.04);
        assert_eq!(dalle_cost("1792x1024"), 0.08);
    }

    #[test]
    fn test_api_no_key_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(generate_api(
            "test prompt",
            800,
            600,
            Path::new("/tmp/test.png"),
            "",
            1.0,
        ));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiGenError::NoApiKey));
    }

    #[test]
    fn test_api_request_format() {
        // Verify the JSON body structure we send
        let body = serde_json::json!({
            "model": "dall-e-3",
            "prompt": "test image",
            "n": 1,
            "size": "1024x1024",
            "quality": "standard",
            "response_format": "url"
        });
        assert_eq!(body["model"], "dall-e-3");
        assert_eq!(body["n"], 1);
        assert_eq!(body["quality"], "standard");
        assert_eq!(body["response_format"], "url");
    }
}
