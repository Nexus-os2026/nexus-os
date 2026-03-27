//! Frontend integration types and handler logic for Tauri commands.
//!
//! This module provides the request/response types and state management
//! without depending on Tauri. The actual `#[tauri::command]` wrappers live
//! in `app/src-tauri/src/main.rs` and delegate to functions here.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::battery::test_problem::{load_battery, TestProblem};
use crate::framework::{CapabilityProfile, MeasurementSession, Vector};
use crate::reporting::scorecard::AgentScorecard;
use crate::scoring::gaming_detection::GamingFlag;

// ── Types ────────────────────────────────────────────────────────────────────

/// Summary of a test battery for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatterySummary {
    pub vector: Vector,
    pub problem_count: usize,
    pub locked_count: usize,
    pub version: String,
}

/// Request to start a measurement session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSessionRequest {
    pub agent_id: String,
    pub agent_autonomy_level: u8,
    /// Which vectors to evaluate (empty = all four).
    pub vectors: Vec<Vector>,
}

/// Agent comparison request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareAgentsRequest {
    pub agent_ids: Vec<String>,
}

// ── State ────────────────────────────────────────────────────────────────────

/// In-memory measurement state held by the Tauri app.
pub struct MeasurementState {
    pub sessions: RwLock<Vec<MeasurementSession>>,
    pub scorecards: RwLock<Vec<AgentScorecard>>,
    pub batteries: Vec<TestProblem>,
}

impl MeasurementState {
    /// Create a new state, loading batteries from the default path.
    /// Falls back to empty batteries if the file is not found.
    pub fn new() -> Self {
        let batteries = load_battery("crates/nexus-capability-measurement/data/battery_v1.json")
            .unwrap_or_default();
        Self {
            sessions: RwLock::new(Vec::new()),
            scorecards: RwLock::new(Vec::new()),
            batteries,
        }
    }

    /// Create a state with pre-loaded batteries (for testing or embedding).
    pub fn with_batteries(batteries: Vec<TestProblem>) -> Self {
        Self {
            sessions: RwLock::new(Vec::new()),
            scorecards: RwLock::new(Vec::new()),
            batteries,
        }
    }
}

impl Default for MeasurementState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Handlers (called by Tauri command wrappers) ──────────────────────────────

/// Create a new pending measurement session. Returns the session ID.
pub fn start_measurement_session(
    state: &MeasurementState,
    agent_id: &str,
    agent_autonomy_level: u8,
) -> Result<String, String> {
    let session = crate::framework::new_session(agent_id, agent_autonomy_level);
    let session_id = session.id.to_string();

    let scorecard = AgentScorecard::from_session(&session);

    state
        .sessions
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .push(session);
    state
        .scorecards
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .push(scorecard);

    Ok(session_id)
}

/// Look up a measurement session by ID.
pub fn get_measurement_session(
    state: &MeasurementState,
    session_id: &str,
) -> Result<MeasurementSession, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    sessions
        .iter()
        .find(|s| s.id.to_string() == session_id)
        .cloned()
        .ok_or_else(|| format!("Session not found: {session_id}"))
}

/// Get the most recent scorecard for an agent.
pub fn get_agent_scorecard(
    state: &MeasurementState,
    agent_id: &str,
) -> Result<AgentScorecard, String> {
    let scorecards = state.scorecards.read().map_err(|e| format!("lock: {e}"))?;
    scorecards
        .iter()
        .rev()
        .find(|s| s.agent_id == agent_id)
        .cloned()
        .ok_or_else(|| format!("No scorecard found for agent: {agent_id}"))
}

/// List all measurement sessions, most recent first.
pub fn list_measurement_sessions(
    state: &MeasurementState,
) -> Result<Vec<MeasurementSession>, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    let mut result = sessions.clone();
    result.reverse();
    Ok(result)
}

