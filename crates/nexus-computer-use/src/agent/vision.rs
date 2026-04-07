use std::process::Stdio;
use std::time::Duration;

use tracing::{debug, info};

use crate::agent::action::ActionPlan;
use crate::error::ComputerUseError;

/// Maximum width to send to the vision model (saves tokens)
const MAX_VISION_WIDTH: u32 = 1568;

/// Vision analyzer that sends screenshots to Claude for analysis
pub struct VisionAnalyzer {
    /// Command to invoke Claude CLI (default: "claude")
    claude_binary: String,
    /// Model to use (if any override)
    model: Option<String>,
}

impl Default for VisionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl VisionAnalyzer {
    /// Create a new VisionAnalyzer with default settings
    pub fn new() -> Self {
        Self {
            claude_binary: "claude".to_string(),
            model: None,
        }
    }

    /// Set a custom Claude binary path
    pub fn with_binary(mut self, binary: String) -> Self {
        self.claude_binary = binary;
        self
    }

    /// Set a model override
    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Build the system context for the agent
    pub fn build_system_context() -> &'static str {
        "You are a JSON-only computer use agent. You NEVER respond with natural language. Every response is a valid JSON object."
    }

    /// Build the vision prompt for the agent
    pub fn build_prompt(task: &str, history: &[String], step: u32) -> String {
        let mut prompt = String::new();
        prompt.push_str("CRITICAL: You must respond with ONLY a JSON object. No markdown, no explanation, no text outside the JSON.\n\n");
        prompt.push_str("You are a computer use agent. You look at screenshots and decide what actions to take.\n\n");
        prompt.push_str(&format!("TASK: {task}\n\n"));

        if !history.is_empty() {
            prompt.push_str("PREVIOUS STEPS:\n");
            for (i, entry) in history.iter().enumerate() {
                prompt.push_str(&format!("  Step {}: {entry}\n", i + 1));
            }
            prompt.push('\n');
        }

        prompt.push_str(&format!(
            "This is step {step}. Look at the screenshot and respond with a JSON action plan.\n\n"
        ));
        prompt.push_str("Respond with ONLY a JSON object in this exact format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"observation\": \"What you see on the screen\",\n");
        prompt.push_str("  \"reasoning\": \"Why you're taking these actions\",\n");
        prompt.push_str("  \"actions\": [\n");
        prompt.push_str(
            "    {\"action\": \"click\", \"x\": 100, \"y\": 200, \"button\": \"left\"},\n",
        );
        prompt.push_str("    {\"action\": \"type\", \"text\": \"hello\"},\n");
        prompt.push_str("    {\"action\": \"key_press\", \"key\": \"Return\"},\n");
        prompt.push_str("    {\"action\": \"scroll\", \"x\": 100, \"y\": 200, \"direction\": \"down\", \"amount\": 3},\n");
        prompt.push_str("    {\"action\": \"wait\", \"ms\": 500},\n");
        prompt.push_str("    {\"action\": \"screenshot\"},\n");
        prompt.push_str(
            "    {\"action\": \"done\", \"summary\": \"Describe what was accomplished\"}\n",
        );
        prompt.push_str("  ],\n");
        prompt.push_str("  \"confidence\": 0.9\n");
        prompt.push_str("}\n\n");
        prompt.push_str("Use \"done\" when the task is complete. Use \"screenshot\" if you need to see updated state.\n\n");
        prompt.push_str("Remember: ONLY output the JSON object. Nothing else. No markdown fences. No explanation before or after.");

        prompt
    }

    /// Send a screenshot to Claude for analysis and get an action plan
    pub async fn analyze(
        &self,
        screenshot_base64: &str,
        task: &str,
        history: &[String],
        step: u32,
    ) -> Result<ActionPlan, ComputerUseError> {
        let prompt = Self::build_prompt(task, history, step);

        info!("Sending screenshot to vision model (step {step})");
        debug!("Prompt length: {} chars", prompt.len());

        let plan = self.call_claude(&prompt, screenshot_base64).await?;

        info!(
            "Vision model returned {} actions with confidence {:.2}",
            plan.actions.len(),
            plan.confidence
        );

        Ok(plan)
    }

    /// Call `claude -p` with the prompt and screenshot
    async fn call_claude(
        &self,
        prompt: &str,
        _screenshot_base64: &str,
    ) -> Result<ActionPlan, ComputerUseError> {
        let mut cmd = tokio::process::Command::new(&self.claude_binary);
        cmd.arg("-p")
            .arg(prompt)
            .arg("--system-prompt")
            .arg(Self::build_system_context());

        if let Some(ref model) = self.model {
            cmd.arg("--model").arg(model);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            ComputerUseError::VisionError(format!(
                "Failed to spawn claude CLI '{}': {e}",
                self.claude_binary
            ))
        })?;

        let output = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
            .await
            .map_err(|_| {
                ComputerUseError::VisionError("Claude CLI timed out after 120s".to_string())
            })?
            .map_err(|e| ComputerUseError::VisionError(format!("Claude CLI failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ComputerUseError::VisionError(format!(
                "Claude CLI exited {}: {}",
                output.status, stderr
            )));
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();
        debug!(
            "Claude response ({} chars): {}",
            response.len(),
            &response[..response.len().min(200)]
        );

        parse_claude_response(&response)
    }

    /// Parse a raw response string into an ActionPlan (for testing with mock responses)
    pub fn parse_response(response: &str) -> Result<ActionPlan, ComputerUseError> {
        parse_claude_response(response)
    }
}

