//! governance domain implementation.

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

// ── Permission Dashboard Commands ──

/// A single permission update for bulk operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionUpdate {
    pub capability_key: String,
    pub enabled: bool,
}

pub(crate) fn get_agent_permissions(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelPermissionCategory>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_agent_permissions(parsed)
        .map_err(agent_error)
}

pub(crate) fn update_agent_permission(
    state: &AppState,
    agent_id: String,
    capability_key: String,
    enabled: bool,
) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .update_agent_permission(parsed, &capability_key, enabled, "user", None)
        .map_err(agent_error)?;

    // Persist permission change
    if enabled {
        if let Err(e) = state
            .db
            .grant_permission(&parsed.to_string(), &capability_key, "medium")
        {
            eprintln!("persistence: grant_permission failed: {e}");
        }
    } else if let Err(e) = state
        .db
        .revoke_permission(&parsed.to_string(), &capability_key)
    {
        eprintln!("persistence: revoke_permission failed: {e}");
    }

    state.log_event(
        parsed,
        EventType::UserAction,
        json!({
            "event": "update_agent_permission",
            "capability": capability_key,
            "enabled": enabled,
        }),
    );
    Ok(())
}

pub(crate) fn get_permission_history(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelPermissionHistoryEntry>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_permission_history(parsed)
        .map_err(agent_error)
}

pub(crate) fn get_capability_request(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelCapabilityRequest>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_capability_requests(parsed)
        .map_err(agent_error)
}

pub(crate) fn bulk_update_permissions(
    state: &AppState,
    agent_id: String,
    updates: Vec<PermissionUpdate>,
    reason: Option<String>,
) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let update_pairs: Vec<(String, bool)> = updates
        .iter()
        .map(|u| (u.capability_key.clone(), u.enabled))
        .collect();
    supervisor
        .bulk_update_agent_permissions(parsed, &update_pairs, "user", reason.as_deref())
        .map_err(agent_error)?;

    // Persist bulk permission changes
    for u in &updates {
        if u.enabled {
            // Best-effort: persist permission grant; in-memory supervisor already updated
            let _ = state
                .db
                .grant_permission(&parsed.to_string(), &u.capability_key, "medium");
        } else {
            // Best-effort: persist permission revocation; in-memory supervisor already updated
            let _ = state
                .db
                .revoke_permission(&parsed.to_string(), &u.capability_key);
        }
    }

    state.log_event(
        parsed,
        EventType::UserAction,
        json!({
            "event": "bulk_update_permissions",
            "updates": updates.len(),
            "reason": reason,
        }),
    );
    Ok(())
}

// ── Policy Engine API ──

pub(crate) fn policy_list() -> Result<serde_json::Value, String> {
    let dir = dirs_policy_dir();
    let mut engine = nexus_kernel::policy_engine::PolicyEngine::new(&dir);
    // Best-effort: load policies from disk; empty set is valid if directory is missing
    let _ = engine.load_policies();
    let policies: Vec<serde_json::Value> = engine
        .policies()
        .iter()
        .map(|p| {
            json!({
                "policy_id": p.policy_id,
                "description": p.description,
                "effect": format!("{:?}", p.effect),
                "principal": p.principal,
                "action": p.action,
                "resource": p.resource,
                "priority": p.priority,
                "conditions": {
                    "min_autonomy_level": p.conditions.min_autonomy_level,
                    "max_fuel_cost": p.conditions.max_fuel_cost,
                    "required_approvers": p.conditions.required_approvers,
                    "time_window": p.conditions.time_window,
                },
            })
        })
        .collect();
    Ok(json!({ "policies": policies, "count": policies.len() }))
}

pub(crate) fn policy_validate(content: String) -> Result<serde_json::Value, String> {
    match toml::from_str::<nexus_kernel::policy_engine::Policy>(&content) {
        Ok(policy) => Ok(json!({
            "valid": true,
            "policy_id": policy.policy_id,
            "effect": format!("{:?}", policy.effect),
        })),
        Err(e) => Ok(json!({
            "valid": false,
            "error": e.to_string(),
        })),
    }
}