/// Extract the capability profile from the most recent session for an agent.
pub fn get_capability_profile(
    state: &MeasurementState,
    agent_id: &str,
) -> Result<CapabilityProfile, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    let session = sessions
        .iter()
        .rev()
        .find(|s| s.agent_id == agent_id)
        .ok_or_else(|| format!("No session found for agent: {agent_id}"))?;

    session
        .cross_vector_analysis
        .as_ref()
        .map(|a| a.capability_profile.clone())
        .ok_or_else(|| "Session has no cross-vector analysis yet".to_string())
}

/// Get all gaming flags from a specific session.
pub fn get_gaming_flags(
    state: &MeasurementState,
    session_id: &str,
) -> Result<Vec<GamingFlag>, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    let session = sessions
        .iter()
        .find(|s| s.id.to_string() == session_id)
        .ok_or_else(|| format!("Session not found: {session_id}"))?;

    let flags: Vec<GamingFlag> = session
        .vector_results
        .iter()
        .flat_map(|vr| vr.gaming_flags.clone())
        .collect();
    Ok(flags)
}

/// Compare agents by returning their most recent scorecards.
pub fn compare_agents(
    state: &MeasurementState,
    agent_ids: &[String],
) -> Result<Vec<AgentScorecard>, String> {
    let scorecards = state.scorecards.read().map_err(|e| format!("lock: {e}"))?;

    let mut results = Vec::new();
    for id in agent_ids {
        let card = scorecards
            .iter()
            .rev()
            .find(|s| s.agent_id == *id)
            .cloned()
            .ok_or_else(|| format!("No scorecard found for agent: {id}"))?;
        results.push(card);
    }
    Ok(results)
}

/// Summarize the loaded test batteries.
pub fn get_locked_batteries(state: &MeasurementState) -> Result<Vec<BatterySummary>, String> {
    let mut by_vector: HashMap<Vector, (usize, usize, String)> = HashMap::new();

    for problem in &state.batteries {
        let entry = by_vector
            .entry(problem.vector)
            .or_insert((0, 0, problem.version.clone()));
        entry.0 += 1;
        if problem.locked {
            entry.1 += 1;
        }
    }

    let summaries = by_vector
        .into_iter()
        .map(|(vector, (count, locked, version))| BatterySummary {
            vector,
            problem_count: count,
            locked_count: locked,
            version,
        })
        .collect();

    Ok(summaries)
}

/// Trigger the Darwin Core feedback loop for an agent.
pub fn trigger_evolution_feedback(
    state: &MeasurementState,
    agent_id: &str,
) -> Result<crate::darwin_bridge::FeedbackResult, String> {
    let scorecard = get_agent_scorecard(state, agent_id)?;
    Ok(crate::darwin_bridge::run_measurement_feedback(&scorecard))
}

/// Evaluate a single agent response against a specific locked problem.
/// Uses keyword-based scoring (no LLM judge required).
pub fn evaluate_single_response(
    state: &MeasurementState,
    problem_id: &str,
    agent_response: &str,
) -> Result<crate::evaluation::comparator::SingleEvaluationResult, String> {
    let problem = state
        .batteries
        .iter()
        .find(|p| p.id == problem_id && p.locked)
        .ok_or_else(|| format!("Locked problem not found: {problem_id}"))?;

    let (coverage, gaps, redundancies, hallucinations) =
        crate::evaluation::comparator::compare_response(
            agent_response,
            &problem.expected_reasoning,
        );

    let primary_score = crate::scoring::asymmetric::compute_primary_score(
        problem.vector,
        coverage,
        gaps,
        redundancies,
        hallucinations,
    );

    let articulation_score = crate::scoring::articulation::empty_articulation(problem.vector);

    let mut gaming_flags = Vec::new();
    if let Some(flag) =
        crate::scoring::gaming_detection::detect_confident_at_level5(problem.level, agent_response)
    {
        gaming_flags.push(flag);
    }
    if let Some(flag) = crate::scoring::gaming_detection::detect_high_primary_zero_articulation(
        primary_score.adjusted_score,
        articulation_score.total,
    ) {
        gaming_flags.push(flag);
    }

    Ok(crate::evaluation::comparator::SingleEvaluationResult {
        problem_id: problem.id.clone(),
        vector: problem.vector,
        level: problem.level,
        primary_score,
        articulation_score,
        gaming_flags,
    })
}