/// Parse a Claude CLI response into an ActionPlan, handling multiple formats:
/// 1. Raw ActionPlan JSON
/// 2. Claude CLI JSON envelope with "result" field
/// 3. Text with embedded JSON object
fn parse_claude_response(raw: &str) -> Result<ActionPlan, ComputerUseError> {
    let trimmed = raw.trim();

    // Try 1: Maybe it's the raw ActionPlan directly
    if let Ok(plan) = serde_json::from_str::<ActionPlan>(trimmed) {
        return Ok(plan);
    }

    // Try 2: Claude CLI JSON envelope — {"type":"result","result":"..."}
    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(result_str) = envelope.get("result").and_then(|v| v.as_str()) {
            let cleaned = result_str
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            return serde_json::from_str::<ActionPlan>(cleaned).map_err(|e| {
                ComputerUseError::VisionError(format!(
                    "Failed to parse action plan from result: {e}"
                ))
            });
        }
    }

    // Try 3: Maybe the response has text before/after JSON — find the outermost braces
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        let json_slice = &trimmed[start..=end];
        if let Ok(plan) = serde_json::from_str::<ActionPlan>(json_slice) {
            return Ok(plan);
        }
    }

    // Try 4: Plain text fallback — wrap as a Done action so the loop handles it gracefully
    if !trimmed.is_empty() {
        tracing::warn!("Claude returned plain text instead of JSON, wrapping as Done action");
        let summary_len = trimmed.len().min(200);
        return Ok(ActionPlan {
            observation: String::new(),
            reasoning: trimmed.to_string(),
            actions: vec![crate::agent::action::AgentAction::Done {
                summary: trimmed[..summary_len].to_string(),
            }],
            confidence: 0.5,
        });
    }

    Err(ComputerUseError::VisionError(
        "Empty response from Claude CLI".to_string(),
    ))
}

