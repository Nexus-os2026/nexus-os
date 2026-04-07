//! consent domain implementation.

#![allow(unused_imports)]

use crate::*;
use base64::Engine;
use chrono::TimeZone;
use nexus_adaptation::evolution::{EvolutionConfig, EvolutionEngine, MutationType, Strategy};
use nexus_auth::SessionManager;
use nexus_conductor::types::UserRequest;
use nexus_connectors_llm::chunking::SupportedFormat;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::model_hub::{self, DownloadProgress, DownloadStatus};
use nexus_connectors_llm::model_registry::ModelRegistry;
use nexus_connectors_llm::nexus_link::NexusLink;
use nexus_connectors_llm::providers::{
    groq::GROQ_MODELS, nvidia::NVIDIA_MODELS, ClaudeProvider, DeepSeekProvider, GeminiProvider,
    GroqProvider, LlmProvider, NvidiaProvider, OllamaProvider, OpenAiProvider,
};
use nexus_connectors_llm::rag::{RagConfig, RagPipeline};
use nexus_connectors_llm::whisper::WhisperTranscriber;
use nexus_connectors_messaging::gateway::{MessageGateway, PlatformStatus};
use nexus_distributed::ghost_protocol::{GhostConfig, GhostProtocol, SyncPeer as GhostSyncPeer};
use nexus_factory::pipeline::FactoryPipeline;
use nexus_integrations::IntegrationRouter;
use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::cognitive::PlannedAction;
use nexus_kernel::computer_control::{
    activate_emergency_kill_switch, analyze_stored_screenshot, capture_and_analyze_screen,
    capture_and_store_screen, ComputerControlEngine, InputAction, InputControlStatus, ScreenRegion,
};
use nexus_kernel::config::{
    load_config, save_config as save_nexus_config, AgentLlmConfig, HardwareConfig, ModelsConfig,
    NexusConfig, OllamaConfig,
};
use nexus_kernel::economic_identity::{EconomicConfig, EconomicEngine, TransactionType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::experience::{
    ConversationalBuilder, LivePreviewEngine, MarketplacePublisher, ProblemSolver, RemixEngine,
    TeachMode,
};
use nexus_kernel::genome::{
    crossover, genome_from_manifest, mutate, set_offspring_prompt, AgentGenome,
    AutoEvolutionManager, EvolutionConfig as AutoEvolveConfig,
    JsonAgentManifest as GenomeJsonManifest,
};
use nexus_kernel::hardware::{recommend_agent_configs, HardwareProfile};
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::manifest::{parse_manifest, AgentManifest};
use nexus_kernel::neural_bridge::{ContextQuery, ContextSource, NeuralBridge, NeuralBridgeConfig};
use nexus_kernel::permissions::{
    CapabilityRequest as KernelCapabilityRequest, PermissionCategory as KernelPermissionCategory,
    PermissionHistoryEntry as KernelPermissionHistoryEntry,
};
use nexus_kernel::protocols::a2a_client::A2aClient;
use nexus_kernel::redaction::RedactionEngine;
use nexus_kernel::simulation::{
    compare_reports, estimate_simulation_fuel, generate_personas, parse_seed,
    run_parallel_simulations as kernel_run_parallel_simulations, PersistedSimulationState,
    PredictionReport, SimulatedWorld, SimulationControl, SimulationObserver, SimulationProgress,
    SimulationRuntime, SimulationStatus as KernelSimulationStatus, SimulationSummary, WorldStatus,
};
use nexus_kernel::supervisor::{AgentId, Supervisor};
use nexus_kernel::tracing::{SpanStatus, TracingEngine};
use nexus_marketplace::payments::{BillingInterval, PaymentEngine, RevenueSplit};
use nexus_persistence::{CheckpointRow, NexusDatabase, StateStore};
use nexus_protocols::mcp_client::{McpAuth, McpHostManager, McpServerConfig, McpTransport};
use nexus_sdk::memory::{AgentMemory, MemoryConfig, MemoryType};
use nexus_tenancy::WorkspaceManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Emitter;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Manager;
use tokio::sync::Notify;
use uuid::Uuid;

// ── Consent / HITL Approval Commands ──

/// Notification payload emitted to the frontend when a consent request arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentNotification {
    pub consent_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub operation_type: String,
    pub operation_summary: String,
    pub risk_level: String,
    pub side_effects_preview: Vec<String>,
    pub fuel_cost_estimate: f64,
    pub requested_at: String,
    pub auto_deny_at: String,
    pub min_review_seconds: Option<u64>,
    pub goal_id: Option<String>,
    pub batch_action_count: Option<u32>,
    pub batch_actions: Vec<String>,
    pub review_each_available: bool,
}