pub(crate) fn policy_test(
    content: String,
    principal: String,
    action: String,
    resource: String,
) -> Result<serde_json::Value, String> {
    let policy: nexus_kernel::policy_engine::Policy =
        toml::from_str(&content).map_err(|e| format!("invalid policy TOML: {e}"))?;
    let engine = nexus_kernel::policy_engine::PolicyEngine::with_policies(vec![policy]);
    let ctx = nexus_kernel::policy_engine::EvaluationContext::default();
    let decision = engine.evaluate(&principal, &action, &resource, &ctx);
    Ok(json!({
        "principal": principal,
        "action": action,
        "resource": resource,
        "decision": format!("{decision:?}"),
    }))
}

pub(crate) fn policy_detect_conflicts() -> Result<serde_json::Value, String> {
    let dir = dirs_policy_dir();
    let mut engine = nexus_kernel::policy_engine::PolicyEngine::new(&dir);
    // Best-effort: load policies from disk; empty set means no conflicts detected
    let _ = engine.load_policies();

    let policies = engine.policies();
    let mut conflicts: Vec<serde_json::Value> = Vec::new();

    for (i, a) in policies.iter().enumerate() {
        for b in policies.iter().skip(i + 1) {
            let principal_overlap =
                a.principal == "*" || b.principal == "*" || a.principal == b.principal;
            let action_overlap = a.action == "*" || b.action == "*" || a.action == b.action;
            let resource_overlap =
                a.resource == "*" || b.resource == "*" || a.resource == b.resource;
            let effect_differs = a.effect != b.effect;

            if principal_overlap && action_overlap && resource_overlap && effect_differs {
                conflicts.push(json!({
                    "policy_a": a.policy_id,
                    "policy_b": b.policy_id,
                    "effect_a": format!("{:?}", a.effect),
                    "effect_b": format!("{:?}", b.effect),
                    "overlap": {
                        "principal": if a.principal == b.principal { &a.principal } else { "*" },
                        "action": if a.action == b.action { &a.action } else { "*" },
                        "resource": if a.resource == b.resource { &a.resource } else { "*" },
                    },
                }));
            }
        }
    }

    Ok(json!({ "conflicts": conflicts, "count": conflicts.len() }))
}

pub(crate) fn dirs_policy_dir() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home)
            .join(".nexus")
            .join("policies")
    } else {
        std::path::PathBuf::from("~/.nexus/policies")
    }
}

/// Check if setup has been completed (hardware detected).
pub(crate) fn is_setup_complete() -> bool {
    match load_config() {
        Ok(cfg) => !cfg.hardware.gpu.is_empty() && cfg.hardware.gpu != "none",
        Err(_) => false,
    }
}

pub(crate) fn run_setup_wizard(ollama_url: Option<String>) -> Result<SetupResult, String> {
    let hw_info = detect_hardware()?;
    let ollama_status = check_ollama(ollama_url.clone())?;

    // Build and save config
    let mut config = load_config().map_err(|e| e.to_string())?;

    config.hardware = HardwareConfig {
        gpu: hw_info.gpu.clone(),
        vram_mb: hw_info.vram_mb,
        ram_mb: hw_info.ram_mb,
        detected_at: hw_info.detected_at.clone(),
    };

    config.ollama = OllamaConfig {
        base_url: ollama_status.base_url.clone(),
        status: if ollama_status.connected {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        },
    };
    config.llm.ollama_url = ollama_status.base_url.clone();

    config.models = ModelsConfig {
        primary: hw_info.recommended_primary.clone(),
        fast: hw_info.recommended_fast.clone(),
    };

    // Set default model to the recommended primary
    if ollama_status.connected {
        config.llm.default_model = hw_info.recommended_primary.clone();
    }

    // Apply agent configs
    let hw = HardwareProfile {
        gpu: hw_info.gpu.clone(),
        vram_mb: hw_info.vram_mb,
        ram_mb: hw_info.ram_mb,
        detected_at: hw_info.detected_at.clone(),
    };
    let tier = hw.recommended_tier();
    let agent_configs = recommend_agent_configs(tier);
    let mut agents_map = BTreeMap::new();
    for (name, ac) in &agent_configs {
        agents_map.insert(
            name.to_string(),
            AgentLlmConfig {
                model: ac.model.clone(),
                temperature: ac.temperature,
                max_tokens: ac.max_tokens,
            },
        );
    }
    config.agents = agents_map;

    let config_saved = save_nexus_config(&config).is_ok();

    Ok(SetupResult {
        hardware: hw_info,
        ollama: ollama_status,
        config_saved,
    })
}