/// Run batch evaluation with real LLM inference. Requires GROQ_API_KEY or
/// NVIDIA_NIM_API_KEY. Returns error if no API key is configured.
pub fn run_batch_evaluation(
    state: &MeasurementState,
    agent_entries: &[(String, u8)],
) -> Result<crate::evaluation::batch::BatchResult, String> {
    let api_key = std::env::var("GROQ_API_KEY")
        .or_else(|_| std::env::var("NVIDIA_NIM_API_KEY"))
        .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
        .map_err(|_| {
            "Real inference requires GROQ_API_KEY, NVIDIA_NIM_API_KEY, or OPENROUTER_API_KEY. \
             Configure one to run real validation."
                .to_string()
        })?;
    let client = std::sync::Arc::new(crate::evaluation::nim_client::NimClient::new(
        api_key,
        "llama-3.1-8b-instant".into(),
    ));
    let adapters: Vec<crate::evaluation::agent_adapter::AgentAdapter> = agent_entries
        .iter()
        .map(|(id, level)| {
            let c = client.clone();
            crate::evaluation::agent_adapter::AgentAdapter::new(id.clone(), *level, move |prompt| {
                c.query("You are a helpful assistant.", prompt, 512)
            })
        })
        .collect();

    let evaluator = crate::evaluation::batch::BatchEvaluator::new(state.batteries.clone(), None);
    let result = evaluator.evaluate_all(&adapters, None);

    // Store sessions and scorecards
    let mut sessions = state.sessions.write().map_err(|e| format!("lock: {e}"))?;
    let mut scorecards = state.scorecards.write().map_err(|e| format!("lock: {e}"))?;
    for session in &result.sessions {
        let card = crate::reporting::scorecard::AgentScorecard::from_session(session);
        sessions.push(session.clone());
        scorecards.push(card);
    }

    Ok(result)
}

/// Get the boundary map from the most recent batch result.
pub fn get_boundary_map(
    state: &MeasurementState,
) -> Result<Vec<crate::evaluation::batch::AgentBoundary>, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    Ok(crate::evaluation::batch::build_boundary_map(&sessions))
}

/// Get the calibration report from all sessions.
pub fn get_calibration_report(
    state: &MeasurementState,
) -> Result<crate::evaluation::batch::CalibrationReport, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    Ok(crate::evaluation::batch::check_calibration(&sessions))
}

/// Get the classification census from all sessions.
pub fn get_classification_census(
    state: &MeasurementState,
) -> Result<crate::evaluation::batch::ClassificationCensus, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    Ok(crate::evaluation::batch::build_classification_census(
        &sessions,
    ))
}

/// Get the gaming report from all sessions.
pub fn get_gaming_report_batch(
    state: &MeasurementState,
) -> Result<crate::evaluation::batch::GamingReport, String> {
    let sessions = state.sessions.read().map_err(|e| format!("lock: {e}"))?;
    Ok(crate::evaluation::batch::build_gaming_report(&sessions))
}

/// Convert all scorecards to Darwin Core fitness signals.
pub fn upload_to_darwin(
    state: &MeasurementState,
) -> Result<crate::evaluation::batch::DarwinUploadSummary, String> {
    let scorecards = state.scorecards.read().map_err(|e| format!("lock: {e}"))?;

    let mut fitness_signals = 0;
    let mut reevaluation_triggers = 0;
    let mut mutation_targets = 0;

    for card in scorecards.iter() {
        let _signal = crate::darwin_bridge::EvolutionFitnessProvider::to_fitness_signal(card);
        let triggers =
            crate::darwin_bridge::EvolutionFitnessProvider::to_reevaluation_triggers(card);
        let guidance = crate::darwin_bridge::EvolutionFitnessProvider::to_mutation_guidance(card);

        fitness_signals += 1;
        reevaluation_triggers += triggers.len();
        mutation_targets += guidance.mutation_targets.len();
    }

    Ok(crate::evaluation::batch::DarwinUploadSummary {
        agents_uploaded: scorecards.len(),
        fitness_signals,
        reevaluation_triggers,
        mutation_targets,
    })
}