/// Compute the auto-deny deadline given a risk level and creation timestamp.
pub(crate) fn compute_auto_deny_at(risk_level: &str, created_at: &str) -> String {
    use chrono::{DateTime, Duration, Utc};
    let base = created_at
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());
    // 5-minute timeout for all risk levels — enough time for user to review and respond
    let timeout = Duration::minutes(5);
    // suppress unused risk_level — all levels use the same 5-minute timeout
    let _ = risk_level;
    (base + timeout).to_rfc3339()
}

/// Map hitl_tier to a human-readable risk level string.
pub(crate) fn tier_to_risk_level(tier: &str) -> String {
    match tier {
        "Tier3" => "Critical".to_string(),
        "Tier2" => "High".to_string(),
        "Tier1" => "Medium".to_string(),
        _ => "Low".to_string(),
    }
}

/// Build a ConsentNotification from a persisted ConsentRow, enriching with agent name.
pub(crate) fn consent_row_to_notification(
    row: &nexus_persistence::ConsentRow,
    agent_name: &str,
) -> ConsentNotification {
    let risk_level = tier_to_risk_level(&row.hitl_tier);
    let auto_deny_at = compute_auto_deny_at(&risk_level, &row.created_at);

    // Parse operation_json for summary and side effects
    let op_json: serde_json::Value = serde_json::from_str(&row.operation_json).unwrap_or(json!({}));
    let summary = op_json
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or(&row.operation_type)
        .to_string();
    let side_effects = op_json
        .get("side_effects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let fuel_cost = op_json
        .get("fuel_cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let min_review_seconds = op_json
        .get("min_review_seconds")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            if row.operation_type.contains("l6")
                || row.operation_type.contains("transcendent")
                || summary.to_lowercase().contains("l6")
            {
                Some(60)
            } else {
                None
            }
        });
    let goal_id = consent_goal_id(&op_json);
    let batch_action_count = op_json
        .get("batch_action_count")
        .and_then(|v| v.as_u64())
        .map(|value| value as u32);
    let batch_actions = op_json
        .get("batch_actions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let review_each_available = op_json
        .get("review_each_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    ConsentNotification {
        consent_id: row.id.clone(),
        agent_id: row.agent_id.clone(),
        agent_name: agent_name.to_string(),
        operation_type: row.operation_type.clone(),
        operation_summary: summary,
        risk_level,
        side_effects_preview: side_effects,
        fuel_cost_estimate: fuel_cost,
        requested_at: row.created_at.clone(),
        auto_deny_at,
        min_review_seconds,
        goal_id,
        batch_action_count,
        batch_actions,
        review_each_available,
    }
}

pub fn approve_consent_request(
    state: &AppState,
    consent_id: String,
    approved_by: String,
) -> Result<(), String> {
    // Look up agent_id from pending consents in DB
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;
    let consent_row = pending
        .iter()
        .find(|r| r.id == consent_id)
        .ok_or_else(|| format!("consent request '{consent_id}' not found or already resolved"))?;
    let agent_id_str = consent_row.agent_id.clone();
    let op_json: serde_json::Value =
        serde_json::from_str(&consent_row.operation_json).unwrap_or(json!({}));

    // 1. Resolve in database
    state
        .db
        .resolve_consent(&consent_id, "approved", &approved_by)
        .map_err(|e| format!("db error: {e}"))?;

    if consent_row.operation_type == "transcendent_creation" {
        match op_json
            .get("mode")
            .and_then(|value| value.as_str())
            .unwrap_or("create_new")
        {
            "activate_existing" => {
                let parsed = parse_agent_id(&agent_id_str)?;
                let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
                supervisor.restart_agent(parsed).map_err(agent_error)?;
                // Best-effort: persist state to DB; in-memory supervisor already updated
                let _ = state.db.update_agent_state(&agent_id_str, "running");
                update_last_action(state, parsed, "started");
            }
            _ => {
                let manifest_json = op_json
                    .get("manifest_json")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        "approved transcendent creation missing manifest_json payload".to_string()
                    })?;
                let manifest = parse_agent_manifest_json(manifest_json)?;
                let created_id =
                    create_agent_immediately(state, manifest, manifest_json.to_string())?;
                // Best-effort: remove pending placeholder; new agent already created
                let _ = state.db.delete_agent(&agent_id_str);
                state.log_event(
                    SYSTEM_UUID,
                    EventType::UserAction,
                    json!({
                        "action": "transcendent_creation_approved",
                        "consent_id": consent_id,
                        "created_agent_id": created_id,
                    }),
                );
            }
        }
    }

    // 2. Approve in kernel consent runtime (best-effort — agent may not exist in supervisor)
    if let Ok(agent_uuid) = Uuid::parse_str(&agent_id_str) {
        let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: forward approval to kernel consent runtime; agent may not have a pending consent
        let _ = supervisor.approve_consent(agent_uuid, &consent_id, &approved_by);
    }
    // Best-effort: unblock cognitive loop step waiting on this consent
    let _ = state.cognitive_runtime.approve_blocked_step(&agent_id_str);
    state.wake_blocked_consent_wait(&agent_id_str, &consent_id);
    let approval_snapshot = capture_agent_snapshot(state, &agent_id_str);
    record_agent_execution_checkpoint(
        state,
        &agent_id_str,
        "approval_granted",
        approval_snapshot.as_ref(),
        approval_snapshot.as_ref(),
        consent_row.operation_type.as_str(),
    );

    // 3. Log audit event
    state.log_event(
        Uuid::parse_str(&agent_id_str).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "consent_approved",
            "consent_id": consent_id,
            "approved_by": approved_by,
        }),
    );

    Ok(())
}

