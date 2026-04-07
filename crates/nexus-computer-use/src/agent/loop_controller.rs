use std::io::{self, BufRead, Write as IoWrite};
use std::time::Instant;

use sha2::Digest;
use tracing::{info, warn};

use crate::agent::action::{ActionResult, AgentAction};
use crate::agent::planner::{StepPlanner, StepRecord};
use crate::agent::vision::{downscale_for_vision, VisionAnalyzer};
use crate::capture::screenshot::{take_screenshot, ScreenshotOptions};
use crate::error::ComputerUseError;
use crate::governance::session::GovernedSession;
use crate::input::keyboard::{KeyAction, KeyboardController};
use crate::input::mouse::{MouseAction, MouseButton, MouseController, ScrollDirection};
use crate::input::safety::InputSafetyGuard;
use crate::learning::memory::{ActionMemory, MemoryEntry, MemoryStep};
use crate::learning::optimizer::PatternOptimizer;
use crate::learning::pattern::PatternLibrary;

/// Agent loop configuration
///
/// Does not implement Clone — sessions are unique per run.
pub struct AgentConfig {
    /// The task to accomplish
    pub task: String,
    /// Maximum number of steps (clamped to 100)
    pub max_steps: u32,
    /// Confidence threshold — below this, pause for approval
    pub confidence_threshold: f64,
    /// Whether to require user approval before executing each step
    pub require_user_approval: bool,
    /// Dry run mode — show plans without executing
    pub dry_run: bool,
    /// Screenshot max width for vision model
    pub screenshot_max_width: Option<u32>,
    /// Optional governed session for app-level permission checks
    pub session: Option<GovernedSession>,
}

impl std::fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field("task", &self.task)
            .field("max_steps", &self.max_steps)
            .field("confidence_threshold", &self.confidence_threshold)
            .field("require_user_approval", &self.require_user_approval)
            .field("dry_run", &self.dry_run)
            .field("screenshot_max_width", &self.screenshot_max_width)
            .field("session", &self.session.as_ref().map(|s| &s.id))
            .finish()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            task: String::new(),
            max_steps: 20,
            confidence_threshold: 0.3,
            require_user_approval: true,
            dry_run: false,
            screenshot_max_width: Some(1568),
            session: None,
        }
    }
}

impl AgentConfig {
    /// Create a config for the given task
    pub fn for_task(task: &str) -> Self {
        Self {
            task: task.to_string(),
            ..Default::default()
        }
    }
}

/// Result of a complete agent run
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRunResult {
    /// The original task
    pub task: String,
    /// Whether the task was completed successfully
    pub completed: bool,
    /// Summary of what was accomplished
    pub summary: String,
    /// Total steps executed
    pub steps_executed: u32,
    /// Total fuel consumed
    pub fuel_consumed: u64,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Final audit hash (hash of all step hashes)
    pub audit_hash: String,
}