// ── Protocols Dashboard Commands ──

/// Protocol server status for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolsStatusRow {
    pub a2a_status: String,
    pub a2a_version: String,
    pub a2a_peers: u32,
    pub a2a_tasks_processed: u64,
    pub mcp_status: String,
    pub mcp_registered_tools: u32,
    pub mcp_invocations: u64,
    pub gateway_port: Option<u16>,
    pub governance_bridge_active: bool,
    pub audit_integrity: bool,
}

/// A protocol request log entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRequestRow {
    pub id: String,
    pub timestamp: u64,
    pub protocol: String,
    pub method: String,
    pub sender: String,
    pub agent: String,
    pub status: String,
    pub fuel_consumed: u64,
    pub governance_decision: String,
}

/// MCP tool entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRow {
    pub name: String,
    pub description: String,
    pub agent: String,
    pub fuel_cost: u64,
    pub requires_hitl: bool,
    pub invocations: u64,
}

/// Agent Card summary for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardRow {
    pub agent_name: String,
    pub url: String,
    pub skills_count: usize,
    pub auth_scheme: String,
    pub rate_limit_rpm: u64,
    pub card_json: serde_json::Value,
}

pub(crate) fn get_protocols_status(state: &AppState) -> Result<ProtocolsStatusRow, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let audit = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let agent_count = supervisor.health_check().len() as u32;

    Ok(ProtocolsStatusRow {
        a2a_status: "stopped".to_string(),
        a2a_version: "0.2.1".to_string(),
        a2a_peers: 0,
        a2a_tasks_processed: 0,
        mcp_status: "stopped".to_string(),
        mcp_registered_tools: agent_count * 3, // estimate: ~3 tools per agent
        mcp_invocations: 0,
        gateway_port: None,
        governance_bridge_active: false,
        audit_integrity: audit.verify_integrity(),
    })
}

pub(crate) fn get_protocols_requests(_state: &AppState) -> Result<Vec<ProtocolRequestRow>, String> {
    // Return recent protocol requests — empty until gateway is started
    Ok(Vec::new())
}

pub(crate) fn get_mcp_tools(state: &AppState) -> Result<Vec<McpToolRow>, String> {
    use nexus_kernel::protocols::mcp::McpServer;

    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let mut mcp = McpServer::new();
            mcp.register_agent(agent_status.id, handle.manifest.clone());
            if let Ok(tools) = mcp.list_tools(agent_status.id) {
                for tool in tools {
                    rows.push(McpToolRow {
                        name: tool.name,
                        description: tool.description.unwrap_or_default(),
                        agent: handle.manifest.name.clone(),
                        fuel_cost: tool.governance.estimated_fuel_cost,
                        requires_hitl: tool.governance.requires_hitl,
                        invocations: 0,
                    });
                }
            }
        }
    }
    Ok(rows)
}

pub(crate) fn get_agent_cards(state: &AppState) -> Result<Vec<AgentCardRow>, String> {
    use nexus_kernel::protocols::a2a::AgentCard;

    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let card = AgentCard::from_manifest(&handle.manifest, "http://localhost:3000");
            let card_json = serde_json::to_value(&card).unwrap_or_default();
            let auth_scheme = if card.authentication.is_empty() {
                "none".to_string()
            } else {
                card.authentication
                    .iter()
                    .map(|a| a.scheme_type.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            rows.push(AgentCardRow {
                agent_name: card.name.clone(),
                url: card.url.clone(),
                skills_count: card.skills.len(),
                auth_scheme,
                rate_limit_rpm: card.rate_limit_rpm.unwrap_or(0),
                card_json,
            });
        }
    }
    Ok(rows)
}

// ── A2A Client Commands ──