pub(crate) fn deny_consent_request(
    state: &AppState,
    consent_id: String,
    denied_by: String,
    reason: Option<String>,
) -> Result<(), String> {
    // Look up agent_id from pending consents in DB
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;
    let consent_row = pending
        .iter()
        .find(|r| r.id == consent_id)
        .ok_or_else(|| format!("consent request '{consent_id}' not found or already resolved"))?;
    let agent_id_str = consent_row.agent_id.clone();
    let op_json: serde_json::Value =
        serde_json::from_str(&consent_row.operation_json).unwrap_or(json!({}));

    // 1. Resolve in database
    state
        .db
        .resolve_consent(&consent_id, "denied", &denied_by)
        .map_err(|e| format!("db error: {e}"))?;

    if consent_row.operation_type == "transcendent_creation"
        && op_json
            .get("mode")
            .and_then(|value| value.as_str())
            .unwrap_or("create_new")
            == "create_new"
    {
        // Best-effort: remove pending agent placeholder after denial
        let _ = state.db.delete_agent(&agent_id_str);
    }

    // 2. Deny in kernel consent runtime (best-effort)
    if let Ok(agent_uuid) = Uuid::parse_str(&agent_id_str) {
        let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: forward denial to kernel consent runtime; agent may not have a pending consent
        let _ = supervisor.deny_consent(agent_uuid, &consent_id, &denied_by);
    }
    let deny_reason = reason
        .clone()
        .unwrap_or_else(|| "Consent denied by user".to_string());
    // Best-effort: unblock cognitive loop step with denial reason
    let _ = state
        .cognitive_runtime
        .deny_blocked_step(&agent_id_str, Some(&deny_reason));
    state.wake_blocked_consent_wait(&agent_id_str, &consent_id);

    // 3. Log audit event
    state.log_event(
        Uuid::parse_str(&agent_id_str).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "consent_denied",
            "consent_id": consent_id,
            "denied_by": denied_by,
            "reason": reason,
        }),
    );

    Ok(())
}

pub(crate) fn batch_approve_consents(
    state: &AppState,
    goal_id: String,
    approved_by: String,
) -> Result<Vec<String>, String> {
    let consent_rows = consent_rows_for_goal(state, &goal_id)?;
    if consent_rows.is_empty() {
        return Err(format!(
            "no pending consent requests found for goal '{goal_id}'"
        ));
    }

    let agent_id = consent_rows[0].agent_id.clone();
    let approval_count = state
        .cognitive_runtime
        .pending_hitl_steps(&agent_id)
        .map(|steps| steps.len() as u32)
        .unwrap_or(consent_rows.len() as u32)
        .max(1);

    let mut resolved_ids = Vec::with_capacity(consent_rows.len());
    for row in &consent_rows {
        state
            .db
            .resolve_consent(&row.id, "approved", &approved_by)
            .map_err(|e| format!("db error: {e}"))?;
        resolved_ids.push(row.id.clone());
    }

    // Best-effort: disable review-each mode after batch approval
    let _ = state
        .cognitive_runtime
        .set_review_each_mode(&agent_id, false);
    // Best-effort: unblock all cognitive loop steps pending consent
    let _ = state
        .cognitive_runtime
        .approve_blocked_steps(&agent_id, approval_count);
    for row in &consent_rows {
        state.wake_blocked_consent_wait(&row.agent_id, &row.id);
    }

    state.log_event(
        Uuid::parse_str(&agent_id).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "consent_batch_approved",
            "goal_id": goal_id,
            "consent_ids": resolved_ids.clone(),
            "approved_by": approved_by,
            "approved_steps": approval_count,
        }),
    );

    Ok(resolved_ids)
}