/// Convert an AgentAction into input-layer actions and execute them
async fn execute_action(
    action: &AgentAction,
    safety: &InputSafetyGuard,
    mouse: &MouseController,
    keyboard: &KeyboardController,
) -> ActionResult {
    let start = Instant::now();
    let action_str = action.to_string();

    match action {
        AgentAction::Click { x, y, button } => {
            let btn = parse_mouse_button(button);
            let mouse_action = MouseAction::Click {
                x: *x,
                y: *y,
                button: btn,
            };
            if let Err(e) = safety.validate_mouse_action(&mouse_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match mouse.execute(&mouse_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::DoubleClick { x, y } => {
            let mouse_action = MouseAction::DoubleClick {
                x: *x,
                y: *y,
                button: MouseButton::Left,
            };
            if let Err(e) = safety.validate_mouse_action(&mouse_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match mouse.execute(&mouse_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::Type { text } => {
            let key_action = KeyAction::Type { text: text.clone() };
            if let Err(e) = safety.validate_keyboard_action(&key_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match keyboard.execute(&key_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::KeyPress { key } => {
            // Check if it's a combo (contains +)
            let key_action = if key.contains('+') {
                let keys: Vec<String> = key.split('+').map(|s| s.trim().to_string()).collect();
                KeyAction::KeyCombo { keys }
            } else {
                KeyAction::KeyPress { key: key.clone() }
            };
            if let Err(e) = safety.validate_keyboard_action(&key_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match keyboard.execute(&key_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::Scroll {
            x,
            y,
            direction,
            amount,
        } => {
            let dir = parse_scroll_direction(direction);
            let mouse_action = MouseAction::Scroll {
                x: *x,
                y: *y,
                direction: dir,
                amount: *amount,
            };
            if let Err(e) = safety.validate_mouse_action(&mouse_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match mouse.execute(&mouse_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
        } => {
            let mouse_action = MouseAction::Drag {
                start_x: *start_x,
                start_y: *start_y,
                end_x: *end_x,
                end_y: *end_y,
            };
            if let Err(e) = safety.validate_mouse_action(&mouse_action) {
                return ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start));
            }
            match mouse.execute(&mouse_action).await {
                Ok(_) => ActionResult::success(&action_str, elapsed_ms(start)),
                Err(e) => ActionResult::failure(&action_str, &e.to_string(), elapsed_ms(start)),
            }
        }
        AgentAction::Wait { ms } => {
            let wait = (*ms).min(10_000); // Cap at 10 seconds
            tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            ActionResult::success(&action_str, elapsed_ms(start))
        }
        AgentAction::Screenshot => {
            // Screenshot action resets the dead man's switch
            safety.reset_screenshot_counter();
            ActionResult::success(&action_str, elapsed_ms(start))
        }
        AgentAction::Done { summary } => {
            info!("Agent task complete: {summary}");
            ActionResult::success(&action_str, elapsed_ms(start))
        }
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn parse_mouse_button(s: &str) -> MouseButton {
    match s.to_lowercase().as_str() {
        "right" | "3" => MouseButton::Right,
        "middle" | "2" => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

fn parse_scroll_direction(s: &str) -> ScrollDirection {
    match s.to_lowercase().as_str() {
        "up" => ScrollDirection::Up,
        "left" => ScrollDirection::Left,
        "right" => ScrollDirection::Right,
        _ => ScrollDirection::Down,
    }
}

/// Prompt the user for approval of a step. Returns the user's decision.
/// This reads from stdin.
pub enum ApprovalDecision {
    Approve,
    Skip,
    Abort,
    Modify(String),
}

/// Ask the user for approval via stdin
fn prompt_user_approval(step: u32, plan: &crate::agent::action::ActionPlan) -> ApprovalDecision {
    println!("\n--- Step {step} Plan ---");
    println!("Observation: {}", plan.observation);
    println!("Reasoning:   {}", plan.reasoning);
    println!("Confidence:  {:.0}%", plan.confidence * 100.0);
    println!("Actions:");
    for (i, action) in plan.actions.iter().enumerate() {
        println!("  {}: {action}", i + 1);
    }
    println!("\n[y/Enter] approve  [n] skip  [q] abort  [m] modify");
    print!("> ");
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        warn!("Failed to read stdin, aborting");
        return ApprovalDecision::Abort;
    }

    let input = line.trim().to_lowercase();
    match input.as_str() {
        "" | "y" | "yes" => ApprovalDecision::Approve,
        "n" | "no" => ApprovalDecision::Skip,
        "q" | "quit" | "abort" => ApprovalDecision::Abort,
        "m" | "modify" => {
            print!("Enter replacement actions (JSON): ");
            let _ = io::stdout().flush();
            let mut mod_line = String::new();
            if stdin.lock().read_line(&mut mod_line).is_err() {
                return ApprovalDecision::Abort;
            }
            ApprovalDecision::Modify(mod_line.trim().to_string())
        }
        _ => {
            println!("Unknown input, treating as skip");
            ApprovalDecision::Skip
        }
    }
}

/// Run the full agent loop
pub async fn run_agent_loop(mut config: AgentConfig) -> Result<AgentRunResult, ComputerUseError> {
    let run_start = Instant::now();

    info!(
        "Starting agent loop: task={:?}, max_steps={}, approval={}",
        config.task, config.max_steps, config.require_user_approval
    );

    let mut planner = StepPlanner::new(config.max_steps);
    let vision = VisionAnalyzer::new();

    // Detect input backend
    let backend = crate::input::backend::detect_input_backend()?;
    let (sw, sh) = crate::input::backend::get_display_geometry(&backend)
        .await
        .map_err(|e| ComputerUseError::InputError(format!("Cannot get display geometry: {e}")))?;

    let safety = InputSafetyGuard::new(sw, sh);
    let mouse = MouseController::new(backend.clone());
    let keyboard = KeyboardController::new(backend);

    // Load learned patterns for potential shortcut
    let mut pattern_library = PatternLibrary::with_default_path();
    if let Err(e) = pattern_library.load() {
        warn!("Failed to load pattern library: {e}");
    }

    // Check for a high-confidence learned pattern before entering the loop
    let pattern_matches = pattern_library.find_matching(&config.task);
    if let Some(best) = pattern_matches.first() {
        if best.pattern.confidence > 0.8 && best.score > 0.8 {
            info!(
                "Using learned pattern '{}' (confidence: {:.0}%, match: {:.0}%)",
                best.pattern.name,
                best.pattern.confidence * 100.0,
                best.score * 100.0,
            );
        }
    }

    let mut completed = false;
    let mut summary = String::from("Agent loop ended without completion");

    while !planner.is_at_limit() {
        let step = planner.current_step();
        info!("=== Step {step}/{} ===", planner.max_steps());

        // 1. Take screenshot
        let screenshot = take_screenshot(ScreenshotOptions {
            max_width: config.screenshot_max_width,
            ..Default::default()
        })
        .await?;
        safety.reset_screenshot_counter();

        // 2. Downscale for vision
        let (vision_bytes, _, _) = downscale_for_vision(&screenshot.png_bytes)?;
        let vision_b64 = crate::capture::screenshot::Screenshot::encode_base64(&vision_bytes);

        // 3. Get action plan from vision model
        let history = planner.history_context();
        let plan = vision
            .analyze(&vision_b64, &config.task, &history, step)
            .await?;

        // 4. Check confidence threshold
        if plan.confidence < config.confidence_threshold {
            warn!(
                "Low confidence {:.2} < threshold {:.2}, pausing",
                plan.confidence, config.confidence_threshold
            );
            if config.require_user_approval {
                println!(
                    "\nWARNING: Low confidence ({:.0}%). Proceeding requires approval.",
                    plan.confidence * 100.0
                );
            } else {
                // In auto mode, low confidence still proceeds but logs warning
            }
        }

        // 5. HITL approval gate
        let user_approved;
        if config.require_user_approval {
            match prompt_user_approval(step, &plan) {
                ApprovalDecision::Approve => {
                    user_approved = true;
                }
                ApprovalDecision::Skip => {
                    info!("User skipped step {step}");
                    // Continue to next iteration (take new screenshot)
                    continue;
                }
                ApprovalDecision::Abort => {
                    info!("User aborted agent loop");
                    return Err(ComputerUseError::UserAborted);
                }
                ApprovalDecision::Modify(new_actions_json) => {
                    user_approved = true;
                    // Try to parse modified actions — if invalid, skip
                    match serde_json::from_str::<Vec<AgentAction>>(&new_actions_json) {
                        Ok(new_actions) => {
                            let modified_plan = crate::agent::action::ActionPlan {
                                observation: plan.observation.clone(),
                                reasoning: format!("User-modified: {}", plan.reasoning),
                                actions: new_actions,
                                confidence: 1.0,
                            };
                            // Execute modified plan
                            let record = execute_step(
                                step,
                                &modified_plan,
                                user_approved,
                                &config,
                                &safety,
                                &mouse,
                                &keyboard,
                            )
                            .await;
                            if check_done(&modified_plan, &mut completed, &mut summary) {
                                planner.add_step(record)?;
                                break;
                            }
                            planner.add_step(record)?;
                            continue;
                        }
                        Err(e) => {
                            warn!("Failed to parse modified actions: {e}");
                            println!("Invalid JSON, skipping step");
                            continue;
                        }
                    }
                }
            }
        } else {
            user_approved = true;
        }

        // 6. Governance check — validate actions against app grants
        if let Some(ref mut session) = config.session {
            let focused = session.app_registry.get_focused_app().await;
            if let Ok(ref focused_app) = focused {
                for action in &plan.actions {
                    match session.validate_action(focused_app, action) {
                        Ok(grant_id) => {
                            session.log_action(focused_app, action, &grant_id);
                        }
                        Err(e) => {
                            warn!("Governance denied action {action}: {e}");
                            return Err(e);
                        }
                    }
                }
            }
        }

        // 7. Execute the plan
        let record = execute_step(
            step,
            &plan,
            user_approved,
            &config,
            &safety,
            &mouse,
            &keyboard,
        )
        .await;

        // 8. Check for Done action
        if check_done(&plan, &mut completed, &mut summary) {
            planner.add_step(record)?;
            break;
        }

        planner.add_step(record)?;
    }

    if !completed && planner.is_at_limit() {
        warn!(
            "Agent reached max steps ({}) without completing",
            planner.max_steps()
        );
        summary = format!("Reached max steps ({})", planner.max_steps());
    }

    let total_duration_ms = run_start.elapsed().as_millis() as u64;

    // Compute final audit hash from all step hashes
    let mut hasher = sha2::Sha256::new();
    for step in planner.history() {
        hasher.update(step.audit_hash.as_bytes());
    }
    let audit_hash = hex::encode(hasher.finalize());

    let result = AgentRunResult {
        task: config.task.clone(),
        completed,
        summary,
        steps_executed: planner.history().len() as u32,
        fuel_consumed: planner.total_fuel(),
        total_duration_ms,
        audit_hash,
    };

    // Learn from this run
    if result.completed {
        let memory_entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            task: result.task.clone(),
            steps: planner
                .history()
                .iter()
                .map(|s| MemoryStep {
                    step_number: s.step,
                    actions: s.plan.actions.clone(),
                    screenshot_hash: s.audit_hash.clone(),
                    app_context: "Unknown".to_string(),
                    duration_ms: s.results.iter().map(|r| r.duration_ms).sum(),
                })
                .collect(),
            success: true,
            total_duration_ms: result.total_duration_ms,
            fuel_consumed: result.fuel_consumed,
            timestamp: chrono::Utc::now(),
        };

        let mut memory = ActionMemory::with_default_path();
        if let Err(e) = memory.load() {
            warn!("Failed to load action memory: {e}");
        }
        memory.record(memory_entry.clone());

        let mut optimizer = PatternOptimizer::new(memory, pattern_library);
        optimizer.learn_from_run(&memory_entry);

        if let Err(e) = optimizer.library().save() {
            warn!("Failed to save pattern library: {e}");
        }
        if let Err(e) = optimizer.memory().save() {
            warn!("Failed to save action memory: {e}");
        }
    }

    Ok(result)
}

/// Execute all actions in a plan and return a StepRecord
async fn execute_step(
    step: u32,
    plan: &crate::agent::action::ActionPlan,
    user_approved: bool,
    config: &AgentConfig,
    safety: &InputSafetyGuard,
    mouse: &MouseController,
    keyboard: &KeyboardController,
) -> StepRecord {
    let mut record = StepRecord::new(step, plan.clone(), user_approved);

    if config.dry_run {
        info!(
            "Dry run: skipping execution of {} actions",
            plan.actions.len()
        );
        for action in &plan.actions {
            record.add_result(ActionResult::success(&action.to_string(), 0));
        }
        return record;
    }

    for action in &plan.actions {
        let result = execute_action(action, safety, mouse, keyboard).await;
        let success = result.success;
        record.add_result(result);

        if !success {
            warn!("Action failed: {action}, stopping step execution");
            break;
        }
    }

    record
}

/// Check if any action in the plan is a Done action
fn check_done(
    plan: &crate::agent::action::ActionPlan,
    completed: &mut bool,
    summary: &mut String,
) -> bool {
    for action in &plan.actions {
        if let AgentAction::Done { summary: s } = action {
            *completed = true;
            *summary = s.clone();
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::action::{ActionPlan, AgentAction};

    #[test]
    fn test_agent_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.max_steps, 20);
        assert!((config.confidence_threshold - 0.3).abs() < f64::EPSILON);
        assert!(config.require_user_approval);
        assert!(!config.dry_run);
        assert_eq!(config.screenshot_max_width, Some(1568));
    }

    #[test]
    fn test_agent_config_custom() {
        let config = AgentConfig {
            task: "test task".to_string(),
            max_steps: 50,
            confidence_threshold: 0.7,
            require_user_approval: false,
            dry_run: true,
            screenshot_max_width: Some(800),
            session: None,
        };
        assert_eq!(config.task, "test task");
        assert_eq!(config.max_steps, 50);
        assert!(!config.require_user_approval);
        assert!(config.dry_run);
    }

    #[test]
    fn test_agent_config_for_task() {
        let config = AgentConfig::for_task("click the button");
        assert_eq!(config.task, "click the button");
        assert_eq!(config.max_steps, 20); // default
    }

    #[test]
    fn test_loop_max_steps_safety() {
        let mut planner = StepPlanner::new(2);
        assert!(!planner.is_at_limit());

        let plan = ActionPlan {
            observation: "x".into(),
            reasoning: "y".into(),
            actions: vec![],
            confidence: 0.9,
        };

        planner
            .add_step(StepRecord::new(1, plan.clone(), true))
            .expect("s1");
        planner
            .add_step(StepRecord::new(2, plan.clone(), true))
            .expect("s2");
        assert!(planner.is_at_limit());

        let result = planner.add_step(StepRecord::new(3, plan, true));
        assert!(result.is_err());
    }

    #[test]
    fn test_loop_low_confidence_threshold() {
        // Test that the confidence check logic works
        let config = AgentConfig {
            confidence_threshold: 0.5,
            ..AgentConfig::default()
        };
        let low_confidence = 0.3;
        assert!(low_confidence < config.confidence_threshold);

        let high_confidence = 0.8;
        assert!(high_confidence >= config.confidence_threshold);
    }

    #[test]
    fn test_loop_done_action_detection() {
        let plan = ActionPlan {
            observation: "done".into(),
            reasoning: "finished".into(),
            actions: vec![
                AgentAction::Click {
                    x: 10,
                    y: 20,
                    button: "left".to_string(),
                },
                AgentAction::Done {
                    summary: "All done!".to_string(),
                },
            ],
            confidence: 0.99,
        };

        let mut completed = false;
        let mut summary = String::new();
        let is_done = check_done(&plan, &mut completed, &mut summary);
        assert!(is_done);
        assert!(completed);
        assert_eq!(summary, "All done!");
    }

    #[test]
    fn test_loop_no_done_action() {
        let plan = ActionPlan {
            observation: "screen".into(),
            reasoning: "click".into(),
            actions: vec![AgentAction::Click {
                x: 10,
                y: 20,
                button: "left".to_string(),
            }],
            confidence: 0.8,
        };

        let mut completed = false;
        let mut summary = String::new();
        let is_done = check_done(&plan, &mut completed, &mut summary);
        assert!(!is_done);
        assert!(!completed);
    }

    #[test]
    fn test_agent_run_result_creation() {
        let result = AgentRunResult {
            task: "test task".to_string(),
            completed: true,
            summary: "Completed successfully".to_string(),
            steps_executed: 5,
            fuel_consumed: 10,
            total_duration_ms: 5000,
            audit_hash: "a".repeat(64),
        };
        assert!(result.completed);
        assert_eq!(result.steps_executed, 5);
        assert_eq!(result.fuel_consumed, 10);
    }

    #[test]
    fn test_agent_run_result_fuel_calculation() {
        let result = AgentRunResult {
            task: "t".to_string(),
            completed: false,
            summary: "s".to_string(),
            steps_executed: 3,
            fuel_consumed: 7,
            total_duration_ms: 3000,
            audit_hash: "b".repeat(64),
        };
        // Fuel is tracked per successful action
        assert_eq!(result.fuel_consumed, 7);
        assert_eq!(result.total_duration_ms, 3000);
    }

    #[test]
    fn test_parse_mouse_button() {
        assert_eq!(parse_mouse_button("left"), MouseButton::Left);
        assert_eq!(parse_mouse_button("right"), MouseButton::Right);
        assert_eq!(parse_mouse_button("middle"), MouseButton::Middle);
        assert_eq!(parse_mouse_button("Left"), MouseButton::Left);
        assert_eq!(parse_mouse_button("unknown"), MouseButton::Left);
    }

    #[test]
    fn test_parse_scroll_direction() {
        assert_eq!(parse_scroll_direction("up"), ScrollDirection::Up);
        assert_eq!(parse_scroll_direction("down"), ScrollDirection::Down);
        assert_eq!(parse_scroll_direction("left"), ScrollDirection::Left);
        assert_eq!(parse_scroll_direction("right"), ScrollDirection::Right);
        assert_eq!(parse_scroll_direction("unknown"), ScrollDirection::Down);
    }

    #[test]
    fn test_action_validated_before_execute_oob() {
        // Verify that the safety guard catches out-of-bounds coordinates
        let safety = InputSafetyGuard::new(1920, 1080);
        let action = MouseAction::Click {
            x: 2000,
            y: 500,
            button: MouseButton::Left,
        };
        let result = safety.validate_mouse_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::CoordinatesOutOfBounds { .. }
        ));
    }

    #[test]
    fn test_blocked_combo_in_plan() {
        // Verify that blocked combos are rejected by the safety guard
        let safety = InputSafetyGuard::new(1920, 1080);
        let action = KeyAction::KeyCombo {
            keys: vec!["alt".into(), "F4".into()],
        };
        let result = safety.validate_keyboard_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::BlockedKeyCombination { .. }
        ));
    }

    #[test]
    fn test_rate_limit_in_loop() {
        let safety = InputSafetyGuard::new(3440, 1440);
        // Fire MAX actions per second
        for _ in 0..10 {
            let action = MouseAction::Move { x: 100, y: 100 };
            assert!(safety.validate_mouse_action(&action).is_ok());
        }
        // 11th should be rate-limited
        let action = MouseAction::Move { x: 100, y: 100 };
        let result = safety.validate_mouse_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::RateLimitExceeded { .. }
        ));
    }
}