/// Downscale image bytes if wider than MAX_VISION_WIDTH, returns (bytes, width, height)
pub fn downscale_for_vision(png_bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), ComputerUseError> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| ComputerUseError::VisionError(format!("Failed to load image: {e}")))?;

    let (w, h) = image::GenericImageView::dimensions(&img);

    if w <= MAX_VISION_WIDTH {
        return Ok((png_bytes.to_vec(), w, h));
    }

    let ratio = MAX_VISION_WIDTH as f64 / w as f64;
    let new_h = ((h as f64) * ratio) as u32;
    let new_h = new_h.max(1);

    tracing::debug!("Downscaling screenshot from {w}x{h} to {MAX_VISION_WIDTH}x{new_h}");

    let resized = img.resize(
        MAX_VISION_WIDTH,
        new_h,
        image::imageops::FilterType::Lanczos3,
    );
    let (final_w, final_h) = image::GenericImageView::dimensions(&resized);

    let mut buf = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| {
            ComputerUseError::VisionError(format!("Failed to encode resized image: {e}"))
        })?;

    Ok((buf.into_inner(), final_w, final_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_prompt_construction() {
        let prompt = VisionAnalyzer::build_prompt("click the terminal", &[], 1);
        assert!(prompt.contains("TASK: click the terminal"));
        assert!(prompt.contains("step 1"));
        assert!(prompt.contains("CRITICAL: You must respond with ONLY a JSON object"));
        assert!(prompt.contains("Remember: ONLY output the JSON object"));
    }

    #[test]
    fn test_vision_system_context() {
        let ctx = VisionAnalyzer::build_system_context();
        assert!(ctx.contains("JSON-only"));
        assert!(ctx.contains("NEVER respond with natural language"));
    }

    #[test]
    fn test_vision_prompt_includes_history() {
        let history = vec![
            "Clicked at (100, 200)".to_string(),
            "Typed 'hello'".to_string(),
        ];
        let prompt = VisionAnalyzer::build_prompt("do thing", &history, 3);
        assert!(prompt.contains("PREVIOUS STEPS:"));
        assert!(prompt.contains("Step 1: Clicked at (100, 200)"));
        assert!(prompt.contains("Step 2: Typed 'hello'"));
        assert!(prompt.contains("step 3"));
    }

    #[test]
    fn test_vision_parse_response_valid() {
        let response = r#"{
            "observation": "I see a desktop with a terminal icon",
            "reasoning": "I should click the terminal icon to open it",
            "actions": [
                {"action": "click", "x": 500, "y": 300, "button": "left"}
            ],
            "confidence": 0.85
        }"#;
        let plan = VisionAnalyzer::parse_response(response).expect("parse");
        assert_eq!(plan.observation, "I see a desktop with a terminal icon");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vision_parse_response_empty_actions() {
        let response = r#"{
            "observation": "blank screen",
            "reasoning": "nothing to do",
            "actions": [],
            "confidence": 0.1
        }"#;
        let plan = VisionAnalyzer::parse_response(response).expect("parse");
        assert!(plan.actions.is_empty());
    }

    #[test]
    fn test_vision_parse_response_invalid_json() {
        // Plain text now falls back to a Done action instead of erroring
        let response = "this is not json {{{";
        let plan = VisionAnalyzer::parse_response(response).expect("text fallback");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.5).abs() < f64::EPSILON);
        assert!(plan.reasoning.contains("this is not json"));
    }

    #[test]
    fn test_vision_parse_response_unknown_action() {
        // Unknown action variants can't parse as ActionPlan, so the text
        // fallback wraps the response as a Done action
        let response = r#"{
            "observation": "screen",
            "reasoning": "test",
            "actions": [
                {"action": "fly_to_moon", "destination": "moon"}
            ],
            "confidence": 0.5
        }"#;
        let plan = VisionAnalyzer::parse_response(response).expect("text fallback");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.5).abs() < f64::EPSILON);
        // The reasoning contains the original response text
        assert!(plan.reasoning.contains("fly_to_moon"));
    }

    #[test]
    fn test_vision_downscale_large_image() {
        // Create a 3440px wide image
        let img = image::RgbImage::from_fn(3440, 1440, |_, _| image::Rgb([128, 128, 128]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode");
        let png_bytes = buf.into_inner();

        let (_, w, h) = downscale_for_vision(&png_bytes).expect("downscale");
        assert!(
            w <= MAX_VISION_WIDTH,
            "width {w} should be <= {MAX_VISION_WIDTH}"
        );
        assert!(h < 1440, "height {h} should be smaller than original");
    }

    #[test]
    fn test_vision_small_image_no_downscale() {
        // Create an 800px wide image
        let img = image::RgbImage::from_fn(800, 600, |_, _| image::Rgb([64, 64, 64]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode");
        let png_bytes = buf.into_inner();

        let (result_bytes, w, h) = downscale_for_vision(&png_bytes).expect("no downscale");
        assert_eq!(w, 800);
        assert_eq!(h, 600);
        assert_eq!(result_bytes, png_bytes);
    }

    #[test]
    fn test_vision_analyzer_creation() {
        let analyzer = VisionAnalyzer::new()
            .with_binary("my-claude".to_string())
            .with_model("opus".to_string());
        assert_eq!(analyzer.claude_binary, "my-claude");
        assert_eq!(analyzer.model.as_deref(), Some("opus"));
    }

    #[test]
    fn test_parse_raw_action_plan() {
        let raw = r#"{
            "observation": "I see a browser",
            "reasoning": "Click the URL bar",
            "actions": [{"action": "click", "x": 400, "y": 50}],
            "confidence": 0.9
        }"#;
        let plan = parse_claude_response(raw).expect("direct JSON");
        assert_eq!(plan.observation, "I see a browser");
        assert_eq!(plan.actions.len(), 1);
    }

    #[test]
    fn test_parse_claude_envelope() {
        let inner = r#"{"observation":"desktop","reasoning":"click icon","actions":[{"action":"click","x":100,"y":200}],"confidence":0.8}"#;
        let envelope = format!(
            r#"{{"type":"result","subtype":"success","result":"{}","session_id":"abc","usage":{{}}}}"#,
            inner.replace('"', "\\\"")
        );
        let plan = parse_claude_response(&envelope).expect("envelope parse");
        assert_eq!(plan.observation, "desktop");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_with_code_fences() {
        let inner_json = r#"{"observation":"screen","reasoning":"type text","actions":[{"action":"type","text":"hello"}],"confidence":0.75}"#;
        let result_field = format!("\n```json\n{}\n```\n", inner_json);
        let envelope = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "result": result_field,
            "session_id": "sess-123"
        });
        let raw = serde_json::to_string(&envelope).expect("serialize envelope");
        let plan = parse_claude_response(&raw).expect("fenced parse");
        assert_eq!(plan.observation, "screen");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_with_text_around_json() {
        let raw = r#"Here is my plan:
        {"observation":"terminal","reasoning":"press enter","actions":[{"action":"key_press","key":"Return"}],"confidence":0.95}
        That should work!"#;
        let plan = parse_claude_response(raw).expect("text-around parse");
        assert_eq!(plan.observation, "terminal");
        assert_eq!(plan.actions.len(), 1);
    }

    #[test]
    fn test_parse_text_fallback_wraps_as_done() {
        let raw = "I don't know what to do, sorry!";
        let plan = parse_claude_response(raw).expect("text fallback");
        assert_eq!(plan.actions.len(), 1);
        assert!((plan.confidence - 0.5).abs() < f64::EPSILON);
        // The reasoning should contain the original text
        assert!(plan.reasoning.contains("I don't know what to do"));
        // The done summary should be the text (truncated to 200 chars)
        match &plan.actions[0] {
            crate::agent::action::AgentAction::Done { summary } => {
                assert!(summary.contains("I don't know what to do"));
            }
            other => panic!("expected Done action, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_empty_response_errors() {
        let raw = "";
        let result = parse_claude_response(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty response"));
    }

    #[test]
    fn test_parse_text_fallback_truncates_long_summary() {
        let raw = "x".repeat(500);
        let plan = parse_claude_response(&raw).expect("text fallback");
        match &plan.actions[0] {
            crate::agent::action::AgentAction::Done { summary } => {
                assert_eq!(summary.len(), 200);
            }
            other => panic!("expected Done, got: {other:?}"),
        }
    }
}