pub(crate) fn review_consent_batch(
    state: &AppState,
    consent_id: String,
    reviewed_by: String,
) -> Result<(), String> {
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;
    let consent_row = pending
        .iter()
        .find(|row| row.id == consent_id)
        .ok_or_else(|| format!("consent request '{consent_id}' not found or already resolved"))?;

    state
        .db
        .resolve_consent(&consent_id, "review_each", &reviewed_by)
        .map_err(|e| format!("db error: {e}"))?;
    // Best-effort: enable review-each mode so subsequent steps require individual approval
    let _ = state
        .cognitive_runtime
        .set_review_each_mode(&consent_row.agent_id, true);
    state.wake_blocked_consent_wait(&consent_row.agent_id, &consent_id);

    state.log_event(
        Uuid::parse_str(&consent_row.agent_id).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "consent_batch_review_each",
            "consent_id": consent_id,
            "reviewed_by": reviewed_by,
        }),
    );

    Ok(())
}

pub(crate) fn batch_deny_consents(
    state: &AppState,
    goal_id: String,
    denied_by: String,
    reason: Option<String>,
) -> Result<Vec<String>, String> {
    let consent_rows = consent_rows_for_goal(state, &goal_id)?;
    if consent_rows.is_empty() {
        return Err(format!(
            "no pending consent requests found for goal '{goal_id}'"
        ));
    }

    let agent_id = consent_rows[0].agent_id.clone();
    let deny_reason = reason
        .clone()
        .unwrap_or_else(|| "Consent batch denied by user".to_string());

    let mut resolved_ids = Vec::with_capacity(consent_rows.len());
    for row in &consent_rows {
        state
            .db
            .resolve_consent(&row.id, "denied", &denied_by)
            .map_err(|e| format!("db error: {e}"))?;
        resolved_ids.push(row.id.clone());
    }

    // Best-effort: deny all blocked steps and trigger replanning
    let _ = state
        .cognitive_runtime
        .deny_blocked_steps_and_replan(&agent_id, Some(&deny_reason));
    for row in &consent_rows {
        state.wake_blocked_consent_wait(&row.agent_id, &row.id);
    }

    state.log_event(
        Uuid::parse_str(&agent_id).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "consent_batch_denied",
            "goal_id": goal_id,
            "consent_ids": resolved_ids.clone(),
            "denied_by": denied_by,
            "reason": reason,
        }),
    );

    Ok(resolved_ids)
}

pub(crate) fn list_pending_consents(state: &AppState) -> Result<Vec<ConsentNotification>, String> {
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;

    let meta = state.meta.lock().unwrap_or_else(|p| p.into_inner());
    let notifications: Vec<ConsentNotification> = pending
        .iter()
        .map(|row| {
            let agent_name = row
                .agent_id
                .parse::<Uuid>()
                .ok()
                .and_then(|uuid| meta.get(&uuid).map(|m| m.name.clone()))
                .unwrap_or_else(|| row.agent_id.clone());
            consent_row_to_notification(row, &agent_name)
        })
        .collect();
    Ok(notifications)
}

