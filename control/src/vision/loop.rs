use crate::action_log::{ActionLogger, ActionType};
use crate::capture::{ScreenCaptureBackend, ScreenCaptureService, Screenshot};
use crate::vision::cost::{BudgetSignal, VisionCostController};
use crate::ControlAgentContext;
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionAction {
    Click { x: i32, y: i32 },
    TypeText { text: String },
    KeyPress { key: String },
    Done,
    NoOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionInference {
    pub response: String,
    pub token_count: u32,
}

pub trait VisionModel {
    fn infer(
        &mut self,
        screenshot_hash: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<VisionInference, AgentError>;
}

pub struct ProviderVisionModel<P: LlmProvider> {
    provider: P,
    model_name: String,
}

impl<P: LlmProvider> ProviderVisionModel<P> {
    pub fn new(provider: P, model_name: impl Into<String>) -> Self {
        Self {
            provider,
            model_name: model_name.into(),
        }
    }
}

impl<P: LlmProvider> VisionModel for ProviderVisionModel<P> {
    fn infer(
        &mut self,
        screenshot_hash: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<VisionInference, AgentError> {
        let composed_prompt =
            format!("Task: {prompt}\nScreenshot hash: {screenshot_hash}\nReturn one action.");
        let response = self.provider.query(
            composed_prompt.as_str(),
            max_tokens,
            self.model_name.as_str(),
        )?;

        Ok(VisionInference {
            response: response.output_text,
            token_count: response.token_count,
        })
    }
}

pub trait VisionExecutor {
    fn execute(&mut self, action: &VisionAction) -> Result<(), AgentError>;
}

pub trait VisionVerifier {
    fn verify(&mut self, step: u32, action: &VisionAction) -> Result<bool, AgentError>;
}

#[derive(Debug, Default, Clone)]
pub struct NoOpVisionExecutor;

impl VisionExecutor for NoOpVisionExecutor {
    fn execute(&mut self, _action: &VisionAction) -> Result<(), AgentError> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct NoOpVisionVerifier;

impl VisionVerifier for NoOpVisionVerifier {
    fn verify(&mut self, _step: u32, action: &VisionAction) -> Result<bool, AgentError> {
        Ok(matches!(action, VisionAction::Done))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionOutcome {
    Completed,
    PartialCompletion,
    PausedBudget,
    StoppedBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionLoopConfig {
    pub step_limit: u32,
    pub min_capture_interval_secs: u64,
    pub max_capture_interval_secs: u64,
    pub max_tokens_per_step: u32,
    pub vision_fuel_multiplier: u64,
    pub base_text_step_cost: u64,
    pub ephemeral_screenshots: bool,
    pub hard_pause_at_ninety: bool,
}

impl Default for VisionLoopConfig {
    fn default() -> Self {
        Self {
            step_limit: 50,
            min_capture_interval_secs: 2,
            max_capture_interval_secs: 30,
            max_tokens_per_step: 64,
            vision_fuel_multiplier: 10,
            base_text_step_cost: 10,
            ephemeral_screenshots: true,
            hard_pause_at_ninety: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionLoopReport {
    pub outcome: VisionOutcome,
    pub steps_executed: u32,
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
    pub screenshot_hashes: Vec<String>,
    pub interval_plan_secs: Vec<u64>,
}

pub struct VisionLoop<B: ScreenCaptureBackend, M: VisionModel, E: VisionExecutor, V: VisionVerifier>
{
    capture_service: ScreenCaptureService<B>,
    model: M,
    executor: E,
    verifier: V,
    logger: ActionLogger,
    config: VisionLoopConfig,
    screenshot_dir: PathBuf,
}

impl<B: ScreenCaptureBackend, M: VisionModel, E: VisionExecutor, V: VisionVerifier>
    VisionLoop<B, M, E, V>
{
    pub fn new(
        capture_backend: B,
        model: M,
        executor: E,
        verifier: V,
        logger: ActionLogger,
        config: VisionLoopConfig,
    ) -> Result<Self, AgentError> {
        let screenshot_dir = std::env::temp_dir().join(format!("nexus-vision-{}", Uuid::new_v4()));
        fs::create_dir_all(&screenshot_dir).map_err(|error| {
            AgentError::SupervisorError(format!("failed to create screenshot dir: {error}"))
        })?;

        Ok(Self {
            capture_service: ScreenCaptureService::new(capture_backend, logger.clone()),
            model,
            executor,
            verifier,
            logger,
            config,
            screenshot_dir,
        })
    }

    pub fn with_screenshot_dir(
        capture_backend: B,
        model: M,
        executor: E,
        verifier: V,
        logger: ActionLogger,
        config: VisionLoopConfig,
        screenshot_dir: PathBuf,
    ) -> Result<Self, AgentError> {
        fs::create_dir_all(&screenshot_dir).map_err(|error| {
            AgentError::SupervisorError(format!("failed to create screenshot dir: {error}"))
        })?;

        Ok(Self {
            capture_service: ScreenCaptureService::new(capture_backend, logger.clone()),
            model,
            executor,
            verifier,
            logger,
            config,
            screenshot_dir,
        })
    }

    pub fn run(
        &mut self,
        context: &ControlAgentContext,
        task_prompt: &str,
        fuel_budget: u64,
    ) -> Result<VisionLoopReport, AgentError> {
        ensure_capability(context, "screen.capture")?;
        ensure_capability(context, "llm.query")?;

        let mut cost_controller = VisionCostController::new(fuel_budget);
        let mut screenshot_hashes = Vec::new();
        let mut interval_plan_secs = Vec::new();
        let mut steps_executed = 0_u32;
        let mut current_interval = self.config.min_capture_interval_secs;
        let mut previous_hash: Option<String> = None;

        for step_index in 0..self.config.step_limit {
            let screenshot = self.capture_service.capture_screen(context)?;
            let screenshot_hash = hash_screenshot(&screenshot);
            let screenshot_path = self.persist_screenshot(step_index, &screenshot)?;

            let _ = self.logger.log_action(
                ActionType::VisionCapture,
                context.agent_id,
                None,
                screenshot.window_id.as_deref(),
                json!({
                    "screenshot_hash": screenshot_hash,
                    "platform": screenshot.platform,
                    "ephemeral": self.config.ephemeral_screenshots,
                }),
            );

            let inference = self.model.infer(
                screenshot_hash.as_str(),
                task_prompt,
                self.config.max_tokens_per_step,
            )?;

            let action = parse_vision_action(inference.response.as_str());
            ensure_action_capability(context, &action)?;

            self.executor.execute(&action)?;
            let _ = self.logger.log_action(
                ActionType::VisionAction,
                context.agent_id,
                None,
                None,
                json!({"action": format!("{:?}", action)}),
            );

            let verified = self.verifier.verify(step_index + 1, &action)?;
            let _ = self.logger.log_action(
                ActionType::VisionVerify,
                context.agent_id,
                None,
                None,
                json!({"verified": verified}),
            );

            let token_cost = u64::from(inference.token_count).max(self.config.base_text_step_cost);
            let step_cost = token_cost.saturating_mul(self.config.vision_fuel_multiplier);
            let snapshot = cost_controller.consume(step_cost);

            match snapshot.signal {
                BudgetSignal::Alert50 => {
                    let _ = self.logger.log_action(
                        ActionType::VisionBudgetAlert,
                        context.agent_id,
                        None,
                        None,
                        json!({"threshold": 50, "remaining": snapshot.remaining}),
                    );
                }
                BudgetSignal::Pause90 => {
                    let _ = self.logger.log_action(
                        ActionType::VisionBudgetPause,
                        context.agent_id,
                        None,
                        None,
                        json!({"threshold": 90, "remaining": snapshot.remaining}),
                    );
                    if self.config.hard_pause_at_ninety {
                        self.finalize_screenshot_file(&screenshot_path)?;
                        screenshot_hashes.push(screenshot_hash);
                        steps_executed = steps_executed.saturating_add(1);
                        return Ok(VisionLoopReport {
                            outcome: VisionOutcome::PausedBudget,
                            steps_executed,
                            fuel_consumed: cost_controller.consumed(),
                            fuel_remaining: cost_controller.remaining(),
                            screenshot_hashes,
                            interval_plan_secs,
                        });
                    }
                }
                BudgetSignal::Stop100 => {
                    let _ = self.logger.log_action(
                        ActionType::VisionBudgetStop,
                        context.agent_id,
                        None,
                        None,
                        json!({"threshold": 100, "remaining": snapshot.remaining}),
                    );
                    self.finalize_screenshot_file(&screenshot_path)?;
                    screenshot_hashes.push(screenshot_hash);
                    steps_executed = steps_executed.saturating_add(1);
                    return Ok(VisionLoopReport {
                        outcome: VisionOutcome::StoppedBudget,
                        steps_executed,
                        fuel_consumed: cost_controller.consumed(),
                        fuel_remaining: cost_controller.remaining(),
                        screenshot_hashes,
                        interval_plan_secs,
                    });
                }
                BudgetSignal::Normal => {}
            }

            if let Some(previous) = previous_hash.as_ref() {
                if previous == &screenshot_hash {
                    current_interval = current_interval
                        .saturating_mul(2)
                        .min(self.config.max_capture_interval_secs.max(1));
                } else {
                    current_interval = self.config.min_capture_interval_secs.max(1);
                }
            }

            interval_plan_secs.push(current_interval);
            previous_hash = Some(screenshot_hash.clone());
            screenshot_hashes.push(screenshot_hash);

            self.finalize_screenshot_file(&screenshot_path)?;

            steps_executed = steps_executed.saturating_add(1);
            if verified || matches!(action, VisionAction::Done) {
                return Ok(VisionLoopReport {
                    outcome: VisionOutcome::Completed,
                    steps_executed,
                    fuel_consumed: cost_controller.consumed(),
                    fuel_remaining: cost_controller.remaining(),
                    screenshot_hashes,
                    interval_plan_secs,
                });
            }
        }

        Ok(VisionLoopReport {
            outcome: VisionOutcome::PartialCompletion,
            steps_executed,
            fuel_consumed: cost_controller.consumed(),
            fuel_remaining: cost_controller.remaining(),
            screenshot_hashes,
            interval_plan_secs,
        })
    }

    pub fn screenshot_dir(&self) -> &Path {
        &self.screenshot_dir
    }

    pub fn screenshot_file_count(&self) -> Result<usize, AgentError> {
        count_files(self.screenshot_dir.as_path())
    }

    fn persist_screenshot(
        &self,
        step_index: u32,
        screenshot: &Screenshot,
    ) -> Result<PathBuf, AgentError> {
        let path = self.screenshot_dir.join(format!("step-{step_index}.rgba"));
        fs::write(path.as_path(), &screenshot.pixels_rgba).map_err(|error| {
            AgentError::SupervisorError(format!("failed to persist screenshot: {error}"))
        })?;
        Ok(path)
    }

    fn finalize_screenshot_file(&self, path: &Path) -> Result<(), AgentError> {
        if self.config.ephemeral_screenshots && path.exists() {
            fs::remove_file(path).map_err(|error| {
                AgentError::SupervisorError(format!("failed to delete screenshot file: {error}"))
            })?;
        }

        Ok(())
    }
}

fn ensure_capability(context: &ControlAgentContext, capability: &str) -> Result<(), AgentError> {
    if !context.has_capability(capability) {
        return Err(AgentError::CapabilityDenied(capability.to_string()));
    }

    Ok(())
}

fn ensure_action_capability(
    context: &ControlAgentContext,
    action: &VisionAction,
) -> Result<(), AgentError> {
    match action {
        VisionAction::Click { .. } => ensure_capability(context, "input.mouse"),
        VisionAction::TypeText { .. } | VisionAction::KeyPress { .. } => {
            ensure_capability(context, "input.keyboard")
        }
        VisionAction::Done | VisionAction::NoOp => Ok(()),
    }
}

fn hash_screenshot(screenshot: &Screenshot) -> String {
    let mut hasher = Sha256::new();
    hasher.update(screenshot.width.to_le_bytes());
    hasher.update(screenshot.height.to_le_bytes());
    hasher.update(&screenshot.pixels_rgba);
    format!("{:x}", hasher.finalize())
}

fn count_files(dir: &Path) -> Result<usize, AgentError> {
    let entries = fs::read_dir(dir).map_err(|error| {
        AgentError::SupervisorError(format!("failed to read screenshot directory: {error}"))
    })?;
    let mut count = 0_usize;
    for entry in entries {
        let _ = entry.map_err(|error| {
            AgentError::SupervisorError(format!("failed to inspect screenshot file: {error}"))
        })?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

pub fn parse_vision_action(raw: &str) -> VisionAction {
    let normalized = raw.trim();
    let lower = normalized.to_lowercase();

    if lower.starts_with("click:") {
        let coords = normalized[6..].trim();
        let mut parts = coords.split(',');
        let x = parts
            .next()
            .and_then(|value| value.trim().parse::<i32>().ok());
        let y = parts
            .next()
            .and_then(|value| value.trim().parse::<i32>().ok());
        if let (Some(x), Some(y)) = (x, y) {
            return VisionAction::Click { x, y };
        }
    }

    if lower.starts_with("type:") {
        return VisionAction::TypeText {
            text: normalized[5..].trim().to_string(),
        };
    }

    if lower.starts_with("key:") {
        return VisionAction::KeyPress {
            key: normalized[4..].trim().to_string(),
        };
    }

    if lower == "done" {
        return VisionAction::Done;
    }

    VisionAction::NoOp
}

#[cfg(test)]
mod tests {
    use super::{
        parse_vision_action, NoOpVisionExecutor, VisionAction, VisionInference, VisionLoop,
        VisionLoopConfig, VisionModel, VisionOutcome, VisionVerifier,
    };
    use crate::action_log::ActionLogger;
    use crate::capture::MockCaptureBackend;
    use crate::ControlAgentContext;
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    struct FixedModel {
        response: String,
        token_count: u32,
    }

    impl VisionModel for FixedModel {
        fn infer(
            &mut self,
            _screenshot_hash: &str,
            _prompt: &str,
            _max_tokens: u32,
        ) -> Result<VisionInference, AgentError> {
            Ok(VisionInference {
                response: self.response.clone(),
                token_count: self.token_count,
            })
        }
    }

    #[derive(Debug, Clone, Default)]
    struct AlwaysFalseVerifier;

    impl VisionVerifier for AlwaysFalseVerifier {
        fn verify(&mut self, _step: u32, _action: &VisionAction) -> Result<bool, AgentError> {
            Ok(false)
        }
    }

    fn context_with_caps(caps: &[&str]) -> ControlAgentContext {
        let capabilities = caps
            .iter()
            .map(|cap| (*cap).to_string())
            .collect::<HashSet<_>>();
        ControlAgentContext::new(Uuid::new_v4(), capabilities)
    }

    fn temp_dir_for_test(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nexus-vision-test-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn test_step_limit_enforced() {
        let logger = ActionLogger::new();
        let config = VisionLoopConfig {
            step_limit: 5,
            hard_pause_at_ninety: false,
            ..VisionLoopConfig::default()
        };

        let mut loop_engine = VisionLoop::with_screenshot_dir(
            MockCaptureBackend::default(),
            FixedModel {
                response: "click:10,20".to_string(),
                token_count: 10,
            },
            NoOpVisionExecutor,
            AlwaysFalseVerifier,
            logger,
            config,
            temp_dir_for_test("step-limit"),
        );
        assert!(loop_engine.is_ok());

        let context = context_with_caps(&["screen.capture", "llm.query", "input.mouse"]);
        let report = loop_engine
            .as_mut()
            .ok()
            .and_then(|engine| engine.run(&context, "perform 10 operations", 100_000).ok());
        assert!(report.is_some());

        if let Some(report) = report {
            assert_eq!(report.outcome, VisionOutcome::PartialCompletion);
            assert_eq!(report.steps_executed, 5);
        }
    }

    #[test]
    fn test_vision_fuel_multiplier() {
        let logger = ActionLogger::new();
        let config = VisionLoopConfig {
            step_limit: 10,
            hard_pause_at_ninety: false,
            vision_fuel_multiplier: 10,
            base_text_step_cost: 10,
            ..VisionLoopConfig::default()
        };

        let mut loop_engine = VisionLoop::with_screenshot_dir(
            MockCaptureBackend::default(),
            FixedModel {
                response: "noop".to_string(),
                token_count: 10,
            },
            NoOpVisionExecutor,
            AlwaysFalseVerifier,
            logger,
            config,
            temp_dir_for_test("fuel-multiplier"),
        );
        assert!(loop_engine.is_ok());

        let context = context_with_caps(&["screen.capture", "llm.query"]);
        let report = loop_engine
            .as_mut()
            .ok()
            .and_then(|engine| engine.run(&context, "loop", 1_000).ok());
        assert!(report.is_some());

        if let Some(report) = report {
            assert_eq!(report.fuel_remaining, 0);
            assert_eq!(report.fuel_consumed, 1_000);
            assert_eq!(report.steps_executed, 10);
        }
    }

    #[test]
    fn test_screenshots_ephemeral() {
        let logger = ActionLogger::new();
        let screenshot_dir = temp_dir_for_test("ephemeral");
        let config = VisionLoopConfig {
            step_limit: 3,
            ephemeral_screenshots: true,
            hard_pause_at_ninety: false,
            ..VisionLoopConfig::default()
        };

        let mut loop_engine = VisionLoop::with_screenshot_dir(
            MockCaptureBackend::default(),
            FixedModel {
                response: "noop".to_string(),
                token_count: 1,
            },
            NoOpVisionExecutor,
            AlwaysFalseVerifier,
            logger.clone(),
            config,
            screenshot_dir.clone(),
        );
        assert!(loop_engine.is_ok());

        let context = context_with_caps(&["screen.capture", "llm.query"]);
        let report = loop_engine
            .as_mut()
            .ok()
            .and_then(|engine| engine.run(&context, "observe", 1_000).ok());
        assert!(report.is_some());

        let file_count = loop_engine
            .as_ref()
            .ok()
            .and_then(|engine| engine.screenshot_file_count().ok());
        assert_eq!(file_count, Some(0));

        let events = logger.events();
        let capture_events = events
            .iter()
            .filter(|event| {
                event
                    .payload
                    .get("action_type")
                    .and_then(|value| value.as_str())
                    == Some("VisionCapture")
            })
            .collect::<Vec<_>>();

        assert!(!capture_events.is_empty());
        assert!(capture_events.iter().all(|event| {
            event
                .payload
                .get("details")
                .and_then(|value| value.get("screenshot_hash"))
                .is_some()
        }));
        assert!(capture_events.iter().all(|event| {
            event
                .payload
                .get("details")
                .and_then(|value| value.get("pixels_rgba"))
                .is_none()
        }));

        let _ = std::fs::remove_dir_all(screenshot_dir);
    }

    #[test]
    fn test_parse_vision_action() {
        assert_eq!(parse_vision_action("done"), VisionAction::Done);
        assert_eq!(
            parse_vision_action("click:1,2"),
            VisionAction::Click { x: 1, y: 2 }
        );
    }
}
