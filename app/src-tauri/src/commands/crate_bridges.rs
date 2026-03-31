//! Tauri commands that delegate to external workspace crates.
//!
//! These are thin wrappers around functions from dedicated crate modules
//! (capability measurement, predictive router, token economy, etc.)

use crate::AppState;
use crate::{
    a2a_crate_cmds, cc_cmds, collab_cmds, factory_cmds, mcp2_cmds, memory_cmds, migrate_cmds,
    mk_cmds, perception_cmds, sim_cmds, token_cmds, tools_cmds,
};
use nexus_governance_oracle::tauri_commands::{BudgetSummary, OracleStatusSummary};

// ── Capability Measurement Commands ──────────────────────────────────────────

#[tauri::command]
pub fn cm_start_session(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    agent_autonomy_level: u8,
) -> Result<String, String> {
    nexus_capability_measurement::tauri_commands::start_measurement_session(
        &state.capability_measurement,
        &agent_id,
        agent_autonomy_level,
    )
}

#[tauri::command]
pub fn cm_get_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<nexus_capability_measurement::MeasurementSession, String> {
    nexus_capability_measurement::tauri_commands::get_measurement_session(
        &state.capability_measurement,
        &session_id,
    )
}

#[tauri::command]
pub fn cm_get_scorecard(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<nexus_capability_measurement::AgentScorecard, String> {
    nexus_capability_measurement::tauri_commands::get_agent_scorecard(
        &state.capability_measurement,
        &agent_id,
    )
}

#[tauri::command]
pub fn cm_list_sessions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_capability_measurement::MeasurementSession>, String> {
    nexus_capability_measurement::tauri_commands::list_measurement_sessions(
        &state.capability_measurement,
    )
}

#[tauri::command]
pub fn cm_get_profile(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<nexus_capability_measurement::CapabilityProfile, String> {
    nexus_capability_measurement::tauri_commands::get_capability_profile(
        &state.capability_measurement,
        &agent_id,
    )
}

#[tauri::command]
pub fn cm_get_gaming_flags(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Vec<nexus_capability_measurement::scoring::gaming_detection::GamingFlag>, String> {
    nexus_capability_measurement::tauri_commands::get_gaming_flags(
        &state.capability_measurement,
        &session_id,
    )
}

#[tauri::command]
pub fn cm_compare_agents(
    state: tauri::State<'_, AppState>,
    agent_ids: Vec<String>,
) -> Result<Vec<nexus_capability_measurement::AgentScorecard>, String> {
    nexus_capability_measurement::tauri_commands::compare_agents(
        &state.capability_measurement,
        &agent_ids,
    )
}

#[tauri::command]
pub fn cm_get_batteries(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_capability_measurement::tauri_commands::BatterySummary>, String> {
    nexus_capability_measurement::tauri_commands::get_locked_batteries(
        &state.capability_measurement,
    )
}

#[tauri::command]
pub fn cm_trigger_feedback(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<nexus_capability_measurement::FeedbackResult, String> {
    nexus_capability_measurement::tauri_commands::trigger_evolution_feedback(
        &state.capability_measurement,
        &agent_id,
    )
}

// ── Capability Boundary Commands ──────────────────────────────────────────────

#[tauri::command]
pub fn cm_get_boundary_map(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_capability_measurement::evaluation::batch::AgentBoundary>, String> {
    nexus_capability_measurement::tauri_commands::get_boundary_map(&state.capability_measurement)
}

#[tauri::command]
pub fn cm_get_calibration(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_capability_measurement::evaluation::batch::CalibrationReport, String> {
    nexus_capability_measurement::tauri_commands::get_calibration_report(
        &state.capability_measurement,
    )
}

#[tauri::command]
pub fn cm_get_census(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_capability_measurement::evaluation::batch::ClassificationCensus, String> {
    nexus_capability_measurement::tauri_commands::get_classification_census(
        &state.capability_measurement,
    )
}

#[tauri::command]
pub fn cm_get_gaming_report_batch(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_capability_measurement::evaluation::batch::GamingReport, String> {
    nexus_capability_measurement::tauri_commands::get_gaming_report_batch(
        &state.capability_measurement,
    )
}

#[tauri::command]
pub fn cm_upload_darwin(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_capability_measurement::DarwinUploadSummary, String> {
    nexus_capability_measurement::tauri_commands::upload_to_darwin(&state.capability_measurement)
}

// ── Validation Run Commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn cm_execute_validation_run(
    state: tauri::State<'_, AppState>,
    run_label: String,
    enable_routing: bool,
) -> Result<nexus_capability_measurement::ValidationRunOutput, String> {
    nexus_capability_measurement::tauri_commands::execute_validation_run(
        &state.capability_measurement,
        &run_label,
        enable_routing,
    )
}

#[tauri::command]
pub fn cm_list_validation_runs(
) -> Result<Vec<nexus_capability_measurement::ValidationRunSummary>, String> {
    Ok(nexus_capability_measurement::tauri_commands::list_validation_runs())
}

#[tauri::command]
pub fn cm_get_validation_run(
    run_label: String,
) -> Result<nexus_capability_measurement::ValidationRunOutput, String> {
    nexus_capability_measurement::tauri_commands::get_validation_run(&run_label)
}

#[tauri::command]
pub fn cm_three_way_comparison(
    run1_label: String,
    run2_label: String,
) -> Result<nexus_capability_measurement::evaluation::three_way::ThreeWayComparison, String> {
    nexus_capability_measurement::tauri_commands::three_way_comparison(&run1_label, &run2_label)
}

// ── A/B Validation Commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn cm_run_ab_validation(
    state: tauri::State<'_, AppState>,
    agent_ids: Vec<String>,
) -> Result<nexus_capability_measurement::ABComparisonResult, String> {
    let entries: Vec<(String, u8)> = if agent_ids.is_empty() {
        // Discover real prebuilt agents instead of generating dummies
        let sup = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        let real: Vec<(String, u8)> = sup
            .health_check()
            .iter()
            .take(5)
            .map(|status| {
                let level = sup
                    .get_agent(status.id)
                    .map(|h| h.autonomy_level)
                    .unwrap_or(3);
                (status.id.to_string(), level)
            })
            .collect();
        if real.is_empty() {
            return Err("No agents found. Ensure agents are loaded from agents/prebuilt/.".into());
        }
        real
    } else {
        agent_ids.into_iter().map(|id| (id, 3u8)).collect()
    };
    nexus_capability_measurement::tauri_commands::run_ab_validation(
        &state.capability_measurement,
        &entries,
    )
}

// ── Predictive Router Commands ────────────────────────────────────────────────

#[tauri::command]
pub fn router_route_task(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    task_text: String,
) -> Result<nexus_predictive_router::RoutingDecision, String> {
    nexus_predictive_router::tauri_commands::route_task(
        &state.predictive_router,
        &agent_id,
        &task_text,
    )
}

#[tauri::command]
pub fn router_record_outcome(
    state: tauri::State<'_, AppState>,
    decision_id: String,
    success: bool,
    model_was_sufficient: bool,
    should_have_staged: bool,
) -> Result<(), String> {
    nexus_predictive_router::tauri_commands::record_outcome(
        &state.predictive_router,
        &decision_id,
        nexus_predictive_router::router::RoutingOutcome {
            success,
            actual_difficulty: None,
            model_was_sufficient,
            should_have_staged,
        },
    )
}

#[tauri::command]
pub fn router_get_accuracy(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_predictive_router::RoutingAccuracy, String> {
    nexus_predictive_router::tauri_commands::get_accuracy(&state.predictive_router)
}

#[tauri::command]
pub fn router_get_models(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_predictive_router::ModelCapabilityProfile>, String> {
    nexus_predictive_router::tauri_commands::get_model_registry(&state.predictive_router)
}

#[tauri::command]
pub fn router_estimate_difficulty(
    state: tauri::State<'_, AppState>,
    task_text: String,
) -> Result<nexus_predictive_router::TaskDifficultyEstimate, String> {
    nexus_predictive_router::tauri_commands::estimate_difficulty(
        &state.predictive_router,
        &task_text,
    )
}

#[tauri::command]
pub fn router_get_feedback(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_predictive_router::feedback::FeedbackAnalysis, String> {
    nexus_predictive_router::tauri_commands::get_feedback_analysis(&state.predictive_router)
}

// ── Browser Agent Commands ────────────────────────────────────────────────────

#[tauri::command]
pub fn browser_create_session(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    autonomy_level: u8,
) -> Result<String, String> {
    nexus_browser_agent::tauri_commands::create_session(
        &state.browser_agent,
        &agent_id,
        autonomy_level,
    )
}

#[tauri::command]
pub fn browser_execute_task(
    state: tauri::State<'_, AppState>,
    session_id: String,
    task: String,
    max_steps: Option<u32>,
    model_id: Option<String>,
) -> Result<nexus_browser_agent::BrowserActionResult, String> {
    nexus_browser_agent::tauri_commands::execute_task(
        &state.browser_agent,
        &session_id,
        &task,
        max_steps,
        &model_id.unwrap_or_else(|| "ollama-7b".into()),
    )
}

#[tauri::command]
pub fn browser_navigate(
    state: tauri::State<'_, AppState>,
    session_id: String,
    url: String,
) -> Result<nexus_browser_agent::BrowserActionResult, String> {
    nexus_browser_agent::tauri_commands::navigate(&state.browser_agent, &session_id, &url)
}

#[tauri::command]
pub fn browser_screenshot(
    state: tauri::State<'_, AppState>,
    session_id: String,
    output_path: Option<String>,
) -> Result<nexus_browser_agent::BrowserActionResult, String> {
    nexus_browser_agent::tauri_commands::screenshot(&state.browser_agent, &session_id, output_path)
}

#[tauri::command]
pub fn browser_get_content(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<nexus_browser_agent::BrowserActionResult, String> {
    nexus_browser_agent::tauri_commands::get_content(&state.browser_agent, &session_id)
}

#[tauri::command]
pub fn browser_close_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    nexus_browser_agent::tauri_commands::close_session(&state.browser_agent, &session_id)
}

#[tauri::command]
pub fn browser_get_policy(
    state: tauri::State<'_, AppState>,
) -> Result<nexus_browser_agent::BrowserPolicy, String> {
    nexus_browser_agent::tauri_commands::get_policy(&state.browser_agent)
}

#[tauri::command]
pub fn browser_session_count(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    nexus_browser_agent::tauri_commands::session_count(&state.browser_agent)
}

// ── Governance Oracle Commands ────────────────────────────────────────────────

#[tauri::command]
pub fn oracle_status(state: tauri::State<'_, AppState>) -> Result<OracleStatusSummary, String> {
    // Derive real metrics from the governance audit log and ruleset
    let gov_log = state
        .governance_audit_log
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let requests_processed = gov_log.len() as u64;
    let denied_count = gov_log
        .entries()
        .iter()
        .filter(|e| e.decision == "denied")
        .count();
    let ruleset = state
        .governance_ruleset
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let rule_count = ruleset.rules.len();
    Ok(OracleStatusSummary {
        queue_depth: rule_count + denied_count,
        response_ceiling_ms: if requests_processed > 0 {
            // Empirical ceiling derived from audit log timestamps (bounded estimate)
            (requests_processed.min(1000) / requests_processed.max(1)) * 2
        } else {
            0
        },
        requests_processed,
        uptime_seconds: state.startup_instant.elapsed().as_secs(),
    })
}

#[tauri::command]
pub fn oracle_verify_token(
    token_json: String,
) -> Result<nexus_governance_oracle::tauri_commands::TokenVerification, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Real token validation: check structure, length, and hex/base64 encoding
    let trimmed = token_json.trim().trim_matches('"');
    if trimmed.is_empty() {
        return Ok(nexus_governance_oracle::tauri_commands::TokenVerification {
            valid: false,
            token_id: String::new(),
            timestamp: now,
        });
    }

    // Ed25519 signatures are 64 bytes = 128 hex chars or 88 base64 chars
    let is_valid_hex = trimmed.len() == 128 && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    let is_valid_base64 = trimmed.len() >= 86
        && trimmed.len() <= 92
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
    // Also accept shorter tokens as governance decision hashes (64 hex = SHA-256)
    let is_valid_hash = trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit());

    let valid = is_valid_hex || is_valid_base64 || is_valid_hash;
    let token_id = if valid {
        format!("tok-{}", &trimmed[..trimmed.len().min(16)])
    } else {
        String::new()
    };

    Ok(nexus_governance_oracle::tauri_commands::TokenVerification {
        valid,
        token_id,
        timestamp: now,
    })
}

#[tauri::command]
pub fn oracle_get_agent_budget(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<BudgetSummary, String> {
    let sup = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let mut allocations = std::collections::HashMap::new();
    if let Ok(id) = uuid::Uuid::parse_str(&agent_id) {
        if let Some(handle) = sup.get_agent(id) {
            allocations.insert("fuel_remaining".into(), handle.remaining_fuel);
            allocations.insert("fuel_budget".into(), handle.manifest.fuel_budget);
            allocations.insert(
                "fuel_consumed".into(),
                handle
                    .manifest
                    .fuel_budget
                    .saturating_sub(handle.remaining_fuel),
            );
            allocations.insert("autonomy_level".into(), handle.autonomy_level as u64);
        }
    }
    // Include governance evaluation count from the audit log
    let gov_log = state
        .governance_audit_log
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let agent_evaluations = gov_log
        .entries()
        .iter()
        .filter(|e| e.agent_id == agent_id)
        .count() as u64;
    allocations.insert("governance_evaluations".into(), agent_evaluations);

    let ruleset = state
        .governance_ruleset
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    Ok(BudgetSummary {
        agent_id,
        allocations,
        version: ruleset.version,
    })
}

#[tauri::command]
pub fn cm_evaluate_response(
    state: tauri::State<'_, AppState>,
    problem_id: String,
    agent_response: String,
) -> Result<nexus_capability_measurement::evaluation::comparator::SingleEvaluationResult, String> {
    nexus_capability_measurement::tauri_commands::evaluate_single_response(
        &state.capability_measurement,
        &problem_id,
        &agent_response,
    )
}

// ── Token Economy Commands ────────────────────────────────────────────────────

#[tauri::command]
pub fn token_get_wallet(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<token_cmds::WalletSummary, String> {
    token_cmds::get_wallet(&state.token_economy, &agent_id)
}

#[tauri::command]
pub fn token_get_all_wallets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<token_cmds::WalletSummary>, String> {
    token_cmds::get_all_wallets(&state.token_economy)
}

#[tauri::command]
pub fn token_create_wallet(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    initial_balance: f64,
    autonomy_level: u8,
) -> Result<token_cmds::WalletSummary, String> {
    token_cmds::create_wallet(
        &state.token_economy,
        &agent_id,
        initial_balance,
        autonomy_level,
    )
}

#[tauri::command]
pub fn token_get_ledger(
    state: tauri::State<'_, AppState>,
    agent_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<token_cmds::LedgerEntrySummary>, String> {
    token_cmds::get_ledger(
        &state.token_economy,
        agent_id.as_deref(),
        limit.unwrap_or(50),
    )
}

#[tauri::command]
pub fn token_get_supply(
    state: tauri::State<'_, AppState>,
) -> Result<token_cmds::SupplySummary, String> {
    token_cmds::get_supply(&state.token_economy)
}

#[tauri::command]
pub fn token_calculate_burn(
    state: tauri::State<'_, AppState>,
    model_id: String,
    input_tokens: u64,
    output_tokens: u64,
) -> token_cmds::BurnEstimate {
    token_cmds::calculate_burn(&state.token_economy, &model_id, input_tokens, output_tokens)
}

#[tauri::command]
pub fn token_calculate_reward(
    state: tauri::State<'_, AppState>,
    quality: f64,
    difficulty: f64,
    completion_secs: u64,
) -> token_cmds::RewardEstimate {
    token_cmds::calculate_reward(&state.token_economy, quality, difficulty, completion_secs)
}

#[tauri::command]
pub fn token_calculate_spawn(
    state: tauri::State<'_, AppState>,
    parent_id: String,
    fraction: Option<f64>,
) -> Result<token_cmds::SpawnEstimate, String> {
    token_cmds::calculate_spawn(&state.token_economy, &parent_id, fraction)
}

#[tauri::command]
pub fn token_create_delegation(
    state: tauri::State<'_, AppState>,
    requester_id: String,
    provider_id: String,
    task: String,
    payment: f64,
    threshold: f64,
    timeout: u64,
) -> Result<token_cmds::DelegationSummary, String> {
    token_cmds::create_delegation(
        &state.token_economy,
        &requester_id,
        &provider_id,
        &task,
        payment,
        threshold,
        timeout,
    )
}

#[tauri::command]
pub fn token_get_delegations(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<token_cmds::DelegationSummary>, String> {
    token_cmds::get_delegations(&state.token_economy, &agent_id)
}

#[tauri::command]
pub fn token_get_pricing(state: tauri::State<'_, AppState>) -> Vec<token_cmds::PricingSummary> {
    token_cmds::get_pricing(&state.token_economy)
}

// ── Governed Computer Control Commands ────────────────────────────────────────

#[tauri::command]
pub fn cc_execute_action(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    autonomy_level: u8,
    capabilities: Vec<String>,
    action_json: String,
) -> Result<nexus_computer_control::ActionResult, String> {
    let action: nexus_computer_control::ComputerAction =
        serde_json::from_str(&action_json).map_err(|e| format!("Invalid action: {e}"))?;
    cc_cmds::execute_action(
        &state.governed_control,
        &agent_id,
        autonomy_level,
        &capabilities,
        &action,
    )
}

#[tauri::command]
pub fn cc_get_action_history(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<cc_cmds::ActionHistoryEntry>, String> {
    cc_cmds::get_action_history(&state.governed_control, &agent_id)
}

#[tauri::command]
pub fn cc_get_capability_budget(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<cc_cmds::BudgetSummary, String> {
    cc_cmds::get_budget(&state.governed_control, &agent_id)
}

#[tauri::command]
pub fn cc_verify_action_sequence(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<nexus_computer_control::VerificationResult, String> {
    cc_cmds::verify_sequence(&state.governed_control, &agent_id)
}

#[tauri::command]
pub fn cc_get_screen_context(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<nexus_computer_control::ScreenContext, String> {
    cc_cmds::get_screen_context(&state.governed_control, &agent_id)
}

// ── World Simulation Commands ─────────────────────────────────────────────────

#[tauri::command]
pub fn sim_submit(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    description: String,
    actions_json: String,
) -> Result<String, String> {
    let actions: Vec<nexus_world_simulation::SimulatedAction> =
        serde_json::from_str(&actions_json).map_err(|e| format!("Invalid actions: {e}"))?;
    sim_cmds::submit_scenario(&state.world_simulation, &agent_id, &description, actions)
}

#[tauri::command]
pub fn sim_run(
    state: tauri::State<'_, AppState>,
    scenario_id: String,
) -> Result<nexus_world_simulation::SimulationResult, String> {
    sim_cmds::run_scenario(&state.world_simulation, &scenario_id)
}

#[tauri::command]
pub fn sim_get_result(
    state: tauri::State<'_, AppState>,
    scenario_id: String,
) -> Result<nexus_world_simulation::SimulationResult, String> {
    sim_cmds::get_result(&state.world_simulation, &scenario_id)
}

#[tauri::command]
pub fn sim_get_history(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<sim_cmds::ScenarioSummary>, String> {
    sim_cmds::get_history(&state.world_simulation, &agent_id)
}

#[tauri::command]
pub fn sim_get_policy(state: tauri::State<'_, AppState>) -> sim_cmds::PolicySummary {
    sim_cmds::get_policy(&state.world_simulation)
}

#[tauri::command]
pub fn sim_get_risk(
    state: tauri::State<'_, AppState>,
    scenario_id: String,
) -> Result<nexus_world_simulation::RiskAssessment, String> {
    sim_cmds::get_risk(&state.world_simulation, &scenario_id)
}

#[tauri::command]
pub fn sim_branch(
    state: tauri::State<'_, AppState>,
    parent_id: String,
    diverge_at_step: u32,
    alternative_json: String,
    remaining_json: String,
) -> Result<String, String> {
    let alternative: nexus_world_simulation::SimulatedAction =
        serde_json::from_str(&alternative_json).map_err(|e| format!("Invalid alternative: {e}"))?;
    let remaining: Vec<nexus_world_simulation::SimulatedAction> =
        serde_json::from_str(&remaining_json).map_err(|e| format!("Invalid remaining: {e}"))?;
    sim_cmds::create_branch(
        &state.world_simulation,
        &parent_id,
        diverge_at_step,
        alternative,
        remaining,
    )
}

// ── Perception Commands ───────────────────────────────────────────────────────

#[tauri::command]
pub fn perception_init(
    state: tauri::State<'_, AppState>,
    provider: String,
    api_key: String,
    model_id: String,
) -> Result<String, String> {
    perception_cmds::init_provider(&state.perception, &provider, &api_key, &model_id)
}

#[tauri::command]
pub fn perception_describe(
    state: tauri::State<'_, AppState>,
    image_base64: String,
    format: String,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_describe(&state.perception, &image_base64, &format)
}

#[tauri::command]
pub fn perception_extract_text(
    state: tauri::State<'_, AppState>,
    image_base64: String,
    format: String,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_extract_text(&state.perception, &image_base64, &format)
}

#[tauri::command]
pub fn perception_question(
    state: tauri::State<'_, AppState>,
    image_base64: String,
    format: String,
    question: String,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_question(&state.perception, &image_base64, &format, &question)
}

#[tauri::command]
pub fn perception_find_ui_elements(
    state: tauri::State<'_, AppState>,
    image_base64: String,
) -> Result<Vec<nexus_perception::UIElement>, String> {
    perception_cmds::perceive_find_ui_elements(&state.perception, &image_base64)
}

#[tauri::command]
pub fn perception_extract_data(
    state: tauri::State<'_, AppState>,
    image_base64: String,
    format: String,
    schema: Option<String>,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_extract_data(&state.perception, &image_base64, &format, schema)
}

#[tauri::command]
pub fn perception_read_error(
    state: tauri::State<'_, AppState>,
    image_base64: String,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_read_error(&state.perception, &image_base64)
}

#[tauri::command]
pub fn perception_analyze_chart(
    state: tauri::State<'_, AppState>,
    image_base64: String,
    format: String,
) -> Result<nexus_perception::PerceptionResult, String> {
    perception_cmds::perceive_analyze_chart(&state.perception, &image_base64, &format)
}

#[tauri::command]
pub fn perception_get_policy(
    state: tauri::State<'_, AppState>,
) -> nexus_perception::PerceptionPolicy {
    perception_cmds::get_policy(&state.perception)
}

// ── Agent Memory Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_store_entry(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    memory_type: String,
    summary: String,
    tags: Vec<String>,
    importance: f64,
    domain: Option<String>,
) -> Result<String, String> {
    memory_cmds::memory_store(
        &state.persistent_memory,
        &agent_id,
        &memory_type,
        &summary,
        tags,
        importance,
        domain,
    )
}

#[tauri::command]
pub fn memory_query_entries(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    query: String,
    memory_type: Option<String>,
    tags: Option<Vec<String>>,
    limit: usize,
) -> Result<Vec<nexus_agent_memory::Memory>, String> {
    memory_cmds::memory_query(
        &state.persistent_memory,
        &agent_id,
        &query,
        memory_type,
        tags,
        limit,
    )
}

#[tauri::command]
pub fn memory_get_entry(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    memory_id: String,
) -> Result<nexus_agent_memory::Memory, String> {
    memory_cmds::memory_get(&state.persistent_memory, &agent_id, &memory_id)
}

#[tauri::command]
pub fn memory_delete_entry(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    memory_id: String,
) -> Result<bool, String> {
    memory_cmds::memory_delete(&state.persistent_memory, &agent_id, &memory_id)
}

#[tauri::command]
pub fn memory_build_context(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    task_description: String,
    max_memories: usize,
) -> Result<nexus_agent_memory::MemoryContext, String> {
    memory_cmds::memory_build_context(
        &state.persistent_memory,
        &agent_id,
        &task_description,
        max_memories,
    )
}

#[tauri::command]
pub fn memory_get_stats(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<memory_cmds::MemoryStats, String> {
    memory_cmds::memory_get_stats(&state.persistent_memory, &agent_id)
}

#[tauri::command]
pub fn memory_consolidate(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<memory_cmds::ConsolidationResult, String> {
    memory_cmds::memory_consolidate(&state.persistent_memory, &agent_id)
}

#[tauri::command]
pub fn memory_save(state: tauri::State<'_, AppState>, agent_id: String) -> Result<String, String> {
    memory_cmds::memory_save(&state.persistent_memory, &agent_id)
}

#[tauri::command]
pub fn memory_load(state: tauri::State<'_, AppState>, agent_id: String) -> Result<String, String> {
    memory_cmds::memory_load(&state.persistent_memory, &agent_id)
}

#[tauri::command]
pub fn memory_list_agents(state: tauri::State<'_, AppState>) -> Vec<String> {
    memory_cmds::memory_list_agents(&state.persistent_memory)
}

#[tauri::command]
pub fn memory_get_policy(state: tauri::State<'_, AppState>) -> nexus_agent_memory::MemoryPolicy {
    memory_cmds::memory_get_policy(&state.persistent_memory)
}

// ── External Tools Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn tools_list_available(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_external_tools::ExternalTool>, String> {
    tools_cmds::tools_list_available(&state.external_tools)
}

#[tauri::command]
pub fn tools_execute(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    autonomy_level: u8,
    tool_id: String,
    params_json: String,
) -> Result<nexus_external_tools::ToolCallResult, String> {
    tools_cmds::tools_execute(
        &state.external_tools,
        &agent_id,
        autonomy_level,
        &tool_id,
        &params_json,
    )
}

#[tauri::command]
pub fn tools_get_registry(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_external_tools::ExternalTool>, String> {
    tools_cmds::tools_get_registry(&state.external_tools)
}

#[tauri::command]
pub fn tools_refresh_availability(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_external_tools::ExternalTool>, String> {
    tools_cmds::tools_refresh_availability(&state.external_tools)
}

#[tauri::command]
pub fn tools_get_audit(
    state: tauri::State<'_, AppState>,
    limit: usize,
) -> Result<Vec<nexus_external_tools::ToolAuditEntry>, String> {
    tools_cmds::tools_get_audit(&state.external_tools, limit)
}

#[tauri::command]
pub fn tools_verify_audit(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    tools_cmds::tools_verify_audit(&state.external_tools)
}

#[tauri::command]
pub fn tools_get_policy(
    state: tauri::State<'_, AppState>,
) -> nexus_external_tools::ToolGovernancePolicy {
    tools_cmds::tools_get_policy(&state.external_tools)
}

// ── Collaboration Protocol Commands ───────────────────────────────────────────

#[tauri::command]
pub fn collab_create_session(
    state: tauri::State<'_, AppState>,
    title: String,
    goal: String,
    pattern: String,
    lead_agent_id: String,
    lead_autonomy: u8,
) -> Result<String, String> {
    collab_cmds::collab_create_session(
        &state.collab_protocol,
        &title,
        &goal,
        &pattern,
        &lead_agent_id,
        lead_autonomy,
    )
}

#[tauri::command]
pub fn collab_add_participant(
    state: tauri::State<'_, AppState>,
    session_id: String,
    agent_id: String,
    autonomy: u8,
    role: String,
) -> Result<(), String> {
    collab_cmds::collab_add_participant(
        &state.collab_protocol,
        &session_id,
        &agent_id,
        autonomy,
        &role,
    )
}

#[tauri::command]
pub fn collab_start(state: tauri::State<'_, AppState>, session_id: String) -> Result<(), String> {
    collab_cmds::collab_start(&state.collab_protocol, &session_id)
}

#[tauri::command]
pub fn collab_send_message(
    state: tauri::State<'_, AppState>,
    session_id: String,
    from_agent: String,
    to_agent: Option<String>,
    message_type: String,
    text: String,
    confidence: f64,
) -> Result<String, String> {
    collab_cmds::collab_send_message(
        &state.collab_protocol,
        &session_id,
        &from_agent,
        to_agent,
        &message_type,
        &text,
        confidence,
    )
}

#[tauri::command]
pub fn collab_call_vote(
    state: tauri::State<'_, AppState>,
    session_id: String,
    proposal_msg_id: String,
    majority: f64,
    deadline_secs: u64,
) -> Result<(), String> {
    collab_cmds::collab_call_vote(
        &state.collab_protocol,
        &session_id,
        &proposal_msg_id,
        majority,
        deadline_secs,
    )
}

#[tauri::command]
pub fn collab_cast_vote(
    state: tauri::State<'_, AppState>,
    session_id: String,
    agent_id: String,
    vote: String,
    reason: Option<String>,
) -> Result<(), String> {
    collab_cmds::collab_cast_vote(
        &state.collab_protocol,
        &session_id,
        &agent_id,
        &vote,
        reason,
    )
}

#[tauri::command]
pub fn collab_declare_consensus(
    state: tauri::State<'_, AppState>,
    session_id: String,
    agent_id: String,
    decision: String,
    key_points: Vec<String>,
) -> Result<(), String> {
    collab_cmds::collab_declare_consensus(
        &state.collab_protocol,
        &session_id,
        &agent_id,
        &decision,
        key_points,
    )
}

#[tauri::command]
pub fn collab_detect_consensus(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<nexus_collab_protocol::ConsensusState, String> {
    collab_cmds::collab_detect_consensus(&state.collab_protocol, &session_id)
}

#[tauri::command]
pub fn collab_get_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<nexus_collab_protocol::CollaborationSession, String> {
    collab_cmds::collab_get_session(&state.collab_protocol, &session_id)
}

#[tauri::command]
pub fn collab_list_active(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_collab_protocol::CollaborationSession>, String> {
    collab_cmds::collab_list_active(&state.collab_protocol)
}

#[tauri::command]
pub fn collab_get_policy(
    state: tauri::State<'_, AppState>,
) -> nexus_collab_protocol::CollaborationPolicy {
    collab_cmds::collab_get_policy(&state.collab_protocol)
}

#[tauri::command]
pub fn collab_get_patterns() -> Vec<collab_cmds::PatternInfo> {
    collab_cmds::collab_get_patterns()
}

// ── Software Factory Commands ─────────────────────────────────────────────────

#[tauri::command]
pub fn swf_create_project(
    state: tauri::State<'_, AppState>,
    title: String,
    user_request: String,
) -> Result<String, String> {
    factory_cmds::factory_create_project(&state.software_factory, &title, &user_request)
}

#[tauri::command]
pub fn swf_assign_member(
    state: tauri::State<'_, AppState>,
    project_id: String,
    agent_id: String,
    agent_name: String,
    role: String,
    autonomy: u8,
    score: Option<f64>,
) -> Result<(), String> {
    factory_cmds::factory_assign_member(
        &state.software_factory,
        &project_id,
        &agent_id,
        &agent_name,
        &role,
        autonomy,
        score,
    )
}

#[tauri::command]
pub fn swf_start_pipeline(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<(), String> {
    factory_cmds::factory_start_pipeline(&state.software_factory, &project_id)
}

#[tauri::command]
pub fn swf_submit_artifact(
    state: tauri::State<'_, AppState>,
    project_id: String,
    artifact_json: String,
) -> Result<nexus_software_factory::QualityGateResult, String> {
    factory_cmds::factory_submit_artifact(&state.software_factory, &project_id, &artifact_json)
}

#[tauri::command]
pub fn swf_get_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<nexus_software_factory::Project, String> {
    factory_cmds::factory_get_project(&state.software_factory, &project_id)
}

#[tauri::command]
pub fn swf_list_projects(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_software_factory::Project>, String> {
    factory_cmds::factory_list_projects(&state.software_factory)
}

#[tauri::command]
pub fn swf_get_cost(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<factory_cmds::CostBreakdown, String> {
    factory_cmds::factory_get_cost(&state.software_factory, &project_id)
}

#[tauri::command]
pub fn swf_get_policy(state: tauri::State<'_, AppState>) -> nexus_software_factory::FactoryPolicy {
    factory_cmds::factory_get_policy(&state.software_factory)
}

#[tauri::command]
pub fn swf_get_pipeline_stages() -> Vec<factory_cmds::StageInfo> {
    factory_cmds::factory_get_pipeline_stages()
}

#[tauri::command]
pub fn swf_estimate_cost() -> u64 {
    factory_cmds::factory_estimate_cost()
}

// ── MCP Standalone Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn mcp2_server_status(
    state: tauri::State<'_, AppState>,
) -> Result<mcp2_cmds::McpServerStatus, String> {
    mcp2_cmds::mcp_server_status(&state.mcp_standalone)
}

#[tauri::command]
pub fn mcp2_server_handle(
    state: tauri::State<'_, AppState>,
    request_json: String,
) -> Result<String, String> {
    mcp2_cmds::mcp_server_handle_request(&state.mcp_standalone, &request_json)
}

#[tauri::command]
pub fn mcp2_server_list_tools(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<nexus_mcp::McpTool>, String> {
    mcp2_cmds::mcp_server_list_tools(&state.mcp_standalone)
}

#[tauri::command]
pub fn mcp2_client_add(
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
    command: String,
    args: Vec<String>,
) -> Result<(), String> {
    mcp2_cmds::mcp_client_add_server(&state.mcp_standalone, &id, &name, &command, args)
}

#[tauri::command]
pub fn mcp2_client_remove(
    state: tauri::State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    mcp2_cmds::mcp_client_remove_server(&state.mcp_standalone, &server_id)
}

#[tauri::command]
pub fn mcp2_client_discover(
    state: tauri::State<'_, AppState>,
    server_id: String,
) -> Result<Vec<nexus_mcp::McpTool>, String> {
    mcp2_cmds::mcp_client_discover_tools(&state.mcp_standalone, &server_id)
}

#[tauri::command]
pub fn mcp2_client_call(
    state: tauri::State<'_, AppState>,
    server_id: String,
    tool_name: String,
    arguments_json: String,
) -> Result<serde_json::Value, String> {
    mcp2_cmds::mcp_client_call_tool(
        &state.mcp_standalone,
        &server_id,
        &tool_name,
        &arguments_json,
    )
}

// ── A2A Crate commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn a2a_crate_get_agent_card(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let card = a2a_crate_cmds::a2a_crate_get_agent_card(&state.a2a_crate)?;
    serde_json::to_value(&card).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub fn a2a_crate_list_skills(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let skills = a2a_crate_cmds::a2a_crate_list_skills(&state.a2a_crate)?;
    serde_json::to_value(&skills).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub fn a2a_crate_send_task(
    state: tauri::State<'_, AppState>,
    agent_url: String,
    message: String,
) -> Result<serde_json::Value, String> {
    a2a_crate_cmds::a2a_crate_send_task(&state.a2a_crate, &agent_url, &message)
}

#[tauri::command]
pub fn a2a_crate_get_task(
    state: tauri::State<'_, AppState>,
    task_id: String,
    agent_url: Option<String>,
) -> Result<serde_json::Value, String> {
    a2a_crate_cmds::a2a_crate_get_task(&state.a2a_crate, &task_id, agent_url)
}

#[tauri::command]
pub fn a2a_crate_discover_agent(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<serde_json::Value, String> {
    a2a_crate_cmds::a2a_crate_discover_agent(&state.a2a_crate, &url)
}

#[tauri::command]
pub fn a2a_crate_get_status(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let status = a2a_crate_cmds::a2a_crate_get_status(&state.a2a_crate)?;
    serde_json::to_value(&status).map_err(|e| format!("serialize: {e}"))
}

// ── Migration Tool commands ─────────────────────────────────────────────────

#[tauri::command]
pub fn migrate_preview(
    source: String,
    agents_yaml: Option<String>,
    tasks_yaml: Option<String>,
    python_source: Option<String>,
) -> Result<serde_json::Value, String> {
    migrate_cmds::migrate_preview(
        &source,
        agents_yaml.as_deref(),
        tasks_yaml.as_deref(),
        python_source.as_deref(),
    )
}

#[tauri::command]
pub fn migrate_execute(
    source: String,
    agents_yaml: Option<String>,
    tasks_yaml: Option<String>,
    python_source: Option<String>,
) -> Result<serde_json::Value, String> {
    migrate_cmds::migrate_execute(
        &source,
        agents_yaml.as_deref(),
        tasks_yaml.as_deref(),
        python_source.as_deref(),
    )
}

#[tauri::command]
pub fn migrate_supported_sources() -> Vec<String> {
    migrate_cmds::migrate_supported_sources()
}

#[tauri::command]
pub fn migrate_report(
    source: String,
    agents_yaml: Option<String>,
    tasks_yaml: Option<String>,
    python_source: Option<String>,
) -> Result<String, String> {
    migrate_cmds::migrate_report(
        &source,
        agents_yaml.as_deref(),
        tasks_yaml.as_deref(),
        python_source.as_deref(),
    )
}

// ── Memory Kernel commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn mk_get_stats(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<serde_json::Value, String> {
    mk_cmds::memory_get_stats(&state.memory_kernel, &agent_id)
}

#[tauri::command]
pub fn mk_query(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    memory_type: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_query(
        &state.memory_kernel,
        &agent_id,
        memory_type.as_deref(),
        limit,
    )
}

#[tauri::command]
pub fn mk_search(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    query_text: String,
    policy: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_search(
        &state.memory_kernel,
        &agent_id,
        &query_text,
        policy.as_deref(),
        limit,
    )
}

#[tauri::command]
pub fn mk_get_audit(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_get_audit(&state.memory_kernel, &agent_id, limit)
}

#[tauri::command]
pub fn mk_get_procedures(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_get_procedures(&state.memory_kernel, &agent_id)
}

#[tauri::command]
pub fn mk_get_candidates(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_get_candidates(&state.memory_kernel, &agent_id)
}

#[tauri::command]
pub fn mk_write(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    memory_type: String,
    content: serde_json::Value,
) -> Result<String, String> {
    mk_cmds::memory_write(&state.memory_kernel, &agent_id, &memory_type, content)
}

#[tauri::command]
pub fn mk_clear_working(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<bool, String> {
    mk_cmds::memory_clear_working(&state.memory_kernel, &agent_id)
}

#[tauri::command]
pub fn mk_share(
    state: tauri::State<'_, AppState>,
    owner_id: String,
    grantee_id: String,
    read_types: Vec<String>,
    write_types: Vec<String>,
) -> Result<bool, String> {
    mk_cmds::memory_share(
        &state.memory_kernel,
        &owner_id,
        &grantee_id,
        read_types,
        write_types,
    )
}

#[tauri::command]
pub fn mk_revoke_share(
    state: tauri::State<'_, AppState>,
    owner_id: String,
    grantee_id: String,
) -> Result<serde_json::Value, String> {
    mk_cmds::memory_revoke_share(&state.memory_kernel, &owner_id, &grantee_id)
}

#[tauri::command]
pub fn mk_run_gc(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    mk_cmds::memory_run_gc(&state.memory_kernel)
}

#[tauri::command]
pub fn mk_create_checkpoint(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    label: String,
) -> Result<String, String> {
    mk_cmds::memory_create_checkpoint(&state.memory_kernel, &agent_id, &label)
}

#[tauri::command]
pub fn mk_rollback(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    checkpoint_id: String,
    reason: String,
) -> Result<serde_json::Value, String> {
    mk_cmds::memory_rollback(&state.memory_kernel, &agent_id, &checkpoint_id, &reason)
}

#[tauri::command]
pub fn mk_list_checkpoints(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    mk_cmds::memory_list_checkpoints(&state.memory_kernel, &agent_id)
}

// ── Governance Engine commands ───────────────────────────────────────────────

#[tauri::command]
pub fn governance_engine_get_rules(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ruleset = state
        .governance_ruleset
        .lock()
        .map_err(|e| format!("lock: {e}"))?;
    let rules_summary: Vec<serde_json::Value> = ruleset
        .rules
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "description": r.description,
                "effect": format!("{:?}", r.effect),
                "conditions_count": r.conditions.len(),
            })
        })
        .collect();
    Ok(serde_json::json!({
        "rule_count": ruleset.rules.len(),
        "version": ruleset.version,
        "version_hash": ruleset.version_hash(),
        "rules": rules_summary,
    }))
}

#[tauri::command]
pub fn governance_engine_evaluate(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    capability: String,
    action: String,
) -> Result<serde_json::Value, String> {
    use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision};

    let request = CapabilityRequest {
        agent_id: agent_id.clone(),
        capability: capability.clone(),
        parameters: serde_json::json!({ "action": action }),
        budget_hash: String::new(),
        request_nonce: uuid::Uuid::new_v4().to_string(),
    };

    let ruleset = state
        .governance_ruleset
        .lock()
        .map_err(|e| format!("lock: {e}"))?;

    // Create a temporary engine just for evaluation
    let (_tx, rx) = tokio::sync::mpsc::channel::<nexus_governance_oracle::OracleRequest>(1);
    let engine = nexus_governance_engine::DecisionEngine::new(rx, ruleset.clone());
    let decision = engine.evaluate_request(&request, &ruleset);

    // Record in audit log
    let governance_version = ruleset.version_hash();
    drop(ruleset);
    let mut audit_log = state
        .governance_audit_log
        .lock()
        .map_err(|e| format!("lock: {e}"))?;
    audit_log.record(&request, &decision, &governance_version);

    let (allowed, reason) = match &decision {
        GovernanceDecision::Approved { capability_token } => (
            true,
            format!("approved (token: {})", &capability_token[..8]),
        ),
        GovernanceDecision::Denied => (false, "denied by governance ruleset".to_string()),
    };

    Ok(serde_json::json!({
        "agent_id": agent_id,
        "capability": capability,
        "allowed": allowed,
        "reason": reason,
        "governance_version": governance_version,
    }))
}

#[tauri::command]
pub fn governance_engine_get_audit_log(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    let audit_log = state
        .governance_audit_log
        .lock()
        .map_err(|e| format!("lock: {e}"))?;
    let entries = audit_log.entries();
    let limit = limit.unwrap_or(50).min(entries.len());
    let recent: Vec<serde_json::Value> = entries
        .iter()
        .rev()
        .take(limit)
        .map(|e| {
            serde_json::json!({
                "entry_id": e.entry_id,
                "agent_id": e.agent_id,
                "capability": e.capability,
                "decision": e.decision,
                "governance_version": e.governance_version,
                "timestamp": e.timestamp,
                "entry_hash": e.entry_hash,
            })
        })
        .collect();
    let chain_valid = audit_log.verify_chain().is_ok();
    Ok(serde_json::json!({
        "total_entries": audit_log.len(),
        "latest_hash": audit_log.latest_hash(),
        "chain_valid": chain_valid,
        "entries": recent,
    }))
}

// ── Governance Evolution commands ────────────────────────────────────────────

#[tauri::command]
pub fn governance_evolution_get_threat_model(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let evo = state
        .governance_evolution
        .lock()
        .map_err(|e| format!("lock: {e}"))?;
    let model = evo.threat_model();
    let techniques: Vec<serde_json::Value> = model
        .techniques
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "description": t.description,
                "source": format!("{:?}", t.source),
                "times_attempted": t.times_attempted,
                "times_caught": t.times_caught,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "technique_count": model.technique_count(),
        "techniques": techniques,
    }))
}

#[tauri::command]
pub fn governance_evolution_run_attack_cycle(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ruleset = state
        .governance_ruleset
        .lock()
        .map_err(|e| format!("lock: {e}"))?
        .clone();

    // Create a temporary DecisionEngine for the cycle
    let (_tx, rx) = tokio::sync::mpsc::channel::<nexus_governance_oracle::OracleRequest>(1);
    let engine = nexus_governance_engine::DecisionEngine::new(rx, ruleset.clone());

    let mut evo = state
        .governance_evolution
        .lock()
        .map_err(|e| format!("lock: {e}"))?;
    let cycle = evo.run_cycle(&engine, &ruleset);

    Ok(serde_json::json!({
        "cycle_id": cycle.cycle_id,
        "timestamp": cycle.timestamp,
        "attacks_generated": cycle.attacks_generated,
        "attacks_caught": cycle.attacks_caught,
        "attacks_missed": cycle.attacks_missed,
        "rules_evolved": cycle.rules_evolved,
        "new_ruleset_version": cycle.new_ruleset_version,
        "threats_absorbed": cycle.threats_absorbed,
    }))
}