/// Run A/B validation: baseline (fixed model) vs routed (predictive model selection).
/// Requires GROQ_API_KEY or NVIDIA_NIM_API_KEY for real LLM inference.
pub fn run_ab_validation(
    state: &MeasurementState,
    agent_entries: &[(String, u8)],
) -> Result<crate::evaluation::ab_validation::ABComparisonResult, String> {
    let api_key = std::env::var("GROQ_API_KEY")
        .or_else(|_| std::env::var("NVIDIA_NIM_API_KEY"))
        .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
        .map_err(|_| {
            "Real A/B validation requires GROQ_API_KEY, NVIDIA_NIM_API_KEY, or OPENROUTER_API_KEY. \
             Configure one to compare real LLM performance."
                .to_string()
        })?;

    // Baseline: small model (simulates unrouted fixed assignment)
    let baseline_client = std::sync::Arc::new(crate::evaluation::nim_client::NimClient::new(
        api_key.clone(),
        "llama-3.1-8b-instant".into(),
    ));
    let baseline_adapters: Vec<crate::evaluation::agent_adapter::AgentAdapter> = agent_entries
        .iter()
        .map(|(id, level)| {
            let c = baseline_client.clone();
            crate::evaluation::agent_adapter::AgentAdapter::new(id.clone(), *level, move |prompt| {
                c.query("You are a helpful assistant.", prompt, 512)
            })
        })
        .collect();

    // Routed: larger model (simulates predictive routing selecting optimal model)
    let routed_client = std::sync::Arc::new(crate::evaluation::nim_client::NimClient::new(
        api_key,
        "llama-3.3-70b-versatile".into(),
    ));
    let routed_adapters: Vec<crate::evaluation::agent_adapter::AgentAdapter> = agent_entries
        .iter()
        .map(|(id, level)| {
            let c = routed_client.clone();
            crate::evaluation::agent_adapter::AgentAdapter::new(id.clone(), *level, move |prompt| {
                c.query(
                    "You are an expert assistant. Provide thorough, detailed analysis.",
                    prompt,
                    1024,
                )
            })
        })
        .collect();

    let result = crate::evaluation::ab_validation::run_ab_validation(
        &state.batteries,
        &baseline_adapters,
        &routed_adapters,
    )?;

    // Store sessions from both runs
    let mut sessions = state.sessions.write().map_err(|e| format!("lock: {e}"))?;
    let mut scorecards = state.scorecards.write().map_err(|e| format!("lock: {e}"))?;
    for session in result
        .baseline
        .sessions
        .iter()
        .chain(result.routed.sessions.iter())
    {
        let card = crate::reporting::scorecard::AgentScorecard::from_session(session);
        sessions.push(session.clone());
        scorecards.push(card);
    }

    Ok(result)
}

/// Get the most recent A/B comparison result (if stored).
pub fn get_ab_comparison(
    _state: &MeasurementState,
) -> Result<Option<crate::evaluation::ab_validation::ABComparisonResult>, String> {
    // A/B results are returned directly from run_ab_validation.
    // For cached retrieval, the caller should store the result.
    Ok(None)
}

/// Execute a full validation run against all prebuilt agents.
pub fn execute_validation_run(
    state: &MeasurementState,
    run_label: &str,
    enable_routing: bool,
) -> Result<crate::evaluation::validation_run::ValidationRunOutput, String> {
    let config = crate::evaluation::validation_run::ValidationRunConfig {
        run_label: run_label.into(),
        agent_ids: Vec::new(),
        enable_routing,
        staging_threshold: 0.95,
        agent_timeout_secs: 120,
    };

    let agents_dir = std::path::Path::new("agents/prebuilt");
    let output = crate::evaluation::validation_run::execute_validation_run(
        &state.batteries,
        &config,
        agents_dir,
    )?;

    // Store sessions
    let mut sessions = state.sessions.write().map_err(|e| format!("lock: {e}"))?;
    let mut scorecards = state.scorecards.write().map_err(|e| format!("lock: {e}"))?;
    for session in output
        .ab_result
        .baseline
        .sessions
        .iter()
        .chain(output.ab_result.routed.sessions.iter())
    {
        let card = crate::reporting::scorecard::AgentScorecard::from_session(session);
        sessions.push(session.clone());
        scorecards.push(card);
    }

    // Persist to disk
    let runs_dir = std::path::Path::new("data/validation_runs");
    if let Err(e) = crate::evaluation::validation_run::save_validation_run(&output, runs_dir) {
        eprintln!("Warning: failed to persist validation run: {e}");
    }

    Ok(output)
}

