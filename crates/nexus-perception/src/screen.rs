use crate::engine::{PerceptionEngine, PerceptionError};
use crate::vision::{
    ImageFormat, ImageSource, PerceptionResult, PerceptionTask, UIElement, VisualInput,
};

/// Screen reader — takes a screenshot via Computer Control
/// and processes it through the Perception Engine.
pub struct ScreenReader;

impl ScreenReader {
    pub fn read_screen(
        engine: &mut PerceptionEngine,
        screenshot_base64: &str,
        task: PerceptionTask,
    ) -> Result<PerceptionResult, PerceptionError> {
        let input = VisualInput {
            id: uuid::Uuid::new_v4().to_string(),
            image_base64: screenshot_base64.to_string(),
            format: ImageFormat::Png,
            width: None,
            height: None,
            source: ImageSource::Screenshot {
                timestamp: epoch_now(),
            },
        };
        engine.perceive(&input, &task)
    }

    pub fn find_clickable_elements(
        engine: &mut PerceptionEngine,
        screenshot_base64: &str,
    ) -> Result<Vec<UIElement>, PerceptionError> {
        let result = Self::read_screen(
            engine,
            screenshot_base64,
            PerceptionTask::IdentifyUIElements,
        )?;
        Ok(result
            .ui_elements
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.interactive)
            .collect())
    }

    pub fn read_error(
        engine: &mut PerceptionEngine,
        screenshot_base64: &str,
    ) -> Result<PerceptionResult, PerceptionError> {
        Self::read_screen(engine, screenshot_base64, PerceptionTask::ReadErrorMessage)
    }

    pub fn ask_about_screen(
        engine: &mut PerceptionEngine,
        screenshot_base64: &str,
        question: &str,
    ) -> Result<PerceptionResult, PerceptionError> {
        Self::read_screen(
            engine,
            screenshot_base64,
            PerceptionTask::VisualQuestion {
                question: question.into(),
            },
        )
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::VisionProvider;

    struct MockProvider;
    impl VisionProvider for MockProvider {
        fn perceive(&self, _: &str, _: &str, _: &str, _: u32) -> Result<String, String> {
            Ok("Screen shows a terminal with code".into())
        }
        fn model_id(&self) -> &str {
            "mock-v1"
        }
    }

    #[test]
    fn test_screen_reader_creates_input() {
        let mut engine = PerceptionEngine::new(Box::new(MockProvider));
        let result =
            ScreenReader::read_screen(&mut engine, "base64data", PerceptionTask::Describe).unwrap();
        assert!(result.success);
        assert!(result.description.contains("terminal"));
        assert_eq!(result.model_used, "mock-v1");
    }
}
