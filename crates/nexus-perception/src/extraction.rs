use crate::engine::{PerceptionEngine, PerceptionError};
use crate::vision::{PerceptionTask, VisualInput};

/// High-level extraction functions for common data types.
pub struct DataExtractor;

impl DataExtractor {
    pub fn extract_text(
        engine: &mut PerceptionEngine,
        input: &VisualInput,
    ) -> Result<String, PerceptionError> {
        let result = engine.perceive(input, &PerceptionTask::ExtractText)?;
        Ok(result.extracted_text.unwrap_or(result.description))
    }

    pub fn extract_form_data(
        engine: &mut PerceptionEngine,
        input: &VisualInput,
    ) -> Result<serde_json::Value, PerceptionError> {
        let schema = r#"{"type": "object", "description": "Form fields with their labels as keys and values as values"}"#;
        let result = engine.perceive(
            input,
            &PerceptionTask::ExtractStructuredData {
                schema: Some(schema.into()),
            },
        )?;
        Ok(result.structured_data.unwrap_or(serde_json::Value::Null))
    }

    pub fn extract_table_data(
        engine: &mut PerceptionEngine,
        input: &VisualInput,
    ) -> Result<serde_json::Value, PerceptionError> {
        let schema = r#"{"type": "object", "properties": {"headers": {"type": "array"}, "rows": {"type": "array"}}}"#;
        let result = engine.perceive(
            input,
            &PerceptionTask::ExtractStructuredData {
                schema: Some(schema.into()),
            },
        )?;
        Ok(result.structured_data.unwrap_or(serde_json::Value::Null))
    }
}