/// List all persisted validation runs.
pub fn list_validation_runs() -> Vec<crate::evaluation::validation_run::ValidationRunSummary> {
    let dir = std::path::Path::new("data/validation_runs");
    crate::evaluation::validation_run::list_validation_runs(dir)
}

/// Load a specific validation run from disk.
pub fn get_validation_run(
    run_label: &str,
) -> Result<crate::evaluation::validation_run::ValidationRunOutput, String> {
    let path = std::path::Path::new("data/validation_runs").join(format!("{run_label}.json"));
    crate::evaluation::validation_run::load_validation_run(&path)
}

/// Build a three-way comparison from two persisted validation runs.
pub fn three_way_comparison(
    run1_label: &str,
    run2_label: &str,
) -> Result<crate::evaluation::three_way::ThreeWayComparison, String> {
    let dir = std::path::Path::new("data/validation_runs");
    let run1 = crate::evaluation::validation_run::load_validation_run(
        &dir.join(format!("{run1_label}.json")),
    )?;
    let run2 = crate::evaluation::validation_run::load_validation_run(
        &dir.join(format!("{run2_label}.json")),
    )?;
    Ok(crate::evaluation::three_way::build_three_way(&run1, &run2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_and_get_session() {
        let state = MeasurementState::with_batteries(vec![]);
        let id = start_measurement_session(&state, "test-agent", 3).unwrap();
        let session = get_measurement_session(&state, &id).unwrap();
        assert_eq!(session.agent_id, "test-agent");
        assert_eq!(session.agent_autonomy_level, 3);
    }

    #[test]
    fn test_get_scorecard_most_recent() {
        let state = MeasurementState::with_batteries(vec![]);
        let _id1 = start_measurement_session(&state, "agent-a", 2).unwrap();
        let _id2 = start_measurement_session(&state, "agent-a", 4).unwrap();

        let card = get_agent_scorecard(&state, "agent-a").unwrap();
        // Most recent session should be returned (autonomy level 4)
        assert_eq!(card.agent_autonomy_level, 4);
    }

    #[test]
    fn test_get_scorecard_not_found() {
        let state = MeasurementState::with_batteries(vec![]);
        let result = get_agent_scorecard(&state, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_sessions_most_recent_first() {
        let state = MeasurementState::with_batteries(vec![]);
        let id1 = start_measurement_session(&state, "a", 1).unwrap();
        let id2 = start_measurement_session(&state, "b", 2).unwrap();

        let list = list_measurement_sessions(&state).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id.to_string(), id2); // most recent first
        assert_eq!(list[1].id.to_string(), id1);
    }

    #[test]
    fn test_compare_agents_error_on_missing() {
        let state = MeasurementState::with_batteries(vec![]);
        let _id = start_measurement_session(&state, "agent-x", 3).unwrap();

        let result = compare_agents(&state, &["agent-x".into(), "nonexistent".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_locked_batteries_empty() {
        let state = MeasurementState::with_batteries(vec![]);
        let summaries = get_locked_batteries(&state).unwrap();
        assert!(summaries.is_empty());
    }

    #[test]
    fn test_get_gaming_flags_empty_session() {
        let state = MeasurementState::with_batteries(vec![]);
        let id = start_measurement_session(&state, "agent-y", 2).unwrap();
        let flags = get_gaming_flags(&state, &id).unwrap();
        assert!(flags.is_empty());
    }
}
