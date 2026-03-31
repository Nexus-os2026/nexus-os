#![allow(unexpected_cfgs)]
#![allow(unused_imports)]
mod commands;
use base64::Engine;
use chrono::TimeZone;
use nexus_adaptation::evolution::{EvolutionConfig, EvolutionEngine, MutationType, Strategy};
use nexus_conductor::types::UserRequest;
use nexus_conductor::Conductor;
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
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
use tokio::sync::Notify;
use uuid::Uuid;

// Enterprise crate imports
use nexus_auth::SessionManager;
use nexus_integrations::IntegrationRouter;
use nexus_tenancy::WorkspaceManager;

// Flash inference imports
use nexus_flash_infer::SessionManager as FlashSessionManager;

// Capability measurement imports
use nexus_capability_measurement::tauri_commands::MeasurementState;

// Governance oracle imports
// BudgetSummary and OracleStatusSummary moved to commands::crate_bridges

// Predictive router imports
use nexus_predictive_router::tauri_commands::RouterState;

// Browser agent imports
use nexus_browser_agent::BrowserState;

// Token economy imports
use nexus_token_economy::tauri_commands as token_cmds;

// Computer control imports
use nexus_computer_control::tauri_commands as cc_cmds;

// World simulation imports
use nexus_world_simulation::tauri_commands as sim_cmds;

// Perception imports
use nexus_perception::tauri_commands as perception_cmds;

// Agent memory imports
use nexus_agent_memory::tauri_commands as memory_cmds;

// External tools imports
use nexus_external_tools::tauri_commands as tools_cmds;

// Collaboration protocol imports
use nexus_collab_protocol::tauri_commands as collab_cmds;

// Software factory imports
use nexus_software_factory::tauri_commands as factory_cmds;

// MCP imports
use nexus_mcp::tauri_commands as mcp2_cmds;

// A2A crate imports
use nexus_a2a::tauri_commands as a2a_crate_cmds;

// Migration tool imports
use nexus_migrate::tauri_commands as migrate_cmds;

// Memory kernel imports
use nexus_memory::tauri_commands as mk_cmds;

struct GatewayHivemindLlm;

impl nexus_kernel::cognitive::HivemindLlm for GatewayHivemindLlm {
    fn decompose(
        &self,
        prompt: &str,
    ) -> std::result::Result<String, nexus_kernel::errors::AgentError> {
        nexus_kernel::cognitive::PlannerLlm::plan_query(&GatewayPlannerLlm, prompt)
    }

    fn merge(&self, prompt: &str) -> std::result::Result<String, nexus_kernel::errors::AgentError> {
        nexus_kernel::cognitive::PlannerLlm::plan_query(&GatewayPlannerLlm, prompt)
    }
}

#[derive(Clone, Debug)]
struct AgentLlmRoute {
    model: String,
}

thread_local! {
    static ACTIVE_AGENT_LLM_ROUTE: RefCell<Option<AgentLlmRoute>> = const { RefCell::new(None) };
    /// Cached Flash provider for the current agent's cognitive loop.
    /// Set by `with_agent_llm_route` when a `flash:*` route is active.
    static ACTIVE_FLASH_PROVIDER: RefCell<Option<std::sync::Arc<nexus_connectors_llm::providers::FlashProvider>>> = const { RefCell::new(None) };
}

fn normalize_agent_config_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn agent_lookup_keys(state: &AppState, agent_id: &str) -> Vec<String> {
    let mut keys = vec![agent_id.to_string()];
    let maybe_name = {
        // Optional: agent_id may not be a valid UUID (e.g. name-based lookup)
        let parsed_id = Uuid::parse_str(agent_id).ok();
        if let Some(parsed_id) = parsed_id {
            let meta = state.meta.lock().unwrap_or_else(|p| p.into_inner());
            meta.get(&parsed_id).map(|entry| entry.name.clone())
        } else {
            None
        }
    };

    if let Some(name) = maybe_name {
        let normalized = normalize_agent_config_key(&name);
        if !keys.iter().any(|candidate| candidate == &name) {
            keys.push(name.clone());
        }
        if !keys.iter().any(|candidate| candidate == &normalized) {
            keys.push(normalized);
        }
    }

    let normalized_id = normalize_agent_config_key(agent_id);
    if !keys.iter().any(|candidate| candidate == &normalized_id) {
        keys.push(normalized_id);
    }
    keys
}

fn route_from_model_mapping(value: &Value) -> Option<String> {
    if let (Some(provider), Some(model)) = (
        value.get("provider").and_then(|entry| entry.as_str()),
        value.get("model").and_then(|entry| entry.as_str()),
    ) {
        return Some(format!("{provider}/{model}"));
    }

    for key in [
        "planning",
        "plan",
        "default",
        "acting",
        "action",
        "reflection",
        "reflect",
        "observe",
    ] {
        if let Some(route) = value.get(key).and_then(route_from_model_mapping) {
            return Some(route);
        }
    }

    value.as_object().and_then(|entries| {
        entries
            .values()
            .find_map(route_from_model_mapping)
            .filter(|route| !route.trim().is_empty())
    })
}

fn resolve_agent_llm_route(state: &AppState, agent_id: &str) -> Option<AgentLlmRoute> {
    // Optional: returns None if config file cannot be loaded
    let config = load_config().ok()?;
    let _agent_short = &agent_id[..agent_id.len().min(8)];

    // 1. Check agent memory for explicit model mapping (user override)
    if let Ok(memories) = state.db.load_memories(agent_id, Some("model_mapping"), 10) {
        for row in memories {
            if let Ok(parsed) = serde_json::from_str::<Value>(&row.value_json) {
                if let Some(model) = route_from_model_mapping(&parsed) {
                    return Some(AgentLlmRoute { model });
                }
            }
        }
    }

    // 2. Check config-level agent assignments
    for key in agent_lookup_keys(state, agent_id) {
        if let Some(agent_cfg) = config.agents.get(&key) {
            if !agent_cfg.model.trim().is_empty() && agent_cfg.model.trim() != "auto" {
                return Some(AgentLlmRoute {
                    model: agent_cfg.model.clone(),
                });
            }
        }
        if let Some(assignment) = config.agent_llm_assignments.get(&key) {
            let pid = assignment.provider_id.trim();
            if !pid.is_empty() && pid != "auto" {
                // If provider is "flash" or "flash/model", resolve to an active
                // Flash session so the downstream code finds the loaded provider.
                if pid == "flash" || pid.starts_with("flash/") {
                    let cache = state
                        .flash_providers
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    eprintln!(
                        "[resolve-route] agent={} assignment pid='{}', flash cache has {} entries",
                        _agent_short,
                        pid,
                        cache.len()
                    );
                    if let Some((session_id, _)) = cache.iter().next() {
                        eprintln!("[resolve-route] resolved to flash:{session_id}");
                        return Some(AgentLlmRoute {
                            model: format!("flash:{session_id}"),
                        });
                    }
                    // Cache empty — return the original "flash/model" route
                    eprintln!("[resolve-route] flash cache empty, returning raw pid '{pid}'");
                }
                return Some(AgentLlmRoute {
                    model: assignment.provider_id.clone(),
                });
            }
        }
    }

    // 2.5. Check agent manifest llm_model field
    //   Supports: "flash", "flash:fast", "flash:balanced", "auto", or "provider/model"
    if let Ok(agent_uuid) = uuid::Uuid::parse_str(agent_id) {
        let sup = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(handle) = sup.get_agent(agent_uuid) {
            if let Some(ref llm_model) = handle.manifest.llm_model {
                let model = llm_model.trim();
                if !model.is_empty() && model != "auto" {
                    // "flash" or "flash:*" — resolve to active Flash session
                    if model == "flash" || model.starts_with("flash:") {
                        let cache = state
                            .flash_providers
                            .lock()
                            .unwrap_or_else(|p| p.into_inner());
                        if let Some((session_id, _)) = cache.iter().next() {
                            return Some(AgentLlmRoute {
                                model: format!("flash:{session_id}"),
                            });
                        }
                        // Flash requested but no session — fall through to auto
                    } else {
                        return Some(AgentLlmRoute {
                            model: model.to_string(),
                        });
                    }
                }
            }
        }
    }

    // 3. Smart auto-routing: pick the best available model.
    //    Priority: Flash Inference (local GGUF) → Ollama → Cloud providers.
    //    Returns None if nothing is available (GatewayPlannerLlm handles fallback).
    auto_select_best_model(state, &config)
}

/// Smart model selection: pick the best available LLM provider and model.
///
/// Priority order:
///   1. Flash Inference sessions (local GGUF models — fastest, free, private)
///   2. Ollama local models (free, private)
///   3. Cloud providers with API keys configured (paid, external)
///
/// Within each tier, prefers larger models for better reasoning quality.
fn auto_select_best_model(
    state: &AppState,
    config: &nexus_kernel::config::NexusConfig,
) -> Option<AgentLlmRoute> {
    // --- Tier 0: Check Flash Inference sessions (best: local, fast, free, smart) ---
    // If a Flash Inference model is loaded, use it — these are larger/smarter models
    // (Qwen 35B, Gemma 27B) that the user explicitly loaded via the Flash Inference UI.
    {
        let cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some((session_id, _provider)) = cache.iter().next() {
            eprintln!(
                "[auto-select] using Flash Inference session {session_id} (local GGUF, free)"
            );
            return Some(AgentLlmRoute {
                model: format!("flash:{session_id}"),
            });
        }
    }

    // --- Tier 1: Check for FLASH_MODEL_PATH env var (auto-load Flash model) ---
    if let Ok(model_path) = std::env::var("FLASH_MODEL_PATH") {
        if std::path::Path::new(&model_path).exists() {
            eprintln!("[auto-select] using Flash Inference from FLASH_MODEL_PATH={model_path}");
            return Some(AgentLlmRoute {
                model: format!("flash/{model_path}"),
            });
        }
    }

    // --- Tier 2: Check Ollama for available models ---
    // Fast TCP probe first — if Ollama isn't running, skip the slow list_models() call
    let prov_config = build_provider_config(config);
    let ollama = OllamaProvider::from_env();
    if ollama.health_check().is_ok() {
        if let Ok(models) = ollama.list_models() {
            if !models.is_empty() {
                // Prefer larger models for agent reasoning (35b > 9b > 4b)
                let best = models
                    .iter()
                    .max_by_key(|m| {
                        let name = m.name.to_lowercase();
                        if name.contains("35b") || name.contains("32b") || name.contains("70b") {
                            3
                        } else if name.contains("14b")
                            || name.contains("13b")
                            || name.contains("9b")
                            || name.contains("coder")
                        {
                            2 // Prefer medium and coder models
                        } else {
                            1
                        }
                    })
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| models[0].name.clone());

                return Some(AgentLlmRoute { model: best });
            }
        }
    } // close health_check guard

    // --- Tier 3: Check cloud providers with API keys ---
    if select_provider(&prov_config).is_ok() {
        return None;
    }

    None
}

fn with_agent_llm_route<T>(state: &AppState, agent_id: &str, op: impl FnOnce() -> T) -> T {
    let route = resolve_agent_llm_route(state, agent_id);

    // If the route points to Flash, resolve the cached provider now
    // so GatewayPlannerLlm can use it without needing AppState access.
    // Handles both "flash:{session_id}" and "flash/{model_name}" formats.
    let flash_prov = route.as_ref().and_then(|r| {
        let wants_flash =
            r.model.starts_with("flash:") || r.model.starts_with("flash/") || r.model == "flash";
        if !wants_flash {
            return None;
        }

        // Try the provider cache first
        let cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let cache_keys: Vec<_> = cache.keys().cloned().collect();
        eprintln!(
            "[flash-route] route='{}', cache has {} entries: {:?}",
            r.model,
            cache.len(),
            cache_keys
        );

        // Try exact session ID match for "flash:{id}" routes
        if let Some(session_id) = r.model.strip_prefix("flash:") {
            if let Some(prov) = cache.get(session_id) {
                eprintln!("[flash-route] found provider by session ID '{session_id}'");
                return Some(prov.clone());
            }
        }
        // For any flash route, use whatever provider is cached
        if let Some((key, prov)) = cache.iter().next() {
            eprintln!("[flash-route] using cached provider '{key}'");
            return Some(prov.clone());
        }
        drop(cache);

        eprintln!(
            "[flash-route] provider cache empty for route '{}' — model not loaded",
            r.model
        );
        None
    });

    ACTIVE_AGENT_LLM_ROUTE.with(|route_slot| {
        ACTIVE_FLASH_PROVIDER.with(|flash_slot| {
            let prev_route = route_slot.replace(route);
            let prev_flash = flash_slot.replace(flash_prov);
            let output = op();
            route_slot.replace(prev_route);
            flash_slot.replace(prev_flash);
            output
        })
    })
}

#[derive(Clone)]
struct TauriProviderStub {
    name: String,
}

impl nexus_kernel::cognitive::LlmProvider for TauriProviderStub {
    fn name(&self) -> &str {
        &self.name
    }
}

fn build_provider_registry() -> HashMap<String, Arc<dyn nexus_kernel::cognitive::LlmProvider>> {
    [
        "anthropic",
        "cohere",
        "fireworks",
        "gemini",
        "groq",
        "mistral",
        "mock",
        "ollama",
        "openai",
        "openrouter",
        "perplexity",
        "together",
    ]
    .into_iter()
    .map(|name| {
        (
            name.to_string(),
            Arc::new(TauriProviderStub {
                name: name.to_string(),
            }) as Arc<dyn nexus_kernel::cognitive::LlmProvider>,
        )
    })
    .collect()
}

fn fuel_ledger_row_from_report(
    agent_id: &str,
    report: &nexus_kernel::fuel_hardening::FuelAuditReport,
) -> nexus_persistence::FuelLedgerRow {
    let now = chrono::Utc::now().to_rfc3339();
    nexus_persistence::FuelLedgerRow {
        agent_id: agent_id.to_string(),
        budget_total: report.cap_units as f64,
        budget_consumed: report.spent_units as f64,
        period_start: report.period.0.clone(),
        period_end: now.clone(),
        anomaly_count: report.anomalies.len() as i64,
        ledger_json: serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string()),
        updated_at: now,
    }
}

fn load_fuel_report_from_row(
    row: &nexus_persistence::FuelLedgerRow,
) -> Option<nexus_kernel::fuel_hardening::FuelAuditReport> {
    // Optional: ledger JSON may be from an older schema; fall back to constructing a report
    serde_json::from_str::<nexus_kernel::fuel_hardening::FuelAuditReport>(&row.ledger_json)
        .ok()
        .or_else(|| {
            // Optional: returns None if agent_id is not a valid UUID
            let agent_id = Uuid::parse_str(&row.agent_id).ok()?;
            Some(nexus_kernel::fuel_hardening::FuelAuditReport {
                agent_id,
                period: nexus_kernel::fuel_hardening::BudgetPeriodId::new(&row.period_start),
                cap_units: row.budget_total.max(0.0) as u64,
                spent_units: row.budget_consumed.max(0.0) as u64,
                anomalies: Vec::new(),
                halts: 0,
                model_breakdown: Vec::new(),
            })
        })
}

/// Bridges `NexusDatabase` (persistence) to the kernel `StrategyStore` trait.
struct DbStrategyStore {
    db: Arc<NexusDatabase>,
}

impl nexus_kernel::cognitive::StrategyStore for DbStrategyStore {
    fn upsert_strategy_score(
        &self,
        agent_id: &str,
        strategy_hash: &str,
        goal_type: &str,
        success: bool,
        fuel: f64,
        duration: f64,
    ) -> std::result::Result<(), String> {
        self.db
            .upsert_strategy_score(agent_id, strategy_hash, goal_type, success, fuel, duration)
            .map_err(|e| e.to_string())
    }

    fn load_top_strategies(
        &self,
        agent_id: &str,
        goal_type: &str,
        limit: usize,
    ) -> std::result::Result<Vec<nexus_kernel::cognitive::StrategyScore>, String> {
        let rows = self
            .db
            .load_top_strategies(agent_id, goal_type, limit)
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| nexus_kernel::cognitive::StrategyScore {
                agent_id: r.agent_id,
                strategy_hash: r.strategy_hash,
                goal_type: r.goal_type,
                uses: r.uses,
                successes: r.successes,
                total_fuel: r.total_fuel,
                total_duration_secs: r.total_duration_secs,
                composite_score: r.composite_score,
            })
            .collect())
    }

    fn load_strategy_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> std::result::Result<Vec<nexus_kernel::cognitive::StrategyScore>, String> {
        let rows = self
            .db
            .load_strategy_history(agent_id, limit)
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| nexus_kernel::cognitive::StrategyScore {
                agent_id: r.agent_id,
                strategy_hash: r.strategy_hash,
                goal_type: r.goal_type,
                uses: r.uses,
                successes: r.successes,
                total_fuel: r.total_fuel,
                total_duration_secs: r.total_duration_secs,
                composite_score: r.composite_score,
            })
            .collect())
    }
}

/// Bridges `NexusDatabase` to the kernel `MemoryStore` trait.
pub struct DbMemoryStore {
    pub db: Arc<NexusDatabase>,
}

impl nexus_kernel::cognitive::MemoryStore for DbMemoryStore {
    fn save_memory(
        &self,
        agent_id: &str,
        memory_type: &str,
        key: &str,
        value_json: &str,
    ) -> std::result::Result<(), String> {
        StateStore::save_memory(&*self.db, agent_id, memory_type, key, value_json)
            .map_err(|e| e.to_string())
    }

    fn load_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: usize,
    ) -> std::result::Result<Vec<nexus_kernel::cognitive::MemoryEntry>, String> {
        let rows = StateStore::load_memories(&*self.db, agent_id, memory_type, limit)
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| nexus_kernel::cognitive::MemoryEntry {
                id: r.id,
                agent_id: r.agent_id,
                memory_type: r.memory_type,
                key: r.key,
                value_json: r.value_json,
                relevance_score: r.relevance_score,
                access_count: r.access_count,
                created_at: r.created_at,
                last_accessed: r.last_accessed,
            })
            .collect())
    }

    fn touch_memory(&self, id: i64) -> std::result::Result<(), String> {
        StateStore::touch_memory(&*self.db, id).map_err(|e| e.to_string())
    }

    fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> std::result::Result<(), String> {
        StateStore::decay_memories(&*self.db, agent_id, decay_factor).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub autonomy_level: Option<u8>,
    pub fuel_remaining: u64,
    pub fuel_budget: u64,
    pub last_action: String,
    pub capabilities: Vec<String>,
    pub sandbox_runtime: String,
    pub did: Option<String>,
    #[serde(default)]
    pub description: String,
}

