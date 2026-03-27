use crate::engine::{PerceptionEngine, PerceptionError};
use crate::vision::{ImageFormat, ImageSource, PerceptionResult, PerceptionTask, VisualInput};

/// Document understanding — processes document images and extracts content.
pub struct DocumentReader;

impl DocumentReader {
    pub fn read_page(
        engine: &mut PerceptionEngine,
        image_base64: &str,
        format: ImageFormat,
    ) -> Result<PerceptionResult, PerceptionError> {
        let input = VisualInput {
            id: uuid::Uuid::new_v4().to_string(),
            image_base64: image_base64.to_string(),
            format,
            width: None,
            height: None,
            source: ImageSource::File {
                path: "document".into(),
            },
        };
        engine.perceive(&input, &PerceptionTask::ReadDocument)
    }

    pub fn extract_table(
        engine: &mut PerceptionEngine,
        image_base64: &str,
        format: ImageFormat,
    ) -> Result<PerceptionResult, PerceptionError> {
        let schema = r#"{"type": "array", "items": {"type": "object"}}"#;
        let input = VisualInput {
            id: uuid::Uuid::new_v4().to_string(),
            image_base64: image_base64.to_string(),
            format,
            width: None,
            height: None,
            source: ImageSource::File {
                path: "document".into(),
            },
        };
        engine.perceive(
            &input,
            &PerceptionTask::ExtractStructuredData {
                schema: Some(schema.into()),
            },
        )
    }

    pub fn analyze_chart(
        engine: &mut PerceptionEngine,
        image_base64: &str,
        format: ImageFormat,
    ) -> Result<PerceptionResult, PerceptionError> {
        let input = VisualInput {
            id: uuid::Uuid::new_v4().to_string(),
            image_base64: image_base64.to_string(),
            format,
            width: None,
            height: None,
            source: ImageSource::File {
                path: "chart".into(),
            },
        };
        engine.perceive(&input, &PerceptionTask::AnalyzeChart)
    }
}
