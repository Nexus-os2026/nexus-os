//! Frontend integration types.

use std::sync::RwLock;

use crate::engine::PerceptionEngine;
use crate::governance::PerceptionPolicy;
use crate::vision::{
    ApiVisionProvider, ImageFormat, ImageSource, PerceptionResult, PerceptionTask, UIElement,
    VisualInput,
};

/// In-memory perception state held by the Tauri app.
pub struct PerceptionState {
    pub engine: RwLock<Option<PerceptionEngine>>,
    pub policy: PerceptionPolicy,
}

impl Default for PerceptionState {
    fn default() -> Self {
        Self {
            engine: RwLock::new(None),
            policy: PerceptionPolicy::default(),
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn init_provider(
    state: &PerceptionState,
    provider: &str,
    api_key: &str,
    model_id: &str,
) -> Result<String, String> {
    let vision: Box<dyn crate::vision::VisionProvider> = match provider {
        "groq" => Box::new(ApiVisionProvider::new_groq(api_key.into(), model_id.into())),
        "nim" => Box::new(ApiVisionProvider::new_nim(api_key.into(), model_id.into())),
        other => return Err(format!("Unknown provider: {other}")),
    };

    let engine = PerceptionEngine::new(vision);
    *state.engine.write().map_err(|e| format!("lock: {e}"))? = Some(engine);
    Ok(format!("Perception initialized with {provider}/{model_id}"))
}

pub fn perceive_describe(
    state: &PerceptionState,
    image_base64: &str,
    format: &str,
) -> Result<PerceptionResult, String> {
    run_task(state, image_base64, format, PerceptionTask::Describe)
}

pub fn perceive_extract_text(
    state: &PerceptionState,
    image_base64: &str,
    format: &str,
) -> Result<PerceptionResult, String> {
    run_task(state, image_base64, format, PerceptionTask::ExtractText)
}

pub fn perceive_question(
    state: &PerceptionState,
    image_base64: &str,
    format: &str,
    question: &str,
) -> Result<PerceptionResult, String> {
    run_task(
        state,
        image_base64,
        format,
        PerceptionTask::VisualQuestion {
            question: question.into(),
        },
    )
}

pub fn perceive_find_ui_elements(
    state: &PerceptionState,
    image_base64: &str,
) -> Result<Vec<UIElement>, String> {
    let result = run_task(
        state,
        image_base64,
        "png",
        PerceptionTask::IdentifyUIElements,
    )?;
    Ok(result.ui_elements.unwrap_or_default())
}

pub fn perceive_extract_data(
    state: &PerceptionState,
    image_base64: &str,
    format: &str,
    schema: Option<String>,
) -> Result<PerceptionResult, String> {
    run_task(
        state,
        image_base64,
        format,
        PerceptionTask::ExtractStructuredData { schema },
    )
}

pub fn perceive_read_error(
    state: &PerceptionState,
    image_base64: &str,
) -> Result<PerceptionResult, String> {
    run_task(state, image_base64, "png", PerceptionTask::ReadErrorMessage)
}

pub fn perceive_analyze_chart(
    state: &PerceptionState,
    image_base64: &str,
    format: &str,
) -> Result<PerceptionResult, String> {
    run_task(state, image_base64, format, PerceptionTask::AnalyzeChart)
}

pub fn get_policy(state: &PerceptionState) -> PerceptionPolicy {
    state.policy.clone()
}

// ── Internal ─────────────────────────────────────────────────────────────────

fn run_task(
    state: &PerceptionState,
    image_base64: &str,
    format_str: &str,
    task: PerceptionTask,
) -> Result<PerceptionResult, String> {
    let format = ImageFormat::from_extension(format_str).unwrap_or(ImageFormat::Png);
    let input = VisualInput {
        id: uuid::Uuid::new_v4().to_string(),
        image_base64: image_base64.to_string(),
        format,
        width: None,
        height: None,
        source: ImageSource::UserUpload,
    };

    let mut guard = state.engine.write().map_err(|e| format!("lock: {e}"))?;
    let engine = guard
        .as_mut()
        .ok_or("Perception not initialized — call perception_init first")?;
    engine.perceive(&input, &task).map_err(|e| e.to_string())
}
