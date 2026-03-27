use std::collections::HashMap;

use serde::Deserialize;

use crate::vision::{
    PerceptionResult, PerceptionTask, UIElement, UIElementType, VisionProvider, VisualInput,
};

/// Core perception engine — processes visual inputs through vision models.
pub struct PerceptionEngine {
    provider: Box<dyn VisionProvider>,
    cache: HashMap<String, PerceptionResult>,
    max_cache: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum PerceptionError {
    #[error("Vision model error: {0}")]
    ModelError(String),
    #[error("Invalid image: {0}")]
    InvalidImage(String),
    #[error("Governance denied: {0}")]
    GovernanceDenied(String),
    #[error("Insufficient balance")]
    InsufficientBalance,
}

impl PerceptionEngine {
    pub fn new(provider: Box<dyn VisionProvider>) -> Self {
        Self {
            provider,
            cache: HashMap::new(),
            max_cache: 100,
        }
    }

    pub fn perceive(
        &mut self,
        input: &VisualInput,
        task: &PerceptionTask,
    ) -> Result<PerceptionResult, PerceptionError> {
        let cache_key = format!("{}:{:?}", input.id, task);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let prompt = self.build_prompt(task);
        let response = self
            .provider
            .perceive(
                &input.image_base64,
                input.format.mime_type(),
                &prompt,
                self.max_tokens_for_task(task),
            )
            .map_err(PerceptionError::ModelError)?;

        let result = self.parse_response(input, task, &response);

        if self.cache.len() >= self.max_cache {
            if let Some(key) = self.cache.keys().next().cloned() {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(cache_key, result.clone());

        Ok(result)
    }

    pub fn build_prompt(&self, task: &PerceptionTask) -> String {
        match task {
            PerceptionTask::Describe => {
                "Describe this image in detail. What do you see? Be specific and thorough.".into()
            }
            PerceptionTask::ExtractText => {
                "Extract ALL text visible in this image. Return the text exactly as it appears, \
                 preserving layout where possible. If there are multiple text areas, separate them \
                 with newlines."
                    .into()
            }
            PerceptionTask::VisualQuestion { question } => {
                format!(
                    "Look at this image and answer the following question: {}",
                    question
                )
            }
            PerceptionTask::IdentifyUIElements => {
                "Identify all interactive UI elements in this screenshot. For each element, provide:\n\
                 - Type (button, text input, dropdown, checkbox, link, menu, tab, dialog)\n\
                 - Label or text content\n\
                 - Whether it appears clickable/interactive\n\
                 Return as a JSON array of objects with fields: type, label, interactive".into()
            }
            PerceptionTask::ExtractStructuredData { schema } => {
                if let Some(s) = schema {
                    format!(
                        "Extract structured data from this image according to this schema:\n{}\n\
                         Return ONLY valid JSON matching the schema.",
                        s
                    )
                } else {
                    "Extract any structured data visible in this image (tables, forms, lists). \
                     Return as JSON."
                        .into()
                }
            }
            PerceptionTask::Compare { .. } => {
                "Compare this image with the previous one. Describe what has changed, \
                 what's new, and what's missing."
                    .into()
            }
            PerceptionTask::ReadErrorMessage => {
                "Read the error message or dialog in this screenshot. Extract:\n\
                 1. The error title/type\n\
                 2. The error message text\n\
                 3. Any error codes\n\
                 4. Available actions (buttons like OK, Cancel, Retry)\n\
                 Return as JSON with fields: title, message, error_code, actions"
                    .into()
            }
            PerceptionTask::AnalyzeChart => {
                "Analyze this chart or graph. Extract:\n\
                 1. Chart type (bar, line, pie, scatter, etc.)\n\
                 2. Title and axis labels\n\
                 3. Key data points and trends\n\
                 4. Any notable patterns or anomalies\n\
                 Be specific with numbers where visible."
                    .into()
            }
            PerceptionTask::ReadDocument => {
                "Read this document page. Extract the full text content, preserving:\n\
                 - Headings and structure\n\
                 - Paragraph breaks\n\
                 - Any lists or tables\n\
                 Return the document content as structured text."
                    .into()
            }
        }
    }

    pub fn max_tokens_for_task(&self, task: &PerceptionTask) -> u32 {
        match task {
            PerceptionTask::Describe => 512,
            PerceptionTask::ExtractText => 2048,
            PerceptionTask::VisualQuestion { .. } => 512,
            PerceptionTask::IdentifyUIElements => 1024,
            PerceptionTask::ExtractStructuredData { .. } => 1024,
            PerceptionTask::Compare { .. } => 512,
            PerceptionTask::ReadErrorMessage => 256,
            PerceptionTask::AnalyzeChart => 512,
            PerceptionTask::ReadDocument => 2048,
        }
    }

    fn parse_response(
        &self,
        input: &VisualInput,
        task: &PerceptionTask,
        response: &str,
    ) -> PerceptionResult {
        let mut result = PerceptionResult {
            input_id: input.id.clone(),
            task: format!("{:?}", task),
            success: true,
            description: response.to_string(),
            extracted_text: None,
            structured_data: None,
            ui_elements: None,
            confidence: 0.8,
            model_used: self.provider.model_id().to_string(),
            tokens_used: (response.len() as u64) / 4,
        };

        match task {
            PerceptionTask::ExtractText => {
                result.extracted_text = Some(response.to_string());
            }
            PerceptionTask::IdentifyUIElements => {
                if let Ok(elements) = self.parse_ui_elements(response) {
                    result.ui_elements = Some(elements);
                }
            }
            PerceptionTask::ExtractStructuredData { .. }
            | PerceptionTask::ReadErrorMessage
            | PerceptionTask::AnalyzeChart => {
                if let Ok(data) =
                    serde_json::from_str::<serde_json::Value>(strip_code_fences(response))
                {
                    result.structured_data = Some(data);
                }
            }
            _ => {}
        }

        result
    }

    fn parse_ui_elements(&self, response: &str) -> Result<Vec<UIElement>, String> {
        let cleaned = strip_code_fences(response);

        #[derive(Deserialize)]
        struct RawElement {
            #[serde(alias = "type")]
            element_type: String,
            label: String,
            interactive: Option<bool>,
        }

        let raw: Vec<RawElement> =
            serde_json::from_str(cleaned).map_err(|e| format!("Parse error: {e}"))?;

        Ok(raw
            .into_iter()
            .map(|r| {
                let element_type = match r.element_type.to_lowercase().as_str() {
                    "button" => UIElementType::Button,
                    "text_input" | "textinput" | "input" => UIElementType::TextInput,
                    "dropdown" | "select" => UIElementType::Dropdown,
                    "checkbox" => UIElementType::Checkbox,
                    "radio" | "radiobutton" => UIElementType::RadioButton,
                    "link" | "a" => UIElementType::Link,
                    "menu" => UIElementType::Menu,
                    "tab" => UIElementType::Tab,
                    "dialog" | "modal" => UIElementType::Dialog,
                    "error" => UIElementType::ErrorMessage,
                    "table" => UIElementType::Table,
                    "chart" => UIElementType::Chart,
                    other => UIElementType::Other(other.into()),
                };

                UIElement {
                    element_type,
                    label: r.label,
                    bounds: None,
                    interactive: r.interactive.unwrap_or(true),
                }
            })
            .collect())
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn provider_model_id(&self) -> &str {
        self.provider.model_id()
    }
}

fn strip_code_fences(s: &str) -> &str {
    let trimmed = s.trim();
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    stripped.strip_suffix("```").unwrap_or(stripped).trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::{ImageFormat, ImageSource, VisualInput};

    struct MockProvider {
        response: String,
    }

    impl MockProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    impl VisionProvider for MockProvider {
        fn perceive(
            &self,
            _image_base64: &str,
            _mime_type: &str,
            _prompt: &str,
            _max_tokens: u32,
        ) -> Result<String, String> {
            Ok(self.response.clone())
        }
        fn model_id(&self) -> &str {
            "mock-v1"
        }
    }

    fn test_input() -> VisualInput {
        VisualInput {
            id: "test-1".into(),
            image_base64: "aGVsbG8=".into(),
            format: ImageFormat::Png,
            width: None,
            height: None,
            source: ImageSource::UserUpload,
        }
    }

    #[test]
    fn test_perception_task_prompt_describe() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("ok")));
        let prompt = engine.build_prompt(&PerceptionTask::Describe);
        assert!(prompt.contains("Describe this image"));
    }