/// Lightweight event emitted when agent status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusEvent {
    pub agent_id: String,
    pub status: String,
    pub fuel_remaining: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRow {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub hash: String,
    pub previous_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
    pub token_count: u32,
    pub cost: f64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrayStatus {
    pub running_agents: usize,
    pub menu_items: Vec<String>,
}

#[derive(Debug, Clone)]
struct AgentMeta {
    name: String,
    last_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VoiceRuntimeState {
    pub wake_word_enabled: bool,
    pub push_to_talk_enabled: bool,
    pub overlay_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerActionSessionState {
    pub session_id: String,
    pub description: String,
    pub running: bool,
}

/// Tracks the Python voice server subprocess.
#[derive(Default)]
struct VoiceProcess {
    child: Option<std::process::Child>,
    running: bool,
}

#[derive(Clone)]
struct BlockedConsentWait {
    consent_id: String,
    notify: Arc<Notify>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationPersonalityView {
    pub openness: f64,
    pub conscientiousness: f64,
    pub extraversion: f64,
    pub agreeableness: f64,
    pub neuroticism: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationMemoryView {
    pub event: String,
    pub timestamp: u64,
    pub emotional_impact: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationPersonaView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub personality: SimulationPersonalityView,
    pub beliefs: HashMap<String, f64>,
    pub memories: Vec<SimulationMemoryView>,
    pub relationships: HashMap<String, f64>,
    pub influence_score: f64,
    pub last_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationStatusView {
    pub world_id: String,
    pub name: String,
    pub status: String,
    pub tick_count: u64,
    pub persona_count: usize,
    pub max_ticks: u64,
    pub tick_interval_ms: u64,
    pub fuel_consumed: f64,
    pub estimated_fuel: u64,
    pub report_available: bool,
    pub variables: BTreeMap<String, String>,
    pub personas: Vec<SimulationPersonaView>,
}

#[derive(Debug, Clone)]
struct SimulationHandle {
    control: SimulationControl,
    max_ticks: u64,
}

#[derive(Default)]
struct SimulationManager {
    active: Mutex<HashMap<String, SimulationHandle>>,
}

impl SimulationManager {
    fn insert(&self, world_id: String, handle: SimulationHandle) {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(world_id, handle);
    }

    fn get(&self, world_id: &str) -> Option<SimulationHandle> {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(world_id)
            .cloned()
    }

    fn remove(&self, world_id: &str) {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(world_id);
    }
}

// ── Chat Pipeline: Complexity + Routing ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComplexityLevel {
    SimpleQuestion,
    SmallTask,
    ComplexProject,
}

/// Tracks conversation-level state for the autopilot / project builder flow.
#[derive(Debug, Clone, Default)]
struct ChatConversationState {
    /// The last project plan shown to the user, keyed by conversation-like session.
    last_project_plan: Option<String>,
    /// Whether we're waiting for user to approve a project plan.
    awaiting_approval: bool,
    /// Active autopilot project description (if running).
    active_project: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub supervisor: Arc<Mutex<Supervisor>>,
    pub audit: Arc<Mutex<AuditTrail>>,
    meta: Arc<Mutex<HashMap<AgentId, AgentMeta>>>,
    voice: Arc<Mutex<VoiceRuntimeState>>,
    identity_mgr: Arc<Mutex<nexus_kernel::identity::IdentityManager>>,
    browser: Arc<Mutex<BrowserManager>>,
    research: Arc<Mutex<ResearchManager>>,
    build: Arc<Mutex<BuildManager>>,
    learning: Arc<Mutex<LearningManager>>,
    rag: Arc<Mutex<RagPipeline>>,
    redaction_engine: Arc<Mutex<RedactionEngine>>,
    model_registry: Arc<Mutex<ModelRegistry>>,
    nexus_link: Arc<Mutex<NexusLink>>,
    evolution: Arc<Mutex<EvolutionEngine>>,
    mcp_host: Arc<Mutex<McpHostManager>>,
    ghost_protocol: Arc<Mutex<GhostProtocol>>,
    voice_process: Arc<Mutex<VoiceProcess>>,
    factory: Arc<Mutex<FactoryPipeline>>,
    computer_control: Arc<Mutex<ComputerControlEngine>>,
    neural_bridge: Arc<Mutex<NeuralBridge>>,
    economic_engine: Arc<Mutex<EconomicEngine>>,
    agent_memory: Arc<Mutex<AgentMemory>>,
    tracing_engine: Arc<Mutex<TracingEngine>>,
    payment_engine: Arc<Mutex<PaymentEngine>>,
    whisper: Arc<Mutex<WhisperTranscriber>>,
    replay_recorder: Arc<Mutex<nexus_kernel::replay::recorder::ReplayRecorder>>,
    reputation_registry: Arc<Mutex<nexus_kernel::reputation::ReputationRegistry>>,
    pub db: Arc<NexusDatabase>,
    pub cognitive_runtime: Arc<nexus_kernel::cognitive::CognitiveRuntime>,
    blocked_consent_waits: Arc<Mutex<HashMap<String, BlockedConsentWait>>>,
    computer_action_cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    hivemind: Arc<nexus_kernel::cognitive::HivemindCoordinator>,
    message_gateway: Arc<Mutex<MessageGateway>>,
    pub evolution_tracker: Arc<nexus_kernel::cognitive::EvolutionTracker>,
    auto_evolution: Arc<AutoEvolutionManager>,
    agent_scheduler: Arc<nexus_kernel::cognitive::AgentScheduler>,
    simulation_manager: Arc<SimulationManager>,
    consciousness: Arc<Mutex<nexus_kernel::consciousness::ConsciousnessEngine>>,
    dream_engine: Arc<Mutex<nexus_kernel::dreams::DreamEngine>>,
    temporal_engine: Arc<Mutex<nexus_kernel::temporal::TemporalEngine>>,
    immune_scan_results: Arc<Mutex<Vec<nexus_kernel::immune::ThreatEvent>>>,
    immune_last_scan: Arc<Mutex<u64>>,
    self_rewrite_patches: Arc<Mutex<Vec<nexus_kernel::self_rewrite::Patch>>>,
    temporal_checkpoints: Arc<Mutex<nexus_kernel::temporal::TemporalCheckpointManager>>,
    time_dilator: Arc<Mutex<nexus_kernel::temporal::TimeDilator>>,
    self_improving_os: Arc<Mutex<nexus_kernel::self_improve::SelfImprovingOS>>,
    pub self_improve_state: Arc<Mutex<commands::self_improvement::SelfImproveState>>,
    screenshot_cloner: Arc<Mutex<nexus_kernel::autopilot::screenshot_clone::ScreenshotCloner>>,
    voice_project: Arc<Mutex<nexus_kernel::autopilot::voice_project::VoiceProjectBuilder>>,
    stress_simulator: Arc<Mutex<nexus_kernel::autopilot::stress_test::StressSimulator>>,
    live_deployer: Arc<Mutex<nexus_kernel::autopilot::deploy::LiveDeployer>>,
    live_evolver: Arc<Mutex<nexus_kernel::autopilot::live_evolution::LiveAppEvolver>>,
    freelance_engine: Arc<Mutex<nexus_kernel::economy::freelancer::FreelanceEngine>>,
    conversational_builder: Arc<Mutex<ConversationalBuilder>>,
    live_previews: Arc<Mutex<HashMap<String, LivePreviewEngine>>>,
    remix_engine: Arc<Mutex<RemixEngine>>,
    problem_solver: Arc<Mutex<ProblemSolver>>,
    marketplace_publisher: Arc<Mutex<MarketplacePublisher>>,
    teach_modes: Arc<Mutex<HashMap<String, TeachMode>>>,
    routing_learner: Arc<Mutex<nexus_kernel::self_improve::RoutingLearner>>,
    startup_instant: std::time::Instant,
    rate_limiter: nexus_kernel::rate_limit::NexusRateLimiter,
    api_config: nexus_kernel::rate_limit::ApiHardeningConfig,
    chat_conversation_state: Arc<Mutex<ChatConversationState>>,
    // Enterprise crate state
    session_manager: Arc<SessionManager>,
    workspace_manager: Arc<Mutex<WorkspaceManager>>,
    integration_router: Arc<IntegrationRouter>,
    metering_store: Arc<Mutex<nexus_metering::MeteringStore>>,
    metering_rates: Arc<nexus_metering::CostRates>,
    telemetry_config: Arc<Mutex<nexus_telemetry::TelemetryConfig>>,
    a2a_client: Arc<Mutex<A2aClient>>,
    schedule_store: Arc<nexus_kernel::scheduler::ScheduleStore>,
    schedule_runner: Arc<nexus_kernel::scheduler::ScheduleRunner>,
    flash_session_manager: Arc<FlashSessionManager>,
    /// Cached FlashProvider instances per session ID — avoids reloading model on every call.
    /// Wrapped in Arc so the provider (and its loaded model handle) can be shared with
    /// GovernedLlmGateway without transferring ownership.
    flash_providers:
        Arc<Mutex<HashMap<String, std::sync::Arc<nexus_connectors_llm::providers::FlashProvider>>>>,
    /// Speculative decoding engine — pairs a fast draft model with the loaded target.
    flash_speculative: Arc<Mutex<Option<nexus_flash_infer::SpeculativeEngine>>>,
    adversarial_arena:
        Arc<Mutex<nexus_kernel::cognitive::algorithms::adversarial::AdversarialArena>>,
    capability_measurement: Arc<MeasurementState>,
    predictive_router: Arc<RouterState>,
    browser_agent: Arc<BrowserState>,
    token_economy: Arc<token_cmds::EconomyState>,
    governed_control: Arc<cc_cmds::ControlState>,
    world_simulation: Arc<sim_cmds::SimulationState>,
    perception: Arc<perception_cmds::PerceptionState>,
    persistent_memory: Arc<memory_cmds::MemoryState>,
    external_tools: Arc<tools_cmds::ToolState>,
    collab_protocol: Arc<collab_cmds::CollabState>,
    software_factory: Arc<factory_cmds::FactoryState>,
    mcp_standalone: Arc<mcp2_cmds::McpState>,
    a2a_crate: Arc<a2a_crate_cmds::A2aState>,
    memory_kernel: Arc<mk_cmds::MemoryKernelState>,
    governance_ruleset: Arc<Mutex<nexus_governance_engine::GovernanceRuleset>>,
    governance_audit_log: Arc<Mutex<nexus_governance_engine::DecisionAuditLog>>,
    governance_evolution: Arc<Mutex<nexus_governance_evolution::GovernanceEvolution>>,
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    app_handle: Arc<Mutex<Option<tauri::AppHandle<tauri::Wry>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        #[cfg(not(test))]
        maybe_cleanup_legacy_agent_db();

        let supervisor = Arc::new(Mutex::new(Supervisor::new()));
        let db = Arc::new(
            NexusDatabase::open(&NexusDatabase::default_db_path()).unwrap_or_else(|e| {
                eprintln!("persistence: falling back to in-memory DB: {e}");
                NexusDatabase::in_memory().unwrap_or_else(|e2| {
                    eprintln!("╔══════════════════════════════════════════╗");
                    eprintln!("║  FATAL: Nexus OS failed to start         ║");
                    eprintln!("╠══════════════════════════════════════════╣");
                    eprintln!("║  Error: {e2}");
                    eprintln!("║                                          ║");
                    eprintln!("║  Please check:                           ║");
                    eprintln!("║  1. Config file exists and is valid      ║");
                    eprintln!("║  2. Required ports are available         ║");
                    eprintln!("║  3. Sufficient disk space and memory     ║");
                    eprintln!("╚══════════════════════════════════════════╝");
                    std::process::exit(1);
                })
            }),
        );
        let evolution_tracker = Arc::new(nexus_kernel::cognitive::EvolutionTracker::new(Box::new(
            DbStrategyStore { db: db.clone() },
        )));
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let cognitive_runtime = Arc::new(
            nexus_kernel::cognitive::CognitiveRuntime::with_provider_registry(
                supervisor.clone(),
                nexus_kernel::cognitive::LoopConfig::default(),
                Arc::new(nexus_kernel::cognitive::NoOpEmitter),
                build_provider_registry(),
            ),
        );
        let agent_scheduler = Arc::new(nexus_kernel::cognitive::AgentScheduler::new(
            cognitive_runtime.clone(),
            audit.clone(),
        ));
        let audit_for_runner = audit.clone();
        let supervisor_for_runner = supervisor.clone();
        let state = Self {
            supervisor: supervisor.clone(),
            audit,
            meta: Arc::new(Mutex::new(HashMap::new())),
            voice: Arc::new(Mutex::new(VoiceRuntimeState {
                wake_word_enabled: true,
                push_to_talk_enabled: true,
                overlay_visible: false,
            })),
            identity_mgr: Arc::new(Mutex::new(
                nexus_kernel::identity::IdentityManager::in_memory(),
            )),
            browser: Arc::new(Mutex::new(BrowserManager::new())),
            research: Arc::new(Mutex::new(ResearchManager::new())),
            build: Arc::new(Mutex::new(BuildManager::new())),
            learning: Arc::new(Mutex::new(LearningManager::new())),
            rag: Arc::new(Mutex::new(RagPipeline::new(RagConfig::default()))),
            redaction_engine: Arc::new(Mutex::new(RedactionEngine::default())),
            model_registry: Arc::new(Mutex::new(ModelRegistry::default_dir())),
            nexus_link: Arc::new(Mutex::new({
                let hostname = std::env::var("HOSTNAME")
                    .or_else(|_| std::env::var("COMPUTERNAME"))
                    .unwrap_or_else(|_| "nexus-device".to_string());
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let models_dir = std::path::Path::new(&home).join(".nexus").join("models");
                NexusLink::new(&hostname, &models_dir.display().to_string())
            })),
            evolution: Arc::new(Mutex::new(EvolutionEngine::new(EvolutionConfig::default()))),
            mcp_host: Arc::new(Mutex::new(McpHostManager::new())),
            ghost_protocol: Arc::new(Mutex::new(GhostProtocol::new(GhostConfig::default()))),
            voice_process: Arc::new(Mutex::new(VoiceProcess::default())),
            factory: Arc::new(Mutex::new(FactoryPipeline::new())),
            computer_control: Arc::new(Mutex::new(ComputerControlEngine::new())),
            neural_bridge: Arc::new(Mutex::new(NeuralBridge::new(NeuralBridgeConfig::default()))),
            economic_engine: Arc::new(Mutex::new(EconomicEngine::new(EconomicConfig::default()))),
            agent_memory: Arc::new(Mutex::new(AgentMemory::new(MemoryConfig::default()))),
            tracing_engine: Arc::new(Mutex::new(TracingEngine::new(1000))),
            payment_engine: Arc::new(Mutex::new(PaymentEngine::new(RevenueSplit::default()))),
            whisper: Arc::new(Mutex::new(WhisperTranscriber::new())),
            replay_recorder: Arc::new(Mutex::new(
                nexus_kernel::replay::recorder::ReplayRecorder::new(500),
            )),
            reputation_registry: Arc::new(Mutex::new(
                nexus_kernel::reputation::ReputationRegistry::new(),
            )),
            db,
            cognitive_runtime: cognitive_runtime.clone(),
            blocked_consent_waits: Arc::new(Mutex::new(HashMap::new())),
            computer_action_cancellations: Arc::new(Mutex::new(HashMap::new())),
            hivemind: Arc::new(nexus_kernel::cognitive::HivemindCoordinator::new(
                Box::new(GatewayHivemindLlm),
                Arc::new(nexus_kernel::cognitive::hivemind::NoOpHivemindEmitter),
                Arc::new(Mutex::new(AuditTrail::new())),
            )),
            message_gateway: Arc::new(Mutex::new({
                let mut gw = MessageGateway::new();
                // Register enabled platforms from environment
                let enabled = std::env::var("NEXUS_MESSAGING_ENABLED").unwrap_or_default();
                for platform_name in enabled
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    match platform_name {
                        "telegram" => {
                            gw.register_platform(Box::new(
                                nexus_connectors_messaging::telegram::TelegramAdapter::new(),
                            ));
                        }
                        "discord" => {
                            gw.register_platform(Box::new(
                                nexus_connectors_messaging::discord::DiscordAdapter::new(),
                            ));
                        }
                        "slack" => {
                            gw.register_platform(Box::new(
                                nexus_connectors_messaging::slack::SlackAdapter::new(),
                            ));
                        }
                        "whatsapp" => {
                            gw.register_platform(Box::new(
                                nexus_connectors_messaging::whatsapp::WhatsAppAdapter::new(
                                    nexus_connectors_messaging::whatsapp::WhatsAppQualityTier::Medium,
                                ),
                            ));
                        }
                        other => {
                            eprintln!("messaging: unknown platform '{other}', skipping");
                        }
                    }
                }
                gw
            })),
            evolution_tracker,
            auto_evolution: Arc::new(AutoEvolutionManager::new()),
            agent_scheduler,
            simulation_manager: Arc::new(SimulationManager::default()),
            consciousness: Arc::new(Mutex::new(
                nexus_kernel::consciousness::ConsciousnessEngine::new(),
            )),
            dream_engine: Arc::new(Mutex::new(nexus_kernel::dreams::DreamEngine::new(
                nexus_kernel::dreams::DreamScheduler::new(),
            ))),
            temporal_engine: Arc::new(
                Mutex::new(nexus_kernel::temporal::TemporalEngine::default()),
            ),
            immune_scan_results: Arc::new(Mutex::new(Vec::new())),
            immune_last_scan: Arc::new(Mutex::new(0)),
            self_rewrite_patches: Arc::new(Mutex::new(Vec::new())),
            temporal_checkpoints: Arc::new(Mutex::new(
                nexus_kernel::temporal::TemporalCheckpointManager::default(),
            )),
            time_dilator: Arc::new(Mutex::new(nexus_kernel::temporal::TimeDilator::default())),
            self_improving_os: Arc::new(Mutex::new(
                nexus_kernel::self_improve::SelfImprovingOS::new(),
            )),
            self_improve_state: Arc::new(Mutex::new(
                commands::self_improvement::SelfImproveState::default(),
            )),
            screenshot_cloner: Arc::new(Mutex::new(
                nexus_kernel::autopilot::screenshot_clone::ScreenshotCloner::default(),
            )),
            voice_project: Arc::new(Mutex::new(
                nexus_kernel::autopilot::voice_project::VoiceProjectBuilder::default(),
            )),
            stress_simulator: Arc::new(Mutex::new(
                nexus_kernel::autopilot::stress_test::StressSimulator::default(),
            )),
            live_deployer: Arc::new(Mutex::new(
                nexus_kernel::autopilot::deploy::LiveDeployer::default(),
            )),
            live_evolver: Arc::new(Mutex::new(
                nexus_kernel::autopilot::live_evolution::LiveAppEvolver::default(),
            )),
            freelance_engine: Arc::new(Mutex::new(
                nexus_kernel::economy::freelancer::FreelanceEngine::default(),
            )),
            conversational_builder: Arc::new(Mutex::new(ConversationalBuilder::new())),
            live_previews: Arc::new(Mutex::new(HashMap::new())),
            remix_engine: Arc::new(Mutex::new(RemixEngine::new())),
            problem_solver: Arc::new(Mutex::new(ProblemSolver::new())),
            marketplace_publisher: Arc::new(Mutex::new(MarketplacePublisher::new())),
            teach_modes: Arc::new(Mutex::new(HashMap::new())),
            routing_learner: Arc::new(
                Mutex::new(nexus_kernel::self_improve::RoutingLearner::new()),
            ),
            startup_instant: std::time::Instant::now(),
            rate_limiter: {
                let rl_config = load_config().map(|c| c.rate_limiting).unwrap_or_default();
                nexus_kernel::rate_limit::NexusRateLimiter::from_config(&rl_config)
            },
            api_config: load_config().map(|c| c.api).unwrap_or_default(),
            chat_conversation_state: Arc::new(Mutex::new(ChatConversationState::default())),
            // Enterprise crate state
            session_manager: Arc::new(SessionManager::new(8)),
            workspace_manager: Arc::new(Mutex::new(WorkspaceManager::new())),
            integration_router: Arc::new(IntegrationRouter::from_config(
                &nexus_integrations::IntegrationConfig::default(),
            )),
            metering_store: Arc::new(Mutex::new({
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let metering_path = std::path::Path::new(&home)
                    .join(".nexus")
                    .join("metering.db");
                if let Some(parent) = metering_path.parent() {
                    // Best-effort: create parent directory for metering DB; fallback to in-memory below
                    let _ = std::fs::create_dir_all(parent);
                }
                nexus_metering::MeteringStore::open(&metering_path).unwrap_or_else(|e| {
                    eprintln!("metering: falling back to in-memory DB: {e}");
                    nexus_metering::MeteringStore::in_memory().unwrap_or_else(|e2| {
                        eprintln!("╔══════════════════════════════════════════╗");
                        eprintln!("║  FATAL: Nexus OS failed to start         ║");
                        eprintln!("╠══════════════════════════════════════════╣");
                        eprintln!("║  Error: {e2}");
                        eprintln!("║                                          ║");
                        eprintln!("║  Please check:                           ║");
                        eprintln!("║  1. Config file exists and is valid      ║");
                        eprintln!("║  2. Required ports are available         ║");
                        eprintln!("║  3. Sufficient disk space and memory     ║");
                        eprintln!("╚══════════════════════════════════════════╝");
                        std::process::exit(1);
                    })
                })
            })),
            metering_rates: Arc::new(nexus_metering::CostRates::default()),
            telemetry_config: Arc::new(Mutex::new(nexus_telemetry::TelemetryConfig::desktop())),
            a2a_client: Arc::new(Mutex::new(A2aClient::new())),
            schedule_store: {
                let ss = Arc::new(nexus_kernel::scheduler::ScheduleStore::new(
                    NexusDatabase::default_db_path()
                        .parent()
                        .unwrap_or(std::path::Path::new(".")),
                ));
                ss
            },
            schedule_runner: {
                // Uses the same ScheduleStore path — ScheduleStore internally re-reads from disk
                let runner_store = Arc::new(nexus_kernel::scheduler::ScheduleStore::new(
                    NexusDatabase::default_db_path()
                        .parent()
                        .unwrap_or(std::path::Path::new(".")),
                ));
                let sched_executor = Arc::new(nexus_kernel::scheduler::ScheduledExecutor::new(
                    supervisor_for_runner,
                    Arc::new(Mutex::new(
                        nexus_kernel::cognitive::algorithms::adversarial::AdversarialArena::new(),
                    )),
                    audit_for_runner,
                ));
                Arc::new(nexus_kernel::scheduler::ScheduleRunner::new(
                    runner_store,
                    sched_executor,
                ))
            },
            flash_session_manager: Arc::new(FlashSessionManager::new(
                nexus_flash_infer::detect_hardware(),
            )),
            flash_providers: Arc::new(Mutex::new(HashMap::new())),
            flash_speculative: Arc::new(Mutex::new(None)),
            adversarial_arena: Arc::new(Mutex::new(
                nexus_kernel::cognitive::algorithms::adversarial::AdversarialArena::new(),
            )),
            capability_measurement: Arc::new(MeasurementState::new()),
            predictive_router: Arc::new(RouterState::new()),
            browser_agent: Arc::new(BrowserState::default()),
            token_economy: Arc::new(token_cmds::EconomyState::new()),
            governed_control: Arc::new(cc_cmds::ControlState::default()),
            world_simulation: Arc::new(sim_cmds::SimulationState::new()),
            perception: Arc::new(perception_cmds::PerceptionState::default()),
            persistent_memory: Arc::new(memory_cmds::MemoryState::default()),
            external_tools: Arc::new(tools_cmds::ToolState::default()),
            collab_protocol: Arc::new(collab_cmds::CollabState::default()),
            software_factory: Arc::new(factory_cmds::FactoryState::default()),
            mcp_standalone: Arc::new(mcp2_cmds::McpState::default()),
            a2a_crate: Arc::new(a2a_crate_cmds::A2aState::default()),
            memory_kernel: Arc::new(mk_cmds::MemoryKernelState::default()),
            governance_ruleset: Arc::new(Mutex::new(
                nexus_governance_engine::GovernanceRuleset::new(
                    "nexus-default".into(),
                    1,
                    vec![
                        nexus_governance_engine::GovernanceRule {
                            id: "allow-llm".into(),
                            description: "Allow LLM queries".into(),
                            effect: nexus_governance_engine::RuleEffect::Allow,
                            conditions: vec![
                                nexus_governance_engine::RuleCondition::CapabilityInSet(vec![
                                    "llm.query".into(),
                                ]),
                            ],
                        },
                        nexus_governance_engine::GovernanceRule {
                            id: "deny-dangerous".into(),
                            description: "Deny dangerous capabilities by default".into(),
                            effect: nexus_governance_engine::RuleEffect::Deny,
                            conditions: vec![
                                nexus_governance_engine::RuleCondition::CapabilityInSet(vec![
                                    "agent.create".into(),
                                    "process.exec".into(),
                                ]),
                            ],
                        },
                    ],
                ),
            )),
            governance_audit_log: Arc::new(Mutex::new(
                nexus_governance_engine::DecisionAuditLog::new(),
            )),
            governance_evolution: Arc::new(Mutex::new(
                nexus_governance_evolution::GovernanceEvolution::new(
                    nexus_governance_evolution::ThreatModel::new(),
                    nexus_governance_evolution::default_attack_generators(),
                ),
            )),
            #[cfg(all(
                feature = "tauri-runtime",
                any(target_os = "windows", target_os = "macos", target_os = "linux")
            ))]
            app_handle: Arc::new(Mutex::new(None)),
        };

        state
    }

    /// Heavy agent loading deferred from `new()` so the GUI thread is not blocked.
    fn load_agents_deferred(&self) {
        restore_persisted_agents(self);
        self.load_prebuilt_agents();
    }

    /// Create an AppState backed by an in-memory DB (for tests).
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_in_memory() -> Self {
        let supervisor = Arc::new(Mutex::new(Supervisor::new()));
        let test_db = Arc::new(NexusDatabase::in_memory().unwrap_or_else(|e| {
            eprintln!("in-memory DB must succeed: {e}");
            std::process::exit(1)
        }));
        let evolution_tracker = Arc::new(nexus_kernel::cognitive::EvolutionTracker::new(Box::new(
            DbStrategyStore {
                db: test_db.clone(),
            },
        )));
        Self {
            supervisor: supervisor.clone(),
            audit: Arc::new(Mutex::new(AuditTrail::new())),
            meta: Arc::new(Mutex::new(HashMap::new())),
            voice: Arc::new(Mutex::new(VoiceRuntimeState {
                wake_word_enabled: true,
                push_to_talk_enabled: true,
                overlay_visible: false,
            })),
            identity_mgr: Arc::new(Mutex::new(
                nexus_kernel::identity::IdentityManager::in_memory(),
            )),
            browser: Arc::new(Mutex::new(BrowserManager::new())),
            research: Arc::new(Mutex::new(ResearchManager::new())),
            build: Arc::new(Mutex::new(BuildManager::new())),
            learning: Arc::new(Mutex::new(LearningManager::new())),
            rag: Arc::new(Mutex::new(RagPipeline::new(RagConfig::default()))),
            redaction_engine: Arc::new(Mutex::new(RedactionEngine::default())),
            model_registry: Arc::new(Mutex::new(ModelRegistry::default_dir())),
            nexus_link: Arc::new(Mutex::new({
                let hostname = std::env::var("HOSTNAME")
                    .or_else(|_| std::env::var("COMPUTERNAME"))
                    .unwrap_or_else(|_| "nexus-device".to_string());
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let models_dir = std::path::Path::new(&home).join(".nexus").join("models");
                NexusLink::new(&hostname, &models_dir.display().to_string())
            })),
            evolution: Arc::new(Mutex::new(EvolutionEngine::new(EvolutionConfig::default()))),
            mcp_host: Arc::new(Mutex::new(McpHostManager::new())),
            ghost_protocol: Arc::new(Mutex::new(GhostProtocol::new(GhostConfig::default()))),
            voice_process: Arc::new(Mutex::new(VoiceProcess::default())),
            factory: Arc::new(Mutex::new(FactoryPipeline::new())),
            computer_control: Arc::new(Mutex::new(ComputerControlEngine::new())),
            neural_bridge: Arc::new(Mutex::new(NeuralBridge::new(NeuralBridgeConfig::default()))),
            economic_engine: Arc::new(Mutex::new(EconomicEngine::new(EconomicConfig::default()))),
            agent_memory: Arc::new(Mutex::new(AgentMemory::new(MemoryConfig::default()))),
            tracing_engine: Arc::new(Mutex::new(TracingEngine::new(1000))),
            payment_engine: Arc::new(Mutex::new(PaymentEngine::new(RevenueSplit::default()))),
            whisper: Arc::new(Mutex::new(WhisperTranscriber::new())),
            replay_recorder: Arc::new(Mutex::new(
                nexus_kernel::replay::recorder::ReplayRecorder::new(500),
            )),
            reputation_registry: Arc::new(Mutex::new(
                nexus_kernel::reputation::ReputationRegistry::new(),
            )),
            db: test_db,
            cognitive_runtime: Arc::new(
                nexus_kernel::cognitive::CognitiveRuntime::with_provider_registry(
                    supervisor,
                    nexus_kernel::cognitive::LoopConfig::default(),
                    Arc::new(nexus_kernel::cognitive::NoOpEmitter),
                    build_provider_registry(),
                ),
            ),
            blocked_consent_waits: Arc::new(Mutex::new(HashMap::new())),
            computer_action_cancellations: Arc::new(Mutex::new(HashMap::new())),
            hivemind: Arc::new(nexus_kernel::cognitive::HivemindCoordinator::new(
                Box::new(GatewayHivemindLlm),
                Arc::new(nexus_kernel::cognitive::hivemind::NoOpHivemindEmitter),
                Arc::new(Mutex::new(AuditTrail::new())),
            )),
            message_gateway: Arc::new(Mutex::new(MessageGateway::new())),
            evolution_tracker,
            auto_evolution: Arc::new(AutoEvolutionManager::new()),
            agent_scheduler: Arc::new(nexus_kernel::cognitive::AgentScheduler::new(
                Arc::new(
                    nexus_kernel::cognitive::CognitiveRuntime::with_provider_registry(
                        Arc::new(Mutex::new(Supervisor::new())),
                        nexus_kernel::cognitive::LoopConfig::default(),
                        Arc::new(nexus_kernel::cognitive::NoOpEmitter),
                        build_provider_registry(),
                    ),
                ),
                Arc::new(Mutex::new(AuditTrail::new())),
            )),
            simulation_manager: Arc::new(SimulationManager::default()),
            consciousness: Arc::new(Mutex::new(
                nexus_kernel::consciousness::ConsciousnessEngine::new(),
            )),
            dream_engine: Arc::new(Mutex::new(nexus_kernel::dreams::DreamEngine::new(
                nexus_kernel::dreams::DreamScheduler::new(),
            ))),
            temporal_engine: Arc::new(
                Mutex::new(nexus_kernel::temporal::TemporalEngine::default()),
            ),
            immune_scan_results: Arc::new(Mutex::new(Vec::new())),
            immune_last_scan: Arc::new(Mutex::new(0)),
            self_rewrite_patches: Arc::new(Mutex::new(Vec::new())),
            temporal_checkpoints: Arc::new(Mutex::new(
                nexus_kernel::temporal::TemporalCheckpointManager::default(),
            )),
            time_dilator: Arc::new(Mutex::new(nexus_kernel::temporal::TimeDilator::default())),
            self_improving_os: Arc::new(Mutex::new(
                nexus_kernel::self_improve::SelfImprovingOS::new(),
            )),
            self_improve_state: Arc::new(Mutex::new(
                commands::self_improvement::SelfImproveState::default(),
            )),
            screenshot_cloner: Arc::new(Mutex::new(
                nexus_kernel::autopilot::screenshot_clone::ScreenshotCloner::default(),
            )),
            voice_project: Arc::new(Mutex::new(
                nexus_kernel::autopilot::voice_project::VoiceProjectBuilder::default(),
            )),
            stress_simulator: Arc::new(Mutex::new(
                nexus_kernel::autopilot::stress_test::StressSimulator::default(),
            )),
            live_deployer: Arc::new(Mutex::new(
                nexus_kernel::autopilot::deploy::LiveDeployer::default(),
            )),
            live_evolver: Arc::new(Mutex::new(
                nexus_kernel::autopilot::live_evolution::LiveAppEvolver::default(),
            )),
            freelance_engine: Arc::new(Mutex::new(
                nexus_kernel::economy::freelancer::FreelanceEngine::default(),
            )),
            conversational_builder: Arc::new(Mutex::new(ConversationalBuilder::new())),
            live_previews: Arc::new(Mutex::new(HashMap::new())),
            remix_engine: Arc::new(Mutex::new(RemixEngine::new())),
            problem_solver: Arc::new(Mutex::new(ProblemSolver::new())),
            marketplace_publisher: Arc::new(Mutex::new(MarketplacePublisher::new())),
            teach_modes: Arc::new(Mutex::new(HashMap::new())),
            routing_learner: Arc::new(
                Mutex::new(nexus_kernel::self_improve::RoutingLearner::new()),
            ),
            chat_conversation_state: Arc::new(Mutex::new(ChatConversationState::default())),
            // Enterprise crate state (test)
            session_manager: Arc::new(SessionManager::new(8)),
            workspace_manager: Arc::new(Mutex::new(WorkspaceManager::new())),
            integration_router: Arc::new(IntegrationRouter::empty()),
            metering_store: Arc::new(Mutex::new(
                nexus_metering::MeteringStore::in_memory().unwrap_or_else(|e| {
                    eprintln!("in-memory metering DB must succeed: {e}");
                    std::process::exit(1)
                }),
            )),
            metering_rates: Arc::new(nexus_metering::CostRates::default()),
            telemetry_config: Arc::new(Mutex::new(nexus_telemetry::TelemetryConfig::desktop())),
            startup_instant: std::time::Instant::now(),
            rate_limiter: nexus_kernel::rate_limit::NexusRateLimiter::disabled(),
            api_config: nexus_kernel::rate_limit::ApiHardeningConfig::default(),
            a2a_client: Arc::new(Mutex::new(A2aClient::new())),
            schedule_store: Arc::new(nexus_kernel::scheduler::ScheduleStore::new(
                std::env::temp_dir().as_path(),
            )),
            schedule_runner: Arc::new(nexus_kernel::scheduler::ScheduleRunner::new(
                Arc::new(nexus_kernel::scheduler::ScheduleStore::new(
                    std::env::temp_dir().as_path(),
                )),
                Arc::new(nexus_kernel::scheduler::ScheduledExecutor::new(
                    Arc::new(Mutex::new(nexus_kernel::supervisor::Supervisor::new())),
                    Arc::new(Mutex::new(
                        nexus_kernel::cognitive::algorithms::adversarial::AdversarialArena::new(),
                    )),
                    Arc::new(Mutex::new(nexus_kernel::audit::AuditTrail::new())),
                )),
            )),
            flash_session_manager: Arc::new(FlashSessionManager::new(
                nexus_flash_infer::HardwareInfo::default(),
            )),
            flash_providers: Arc::new(Mutex::new(HashMap::new())),
            flash_speculative: Arc::new(Mutex::new(None)),
            adversarial_arena: Arc::new(Mutex::new(
                nexus_kernel::cognitive::algorithms::adversarial::AdversarialArena::new(),
            )),
            capability_measurement: Arc::new(MeasurementState::new()),
            predictive_router: Arc::new(RouterState::new()),
            browser_agent: Arc::new(BrowserState::default()),
            token_economy: Arc::new(token_cmds::EconomyState::new()),
            governed_control: Arc::new(cc_cmds::ControlState::default()),
            world_simulation: Arc::new(sim_cmds::SimulationState::new()),
            perception: Arc::new(perception_cmds::PerceptionState::default()),
            persistent_memory: Arc::new(memory_cmds::MemoryState::default()),
            external_tools: Arc::new(tools_cmds::ToolState::default()),
            collab_protocol: Arc::new(collab_cmds::CollabState::default()),
            software_factory: Arc::new(factory_cmds::FactoryState::default()),
            mcp_standalone: Arc::new(mcp2_cmds::McpState::default()),
            a2a_crate: Arc::new(a2a_crate_cmds::A2aState::default()),
            memory_kernel: Arc::new(mk_cmds::MemoryKernelState::default()),
            governance_ruleset: Arc::new(Mutex::new(
                nexus_governance_engine::GovernanceRuleset::new("test".into(), 1, vec![]),
            )),
            governance_audit_log: Arc::new(Mutex::new(
                nexus_governance_engine::DecisionAuditLog::new(),
            )),
            governance_evolution: Arc::new(Mutex::new(
                nexus_governance_evolution::GovernanceEvolution::new(
                    nexus_governance_evolution::ThreatModel::new(),
                    nexus_governance_evolution::default_attack_generators(),
                ),
            )),
            #[cfg(all(
                feature = "tauri-runtime",
                any(target_os = "windows", target_os = "macos", target_os = "linux")
            ))]
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    fn log_event(&self, agent_id: AgentId, event_type: EventType, payload: serde_json::Value) {
        let event_type_str = format!("{event_type:?}");
        let mut guard = match self.audit.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Err(e) = guard.append_event(agent_id, event_type, payload.clone()) {
            eprintln!("audit append failed: {e}");
        }

        // Persist audit event to database
        let prev_hash = self
            .db
            .get_latest_audit_hash()
            .ok()
            .flatten()
            .unwrap_or_else(|| "0".repeat(64));
        let sequence = self.db.get_audit_count().unwrap_or(0);
        let detail = serde_json::to_string(&payload).unwrap_or_default();
        let hash_input = format!("{prev_hash}:{sequence}:{detail}");
        let current_hash = format!("{:x}", sha2::Sha256::digest(hash_input.as_bytes()));
        if let Err(e) = self.db.append_audit_event(
            &agent_id.to_string(),
            &event_type_str,
            &detail,
            &prev_hash,
            &current_hash,
            sequence,
        ) {
            eprintln!("persistence: audit append failed: {e}");
        }
    }

    /// Check rate limit for the given category. Returns `Err(String)` if exceeded.
    fn check_rate(&self, category: nexus_kernel::rate_limit::RateCategory) -> Result<(), String> {
        self.rate_limiter
            .check(category, "desktop")
            .map_err(|e| e.to_string())
    }

    /// Validate a string input against API hardening limits.
    fn validate_input(&self, value: &str) -> Result<(), String> {
        nexus_kernel::rate_limit::validate_string(value, &self.api_config)
            .map_err(|e| e.to_string())
    }

    /// Validate a file path against traversal attacks.
    fn validate_path_input(&self, path: &str) -> Result<(), String> {
        nexus_kernel::rate_limit::validate_path(path).map_err(|e| e.to_string())
    }

    pub fn register_blocked_consent_wait(&self, agent_id: &str, consent_id: &str) -> Arc<Notify> {
        let notify = Arc::new(Notify::new());
        self.blocked_consent_waits
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                agent_id.to_string(),
                BlockedConsentWait {
                    consent_id: consent_id.to_string(),
                    notify: notify.clone(),
                },
            );
        notify
    }

    pub fn clear_blocked_consent_wait(&self, agent_id: &str, consent_id: &str) {
        let mut waits = self
            .blocked_consent_waits
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let should_remove = waits
            .get(agent_id)
            .is_some_and(|wait| wait.consent_id == consent_id);
        if should_remove {
            waits.remove(agent_id);
        }
    }

    fn wake_blocked_consent_wait(&self, agent_id: &str, consent_id: &str) -> bool {
        let notify = self
            .blocked_consent_waits
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(agent_id)
            .filter(|wait| wait.consent_id == consent_id)
            .map(|wait| wait.notify.clone());
        if let Some(notify) = notify {
            notify.notify_one();
            true
        } else {
            false
        }
    }

    fn wake_and_clear_blocked_consent_wait(&self, agent_id: &str) -> bool {
        let wait = self
            .blocked_consent_waits
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(agent_id);
        if let Some(wait) = wait {
            wait.notify.notify_one();
            true
        } else {
            false
        }
    }

    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    fn set_app_handle(&self, app_handle: tauri::AppHandle<tauri::Wry>) {
        let mut guard = self.app_handle.lock().unwrap_or_else(|p| p.into_inner());
        *guard = Some(app_handle);
    }

    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    fn app_handle(&self) -> Option<tauri::AppHandle<tauri::Wry>> {
        self.app_handle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn initialize_startup_schedules(&self) {
        let rows = match self.db.list_agents() {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("scheduler: failed to scan persisted agents: {error}");
                return;
            }
        };

        for row in rows {
            if !row.was_running {
                continue;
            }
            let Ok(json_manifest) = serde_json::from_str::<JsonAgentManifest>(&row.manifest_json)
            else {
                continue;
            };
            register_manifest_schedule(
                self,
                &row.id,
                json_manifest.manifest.schedule.as_deref(),
                json_manifest.manifest.default_goal.as_deref(),
                json_manifest.description.as_deref(),
            );
        }
    }
}

