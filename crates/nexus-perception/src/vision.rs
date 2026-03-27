use serde::{Deserialize, Serialize};

/// A visual input — image bytes with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualInput {
    pub id: String,
    pub image_base64: String,
    pub format: ImageFormat,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub source: ImageSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Gif,
    Bmp,
}

impl ImageFormat {
    pub fn mime_type(&self) -> &str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::WebP),
            "gif" => Some(Self::Gif),
            "bmp" => Some(Self::Bmp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSource {
    Screenshot { timestamp: u64 },
    BrowserCapture { url: String },
    File { path: String },
    AgentGenerated { agent_id: String },
    UserUpload,
}

/// What the agent wants to know about the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerceptionTask {
    Describe,
    ExtractText,
    VisualQuestion { question: String },
    IdentifyUIElements,
    ExtractStructuredData { schema: Option<String> },
    Compare { other_image_id: String },
    ReadErrorMessage,
    AnalyzeChart,
    ReadDocument,
}

/// Result of visual perception.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceptionResult {
    pub input_id: String,
    pub task: String,
    pub success: bool,
    pub description: String,
    pub extracted_text: Option<String>,
    pub structured_data: Option<serde_json::Value>,
    pub ui_elements: Option<Vec<UIElement>>,
    pub confidence: f64,
    pub model_used: String,
    pub tokens_used: u64,
}

/// A UI element identified in a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    pub element_type: UIElementType,
    pub label: String,
    pub bounds: Option<(f64, f64, f64, f64)>,
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UIElementType {
    Button,
    TextInput,
    Dropdown,
    Checkbox,
    RadioButton,
    Link,
    Menu,
    Tab,
    Dialog,
    ErrorMessage,
    Icon,
    Text,
    Image,
    Table,
    Chart,
    Other(String),
}

/// Trait for vision model providers — any model that can process images.
pub trait VisionProvider: Send + Sync {
    fn perceive(
        &self,
        image_base64: &str,
        mime_type: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<String, String>;

    fn model_id(&self) -> &str;
}

/// Groq/NIM vision provider — uses OpenAI-compatible vision API.
pub struct ApiVisionProvider {
    api_key: String,
    model_id: String,
    endpoint: String,
}

impl ApiVisionProvider {
    pub fn new_groq(api_key: String, model_id: String) -> Self {
        Self {
            api_key,
            model_id,
            endpoint: "https://api.groq.com/openai/v1/chat/completions".into(),
        }
    }

    pub fn new_nim(api_key: String, model_id: String) -> Self {
        Self {
            api_key,
            model_id,
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".into(),
        }
    }

    fn call_api(
        &self,
        image_base64: &str,
        mime_type: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model_id,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", mime_type, image_base64)
                        }
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }],
            "max_tokens": max_tokens
        });

        let encoded = serde_json::to_string(&body).map_err(|e| format!("json: {e}"))?;

        let marker = "__NX_P__:";
        let out = std::process::Command::new("curl")
            .args(["-sS", "-L", "-m", "60"])
            .arg("-H")
            .arg(format!("authorization: Bearer {}", self.api_key))
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(&encoded)
            .arg("-w")
            .arg(format!("\n{marker}%{{http_code}}"))
            .arg(&self.endpoint)
            .output()
            .map_err(|e| format!("curl: {e}"))?;

        if !out.status.success() {
            return Err("curl failed".into());
        }

        let raw = String::from_utf8(out.stdout).map_err(|e| format!("utf8: {e}"))?;
        let (body_raw, status_raw) = raw.rsplit_once(marker).ok_or("no status marker")?;
        let status: u16 = status_raw
            .trim()
            .parse()
            .map_err(|e| format!("status: {e}"))?;

        if !(200..300).contains(&status) {
            return Err(format!("API status {status}"));
        }

        let payload: serde_json::Value =
            serde_json::from_str(body_raw.trim()).map_err(|e| format!("parse: {e}"))?;

        payload
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No content in response".into())
    }
}

impl VisionProvider for ApiVisionProvider {
    fn perceive(
        &self,
        image_base64: &str,
        mime_type: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        for attempt in 0..3u32 {
            match self.call_api(image_base64, mime_type, prompt, max_tokens) {
                Ok(text) => return Ok(text),
                Err(e) if e.contains("429") && attempt < 2 => {
                    std::thread::sleep(std::time::Duration::from_millis(3000 * 2u64.pow(attempt)));
                    continue;
                }
                Err(_e) if attempt < 2 => {
                    std::thread::sleep(std::time::Duration::from_millis(500 * 2u64.pow(attempt)));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err("Exhausted retries".into())
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_mime() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
        assert_eq!(ImageFormat::Bmp.mime_type(), "image/bmp");
    }

    #[test]
    fn test_image_format_from_extension() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("JPG"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::WebP));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("bmp"), Some(ImageFormat::Bmp));
        assert_eq!(ImageFormat::from_extension("tiff"), None);
    }

    #[test]
    fn test_visual_input_creation() {
        let input = VisualInput {
            id: "test-123".into(),
            image_base64: "aGVsbG8=".into(),
            format: ImageFormat::Png,
            width: Some(1920),
            height: Some(1080),
            source: ImageSource::Screenshot { timestamp: 1000 },
        };
        assert_eq!(input.id, "test-123");
        assert_eq!(input.format, ImageFormat::Png);
        assert!(input.width.is_some());
    }

    #[test]
    fn test_mock_vision_provider() {
        struct MockProvider;
        impl VisionProvider for MockProvider {
            fn perceive(
                &self,
                _image_base64: &str,
                _mime_type: &str,
                _prompt: &str,
                _max_tokens: u32,
            ) -> Result<String, String> {
                Ok("A screenshot showing a terminal window".into())
            }
            fn model_id(&self) -> &str {
                "mock-vision-v1"
            }
        }

        let provider = MockProvider;
        let result = provider
            .perceive("base64data", "image/png", "describe", 512)
            .unwrap();
        assert!(result.contains("terminal"));
        assert_eq!(provider.model_id(), "mock-vision-v1");
    }
}