    #[test]
    fn test_perception_task_prompt_extract_text() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("ok")));
        let prompt = engine.build_prompt(&PerceptionTask::ExtractText);
        assert!(prompt.contains("Extract ALL text"));
    }

    #[test]
    fn test_perception_task_prompt_ui_elements() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("ok")));
        let prompt = engine.build_prompt(&PerceptionTask::IdentifyUIElements);
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn test_perception_task_prompt_question() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("ok")));
        let prompt = engine.build_prompt(&PerceptionTask::VisualQuestion {
            question: "What color is the button?".into(),
        });
        assert!(prompt.contains("What color is the button?"));
    }

    #[test]
    fn test_max_tokens_varies_by_task() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("ok")));
        let describe = engine.max_tokens_for_task(&PerceptionTask::Describe);
        let extract = engine.max_tokens_for_task(&PerceptionTask::ExtractText);
        assert!(extract > describe);
    }

    #[test]
    fn test_perception_cache() {
        let mut engine = PerceptionEngine::new(Box::new(MockProvider::new("A terminal window")));
        let input = test_input();

        let r1 = engine.perceive(&input, &PerceptionTask::Describe).unwrap();
        let r2 = engine.perceive(&input, &PerceptionTask::Describe).unwrap();
        assert_eq!(r1.description, r2.description);
        assert_eq!(r1.input_id, r2.input_id);
    }

    #[test]
    fn test_parse_ui_elements() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("")));
        let json = r#"[{"type":"button","label":"Submit","interactive":true},{"type":"input","label":"Name","interactive":true}]"#;
        let elements = engine.parse_ui_elements(json).unwrap();
        assert_eq!(elements.len(), 2);
        assert!(matches!(elements[0].element_type, UIElementType::Button));
        assert!(matches!(elements[1].element_type, UIElementType::TextInput));
    }

    #[test]
    fn test_parse_ui_elements_malformed() {
        let engine = PerceptionEngine::new(Box::new(MockProvider::new("")));
        let result = engine.parse_ui_elements("not valid json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_code_fences() {
        assert_eq!(strip_code_fences("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_code_fences("```\nhello\n```"), "hello");
        assert_eq!(strip_code_fences("plain text"), "plain text");
    }
}