// Re-export all domain implementations so mod runtime's `use super::*` resolves.
// pub (not pub(crate)) so integration tests can import these symbols.
pub use commands::advanced::*;
pub use commands::agents::*;
pub use commands::apps::*;
pub use commands::audit_compliance::*;
pub use commands::autopilot::*;
pub use commands::browser_research::*;
pub use commands::chat_llm::*;
pub use commands::cognitive::*;
pub use commands::consent::*;
pub use commands::enterprise::*;
pub use commands::governance::*;
pub use commands::model_hub::*;
pub use commands::simulation::*;
pub use commands::tools_infra::*;
pub use commands::trust_security::*;

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub mod runtime {
    use super::*;
    #[cfg(not(target_os = "linux"))]
    use tauri::menu::{Menu, MenuItem};
    #[cfg(not(target_os = "linux"))]
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    struct TauriSimulationObserver {
        app: tauri::AppHandle,
        state: AppState,
    }

    impl SimulationObserver for TauriSimulationObserver {
        fn on_tick(&self, progress: &SimulationProgress) {
            // Best-effort: forward simulation tick to frontend; missed ticks are non-fatal
            let _ = self.app.emit("simulation-tick", progress);
            self.state.log_event(
                Uuid::parse_str(&progress.world_id).unwrap_or_else(|_| Uuid::nil()),
                EventType::UserAction,
                json!({
                    "action": "simulation_tick",
                    "world_id": &progress.world_id,
                    "tick": progress.tick,
                    "status": &progress.status,
                    "events_count": progress.events_count,
                    "events": &progress.events,
                    "fuel_consumed": progress.fuel_consumed,
                    "belief_summary": &progress.belief_summary,
                }),
            );
        }

        fn on_complete(&self, world_id: &str, report: &PredictionReport) {
            // Best-effort: notify frontend of simulation completion
            let _ = self.app.emit(
                "simulation-complete",
                &json!({
                    "world_id": world_id,
                    "prediction": report.prediction,
                    "confidence": report.confidence,
                }),
            );
        }
    }

    #[tauri::command]
    fn list_agents(state: tauri::State<'_, AppState>) -> Result<Vec<AgentRow>, String> {
        super::list_agents(state.inner())
    }

    #[tauri::command]
    fn create_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        manifest_json: String,
    ) -> Result<String, String> {
        let id = super::create_agent(state.inner(), manifest_json)?;
        if uuid::Uuid::parse_str(&id).is_ok() {
            emit_agent_status(&window, state.inner(), &id);
        }
        Ok(id)
    }

    #[tauri::command]
    fn start_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::start_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn stop_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::stop_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn clear_all_agents(state: tauri::State<'_, AppState>) -> Result<usize, String> {
        super::clear_all_agents(state.inner())
    }

    #[tauri::command]
    fn get_scheduled_agents(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {
        super::get_scheduled_agents(state.inner())
    }

    #[tauri::command]
    fn get_preinstalled_agents(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::PreinstalledAgentRow>, String> {
        super::get_preinstalled_agents(state.inner())
    }

    #[tauri::command]
    fn pause_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::pause_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn resume_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::resume_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    /// Emit an agent-status-changed event to the frontend.
    fn emit_agent_status(window: &tauri::Window, state: &AppState, agent_id: &str) {
        let parsed = match uuid::Uuid::parse_str(agent_id) {
            Ok(id) => id,
            Err(_) => return,
        };
        let supervisor = match state.supervisor.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(status) = supervisor
            .health_check()
            .into_iter()
            .find(|s| s.id == parsed)
        {
            // Best-effort: push agent status change to frontend via event
            let _ = window.emit(
                "agent-status-changed",
                AgentStatusEvent {
                    agent_id: agent_id.to_string(),
                    status: status.state.to_string(),
                    fuel_remaining: status.remaining_fuel,
                },
            );
        }
    }

    #[tauri::command]
    fn get_audit_log(
        state: tauri::State<'_, AppState>,
        agent_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditRow>, String> {
        super::get_audit_log(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn send_chat(
        state: tauri::State<'_, AppState>,
        message: String,
        model_id: Option<String>,
        agent_name: Option<String>,
    ) -> Result<ChatResponse, String> {
        super::send_chat(state.inner(), message, model_id, agent_name)
    }

    #[tauri::command]
    fn get_agent_performance(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<nexus_kernel::genome::AgentPerformanceTracker, String> {
        super::get_agent_performance(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_auto_evolution_log(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        limit: u32,
    ) -> Result<Vec<nexus_kernel::genome::EvolutionEvent>, String> {
        super::get_auto_evolution_log(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn set_auto_evolution_config(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        enabled: bool,
        threshold: f64,
        cooldown_seconds: u64,
    ) -> Result<(), String> {
        super::set_auto_evolution_config(
            state.inner(),
            agent_id,
            enabled,
            threshold,
            cooldown_seconds,
        )
    }

    #[tauri::command]
    fn force_evolve_agent(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<nexus_kernel::genome::EvolutionResult, String> {
        super::force_evolve_agent(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_config() -> Result<NexusConfig, String> {
        super::get_config()
    }

    #[tauri::command]
    fn save_config(state: tauri::State<'_, AppState>, config: NexusConfig) -> Result<(), String> {
        state.check_rate(nexus_kernel::rate_limit::RateCategory::AdminOperation)?;
        super::save_config(config)
    }

    #[tauri::command]
    fn start_jarvis_mode(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::start_jarvis_mode(state.inner())
    }

    #[tauri::command]
    fn stop_jarvis_mode(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::stop_jarvis_mode(state.inner())
    }

    #[tauri::command]
    fn jarvis_status(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::jarvis_status(state.inner())
    }

    #[tauri::command]
    fn transcribe_push_to_talk() -> Result<String, String> {
        super::transcribe_push_to_talk()
    }

    #[tauri::command]
    fn tray_status(state: tauri::State<'_, AppState>) -> Result<TrayStatus, String> {
        super::tray_status(state.inner())
    }

    #[tauri::command]
    fn detect_hardware() -> Result<HardwareInfo, String> {
        super::detect_hardware()
    }

    #[tauri::command]
    fn check_ollama(base_url: Option<String>) -> Result<OllamaStatus, String> {
        super::check_ollama(base_url)
    }

    #[tauri::command]
    fn pull_ollama_model(model_name: String, base_url: Option<String>) -> Result<String, String> {
        super::pull_ollama_model(model_name, base_url)
    }

    /// Pull a model on a background thread with throttled progress events.
    /// The Tauri async runtime keeps the main thread free while we block here.
    #[tauri::command]
    async fn pull_model(
        window: tauri::Window,
        model_name: String,
        base_url: Option<String>,
    ) -> Result<String, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let result = super::pull_ollama_model_throttled(model_name, base_url, |progress| {
                // Best-effort: forward pull progress to frontend; missed events are non-fatal
                let _ = window.emit("model-pull-progress", &progress);
            });
            // Best-effort: send result back to async receiver; thread termination handled by recv
            let _ = tx.send(result);
        });
        // recv() blocks this async task's thread, but Tauri runs async commands
        // on a thread pool so the main/UI thread stays responsive.
        rx.recv()
            .unwrap_or(Err("Download thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn ensure_ollama(base_url: Option<String>) -> Result<bool, String> {
        super::ensure_ollama(base_url)
    }

    #[tauri::command]
    fn is_ollama_installed() -> bool {
        super::is_ollama_installed()
    }

    #[tauri::command]
    fn delete_model(model_name: String, base_url: Option<String>) -> Result<(), String> {
        super::delete_ollama_model(model_name, base_url)
    }

    #[tauri::command]
    fn is_setup_complete() -> bool {
        super::is_setup_complete()
    }

    #[tauri::command]
    fn run_setup_wizard(ollama_url: Option<String>) -> Result<SetupResult, String> {
        super::run_setup_wizard(ollama_url)
    }

    #[tauri::command]
    fn list_available_models() -> Result<Vec<super::AvailableModel>, String> {
        super::list_available_models()
    }

    #[tauri::command]
    fn list_provider_models() -> Result<Vec<super::ProviderModel>, String> {
        super::list_provider_models()
    }

    #[tauri::command]
    fn get_provider_status() -> Result<super::ProviderStatus, String> {
        super::get_provider_status()
    }

    #[tauri::command]
    async fn get_available_providers(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::AvailableProvider>, String> {
        super::get_available_providers(state.inner()).await
    }

    #[tauri::command]
    fn save_api_key(provider: String, api_key: String) -> Result<(), String> {
        super::save_provider_api_key(provider, api_key)
    }

    /// Stream chat via Ollama's OpenAI-compatible endpoint.
    /// Emits `chat-token` events with throttling, returns full text.
    #[tauri::command]
    async fn chat_with_ollama(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        messages: Vec<serde_json::Value>,
        model: String,
        base_url: Option<String>,
    ) -> Result<String, String> {
        let app_state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let mut last_emit = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now);
            let mut full = String::new();

            let result =
                super::chat_with_ollama_streaming(&app_state, messages, model, base_url, |token| {
                    full.push_str(token);

                    // Throttle: emit at most every 50ms
                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit).as_millis() >= 50 {
                        // Best-effort: stream chat token to frontend; dropped tokens are non-fatal
                        let _ = window.emit(
                            "chat-token",
                            serde_json::json!({
                                "token": token,
                                "full": &full,
                                "done": false,
                            }),
                        );
                        last_emit = now;
                    }
                });

            match &result {
                Ok(text) => {
                    // Best-effort: emit final chat completion to frontend
                    let _ = window.emit(
                        "chat-token",
                        serde_json::json!({
                            "token": "",
                            "full": text,
                            "done": true,
                        }),
                    );
                }
                Err(e) => {
                    // Best-effort: emit chat error to frontend
                    let _ = window.emit(
                        "chat-token",
                        serde_json::json!({
                            "token": "",
                            "full": "",
                            "done": true,
                            "error": e,
                        }),
                    );
                }
            }

            // Best-effort: send result back to async receiver; thread termination handled by recv
            let _ = tx.send(result);
        });
        rx.recv()
            .unwrap_or(Err("Chat thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn set_agent_model(agent: String, model: String) -> Result<(), String> {
        super::set_agent_model(agent, model)
    }

    #[tauri::command]
    fn check_llm_status() -> Result<super::LlmStatusResponse, String> {
        super::check_llm_status()
    }

    #[tauri::command]
    fn get_llm_recommendations() -> Result<super::LlmRecommendations, String> {
        super::get_llm_recommendations()
    }

    #[tauri::command]
    fn set_agent_llm_provider(
        agent_id: String,
        provider_id: String,
        local_only: bool,
        budget_dollars: u32,
        budget_tokens: u64,
    ) -> Result<(), String> {
        super::set_agent_llm_provider(
            agent_id,
            provider_id,
            local_only,
            budget_dollars,
            budget_tokens,
        )
    }

    #[tauri::command]
    fn get_provider_usage_stats(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ProviderUsageStats>, String> {
        super::get_provider_usage_stats(state.inner())
    }

    #[tauri::command]
    fn test_llm_connection(provider_name: String) -> Result<super::TestConnectionResult, String> {
        super::test_llm_connection(provider_name)
    }

    #[tauri::command]
    fn get_system_info() -> Result<SystemInfo, String> {
        super::get_system_info()
    }

    // ── Permission Dashboard Commands ──

    #[tauri::command]
    fn get_agent_permissions(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelPermissionCategory>, String> {
        super::get_agent_permissions(state.inner(), agent_id)
    }

    #[tauri::command]
    fn update_agent_permission(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        capability_key: String,
        enabled: bool,
    ) -> Result<(), String> {
        super::update_agent_permission(state.inner(), agent_id, capability_key, enabled)
    }

    #[tauri::command]
    fn get_permission_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelPermissionHistoryEntry>, String> {
        super::get_permission_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_capability_request(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelCapabilityRequest>, String> {
        super::get_capability_request(state.inner(), agent_id)
    }

    #[tauri::command]
    fn bulk_update_permissions(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        updates: Vec<super::PermissionUpdate>,
        reason: Option<String>,
    ) -> Result<(), String> {
        super::bulk_update_permissions(state.inner(), agent_id, updates, reason)
    }

    // ── Protocols Dashboard Commands ──

    #[tauri::command]
    fn get_protocols_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::ProtocolsStatusRow, String> {
        super::get_protocols_status(state.inner())
    }

    #[tauri::command]
    fn get_protocols_requests(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ProtocolRequestRow>, String> {
        super::get_protocols_requests(state.inner())
    }

    #[tauri::command]
    fn get_mcp_tools(state: tauri::State<'_, AppState>) -> Result<Vec<super::McpToolRow>, String> {
        super::get_mcp_tools(state.inner())
    }

    #[tauri::command]
    fn get_agent_cards(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::AgentCardRow>, String> {
        super::get_agent_cards(state.inner())
    }

    // ── A2A Client Commands ──

    #[tauri::command]
    fn a2a_discover_agent(
        state: tauri::State<'_, AppState>,
        url: String,
    ) -> Result<serde_json::Value, String> {
        super::a2a_discover_agent(state.inner(), url)
    }

    #[tauri::command]
    fn a2a_send_task(
        state: tauri::State<'_, AppState>,
        agent_url: String,
        message: String,
    ) -> Result<serde_json::Value, String> {
        super::a2a_send_task(state.inner(), agent_url, message)
    }

    #[tauri::command]
    fn a2a_get_task_status(
        state: tauri::State<'_, AppState>,
        agent_url: String,
        task_id: String,
    ) -> Result<serde_json::Value, String> {
        super::a2a_get_task_status(state.inner(), agent_url, task_id)
    }

    #[tauri::command]
    fn a2a_cancel_task(
        state: tauri::State<'_, AppState>,
        agent_url: String,
        task_id: String,
    ) -> Result<(), String> {
        super::a2a_cancel_task(state.inner(), agent_url, task_id)
    }

    #[tauri::command]
    fn a2a_known_agents(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
        super::a2a_known_agents(state.inner())
    }

    // ── Identity Commands ──

    #[tauri::command]
    fn get_agent_identity(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<super::IdentityRow, String> {
        super::get_agent_identity(state.inner(), agent_id)
    }

    #[tauri::command]
    fn list_identities(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::IdentityRow>, String> {
        super::list_identities(state.inner())
    }

    // ── Firewall Commands ──

    #[tauri::command]
    fn get_firewall_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::FirewallStatusRow, String> {
        super::get_firewall_status(state.inner())
    }

    #[tauri::command]
    fn get_firewall_patterns() -> Result<super::FirewallPatternsRow, String> {
        super::get_firewall_patterns()
    }

    // ── Marketplace Commands ──

    #[tauri::command]
    fn marketplace_search(query: String) -> Result<Vec<super::MarketplaceAgentRow>, String> {
        super::marketplace_search(&query)
    }

    #[tauri::command]
    fn marketplace_install(package_id: String) -> Result<super::MarketplaceAgentRow, String> {
        super::marketplace_install(&package_id)
    }

    #[tauri::command]
    fn marketplace_info(agent_id: String) -> Result<super::MarketplaceDetailRow, String> {
        super::marketplace_info(&agent_id)
    }

    #[tauri::command]
    fn marketplace_publish(bundle_json: String) -> Result<super::MarketplacePublishResult, String> {
        super::marketplace_publish(&bundle_json)
    }

    #[tauri::command]
    fn marketplace_my_agents(author: String) -> Result<Vec<super::MarketplaceAgentRow>, String> {
        super::marketplace_my_agents(&author)
    }

    // ── Learn Mode Commands ──

    #[tauri::command]
    fn start_learning(
        state: tauri::State<'_, AppState>,
        sources: Vec<super::LearningSource>,
    ) -> Result<super::LearningSessionState, String> {
        super::start_learning(state.inner(), sources)
    }

    #[tauri::command]
    fn learning_agent_action(
        state: tauri::State<'_, AppState>,
        session_id: String,
        action: String,
        url: Option<String>,
        content: Option<String>,
    ) -> Result<super::LearningSessionState, String> {
        super::learning_agent_action(state.inner(), session_id, action, url, content)
    }

    #[tauri::command]
    fn get_learning_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::LearningSessionState, String> {
        super::get_learning_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_knowledge_base(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::KnowledgeEntry>, String> {
        super::get_knowledge_base(state.inner())
    }

    // ── Agent Browser Commands ──

    #[tauri::command]
    fn navigate_to(
        state: tauri::State<'_, AppState>,
        url: String,
    ) -> Result<super::BrowserNavigateResult, String> {
        super::navigate_to(state.inner(), url)
    }

    #[tauri::command]
    fn get_browser_history(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::BrowserHistoryEntry>, String> {
        super::get_browser_history(state.inner())
    }

    #[tauri::command]
    fn get_agent_activity(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ActivityMessageRow>, String> {
        super::get_agent_activity(state.inner())
    }

    // ── Research Mode Commands ──

    #[tauri::command]
    fn start_research(
        state: tauri::State<'_, AppState>,
        topic: String,
        num_agents: u32,
    ) -> Result<super::ResearchSessionState, String> {
        super::start_research(state.inner(), topic, num_agents)
    }

    #[tauri::command]
    fn research_agent_action(
        state: tauri::State<'_, AppState>,
        session_id: String,
        agent_id: String,
        action: String,
        url: Option<String>,
        content: Option<String>,
    ) -> Result<super::ResearchSessionState, String> {
        super::research_agent_action(state.inner(), session_id, agent_id, action, url, content)
    }

    #[tauri::command]
    fn complete_research(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::ResearchSessionState, String> {
        super::complete_research(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_research_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::ResearchSessionState, String> {
        super::get_research_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn list_research_sessions(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ResearchSessionState>, String> {
        super::list_research_sessions(state.inner())
    }

    // ── Build Mode Commands ──

    #[tauri::command]
    fn start_build(
        state: tauri::State<'_, AppState>,
        description: String,
    ) -> Result<super::BuildSessionState, String> {
        super::start_build(state.inner(), description)
    }

    #[tauri::command]
    fn build_append_code(
        state: tauri::State<'_, AppState>,
        session_id: String,
        delta: String,
        agent_name: String,
    ) -> Result<super::BuildSessionState, String> {
        super::build_append_code(state.inner(), session_id, delta, agent_name)
    }

    #[tauri::command]
    fn build_add_message(
        state: tauri::State<'_, AppState>,
        session_id: String,
        agent_name: String,
        role: String,
        content: String,
    ) -> Result<super::BuildSessionState, String> {
        super::build_add_message(state.inner(), session_id, agent_name, role, content)
    }

    #[tauri::command]
    fn complete_build(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::BuildSessionState, String> {
        super::complete_build(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::BuildSessionState, String> {
        super::get_build_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_code(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<String, String> {
        super::get_build_code(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_preview(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<String, String> {
        super::get_build_preview(state.inner(), session_id)
    }

    #[tauri::command]
    fn policy_list() -> Result<serde_json::Value, String> {
        super::policy_list()
    }

    #[tauri::command]
    fn policy_validate(content: String) -> Result<serde_json::Value, String> {
        super::policy_validate(content)
    }

    #[tauri::command]
    fn policy_test(
        content: String,
        principal: String,
        action: String,
        resource: String,
    ) -> Result<serde_json::Value, String> {
        super::policy_test(content, principal, action, resource)
    }

    #[tauri::command]
    fn policy_detect_conflicts() -> Result<serde_json::Value, String> {
        super::policy_detect_conflicts()
    }

    // ── RAG Pipeline Commands ──

    #[tauri::command]
    fn index_document(
        state: tauri::State<'_, AppState>,
        file_path: String,
    ) -> Result<String, String> {
        super::index_document(state.inner(), file_path)
    }

    #[tauri::command]
    fn search_documents(
        state: tauri::State<'_, AppState>,
        query: String,
        top_k: Option<u32>,
    ) -> Result<String, String> {
        super::search_documents(state.inner(), query, top_k)
    }

    #[tauri::command]
    fn chat_with_documents(
        state: tauri::State<'_, AppState>,
        question: String,
    ) -> Result<String, String> {
        super::chat_with_documents(state.inner(), question)
    }

    #[tauri::command]
    fn list_indexed_documents(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::list_indexed_documents(state.inner())
    }

    #[tauri::command]
    fn remove_indexed_document(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::remove_indexed_document(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_document_governance(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::get_document_governance(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_semantic_map(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_semantic_map(state.inner())
    }

    #[tauri::command]
    fn get_document_access_log(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::get_document_access_log(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_active_llm_provider(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_active_llm_provider(state.inner())
    }

    // ── Model Hub Commands ──

    #[tauri::command]
    fn search_models(
        state: tauri::State<'_, AppState>,
        query: String,
        limit: Option<u32>,
    ) -> Result<String, String> {
        super::search_models(state.inner(), query, limit)
    }

    #[tauri::command]
    fn get_model_info(
        state: tauri::State<'_, AppState>,
        model_id: String,
    ) -> Result<String, String> {
        super::get_model_info(state.inner(), model_id)
    }

    #[tauri::command]
    fn check_model_compatibility(
        state: tauri::State<'_, AppState>,
        file_size_bytes: u64,
    ) -> Result<String, String> {
        super::check_model_compatibility(state.inner(), file_size_bytes)
    }

    #[tauri::command]
    async fn download_model(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        model_id: String,
        filename: String,
    ) -> Result<String, String> {
        // Read models_dir from registry (lock briefly)
        let models_dir = {
            let registry = state
                .model_registry
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.models_dir().clone()
        };

        let model_id_clone = model_id.clone();
        let filename_clone = filename.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let target_dir = models_dir.display().to_string();
            let last_emit = std::cell::Cell::new(
                std::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(1))
                    .unwrap_or_else(std::time::Instant::now),
            );

            let result = super::model_hub::download_model_file(
                &model_id_clone,
                &filename_clone,
                &target_dir,
                |progress: DownloadProgress| {
                    let now = std::time::Instant::now();
                    let is_terminal = matches!(
                        progress.status,
                        DownloadStatus::Completed | DownloadStatus::Failed(_)
                    );

                    // Throttle at 300ms, but always emit terminal states
                    if is_terminal || now.duration_since(last_emit.get()).as_millis() >= 300 {
                        // Best-effort: forward download progress to frontend; missed events are non-fatal
                        let _ = window.emit("model-download-progress", &progress);
                        last_emit.set(now);
                    }
                },
            );

            match &result {
                Ok(model_path) => {
                    // Best-effort: generate nexus-model.toml so ModelRegistry can discover it
                    let _ = super::model_hub::generate_model_config(
                        &model_id_clone,
                        &filename_clone,
                        model_path,
                    );
                    // Best-effort: register with Ollama so it appears in Chat model list
                    let model_file_path = std::path::Path::new(model_path).join(&filename_clone);
                    let ollama_name = model_id_clone.replace('/', "--");
                    let _ = super::model_hub::register_downloaded_model_with_ollama(
                        &model_file_path,
                        &ollama_name,
                    );
                    // Best-effort: emit model-downloaded event so Chat can refresh its model list
                    let _ = window.emit(
                        "model-downloaded",
                        serde_json::json!({"model_id": &model_id_clone, "name": &ollama_name}),
                    );
                    // Best-effort: emit download completion event to frontend
                    let _ = window.emit(
                        "model-download-complete",
                        serde_json::json!({"model_id": &model_id_clone, "path": model_path}),
                    );
                }
                Err(e) => {
                    // Best-effort: emit download error to frontend
                    let _ = window.emit(
                        "model-download-complete",
                        serde_json::json!({"model_id": &model_id_clone, "error": e}),
                    );
                }
            }

            // Best-effort: send result back to async receiver; thread termination handled by recv
            let _ = tx.send(result);
        });

        // Return immediately — the thread will emit progress events
        // But we still wait for the result so Tauri knows when the command finishes
        rx.recv()
            .unwrap_or(Err("Download thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn list_local_models(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::list_local_models(state.inner())
    }

    #[tauri::command]
    fn delete_local_model(
        state: tauri::State<'_, AppState>,
        model_id: String,
    ) -> Result<String, String> {
        super::delete_local_model(state.inner(), model_id)
    }

    #[tauri::command]
    fn get_system_specs() -> Result<String, String> {
        super::get_system_specs()
    }

    #[tauri::command]
    fn get_live_system_metrics(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_live_system_metrics(state.inner())
    }

    #[tauri::command]
    fn time_machine_list_checkpoints(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_list_checkpoints(state.inner())
    }

    #[tauri::command]
    fn time_machine_get_checkpoint(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_get_checkpoint(state.inner(), id)
    }

    #[tauri::command]
    fn time_machine_create_checkpoint(
        state: tauri::State<'_, AppState>,
        label: String,
    ) -> Result<String, String> {
        super::time_machine_create_checkpoint(state.inner(), label)
    }

    #[tauri::command]
    fn time_machine_undo(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_undo(state.inner())
    }

    #[tauri::command]
    fn time_machine_undo_checkpoint(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_undo_checkpoint(state.inner(), id)
    }

    #[tauri::command]
    fn time_machine_redo(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_redo(state.inner())
    }

    #[tauri::command]
    fn time_machine_get_diff(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_get_diff(state.inner(), id)
    }

    #[tauri::command]
    fn time_machine_what_if(
        state: tauri::State<'_, AppState>,
        id: String,
        variable_key: String,
        variable_value: String,
    ) -> Result<String, String> {
        super::time_machine_what_if(state.inner(), id, variable_key, variable_value)
    }

    // ── Nexus Link commands ─────────────────────────────────────────────

    #[tauri::command]
    fn nexus_link_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::nexus_link_status(state.inner())
    }

    #[tauri::command]
    fn nexus_link_toggle_sharing(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::nexus_link_toggle_sharing(state.inner(), enabled)
    }

    #[tauri::command]
    fn nexus_link_add_peer(
        state: tauri::State<'_, AppState>,
        address: String,
        name: String,
    ) -> Result<String, String> {
        super::nexus_link_add_peer(state.inner(), address, name)
    }

    #[tauri::command]
    fn nexus_link_remove_peer(
        state: tauri::State<'_, AppState>,
        device_id: String,
    ) -> Result<String, String> {
        super::nexus_link_remove_peer(state.inner(), device_id)
    }

    #[tauri::command]
    fn nexus_link_list_peers(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::nexus_link_list_peers(state.inner())
    }

    #[tauri::command]
    async fn nexus_link_send_model(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        peer_address: String,
        model_id: String,
        filename: String,
    ) -> Result<String, String> {
        // Clone what we need from state before spawning thread
        let link_arc = state.nexus_link.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let link = link_arc.lock().unwrap_or_else(|p| p.into_inner());

            let last_emit = std::cell::Cell::new(
                std::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(1))
                    .unwrap_or_else(std::time::Instant::now),
            );

            let result = link.send_model(
                &peer_address,
                &model_id,
                &filename,
                |progress: nexus_connectors_llm::nexus_link::TransferProgress| {
                    let now = std::time::Instant::now();
                    let is_terminal = matches!(
                        progress.status,
                        nexus_connectors_llm::nexus_link::TransferStatus::Completed
                            | nexus_connectors_llm::nexus_link::TransferStatus::Failed(_)
                    );

                    if is_terminal || now.duration_since(last_emit.get()).as_millis() >= 300 {
                        // Best-effort: forward transfer progress to frontend; missed events are non-fatal
                        let _ = window.emit("nexus-link-transfer-progress", &progress);
                        last_emit.set(now);
                    }
                },
            );

            // Best-effort: send result back to async receiver; thread termination handled by recv
            let _ = tx.send(result.map(|()| "completed".to_string()));
        });

        // Return immediately — progress is emitted via events
        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(format!("Transfer thread failed: {e}")),
        }
    }

    // ── Evolution commands ───────────────────────────────────────────────

    #[tauri::command]
    fn evolution_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::evolution_get_status(state.inner())
    }

    #[tauri::command]
    fn evolution_register_strategy(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        name: String,
        parameters: String,
    ) -> Result<String, String> {
        super::evolution_register_strategy(state.inner(), agent_id, name, parameters)
    }

    #[tauri::command]
    fn evolution_evolve_once(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_evolve_once(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_get_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_get_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_rollback(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_rollback(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_get_active_strategy(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_get_active_strategy(state.inner(), agent_id)
    }

    // ── Agent DNA / Genome commands ─────────────────────────────────────

    #[tauri::command]
    fn get_agent_genome(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::get_agent_genome(state.inner(), agent_id)
    }

    #[tauri::command]
    fn breed_agents(
        state: tauri::State<'_, AppState>,
        parent_a: String,
        parent_b: String,
    ) -> Result<String, String> {
        super::breed_agents(state.inner(), parent_a, parent_b)
    }

    #[tauri::command]
    fn mutate_agent(state: tauri::State<'_, AppState>, agent_id: String) -> Result<String, String> {
        super::mutate_agent(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_agent_lineage(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::get_agent_lineage(state.inner(), agent_id)
    }

    #[tauri::command]
    fn generate_all_genomes(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::generate_all_genomes(state.inner())
    }

    #[tauri::command]
    fn evolve_population(
        state: tauri::State<'_, AppState>,
        agent_ids: Vec<String>,
        task: String,
        generations: u32,
    ) -> Result<String, String> {
        super::evolve_population(state.inner(), agent_ids, task, generations)
    }

    // ── Genesis Protocol commands ──────────────────────────────────────

    #[tauri::command]
    fn genesis_analyze_gap(
        state: tauri::State<'_, AppState>,
        user_request: String,
    ) -> Result<String, String> {
        super::genesis_analyze_gap(state.inner(), user_request)
    }

    #[tauri::command]
    fn genesis_preview_agent(
        state: tauri::State<'_, AppState>,
        user_request: String,
        llm_response: String,
    ) -> Result<String, String> {
        super::genesis_preview_agent(state.inner(), user_request, llm_response)
    }

    #[tauri::command]
    fn genesis_create_agent(
        state: tauri::State<'_, AppState>,
        spec_json: String,
        system_prompt: String,
    ) -> Result<String, String> {
        super::genesis_create_agent(state.inner(), spec_json, system_prompt)
    }

    #[tauri::command]
    fn genesis_store_pattern(
        state: tauri::State<'_, AppState>,
        spec_json: String,
        missing_capabilities: Vec<String>,
        test_score: f64,
    ) -> Result<String, String> {
        super::genesis_store_pattern(state.inner(), spec_json, missing_capabilities, test_score)
    }

    #[tauri::command]
    fn genesis_list_generated(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::genesis_list_generated(state.inner())
    }

    #[tauri::command]
    fn genesis_delete_agent(
        state: tauri::State<'_, AppState>,
        agent_name: String,
    ) -> Result<String, String> {
        super::genesis_delete_agent(state.inner(), agent_name)
    }

    // ── Consciousness commands ──────────────────────────────────────────

    #[tauri::command]
    fn get_agent_consciousness(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::get_agent_consciousness(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_user_behavior_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_user_behavior_state(state.inner())
    }

    #[tauri::command]
    fn report_user_keystroke(
        state: tauri::State<'_, AppState>,
        is_deletion: bool,
        timestamp: u64,
    ) -> Result<(), String> {
        super::report_user_keystroke(state.inner(), is_deletion, timestamp)
    }

    #[tauri::command]
    fn get_consciousness_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        limit: u32,
    ) -> Result<String, String> {
        super::get_consciousness_history(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn reset_agent_consciousness(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::reset_agent_consciousness(state.inner(), agent_id)
    }

    // ── Dream Forge commands ────────────────────────────────────────────

    #[tauri::command]
    fn get_dream_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_dream_status(state.inner())
    }

    #[tauri::command]
    fn get_dream_queue(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_dream_queue(state.inner())
    }

    #[tauri::command]
    fn get_morning_briefing(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_morning_briefing(state.inner())
    }

    #[tauri::command]
    fn set_dream_config(
        state: tauri::State<'_, AppState>,
        enabled: bool,
        idle_trigger_minutes: u32,
        budget_tokens: u64,
        budget_calls: u32,
    ) -> Result<(), String> {
        super::set_dream_config(
            state.inner(),
            enabled,
            idle_trigger_minutes,
            budget_tokens,
            budget_calls,
        )
    }

    #[tauri::command]
    fn trigger_dream_now(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::trigger_dream_now(state.inner())
    }

    #[tauri::command]
    fn get_dream_history(state: tauri::State<'_, AppState>, limit: u32) -> Result<String, String> {
        super::get_dream_history(state.inner(), limit)
    }

    // ── Temporal Engine commands ─────────────────────────────────────────

    #[tauri::command]
    fn temporal_fork(
        state: tauri::State<'_, AppState>,
        request: String,
        agent_id: String,
        fork_count: Option<u32>,
    ) -> Result<String, String> {
        super::temporal_fork(state.inner(), request, agent_id, fork_count)
    }

    #[tauri::command]
    fn temporal_select_fork(
        state: tauri::State<'_, AppState>,
        decision_id: String,
        fork_id: String,
    ) -> Result<(), String> {
        super::temporal_select_fork(state.inner(), decision_id, fork_id)
    }

    #[tauri::command]
    fn temporal_rollback(
        state: tauri::State<'_, AppState>,
        decision_id: String,
    ) -> Result<String, String> {
        super::temporal_rollback(state.inner(), decision_id)
    }

    #[tauri::command]
    fn run_dilated_session(
        state: tauri::State<'_, AppState>,
        task: String,
        agent_ids: Vec<String>,
        max_iterations: u32,
    ) -> Result<String, String> {
        super::run_dilated_session(state.inner(), task, agent_ids, max_iterations)
    }

    #[tauri::command]
    fn get_temporal_history(
        state: tauri::State<'_, AppState>,
        limit: u32,
    ) -> Result<String, String> {
        super::get_temporal_history(state.inner(), limit)
    }

    #[tauri::command]
    fn set_temporal_config(
        state: tauri::State<'_, AppState>,
        max_forks: u32,
        eval_strategy: String,
        budget_tokens: u64,
    ) -> Result<(), String> {
        super::set_temporal_config(state.inner(), max_forks, eval_strategy, budget_tokens)
    }

    // ── MCP Host commands ───────────────────────────────────────────────

    #[tauri::command]
    fn mcp_host_list_servers(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::mcp_host_list_servers(state.inner())
    }

    #[tauri::command]
    fn mcp_host_add_server(
        state: tauri::State<'_, AppState>,
        name: String,
        url: String,
        transport: String,
        auth_token: Option<String>,
    ) -> Result<String, String> {
        super::mcp_host_add_server(state.inner(), name, url, transport, auth_token)
    }

    #[tauri::command]
    fn mcp_host_remove_server(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_remove_server(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_connect(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_connect(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_disconnect(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_disconnect(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_list_tools(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::mcp_host_list_tools(state.inner())
    }

    #[tauri::command]
    fn mcp_host_call_tool(
        state: tauri::State<'_, AppState>,
        tool_name: String,
        arguments: String,
    ) -> Result<String, String> {
        super::mcp_host_call_tool(state.inner(), tool_name, arguments)
    }

    // ── Ghost Protocol commands ─────────────────────────────────────────

    #[tauri::command]
    fn ghost_protocol_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_status(state.inner())
    }

    #[tauri::command]
    fn ghost_protocol_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::ghost_protocol_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn ghost_protocol_add_peer(
        state: tauri::State<'_, AppState>,
        address: String,
        name: String,
    ) -> Result<String, String> {
        super::ghost_protocol_add_peer(state.inner(), address, name)
    }

    #[tauri::command]
    fn ghost_protocol_remove_peer(
        state: tauri::State<'_, AppState>,
        device_id: String,
    ) -> Result<String, String> {
        super::ghost_protocol_remove_peer(state.inner(), device_id)
    }

    #[tauri::command]
    fn ghost_protocol_sync_now(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_sync_now(state.inner())
    }

    #[tauri::command]
    fn ghost_protocol_get_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_get_state(state.inner())
    }

    // ── Voice Assistant commands ─────────────────────────────────────

    #[tauri::command]
    fn voice_start_listening(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_start_listening(state.inner())
    }

    #[tauri::command]
    fn voice_stop_listening(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_stop_listening(state.inner())
    }

    #[tauri::command]
    fn voice_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_get_status(state.inner())
    }

    #[tauri::command]
    fn voice_transcribe(
        state: tauri::State<'_, AppState>,
        audio_base64: String,
    ) -> Result<String, String> {
        super::voice_transcribe(state.inner(), audio_base64)
    }

    #[tauri::command]
    fn voice_load_whisper_model(
        state: tauri::State<'_, AppState>,
        model_path: String,
    ) -> Result<String, String> {
        super::voice_load_whisper_model(state.inner(), model_path)
    }

    // ── Software Factory commands ────────────────────────────────────

    #[tauri::command]
    fn factory_create_project(
        state: tauri::State<'_, AppState>,
        name: String,
        language: String,
        source_dir: String,
    ) -> Result<String, String> {
        super::factory_create_project(state.inner(), name, language, source_dir)
    }

    #[tauri::command]
    fn factory_build_project(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_build_project(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_test_project(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_test_project(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_run_pipeline(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_run_pipeline(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_list_projects(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::factory_list_projects(state.inner())
    }

    #[tauri::command]
    fn factory_get_build_history(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_get_build_history(state.inner(), project_id)
    }

    /// Run the Conductor orchestration pipeline with progress events.
    #[tauri::command]
    async fn conduct_build(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        prompt: String,
        output_dir: Option<String>,
        model: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let app_state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();

        std::thread::spawn(move || {
            // Compute output dir
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let out_dir = output_dir.unwrap_or_else(|| format!("{home}/.nexus/builds/{timestamp}"));

            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                // Best-effort: send error back to caller before returning from thread
                let _ = tx.send(Err(format!("failed to create output dir: {e}")));
                return;
            }

            let full_model = model.unwrap_or_else(|| "mistral".to_string());
            let config = match super::load_config() {
                Ok(c) => c,
                Err(e) => {
                    // Best-effort: send error back to caller before returning from thread
                    let _ = tx.send(Err(format!("config error: {e}")));
                    return;
                }
            };
            let prov_config = super::build_provider_config(&config);
            let (provider, model_name) =
                match super::provider_from_prefixed_model(&full_model, &prov_config) {
                    Ok(p) => p,
                    Err(e) => {
                        // Best-effort: send error back to caller before returning from thread
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
            let mut conductor = super::Conductor::new(provider, &model_name);

            // Preview plan and emit
            let request_for_plan = super::UserRequest::new(&prompt, &out_dir);
            let plan = match conductor.preview_plan(&request_for_plan) {
                Ok(p) => p,
                Err(e) => {
                    // Best-effort: send error back to caller before returning from thread
                    let _ = tx.send(Err(format!("planning failed: {e}")));
                    return;
                }
            };
            // Best-effort: emit execution plan to frontend for preview
            let _ = window.emit("conductor:plan", &plan);

            // Run full orchestration
            let request = super::UserRequest::new(&prompt, &out_dir);
            let mut supervisor = app_state
                .supervisor
                .lock()
                .unwrap_or_else(|p| p.into_inner());

            let start = std::time::Instant::now();
            let result = conductor.run(request, &mut supervisor);
            drop(supervisor);

            match result {
                Ok(mut res) => {
                    res.duration_secs = start.elapsed().as_secs_f64();

                    // Best-effort: emit per-agent completion events to frontend
                    let _ = window.emit(
                        "conductor:agent_completed",
                        &serde_json::json!({
                            "agents_used": res.agents_used,
                            "output_files": &res.output_files,
                        }),
                    );

                    // Best-effort: emit conductor finished event to frontend
                    let _ = window.emit("conductor:finished", &res);

                    // Audit log
                    app_state.log_event(
                        uuid::Uuid::nil(),
                        super::EventType::StateChange,
                        serde_json::json!({
                            "source": "conductor",
                            "action": "conduct_build",
                            "status": format!("{:?}", res.status),
                            "agents_used": res.agents_used,
                            "total_fuel_used": res.total_fuel_used,
                            "duration_secs": res.duration_secs,
                        }),
                    );

                    let plan_json = serde_json::to_value(&plan).unwrap_or_default();
                    let result_json = serde_json::to_value(&res).unwrap_or_default();
                    // Best-effort: send result back to caller; thread termination handled by recv
                    let _ = tx.send(Ok(serde_json::json!({
                        "plan": plan_json,
                        "result": result_json,
                    })));
                }
                Err(e) => {
                    // Best-effort: send error back to caller before returning from thread
                    let _ = tx.send(Err(format!("conductor failed: {e}")));
                }
            }
        });

        rx.recv()
            .unwrap_or(Err("Conductor thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn execute_tool(
        state: tauri::State<'_, AppState>,
        tool_json: String,
    ) -> Result<String, String> {
        super::execute_tool(state.inner(), tool_json)
    }

    #[tauri::command]
    fn list_tools() -> Result<String, String> {
        super::list_tools()
    }

    #[tauri::command]
    fn terminal_execute(
        state: tauri::State<'_, AppState>,
        command: String,
        cwd: String,
    ) -> Result<String, String> {
        super::terminal_execute(state.inner(), command, cwd)
    }

    #[tauri::command]
    fn terminal_execute_approved(
        state: tauri::State<'_, AppState>,
        command: String,
        cwd: String,
    ) -> Result<String, String> {
        super::terminal_execute_approved(state.inner(), command, cwd)
    }

    #[tauri::command]
    fn replay_list_bundles(
        state: tauri::State<'_, AppState>,
        agent_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::replay_list_bundles(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn replay_get_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_get_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_verify_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_verify_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_export_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_export_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_toggle_recording(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::replay_toggle_recording(state.inner(), enabled)
    }

    #[tauri::command]
    fn airgap_create_bundle(
        state: tauri::State<'_, AppState>,
        target_os: String,
        target_arch: String,
        output_path: String,
        components: Option<String>,
    ) -> Result<String, String> {
        super::airgap_create_bundle(
            state.inner(),
            target_os,
            target_arch,
            output_path,
            components,
        )
    }

    #[tauri::command]
    fn airgap_validate_bundle(
        state: tauri::State<'_, AppState>,
        bundle_path: String,
    ) -> Result<String, String> {
        super::airgap_validate_bundle(state.inner(), bundle_path)
    }

    #[tauri::command]
    fn airgap_install_bundle(
        state: tauri::State<'_, AppState>,
        bundle_path: String,
        install_dir: String,
    ) -> Result<String, String> {
        super::airgap_install_bundle(state.inner(), bundle_path, install_dir)
    }

    #[tauri::command]
    fn airgap_get_system_info(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::airgap_get_system_info(state.inner())
    }

    #[tauri::command]
    fn reputation_register(
        state: tauri::State<'_, AppState>,
        did: String,
        name: String,
    ) -> Result<String, String> {
        super::reputation_register(state.inner(), did, name)
    }

    #[tauri::command]
    fn reputation_record_task(
        state: tauri::State<'_, AppState>,
        did: String,
        success: bool,
    ) -> Result<String, String> {
        super::reputation_record_task(state.inner(), did, success)
    }

    #[tauri::command]
    fn reputation_rate_agent(
        state: tauri::State<'_, AppState>,
        did: String,
        rater_did: String,
        score: f64,
        comment: Option<String>,
    ) -> Result<String, String> {
        super::reputation_rate_agent(state.inner(), did, rater_did, score, comment)
    }

    #[tauri::command]
    fn reputation_get(state: tauri::State<'_, AppState>, did: String) -> Result<String, String> {
        super::reputation_get(state.inner(), did)
    }

    #[tauri::command]
    fn reputation_top(
        state: tauri::State<'_, AppState>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::reputation_top(state.inner(), limit)
    }

    #[tauri::command]
    fn reputation_export(state: tauri::State<'_, AppState>, did: String) -> Result<String, String> {
        super::reputation_export(state.inner(), did)
    }

    #[tauri::command]
    fn reputation_import(
        state: tauri::State<'_, AppState>,
        json: String,
    ) -> Result<String, String> {
        super::reputation_import(state.inner(), json)
    }

    #[tauri::command]
    fn get_trust_overview(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::TrustOverviewAgent>, String> {
        super::get_trust_overview(state.inner())
    }

    #[tauri::command]
    fn computer_control_capture_screen(
        state: tauri::State<'_, AppState>,
        region: Option<String>,
    ) -> Result<String, String> {
        super::computer_control_capture_screen(state.inner(), region)
    }

    #[tauri::command]
    fn computer_control_execute_action(
        state: tauri::State<'_, AppState>,
        action_json: String,
    ) -> Result<String, String> {
        super::computer_control_execute_action(state.inner(), action_json)
    }

    #[tauri::command]
    fn computer_control_get_history(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::computer_control_get_history(state.inner())
    }

    #[tauri::command]
    fn computer_control_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::computer_control_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn computer_control_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::computer_control_status(state.inner())
    }

    #[tauri::command]
    fn capture_screen(
        state: tauri::State<'_, AppState>,
        region: Option<ScreenRegion>,
    ) -> Result<String, String> {
        super::capture_screen(state.inner(), region)
    }

    #[tauri::command]
    fn analyze_screen(state: tauri::State<'_, AppState>, query: String) -> Result<String, String> {
        super::analyze_screen(state.inner(), query)
    }

    #[tauri::command]
    fn analyze_media_file(
        state: tauri::State<'_, AppState>,
        path: String,
        query: String,
    ) -> Result<String, String> {
        super::analyze_media_file(state.inner(), path, query)
    }

    #[tauri::command]
    fn start_computer_action(
        state: tauri::State<'_, AppState>,
        description: String,
        max_steps: u32,
    ) -> Result<String, String> {
        super::start_computer_action(state.inner(), description, max_steps)
    }

    #[tauri::command]
    fn stop_computer_action(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::stop_computer_action(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_input_control_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<InputControlStatus, String> {
        super::get_input_control_status(state.inner())
    }

    #[tauri::command]
    fn neural_bridge_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::neural_bridge_status(state.inner())
    }

    #[tauri::command]
    fn neural_bridge_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::neural_bridge_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn neural_bridge_ingest(
        state: tauri::State<'_, AppState>,
        source_type: String,
        content: String,
        metadata: serde_json::Value,
    ) -> Result<String, String> {
        super::neural_bridge_ingest(state.inner(), source_type, content, metadata)
    }

    #[tauri::command]
    fn neural_bridge_search(
        state: tauri::State<'_, AppState>,
        query: String,
        time_range: Option<(u64, u64)>,
        source_filter: Option<Vec<String>>,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::neural_bridge_search(state.inner(), query, time_range, source_filter, max_results)
    }

    #[tauri::command]
    fn neural_bridge_delete(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::neural_bridge_delete(state.inner(), id)
    }

    #[tauri::command]
    fn neural_bridge_clear_old(
        state: tauri::State<'_, AppState>,
        before_timestamp: u64,
    ) -> Result<String, String> {
        super::neural_bridge_clear_old(state.inner(), before_timestamp)
    }

    #[tauri::command]
    fn economy_create_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_create_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_get_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_spend(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        amount: f64,
        tx_type: String,
        description: String,
    ) -> Result<String, String> {
        super::economy_spend(state.inner(), agent_id, amount, tx_type, description)
    }

    #[tauri::command]
    fn economy_earn(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        amount: f64,
        description: String,
    ) -> Result<String, String> {
        super::economy_earn(state.inner(), agent_id, amount, description)
    }

    #[tauri::command]
    fn economy_transfer(
        state: tauri::State<'_, AppState>,
        from: String,
        to: String,
        amount: f64,
        description: String,
    ) -> Result<String, String> {
        super::economy_transfer(state.inner(), from, to, amount, description)
    }

    #[tauri::command]
    fn economy_freeze_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_freeze_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_get_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::economy_get_stats(state.inner())
    }

    #[tauri::command]
    #[allow(clippy::too_many_arguments)]
    fn economy_create_contract(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        client_id: String,
        description: String,
        criteria_json: String,
        reward: f64,
        penalty: f64,
        deadline: Option<u64>,
    ) -> Result<String, String> {
        super::economy_create_contract(
            state.inner(),
            agent_id,
            client_id,
            description,
            criteria_json,
            reward,
            penalty,
            deadline,
        )
    }

    #[tauri::command]
    fn economy_complete_contract(
        state: tauri::State<'_, AppState>,
        contract_id: String,
        success: bool,
        evidence: Option<String>,
    ) -> Result<String, String> {
        super::economy_complete_contract(state.inner(), contract_id, success, evidence)
    }

    #[tauri::command]
    fn economy_list_contracts(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_list_contracts(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_dispute_contract(
        state: tauri::State<'_, AppState>,
        contract_id: String,
        reason: String,
    ) -> Result<String, String> {
        super::economy_dispute_contract(state.inner(), contract_id, reason)
    }

    #[tauri::command]
    fn economy_agent_performance(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_agent_performance(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_remember(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        content: String,
        memory_type: String,
        importance: f64,
        tags: Vec<String>,
    ) -> Result<String, String> {
        super::agent_memory_remember(
            state.inner(),
            agent_id,
            content,
            memory_type,
            importance,
            tags,
        )
    }

    #[tauri::command]
    fn agent_memory_recall(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        query: String,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::agent_memory_recall(state.inner(), agent_id, query, max_results)
    }

    #[tauri::command]
    fn agent_memory_recall_by_type(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        memory_type: String,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::agent_memory_recall_by_type(state.inner(), agent_id, memory_type, max_results)
    }

    #[tauri::command]
    fn agent_memory_forget(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        memory_id: String,
    ) -> Result<String, String> {
        super::agent_memory_forget(state.inner(), agent_id, memory_id)
    }

    #[tauri::command]
    fn agent_memory_get_stats(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_get_stats(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_save(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_save(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_clear(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_clear(state.inner(), agent_id)
    }

    #[tauri::command]
    fn tracing_start_trace(
        state: tauri::State<'_, AppState>,
        operation_name: String,
        agent_id: Option<String>,
    ) -> Result<String, String> {
        super::tracing_start_trace(state.inner(), operation_name, agent_id)
    }

    #[tauri::command]
    fn tracing_start_span(
        state: tauri::State<'_, AppState>,
        trace_id: String,
        parent_span_id: String,
        operation_name: String,
        agent_id: Option<String>,
    ) -> Result<String, String> {
        super::tracing_start_span(
            state.inner(),
            trace_id,
            parent_span_id,
            operation_name,
            agent_id,
        )
    }

    #[tauri::command]
    fn tracing_end_span(
        state: tauri::State<'_, AppState>,
        span_id: String,
        status: String,
        error_message: Option<String>,
    ) -> Result<String, String> {
        super::tracing_end_span(state.inner(), span_id, status, error_message)
    }

    #[tauri::command]
    fn tracing_end_trace(
        state: tauri::State<'_, AppState>,
        trace_id: String,
    ) -> Result<String, String> {
        super::tracing_end_trace(state.inner(), trace_id)
    }

    #[tauri::command]
    fn tracing_list_traces(
        state: tauri::State<'_, AppState>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::tracing_list_traces(state.inner(), limit)
    }

    #[tauri::command]
    fn tracing_get_trace(
        state: tauri::State<'_, AppState>,
        trace_id: String,
    ) -> Result<String, String> {
        super::tracing_get_trace(state.inner(), trace_id)
    }

    #[tauri::command]
    fn payment_create_plan(
        state: tauri::State<'_, AppState>,
        name: String,
        price_cents: u64,
        interval: String,
        features: Vec<String>,
    ) -> Result<String, String> {
        super::payment_create_plan(state.inner(), name, price_cents, interval, features)
    }

    #[tauri::command]
    fn payment_list_plans(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::payment_list_plans(state.inner())
    }

    #[tauri::command]
    fn payment_create_invoice(
        state: tauri::State<'_, AppState>,
        plan_id: String,
        buyer_id: String,
    ) -> Result<String, String> {
        super::payment_create_invoice(state.inner(), plan_id, buyer_id)
    }

    #[tauri::command]
    fn payment_pay_invoice(
        state: tauri::State<'_, AppState>,
        invoice_id: String,
    ) -> Result<String, String> {
        super::payment_pay_invoice(state.inner(), invoice_id)
    }

    #[tauri::command]
    fn payment_get_revenue_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::payment_get_revenue_stats(state.inner())
    }

    #[tauri::command]
    fn payment_create_payout(
        state: tauri::State<'_, AppState>,
        developer_id: String,
        agent_id: String,
        amount_cents: u64,
        period: String,
    ) -> Result<String, String> {
        super::payment_create_payout(state.inner(), developer_id, agent_id, amount_cents, period)
    }

    #[tauri::command]
    fn get_compliance_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::ComplianceStatusRow, String> {
        super::get_compliance_status(state.inner())
    }

    #[tauri::command]
    fn get_compliance_agents(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ComplianceAgentRow>, String> {
        super::get_compliance_agents(state.inner())
    }

    #[tauri::command]
    fn get_audit_chain_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::AuditChainStatusRow, String> {
        super::get_audit_chain_status(state.inner())
    }

    #[tauri::command]
    fn get_git_repo_status() -> Result<super::GitRepoStatusRow, String> {
        super::get_git_repo_status()
    }

    #[tauri::command]
    fn verify_governance_invariants(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::verify_governance_invariants(state.inner())
    }

    #[tauri::command]
    fn verify_specific_invariant(
        state: tauri::State<'_, AppState>,
        invariant_name: String,
    ) -> Result<String, String> {
        super::verify_specific_invariant(state.inner(), invariant_name)
    }

    #[tauri::command]
    fn export_compliance_report(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::export_compliance_report(state.inner())
    }

    #[tauri::command]
    fn audit_search(
        state: tauri::State<'_, AppState>,
        query: super::AuditSearchQuery,
    ) -> Result<String, String> {
        super::audit_search(state.inner(), query)
    }

    #[tauri::command]
    fn audit_statistics(
        state: tauri::State<'_, AppState>,
        time_range: String,
    ) -> Result<String, String> {
        super::audit_statistics(state.inner(), time_range)
    }

    #[tauri::command]
    fn audit_verify_chain(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::audit_verify_chain(state.inner())
    }

    #[tauri::command]
    fn audit_export_report(
        state: tauri::State<'_, AppState>,
        format: String,
        time_range: String,
    ) -> Result<String, String> {
        super::audit_export_report(state.inner(), format, time_range)
    }

    #[tauri::command]
    fn compliance_governance_metrics(
        state: tauri::State<'_, AppState>,
        time_range: String,
    ) -> Result<String, String> {
        super::compliance_governance_metrics(state.inner(), time_range)
    }

    #[tauri::command]
    fn compliance_security_events(
        state: tauri::State<'_, AppState>,
        time_range: String,
    ) -> Result<String, String> {
        super::compliance_security_events(state.inner(), time_range)
    }

    #[tauri::command]
    fn file_manager_list(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_list(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_read(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_read(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_write(
        state: tauri::State<'_, AppState>,
        path: String,
        content: String,
    ) -> Result<String, String> {
        super::file_manager_write(state.inner(), path, content)
    }

    #[tauri::command]
    fn file_manager_create_dir(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_create_dir(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_delete(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_delete(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_rename(
        state: tauri::State<'_, AppState>,
        from: String,
        to: String,
    ) -> Result<String, String> {
        super::file_manager_rename(state.inner(), from, to)
    }

    #[tauri::command]
    fn file_manager_home() -> Result<String, String> {
        super::file_manager_home()
    }

    // ── Database Manager commands ──
    #[tauri::command]
    fn db_connect(
        state: tauri::State<'_, AppState>,
        connection_string: String,
    ) -> Result<String, String> {
        super::db_connect(state.inner(), connection_string)
    }

    #[tauri::command]
    fn db_execute_query(
        state: tauri::State<'_, AppState>,
        connection_string: String,
        query: String,
    ) -> Result<String, String> {
        super::db_execute_query(state.inner(), connection_string, query)
    }

    #[tauri::command]
    fn db_list_tables(
        state: tauri::State<'_, AppState>,
        connection_string: String,
    ) -> Result<String, String> {
        super::db_list_tables(state.inner(), connection_string)
    }

    #[tauri::command]
    fn db_export_table(
        state: tauri::State<'_, AppState>,
        connection_string: String,
        table_name: String,
        format: String,
    ) -> Result<String, String> {
        super::db_export_table(state.inner(), connection_string, table_name, format)
    }

    #[tauri::command]
    fn db_disconnect(state: tauri::State<'_, AppState>, db_path: String) -> Result<(), String> {
        super::db_disconnect(state.inner(), db_path)
    }

    // ── API Client commands ──
    #[tauri::command]
    fn api_client_request(
        state: tauri::State<'_, AppState>,
        method: String,
        url: String,
        headers_json: String,
        body: String,
    ) -> Result<String, String> {
        super::api_client_request(state.inner(), method, url, headers_json, body)
    }

    // ── API Client Collections commands ──
    #[tauri::command]
    fn api_client_list_collections() -> Result<String, String> {
        super::api_client_list_collections()
    }

    #[tauri::command]
    fn api_client_save_collections(data_json: String) -> Result<(), String> {
        super::api_client_save_collections(data_json)
    }

    // ── Learning Progress commands ──
    #[tauri::command]
    fn learning_save_progress(data_json: String) -> Result<(), String> {
        super::learning_save_progress(data_json)
    }

    #[tauri::command]
    fn learning_get_progress() -> Result<String, String> {
        super::learning_get_progress()
    }

    #[tauri::command]
    fn learning_execute_challenge(
        challenge_id: String,
        code: String,
        language: String,
    ) -> Result<String, String> {
        super::learning_execute_challenge(challenge_id, code, language)
    }

    // ── Email Client commands ──
    #[tauri::command]
    fn email_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::email_list(state.inner())
    }

    #[tauri::command]
    fn email_save(
        state: tauri::State<'_, AppState>,
        id: String,
        data_json: String,
    ) -> Result<String, String> {
        super::email_save(state.inner(), id, data_json)
    }

    #[tauri::command]
    fn email_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::email_delete(state.inner(), id)
    }

    // ── Email OAuth2 commands ──
    #[tauri::command]
    fn email_start_oauth(
        state: tauri::State<'_, AppState>,
        provider: String,
    ) -> Result<String, String> {
        super::email_start_oauth(state.inner(), provider)
    }

    #[tauri::command]
    fn email_oauth_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::email_oauth_status(state.inner())
    }

    #[tauri::command]
    fn email_fetch_messages(
        state: tauri::State<'_, AppState>,
        provider: String,
        folder: String,
        page: u32,
    ) -> Result<String, String> {
        super::email_fetch_messages(state.inner(), provider, folder, page)
    }

    #[tauri::command]
    fn email_send_message(
        state: tauri::State<'_, AppState>,
        provider: String,
        to: String,
        subject: String,
        body: String,
    ) -> Result<String, String> {
        super::email_send_message(state.inner(), provider, to, subject, body)
    }

    #[tauri::command]
    fn email_search_messages(
        state: tauri::State<'_, AppState>,
        provider: String,
        query: String,
    ) -> Result<String, String> {
        super::email_search_messages(state.inner(), provider, query)
    }

    #[tauri::command]
    fn email_disconnect(
        state: tauri::State<'_, AppState>,
        provider: String,
    ) -> Result<String, String> {
        super::email_disconnect(state.inner(), provider)
    }

    // ── Messaging Platform commands ──
    #[tauri::command]
    fn messaging_connect_platform(
        state: tauri::State<'_, AppState>,
        platform: String,
        token_value: String,
    ) -> Result<String, String> {
        super::messaging_connect_platform(state.inner(), platform, token_value)
    }

    #[tauri::command]
    fn messaging_send(
        state: tauri::State<'_, AppState>,
        platform: String,
        channel: String,
        text: String,
    ) -> Result<String, String> {
        super::messaging_send(state.inner(), platform, channel, text)
    }

    #[tauri::command]
    fn messaging_poll_messages(
        state: tauri::State<'_, AppState>,
        platform: String,
        channel: String,
        last_id: String,
    ) -> Result<String, String> {
        super::messaging_poll_messages(state.inner(), platform, channel, last_id)
    }

    // ── Integration OAuth commands ──
    #[tauri::command]
    fn integration_start_oauth(
        state: tauri::State<'_, AppState>,
        provider_id: String,
    ) -> Result<String, String> {
        super::integration_start_oauth(state.inner(), provider_id)
    }

    // ── Marketplace GitLab search ──
    #[tauri::command]
    fn marketplace_search_gitlab(query: String) -> Result<String, String> {
        super::marketplace_search_gitlab(query)
    }

    // ── Agent Output Panel ──
    #[tauri::command]
    fn get_agent_outputs(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        limit: u32,
    ) -> Result<String, String> {
        super::get_agent_outputs(state.inner(), agent_id, limit)
    }

    // ── Project Manager commands ──
    #[tauri::command]
    fn project_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::project_list(state.inner())
    }

    #[tauri::command]
    fn project_get(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::project_get(state.inner(), id)
    }

    #[tauri::command]
    fn project_save(
        state: tauri::State<'_, AppState>,
        id: String,
        data_json: String,
    ) -> Result<String, String> {
        super::project_save(state.inner(), id, data_json)
    }

    #[tauri::command]
    fn project_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::project_delete(state.inner(), id)
    }

    // ── Notes App commands ──
    #[tauri::command]
    fn notes_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::notes_list(state.inner())
    }

    #[tauri::command]
    fn notes_get(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::notes_get(state.inner(), id)
    }

    #[tauri::command]
    fn notes_save(
        state: tauri::State<'_, AppState>,
        id: String,
        title: String,
        content: String,
        folder_id: String,
        tags_json: String,
    ) -> Result<String, String> {
        super::notes_save(state.inner(), id, title, content, folder_id, tags_json)
    }

    #[tauri::command]
    fn notes_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::notes_delete(state.inner(), id)
    }

    // ── Cognitive Runtime commands ──

    #[tauri::command]
    fn assign_agent_goal(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        goal_description: String,
        priority: u8,
    ) -> Result<String, String> {
        super::assign_agent_goal(state.inner(), agent_id, goal_description, priority)
    }

    /// Execute a goal end-to-end: assign, run cognitive cycles in background,
    /// emit events for steps/phases/completions, handle HITL consent.
    #[tauri::command]
    async fn execute_agent_goal(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
        goal_description: String,
        priority: u8,
    ) -> Result<String, String> {
        let goal_id =
            super::execute_agent_goal(state.inner(), agent_id.clone(), goal_description, priority)?;
        // Spawn the background cognitive loop driver
        super::spawn_cognitive_loop(window, state.inner().clone(), agent_id, goal_id.clone());
        Ok(goal_id)
    }

    /// Start an autonomous agent loop — the agent runs its default goal on
    /// a recurring interval (cron expression). If the agent manifest already
    /// has a schedule and default_goal, those are used automatically. Provide
    /// overrides via `interval_seconds` and `goal_override` to customize.
    #[tauri::command]
    fn start_autonomous_loop(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        interval_seconds: Option<u64>,
        goal_override: Option<String>,
    ) -> Result<(), String> {
        let interval = interval_seconds.unwrap_or(60);
        // Build a cron expression from interval: "0 */N * * * *" (every N minutes) or
        // use seconds-level scheduling for intervals < 60s.
        let cron_expr = if interval < 60 {
            format!("*/{interval} * * * * *") // every N seconds
        } else {
            let mins = (interval / 60).max(1);
            format!("0 */{mins} * * * *") // every N minutes
        };

        let manifest = super::find_manifest(state.inner(), &agent_id);
        let goal = goal_override
            .or_else(|| manifest.as_ref().and_then(|m| m.default_goal.clone()))
            .unwrap_or_else(|| "Execute autonomous task".to_string());
        let description = super::find_manifest_description(state.inner(), &agent_id);

        let full_goal = super::goal_with_manifest_context(&agent_id, &goal, description.as_deref());

        state
            .agent_scheduler
            .register_agent(&agent_id, &cron_expr, &full_goal)
            .map_err(super::agent_error)?;

        Ok(())
    }

    /// Stop an autonomous agent loop (unregister from scheduler).
    #[tauri::command]
    fn stop_autonomous_loop(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        state.agent_scheduler.unregister_agent(&agent_id);
        Ok(())
    }

    #[tauri::command]
    fn stop_agent_goal(state: tauri::State<'_, AppState>, agent_id: String) -> Result<(), String> {
        super::stop_agent_goal(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_agent_cognitive_status(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<serde_json::Value, String> {
        super::get_agent_cognitive_status(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_agent_task_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, String> {
        super::get_agent_task_history(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn get_agent_memories(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        memory_type: Option<String>,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, String> {
        super::get_agent_memories(state.inner(), agent_id, memory_type, limit)
    }

    // ── Self-Evolution commands ──

    #[tauri::command]
    fn get_self_evolution_metrics(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<serde_json::Value, String> {
        super::get_self_evolution_metrics(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_self_evolution_strategies(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<serde_json::Value>, String> {
        super::get_self_evolution_strategies(state.inner(), agent_id)
    }

    #[tauri::command]
    fn trigger_cross_agent_learning(state: tauri::State<'_, AppState>) -> Result<u32, String> {
        super::trigger_cross_agent_learning(state.inner())
    }

    // ── Consent / HITL Approval commands ──

    #[tauri::command]
    fn approve_consent_request(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        consent_id: String,
        approved_by: String,
    ) -> Result<(), String> {
        super::approve_consent_request(state.inner(), consent_id.clone(), approved_by)?;
        // Best-effort: notify frontend that consent was resolved
        let _ = window.emit(
            "consent-resolved",
            serde_json::json!({"consent_id": consent_id, "status": "approved"}),
        );
        Ok(())
    }

    #[tauri::command]
    fn deny_consent_request(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        consent_id: String,
        denied_by: String,
        reason: Option<String>,
    ) -> Result<(), String> {
        super::deny_consent_request(state.inner(), consent_id.clone(), denied_by, reason)?;
        // Best-effort: notify frontend that consent was resolved
        let _ = window.emit(
            "consent-resolved",
            serde_json::json!({"consent_id": consent_id, "status": "denied"}),
        );
        Ok(())
    }

    #[tauri::command]
    fn set_agent_review_mode(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        review_each: bool,
    ) -> Result<(), String> {
        state
            .cognitive_runtime
            .set_review_each_mode(&agent_id, review_each)
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    fn batch_approve_consents(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        goal_id: String,
        approved_by: String,
    ) -> Result<(), String> {
        let consent_ids = super::batch_approve_consents(state.inner(), goal_id, approved_by)?;
        for consent_id in consent_ids {
            // Best-effort: notify frontend of each resolved consent
            let _ = window.emit(
                "consent-resolved",
                serde_json::json!({"consent_id": consent_id, "status": "approved"}),
            );
        }
        Ok(())
    }

    #[tauri::command]
    fn review_consent_batch(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        consent_id: String,
        reviewed_by: String,
    ) -> Result<(), String> {
        super::review_consent_batch(state.inner(), consent_id.clone(), reviewed_by)?;
        // Best-effort: notify frontend that consent entered review-each mode
        let _ = window.emit(
            "consent-resolved",
            serde_json::json!({"consent_id": consent_id, "status": "review_each"}),
        );
        Ok(())
    }

    #[tauri::command]
    fn batch_deny_consents(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        goal_id: String,
        denied_by: String,
        reason: Option<String>,
    ) -> Result<(), String> {
        let consent_ids = super::batch_deny_consents(state.inner(), goal_id, denied_by, reason)?;
        for consent_id in consent_ids {
            // Best-effort: notify frontend of each resolved consent
            let _ = window.emit(
                "consent-resolved",
                serde_json::json!({"consent_id": consent_id, "status": "denied"}),
            );
        }
        Ok(())
    }

    #[tauri::command]
    fn list_pending_consents(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ConsentNotification>, String> {
        super::list_pending_consents(state.inner())
    }

    #[tauri::command]
    fn get_consent_history(
        state: tauri::State<'_, AppState>,
        limit: u32,
    ) -> Result<Vec<super::ConsentNotification>, String> {
        super::get_consent_history(state.inner(), limit)
    }

    #[tauri::command]
    fn hitl_stats(state: tauri::State<'_, AppState>) -> Result<super::HitlStats, String> {
        super::hitl_stats(state.inner())
    }

    #[tauri::command]
    fn create_simulation(
        state: tauri::State<'_, AppState>,
        name: String,
        seed_text: String,
        persona_count: u32,
        max_ticks: u32,
        tick_interval_ms: Option<u64>,
    ) -> Result<String, String> {
        super::create_simulation(
            state.inner(),
            name,
            seed_text,
            persona_count,
            max_ticks,
            tick_interval_ms,
        )
    }

    #[tauri::command]
    fn start_simulation(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        world_id: String,
    ) -> Result<(), String> {
        let observer = Arc::new(TauriSimulationObserver {
            app: window.app_handle().clone(),
            state: state.inner().clone(),
        }) as Arc<dyn SimulationObserver>;
        super::start_simulation_with_observer(state.inner(), world_id, observer)
    }

    #[tauri::command]
    fn pause_simulation(state: tauri::State<'_, AppState>, world_id: String) -> Result<(), String> {
        super::pause_simulation(state.inner(), world_id)
    }

    #[tauri::command]
    fn inject_variable(
        state: tauri::State<'_, AppState>,
        world_id: String,
        key: String,
        value: String,
    ) -> Result<(), String> {
        super::inject_simulation_variable(state.inner(), world_id, key, value)
    }

    #[tauri::command]
    fn get_simulation_status(
        state: tauri::State<'_, AppState>,
        world_id: String,
    ) -> Result<SimulationStatusView, String> {
        super::get_simulation_status(state.inner(), world_id)
    }

    #[tauri::command]
    fn get_simulation_report(
        state: tauri::State<'_, AppState>,
        world_id: String,
    ) -> Result<PredictionReport, String> {
        super::get_simulation_report(state.inner(), world_id)
    }

    #[tauri::command]
    fn chat_with_persona(
        state: tauri::State<'_, AppState>,
        world_id: String,
        persona_id: String,
        message: String,
    ) -> Result<String, String> {
        super::chat_with_simulation_persona(state.inner(), world_id, persona_id, message)
    }

    #[tauri::command]
    fn list_simulations(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<SimulationSummary>, String> {
        super::list_simulations(state.inner())
    }

    #[tauri::command]
    fn run_parallel_simulations(
        state: tauri::State<'_, AppState>,
        seed_text: String,
        variant_count: u32,
    ) -> Result<Vec<PredictionReport>, String> {
        super::run_parallel_simulation_reports(state.inner(), seed_text, variant_count)
    }

    // ── Messaging Gateway commands ──

    #[tauri::command]
    fn get_messaging_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::PlatformStatus>, String> {
        super::get_messaging_status(state.inner())
    }

    #[tauri::command]
    fn set_default_agent(
        state: tauri::State<'_, AppState>,
        user_id: String,
        agent_id: String,
    ) -> Result<(), String> {
        super::set_default_agent(state.inner(), user_id, agent_id)
    }

    // ── Hivemind commands ──

    #[tauri::command]
    fn start_hivemind(
        state: tauri::State<'_, AppState>,
        goal: String,
        agent_ids: Vec<String>,
    ) -> Result<serde_json::Value, String> {
        super::start_hivemind(state.inner(), goal, agent_ids)
    }

    #[tauri::command]
    fn get_hivemind_status(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<serde_json::Value, String> {
        super::get_hivemind_status(state.inner(), session_id)
    }

    #[tauri::command]
    fn cancel_hivemind(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<(), String> {
        super::cancel_hivemind(state.inner(), session_id)
    }

    // ── Immune System ──

    #[tauri::command]
    fn get_immune_status(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
        super::get_immune_status(state.inner())
    }

    #[tauri::command]
    fn get_threat_log(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
        super::get_threat_log(state.inner())
    }

    #[tauri::command]
    fn trigger_immune_scan(state: tauri::State<'_, AppState>) -> Result<(), String> {
        super::trigger_immune_scan(state.inner())
    }

    #[tauri::command]
    fn run_adversarial_session(
        attacker_id: String,
        defender_id: String,
        rounds: u32,
    ) -> Result<serde_json::Value, String> {
        super::run_adversarial_session(attacker_id, defender_id, rounds)
    }

    #[tauri::command]
    fn get_immune_memory() -> Result<serde_json::Value, String> {
        super::get_immune_memory()
    }

    #[tauri::command]
    fn set_privacy_rules(rules: serde_json::Value) -> Result<(), String> {
        super::set_privacy_rules(rules)
    }

    // ── Cognitive Filesystem ──

    #[tauri::command]
    fn cogfs_index_file(path: String) -> Result<(), String> {
        super::cogfs_index_file(path)
    }

    #[tauri::command]
    fn cogfs_query(question: String) -> Result<serde_json::Value, String> {
        super::cogfs_query(question)
    }

    #[tauri::command]
    fn cogfs_get_graph(file_path: String) -> Result<serde_json::Value, String> {
        super::cogfs_get_graph(file_path)
    }

    #[tauri::command]
    fn cogfs_watch_directory(path: String) -> Result<(), String> {
        super::cogfs_watch_directory(path)
    }

    #[tauri::command]
    fn cogfs_get_entities(file_path: String) -> Result<serde_json::Value, String> {
        super::cogfs_get_entities(file_path)
    }

    #[tauri::command]
    fn cogfs_search(query: String, limit: usize) -> Result<serde_json::Value, String> {
        super::cogfs_search(query, limit)
    }

    #[tauri::command]
    fn cogfs_get_context(topic: String) -> Result<serde_json::Value, String> {
        super::cogfs_get_context(topic)
    }

    // ── Civilization ──

    #[tauri::command]
    fn civ_propose_rule(
        proposer_id: String,
        rule_text: String,
    ) -> Result<serde_json::Value, String> {
        super::civ_propose_rule(proposer_id, rule_text)
    }

    #[tauri::command]
    fn civ_vote(agent_id: String, proposal_id: String, vote: bool) -> Result<(), String> {
        super::civ_vote(agent_id, proposal_id, vote)
    }

    #[tauri::command]
    fn civ_get_parliament_status() -> Result<serde_json::Value, String> {
        super::civ_get_parliament_status()
    }

    #[tauri::command]
    fn civ_get_economy_status() -> Result<serde_json::Value, String> {
        super::civ_get_economy_status()
    }

    #[tauri::command]
    fn civ_get_roles() -> Result<serde_json::Value, String> {
        super::civ_get_roles()
    }

    #[tauri::command]
    fn civ_run_election(role: String) -> Result<serde_json::Value, String> {
        super::civ_run_election(role)
    }

    #[tauri::command]
    fn civ_resolve_dispute(
        agent_a: String,
        agent_b: String,
        issue: String,
    ) -> Result<serde_json::Value, String> {
        super::civ_resolve_dispute(agent_a, agent_b, issue)
    }

    #[tauri::command]
    fn civ_get_governance_log(limit: u32) -> Result<serde_json::Value, String> {
        super::civ_get_governance_log(limit)
    }

    // ── Sovereign Identity ──

    #[tauri::command]
    fn identity_get_agent_passport(agent_id: String) -> Result<serde_json::Value, String> {
        super::identity_get_agent_passport(agent_id)
    }

    #[tauri::command]
    fn identity_generate_proof(
        agent_id: String,
        claim: String,
    ) -> Result<serde_json::Value, String> {
        super::identity_generate_proof(agent_id, claim)
    }

    #[tauri::command]
    fn identity_verify_proof(proof: serde_json::Value) -> Result<bool, String> {
        super::identity_verify_proof(proof)
    }

    #[tauri::command]
    fn identity_export_passport(agent_id: String) -> Result<serde_json::Value, String> {
        super::identity_export_passport(agent_id)
    }

    // ── Mesh ──

    #[tauri::command]
    fn mesh_discover_peers() -> Result<serde_json::Value, String> {
        super::mesh_discover_peers()
    }

    #[tauri::command]
    fn mesh_add_peer(address: String) -> Result<(), String> {
        super::mesh_add_peer(address)
    }

    #[tauri::command]
    fn mesh_get_peers() -> Result<serde_json::Value, String> {
        super::mesh_get_peers()
    }

    #[tauri::command]
    fn mesh_migrate_agent(
        agent_id: String,
        target_peer: String,
    ) -> Result<serde_json::Value, String> {
        super::mesh_migrate_agent(agent_id, target_peer)
    }

    #[tauri::command]
    fn mesh_distribute_task(
        task: String,
        agent_ids: Vec<String>,
    ) -> Result<serde_json::Value, String> {
        super::mesh_distribute_task(task, agent_ids)
    }

    #[tauri::command]
    fn mesh_get_sync_status() -> Result<serde_json::Value, String> {
        super::mesh_get_sync_status()
    }

    // ── Self-Rewrite ──

    #[tauri::command]
    fn self_rewrite_analyze(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        super::self_rewrite_analyze(state.inner())
    }

    #[tauri::command]
    fn self_rewrite_suggest_patches(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        super::self_rewrite_suggest_patches(state.inner())
    }

    #[tauri::command]
    fn self_rewrite_preview_patch(
        state: tauri::State<'_, AppState>,
        patch_id: String,
    ) -> Result<serde_json::Value, String> {
        super::self_rewrite_preview_patch(state.inner(), patch_id)
    }

    #[tauri::command]
    fn self_rewrite_test_patch(
        state: tauri::State<'_, AppState>,
        patch_id: String,
    ) -> Result<serde_json::Value, String> {
        super::self_rewrite_test_patch(state.inner(), patch_id)
    }

    #[tauri::command]
    fn self_rewrite_apply_patch(
        state: tauri::State<'_, AppState>,
        patch_id: String,
    ) -> Result<(), String> {
        super::self_rewrite_apply_patch(state.inner(), patch_id)
    }

    #[tauri::command]
    fn self_rewrite_rollback(
        state: tauri::State<'_, AppState>,
        patch_id: String,
    ) -> Result<(), String> {
        super::self_rewrite_rollback(state.inner(), patch_id)
    }

    #[tauri::command]
    fn self_rewrite_get_history() -> Result<serde_json::Value, String> {
        super::self_rewrite_get_history()
    }

    // ── Self-Improvement Pipeline ──

    #[tauri::command]
    fn self_improve_get_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_status(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_signals(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_signals(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_opportunities(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_opportunities(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_proposals(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_proposals(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_history(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_history(state.inner())
    }

    #[tauri::command]
    fn self_improve_run_cycle(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_run_cycle(state.inner())
    }

    #[tauri::command]
    fn self_improve_approve_proposal(
        state: tauri::State<'_, AppState>,
        proposal_id: String,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_approve_proposal(state.inner(), proposal_id)
    }

    #[tauri::command]
    fn self_improve_reject_proposal(
        state: tauri::State<'_, AppState>,
        proposal_id: String,
        reason: String,
    ) -> Result<(), String> {
        commands::self_improvement::self_improve_reject_proposal(state.inner(), proposal_id, reason)
    }

    #[tauri::command]
    fn self_improve_rollback(
        state: tauri::State<'_, AppState>,
        improvement_id: String,
    ) -> Result<(), String> {
        commands::self_improvement::self_improve_rollback(state.inner(), improvement_id)
    }

    #[tauri::command]
    fn self_improve_get_invariants(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_invariants(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_config(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_config(state.inner())
    }

    #[tauri::command]
    fn self_improve_update_config(
        state: tauri::State<'_, AppState>,
        config: commands::self_improvement::SelfImproveConfig,
    ) -> Result<(), String> {
        commands::self_improvement::self_improve_update_config(state.inner(), config)
    }

    // ── Omniscience ──

    #[tauri::command]
    fn omniscience_get_screen_context() -> Result<serde_json::Value, String> {
        super::omniscience_get_screen_context()
    }

    #[tauri::command]
    fn omniscience_get_predictions() -> Result<serde_json::Value, String> {
        super::omniscience_get_predictions()
    }

    #[tauri::command]
    fn omniscience_enable(interval_ms: u64) -> Result<(), String> {
        super::omniscience_enable(interval_ms)
    }

    #[tauri::command]
    fn omniscience_disable() -> Result<(), String> {
        super::omniscience_disable()
    }

    #[tauri::command]
    fn omniscience_execute_action(action: serde_json::Value) -> Result<serde_json::Value, String> {
        super::omniscience_execute_action(action)
    }

    #[tauri::command]
    fn omniscience_get_app_context(app_name: String) -> Result<serde_json::Value, String> {
        super::omniscience_get_app_context(app_name)
    }

    // ── Consciousness Heatmap ──

    #[tauri::command]
    fn get_consciousness_heatmap(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        super::get_consciousness_heatmap(state.inner())
    }

    // ── Self-Improving OS ──

    #[tauri::command]
    fn get_os_fitness(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_os_fitness(state.inner())
    }

    #[tauri::command]
    fn get_fitness_history(state: tauri::State<'_, AppState>, days: u32) -> Result<String, String> {
        super::get_fitness_history(state.inner(), days)
    }

    #[tauri::command]
    fn get_routing_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_routing_stats(state.inner())
    }

    #[tauri::command]
    fn get_ui_adaptations(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_ui_adaptations(state.inner())
    }

    #[tauri::command]
    fn get_user_profile(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_user_profile(state.inner())
    }

    #[tauri::command]
    fn record_page_visit(state: tauri::State<'_, AppState>, page: String) -> Result<(), String> {
        super::record_page_visit(state.inner(), page)
    }

    #[tauri::command]
    fn record_feature_use(
        state: tauri::State<'_, AppState>,
        feature: String,
    ) -> Result<(), String> {
        super::record_feature_use(state.inner(), feature)
    }

    #[tauri::command]
    fn override_security_block(
        state: tauri::State<'_, AppState>,
        event_id: String,
        rule_id: String,
    ) -> Result<(), String> {
        super::override_security_block(state.inner(), event_id, rule_id)
    }

    #[tauri::command]
    fn get_os_improvement_log(
        state: tauri::State<'_, AppState>,
        limit: u32,
    ) -> Result<String, String> {
        super::get_os_improvement_log(state.inner(), limit)
    }

    #[tauri::command]
    fn get_morning_os_briefing(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_morning_os_briefing(state.inner())
    }

    #[tauri::command]
    fn record_routing_outcome(
        state: tauri::State<'_, AppState>,
        category: String,
        agent_id: String,
        score: f64,
    ) -> Result<(), String> {
        super::record_routing_outcome(state.inner(), category, agent_id, score)
    }

    #[tauri::command]
    fn record_operation_timing(
        state: tauri::State<'_, AppState>,
        operation: String,
        latency_ms: f64,
    ) -> Result<(), String> {
        super::record_operation_timing(state.inner(), operation, latency_ms)
    }

    #[tauri::command]
    fn get_performance_report(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_performance_report(state.inner())
    }

    #[tauri::command]
    fn get_security_evolution_report(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_security_evolution_report(state.inner())
    }

    #[tauri::command]
    fn record_knowledge_interaction(
        state: tauri::State<'_, AppState>,
        topic: String,
        languages: Vec<String>,
        score: f64,
    ) -> Result<(), String> {
        super::record_knowledge_interaction(state.inner(), topic, languages, score)
    }

    #[tauri::command]
    fn get_os_dream_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_os_dream_status(state.inner())
    }

    #[tauri::command]
    fn set_self_improve_enabled(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<(), String> {
        super::set_self_improve_enabled(state.inner(), enabled)
    }

    // ── Killer Features: Screenshot Clone ──

    #[tauri::command]
    fn screenshot_analyze(
        state: tauri::State<'_, AppState>,
        image_path: String,
    ) -> Result<String, String> {
        super::screenshot_analyze(state.inner(), image_path)
    }

    #[tauri::command]
    fn screenshot_generate_spec(
        state: tauri::State<'_, AppState>,
        analysis_json: String,
        project_name: String,
    ) -> Result<String, String> {
        super::screenshot_generate_spec(state.inner(), analysis_json, project_name)
    }

    // ── Killer Features: Voice Project ──

    #[tauri::command]
    fn voice_project_start(state: tauri::State<'_, AppState>) -> Result<(), String> {
        super::voice_project_start(state.inner())
    }

    #[tauri::command]
    fn voice_project_stop(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_project_stop(state.inner())
    }

    #[tauri::command]
    fn voice_project_add_chunk(
        state: tauri::State<'_, AppState>,
        text: String,
        timestamp: u64,
    ) -> Result<(), String> {
        super::voice_project_add_chunk(state.inner(), text, timestamp)
    }

    #[tauri::command]
    fn voice_project_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_project_get_status(state.inner())
    }

    #[tauri::command]
    fn voice_project_get_prompt(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_project_get_prompt(state.inner())
    }

    #[tauri::command]
    fn voice_project_update_intent(
        state: tauri::State<'_, AppState>,
        response: String,
        timestamp: u64,
    ) -> Result<String, String> {
        super::voice_project_update_intent(state.inner(), response, timestamp)
    }

    // ── Killer Features: Stress Test ──

    #[tauri::command]
    fn stress_generate_personas(
        state: tauri::State<'_, AppState>,
        count: u32,
    ) -> Result<String, String> {
        super::stress_generate_personas(state.inner(), count)
    }

    #[tauri::command]
    fn stress_generate_actions(
        state: tauri::State<'_, AppState>,
        persona_json: String,
    ) -> Result<String, String> {
        super::stress_generate_actions(state.inner(), persona_json)
    }

    #[tauri::command]
    fn stress_evaluate_report(
        state: tauri::State<'_, AppState>,
        report_json: String,
    ) -> Result<String, String> {
        super::stress_evaluate_report(state.inner(), report_json)
    }

    // ── Killer Features: Deploy ──

    #[tauri::command]
    fn deploy_generate_dockerfile(
        state: tauri::State<'_, AppState>,
        config_json: String,
    ) -> Result<String, String> {
        super::deploy_generate_dockerfile(state.inner(), config_json)
    }

    #[tauri::command]
    fn deploy_validate_config(
        state: tauri::State<'_, AppState>,
        config_json: String,
    ) -> Result<String, String> {
        super::deploy_validate_config(state.inner(), config_json)
    }

    #[tauri::command]
    fn deploy_get_commands(
        state: tauri::State<'_, AppState>,
        config_json: String,
    ) -> Result<String, String> {
        super::deploy_get_commands(state.inner(), config_json)
    }

    // ── Killer Features: Live Evolution ──

    #[tauri::command]
    fn evolver_register_app(
        state: tauri::State<'_, AppState>,
        app_json: String,
    ) -> Result<(), String> {
        super::evolver_register_app(state.inner(), app_json)
    }

    #[tauri::command]
    fn evolver_unregister_app(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<bool, String> {
        super::evolver_unregister_app(state.inner(), project_id)
    }

    #[tauri::command]
    fn evolver_list_apps(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::evolver_list_apps(state.inner())
    }

    #[tauri::command]
    fn evolver_detect_issues(
        state: tauri::State<'_, AppState>,
        metrics_json: String,
    ) -> Result<String, String> {
        super::evolver_detect_issues(state.inner(), metrics_json)
    }

    // ── Killer Features: Freelance Engine ──

    #[tauri::command]
    fn freelance_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::freelance_get_status(state.inner())
    }

    #[tauri::command]
    fn freelance_start_scanning(state: tauri::State<'_, AppState>) -> Result<(), String> {
        super::freelance_start_scanning(state.inner())
    }

    #[tauri::command]
    fn freelance_stop_scanning(state: tauri::State<'_, AppState>) -> Result<(), String> {
        super::freelance_stop_scanning(state.inner())
    }

    #[tauri::command]
    fn freelance_evaluate_job(
        state: tauri::State<'_, AppState>,
        job_json: String,
    ) -> Result<String, String> {
        super::freelance_evaluate_job(state.inner(), job_json)
    }

    #[tauri::command]
    fn freelance_get_revenue(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::freelance_get_revenue(state.inner())
    }

    // Experience Layer commands
    #[tauri::command]
    fn start_conversational_build(
        state: tauri::State<'_, AppState>,
        message: String,
    ) -> Result<String, String> {
        super::start_conversational_build(state.inner(), message)
    }

    #[tauri::command]
    fn builder_respond(
        state: tauri::State<'_, AppState>,
        message: String,
    ) -> Result<String, String> {
        super::builder_respond(state.inner(), message)
    }

    #[tauri::command]
    fn get_live_preview(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::get_live_preview(state.inner(), project_id)
    }

    #[tauri::command]
    fn remix_project(
        state: tauri::State<'_, AppState>,
        project_id: String,
        change: String,
    ) -> Result<String, String> {
        super::remix_project(state.inner(), project_id, change)
    }

    #[tauri::command]
    fn analyze_problem(
        state: tauri::State<'_, AppState>,
        problem: String,
    ) -> Result<String, String> {
        super::analyze_problem(state.inner(), problem)
    }

    #[tauri::command]
    fn publish_to_marketplace(
        state: tauri::State<'_, AppState>,
        project_id: String,
        pricing: String,
    ) -> Result<String, String> {
        super::publish_to_marketplace(state.inner(), project_id, pricing)
    }

    #[tauri::command]
    fn install_from_marketplace(
        state: tauri::State<'_, AppState>,
        listing_id: String,
    ) -> Result<String, String> {
        super::install_from_marketplace(state.inner(), listing_id)
    }

    #[tauri::command]
    fn start_teach_mode(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::start_teach_mode(state.inner(), project_id)
    }

    #[tauri::command]
    fn teach_mode_respond(
        state: tauri::State<'_, AppState>,
        project_id: String,
        response: String,
    ) -> Result<String, String> {
        super::teach_mode_respond(state.inner(), project_id, response)
    }

    #[tauri::command]
    fn backup_create(
        state: tauri::State<'_, AppState>,
        include_audit: bool,
        include_genomes: bool,
        include_config: bool,
        encrypt: bool,
    ) -> Result<String, String> {
        super::backup_create(
            state.inner(),
            include_audit,
            include_genomes,
            include_config,
            encrypt,
        )
    }

    #[tauri::command]
    fn backup_restore(
        state: tauri::State<'_, AppState>,
        archive_path: String,
    ) -> Result<String, String> {
        super::backup_restore(state.inner(), archive_path)
    }

    #[tauri::command]
    fn backup_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::backup_list(state.inner())
    }

    #[tauri::command]
    fn backup_verify(
        state: tauri::State<'_, AppState>,
        archive_path: String,
    ) -> Result<String, String> {
        super::backup_verify(state.inner(), archive_path)
    }

    // ── Admin Console Commands ──

    #[tauri::command]
    fn admin_overview(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::admin_overview(state.inner())
    }

    #[tauri::command]
    fn admin_users_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::admin_users_list(state.inner())
    }

    #[tauri::command]
    fn admin_user_create(
        state: tauri::State<'_, AppState>,
        email: String,
        name: String,
        role: String,
    ) -> Result<String, String> {
        super::admin_user_create(state.inner(), email, name, role)
    }

    #[tauri::command]
    fn admin_user_update_role(
        state: tauri::State<'_, AppState>,
        user_id: String,
        role: String,
    ) -> Result<(), String> {
        super::admin_user_update_role(state.inner(), user_id, role)
    }

    #[tauri::command]
    fn admin_user_deactivate(
        state: tauri::State<'_, AppState>,
        user_id: String,
    ) -> Result<(), String> {
        super::admin_user_deactivate(state.inner(), user_id)
    }

    #[tauri::command]
    fn admin_fleet_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::admin_fleet_status(state.inner())
    }

    #[tauri::command]
    fn admin_agent_stop_all(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
    ) -> Result<u32, String> {
        super::admin_agent_stop_all(state.inner(), workspace_id)
    }

    #[tauri::command]
    fn admin_agent_bulk_update(
        state: tauri::State<'_, AppState>,
        agent_dids: Vec<String>,
        update: String,
    ) -> Result<String, String> {
        super::admin_agent_bulk_update(state.inner(), agent_dids, update)
    }

    #[tauri::command]
    fn admin_policy_get(
        state: tauri::State<'_, AppState>,
        scope: String,
    ) -> Result<String, String> {
        super::admin_policy_get(state.inner(), scope)
    }

    #[tauri::command]
    fn admin_policy_update(
        state: tauri::State<'_, AppState>,
        scope: String,
        policy: String,
    ) -> Result<(), String> {
        super::admin_policy_update(state.inner(), scope, policy)
    }

    #[tauri::command]
    fn admin_policy_history(
        state: tauri::State<'_, AppState>,
        scope: String,
    ) -> Result<String, String> {
        super::admin_policy_history(state.inner(), scope)
    }

    #[tauri::command]
    fn admin_compliance_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::admin_compliance_status(state.inner())
    }

    #[tauri::command]
    fn admin_compliance_export(
        state: tauri::State<'_, AppState>,
        format: String,
    ) -> Result<String, String> {
        super::admin_compliance_export(state.inner(), format)
    }

    #[tauri::command]
    fn admin_system_health(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::admin_system_health(state.inner())
    }

    #[tauri::command]
    fn integrations_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::integrations_list(state.inner())
    }

    #[tauri::command]
    fn integration_test(
        state: tauri::State<'_, AppState>,
        provider_id: String,
    ) -> Result<String, String> {
        super::integration_test(state.inner(), &provider_id)
    }

    #[tauri::command]
    fn integration_configure(
        state: tauri::State<'_, AppState>,
        provider_id: String,
        settings: serde_json::Value,
    ) -> Result<String, String> {
        super::integration_configure(state.inner(), &provider_id, settings)
    }

    // ── Auth commands ──

    #[tauri::command]
    fn auth_login(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::auth_login(state.inner())
    }

    #[tauri::command]
    fn auth_session_info(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<String, String> {
        super::auth_session_info(state.inner(), session_id)
    }

    #[tauri::command]
    fn auth_logout(state: tauri::State<'_, AppState>, session_id: String) -> Result<(), String> {
        super::auth_logout(state.inner(), session_id)
    }

    #[tauri::command]
    fn auth_config_get(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::auth_config_get(state.inner())
    }

    // ── Workspace commands ──

    #[tauri::command]
    fn workspace_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::workspace_list(state.inner())
    }

    #[tauri::command]
    fn workspace_create(state: tauri::State<'_, AppState>, name: String) -> Result<String, String> {
        super::workspace_create(state.inner(), name)
    }

    #[tauri::command]
    fn workspace_get(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
    ) -> Result<String, String> {
        super::workspace_get(state.inner(), workspace_id)
    }

    #[tauri::command]
    fn workspace_add_member(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        user_id: String,
        role: String,
    ) -> Result<(), String> {
        super::workspace_add_member(state.inner(), workspace_id, user_id, role)
    }

    #[tauri::command]
    fn workspace_remove_member(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        user_id: String,
    ) -> Result<(), String> {
        super::workspace_remove_member(state.inner(), workspace_id, user_id)
    }

    #[tauri::command]
    fn workspace_set_policy(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        policy_json: String,
    ) -> Result<(), String> {
        super::workspace_set_policy(state.inner(), workspace_id, policy_json)
    }

    #[tauri::command]
    fn workspace_usage(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
    ) -> Result<String, String> {
        super::workspace_usage(state.inner(), workspace_id)
    }

    // ── Telemetry commands ──

    #[tauri::command]
    fn telemetry_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::telemetry_status(state.inner())
    }

    #[tauri::command]
    fn telemetry_health(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::telemetry_health(state.inner())
    }

    #[tauri::command]
    fn telemetry_config_get(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::telemetry_config_get(state.inner())
    }

    #[tauri::command]
    fn telemetry_config_update(
        state: tauri::State<'_, AppState>,
        config_json: String,
    ) -> Result<(), String> {
        super::telemetry_config_update(state.inner(), config_json)
    }

    // ── Metering commands ──

    #[tauri::command]
    fn metering_usage_report(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        period: String,
    ) -> Result<String, String> {
        super::metering_usage_report(state.inner(), workspace_id, period)
    }

    #[tauri::command]
    fn metering_cost_breakdown(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        period: String,
    ) -> Result<String, String> {
        super::metering_cost_breakdown(state.inner(), workspace_id, period)
    }

    #[tauri::command]
    fn metering_export_csv(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        period: String,
    ) -> Result<String, String> {
        super::metering_export_csv(state.inner(), workspace_id, period)
    }

    #[tauri::command]
    fn metering_set_budget_alert(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
        threshold: f64,
    ) -> Result<(), String> {
        super::metering_set_budget_alert(state.inner(), workspace_id, threshold)
    }

    #[tauri::command]
    fn metering_budget_alerts(
        state: tauri::State<'_, AppState>,
        workspace_id: String,
    ) -> Result<String, String> {
        super::metering_budget_alerts(state.inner(), workspace_id)
    }

    #[tauri::command]
    fn get_rate_limit_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        use nexus_kernel::rate_limit::RateCategory;
        let categories = [
            RateCategory::Default,
            RateCategory::LlmRequest,
            RateCategory::AgentExecute,
            RateCategory::AuditExport,
            RateCategory::BackupCreate,
            RateCategory::AdminOperation,
        ];
        let mut status = serde_json::Map::new();
        for cat in &categories {
            let info = state.rate_limiter.remaining(*cat, "desktop");
            status.insert(
                cat.to_string(),
                serde_json::to_value(&info).unwrap_or_default(),
            );
        }
        serde_json::to_string(&status).map_err(|e| format!("serialize: {e}"))
    }

    pub fn run() {
        let builder = tauri::Builder::<tauri::Wry>::default()
            .plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcuts([Shortcut::new(
                        Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
                        Code::KeyK,
                    )])
                    .unwrap_or_else(|e| {
                        eprintln!("FATAL: failed to register emergency kill shortcut: {e}");
                        std::process::exit(1);
                    })
                    .with_handler(|app: &tauri::AppHandle<tauri::Wry>, _shortcut, event| {
                        if event.state != ShortcutState::Pressed {
                            return;
                        }

                        activate_emergency_kill_switch();

                        let state = app.state::<AppState>();
                        {
                            let sessions = state
                                .computer_action_cancellations
                                .lock()
                                .unwrap_or_else(|p| p.into_inner());
                            for cancelled in sessions.values() {
                                cancelled.store(true, Ordering::SeqCst);
                            }
                        }
                        {
                            let mut engine = state
                                .computer_control
                                .lock()
                                .unwrap_or_else(|p| p.into_inner());
                            engine.disable();
                        }

                        state.log_event(
                            Uuid::nil(),
                            EventType::UserAction,
                            json!({
                                "source": "computer-control",
                                "event": "EmergencyKillSwitch activated",
                                "shortcut": "Ctrl+Alt+Shift+K",
                            }),
                        );
                        // Best-effort: notify frontend of kill switch activation
                        let _ = app.emit(
                            "input-kill-switch-activated",
                            json!({
                                "shortcut": "Ctrl+Alt+Shift+K",
                            }),
                        );
                        show_desktop_notification(
                            "All agent input control stopped by emergency kill switch",
                        );
                    })
                    .build(),
            )
            .manage(AppState::new());

        let builder = builder.setup(|app| {
            let state = app.state::<AppState>();
            state.set_app_handle(app.handle().clone());
            state
                .agent_scheduler
                .set_executor(Arc::new(ScheduledGoalExecutor {
                    state: state.inner().clone(),
                }));
            // Set up the background schedule runner callback
            state
                .schedule_runner
                .set_goal_callback(Arc::new(RunnerGoalCallback {
                    state: state.inner().clone(),
                }));

            // Defer heavy agent loading so the window appears immediately.
            let state_clone = state.inner().clone();
            let runner_clone = state.schedule_runner.clone();
            tauri::async_runtime::spawn(async move {
                state_clone.load_agents_deferred();
                state_clone.initialize_startup_schedules();

                // Seed schedules from agent manifests that have schedule + default_goal
                seed_manifests_to_runner(&state_clone);

                // Start the background schedule runner
                eprintln!("[startup] launching background schedule runner");
                runner_clone.run().await;
            });

            #[cfg(not(target_os = "linux"))]
            {
                let show_dashboard =
                    MenuItem::with_id(app, "show_dashboard", "Show Dashboard", true, None::<&str>)?;
                let start_voice =
                    MenuItem::with_id(app, "start_voice", "Start Voice", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_dashboard, &start_voice, &quit])?;

                TrayIconBuilder::new()
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show_dashboard" => {
                            if let Some(window) = app.get_webview_window("main") {
                                // Best-effort: show and focus the main window from tray menu
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "start_voice" => {
                            let state = app.state::<AppState>();
                            // Best-effort: start voice assistant from tray menu
                            let _ = super::start_jarvis_mode(state.inner());
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Down,
                            ..
                        } = event
                        {
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                // Best-effort: show and focus the main window on tray click
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        });

        builder
            .invoke_handler(tauri::generate_handler![
                list_agents,
                create_agent,
                start_agent,
                stop_agent,
                clear_all_agents,
                get_scheduled_agents,
                get_preinstalled_agents,
                pause_agent,
                resume_agent,
                get_audit_log,
                send_chat,
                get_agent_performance,
                get_auto_evolution_log,
                set_auto_evolution_config,
                force_evolve_agent,
                get_config,
                save_config,
                start_jarvis_mode,
                stop_jarvis_mode,
                jarvis_status,
                transcribe_push_to_talk,
                tray_status,
                detect_hardware,
                check_ollama,
                pull_ollama_model,
                pull_model,
                ensure_ollama,
                is_ollama_installed,
                delete_model,
                is_setup_complete,
                run_setup_wizard,
                list_available_models,
                list_provider_models,
                get_provider_status,
                get_available_providers,
                save_api_key,
                chat_with_ollama,
                set_agent_model,
                check_llm_status,
                get_llm_recommendations,
                set_agent_llm_provider,
                get_provider_usage_stats,
                test_llm_connection,
                get_system_info,
                get_agent_permissions,
                update_agent_permission,
                get_permission_history,
                get_capability_request,
                bulk_update_permissions,
                get_protocols_status,
                get_protocols_requests,
                get_mcp_tools,
                get_agent_cards,
                a2a_discover_agent,
                a2a_send_task,
                a2a_get_task_status,
                a2a_cancel_task,
                a2a_known_agents,
                get_agent_identity,
                list_identities,
                get_firewall_status,
                get_firewall_patterns,
                marketplace_search,
                marketplace_install,
                marketplace_info,
                marketplace_publish,
                marketplace_my_agents,
                start_learning,
                learning_agent_action,
                get_learning_session,
                get_knowledge_base,
                navigate_to,
                get_browser_history,
                get_agent_activity,
                start_research,
                research_agent_action,
                complete_research,
                get_research_session,
                list_research_sessions,
                start_build,
                build_append_code,
                build_add_message,
                complete_build,
                get_build_session,
                get_build_code,
                get_build_preview,
                policy_list,
                policy_validate,
                policy_test,
                policy_detect_conflicts,
                index_document,
                search_documents,
                chat_with_documents,
                list_indexed_documents,
                remove_indexed_document,
                get_document_governance,
                get_semantic_map,
                get_document_access_log,
                get_active_llm_provider,
                search_models,
                get_model_info,
                check_model_compatibility,
                download_model,
                list_local_models,
                delete_local_model,
                get_system_specs,
                get_live_system_metrics,
                time_machine_list_checkpoints,
                time_machine_get_checkpoint,
                time_machine_create_checkpoint,
                time_machine_undo,
                time_machine_undo_checkpoint,
                time_machine_redo,
                time_machine_get_diff,
                time_machine_what_if,
                nexus_link_status,
                nexus_link_toggle_sharing,
                nexus_link_add_peer,
                nexus_link_remove_peer,
                nexus_link_list_peers,
                nexus_link_send_model,
                evolution_get_status,
                evolution_register_strategy,
                evolution_evolve_once,
                evolution_get_history,
                evolution_rollback,
                evolution_get_active_strategy,
                mcp_host_list_servers,
                mcp_host_add_server,
                mcp_host_remove_server,
                mcp_host_connect,
                mcp_host_disconnect,
                mcp_host_list_tools,
                mcp_host_call_tool,
                ghost_protocol_status,
                ghost_protocol_toggle,
                ghost_protocol_add_peer,
                ghost_protocol_remove_peer,
                ghost_protocol_sync_now,
                ghost_protocol_get_state,
                voice_start_listening,
                voice_stop_listening,
                voice_get_status,
                voice_transcribe,
                voice_load_whisper_model,
                factory_create_project,
                factory_build_project,
                factory_test_project,
                factory_run_pipeline,
                factory_list_projects,
                factory_get_build_history,
                conduct_build,
                execute_tool,
                list_tools,
                terminal_execute,
                terminal_execute_approved,
                replay_list_bundles,
                replay_get_bundle,
                replay_verify_bundle,
                replay_export_bundle,
                replay_toggle_recording,
                airgap_create_bundle,
                airgap_validate_bundle,
                airgap_install_bundle,
                airgap_get_system_info,
                reputation_register,
                reputation_record_task,
                reputation_rate_agent,
                reputation_get,
                reputation_top,
                reputation_export,
                reputation_import,
                get_trust_overview,
                computer_control_capture_screen,
                computer_control_execute_action,
                computer_control_get_history,
                computer_control_toggle,
                computer_control_status,
                capture_screen,
                analyze_screen,
                analyze_media_file,
                start_computer_action,
                stop_computer_action,
                get_input_control_status,
                neural_bridge_status,
                neural_bridge_toggle,
                neural_bridge_ingest,
                neural_bridge_search,
                neural_bridge_delete,
                neural_bridge_clear_old,
                economy_create_wallet,
                economy_get_wallet,
                economy_spend,
                economy_earn,
                economy_transfer,
                economy_freeze_wallet,
                economy_get_history,
                economy_get_stats,
                economy_create_contract,
                economy_complete_contract,
                economy_list_contracts,
                economy_dispute_contract,
                economy_agent_performance,
                agent_memory_remember,
                agent_memory_recall,
                agent_memory_recall_by_type,
                agent_memory_forget,
                agent_memory_get_stats,
                agent_memory_save,
                agent_memory_clear,
                tracing_start_trace,
                tracing_start_span,
                tracing_end_span,
                tracing_end_trace,
                tracing_list_traces,
                tracing_get_trace,
                payment_create_plan,
                payment_list_plans,
                payment_create_invoice,
                payment_pay_invoice,
                payment_get_revenue_stats,
                payment_create_payout,
                get_compliance_status,
                get_compliance_agents,
                get_audit_chain_status,
                get_git_repo_status,
                verify_governance_invariants,
                verify_specific_invariant,
                export_compliance_report,
                audit_search,
                audit_statistics,
                audit_verify_chain,
                audit_export_report,
                compliance_governance_metrics,
                compliance_security_events,
                file_manager_list,
                file_manager_read,
                file_manager_write,
                file_manager_create_dir,
                file_manager_delete,
                file_manager_rename,
                file_manager_home,
                db_connect,
                db_execute_query,
                db_list_tables,
                db_export_table,
                db_disconnect,
                api_client_request,
                api_client_list_collections,
                api_client_save_collections,
                learning_save_progress,
                learning_get_progress,
                learning_execute_challenge,
                notes_list,
                notes_get,
                notes_save,
                notes_delete,
                email_list,
                email_save,
                email_delete,
                email_start_oauth,
                email_oauth_status,
                email_fetch_messages,
                email_send_message,
                email_search_messages,
                email_disconnect,
                messaging_connect_platform,
                messaging_send,
                messaging_poll_messages,
                integration_start_oauth,
                marketplace_search_gitlab,
                get_agent_outputs,
                project_list,
                project_get,
                project_save,
                project_delete,
                assign_agent_goal,
                execute_agent_goal,
                start_autonomous_loop,
                stop_autonomous_loop,
                stop_agent_goal,
                get_agent_cognitive_status,
                get_agent_task_history,
                get_agent_memories,
                get_self_evolution_metrics,
                get_self_evolution_strategies,
                trigger_cross_agent_learning,
                approve_consent_request,
                deny_consent_request,
                set_agent_review_mode,
                batch_approve_consents,
                review_consent_batch,
                batch_deny_consents,
                list_pending_consents,
                get_consent_history,
                hitl_stats,
                create_simulation,
                start_simulation,
                pause_simulation,
                inject_variable,
                get_simulation_status,
                get_simulation_report,
                chat_with_persona,
                list_simulations,
                run_parallel_simulations,
                start_hivemind,
                get_hivemind_status,
                cancel_hivemind,
                get_messaging_status,
                set_default_agent,
                get_agent_genome,
                breed_agents,
                mutate_agent,
                get_agent_lineage,
                generate_all_genomes,
                evolve_population,
                genesis_analyze_gap,
                genesis_preview_agent,
                genesis_create_agent,
                genesis_store_pattern,
                genesis_list_generated,
                genesis_delete_agent,
                get_agent_consciousness,
                get_user_behavior_state,
                report_user_keystroke,
                get_consciousness_history,
                reset_agent_consciousness,
                get_dream_status,
                get_dream_queue,
                get_morning_briefing,
                set_dream_config,
                trigger_dream_now,
                get_dream_history,
                temporal_fork,
                temporal_select_fork,
                temporal_rollback,
                run_dilated_session,
                get_temporal_history,
                set_temporal_config,
                // Systems 5-11
                get_immune_status,
                get_threat_log,
                trigger_immune_scan,
                run_adversarial_session,
                get_immune_memory,
                set_privacy_rules,
                cogfs_index_file,
                cogfs_query,
                cogfs_get_graph,
                cogfs_watch_directory,
                cogfs_get_entities,
                cogfs_search,
                cogfs_get_context,
                civ_propose_rule,
                civ_vote,
                civ_get_parliament_status,
                civ_get_economy_status,
                civ_get_roles,
                civ_run_election,
                civ_resolve_dispute,
                civ_get_governance_log,
                identity_get_agent_passport,
                identity_generate_proof,
                identity_verify_proof,
                identity_export_passport,
                mesh_discover_peers,
                mesh_add_peer,
                mesh_get_peers,
                mesh_migrate_agent,
                mesh_distribute_task,
                mesh_get_sync_status,
                self_rewrite_analyze,
                self_rewrite_suggest_patches,
                self_rewrite_preview_patch,
                self_rewrite_test_patch,
                self_rewrite_apply_patch,
                self_rewrite_rollback,
                self_rewrite_get_history,
                omniscience_get_screen_context,
                omniscience_get_predictions,
                omniscience_enable,
                omniscience_disable,
                omniscience_execute_action,
                omniscience_get_app_context,
                get_consciousness_heatmap,
                // Self-Improving OS
                get_os_fitness,
                get_fitness_history,
                get_routing_stats,
                get_ui_adaptations,
                get_user_profile,
                record_page_visit,
                record_feature_use,
                override_security_block,
                get_os_improvement_log,
                get_morning_os_briefing,
                record_routing_outcome,
                record_operation_timing,
                get_performance_report,
                get_security_evolution_report,
                record_knowledge_interaction,
                get_os_dream_status,
                set_self_improve_enabled,
                // Self-Improvement Pipeline
                self_improve_get_status,
                self_improve_get_signals,
                self_improve_get_opportunities,
                self_improve_get_proposals,
                self_improve_get_history,
                self_improve_run_cycle,
                self_improve_approve_proposal,
                self_improve_reject_proposal,
                self_improve_rollback,
                self_improve_get_invariants,
                self_improve_get_config,
                self_improve_update_config,
                // Killer Features
                screenshot_analyze,
                screenshot_generate_spec,
                voice_project_start,
                voice_project_stop,
                voice_project_add_chunk,
                voice_project_get_status,
                voice_project_get_prompt,
                voice_project_update_intent,
                stress_generate_personas,
                stress_generate_actions,
                stress_evaluate_report,
                deploy_generate_dockerfile,
                deploy_validate_config,
                deploy_get_commands,
                evolver_register_app,
                evolver_unregister_app,
                evolver_list_apps,
                evolver_detect_issues,
                freelance_get_status,
                freelance_start_scanning,
                freelance_stop_scanning,
                freelance_evaluate_job,
                freelance_get_revenue,
                // Experience Layer
                start_conversational_build,
                builder_respond,
                get_live_preview,
                remix_project,
                analyze_problem,
                publish_to_marketplace,
                install_from_marketplace,
                start_teach_mode,
                teach_mode_respond,
                // Backup & Restore
                backup_create,
                backup_restore,
                backup_list,
                backup_verify,
                // Rate Limiting
                get_rate_limit_status,
                // Admin Console
                admin_overview,
                admin_users_list,
                admin_user_create,
                admin_user_update_role,
                admin_user_deactivate,
                admin_fleet_status,
                admin_agent_stop_all,
                admin_agent_bulk_update,
                admin_policy_get,
                admin_policy_update,
                admin_policy_history,
                admin_compliance_status,
                admin_compliance_export,
                admin_system_health,
                integrations_list,
                integration_test,
                integration_configure,
                // Enterprise: Auth
                auth_login,
                auth_session_info,
                auth_logout,
                auth_config_get,
                // Enterprise: Workspaces
                workspace_list,
                workspace_create,
                workspace_get,
                workspace_add_member,
                workspace_remove_member,
                workspace_set_policy,
                workspace_usage,
                // Enterprise: Telemetry
                telemetry_status,
                telemetry_health,
                telemetry_config_get,
                telemetry_config_update,
                // Enterprise: Metering
                metering_usage_report,
                metering_cost_breakdown,
                metering_export_csv,
                metering_set_budget_alert,
                metering_budget_alerts,
                // Background Scheduler
                crate::commands::orchestration::scheduler_create,
                crate::commands::orchestration::scheduler_list,
                crate::commands::orchestration::scheduler_enable,
                crate::commands::orchestration::scheduler_disable,
                crate::commands::orchestration::scheduler_delete,
                crate::commands::orchestration::scheduler_history,
                crate::commands::orchestration::scheduler_trigger_now,
                crate::commands::orchestration::scheduler_runner_status,
                crate::commands::orchestration::execute_team_workflow,
                crate::commands::orchestration::transfer_agent_fuel,
                crate::commands::orchestration::run_content_pipeline,
                // Flash Inference
                crate::commands::flash::flash_detect_hardware,
                crate::commands::flash::flash_profile_model,
                crate::commands::flash::flash_auto_configure,
                crate::commands::flash::flash_create_session,
                crate::commands::flash::flash_generate,
                crate::commands::flash::flash_list_sessions,
                crate::commands::flash::flash_unload_session,
                crate::commands::flash::flash_clear_sessions,
                crate::commands::flash::flash_get_metrics,
                crate::commands::flash::flash_system_metrics,
                crate::commands::flash::flash_estimate_performance,
                crate::commands::flash::flash_run_benchmark,
                crate::commands::flash::flash_export_benchmark_report,
                crate::commands::flash::flash_enable_speculative,
                crate::commands::flash::flash_disable_speculative,
                crate::commands::flash::flash_speculative_status,
                crate::commands::flash::flash_catalog_recommend,
                crate::commands::flash::flash_catalog_search,
                crate::commands::flash::flash_list_local_models,
                crate::commands::flash::flash_download_model,
                crate::commands::flash::flash_download_multi,
                crate::commands::flash::flash_delete_local_model,
                crate::commands::flash::flash_available_disk_space,
                crate::commands::flash::flash_get_model_dir,
                // Capability Measurement
                crate::commands::crate_bridges::cm_start_session,
                crate::commands::crate_bridges::cm_get_session,
                crate::commands::crate_bridges::cm_get_scorecard,
                crate::commands::crate_bridges::cm_list_sessions,
                crate::commands::crate_bridges::cm_get_profile,
                crate::commands::crate_bridges::cm_get_gaming_flags,
                crate::commands::crate_bridges::cm_compare_agents,
                crate::commands::crate_bridges::cm_get_batteries,
                crate::commands::crate_bridges::cm_trigger_feedback,
                crate::commands::crate_bridges::cm_evaluate_response,
                crate::commands::crate_bridges::cm_get_boundary_map,
                crate::commands::crate_bridges::cm_get_calibration,
                crate::commands::crate_bridges::cm_get_census,
                crate::commands::crate_bridges::cm_get_gaming_report_batch,
                crate::commands::crate_bridges::cm_upload_darwin,
                crate::commands::crate_bridges::cm_execute_validation_run,
                crate::commands::crate_bridges::cm_list_validation_runs,
                crate::commands::crate_bridges::cm_get_validation_run,
                crate::commands::crate_bridges::cm_three_way_comparison,
                crate::commands::crate_bridges::cm_run_ab_validation,
                // Predictive Router
                crate::commands::crate_bridges::router_route_task,
                crate::commands::crate_bridges::router_record_outcome,
                crate::commands::crate_bridges::router_get_accuracy,
                crate::commands::crate_bridges::router_get_models,
                crate::commands::crate_bridges::router_estimate_difficulty,
                crate::commands::crate_bridges::router_get_feedback,
                // Browser Agent
                crate::commands::crate_bridges::browser_create_session,
                crate::commands::crate_bridges::browser_execute_task,
                crate::commands::crate_bridges::browser_navigate,
                crate::commands::crate_bridges::browser_screenshot,
                crate::commands::crate_bridges::browser_get_content,
                crate::commands::crate_bridges::browser_close_session,
                crate::commands::crate_bridges::browser_get_policy,
                crate::commands::crate_bridges::browser_session_count,
                // Governance Oracle
                crate::commands::crate_bridges::oracle_status,
                crate::commands::crate_bridges::oracle_verify_token,
                crate::commands::crate_bridges::oracle_get_agent_budget,
                // Token Economy
                crate::commands::crate_bridges::token_get_wallet,
                crate::commands::crate_bridges::token_get_all_wallets,
                crate::commands::crate_bridges::token_create_wallet,
                crate::commands::crate_bridges::token_get_ledger,
                crate::commands::crate_bridges::token_get_supply,
                crate::commands::crate_bridges::token_calculate_burn,
                crate::commands::crate_bridges::token_calculate_reward,
                crate::commands::crate_bridges::token_calculate_spawn,
                crate::commands::crate_bridges::token_create_delegation,
                crate::commands::crate_bridges::token_get_delegations,
                crate::commands::crate_bridges::token_get_pricing,
                // Governed Computer Control
                crate::commands::crate_bridges::cc_execute_action,
                crate::commands::crate_bridges::cc_get_action_history,
                crate::commands::crate_bridges::cc_get_capability_budget,
                crate::commands::crate_bridges::cc_verify_action_sequence,
                crate::commands::crate_bridges::cc_get_screen_context,
                // World Simulation + Perception
                crate::commands::crate_bridges::sim_submit,
                crate::commands::crate_bridges::sim_run,
                crate::commands::crate_bridges::sim_get_result,
                crate::commands::crate_bridges::sim_get_history,
                crate::commands::crate_bridges::sim_get_policy,
                crate::commands::crate_bridges::sim_get_risk,
                crate::commands::crate_bridges::sim_branch,
                crate::commands::crate_bridges::perception_init,
                crate::commands::crate_bridges::perception_describe,
                crate::commands::crate_bridges::perception_extract_text,
                crate::commands::crate_bridges::perception_question,
                crate::commands::crate_bridges::perception_find_ui_elements,
                crate::commands::crate_bridges::perception_extract_data,
                crate::commands::crate_bridges::perception_read_error,
                crate::commands::crate_bridges::perception_analyze_chart,
                crate::commands::crate_bridges::perception_get_policy,
                // Agent Memory + Tools
                crate::commands::crate_bridges::memory_store_entry,
                crate::commands::crate_bridges::memory_query_entries,
                crate::commands::crate_bridges::memory_get_entry,
                crate::commands::crate_bridges::memory_delete_entry,
                crate::commands::crate_bridges::memory_build_context,
                crate::commands::crate_bridges::memory_get_stats,
                crate::commands::crate_bridges::memory_consolidate,
                crate::commands::crate_bridges::memory_save,
                crate::commands::crate_bridges::memory_load,
                crate::commands::crate_bridges::memory_list_agents,
                crate::commands::crate_bridges::memory_get_policy,
                crate::commands::crate_bridges::tools_list_available,
                crate::commands::crate_bridges::tools_execute,
                crate::commands::crate_bridges::tools_get_registry,
                crate::commands::crate_bridges::tools_refresh_availability,
                crate::commands::crate_bridges::tools_get_audit,
                crate::commands::crate_bridges::tools_verify_audit,
                crate::commands::crate_bridges::tools_get_policy,
                // Collaboration + Software Factory
                crate::commands::crate_bridges::collab_create_session,
                crate::commands::crate_bridges::collab_add_participant,
                crate::commands::crate_bridges::collab_start,
                crate::commands::crate_bridges::collab_send_message,
                crate::commands::crate_bridges::collab_call_vote,
                crate::commands::crate_bridges::collab_cast_vote,
                crate::commands::crate_bridges::collab_declare_consensus,
                crate::commands::crate_bridges::collab_detect_consensus,
                crate::commands::crate_bridges::collab_get_session,
                crate::commands::crate_bridges::collab_list_active,
                crate::commands::crate_bridges::collab_get_policy,
                crate::commands::crate_bridges::collab_get_patterns,
                crate::commands::crate_bridges::swf_create_project,
                crate::commands::crate_bridges::swf_assign_member,
                crate::commands::crate_bridges::swf_start_pipeline,
                crate::commands::crate_bridges::swf_submit_artifact,
                crate::commands::crate_bridges::swf_get_project,
                crate::commands::crate_bridges::swf_list_projects,
                crate::commands::crate_bridges::swf_get_cost,
                crate::commands::crate_bridges::swf_get_policy,
                crate::commands::crate_bridges::swf_get_pipeline_stages,
                crate::commands::crate_bridges::swf_estimate_cost,
                // MCP Standalone
                crate::commands::crate_bridges::mcp2_server_status,
                crate::commands::crate_bridges::mcp2_server_handle,
                crate::commands::crate_bridges::mcp2_server_list_tools,
                crate::commands::crate_bridges::mcp2_client_add,
                crate::commands::crate_bridges::mcp2_client_remove,
                crate::commands::crate_bridges::mcp2_client_discover,
                crate::commands::crate_bridges::mcp2_client_call,
                // Governance Engine + Evolution
                crate::commands::crate_bridges::governance_engine_get_rules,
                crate::commands::crate_bridges::governance_engine_evaluate,
                crate::commands::crate_bridges::governance_engine_get_audit_log,
                crate::commands::crate_bridges::governance_evolution_get_threat_model,
                crate::commands::crate_bridges::governance_evolution_run_attack_cycle,
                // A2A Crate
                crate::commands::crate_bridges::a2a_crate_get_agent_card,
                crate::commands::crate_bridges::a2a_crate_list_skills,
                crate::commands::crate_bridges::a2a_crate_send_task,
                crate::commands::crate_bridges::a2a_crate_get_task,
                crate::commands::crate_bridges::a2a_crate_discover_agent,
                crate::commands::crate_bridges::a2a_crate_get_status,
                // Migration Tool
                crate::commands::crate_bridges::migrate_preview,
                crate::commands::crate_bridges::migrate_execute,
                crate::commands::crate_bridges::migrate_supported_sources,
                crate::commands::crate_bridges::migrate_report,
                // Memory Kernel
                crate::commands::crate_bridges::mk_get_stats,
                crate::commands::crate_bridges::mk_query,
                crate::commands::crate_bridges::mk_search,
                crate::commands::crate_bridges::mk_get_audit,
                crate::commands::crate_bridges::mk_get_procedures,
                crate::commands::crate_bridges::mk_get_candidates,
                crate::commands::crate_bridges::mk_write,
                crate::commands::crate_bridges::mk_clear_working,
                crate::commands::crate_bridges::mk_share,
                crate::commands::crate_bridges::mk_revoke_share,
                crate::commands::crate_bridges::mk_run_gc,
                crate::commands::crate_bridges::mk_create_checkpoint,
                crate::commands::crate_bridges::mk_rollback,
                crate::commands::crate_bridges::mk_list_checkpoints,
            ])
            .run(tauri::generate_context!())
            .unwrap_or_else(|e| {
                eprintln!("FATAL: Nexus OS failed to start: {e}");
                std::process::exit(1);
            });
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