pub(crate) fn get_consent_history(
    state: &AppState,
    limit: u32,
) -> Result<Vec<ConsentNotification>, String> {
    let all = state
        .db
        .load_all_consents(limit)
        .map_err(|e| format!("db error: {e}"))?;

    let meta = state.meta.lock().unwrap_or_else(|p| p.into_inner());
    let notifications: Vec<ConsentNotification> = all
        .iter()
        .map(|row| {
            let agent_name = row
                .agent_id
                .parse::<Uuid>()
                .ok()
                .and_then(|uuid| meta.get(&uuid).map(|m| m.name.clone()))
                .unwrap_or_else(|| row.agent_id.clone());
            let mut notif = consent_row_to_notification(row, &agent_name);
            // For resolved items, include the status info in risk_level field
            if row.status != "pending" {
                notif.risk_level = format!("{}:{}", tier_to_risk_level(&row.hitl_tier), row.status);
            }
            notif
        })
        .collect();
    Ok(notifications)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HitlStats {
    pub pending_count: usize,
    pub approval_rate: f64,
    pub avg_response_time_ms: i64,
    pub total_decisions_today: usize,
    pub total_approvals: usize,
    pub total_denials: usize,
}

pub(crate) fn hitl_stats(state: &AppState) -> Result<HitlStats, String> {
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;
    let pending_count = pending.len();

    let approval_rate = state.db.hitl_approval_rate(None).unwrap_or(1.0);

    // Load recent decisions for avg response time and today count
    let decisions = state.db.load_hitl_decisions(None, 1000).unwrap_or_default();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let today_decisions: Vec<&nexus_persistence::HitlDecisionRow> = decisions
        .iter()
        .filter(|d| d.decided_at.starts_with(&today))
        .collect();

    let total_decisions_today = today_decisions.len();

    let avg_response_time_ms = if decisions.is_empty() {
        0
    } else {
        let sum: i64 = decisions.iter().map(|d| d.response_time_ms).sum();
        sum / decisions.len() as i64
    };

    let total_approvals = decisions
        .iter()
        .filter(|d| d.decision == "approved")
        .count();
    let total_denials = decisions.iter().filter(|d| d.decision == "denied").count();

    Ok(HitlStats {
        pending_count,
        approval_rate,
        avg_response_time_ms,
        total_decisions_today,
        total_approvals,
        total_denials,
    })
}

pub(crate) fn build_simulation_llm() -> Arc<dyn nexus_kernel::cognitive::PlannerLlm> {
    #[cfg(test)]
    {
        Arc::new(TestSimulationPlannerLlm)
    }

    #[cfg(not(test))]
    Arc::new(SimulationPlannerLlm)
}

pub(crate) fn load_persisted_simulation_state(
    row: &nexus_persistence::SimulationWorldRow,
) -> Result<PersistedSimulationState, String> {
    serde_json::from_str::<PersistedSimulationState>(&row.config_json)
        .or_else(|_| {
            serde_json::from_str::<SimulatedWorld>(&row.config_json).map(|world| {
                PersistedSimulationState {
                    world,
                    max_ticks: 100,
                    tick_interval_ms: 1_000,
                    batch_size: 25,
                    persona_decision_timeout_ms: 30_000,
                    belief_update_rate: 0.12,
                    fuel_consumed: 0.0,
                }
            })
        })
        .map_err(|error| format!("invalid simulation state: {error}"))
}

pub(crate) fn simulation_status_view(
    row: &nexus_persistence::SimulationWorldRow,
    persisted: &PersistedSimulationState,
) -> SimulationStatusView {
    let variables = persisted
        .world
        .environment
        .variables
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    SimulationStatusView {
        world_id: row.id.clone(),
        name: row.name.clone(),
        status: row.status.clone(),
        tick_count: row.tick_count.max(0) as u64,
        persona_count: row.persona_count.max(0) as usize,
        max_ticks: persisted.max_ticks,
        tick_interval_ms: persisted.tick_interval_ms,
        fuel_consumed: persisted.fuel_consumed,
        estimated_fuel: estimate_simulation_fuel(
            row.persona_count.max(0) as usize,
            persisted.max_ticks,
            persisted.batch_size,
        ),
        report_available: row.report_json.is_some(),
        variables,
        personas: persisted
            .world
            .personas
            .iter()
            .map(|persona| SimulationPersonaView {
                id: persona.id.clone(),
                name: persona.name.clone(),
                role: persona.role.clone(),
                personality: SimulationPersonalityView {
                    openness: persona.personality.openness,
                    conscientiousness: persona.personality.conscientiousness,
                    extraversion: persona.personality.extraversion,
                    agreeableness: persona.personality.agreeableness,
                    neuroticism: persona.personality.neuroticism,
                },
                beliefs: persona.beliefs.clone(),
                memories: persona
                    .memories
                    .iter()
                    .rev()
                    .take(5)
                    .map(|memory| SimulationMemoryView {
                        event: memory.event.clone(),
                        timestamp: memory.timestamp,
                        emotional_impact: memory.emotional_impact,
                        source: memory.source.clone(),
                    })
                    .collect(),
                relationships: persona.relationships.clone(),
                influence_score: persona.influence_score,
                last_action: persona.last_action.clone(),
            })
            .collect(),
    }
}

pub(crate) fn create_simulation(
    state: &AppState,
    name: String,
    seed_text: String,
    persona_count: u32,
    max_ticks: u32,
    tick_interval_ms: Option<u64>,
) -> Result<String, String> {
    let world_id = Uuid::new_v4().to_string();
    let llm = build_simulation_llm();
    let seed = parse_seed(&seed_text, llm.as_ref()).map_err(|error| error.to_string())?;
    let tick_interval_ms = tick_interval_ms.unwrap_or(1_000).clamp(500, 5_000);
    let status = if persona_count > 100 {
        "awaiting_approval"
    } else {
        "ready"
    };
    let personas = if persona_count > 100 {
        Vec::new()
    } else {
        generate_personas(&seed.scenario, persona_count as usize, llm.as_ref())
            .map_err(|error| error.to_string())?
    };
    let mut world = SimulatedWorld::from_seed(
        world_id.clone(),
        name.clone(),
        seed.scenario.clone(),
        &seed,
        personas,
        llm.as_ref(),
    )
    .map_err(|error| error.to_string())?;
    if persona_count > 100 {
        world.status = WorldStatus::Building;
        let consent = nexus_persistence::ConsentRow {
            id: Uuid::new_v4().to_string(),
            agent_id: world_id.clone(),
            operation_type: "large_simulation".to_string(),
            operation_json: json!({
                "name": name.clone(),
                "persona_count": persona_count,
                "max_ticks": max_ticks,
                "estimated_fuel": estimate_simulation_fuel(persona_count as usize, max_ticks as u64, 25),
            })
            .to_string(),
            hitl_tier: "tier2".to_string(),
            status: "pending".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
            resolved_by: None,
        };
        state
            .db
            .enqueue_consent(&consent)
            .map_err(|error| format!("db error: {error}"))?;
    }
    let persisted = PersistedSimulationState {
        world: world.clone(),
        max_ticks: max_ticks as u64,
        tick_interval_ms,
        batch_size: 25,
        persona_decision_timeout_ms: 30_000,
        belief_update_rate: 0.12,
        fuel_consumed: 0.0,
    };
    state
        .db
        .save_simulation_world(
            &world_id,
            &name,
            &seed_text,
            status,
            0,
            persona_count as i64,
            &serde_json::to_string(&persisted).map_err(|error| error.to_string())?,
            None,
            None,
        )
        .map_err(|error| format!("db error: {error}"))?;
    state
        .db
        .replace_simulation_personas(
            &world_id,
            &world
                .personas
                .iter()
                .map(|persona| {
                    (
                        format!("{}::{}", world_id, persona.id),
                        persona.name.clone(),
                        persona.role.clone(),
                        serde_json::to_string(&persona.personality).unwrap_or_default(),
                        serde_json::to_string(&persona.beliefs).unwrap_or_default(),
                        serde_json::to_string(&persona.memories).unwrap_or_default(),
                        serde_json::to_string(&persona.relationships).unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|error| format!("db error: {error}"))?;
    state.log_event(
        Uuid::parse_str(&world_id).unwrap_or(SYSTEM_UUID),
        EventType::UserAction,
        json!({
            "action": "create_simulation",
            "world_id": world_id.clone(),
            "name": name.clone(),
            "persona_count": persona_count,
            "max_ticks": max_ticks,
            "tick_interval_ms": tick_interval_ms,
        }),
    );
    Ok(world_id)
}

pub(crate) fn start_simulation_with_observer(
    state: &AppState,
    world_id: String,
    observer: Arc<dyn SimulationObserver>,
) -> Result<(), String> {
    if let Some(handle) = state.simulation_manager.get(&world_id) {
        handle.control.paused.store(false, Ordering::Relaxed);
        let row = state
            .db
            .load_simulation_world(&world_id)
            .map_err(|error| format!("db error: {error}"))?
            .ok_or_else(|| format!("simulation {world_id} not found"))?;
        state
            .db
            .save_simulation_world(
                &row.id,
                &row.name,
                &row.seed_text,
                "running",
                row.tick_count,
                row.persona_count,
                &row.config_json,
                row.report_json.as_deref(),
                row.completed_at.as_deref(),
            )
            .map_err(|error| format!("db error: {error}"))?;
        return Ok(());
    }

    let row = state
        .db
        .load_simulation_world(&world_id)
        .map_err(|error| format!("db error: {error}"))?
        .ok_or_else(|| format!("simulation {world_id} not found"))?;
    if row.status == "awaiting_approval" {
        return Err(
            "HITL approval is required before starting simulations over 100 personas".to_string(),
        );
    }
    let persisted = load_persisted_simulation_state(&row)?;
    let llm = build_simulation_llm();
    let db = state.db.clone();
    let manager = state.simulation_manager.clone();
    let control = SimulationControl::default();
    manager.insert(
        world_id.clone(),
        SimulationHandle {
            control: control.clone(),
            max_ticks: persisted.max_ticks,
        },
    );
    let thread_world_id = world_id.clone();
    thread::spawn(move || {
        let mut runtime = SimulationRuntime::new(persisted.world, llm, db.clone())
            .with_control(control.clone())
            .with_observer(observer);
        runtime.max_ticks = persisted.max_ticks;
        runtime.tick_interval_ms = persisted.tick_interval_ms;
        runtime.batch_size = persisted.batch_size;
        runtime.persona_decision_timeout_ms = persisted.persona_decision_timeout_ms;
        runtime.belief_update_rate = persisted.belief_update_rate;
        if let Err(error) = runtime.run_simulation() {
            let failed_state = PersistedSimulationState {
                fuel_consumed: runtime.fuel_consumed(),
                ..runtime.persisted_state()
            };
            // Best-effort: persist failed simulation state for debugging
            let _ = db.save_simulation_world(
                &thread_world_id,
                &failed_state.world.name,
                &failed_state.world.description,
                "failed",
                failed_state.world.tick_count as i64,
                failed_state.world.personas.len() as i64,
                &serde_json::to_string(&failed_state).unwrap_or_default(),
                Some(
                    &json!({
                        "summary": format!("Simulation failed: {error}"),
                        "key_findings": [],
                        "opinion_shifts": [],
                        "coalitions": [],
                        "turning_points": [],
                        "prediction": "failed",
                        "confidence": 0.0,
                        "uncertainties": [error.to_string()],
                    })
                    .to_string(),
                ),
                Some(&chrono::Utc::now().to_rfc3339()),
            );
        }
        manager.remove(&thread_world_id);
    });
    Ok(())
}

pub(crate) fn pause_simulation(state: &AppState, world_id: String) -> Result<(), String> {
    let handle = state
        .simulation_manager
        .get(&world_id)
        .ok_or_else(|| format!("simulation {world_id} is not running"))?;
    handle.control.paused.store(true, Ordering::Relaxed);
    let row = state
        .db
        .load_simulation_world(&world_id)
        .map_err(|error| format!("db error: {error}"))?
        .ok_or_else(|| format!("simulation {world_id} not found"))?;
    state
        .db
        .save_simulation_world(
            &row.id,
            &row.name,
            &row.seed_text,
            "paused",
            row.tick_count,
            row.persona_count,
            &row.config_json,
            row.report_json.as_deref(),
            row.completed_at.as_deref(),
        )
        .map_err(|error| format!("db error: {error}"))?;
    Ok(())
}

pub(crate) fn inject_simulation_variable(
    state: &AppState,
    world_id: String,
    key: String,
    value: String,
) -> Result<(), String> {
    if let Some(handle) = state.simulation_manager.get(&world_id) {
        handle
            .control
            .pending_injections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back((key.clone(), value.clone()));
    } else {
        let row = state
            .db
            .load_simulation_world(&world_id)
            .map_err(|error| format!("db error: {error}"))?
            .ok_or_else(|| format!("simulation {world_id} not found"))?;
        let mut persisted = load_persisted_simulation_state(&row)?;
        persisted.world.inject_variable(key.clone(), value.clone());
        state
            .db
            .save_simulation_world(
                &row.id,
                &row.name,
                &row.seed_text,
                &row.status,
                row.tick_count,
                row.persona_count,
                &serde_json::to_string(&persisted).map_err(|error| error.to_string())?,
                row.report_json.as_deref(),
                row.completed_at.as_deref(),
            )
            .map_err(|error| format!("db error: {error}"))?;
    }
    Ok(())
}

pub(crate) fn get_simulation_status(
    state: &AppState,
    world_id: String,
) -> Result<SimulationStatusView, String> {
    let row = state
        .db
        .load_simulation_world(&world_id)
        .map_err(|error| format!("db error: {error}"))?
        .ok_or_else(|| format!("simulation {world_id} not found"))?;
    let mut persisted = load_persisted_simulation_state(&row)?;
    if let Some(handle) = state.simulation_manager.get(&world_id) {
        persisted.max_ticks = handle.max_ticks;
    }
    Ok(simulation_status_view(&row, &persisted))
}

pub(crate) fn get_simulation_report(
    state: &AppState,
    world_id: String,
) -> Result<PredictionReport, String> {
    let row = state
        .db
        .load_simulation_world(&world_id)
        .map_err(|error| format!("db error: {error}"))?
        .ok_or_else(|| format!("simulation {world_id} not found"))?;
    let report_json = row
        .report_json
        .ok_or_else(|| "simulation report not available yet".to_string())?;
    serde_json::from_str::<PredictionReport>(&report_json)
        .map_err(|error| format!("invalid report json: {error}"))
}

pub(crate) fn chat_with_simulation_persona(
    state: &AppState,
    world_id: String,
    persona_id: String,
    message: String,
) -> Result<String, String> {
    let row = state
        .db
        .load_simulation_world(&world_id)
        .map_err(|error| format!("db error: {error}"))?
        .ok_or_else(|| format!("simulation {world_id} not found"))?;
    let persisted = load_persisted_simulation_state(&row)?;
    let runtime = SimulationRuntime::new(persisted.world, build_simulation_llm(), state.db.clone());
    runtime
        .chat_with_persona(&persona_id, &message)
        .map_err(|error| error.to_string())
}

pub(crate) fn list_simulations(state: &AppState) -> Result<Vec<SimulationSummary>, String> {
    let rows = state
        .db
        .list_simulation_worlds()
        .map_err(|error| format!("db error: {error}"))?;
    rows.into_iter()
        .map(|row| {
            let prediction_summary = row
                .report_json
                .as_deref()
                // Optional: report JSON may be absent or malformed; summary will be None
                .and_then(|report_json| serde_json::from_str::<PredictionReport>(report_json).ok())
                .map(|report| report.summary);
            let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|value| value.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let status = match row.status.as_str() {
                "awaiting_approval" => KernelSimulationStatus::AwaitingApproval,
                "ready" => KernelSimulationStatus::Ready,
                "running" => KernelSimulationStatus::Running,
                "paused" => KernelSimulationStatus::Paused,
                "completed" => KernelSimulationStatus::Completed,
                "failed" => KernelSimulationStatus::Failed,
                _ => KernelSimulationStatus::Draft,
            };
            Ok(SimulationSummary {
                id: row.id,
                name: row.name,
                status,
                tick_count: row.tick_count.max(0) as u64,
                persona_count: row.persona_count.max(0) as usize,
                created_at,
                prediction_summary,
            })
        })
        .collect()
}

pub(crate) fn run_parallel_simulation_reports(
    state: &AppState,
    seed_text: String,
    variant_count: u32,
) -> Result<Vec<PredictionReport>, String> {
    let llm = build_simulation_llm();
    let seed = parse_seed(&seed_text, llm.as_ref()).map_err(|error| error.to_string())?;
    let reports =
        kernel_run_parallel_simulations(&seed, variant_count as usize, llm, state.db.clone())
            .map_err(|error| error.to_string())?;
    let analysis = compare_reports(&reports);
    state.log_event(
        Uuid::new_v4(),
        EventType::UserAction,
        json!({
            "action": "run_parallel_simulations",
            "variant_count": variant_count,
            "consensus_prediction": analysis.consensus_prediction,
            "confidence": analysis.confidence,
        }),
    );
    Ok(reports)
}

// ── Immune System commands ──────────────────────────────────────────

pub(crate) fn get_immune_status(state: &AppState) -> Result<serde_json::Value, String> {
    let scan_results = state
        .immune_scan_results
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let threats_blocked = scan_results.len() as u64;
    let threat_level = if scan_results
        .iter()
        .any(|t| matches!(t.severity, nexus_kernel::immune::ThreatSeverity::Critical))
    {
        nexus_kernel::immune::ThreatLevel::Red
    } else if scan_results
        .iter()
        .any(|t| matches!(t.severity, nexus_kernel::immune::ThreatSeverity::High))
    {
        nexus_kernel::immune::ThreatLevel::Orange
    } else if !scan_results.is_empty() {
        nexus_kernel::immune::ThreatLevel::Yellow
    } else {
        nexus_kernel::immune::ThreatLevel::Green
    };

    let last_scan = state
        .immune_last_scan
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    let memory = nexus_kernel::immune::ImmuneMemory::new();
    let active_antibodies = memory.all_signatures().count();

    let status = nexus_kernel::immune::ImmuneStatus {
        threat_level,
        active_antibodies,
        threats_blocked,
        last_scan: *last_scan,
        privacy_violations_blocked: scan_results
            .iter()
            .filter(|t| {
                matches!(
                    t.threat_type,
                    nexus_kernel::immune::ThreatType::DataExfiltration
                )
            })
            .count() as u64,
    };
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

pub(crate) fn get_threat_log(state: &AppState) -> Result<serde_json::Value, String> {
    let scan_results = state
        .immune_scan_results
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    serde_json::to_value(&*scan_results).map_err(|e| e.to_string())
}

pub(crate) fn trigger_immune_scan(state: &AppState) -> Result<(), String> {
    let mut detector = nexus_kernel::immune::ThreatDetector::new();
    let mut all_threats: Vec<nexus_kernel::immune::ThreatEvent> = Vec::new();

    // Scan all agent system prompts for injection/exfil patterns
    let agents_dir = std::path::Path::new("agents/prebuilt");
    if agents_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(agents_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let agent_name = entry
                        .path()
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let threats = detector.scan(&agent_name, &content);
                    all_threats.extend(threats);
                }
            }
        }
    }

    // Also scan generated agents
    let generated_dir = std::path::Path::new("agents/generated");
    if generated_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(generated_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let agent_name = entry
                        .path()
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let threats = detector.scan(&agent_name, &content);
                    all_threats.extend(threats);
                }
            }
        }
    }

    // Scan recent audit log for suspicious patterns
    {
        let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
        let events = audit.events();
        let start = events.len().saturating_sub(50);
        for event in &events[start..] {
            let text = serde_json::to_string(&event.payload).unwrap_or_default();
            let threats = detector.scan("audit-trail", &text);
            all_threats.extend(threats);
        }
    }

    // Store results and update last scan timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    {
        let mut scan_results = state
            .immune_scan_results
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *scan_results = all_threats;
    }
    {
        let mut last_scan = state
            .immune_last_scan
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *last_scan = now;
    }
    Ok(())
}

pub(crate) fn run_adversarial_session(
    attacker_id: String,
    defender_id: String,
    rounds: u32,
) -> Result<serde_json::Value, String> {
    let mut arena = nexus_kernel::immune::AdversarialArena::new();
    let session = arena.run_session(&attacker_id, &defender_id, rounds);
    serde_json::to_value(&session).map_err(|e| e.to_string())
}

pub(crate) fn get_immune_memory() -> Result<serde_json::Value, String> {
    let memory = nexus_kernel::immune::ImmuneMemory::new();
    let sigs: Vec<_> = memory.all_signatures().collect();
    serde_json::to_value(&sigs).map_err(|e| e.to_string())
}

pub(crate) fn set_privacy_rules(rules: serde_json::Value) -> Result<(), String> {
    let _rules: Vec<nexus_kernel::immune::PrivacyRule> =
        serde_json::from_value(rules).map_err(|e| e.to_string())?;
    Ok(())
}