pub(crate) fn a2a_discover_agent(
    state: &AppState,
    url: String,
) -> Result<serde_json::Value, String> {
    let mut client = state.a2a_client.lock().unwrap_or_else(|p| p.into_inner());
    let card = client
        .discover_agent(&url)
        .map_err(|e| format!("A2A discovery failed: {e}"))?;
    serde_json::to_value(&card).map_err(|e| e.to_string())
}

pub(crate) fn a2a_send_task(
    state: &AppState,
    agent_url: String,
    message: String,
) -> Result<serde_json::Value, String> {
    let mut client = state.a2a_client.lock().unwrap_or_else(|p| p.into_inner());
    let result = client
        .send_task(&agent_url, &message)
        .map_err(|e| format!("A2A send failed: {e}"))?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

pub(crate) fn a2a_get_task_status(
    state: &AppState,
    agent_url: String,
    task_id: String,
) -> Result<serde_json::Value, String> {
    let mut client = state.a2a_client.lock().unwrap_or_else(|p| p.into_inner());
    let result = client
        .get_task_status(&agent_url, &task_id)
        .map_err(|e| format!("A2A status failed: {e}"))?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

pub(crate) fn a2a_cancel_task(
    state: &AppState,
    agent_url: String,
    task_id: String,
) -> Result<(), String> {
    let mut client = state.a2a_client.lock().unwrap_or_else(|p| p.into_inner());
    client
        .cancel_task(&agent_url, &task_id)
        .map_err(|e| format!("A2A cancel failed: {e}"))
}

pub(crate) fn a2a_known_agents(state: &AppState) -> Result<serde_json::Value, String> {
    let client = state.a2a_client.lock().unwrap_or_else(|p| p.into_inner());
    let agents: Vec<_> = client.known_agents().into_iter().cloned().collect();
    serde_json::to_value(&agents).map_err(|e| e.to_string())
}

// ── Identity Commands ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRow {
    pub agent_id: String,
    pub did: String,
    pub created_at: u64,
    pub public_key_hex: String,
}

pub(crate) fn get_agent_identity(
    state: &AppState,
    agent_id: String,
) -> Result<IdentityRow, String> {
    let uuid = uuid::Uuid::parse_str(&agent_id).map_err(|e| format!("invalid UUID: {e}"))?;
    let mut mgr = state.identity_mgr.lock().map_err(|e| e.to_string())?;
    let identity = mgr
        .get_or_create(uuid)
        .map_err(|e| format!("identity error: {e}"))?;
    Ok(IdentityRow {
        agent_id: uuid.to_string(),
        did: identity.did.clone(),
        created_at: identity.created_at,
        public_key_hex: identity
            .public_key_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
}

pub(crate) fn list_identities(state: &AppState) -> Result<Vec<IdentityRow>, String> {
    let sup = state.supervisor.lock().map_err(|e| e.to_string())?;
    let mut mgr = state.identity_mgr.lock().map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for agent_status in sup.health_check() {
        if let Ok(identity) = mgr.get_or_create(agent_status.id) {
            rows.push(IdentityRow {
                agent_id: agent_status.id.to_string(),
                did: identity.did.clone(),
                created_at: identity.created_at,
                public_key_hex: identity
                    .public_key_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            });
        }
    }
    Ok(rows)
}

// ── Firewall Commands ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallStatusRow {
    pub status: String,
    pub mode: String,
    pub injection_pattern_count: usize,
    pub pii_pattern_count: usize,
    pub exfil_pattern_count: usize,
    pub sensitive_path_count: usize,
    pub ssn_detection: bool,
    pub passport_detection: bool,
    pub internal_ip_detection: bool,
    pub context_overflow_threshold_bytes: usize,
    pub egress_default_deny: bool,
    pub egress_rate_limit_per_min: u32,
}

pub(crate) fn get_firewall_status(_state: &AppState) -> Result<FirewallStatusRow, String> {
    let summary = nexus_kernel::firewall::pattern_summary();
    Ok(FirewallStatusRow {
        status: "active".to_string(),
        mode: "fail-closed".to_string(),
        injection_pattern_count: summary.injection_count,
        pii_pattern_count: summary.pii_count,
        exfil_pattern_count: summary.exfil_count,
        sensitive_path_count: summary.sensitive_path_count,
        ssn_detection: summary.has_ssn_detection,
        passport_detection: summary.has_passport_detection,
        internal_ip_detection: summary.has_internal_ip_detection,
        context_overflow_threshold_bytes: summary.context_overflow_threshold_bytes,
        egress_default_deny: true,
        egress_rate_limit_per_min: nexus_kernel::firewall::DEFAULT_RATE_LIMIT_PER_MIN,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallPatternsRow {
    pub injection_patterns: Vec<String>,
    pub pii_patterns: Vec<String>,
    pub exfil_patterns: Vec<String>,
    pub sensitive_paths: Vec<String>,
    pub ssn_regex: String,
    pub passport_regex: String,
    pub internal_ip_regex: String,
}

pub(crate) fn get_firewall_patterns() -> Result<FirewallPatternsRow, String> {
    use nexus_kernel::firewall::patterns;
    Ok(FirewallPatternsRow {
        injection_patterns: patterns::INJECTION_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        pii_patterns: patterns::PII_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        exfil_patterns: patterns::EXFIL_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        sensitive_paths: patterns::SENSITIVE_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ssn_regex: patterns::SSN_PATTERN.to_string(),
        passport_regex: patterns::PASSPORT_PATTERN.to_string(),
        internal_ip_regex: patterns::INTERNAL_IP_PATTERN.to_string(),
    })
}

// ── Marketplace API ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceAgentRow {
    pub package_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub tags: Vec<String>,
    pub version: String,
    pub capabilities: Vec<String>,
    pub price_cents: i64,
    pub downloads: i64,
    pub rating: f64,
    pub review_count: i64,
    pub autonomy_level: Option<String>,
    pub fuel_budget: Option<u64>,
    pub schedule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceReviewRow {
    pub reviewer: String,
    pub stars: u8,
    pub comment: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceDetailRow {
    pub agent: MarketplaceAgentRow,
    pub reviews: Vec<MarketplaceReviewRow>,
    pub versions: Vec<MarketplaceVersionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceVersionRow {
    pub version: String,
    pub changelog: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreinstalledAgentRow {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub autonomy_level: u8,
    pub fuel_budget: u64,
    pub schedule: Option<String>,
    pub capabilities: Vec<String>,
    pub status: String,
    pub llm_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePublishResult {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub verdict: String,
    pub checks: Vec<MarketplaceCheckRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceCheckRow {
    pub name: String,
    pub passed: bool,
    pub findings: Vec<String>,
}

pub(crate) fn open_marketplace_registry(
) -> Result<nexus_marketplace::sqlite_registry::SqliteRegistry, String> {
    let db_path = nexus_marketplace::sqlite_registry::SqliteRegistry::default_db_path();
    nexus_marketplace::sqlite_registry::SqliteRegistry::open(&db_path)
        .map_err(|e| format!("Failed to open marketplace database: {e}"))
}

pub(crate) fn marketplace_manifest_profile(
    registry: &nexus_marketplace::sqlite_registry::SqliteRegistry,
    package_id: &str,
) -> Option<AgentManifest> {
    // Optional: returns None if bundle not found or manifest TOML is invalid
    registry
        .signed_bundle(package_id)
        .ok()
        .and_then(|bundle| parse_manifest(&bundle.manifest_toml).ok())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn marketplace_agent_row(
    registry: &nexus_marketplace::sqlite_registry::SqliteRegistry,
    package_id: String,
    name: String,
    description: String,
    author: String,
    tags: Vec<String>,
    version: String,
    capabilities: Vec<String>,
    price_cents: i64,
    downloads: i64,
    rating: f64,
    review_count: i64,
) -> MarketplaceAgentRow {
    let manifest = marketplace_manifest_profile(registry, &package_id);
    MarketplaceAgentRow {
        package_id,
        name,
        description,
        author,
        tags,
        version,
        capabilities,
        price_cents,
        downloads,
        rating,
        review_count,
        autonomy_level: manifest
            .as_ref()
            .and_then(|m| m.autonomy_level)
            .map(|level| format!("L{level}")),
        fuel_budget: manifest.as_ref().map(|m| m.fuel_budget),
        schedule: manifest.and_then(|m| m.schedule),
    }
}

pub(crate) fn get_preinstalled_agents(
    state: &AppState,
) -> Result<Vec<PreinstalledAgentRow>, String> {
    // Build a map of prebuilt agent names → their disk manifest llm_model.
    // This lets us override stale DB values with current disk values.
    let mut prebuilt_disk: HashMap<String, Option<String>> = HashMap::new();
    for path in list_prebuilt_manifest_paths() {
        if let Ok(json_str) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(name) = parsed.get("name").and_then(|v| v.as_str()) {
                    let llm_model = parsed
                        .get("llm_model")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    prebuilt_disk.insert(name.to_string(), llm_model);
                }
            }
        }
    }
    let prebuilt_names: HashSet<String> = prebuilt_disk.keys().cloned().collect();

    let rows = state
        .db
        .list_agents()
        .map_err(|e| format!("Failed to load agents: {e}"))?;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let mut runtime_by_name = HashMap::new();
    for status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            runtime_by_name.insert(
                handle.manifest.name.clone(),
                (status.id.to_string(), status.state.to_string()),
            );
        }
    }

    let mut preinstalled = Vec::new();
    for row in rows {
        let Ok(json_manifest) = serde_json::from_str::<JsonAgentManifest>(&row.manifest_json)
        else {
            continue;
        };
        if !prebuilt_names.contains(&json_manifest.manifest.name) {
            continue;
        }

        let agent_name = json_manifest.manifest.name.clone();
        let (agent_id, status) = runtime_by_name
            .get(&agent_name)
            .cloned()
            .unwrap_or_else(|| (row.id.clone(), "Idle".to_string()));

        // Prefer disk manifest's llm_model over stale DB value
        let resolved_llm = prebuilt_disk
            .get(&agent_name)
            .cloned()
            .unwrap_or(json_manifest.manifest.llm_model);

        preinstalled.push(PreinstalledAgentRow {
            agent_id,
            name: agent_name,
            description: json_manifest.description.unwrap_or_else(|| {
                extract_manifest_description(&row.manifest_json).unwrap_or_default()
            }),
            autonomy_level: json_manifest.manifest.autonomy_level.unwrap_or(0),
            fuel_budget: json_manifest.manifest.fuel_budget,
            schedule: json_manifest.manifest.schedule,
            capabilities: json_manifest.manifest.capabilities,
            status,
            llm_model: resolved_llm,
        });
    }

    preinstalled.sort_by(|left, right| {
        left.autonomy_level
            .cmp(&right.autonomy_level)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(preinstalled)
}

pub(crate) fn marketplace_search(query: &str) -> Result<Vec<MarketplaceAgentRow>, String> {
    let registry = open_marketplace_registry()?;
    let results = registry
        .search(query)
        .map_err(|e| format!("Search failed: {e}"))?;

    Ok(results
        .into_iter()
        .map(|r| {
            // Optional: detail lookup may fail for unlisted packages; row uses defaults
            let detail = registry.get_agent(&r.package_id).ok();
            marketplace_agent_row(
                &registry,
                r.package_id,
                r.name,
                r.description,
                r.author_id,
                r.tags,
                detail
                    .as_ref()
                    .map(|d| d.version.clone())
                    .unwrap_or_default(),
                detail
                    .as_ref()
                    .map(|d| d.capabilities.clone())
                    .unwrap_or_default(),
                detail.as_ref().map(|d| d.price_cents).unwrap_or(0),
                detail.as_ref().map(|d| d.downloads).unwrap_or(0),
                detail.as_ref().map(|d| d.rating).unwrap_or(0.0),
                detail.as_ref().map(|d| d.review_count).unwrap_or(0),
            )
        })
        .collect())
}

pub(crate) fn marketplace_install(package_id: &str) -> Result<MarketplaceAgentRow, String> {
    let registry = open_marketplace_registry()?;
    let bundle = registry
        .install(package_id)
        .map_err(|e| format!("Install failed: {e}"))?;
    let detail = registry
        .get_agent(package_id)
        .map_err(|e| format!("Failed to get agent detail: {e}"))?;

    Ok(marketplace_agent_row(
        &registry,
        bundle.package_id,
        bundle.metadata.name,
        bundle.metadata.description,
        bundle.metadata.author_id,
        bundle.metadata.tags,
        bundle.metadata.version,
        bundle.metadata.capabilities,
        detail.price_cents,
        detail.downloads,
        detail.rating,
        detail.review_count,
    ))
}

pub(crate) fn marketplace_info(agent_id: &str) -> Result<MarketplaceDetailRow, String> {
    let registry = open_marketplace_registry()?;
    let detail = registry
        .get_agent(agent_id)
        .map_err(|e| format!("Agent not found: {e}"))?;
    let reviews = registry.get_reviews(agent_id).unwrap_or_default();
    let versions = registry.version_history(agent_id).unwrap_or_default();

    Ok(MarketplaceDetailRow {
        agent: marketplace_agent_row(
            &registry,
            detail.package_id,
            detail.name,
            detail.description,
            detail.author,
            detail.tags,
            detail.version,
            detail.capabilities,
            detail.price_cents,
            detail.downloads,
            detail.rating,
            detail.review_count,
        ),
        reviews: reviews
            .into_iter()
            .map(|r| MarketplaceReviewRow {
                reviewer: r.reviewer,
                stars: r.stars,
                comment: r.comment,
                created_at: r.created_at,
            })
            .collect(),
        versions: versions
            .into_iter()
            .map(|v| MarketplaceVersionRow {
                version: v.version,
                changelog: v.changelog,
                created_at: v.created_at,
            })
            .collect(),
    })
}

pub(crate) fn marketplace_publish(bundle_json: &str) -> Result<MarketplacePublishResult, String> {
    use nexus_marketplace::package::SignedPackageBundle;
    use nexus_marketplace::verification_pipeline::{verify_bundle, Verdict};

    let bundle: SignedPackageBundle =
        serde_json::from_str(bundle_json).map_err(|e| format!("Invalid bundle format: {e}"))?;

    let verification = verify_bundle(&bundle);
    if verification.verdict == Verdict::Rejected {
        let findings: Vec<String> = verification
            .checks
            .iter()
            .filter(|c| !c.passed)
            .flat_map(|c| c.findings.clone())
            .collect();
        return Err(format!("Verification rejected: {}", findings.join("; ")));
    }

    let registry = open_marketplace_registry()?;
    registry
        .upsert_signed(&bundle)
        .map_err(|e| format!("Publish failed: {e}"))?;

    Ok(MarketplacePublishResult {
        package_id: bundle.package_id,
        name: bundle.metadata.name,
        version: bundle.metadata.version,
        verdict: format!("{:?}", verification.verdict),
        checks: verification
            .checks
            .iter()
            .map(|c| MarketplaceCheckRow {
                name: c.name.clone(),
                passed: c.passed,
                findings: c.findings.clone(),
            })
            .collect(),
    })
}

pub(crate) fn marketplace_my_agents(author: &str) -> Result<Vec<MarketplaceAgentRow>, String> {
    let registry = open_marketplace_registry()?;
    let results = registry
        .search(author)
        .map_err(|e| format!("Query failed: {e}"))?;

    Ok(results
        .into_iter()
        .filter(|r| r.author_id == author)
        .map(|r| {
            // Optional: detail lookup may fail for unlisted packages; row uses defaults
            let detail = registry.get_agent(&r.package_id).ok();
            marketplace_agent_row(
                &registry,
                r.package_id,
                r.name,
                r.description,
                r.author_id,
                r.tags,
                detail
                    .as_ref()
                    .map(|d| d.version.clone())
                    .unwrap_or_default(),
                detail
                    .as_ref()
                    .map(|d| d.capabilities.clone())
                    .unwrap_or_default(),
                detail.as_ref().map(|d| d.price_cents).unwrap_or(0),
                detail.as_ref().map(|d| d.downloads).unwrap_or(0),
                detail.as_ref().map(|d| d.rating).unwrap_or(0.0),
                detail.as_ref().map(|d| d.review_count).unwrap_or(0),
            )
        })
        .collect())
}
