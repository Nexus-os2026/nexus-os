#![allow(unexpected_cfgs)]
#![allow(unused_imports)]
mod commands;
mod nx_bridge;
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
use nexus_connectors_llm::streaming::StreamingLlmProvider;
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

/// Well-known UUID for UI / system-initiated actions that have no specific agent
/// or authenticated user.  Using a deterministic value (UUIDv5 in the DNS
/// namespace for "nexus-os-system") instead of `SYSTEM_UUID` so the audit trail
/// can distinguish "system action" from "missing identity".
pub(crate) const SYSTEM_UUID: Uuid = Uuid::from_bytes([
    0x4e, 0x58, 0x53, 0x59, 0x53, 0x2d, 0x00, 0x01, 0x80, 0x00, 0x4e, 0x45, 0x58, 0x55, 0x53, 0x00,
]);

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

/// Replace the `:root { ... }` CSS block in an HTML string with new CSS.
/// Returns the updated HTML, or the original if no `:root` block is found.
/// Extract the raw `:root { ... }` block content from HTML, returning (start, end)
/// byte offsets so the caller can splice.
fn find_root_block(html: &str) -> Option<(usize, usize)> {
    let root_start = html.find(":root {")?;
    let after = &html[root_start..];
    let mut depth = 0u32;
    for (idx, ch) in after.char_indices() {
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let root_end = root_start + idx + 1;
                return Some((root_start, root_end));
            }
        }
    }
    None
}

/// Extract CSS custom-property names declared inside a `:root { ... }` block.
fn extract_root_var_names(root_block: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in root_block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("--") {
            if let Some(colon) = rest.find(':') {
                let name = rest[..colon].trim();
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Build alias declarations that map common LLM short-form variable names
/// to the structured TokenSet names, so `var(--accent)` resolves even after
/// the `:root` block is replaced with TokenSet CSS.
///
/// Only emits aliases for names that actually appeared in the *original* HTML
/// (i.e. the LLM-generated `:root` block) to avoid bloat.
fn build_compat_aliases(original_var_names: &[String]) -> String {
    // Map: LLM short name → TokenSet structured name (as CSS var reference)
    static ALIAS_MAP: &[(&str, &str)] = &[
        // Colors
        ("accent", "var(--color-accent, var(--color-primary))"),
        ("accent-hover", "var(--color-accent)"),
        ("accent-h", "var(--color-accent)"),
        ("primary", "var(--color-primary)"),
        ("secondary", "var(--color-secondary)"),
        ("bg", "var(--color-bg)"),
        ("background", "var(--color-bg)"),
        ("surface", "var(--color-bg-secondary)"),
        ("text", "var(--color-text)"),
        ("text-secondary", "var(--color-text-secondary)"),
        ("text-muted", "var(--color-text-secondary)"),
        ("muted", "var(--color-text-secondary)"),
        ("border", "var(--color-border)"),
        ("ghost", "var(--color-bg-secondary)"),
        ("outline", "var(--color-border)"),
        // Typography
        ("font-display", "var(--font-heading)"),
        ("ff-display", "var(--font-heading)"),
        ("ff-body", "var(--font-body)"),
        ("fh", "var(--font-heading)"),
        ("fb", "var(--font-body)"),
        // Radius
        ("radius", "var(--radius-md)"),
        ("radius-pill", "var(--radius-full, 9999px)"),
        ("radius-card", "var(--radius-lg)"),
        ("r", "var(--radius-md)"),
        ("rc", "var(--radius-lg)"),
        // Misc
        ("transition", "0.3s ease"),
        ("ease", "cubic-bezier(0.4,0,0.2,1)"),
        ("section-pad", "var(--space-xl, 4rem)"),
        ("shadow", "var(--shadow-md, 0 4px 6px rgba(0,0,0,0.1))"),
    ];

    let mut css = String::new();
    for name in original_var_names {
        // Skip names that are already structured TokenSet names (no alias needed)
        if name.starts_with("color-")
            || name.starts_with("btn-")
            || name.starts_with("hero-")
            || name.starts_with("nav-")
            || name.starts_with("footer-")
            || name.starts_with("card-")
            || name.starts_with("space-")
            || name.starts_with("duration-")
        {
            continue;
        }
        if let Some((_, val)) = ALIAS_MAP.iter().find(|(k, _)| *k == name.as_str()) {
            use std::fmt::Write;
            let _ = writeln!(css, "  --{name}: {val};");
        }
    }
    css
}

fn replace_root_css(html: &str, new_css: &str) -> String {
    if let Some((root_start, root_end)) = find_root_block(html) {
        // Extract original variable names so we can generate compat aliases
        let old_block = &html[root_start..root_end];
        let original_names = extract_root_var_names(old_block);
        let aliases = build_compat_aliases(&original_names);

        if aliases.is_empty() {
            return format!("{}{}{}", &html[..root_start], new_css, &html[root_end..]);
        }
        // Inject aliases into the new CSS right before the closing `}`
        let injected = if let Some(close) = new_css.rfind('}') {
            format!(
                "{}\n  /* Compat aliases for LLM-generated variable names */\n{}{}",
                &new_css[..close],
                aliases,
                &new_css[close..],
            )
        } else {
            format!(
                "{}\n/* Compat aliases */\n:root {{\n{}}}\n",
                new_css, aliases
            )
        };
        return format!("{}{}{}", &html[..root_start], injected, &html[root_end..]);
    }
    html.to_string()
}

/// Update the `:root {}` block in a project's `current/index.html` with new token CSS.
fn persist_token_css_to_html(project_dir: &std::path::Path, token_css: &str) {
    let html_path = project_dir.join("current").join("index.html");
    if html_path.exists() {
        if let Ok(html) = std::fs::read_to_string(&html_path) {
            let updated = replace_root_css(&html, token_css);
            if updated != html {
                let _ = std::fs::write(&html_path, &updated);
            }
        }
    }
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
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
                Uuid::parse_str(&progress.world_id).unwrap_or(SYSTEM_UUID),
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
    async fn send_chat(
        state: tauri::State<'_, AppState>,
        message: String,
        model_id: Option<String>,
        agent_name: Option<String>,
    ) -> Result<ChatResponse, String> {
        let state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = super::send_chat(&state, message, model_id, agent_name);
            let _ = tx.send(result);
        });
        rx.recv()
            .unwrap_or(Err("Chat thread terminated unexpectedly".to_string()))
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

    #[tauri::command]
    fn detect_claude_code_cli() -> Result<super::ClaudeCodeCliStatus, String> {
        Ok(super::detect_claude_code_cli())
    }

    #[tauri::command]
    fn trigger_claude_code_login() -> Result<String, String> {
        super::trigger_claude_code_login()
    }

    #[tauri::command]
    fn detect_codex_cli() -> Result<super::CodexCliCliStatus, String> {
        Ok(super::detect_codex_cli_cmd())
    }

    #[tauri::command]
    fn trigger_codex_cli_login() -> Result<String, String> {
        super::trigger_codex_cli_login()
    }

    #[tauri::command]
    fn load_llm_provider_settings() -> Result<super::LlmProviderSettings, String> {
        super::load_llm_provider_settings(false)
    }

    #[tauri::command]
    fn save_llm_provider_settings(settings: super::LlmProviderSettings) -> Result<(), String> {
        super::save_llm_provider_settings(settings)
    }

    #[tauri::command]
    fn detect_cli_provider(provider_id: String) -> Result<super::CliProviderSetting, String> {
        super::detect_single_cli_provider(&provider_id)
    }

    #[tauri::command]
    fn auto_detect_all_enabled() -> Result<super::LlmProviderSettings, String> {
        super::auto_detect_enabled_providers()
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
    async fn test_llm_connection(
        provider_name: String,
    ) -> Result<super::TestConnectionResult, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = super::test_llm_connection(provider_name);
            let _ = tx.send(result);
        });
        rx.recv().unwrap_or(Err(
            "Test connection thread terminated unexpectedly".to_string()
        ))
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

            let full_model =
                model.unwrap_or_else(|| "openrouter/qwen/qwen3.6-plus:free".to_string());
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
            eprintln!("[conductor] Creating conductor with model={model_name}, provider prefix={full_model}");
            let mut conductor = super::Conductor::new(provider, &model_name);

            // Preview plan and emit
            let request_for_plan = super::UserRequest::new(&prompt, &out_dir);
            eprintln!("[conductor] Running planner...");
            let plan = match conductor.preview_plan(&request_for_plan) {
                Ok(p) => {
                    eprintln!("[conductor] Plan ready: {} tasks", p.tasks.len());
                    p
                }
                Err(e) => {
                    eprintln!("[conductor] PLANNING FAILED: {e}");
                    let _ = tx.send(Err(format!("planning failed (model={model_name}): {e}")));
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

            eprintln!("[conductor] Running orchestration...");
            let start = std::time::Instant::now();
            let result = conductor.run(request, &mut supervisor);
            drop(supervisor);
            eprintln!(
                "[conductor] Orchestration finished in {:.1}s: {:?}",
                start.elapsed().as_secs_f64(),
                result
                    .as_ref()
                    .map(|r| format!("{:?}, {} files", r.status, r.output_files.len()))
                    .unwrap_or_else(|e| format!("ERROR: {e}"))
            );

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
                        SYSTEM_UUID,
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

    /// Run a streaming web build with real-time progress events on `build-stream`.
    ///
    /// Uses the streaming LLM API (currently Anthropic only) to emit progress
    /// events as tokens are generated. Falls back to non-streaming if the
    /// selected model/provider doesn't support streaming.
    #[tauri::command]
    async fn conduct_build_streaming(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        prompt: String,
        output_dir: Option<String>,
        model: Option<String>,
        approved_plan: Option<String>,
        acceptance_criteria: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let app_state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();

        std::thread::spawn(move || {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let out_dir = output_dir.unwrap_or_else(|| format!("{home}/.nexus/builds/{timestamp}"));

            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                let _ = tx.send(Err(format!("failed to create output dir: {e}")));
                return;
            }

            let config = match super::load_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("config error: {e}")));
                    return;
                }
            };
            let prov_config = super::build_provider_config(&config);

            // Load user's model config once — used for full_build and classification steps.
            let model_cfg = web_builder_agent::model_config::load_config();
            let config_source = if std::path::Path::new(&std::env::var("HOME").unwrap_or_default())
                .join(".nexus/builder_model_config.json")
                .exists()
            {
                "user config"
            } else {
                "default \u{2014} no user config found"
            };

            // Auto-select build model from user's saved model config (or smart defaults).
            let full_model = model.unwrap_or_else(|| {
                let choice = &model_cfg.full_build;

                if choice.is_none() {
                    eprintln!("[conductor-stream] WARNING: No models configured for full build");
                    return "claude-sonnet-4-6".to_string(); // ultimate fallback
                }

                let prefixed = web_builder_agent::model_config::to_prefixed_model(choice);
                eprintln!(
                    "[conductor-stream] Step \"full_build\" using model: {} (from {})",
                    choice.display_name, config_source,
                );
                prefixed
            });

            // Try to get a streaming provider for the selected model.
            // Uses prefixed model name so provider selection routes to the correct CLI/API.
            let streaming_result =
                super::streaming_provider_from_prefixed_model(&full_model, &prov_config);

            if let Ok((streaming_provider, model_name)) = streaming_result {
                // Create conductor with a non-streaming provider for fallback
                let (conductor_provider, _) =
                    match super::provider_from_prefixed_model(&full_model, &prov_config) {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            return;
                        }
                    };
                let mut conductor = super::Conductor::new(conductor_provider, &model_name);

                // If an approved plan is provided, augment the prompt
                let effective_prompt = match (&approved_plan, &acceptance_criteria) {
                    (Some(plan_json), Some(criteria_json)) => {
                        match (
                            serde_json::from_str::<web_builder_agent::plan::ProductBrief>(
                                plan_json,
                            ),
                            serde_json::from_str::<web_builder_agent::plan::AcceptanceCriteria>(
                                criteria_json,
                            ),
                        ) {
                            (Ok(brief), Ok(criteria)) => {
                                eprintln!(
                                    "[conductor-stream] Using approved plan: {}",
                                    brief.project_name
                                );

                                // Phase 2: Classify into a template skeleton
                                // Use the planning/classification model from user config
                                let class_choice = &model_cfg.planning;
                                let class_prefixed =
                                    web_builder_agent::model_config::to_prefixed_model(
                                        class_choice,
                                    );
                                let class_model_id = class_choice.model_id.clone();
                                eprintln!(
                                    "[conductor-stream] Step \"planning_classification\" using model: {} (from {})",
                                    class_choice.display_name, config_source,
                                );

                                // Create provider from user config; fall back to model router on failure
                                let (class_provider, effective_class_model): (
                                    Box<dyn nexus_connectors_llm::providers::LlmProvider>,
                                    String,
                                ) = match super::provider_from_prefixed_model(
                                    &class_prefixed,
                                    &prov_config,
                                ) {
                                    Ok((p, _m)) => (p, class_model_id),
                                    Err(e) => {
                                        eprintln!(
                                                "[conductor-stream] Config provider failed for classification: {}, falling back to router",
                                                e
                                            );
                                        let class_budget = web_builder_agent::model_router::RoutingBudget::from_budget_tracker();
                                        let class_selection = web_builder_agent::model_router::select_model(
                                                &web_builder_agent::model_router::BuilderTask::TemplateClassification,
                                                &class_budget,
                                            );
                                        let fb: Box<dyn nexus_connectors_llm::providers::LlmProvider> = match class_selection.provider {
                                                web_builder_agent::model_router::ProviderType::Ollama => {
                                                    Box::new(nexus_connectors_llm::providers::OllamaProvider::from_env())
                                                }
                                                web_builder_agent::model_router::ProviderType::OpenAI => {
                                                    Box::new(super::OpenAiProvider::new(prov_config.openai_api_key.clone()))
                                                }
                                                _ => {
                                                    let has_key = prov_config.anthropic_api_key.as_deref()
                                                        .map(|k| !k.trim().is_empty()).unwrap_or(false);
                                                    if has_key {
                                                        Box::new(super::ClaudeProvider::new(prov_config.anthropic_api_key.clone()))
                                                    } else {
                                                        let status = nexus_connectors_llm::providers::claude_code::detect_claude_code();
                                                        if status.installed && status.authenticated {
                                                            Box::new(nexus_connectors_llm::providers::claude_code::ClaudeCodeProvider::new())
                                                        } else {
                                                            Box::new(super::ClaudeProvider::new(prov_config.anthropic_api_key.clone()))
                                                        }
                                                    }
                                                }
                                            };
                                        (fb, class_selection.model_id.clone())
                                    }
                                };
                                let selection = web_builder_agent::classifier::classify_with_model(
                                    class_provider.as_ref(),
                                    &prompt,
                                    &brief,
                                    &effective_class_model,
                                );

                                // Persist template selection to artefacts
                                if let Err(e) =
                                    web_builder_agent::classifier::save_selection_artefact(
                                        std::path::Path::new(&out_dir),
                                        &selection,
                                    )
                                {
                                    eprintln!("[conductor-stream] Warning: failed to save template_selection.json: {e}");
                                }

                                // Save selected_template to builder_state
                                if !selection.template_id.is_empty() {
                                    let tmpl_path = std::path::Path::new(&out_dir);
                                    if let Ok(mut ps) =
                                        web_builder_agent::project::load_project_state(tmpl_path)
                                    {
                                        ps.selected_template = Some(selection.template_id.clone());
                                        let _ = web_builder_agent::project::save_project_state(
                                            tmpl_path, &ps,
                                        );
                                    }
                                }

                                let template_html = if !selection.template_id.is_empty() {
                                    eprintln!(
                                        "[conductor-stream] Template: {} (confidence: {:.2}, modifiers: {:?})",
                                        selection.template_id, selection.confidence, selection.modifiers
                                    );
                                    if let Some(tmpl) = web_builder_agent::templates::get_template(
                                        &selection.template_id,
                                    ) {
                                        let html = web_builder_agent::templates::modifiers::apply_modifiers(
                                            tmpl.html, &selection.modifiers,
                                        );
                                        Some(html)
                                    } else {
                                        None
                                    }
                                } else {
                                    eprintln!("[conductor-stream] No template matched, generating from scratch");
                                    None
                                };

                                web_builder_agent::plan::build_planned_prompt_with_template(
                                    &prompt,
                                    &brief,
                                    &criteria,
                                    template_html.as_deref(),
                                )
                            }
                            _ => {
                                eprintln!("[conductor-stream] Warning: failed to parse plan JSON, using raw prompt");
                                prompt.clone()
                            }
                        }
                    }
                    _ => prompt.clone(),
                };

                // Create a single web-build task
                let role = nexus_conductor::types::AgentRole::WebBuilder;
                let task = nexus_conductor::types::PlannedTask {
                    role: role.clone(),
                    description: effective_prompt,
                    expected_outputs: vec!["index.html".to_string()],
                    estimated_fuel: 50_000,
                    depends_on: vec![],
                    capabilities_needed: role.default_capabilities(),
                };

                let mut audit = nexus_kernel::audit::AuditTrail::new();
                let agent_id = uuid::Uuid::new_v4();
                let output_path = std::path::Path::new(&out_dir);

                // Emit events via the Tauri window, capturing build cost
                let window_ref = &window;
                let captured_build_cost = std::cell::Cell::new(0.0f64);
                let emit_fn = |event: web_builder_agent::build_stream::BuildStreamEvent| {
                    // Capture cost from BuildCompleted events
                    if let web_builder_agent::build_stream::BuildStreamEvent::BuildCompleted {
                        actual_cost,
                        ..
                    } = &event
                    {
                        captured_build_cost.set(*actual_cost);
                    }
                    let _ = window_ref.emit("build-stream", &event);
                };

                // Ensure builder_state.json exists and transition to Generating
                {
                    let project_id = output_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let mut proj_state = web_builder_agent::project::load_project_state(
                        output_path,
                    )
                    .unwrap_or_else(|_| {
                        // No state yet (direct build without planning) — create from Draft
                        let mut s =
                            web_builder_agent::project::create_project(&project_id, &prompt);
                        // Skip straight to Approved for direct builds
                        s.status = web_builder_agent::project::ProjectStatus::Approved;
                        s
                    });

                    // If still Planned, approve first (user approved in UI)
                    if proj_state.status == web_builder_agent::project::ProjectStatus::Planned {
                        let _ = web_builder_agent::project::transition(
                            &mut proj_state,
                            web_builder_agent::project::ProjectStatus::Approved,
                        );
                    }
                    if let Err(te) = web_builder_agent::project::transition(
                        &mut proj_state,
                        web_builder_agent::project::ProjectStatus::Generating,
                    ) {
                        eprintln!("[conductor-stream] Warning: state transition to Generating failed: {te}");
                    }
                    if let Err(se) =
                        web_builder_agent::project::save_project_state(output_path, &proj_state)
                    {
                        eprintln!(
                            "[conductor-stream] Warning: failed to save builder_state.json: {se}"
                        );
                    } else {
                        eprintln!(
                            "[conductor-stream] Saved builder_state.json (status=Generating)"
                        );
                    }
                }

                eprintln!(
                    "[conductor-stream] Starting streaming build: model={}, dir={}",
                    model_name, out_dir
                );

                let start = std::time::Instant::now();
                match conductor.execute_web_build_streaming(
                    &task,
                    output_path,
                    &mut audit,
                    agent_id,
                    streaming_provider.as_ref(),
                    &emit_fn,
                ) {
                    Ok(paths) => {
                        let elapsed = start.elapsed().as_secs_f64();
                        eprintln!(
                            "[conductor-stream] Build complete: {} files in {:.1}s",
                            paths.len(),
                            elapsed
                        );

                        app_state.log_event(
                            agent_id,
                            super::EventType::StateChange,
                            serde_json::json!({
                                "source": "conductor",
                                "action": "conduct_build_streaming",
                                "status": "Success",
                                "output_files": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                                "duration_secs": elapsed,
                            }),
                        );

                        // Save project metadata
                        let project_dir = std::path::Path::new(&out_dir);
                        let project_id = project_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        // Derive site name from prompt (first few words)
                        let site_name: String = prompt
                            .split_whitespace()
                            .filter(|w| {
                                !["a", "an", "the", "build", "create", "make"]
                                    .contains(&w.to_lowercase().as_str())
                            })
                            .take(4)
                            .collect::<Vec<_>>()
                            .join(" ");
                        let mgr =
                            web_builder_agent::checkpoint::CheckpointManager::new(project_dir);
                        let cps = mgr.list_checkpoints();
                        let html_lines = mgr
                            .read_current_html()
                            .map(|h| h.lines().count())
                            .unwrap_or(0);
                        let now = chrono::Utc::now().to_rfc3339();
                        let meta = web_builder_agent::checkpoint::ProjectMeta {
                            id: project_id,
                            name: if site_name.is_empty() {
                                "Untitled".to_string()
                            } else {
                                site_name
                            },
                            prompt: prompt.clone(),
                            model: model_name.clone(),
                            created_at: now.clone(),
                            updated_at: now,
                            versions: cps.len(),
                            total_cost: 0.0, // updated by budget tracker
                            lines: html_lines,
                        };
                        let _ =
                            web_builder_agent::checkpoint::save_project_meta(project_dir, &meta);

                        // Update builder_state: Generating -> Generated
                        if let Ok(mut proj_state) =
                            web_builder_agent::project::load_project_state(project_dir)
                        {
                            proj_state.line_count = Some(html_lines as u32);
                            proj_state.char_count =
                                mgr.read_current_html().ok().map(|h| h.len() as u32);
                            proj_state.current_checkpoint = cps.last().map(|cp| cp.id.clone());
                            // Set build cost from the captured BuildCompleted event
                            let bc = captured_build_cost.get();
                            if bc > 0.0 {
                                proj_state.build_cost = bc;
                                proj_state.total_cost = proj_state.plan_cost + bc;
                            }
                            let _ = web_builder_agent::project::transition(
                                &mut proj_state,
                                web_builder_agent::project::ProjectStatus::Generated,
                            );
                            if let Err(se) = web_builder_agent::project::save_project_state(
                                project_dir,
                                &proj_state,
                            ) {
                                eprintln!("[conductor-stream] Warning: failed to save Generated state: {se}");
                            } else {
                                eprintln!("[conductor-stream] Saved builder_state.json (status=Generated)");
                            }
                        } else {
                            eprintln!("[conductor-stream] Warning: could not load builder_state.json for Generated transition");
                        }

                        let _ = tx.send(Ok(serde_json::json!({
                            "status": "Success",
                            "output_dir": out_dir,
                            "output_files": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                            "duration_secs": elapsed,
                            "streaming": true,
                        })));
                    }
                    Err(e) => {
                        eprintln!("[conductor-stream] Build failed: {e}");

                        // Transition to GenerationFailed
                        let project_dir = std::path::Path::new(&out_dir);
                        if let Ok(mut proj_state) =
                            web_builder_agent::project::load_project_state(project_dir)
                        {
                            proj_state.error_message = Some(e.to_string());
                            let _ = web_builder_agent::project::transition(
                                &mut proj_state,
                                web_builder_agent::project::ProjectStatus::GenerationFailed,
                            );
                            if let Err(se) = web_builder_agent::project::save_project_state(
                                project_dir,
                                &proj_state,
                            ) {
                                eprintln!(
                                    "[conductor-stream] Warning: failed to save GenerationFailed state: {se}"
                                );
                            }
                        }

                        let _ = tx.send(Err(format!("streaming build failed: {e}")));
                    }
                }
            } else {
                // Non-streaming fallback for providers without StreamingLlmProvider
                eprintln!(
                    "[conductor-stream] Model '{}' does not support streaming, using non-streaming path",
                    full_model
                );
                let (provider, model_name) =
                    match super::provider_from_prefixed_model(&full_model, &prov_config) {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            return;
                        }
                    };

                let mut conductor = super::Conductor::new(provider, &model_name);
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
                        let _ = window.emit("conductor:finished", &res);
                        let result_json = serde_json::to_value(&res).unwrap_or_default();
                        let _ = tx.send(Ok(serde_json::json!({
                            "result": result_json,
                            "streaming": false,
                        })));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("conductor failed: {e}")));
                    }
                }
            }
        });

        rx.recv()
            .unwrap_or(Err("Conductor thread terminated unexpectedly".to_string()))
    }

    /// Read a file from a build output directory. Used by the Builder preview pane.
    #[tauri::command]
    fn read_build_file(path: String) -> Result<String, String> {
        // Security: only allow reading from ~/.nexus/builds/
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let builds_dir = format!("{home}/.nexus/builds/");
        let canonical = std::fs::canonicalize(&path)
            .map_err(|e| format!("file not found: {e}"))?
            .to_string_lossy()
            .to_string();
        if !canonical.starts_with(
            &std::fs::canonicalize(&builds_dir)
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ) {
            return Err("Access denied: can only read files from ~/.nexus/builds/".to_string());
        }
        std::fs::read_to_string(&canonical).map_err(|e| format!("failed to read file: {e}"))
    }

    // ── Builder Budget Tracking ────────────────────────────────────────────

    #[tauri::command]
    fn builder_get_budget() -> Result<serde_json::Value, String> {
        let tracker = web_builder_agent::budget::BudgetTracker::new();
        let status = tracker.get_budget_status();
        serde_json::to_value(status).map_err(|e| format!("serialization error: {e}"))
    }

    #[tauri::command]
    fn builder_set_budget(provider: String, amount: f64) -> Result<(), String> {
        let tracker = web_builder_agent::budget::BudgetTracker::new();
        tracker.set_initial_budget(&provider, amount)
    }

    #[tauri::command]
    fn builder_set_remaining(provider: String, remaining: f64) -> Result<(), String> {
        let tracker = web_builder_agent::budget::BudgetTracker::new();
        tracker.set_remaining(&provider, remaining)
    }

    #[tauri::command]
    fn builder_list_projects() -> Result<serde_json::Value, String> {
        let projects = web_builder_agent::project::list_all_projects();
        serde_json::to_value(projects).map_err(|e| format!("serialization error: {e}"))
    }

    #[tauri::command]
    fn builder_load_project(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let meta = web_builder_agent::checkpoint::load_project_meta(&project_dir)
            .ok_or_else(|| format!("project {project_id} not found"))?;

        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr.read_current_html().unwrap_or_default();
        let checkpoints = mgr.list_checkpoints();

        // Include builder_state if available
        let state = web_builder_agent::project::load_project_state(&project_dir).ok();

        // Load plan artefacts if available
        let plan = web_builder_agent::plan::load_plan_artefacts(&project_dir);

        serde_json::to_value(serde_json::json!({
            "meta": meta,
            "html": html,
            "checkpoints": checkpoints,
            "project_dir": project_dir.to_string_lossy(),
            "state": state,
            "plan": plan,
        }))
        .map_err(|e| format!("serialization error: {e}"))
    }

    #[tauri::command]
    fn builder_delete_project(project_id: String) -> Result<(), String> {
        web_builder_agent::checkpoint::delete_project(&project_id)
    }

    #[tauri::command]
    fn builder_get_history() -> Result<serde_json::Value, String> {
        let tracker = web_builder_agent::budget::BudgetTracker::new();
        let history = tracker.get_build_history();
        serde_json::to_value(history).map_err(|e| format!("serialization error: {e}"))
    }

    #[tauri::command]
    fn builder_record_build(build_json: String) -> Result<(), String> {
        let record: web_builder_agent::budget::BuildRecord =
            serde_json::from_str(&build_json).map_err(|e| format!("invalid build record: {e}"))?;
        let tracker = web_builder_agent::budget::BudgetTracker::new();
        tracker.record_build(record)
    }

    // ─── Model Configuration Commands ─────────────────────────────────────

    /// Detect all available models on this machine (Ollama, CLI providers, API keys).
    #[tauri::command]
    fn builder_get_available_models() -> Result<serde_json::Value, String> {
        let available = web_builder_agent::model_config::detect_available_models();
        serde_json::to_value(&available).map_err(|e| format!("serialization error: {e}"))
    }

    /// Get current model config (or smart defaults if not configured).
    #[tauri::command]
    fn builder_get_model_config() -> Result<serde_json::Value, String> {
        let config = web_builder_agent::model_config::load_config();
        serde_json::to_value(&config).map_err(|e| format!("serialization error: {e}"))
    }

    /// Save user model config to ~/.nexus/builder_model_config.json.
    #[tauri::command]
    fn builder_save_model_config(config_json: String) -> Result<(), String> {
        let config: web_builder_agent::model_config::BuildModelConfig =
            serde_json::from_str(&config_json).map_err(|e| format!("invalid model config: {e}"))?;
        web_builder_agent::model_config::save_config(&config)
    }

    /// Reset model config to smart defaults based on currently available models.
    #[tauri::command]
    fn builder_reset_model_config() -> Result<serde_json::Value, String> {
        let available = web_builder_agent::model_config::detect_available_models();
        let config = web_builder_agent::model_config::generate_smart_defaults(&available);
        web_builder_agent::model_config::save_config(&config)
            .map_err(|e| format!("save error: {e}"))?;
        serde_json::to_value(&config).map_err(|e| format!("serialization error: {e}"))
    }

    /// Get available model choices for each build step.
    #[tauri::command]
    fn builder_get_model_choices() -> Result<serde_json::Value, String> {
        let available = web_builder_agent::model_config::detect_available_models();
        serde_json::to_value(serde_json::json!({
            "planning": available.choices_for_planning(),
            "content_generation": available.choices_for_content(),
            "section_edit": available.choices_for_section_edit(),
            "full_build": available.choices_for_full_build(),
            "security_policies": available.choices_for_security(),
        }))
        .map_err(|e| format!("serialization error: {e}"))
    }

    // ─── CLI Authentication Commands ──────────────────────────────────────

    /// Check whether a CLI provider is authenticated.
    /// For codex: checks auth file on disk, falls back to exec probe.
    /// For claude: runs `claude auth status --text`.
    #[tauri::command]
    fn builder_check_cli_auth(cli: String) -> Result<bool, String> {
        web_builder_agent::model_config::check_cli_auth(&cli)
    }

    /// Spawn CLI login and poll for success. Emits events:
    /// - cli-auth-progress { cli, message }
    /// - cli-auth-success  { cli }
    /// - cli-auth-failed   { cli, reason }
    #[tauri::command]
    async fn builder_authenticate_cli(cli: String, window: tauri::Window) -> Result<(), String> {
        let (bin, args): (&str, Vec<&str>) = match cli.as_str() {
            "claude" => ("claude", vec!["auth", "login"]),
            "codex" => ("codex", vec!["login"]),
            other => return Err(format!("unknown cli: {other}")),
        };

        let cli_name = cli.clone();

        // Emit progress
        let emit_progress = |msg: &str| {
            let _ = window.emit(
                "cli-auth-progress",
                serde_json::json!({ "cli": &cli_name, "message": msg }),
            );
        };

        emit_progress(&format!("Starting {bin} login..."));

        // Spawn the login process (it opens a browser for OAuth)
        let mut child = std::process::Command::new(bin)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!("{bin} is not installed")
                } else {
                    format!("failed to start {bin}: {e}")
                }
            })?;

        emit_progress("Opening browser for authentication...");

        // Poll for auth success in a background task.
        // For codex: check auth file first (instant), then `claude auth status` for claude.
        // Polls every 3s for up to 60s, stops as soon as auth is detected.
        let cli_poll = cli.clone();
        let window_poll = window.clone();
        tokio::spawn(async move {
            let max_polls = 20; // 20 * 3s = 60s
            for i in 0..max_polls {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                let ok = match cli_poll.as_str() {
                    "codex" => {
                        // Fast: check auth file on disk
                        nexus_connectors_llm::providers::codex_cli::check_codex_auth_file()
                    }
                    "claude" => std::process::Command::new("claude")
                        .args(["auth", "status", "--text"])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false),
                    _ => return,
                };

                if ok {
                    let _ = window_poll
                        .emit("cli-auth-success", serde_json::json!({ "cli": &cli_poll }));
                    return;
                }

                if i % 3 == 0 {
                    let _ = window_poll.emit(
                        "cli-auth-progress",
                        serde_json::json!({
                            "cli": &cli_poll,
                            "message": format!("Waiting for browser authentication... ({}s)", (i + 1) * 3)
                        }),
                    );
                }
            }

            let _ = window_poll.emit(
                "cli-auth-failed",
                serde_json::json!({ "cli": &cli_poll, "reason": "timeout" }),
            );
        });

        // Wait for the login process to finish (it exits after auth completes or user cancels)
        let _ = child.wait();

        Ok(())
    }

    /// Read the current preview HTML from a project's current/index.html.
    #[tauri::command]
    fn builder_read_preview(project_dir: String) -> Result<String, String> {
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(std::path::Path::new(
            &project_dir,
        ));
        mgr.read_current_html()
    }

    /// List all checkpoints for a project.
    #[tauri::command]
    fn builder_list_checkpoints(project_dir: String) -> Result<serde_json::Value, String> {
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(std::path::Path::new(
            &project_dir,
        ));
        let checkpoints = mgr.list_checkpoints();
        serde_json::to_value(checkpoints).map_err(|e| format!("serialization error: {e}"))
    }

    /// Rollback to a specific checkpoint.
    #[tauri::command]
    fn builder_rollback(
        project_dir: String,
        checkpoint_id: String,
    ) -> Result<serde_json::Value, String> {
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(std::path::Path::new(
            &project_dir,
        ));
        let cp = mgr.rollback(&checkpoint_id)?;
        serde_json::to_value(cp).map_err(|e| format!("serialization error: {e}"))
    }

    /// Initialize checkpoints from a completed build's output directory.
    #[tauri::command]
    fn builder_init_checkpoint(
        project_dir: String,
        build_output_dir: String,
        cost: f64,
    ) -> Result<serde_json::Value, String> {
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(std::path::Path::new(
            &project_dir,
        ));
        let cp = mgr.init_from_build(std::path::Path::new(&build_output_dir), cost)?;
        serde_json::to_value(cp).map_err(|e| format!("serialization error: {e}"))
    }

    /// Iterate on a completed build: apply a change request via streaming LLM.
    ///
    /// 1. Auto-checkpoints current state
    /// 2. Reads current HTML
    /// 3. Builds iteration prompt with change request
    /// 4. Generates via streaming (emits build-stream events)
    /// 5. Writes result to current/
    /// 6. Records cost in budget tracker
    #[tauri::command]
    async fn builder_iterate(
        window: tauri::Window,
        project_dir: String,
        change_request: String,
        model: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();

        std::thread::spawn(move || {
            let config = match super::load_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("config error: {e}")));
                    return;
                }
            };
            let prov_config = super::build_provider_config(&config);

            // Resolve the full model string from user config (or smart defaults).
            // Falls back to "claude-sonnet-4-6" only if config loading fails entirely.
            let full_model = model.unwrap_or_else(|| {
                let iter_cfg = web_builder_agent::model_config::load_config();
                let choice = &iter_cfg.full_build;
                if choice.is_none() {
                    eprintln!(
                        "[builder-iterate] Step \"full_build\" using model: claude-sonnet-4-6 (default \u{2014} no user config found)"
                    );
                    return "claude-sonnet-4-6".to_string();
                }
                let prefixed = web_builder_agent::model_config::to_prefixed_model(choice);
                eprintln!(
                    "[builder-iterate] Step \"full_build\" using model: {} (from {})",
                    choice.display_name,
                    if std::path::Path::new(
                        &std::env::var("HOME").unwrap_or_default(),
                    )
                    .join(".nexus/builder_model_config.json")
                    .exists()
                    {
                        "user config"
                    } else {
                        "default \u{2014} no user config found"
                    }
                );
                prefixed
            });
            let (streaming_provider, model_name) =
                match super::streaming_provider_from_prefixed_model(&full_model, &prov_config) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };

            let mgr = web_builder_agent::checkpoint::CheckpointManager::new(std::path::Path::new(
                &project_dir,
            ));

            // 1. Auto-checkpoint current state
            let truncated_req: String = change_request.chars().take(50).collect();
            let pre_cp = match mgr.save_checkpoint(&format!("Before: {truncated_req}"), 0.0) {
                Ok(cp) => cp,
                Err(e) => {
                    let _ = tx.send(Err(format!("checkpoint failed: {e}")));
                    return;
                }
            };

            // 2. Read current HTML
            let current_html = match mgr.read_current_html() {
                Ok(html) => html,
                Err(e) => {
                    let _ = tx.send(Err(format!("read current failed: {e}")));
                    return;
                }
            };

            // 2.5. Transition to Iterating
            let pd = std::path::Path::new(&project_dir);
            if let Ok(mut proj_state) = web_builder_agent::project::load_project_state(pd) {
                let _ = web_builder_agent::project::transition(
                    &mut proj_state,
                    web_builder_agent::project::ProjectStatus::Iterating,
                );
                let _ = web_builder_agent::project::save_project_state(pd, &proj_state);
            }

            // Helper: on iteration failure, transition to IterationFailed
            let mark_iteration_failed = |error_msg: &str| {
                let pd = std::path::Path::new(&project_dir);
                if let Ok(mut ps) = web_builder_agent::project::load_project_state(pd) {
                    ps.error_message = Some(error_msg.to_string());
                    let _ = web_builder_agent::project::transition(
                        &mut ps,
                        web_builder_agent::project::ProjectStatus::IterationFailed,
                    );
                    if let Err(se) = web_builder_agent::project::save_project_state(pd, &ps) {
                        eprintln!(
                            "[builder-iterate] Warning: failed to save IterationFailed state: {se}"
                        );
                    }
                }
            };

            // 3. Smart iteration: classify the edit request into tiers
            use web_builder_agent::smart_iterate::*;
            let classification = classify_edit(&change_request, &current_html);
            eprintln!(
                "[builder-iterate] Smart classify: tier={:?}, confidence={:.2}, reason={}",
                classification.tier, classification.confidence, classification.reason
            );

            let window_ref = &window;
            let emit_fn = |event: web_builder_agent::build_stream::BuildStreamEvent| {
                let _ = window_ref.emit("build-stream", &event);
            };

            let start = std::time::Instant::now();

            // Branch on tier
            let (cleaned, input_tokens, output_tokens, actual_cost, tier_label, tier_detail): (
                String,
                usize,
                usize,
                f64,
                String,
                serde_json::Value,
            ) = match classification.tier {
                // ── Tier 1: CSS Variable Edit — instant, $0.00, no LLM ──
                EditTier::CssVariable => {
                    let changes = classification.css_changes.as_ref().unwrap();
                    eprintln!(
                        "[builder-iterate] Tier 1: {} CSS variable changes",
                        changes.len()
                    );

                    emit_fn(
                        web_builder_agent::build_stream::BuildStreamEvent::BuildStarted {
                            project_name: format!("CSS edit: {truncated_req}"),
                            estimated_cost: 0.0,
                            estimated_tasks: 1,
                            model_name: model_name.clone(),
                            timestamp: String::new(),
                        },
                    );

                    let result = match apply_css_changes(&current_html, changes) {
                        Ok(html) => html,
                        Err(e) => {
                            emit_fn(
                                web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.clone(),
                                    tokens_consumed: 0,
                                    cost_consumed: 0.0,
                                },
                            );
                            mark_iteration_failed(&format!("CSS edit failed: {e}"));
                            let _ = tx.send(Err(format!("CSS edit failed: {e}")));
                            return;
                        }
                    };

                    let detail = serde_json::json!({
                        "css_changes": changes.iter().map(|c| serde_json::json!({
                            "variable": c.variable,
                            "old_value": c.old_value,
                            "new_value": c.new_value,
                        })).collect::<Vec<_>>(),
                    });

                    (result, 0, 0, 0.0, "css_variable".to_string(), detail)
                }

                // ── Tier 2: Section-Level Edit — LLM on one section ──
                EditTier::SectionEdit => {
                    let section_id = classification
                        .target_section
                        .as_deref()
                        .unwrap_or("unknown");
                    let is_remove = {
                        let l = change_request.to_lowercase();
                        l.contains("remove the") || l.contains("delete the")
                    };
                    let is_add = {
                        let l = change_request.to_lowercase();
                        l.contains("add a") || l.contains("add an")
                    };

                    eprintln!(
                        "[builder-iterate] Tier 2: section=\"{}\", add={}, remove={}",
                        section_id, is_add, is_remove
                    );

                    // Handle removal (no LLM needed)
                    if is_remove {
                        emit_fn(
                            web_builder_agent::build_stream::BuildStreamEvent::BuildStarted {
                                project_name: format!("Remove section: {section_id}"),
                                estimated_cost: 0.0,
                                estimated_tasks: 1,
                                model_name: model_name.clone(),
                                timestamp: String::new(),
                            },
                        );

                        let result = match remove_section(&current_html, section_id) {
                            Ok(html) => html,
                            Err(e) => {
                                emit_fn(web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.clone(),
                                    tokens_consumed: 0,
                                    cost_consumed: 0.0,
                                });
                                mark_iteration_failed(&format!("Section removal failed: {e}"));
                                let _ = tx.send(Err(format!("Section removal failed: {e}")));
                                return;
                            }
                        };

                        let detail = serde_json::json!({
                            "section": section_id,
                            "action": "removed",
                        });

                        (result, 0, 0, 0.0, "section_edit".to_string(), detail)
                    } else {
                        // Section edit or add — requires LLM
                        let (prompt, system_prompt) = if is_add {
                            (
                                build_section_add_prompt(
                                    section_id,
                                    &change_request,
                                    &current_html,
                                ),
                                SECTION_ADD_SYSTEM_PROMPT,
                            )
                        } else {
                            let section_html = match extract_section(&current_html, section_id) {
                                Some(span) => span.content,
                                None => {
                                    mark_iteration_failed(&format!(
                                        "Section '{}' not found",
                                        section_id
                                    ));
                                    let _ =
                                        tx.send(Err(format!("Section '{}' not found", section_id)));
                                    return;
                                }
                            };
                            (
                                build_section_edit_prompt(&section_html, &change_request),
                                SECTION_EDIT_SYSTEM_PROMPT,
                            )
                        };

                        // Try local model first for section edits (free)
                        let sect_budget =
                            web_builder_agent::model_router::RoutingBudget::from_budget_tracker();
                        let sect_sel = web_builder_agent::model_router::select_model(
                            &web_builder_agent::model_router::BuilderTask::SectionEdit,
                            &sect_budget,
                        );

                        // Attempt Ollama non-streaming first
                        let ollama_result: Option<(
                            String,
                            usize,
                            usize,
                            f64,
                            String,
                            serde_json::Value,
                        )> = if sect_sel.provider
                            == web_builder_agent::model_router::ProviderType::Ollama
                        {
                            let full_prompt = format!("{system_prompt}\n\n{prompt}");
                            let ollama =
                                nexus_connectors_llm::providers::OllamaProvider::from_env();
                            eprintln!(
                                "[builder-iterate] Tier 2: trying Ollama {} for section edit",
                                sect_sel.model_id
                            );

                            emit_fn(
                                web_builder_agent::build_stream::BuildStreamEvent::BuildStarted {
                                    project_name: format!("Section edit: {section_id}"),
                                    estimated_cost: 0.0,
                                    estimated_tasks: 1,
                                    model_name: sect_sel.model_id.clone(),
                                    timestamp: String::new(),
                                },
                            );

                            let ollama_start = std::time::Instant::now();
                            eprintln!(
                                "[builder-iterate] Ollama prompt: {} chars",
                                full_prompt.len()
                            );
                            match ollama.query(&full_prompt, 8192, &sect_sel.model_id) {
                                Ok(resp)
                                    if !resp.output_text.trim().is_empty()
                                        && (resp.output_text.contains("<section")
                                            || resp.output_text.contains("<footer")
                                            || resp.output_text.contains("<header")
                                            || resp.output_text.contains("<nav")) =>
                                {
                                    eprintln!(
                                        "[builder-iterate] Ollama responded in {:.1}s, {} chars output",
                                        ollama_start.elapsed().as_secs_f64(),
                                        resp.output_text.len()
                                    );
                                    let cleaned =
                                        web_builder_agent::llm_codegen::strip_markdown_fences(
                                            &resp.output_text,
                                        );
                                    match splice_section(&current_html, section_id, &cleaned) {
                                        Ok(spliced) => {
                                            eprintln!("[builder-iterate] Ollama section edit succeeded ({})", sect_sel.model_id);
                                            let in_tok = resp.input_tokens.unwrap_or(0) as usize;
                                            let out_tok = resp.token_count as usize;
                                            let detail = serde_json::json!({
                                                "section": section_id,
                                                "action": if is_add { "added" } else { "edited" },
                                            });
                                            Some((
                                                spliced,
                                                in_tok,
                                                out_tok,
                                                0.0,
                                                "section_edit".to_string(),
                                                detail,
                                            ))
                                        }
                                        Err(e) => {
                                            eprintln!("[builder-iterate] Ollama splice failed: {e}, falling back to API");
                                            None
                                        }
                                    }
                                }
                                Ok(resp) => {
                                    eprintln!(
                                        "[builder-iterate] Ollama returned invalid section HTML in {:.1}s ({} chars), falling back to API",
                                        ollama_start.elapsed().as_secs_f64(),
                                        resp.output_text.len()
                                    );
                                    if !resp.output_text.is_empty() {
                                        eprintln!(
                                            "[builder-iterate] Ollama output preview: {}",
                                            &resp.output_text[..resp.output_text.len().min(200)]
                                        );
                                    }
                                    None
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[builder-iterate] Ollama section edit failed in {:.1}s: {e}, falling back to API",
                                        ollama_start.elapsed().as_secs_f64()
                                    );
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        // If Ollama succeeded, use its result; otherwise fall through to streaming API
                        if let Some(result) = ollama_result {
                            result
                        } else {
                            // Streaming API path (Sonnet/GPT-4o)
                            let est_input = prompt.len() / 4;
                            let est_output = 2000;
                            let est_cost = web_builder_agent::build_stream::estimate_cost(
                                &model_name,
                                est_input,
                                est_output,
                            );
                            emit_fn(
                                web_builder_agent::build_stream::BuildStreamEvent::BuildStarted {
                                    project_name: format!("Section edit: {section_id}"),
                                    estimated_cost: est_cost,
                                    estimated_tasks: 1,
                                    model_name: model_name.clone(),
                                    timestamp: String::new(),
                                },
                            );

                            // Stream LLM for section edit (uses whatever provider the user selected)
                            use nexus_connectors_llm::streaming::StreamingLlmProvider;
                            let mut stream = match streaming_provider.as_ref().stream_query(
                                &prompt,
                                system_prompt,
                                8192,
                                &model_name,
                            ) {
                                Ok(s) => s,
                                Err(e) => {
                                    emit_fn(web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.to_string(),
                                    tokens_consumed: 0,
                                    cost_consumed: 0.0,
                                });
                                    mark_iteration_failed(&format!(
                                        "section streaming failed: {e}"
                                    ));
                                    let _ = tx.send(Err(format!("streaming failed: {e}")));
                                    return;
                                }
                            };

                            let mut accumulated = String::new();
                            let mut token_count: usize = 0;
                            let mut last_event_time = std::time::Instant::now();
                            let estimated_total = est_output;

                            loop {
                                match stream.next() {
                                    Some(Ok(chunk)) => {
                                        accumulated.push_str(&chunk.text);
                                        token_count += chunk.token_count.unwrap_or(1);
                                        if last_event_time.elapsed()
                                            >= std::time::Duration::from_millis(500)
                                        {
                                            emit_fn(web_builder_agent::build_stream::BuildStreamEvent::GenerationProgress {
                                            phase: web_builder_agent::build_stream::GenerationPhase::Building,
                                            tokens_generated: token_count,
                                            estimated_total_tokens: estimated_total,
                                            elapsed_seconds: start.elapsed().as_secs_f64(),
                                            raw_chunk: Some(chunk.text),
                                        });
                                            last_event_time = std::time::Instant::now();
                                        }
                                    }
                                    Some(Err(e)) => {
                                        emit_fn(web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                        error: e.to_string(),
                                        tokens_consumed: token_count,
                                        cost_consumed: 0.0,
                                    });
                                        mark_iteration_failed(&format!(
                                            "section streaming error: {e}"
                                        ));
                                        let _ = tx.send(Err(format!("streaming error: {e}")));
                                        return;
                                    }
                                    None => break,
                                }
                            }

                            if accumulated.trim().is_empty() {
                                emit_fn(
                                web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: "LLM returned empty section".to_string(),
                                    tokens_consumed: token_count,
                                    cost_consumed: 0.0,
                                },
                            );
                                mark_iteration_failed("section edit returned empty output");
                                let _ =
                                    tx.send(Err("section edit returned empty output".to_string()));
                                return;
                            }

                            let usage = stream.usage();
                            let in_tok = usage.input_tokens;
                            let out_tok = if usage.output_tokens > 0 {
                                usage.output_tokens
                            } else {
                                token_count
                            };
                            let cost = web_builder_agent::build_stream::calculate_cost(
                                &model_name,
                                in_tok,
                                out_tok,
                            );

                            // Splice the new section into the full HTML
                            let spliced = match splice_section(
                                &current_html,
                                section_id,
                                &accumulated,
                            ) {
                                Ok(html) => html,
                                Err(e) => {
                                    emit_fn(web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.clone(),
                                    tokens_consumed: token_count,
                                    cost_consumed: cost,
                                });
                                    mark_iteration_failed(&format!("splice failed: {e}"));
                                    let _ = tx.send(Err(format!("splice failed: {e}")));
                                    return;
                                }
                            };

                            let detail = serde_json::json!({
                                "section": section_id,
                                "action": if is_add { "added" } else { "edited" },
                            });

                            (
                                spliced,
                                in_tok,
                                out_tok,
                                cost,
                                "section_edit".to_string(),
                                detail,
                            )
                        } // end else (streaming API fallback)
                    }
                }

                // ── Tier 3: Full Regeneration — existing behavior ──
                EditTier::FullRegeneration => {
                    let prompt = web_builder_agent::checkpoint::build_iteration_prompt(
                        &current_html,
                        &change_request,
                    );
                    let system_prompt = web_builder_agent::checkpoint::ITERATION_SYSTEM_PROMPT;
                    let preview_len = prompt.len().min(200);
                    eprintln!(
                        "[builder-iterate] Tier 3: Full regen | Prompt preview: {}",
                        &prompt[..preview_len]
                    );

                    let est_cost = web_builder_agent::build_stream::estimate_cost(
                        &model_name,
                        web_builder_agent::build_stream::ESTIMATED_ITERATION_INPUT_TOKENS,
                        web_builder_agent::build_stream::ESTIMATED_TOTAL_TOKENS,
                    );
                    emit_fn(
                        web_builder_agent::build_stream::BuildStreamEvent::BuildStarted {
                            project_name: format!("Iteration: {truncated_req}"),
                            estimated_cost: est_cost,
                            estimated_tasks: 1,
                            model_name: model_name.clone(),
                            timestamp: String::new(),
                        },
                    );

                    use nexus_connectors_llm::streaming::StreamingLlmProvider;
                    let mut stream = match streaming_provider.as_ref().stream_query(
                        &prompt,
                        system_prompt,
                        16384,
                        &model_name,
                    ) {
                        Ok(s) => s,
                        Err(e) => {
                            emit_fn(
                                web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.to_string(),
                                    tokens_consumed: 0,
                                    cost_consumed: 0.0,
                                },
                            );
                            mark_iteration_failed(&format!("full regen streaming failed: {e}"));
                            let _ = tx.send(Err(format!("streaming failed: {e}")));
                            return;
                        }
                    };

                    let mut accumulated = String::new();
                    let mut token_count: usize = 0;
                    let mut last_event_time = std::time::Instant::now();
                    let estimated_total = web_builder_agent::build_stream::ESTIMATED_TOTAL_TOKENS;

                    loop {
                        match stream.next() {
                            Some(Ok(chunk)) => {
                                accumulated.push_str(&chunk.text);
                                token_count += chunk.token_count.unwrap_or(1);
                                if last_event_time.elapsed()
                                    >= std::time::Duration::from_millis(500)
                                {
                                    let phase = web_builder_agent::build_stream::detect_phase(
                                        &accumulated,
                                        token_count,
                                        estimated_total,
                                    );
                                    emit_fn(web_builder_agent::build_stream::BuildStreamEvent::GenerationProgress {
                                        phase,
                                        tokens_generated: token_count,
                                        estimated_total_tokens: estimated_total,
                                        elapsed_seconds: start.elapsed().as_secs_f64(),
                                        raw_chunk: Some(chunk.text),
                                    });
                                    last_event_time = std::time::Instant::now();
                                }
                            }
                            Some(Err(e)) => {
                                emit_fn(web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                    error: e.to_string(),
                                    tokens_consumed: token_count,
                                    cost_consumed: web_builder_agent::build_stream::calculate_cost(
                                        &model_name, 0, token_count,
                                    ),
                                });
                                mark_iteration_failed(&format!("full regen streaming error: {e}"));
                                let _ = tx.send(Err(format!("streaming error: {e}")));
                                return;
                            }
                            None => break,
                        }
                    }

                    if accumulated.trim().is_empty() {
                        emit_fn(
                            web_builder_agent::build_stream::BuildStreamEvent::BuildFailed {
                                error: "LLM returned empty response".to_string(),
                                tokens_consumed: token_count,
                                cost_consumed: 0.0,
                            },
                        );
                        mark_iteration_failed("iteration returned empty output");
                        let _ = tx.send(Err("iteration returned empty output".to_string()));
                        return;
                    }

                    let usage = stream.usage();
                    let in_tok = usage.input_tokens;
                    let out_tok = if usage.output_tokens > 0 {
                        usage.output_tokens
                    } else {
                        token_count
                    };
                    let cost = web_builder_agent::build_stream::calculate_cost(
                        &model_name,
                        in_tok,
                        out_tok,
                    );

                    let cleaned_html =
                        web_builder_agent::llm_codegen::strip_markdown_fences(&accumulated);

                    let detail = serde_json::json!({
                        "reason": classification.reason,
                    });

                    (
                        cleaned_html,
                        in_tok,
                        out_tok,
                        cost,
                        "full_regeneration".to_string(),
                        detail,
                    )
                }
            };

            eprintln!(
                "[builder-iterate] Tier={}, tokens={}in/{}out, cost=${:.4}, elapsed={:.1}s",
                tier_label,
                input_tokens,
                output_tokens,
                actual_cost,
                start.elapsed().as_secs_f64()
            );

            // 4. Write result
            if let Err(e) = mgr.write_current_html(&cleaned) {
                mark_iteration_failed(&format!("write failed: {e}"));
                let _ = tx.send(Err(format!("write failed: {e}")));
                return;
            }

            // Save post-iteration checkpoint
            let post_cp = mgr
                .save_checkpoint(&format!("After: {truncated_req}"), actual_cost)
                .unwrap_or_else(|_| web_builder_agent::checkpoint::Checkpoint {
                    id: "unknown".to_string(),
                    timestamp: String::new(),
                    description: String::new(),
                    cost: actual_cost,
                    parent_id: None,
                    lines: 0,
                    chars: 0,
                });

            let elapsed = start.elapsed().as_secs_f64();
            let governance = web_builder_agent::build_stream::quick_governance_scan(&cleaned);

            // Emit BuildCompleted
            emit_fn(
                web_builder_agent::build_stream::BuildStreamEvent::BuildCompleted {
                    project_name: format!("Iteration: {truncated_req}"),
                    total_lines: cleaned.lines().count(),
                    total_chars: cleaned.len(),
                    input_tokens,
                    output_tokens,
                    actual_cost,
                    model_name: model_name.clone(),
                    elapsed_seconds: elapsed,
                    checkpoint_id: post_cp.id.clone(),
                    governance_status: governance,
                    output_dir: project_dir.clone(),
                },
            );

            // 5. Record cost in budget tracker (fire-and-forget)
            let tracker = web_builder_agent::budget::BudgetTracker::new();
            let _ = tracker.record_build(web_builder_agent::budget::BuildRecord {
                project_name: format!("Iteration: {truncated_req}"),
                model_name: model_name.clone(),
                provider: "anthropic".to_string(),
                input_tokens,
                output_tokens,
                cost_usd: actual_cost,
                elapsed_seconds: elapsed,
                lines_generated: cleaned.lines().count(),
                checkpoint_id: post_cp.id.clone(),
                timestamp: String::new(),
            });

            // 6. Update project metadata
            let pd = std::path::Path::new(&project_dir);
            if let Some(mut meta) = web_builder_agent::checkpoint::load_project_meta(pd) {
                meta.updated_at = chrono::Utc::now().to_rfc3339();
                meta.versions = mgr.list_checkpoints().len();
                meta.total_cost += actual_cost;
                meta.lines = cleaned.lines().count();
                let _ = web_builder_agent::checkpoint::save_project_meta(pd, &meta);
            }

            // 7. Update builder_state: Iterating -> Generated
            if let Ok(mut proj_state) = web_builder_agent::project::load_project_state(pd) {
                proj_state.iteration_count += 1;
                proj_state.iteration_costs.push(actual_cost);
                proj_state.total_cost += actual_cost;
                proj_state.line_count = Some(cleaned.lines().count() as u32);
                proj_state.char_count = Some(cleaned.len() as u32);
                proj_state.current_checkpoint = Some(post_cp.id.clone());
                let _ = web_builder_agent::project::transition(
                    &mut proj_state,
                    web_builder_agent::project::ProjectStatus::Generated,
                );
                if let Err(se) = web_builder_agent::project::save_project_state(pd, &proj_state) {
                    eprintln!("[builder-iterate] Warning: failed to save builder_state.json: {se}");
                } else {
                    eprintln!(
                        "[builder-iterate] Saved builder_state.json (iteration_count={})",
                        proj_state.iteration_count
                    );
                }
            }

            let _ = tx.send(Ok(serde_json::json!({
                "checkpoint_id": post_cp.id,
                "previous_checkpoint": pre_cp.id,
                "cost": actual_cost,
                "lines": cleaned.lines().count(),
                "elapsed_seconds": elapsed,
                "tier": tier_label,
                "tier_detail": tier_detail,
                "confidence": classification.confidence,
                "reason": classification.reason,
            })));
        });

        rx.recv()
            .unwrap_or(Err("Iteration thread terminated unexpectedly".to_string()))
    }

    /// Generate a build plan using Haiku 4.5 (cheap planning step).
    ///
    /// Returns a structured plan (product brief + acceptance criteria) along with
    /// token usage and cost. The plan is also saved as artefacts to the project
    /// directory so it survives app restarts.
    #[tauri::command]
    async fn builder_generate_plan(
        prompt: String,
        project_id: String,
    ) -> Result<serde_json::Value, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();

        std::thread::spawn(move || {
            let config = match super::load_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("config error: {e}")));
                    return;
                }
            };
            let prov_config = super::build_provider_config(&config);

            // Build provider instances for each provider type the model router may select.
            // For Anthropic: prefer API key, fall back to Claude Code CLI if available.
            let anthropic_provider: Box<dyn nexus_connectors_llm::providers::LlmProvider> = {
                let has_key = prov_config
                    .anthropic_api_key
                    .as_deref()
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false);
                if has_key {
                    Box::new(super::ClaudeProvider::new(
                        prov_config.anthropic_api_key.clone(),
                    ))
                } else {
                    // Try Claude Code CLI as Anthropic-compatible provider
                    let status = nexus_connectors_llm::providers::claude_code::detect_claude_code();
                    if status.installed && status.authenticated {
                        Box::new(
                            nexus_connectors_llm::providers::claude_code::ClaudeCodeProvider::new(),
                        )
                    } else {
                        // Fall through with the (key-less) ClaudeProvider; it will error
                        // if actually selected, but the router may pick a different provider.
                        Box::new(super::ClaudeProvider::new(
                            prov_config.anthropic_api_key.clone(),
                        ))
                    }
                }
            };
            let openai = super::OpenAiProvider::new(prov_config.openai_api_key.clone());

            eprintln!(
                "[builder-plan] Generating plan for project '{}': {:?}",
                project_id,
                &prompt[..prompt.len().min(100)]
            );

            // Save artefacts to the project directory
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let project_dir = std::path::PathBuf::from(&home)
                .join(".nexus")
                .join("builds")
                .join(&project_id);

            // Create or load project state
            let mut proj_state = web_builder_agent::project::load_project_state(&project_dir)
                .unwrap_or_else(|_| {
                    web_builder_agent::project::create_project(&project_id, &prompt)
                });

            // Load user's model config (or smart defaults) — used for planning step.
            let model_cfg = web_builder_agent::model_config::load_config();
            let plan_choice = &model_cfg.planning;
            let plan_prefixed = web_builder_agent::model_config::to_prefixed_model(plan_choice);
            let plan_model_id = plan_choice.model_id.clone();
            let plan_display = plan_choice.display_name.clone();
            let plan_provider_str = plan_choice.provider.clone();

            eprintln!(
                "[builder-plan] Step \"planning\" using model: {} (from {})",
                plan_display,
                if std::path::Path::new(&std::env::var("HOME").unwrap_or_default())
                    .join(".nexus/builder_model_config.json")
                    .exists()
                {
                    "user config"
                } else {
                    "default \u{2014} no user config found"
                }
            );

            // Try primary model, failover on error or bad JSON
            let try_plan = |provider: &dyn nexus_connectors_llm::providers::LlmProvider,
                            model: &str| {
                let start = std::time::Instant::now();
                eprintln!(
                    "[builder-plan] Calling {} (prompt: {} chars)",
                    model,
                    prompt.len().min(200)
                );
                let result =
                    web_builder_agent::plan::generate_plan_with_model(provider, &prompt, model);
                let elapsed = start.elapsed();
                match &result {
                    Ok(r) => eprintln!(
                        "[builder-plan] {} succeeded in {:.1}s, cost=${:.4}",
                        model,
                        elapsed.as_secs_f64(),
                        r.cost_usd
                    ),
                    Err(e) => eprintln!(
                        "[builder-plan] {} failed in {:.1}s: {}",
                        model,
                        elapsed.as_secs_f64(),
                        &e[..e.len().min(200)]
                    ),
                }
                result
            };

            // Create provider from user's model config choice
            let primary_result = super::provider_from_prefixed_model(&plan_prefixed, &prov_config);

            let codex_cli_prov =
                nexus_connectors_llm::providers::codex_cli::CodexCliProvider::new();
            let claude_code_prov =
                nexus_connectors_llm::providers::claude_code::ClaudeCodeProvider::new();

            // Build a ModelSelection-like struct for the response JSON
            use web_builder_agent::model_router::*;
            let make_selection = |display: &str, provider_s: &str, model_id: &str| ModelSelection {
                provider: match provider_s {
                    "ollama" => ProviderType::Ollama,
                    "anthropic_api" | "anthropic" => ProviderType::Anthropic,
                    "openai_api" | "openai" => ProviderType::OpenAI,
                    "codex_cli" => ProviderType::CodexCli,
                    "claude_cli" => ProviderType::ClaudeCode,
                    _ => ProviderType::Ollama,
                },
                model_id: model_id.to_string(),
                display_name: display.to_string(),
                estimated_cost: 0.0,
                is_local: provider_s == "ollama",
            };
            let selection = make_selection(&plan_display, &plan_provider_str, &plan_model_id);

            let (result, used_model) = match primary_result {
                Ok((primary_provider, _)) => {
                    match try_plan(primary_provider.as_ref(), &plan_model_id) {
                        Ok(r) => (r, selection.clone()),
                        Err(e) => {
                            eprintln!(
                                "[builder-plan] {} failed: {}, attempting failover via model router",
                                plan_display, e
                            );
                            // Failover: use model router budget-based selection
                            let budget = RoutingBudget::from_budget_tracker();
                            let fb = select_model(&BuilderTask::PlanGeneration, &budget);
                            eprintln!(
                                "[builder-plan] Failing over to {} ({})",
                                fb.display_name, fb.provider
                            );
                            let fb_provider: &dyn nexus_connectors_llm::providers::LlmProvider =
                                match fb.provider {
                                    ProviderType::Ollama => {
                                        &nexus_connectors_llm::providers::OllamaProvider::from_env()
                                    }
                                    ProviderType::Anthropic => anthropic_provider.as_ref(),
                                    ProviderType::OpenAI => &openai,
                                    ProviderType::CodexCli => &codex_cli_prov,
                                    ProviderType::ClaudeCode => &claude_code_prov,
                                };
                            match try_plan(fb_provider, &fb.model_id) {
                                Ok(r) => (r, fb),
                                Err(e2) => {
                                    proj_state.error_message = Some(e2.clone());
                                    let _ = web_builder_agent::project::transition(
                                        &mut proj_state,
                                        web_builder_agent::project::ProjectStatus::PlanFailed,
                                    );
                                    let _ = web_builder_agent::project::save_project_state(
                                        &project_dir,
                                        &proj_state,
                                    );
                                    let _ = tx.send(Err(format!(
                                        "plan generation failed (failover): {e2}"
                                    )));
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(provider_err) => {
                    eprintln!(
                        "[builder-plan] Could not create provider for {}: {}, falling back to router",
                        plan_display, provider_err
                    );
                    // Provider creation failed — fall back to budget-based router
                    let budget = RoutingBudget::from_budget_tracker();
                    let fb = select_model(&BuilderTask::PlanGeneration, &budget);
                    let fb_provider: &dyn nexus_connectors_llm::providers::LlmProvider =
                        match fb.provider {
                            ProviderType::Ollama => {
                                &nexus_connectors_llm::providers::OllamaProvider::from_env()
                            }
                            ProviderType::Anthropic => anthropic_provider.as_ref(),
                            ProviderType::OpenAI => &openai,
                            ProviderType::CodexCli => &codex_cli_prov,
                            ProviderType::ClaudeCode => &claude_code_prov,
                        };
                    match try_plan(fb_provider, &fb.model_id) {
                        Ok(r) => (r, fb),
                        Err(e) => {
                            proj_state.error_message = Some(e.clone());
                            let _ = web_builder_agent::project::transition(
                                &mut proj_state,
                                web_builder_agent::project::ProjectStatus::PlanFailed,
                            );
                            let _ = web_builder_agent::project::save_project_state(
                                &project_dir,
                                &proj_state,
                            );
                            let _ = tx.send(Err(format!("plan generation failed: {e}")));
                            return;
                        }
                    }
                }
            };

            if let Err(e) = web_builder_agent::plan::save_plan_artefacts(&project_dir, &result.plan)
            {
                eprintln!("[builder-plan] Warning: failed to save plan artefacts: {e}");
            }

            // Record cost
            let project_name = &result.plan.product_brief.project_name;
            web_builder_agent::plan::record_plan_cost(&result, project_name);

            // Update project state: Draft -> Planned
            proj_state.project_name = Some(project_name.clone());
            proj_state.plan_cost = result.cost_usd;
            proj_state.total_cost += result.cost_usd;
            if let Err(te) = web_builder_agent::project::transition(
                &mut proj_state,
                web_builder_agent::project::ProjectStatus::Planned,
            ) {
                eprintln!("[builder-plan] Warning: state transition failed: {te}");
            }
            if let Err(se) =
                web_builder_agent::project::save_project_state(&project_dir, &proj_state)
            {
                eprintln!("[builder-plan] Warning: failed to save builder_state.json: {se}");
            } else {
                eprintln!("[builder-plan] Saved builder_state.json for project {project_id}");
            }

            eprintln!(
                "[builder-plan] Plan generated: {} input tokens, {} output tokens, ${:.4}, {:.1}s",
                result.input_tokens, result.output_tokens, result.cost_usd, result.elapsed_seconds
            );

            let _ = tx.send(Ok(serde_json::json!({
                "plan": result.plan,
                "input_tokens": result.input_tokens,
                "output_tokens": result.output_tokens,
                "cost_usd": result.cost_usd,
                "elapsed_seconds": result.elapsed_seconds,
                "model": used_model.display_name,
                "model_id": used_model.model_id,
                "provider": used_model.provider.to_string(),
                "is_local": used_model.is_local,
                "project_dir": project_dir.to_string_lossy(),
            })));
        });

        rx.recv().unwrap_or(Err(
            "Plan generation thread terminated unexpectedly".to_string()
        ))
    }

    /// Load a previously saved plan from a project's artefact directory.
    ///
    /// Returns null if no plan exists for the given project.
    #[tauri::command]
    fn builder_load_plan(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        match web_builder_agent::plan::load_plan_artefacts(&project_dir) {
            Some(plan) => {
                serde_json::to_value(&plan).map_err(|e| format!("serialization error: {e}"))
            }
            None => Ok(serde_json::Value::Null),
        }
    }

    /// Archive a project: transitions status to Archived.
    #[tauri::command]
    fn builder_archive_project(project_id: String) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut state = web_builder_agent::project::load_project_state(&project_dir)
            .unwrap_or_else(|_| {
                // Legacy project — create a state from project.json
                let mut s = web_builder_agent::project::create_project(&project_id, "");
                s.status = web_builder_agent::project::ProjectStatus::Generated;
                if let Some(meta) = web_builder_agent::checkpoint::load_project_meta(&project_dir) {
                    s.prompt = meta.prompt;
                    s.project_name = Some(meta.name);
                    s.total_cost = meta.total_cost;
                }
                s
            });

        web_builder_agent::project::transition(
            &mut state,
            web_builder_agent::project::ProjectStatus::Archived,
        )?;
        web_builder_agent::project::save_project_state(&project_dir, &state)
    }

    /// Unarchive a project: transitions status from Archived back to Generated.
    #[tauri::command]
    fn builder_unarchive_project(project_id: String) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut state = web_builder_agent::project::load_project_state(&project_dir)
            .map_err(|e| format!("project {project_id} not found: {e}"))?;

        web_builder_agent::project::transition(
            &mut state,
            web_builder_agent::project::ProjectStatus::Generated,
        )?;
        web_builder_agent::project::save_project_state(&project_dir, &state)
    }

    /// Export a project as a ZIP file with governance metadata.
    #[tauri::command]
    fn builder_export_project(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load or create project state
        let mut state = web_builder_agent::project::load_project_state(&project_dir)
            .unwrap_or_else(|_| {
                let mut s = web_builder_agent::project::create_project(&project_id, "");
                s.status = web_builder_agent::project::ProjectStatus::Generated;
                if let Some(meta) = web_builder_agent::checkpoint::load_project_meta(&project_dir) {
                    s.prompt = meta.prompt.clone();
                    s.project_name = Some(meta.name.clone());
                    s.total_cost = meta.total_cost;
                    s.line_count = Some(meta.lines as u32);
                    s.iteration_count = meta.versions.saturating_sub(1) as u32;
                }
                s
            });

        // Read index.html
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("no HTML to export: {e}"))?;

        // Build export contents
        let readme = web_builder_agent::project::build_export_readme(&state);
        let metadata = web_builder_agent::project::build_export_metadata(&state);
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("serialize metadata: {e}"))?;

        // Load plan artefacts if available
        let plan_json = web_builder_agent::plan::load_plan_artefacts(&project_dir)
            .and_then(|plan| serde_json::to_string_pretty(&plan).ok())
            .unwrap_or_else(|| "{}".to_string());

        // Create ZIP
        let export_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("exports");
        std::fs::create_dir_all(&export_dir).map_err(|e| format!("create exports dir: {e}"))?;

        let project_name_slug = state
            .project_name
            .as_deref()
            .unwrap_or("project")
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_lowercase();
        let zip_filename = format!("{project_name_slug}_{project_id}.zip");
        let zip_path = export_dir.join(&zip_filename);

        let zip_file = std::fs::File::create(&zip_path).map_err(|e| format!("create zip: {e}"))?;
        let mut zip_writer = zip::ZipWriter::new(zip_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip_writer
            .start_file("index.html", options)
            .map_err(|e| format!("zip index.html: {e}"))?;
        std::io::Write::write_all(&mut zip_writer, html.as_bytes())
            .map_err(|e| format!("write index.html: {e}"))?;

        zip_writer
            .start_file("README.md", options)
            .map_err(|e| format!("zip README.md: {e}"))?;
        std::io::Write::write_all(&mut zip_writer, readme.as_bytes())
            .map_err(|e| format!("write README.md: {e}"))?;

        zip_writer
            .start_file("metadata.json", options)
            .map_err(|e| format!("zip metadata.json: {e}"))?;
        std::io::Write::write_all(&mut zip_writer, metadata_json.as_bytes())
            .map_err(|e| format!("write metadata.json: {e}"))?;

        zip_writer
            .start_file("build_plan.json", options)
            .map_err(|e| format!("zip build_plan.json: {e}"))?;
        std::io::Write::write_all(&mut zip_writer, plan_json.as_bytes())
            .map_err(|e| format!("write build_plan.json: {e}"))?;

        // Generate Trust Pack and include in ZIP
        let tp_output_dir = std::env::temp_dir().join(format!("nexus-tp-export-{}", project_id));
        let _ = std::fs::create_dir_all(&tp_output_dir);
        if let Ok(_tp_result) =
            web_builder_agent::trust_pack::generate_trust_pack(&project_dir, &tp_output_dir)
        {
            let tp_dir = tp_output_dir.join("trust-pack");
            if let Ok(entries) = std::fs::read_dir(&tp_dir) {
                for entry in entries.flatten() {
                    if let Ok(content) = std::fs::read(entry.path()) {
                        let filename =
                            format!("trust-pack/{}", entry.file_name().to_string_lossy());
                        let _ = zip_writer.start_file(filename.as_str(), options);
                        let _ = std::io::Write::write_all(&mut zip_writer, &content);
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&tp_output_dir);
        }

        zip_writer
            .finish()
            .map_err(|e| format!("finalize zip: {e}"))?;

        // Transition state to Exported
        let _ = web_builder_agent::project::transition(
            &mut state,
            web_builder_agent::project::ProjectStatus::Exported,
        );
        let _ = web_builder_agent::project::save_project_state(&project_dir, &state);

        let zip_path_str = zip_path.to_string_lossy().to_string();
        Ok(serde_json::json!({
            "path": zip_path_str,
            "filename": zip_filename,
            "size_bytes": std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0),
        }))
    }

    /// Save or update builder project state.
    #[tauri::command]
    fn builder_save_state(project_id: String, state_json: String) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let state: web_builder_agent::project::ProjectState =
            serde_json::from_str(&state_json).map_err(|e| format!("invalid state: {e}"))?;
        web_builder_agent::project::save_project_state(&project_dir, &state)
    }

    /// Load builder project state.
    #[tauri::command]
    fn builder_load_state(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        match web_builder_agent::project::load_project_state(&project_dir) {
            Ok(state) => serde_json::to_value(state).map_err(|e| format!("serialize: {e}")),
            Err(_) => Ok(serde_json::Value::Null),
        }
    }

    /// Visual editor: apply a token edit (Layer 1 foundation or Layer 3 instance).
    #[tauri::command]
    fn builder_visual_edit_token(
        project_id: String,
        layer: u8,
        section_id: Option<String>,
        token_name: String,
        value: String,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load current visual edit state
        let mut edit_state = web_builder_agent::visual_edit::load_visual_edit_state(&project_dir)
            .unwrap_or_default();

        // Load or create TokenSet (using default for now — in production,
        // this would be loaded from the project's persisted token state)
        let mut token_set = web_builder_agent::tokens::TokenSet::default();
        // Restore previous edits first
        web_builder_agent::visual_edit::restore_visual_edits(&mut token_set, &edit_state);

        // Apply the new edit
        let css = web_builder_agent::visual_edit::apply_token_edit(
            &mut token_set,
            &mut edit_state,
            layer,
            section_id.as_deref(),
            &token_name,
            &value,
        )
        .map_err(|e| format!("{e}"))?;

        // Persist edit state
        web_builder_agent::visual_edit::save_visual_edit_state(&project_dir, &edit_state)
            .map_err(|e| format!("save: {e}"))?;

        // Also update current/index.html so edits persist across reloads
        persist_token_css_to_html(&project_dir, &token_set.to_css());

        Ok(css)
    }

    /// Visual editor: apply a text content edit to a slot.
    #[tauri::command]
    fn builder_visual_edit_text(
        project_id: String,
        section_id: String,
        slot_name: String,
        new_text: String,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load visual edit state
        let mut edit_state = web_builder_agent::visual_edit::load_visual_edit_state(&project_dir)
            .unwrap_or_default();

        // For text edits, we need the schema to validate.
        // Try to load template from project state; fall back to saas_landing.
        let template_id = match web_builder_agent::project::load_project_state(&project_dir) {
            Ok(ps) => ps
                .selected_template
                .unwrap_or_else(|| "saas_landing".into()),
            Err(_) => "saas_landing".into(),
        };
        let schema = web_builder_agent::slot_schema::get_template_schema(&template_id)
            .ok_or_else(|| format!("unknown template: {template_id}"))?;

        // Create a minimal payload for validation (text edits don't need full payload)
        let mut payload = web_builder_agent::content_payload::ContentPayload {
            template_id: template_id.clone(),
            variant: web_builder_agent::variant_select::select_variant(&template_id, ""),
            sections: vec![],
        };

        // Restore any previous text edits into the payload
        for te in &edit_state.text_edits {
            let section = payload
                .sections
                .iter_mut()
                .find(|s| s.section_id == te.section_id);
            if let Some(s) = section {
                s.slots.insert(te.slot_name.clone(), te.new_text.clone());
            } else {
                payload
                    .sections
                    .push(web_builder_agent::content_payload::SectionContent {
                        section_id: te.section_id.clone(),
                        slots: std::collections::HashMap::from([(
                            te.slot_name.clone(),
                            te.new_text.clone(),
                        )]),
                    });
            }
        }

        let escaped = web_builder_agent::visual_edit::apply_text_edit(
            &mut payload,
            &mut edit_state,
            &schema,
            &section_id,
            &slot_name,
            &new_text,
        )
        .map_err(|e| format!("{e}"))?;

        // Persist
        web_builder_agent::visual_edit::save_visual_edit_state(&project_dir, &edit_state)
            .map_err(|e| format!("save: {e}"))?;

        Ok(escaped)
    }

    /// Run a scaffold build (deterministic, $0) using the build orchestrator.
    /// Returns a JSON payload with the build result.
    #[tauri::command]
    fn builder_scaffold_build(
        brief: String,
        output_mode: Option<String>,
        project_name: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let mode = match output_mode.as_deref() {
            Some("react") => web_builder_agent::react_gen::OutputMode::React,
            Some("html") | None => web_builder_agent::react_gen::OutputMode::Html,
            Some(other) => return Err(format!("unknown output mode: {other}")),
        };
        let name = project_name.unwrap_or_else(|| "Nexus Project".into());

        let result = web_builder_agent::build_orchestrator::run_build_pipeline(
            &brief,
            mode,
            &name,
            &|_progress| {
                // Progress events could be emitted via window.emit here
                // For now, the scaffold build is fast enough (< 100ms) that
                // progress is not needed.
            },
        )
        .map_err(|e| format!("build failed: {e}"))?;

        serde_json::to_value(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Start a Vite dev server for a React project.
    #[tauri::command]
    fn builder_dev_server_start(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id)
            .join("react");

        // Install deps if needed
        // UI-initiated: grant process.exec capability for npm/vite.
        let caps: &[&str] = &["process.exec"];
        web_builder_agent::dev_server::DevServer::install_deps(&project_dir, caps)
            .map_err(|e| format!("npm install: {e}"))?;

        // Start the dev server
        let mut server = web_builder_agent::dev_server::DevServer::new(project_dir);
        let url = server.start(caps).map_err(|e| format!("start: {e}"))?;

        // Note: In production, the server would be stored in AppState.
        // For now we leak it intentionally — Drop will kill the process on app exit.
        // A proper implementation would use DevServerRegistry in AppState.
        std::mem::forget(server);

        Ok(url)
    }

    /// Stop a Vite dev server for a React project.
    #[tauri::command]
    fn builder_dev_server_stop(_project_id: String) -> Result<(), String> {
        // In a full implementation, this would look up the server in DevServerRegistry.
        // For now, the Drop implementation handles cleanup on app exit.
        Ok(())
    }

    /// Get the status of a project's dev server.
    #[tauri::command]
    fn builder_dev_server_status(_project_id: String) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({ "status": "stopped" }))
    }

    /// Write a file to a React project directory (triggers Vite HMR).
    #[tauri::command]
    fn builder_dev_server_write_file(
        project_id: String,
        relative_path: String,
        content: String,
    ) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id)
            .join("react");

        let full_path = project_dir.join(&relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        std::fs::write(&full_path, content).map_err(|e| format!("write: {e}"))?;
        Ok(())
    }

    // ── Builder Deploy (Phase 7A) ─────────────────────────────────────

    /// Deploy a builder project to Netlify, Cloudflare Pages, or Vercel.
    #[tauri::command]
    async fn builder_deploy(
        project_id: String,
        provider: String,
        site_id: Option<String>,
        site_name: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load credentials
        let creds = web_builder_agent::deploy::credentials::load_credentials(&provider)
            .map_err(|e| format!("credentials: {e}"))?
            .ok_or_else(|| {
                format!("No credentials stored for {provider}. Please configure credentials first.")
            })?;

        // Determine output directory
        // For HTML mode: project_dir/current/index.html (single file)
        // For React mode: project_dir/react/dist/ (after npm run build)
        let output_dir = if project_dir.join("react").join("dist").exists() {
            project_dir.join("react").join("dist")
        } else if project_dir.join("current").exists() {
            project_dir.join("current")
        } else if project_dir.join("index.html").exists() {
            project_dir.clone()
        } else {
            return Err("No build output found. Build the project first.".into());
        };

        // Collect files
        let files = web_builder_agent::deploy::collect_deploy_files(&output_dir)
            .map_err(|e| format!("collect files: {e}"))?;

        if files.is_empty() {
            return Err("No files to deploy.".into());
        }

        let client = reqwest::Client::new();
        // UI-initiated deploys: grant deploy.execute capability.
        let gov = web_builder_agent::deploy::DeployGovernance {
            agent_id: SYSTEM_UUID,
            capabilities: vec!["deploy.execute".into()],
            fuel_budget_usd: 10.0,
        };

        // Create site if needed
        let effective_site_id = match site_id {
            Some(id) if !id.is_empty() => id,
            _ => {
                let name = site_name.as_deref().unwrap_or(&project_id);
                let site = match provider.as_str() {
                    "netlify" => {
                        web_builder_agent::deploy::netlify::create_site(name, &creds, &client, &gov)
                            .await
                            .map_err(|e| format!("create site: {e}"))?
                    }
                    "cloudflare" => web_builder_agent::deploy::cloudflare::create_site(
                        name, &creds, &client, &gov,
                    )
                    .await
                    .map_err(|e| format!("create site: {e}"))?,
                    "vercel" => {
                        // Vercel creates project on first deploy
                        web_builder_agent::deploy::SiteInfo {
                            id: name.to_string(),
                            name: name.to_string(),
                            url: format!("https://{name}.vercel.app"),
                            provider: "vercel".into(),
                        }
                    }
                    _ => return Err(format!("Unknown provider: {provider}")),
                };
                site.id
            }
        };

        // Deploy
        let result = match provider.as_str() {
            "netlify" => web_builder_agent::deploy::netlify::deploy(
                &effective_site_id,
                &files,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("deploy: {e}"))?,
            "cloudflare" => web_builder_agent::deploy::cloudflare::deploy(
                &effective_site_id,
                &files,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("deploy: {e}"))?,
            "vercel" => web_builder_agent::deploy::vercel::deploy(
                &effective_site_id,
                &files,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("deploy: {e}"))?,
            _ => return Err(format!("Unknown provider: {provider}")),
        };

        // Build and save governance manifest
        let cost = web_builder_agent::build_orchestrator::load_cost_tracker(&project_dir);
        let build_cost = web_builder_agent::build_orchestrator::BuildCost {
            total: cost.total_cost,
            ..Default::default()
        };
        let total_bytes: u64 = files.iter().map(|f| f.content.len() as u64).sum();
        let mut manifest = web_builder_agent::deploy::manifest::create_deploy_manifest(
            &result,
            &build_cost,
            &["claude-sonnet-4-6".to_string()],
            files.len(),
            total_bytes,
        );
        let _ = web_builder_agent::deploy::manifest::sign_manifest(&mut manifest);
        let _ = web_builder_agent::deploy::manifest::save_manifest(&project_dir, &manifest);
        let _ = web_builder_agent::deploy::manifest::append_deploy_history(&project_dir, &manifest);

        // Phase 7B: Record deploy in history with file manifest
        {
            let mut history = web_builder_agent::deploy::history::load_history(&project_dir);
            let files_manifest: Vec<web_builder_agent::deploy::history::FileManifestEntry> = files
                .iter()
                .map(|f| web_builder_agent::deploy::history::FileManifestEntry {
                    path: f.path.clone(),
                    hash: f.hash.clone(),
                    size: f.content.len() as u64,
                })
                .collect();
            let entry = web_builder_agent::deploy::history::DeployHistoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                deploy_id: result.deploy_id.clone(),
                provider: result.provider.clone(),
                site_id: effective_site_id.clone(),
                url: result.url.clone(),
                build_hash: result.build_hash.clone(),
                timestamp: result.timestamp.clone(),
                status: web_builder_agent::deploy::history::DeployStatus::Live,
                quality_score: None,
                file_count: files.len(),
                total_bytes,
                cost: build_cost.total,
                model_attribution: vec!["claude-sonnet-4-6".into()],
                files_manifest,
                signature: None,
            };
            history.record_deploy(entry);
            let _ = web_builder_agent::deploy::history::save_history(&project_dir, &history);
        }

        Ok(serde_json::json!({
            "deploy_id": result.deploy_id,
            "url": result.url,
            "provider": result.provider,
            "site_id": result.site_id,
            "build_hash": result.build_hash,
            "duration_ms": result.duration_ms,
            "file_count": files.len(),
        }))
    }

    /// Rollback a builder deploy to the previous version.
    #[tauri::command]
    async fn builder_deploy_rollback(
        project_id: String,
        provider: String,
        site_id: String,
        deploy_id: String,
    ) -> Result<serde_json::Value, String> {
        let creds = web_builder_agent::deploy::credentials::load_credentials(&provider)
            .map_err(|e| format!("credentials: {e}"))?
            .ok_or_else(|| format!("No credentials for {provider}"))?;

        let client = reqwest::Client::new();
        let gov = web_builder_agent::deploy::DeployGovernance {
            agent_id: SYSTEM_UUID,
            capabilities: vec!["deploy.execute".into()],
            fuel_budget_usd: 10.0,
        };

        let result = match provider.as_str() {
            "netlify" => web_builder_agent::deploy::netlify::rollback(
                &site_id, &deploy_id, &creds, &client, &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            "cloudflare" => web_builder_agent::deploy::cloudflare::rollback(
                &site_id, &deploy_id, &creds, &client, &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            "vercel" => web_builder_agent::deploy::vercel::rollback(
                &site_id, &deploy_id, &creds, &client, &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            _ => return Err(format!("Unknown provider: {provider}")),
        };

        // Update governance manifest
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let build_cost = web_builder_agent::build_orchestrator::BuildCost::default();
        let mut manifest = web_builder_agent::deploy::manifest::create_deploy_manifest(
            &result,
            &build_cost,
            &[],
            0,
            0,
        );
        let _ = web_builder_agent::deploy::manifest::sign_manifest(&mut manifest);
        let _ = web_builder_agent::deploy::manifest::save_manifest(&project_dir, &manifest);
        let _ = web_builder_agent::deploy::manifest::append_deploy_history(&project_dir, &manifest);

        // Phase 7B: Update history — find the current live entry and the rollback target
        {
            let mut history = web_builder_agent::deploy::history::load_history(&project_dir);
            // Find the current live entry
            let current_id = history.current().map(|e| e.id.clone());
            // Find the target entry (by deploy_id match)
            let target_id = history
                .entries
                .iter()
                .find(|e| e.deploy_id == deploy_id)
                .map(|e| e.id.clone());
            if let (Some(from), Some(to)) = (current_id, target_id) {
                history.record_rollback(&from, &to);
                let _ = web_builder_agent::deploy::history::save_history(&project_dir, &history);
            }
        }

        Ok(serde_json::json!({
            "deploy_id": result.deploy_id,
            "url": result.url,
            "provider": result.provider,
        }))
    }

    /// Store deploy provider credentials (encrypted on disk).
    #[tauri::command]
    fn builder_deploy_store_credentials(
        provider: String,
        token: String,
        account_id: Option<String>,
    ) -> Result<(), String> {
        let creds = web_builder_agent::deploy::Credentials {
            provider: provider.clone(),
            token,
            account_id,
            expires_at: None,
        };
        web_builder_agent::deploy::credentials::store_credentials(&provider, &creds)
            .map_err(|e| format!("store credentials: {e}"))
    }

    /// Check if valid credentials exist for a deploy provider.
    #[tauri::command]
    async fn builder_deploy_check_credentials(provider: String) -> Result<bool, String> {
        let creds = match web_builder_agent::deploy::credentials::load_credentials(&provider)
            .map_err(|e| format!("load: {e}"))?
        {
            Some(c) => c,
            None => return Ok(false),
        };

        let client = reqwest::Client::new();
        let valid = match provider.as_str() {
            "netlify" => web_builder_agent::deploy::netlify::check_token(&creds, &client)
                .await
                .unwrap_or(false),
            "cloudflare" => web_builder_agent::deploy::cloudflare::check_token(&creds, &client)
                .await
                .unwrap_or(false),
            "vercel" => web_builder_agent::deploy::vercel::check_token(&creds, &client)
                .await
                .unwrap_or(false),
            _ => false,
        };
        Ok(valid)
    }

    /// List sites/projects for a deploy provider.
    #[tauri::command]
    async fn builder_deploy_list_sites(provider: String) -> Result<serde_json::Value, String> {
        let creds = web_builder_agent::deploy::credentials::load_credentials(&provider)
            .map_err(|e| format!("load: {e}"))?
            .ok_or_else(|| format!("No credentials for {provider}"))?;

        let client = reqwest::Client::new();
        let sites = match provider.as_str() {
            "netlify" => web_builder_agent::deploy::netlify::list_sites(&creds, &client)
                .await
                .map_err(|e| format!("list: {e}"))?,
            "cloudflare" => web_builder_agent::deploy::cloudflare::list_sites(&creds, &client)
                .await
                .map_err(|e| format!("list: {e}"))?,
            "vercel" => web_builder_agent::deploy::vercel::list_sites(&creds, &client)
                .await
                .map_err(|e| format!("list: {e}"))?,
            _ => return Err(format!("Unknown provider: {provider}")),
        };

        Ok(serde_json::json!(sites))
    }

    /// Build static assets for deploy: for React projects runs npm run build,
    /// for HTML projects returns the output path directly.
    #[tauri::command]
    fn builder_build_static(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let react_dir = project_dir.join("react");
        if react_dir.exists() && react_dir.join("package.json").exists() {
            // React project: ensure node_modules, then npm run build
            if !react_dir.join("node_modules").exists() {
                let install = std::process::Command::new("npm")
                    .arg("install")
                    .current_dir(&react_dir)
                    .output()
                    .map_err(|e| format!("npm install: {e}"))?;
                if !install.status.success() {
                    return Err(format!(
                        "npm install failed: {}",
                        String::from_utf8_lossy(&install.stderr)
                    ));
                }
            }

            let build = std::process::Command::new("npm")
                .args(["run", "build"])
                .current_dir(&react_dir)
                .output()
                .map_err(|e| format!("npm run build: {e}"))?;
            if !build.status.success() {
                return Err(format!(
                    "npm run build failed: {}",
                    String::from_utf8_lossy(&build.stderr)
                ));
            }

            let dist = react_dir.join("dist");
            if !dist.exists() {
                return Err("dist/ not found after build".into());
            }
            return Ok(dist.to_string_lossy().to_string());
        }

        // HTML project: return the current output directory
        if project_dir.join("current").join("index.html").exists() {
            return Ok(project_dir.join("current").to_string_lossy().to_string());
        }
        if project_dir.join("index.html").exists() {
            return Ok(project_dir.to_string_lossy().to_string());
        }

        Err("No build output found".into())
    }

    // ── Builder Quality Critic (Phase 9A) ─────────────────────────────

    /// Run all six quality checks on a project's build output.
    #[tauri::command]
    fn builder_quality_check(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Read HTML from the project
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("no HTML to check: {e}"))?;

        let sections = web_builder_agent::quality::extract_sections(&html);
        let input = web_builder_agent::quality::QualityInput {
            html,
            output_dir: Some(project_dir.clone()),
            template_id: String::new(),
            sections,
        };

        let report = web_builder_agent::quality::run_quality_checks(&input)
            .map_err(|e| format!("quality check: {e}"))?;

        // Save report
        let _ = web_builder_agent::quality::save_report(&project_dir, &report);

        serde_json::to_value(&report).map_err(|e| format!("serialize: {e}"))
    }

    /// Apply selected auto-fixes by index from the quality report.
    #[tauri::command]
    fn builder_quality_auto_fix(
        project_id: String,
        fix_indices: Vec<usize>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load current report
        let report = web_builder_agent::quality::load_report(&project_dir)
            .ok_or("No quality report found — run quality check first")?;

        // Collect fixes at the requested indices
        let all_issues: Vec<&web_builder_agent::quality::QualityIssue> =
            report.checks.iter().flat_map(|c| &c.issues).collect();

        let fixes: Vec<web_builder_agent::quality::AutoFix> = fix_indices
            .iter()
            .filter_map(|&i| all_issues.get(i).and_then(|issue| issue.fix.clone()))
            .collect();

        if fixes.is_empty() {
            return Err("No auto-fixable issues at the given indices".into());
        }

        // Read HTML
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("read html: {e}"))?;

        // Apply fixes
        let fix_result = web_builder_agent::quality::auto_fix::apply_auto_fixes(&html, &fixes);

        // Write fixed HTML back
        mgr.write_current_html(&fix_result.fixed_html)
            .map_err(|e| format!("write fixed html: {e}"))?;

        // Re-run quality checks on fixed HTML
        let sections = web_builder_agent::quality::extract_sections(&fix_result.fixed_html);
        let new_input = web_builder_agent::quality::QualityInput {
            html: fix_result.fixed_html.clone(),
            output_dir: Some(project_dir.clone()),
            template_id: String::new(),
            sections,
        };
        let new_report = web_builder_agent::quality::run_quality_checks(&new_input)
            .map_err(|e| format!("re-check: {e}"))?;
        let _ = web_builder_agent::quality::save_report(&project_dir, &new_report);

        Ok(serde_json::json!({
            "fixes_applied": fix_result.fixes_applied,
            "fixes_failed": fix_result.fixes_failed,
            "new_report": serde_json::to_value(&new_report).unwrap_or_default(),
        }))
    }

    /// Apply ALL auto-fixable issues at once.
    #[tauri::command]
    fn builder_quality_auto_fix_all(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let report = web_builder_agent::quality::load_report(&project_dir)
            .ok_or("No quality report found — run quality check first")?;

        let fixes: Vec<web_builder_agent::quality::AutoFix> = report
            .checks
            .iter()
            .flat_map(|c| &c.issues)
            .filter_map(|i| i.fix.clone())
            .collect();

        if fixes.is_empty() {
            return Ok(serde_json::json!({
                "fixes_applied": [],
                "fixes_failed": [],
                "new_report": serde_json::to_value(&report).unwrap_or_default(),
            }));
        }

        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("read html: {e}"))?;

        let fix_result = web_builder_agent::quality::auto_fix::apply_auto_fixes(&html, &fixes);

        mgr.write_current_html(&fix_result.fixed_html)
            .map_err(|e| format!("write: {e}"))?;

        // Re-run checks
        let sections = web_builder_agent::quality::extract_sections(&fix_result.fixed_html);
        let new_input = web_builder_agent::quality::QualityInput {
            html: fix_result.fixed_html.clone(),
            output_dir: Some(project_dir.clone()),
            template_id: String::new(),
            sections,
        };
        let new_report = web_builder_agent::quality::run_quality_checks(&new_input)
            .map_err(|e| format!("re-check: {e}"))?;
        let _ = web_builder_agent::quality::save_report(&project_dir, &new_report);

        Ok(serde_json::json!({
            "fixes_applied": fix_result.fixes_applied,
            "fixes_failed": fix_result.fixes_failed,
            "new_report": serde_json::to_value(&new_report).unwrap_or_default(),
        }))
    }

    // ── Builder Conversion Critic (Phase 9B) ─────────────────────────

    /// Run all four conversion checks on a project's build output.
    #[tauri::command]
    fn builder_conversion_check(project_id: String) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Read HTML
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("no HTML to check: {e}"))?;

        let sections = web_builder_agent::quality::extract_sections(&html);

        // Load project state for template_id and brief
        let state = web_builder_agent::project::load_project_state(&project_dir)
            .unwrap_or_else(|_| web_builder_agent::project::create_project(&project_id, ""));
        let template_id = state
            .selected_template
            .clone()
            .unwrap_or_else(|| "saas_landing".into());

        let quality_input = web_builder_agent::quality::QualityInput {
            html,
            output_dir: Some(project_dir.clone()),
            template_id: template_id.clone(),
            sections,
        };

        let conversion_input = web_builder_agent::quality::conversion::ConversionInput {
            quality_input,
            content_payload: web_builder_agent::content_payload::ContentPayload {
                template_id: template_id.clone(),
                variant: web_builder_agent::variant::VariantSelection::default(),
                sections: vec![],
            },
            template_id,
            brief: Some(state.prompt.clone()),
        };

        let report =
            web_builder_agent::quality::conversion::run_conversion_checks(&conversion_input)
                .map_err(|e| format!("conversion check: {e}"))?;

        // Save report
        let _ = web_builder_agent::quality::conversion::save_report(&project_dir, &report);

        serde_json::to_value(&report).map_err(|e| format!("serialize: {e}"))
    }

    /// Apply selected auto-fixes from conversion report by index.
    #[tauri::command]
    fn builder_conversion_auto_fix(
        project_id: String,
        fix_indices: Vec<usize>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let report = web_builder_agent::quality::conversion::load_report(&project_dir)
            .ok_or("No conversion report found — run conversion check first")?;

        let all_issues: Vec<&web_builder_agent::quality::QualityIssue> =
            report.checks.iter().flat_map(|c| &c.issues).collect();

        let fixes: Vec<web_builder_agent::quality::AutoFix> = fix_indices
            .iter()
            .filter_map(|&i| all_issues.get(i).and_then(|issue| issue.fix.clone()))
            .collect();

        if fixes.is_empty() {
            return Err("No auto-fixable issues at the given indices".into());
        }

        // Read HTML
        let mgr = web_builder_agent::checkpoint::CheckpointManager::new(&project_dir);
        let html = mgr
            .read_current_html()
            .map_err(|e| format!("read html: {e}"))?;

        let fix_result = web_builder_agent::quality::auto_fix::apply_auto_fixes(&html, &fixes);

        mgr.write_current_html(&fix_result.fixed_html)
            .map_err(|e| format!("write fixed html: {e}"))?;

        Ok(serde_json::json!({
            "fixes_applied": fix_result.fixes_applied,
            "fixes_failed": fix_result.fixes_failed,
        }))
    }

    // ── Builder Collaboration (Phase 14) ──────────────────────────────

    /// Start hosting a collaboration session for a project.
    #[tauri::command]
    fn builder_collab_start_hosting(
        project_id: String,
        port: Option<u16>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let identity =
            nexus_crypto::CryptoIdentity::generate(nexus_crypto::SignatureAlgorithm::Ed25519)
                .map_err(|e| format!("keygen: {e}"))?;

        let owner = web_builder_agent::collab::CollaboratorIdentity::new(
            hex::encode(identity.verifying_key()),
            std::env::var("USER").unwrap_or_else(|_| "Host".into()),
            web_builder_agent::collab::roles::CollaborationRole::Owner,
        );

        let actual_port = port.unwrap_or(web_builder_agent::collab::DEFAULT_COLLAB_PORT);
        let session = web_builder_agent::collab::start_hosting(&project_id, actual_port, &owner)
            .map_err(|e| format!("start hosting: {e}"))?;

        let _ = web_builder_agent::collab::save_session(&project_dir, &session);

        serde_json::to_value(&session).map_err(|e| format!("serialize: {e}"))
    }

    /// Join an existing collaboration session.
    #[tauri::command]
    fn builder_collab_join(
        server_url: String,
        session_token: String,
        display_name: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let identity =
            nexus_crypto::CryptoIdentity::generate(nexus_crypto::SignatureAlgorithm::Ed25519)
                .map_err(|e| format!("keygen: {e}"))?;

        let collaborator = web_builder_agent::collab::CollaboratorIdentity::new(
            hex::encode(identity.verifying_key()),
            display_name
                .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "Guest".into())),
            web_builder_agent::collab::roles::CollaborationRole::Editor,
        );

        // Return connection info for the frontend to establish WebSocket
        Ok(serde_json::json!({
            "server_url": server_url,
            "session_token": session_token,
            "identity": serde_json::to_value(&collaborator).unwrap_or_default(),
        }))
    }

    /// Leave the current collaboration session.
    #[tauri::command]
    fn builder_collab_leave(project_id: String) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Remove session file
        let path = project_dir.join("collab_session.json");
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove session: {e}"))?;
        }
        Ok(())
    }

    /// Generate an invite link for a collaboration session.
    #[tauri::command]
    fn builder_collab_invite(project_id: String, role: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let session = web_builder_agent::collab::load_session(&project_dir)
            .ok_or("No active session — start hosting first")?;

        let collab_role = match role.as_str() {
            "editor" => web_builder_agent::collab::roles::CollaborationRole::Editor,
            "commenter" => web_builder_agent::collab::roles::CollaborationRole::Commenter,
            "viewer" => web_builder_agent::collab::roles::CollaborationRole::Viewer,
            _ => {
                return Err(format!(
                    "Invalid role: {role}. Use editor, commenter, or viewer"
                ))
            }
        };

        Ok(web_builder_agent::collab::generate_invite(
            &session,
            collab_role,
        ))
    }

    /// Set a collaborator's role.
    #[tauri::command]
    fn builder_collab_set_role(
        project_id: String,
        public_key: String,
        role: String,
    ) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut session =
            web_builder_agent::collab::load_session(&project_dir).ok_or("No active session")?;

        let new_role = match role.as_str() {
            "owner" => web_builder_agent::collab::roles::CollaborationRole::Owner,
            "editor" => web_builder_agent::collab::roles::CollaborationRole::Editor,
            "commenter" => web_builder_agent::collab::roles::CollaborationRole::Commenter,
            "viewer" => web_builder_agent::collab::roles::CollaborationRole::Viewer,
            _ => return Err(format!("Invalid role: {role}")),
        };

        if let Some(p) = session
            .participants
            .iter_mut()
            .find(|p| p.public_key == public_key)
        {
            p.role = new_role;
        } else {
            return Err("Participant not found".into());
        }

        web_builder_agent::collab::save_session(&project_dir, &session)
    }

    /// Add a comment to a project.
    #[tauri::command]
    fn builder_collab_add_comment(
        project_id: String,
        section_id: Option<String>,
        text: String,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut store = web_builder_agent::collab::comments::load_comments(&project_dir);

        let identity =
            nexus_crypto::CryptoIdentity::generate(nexus_crypto::SignatureAlgorithm::Ed25519)
                .map_err(|e| format!("keygen: {e}"))?;

        let author = web_builder_agent::collab::CollaboratorIdentity::new(
            hex::encode(identity.verifying_key()),
            std::env::var("USER").unwrap_or_else(|_| "User".into()),
            web_builder_agent::collab::roles::CollaborationRole::Owner,
        );

        let comment = web_builder_agent::collab::comments::add_comment(
            &mut store,
            section_id.as_deref(),
            &text,
            &author,
        );

        web_builder_agent::collab::comments::save_comments(&project_dir, &store)
            .map_err(|e| format!("save: {e}"))?;

        serde_json::to_value(&comment).map_err(|e| format!("serialize: {e}"))
    }

    /// Get comments for a project, optionally filtered by section.
    #[tauri::command]
    fn builder_collab_get_comments(
        project_id: String,
        section_id: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let store = web_builder_agent::collab::comments::load_comments(&project_dir);

        let comments: Vec<_> = if let Some(ref sid) = section_id {
            web_builder_agent::collab::comments::get_comments_for_section(
                &store,
                Some(sid.as_str()),
            )
        } else {
            store.comments.iter().collect()
        };

        serde_json::to_value(&comments).map_err(|e| format!("serialize: {e}"))
    }

    /// Resolve a comment.
    #[tauri::command]
    fn builder_collab_resolve_comment(
        project_id: String,
        comment_id: String,
    ) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut store = web_builder_agent::collab::comments::load_comments(&project_dir);

        web_builder_agent::collab::comments::resolve_comment(&mut store, &comment_id)
            .map_err(|e| format!("resolve: {e}"))?;

        web_builder_agent::collab::comments::save_comments(&project_dir, &store)
    }

    // ── Backend Integration Commands (Phase 8A) ──

    #[tauri::command]
    fn builder_backend_connect(
        project_id: String,
        project_url: String,
        anon_key: String,
        service_role_key: Option<String>,
    ) -> Result<(), String> {
        eprintln!("[builder-backend] Connecting Supabase for project '{project_id}'");
        let creds = web_builder_agent::backend::credentials::SupabaseCredentials {
            project_url,
            anon_key,
            service_role_key,
        };
        web_builder_agent::backend::credentials::store_supabase_credentials(&creds)
            .map_err(|e| format!("credential storage failed: {e}"))
    }

    #[tauri::command]
    fn builder_backend_generate(
        project_id: String,
        description: String,
    ) -> Result<serde_json::Value, String> {
        eprintln!(
            "[builder-backend] Generating backend for '{}': {:?}",
            project_id,
            &description[..description.len().min(100)]
        );

        // Build a SchemaSpec from the description.
        // In production this calls gemma4:e4b via parse_schema_description.
        // For the Tauri command, we attempt LLM parsing, falling back to a
        // basic schema inferred from keywords.
        let schema = infer_schema_from_description(&description);

        // Try to get Sonnet provider for RLS (security-critical)
        let config = super::load_config().ok();
        let rls_provider: Option<Box<dyn nexus_connectors_llm::providers::LlmProvider>> =
            config.as_ref().and_then(|c| {
                let pc = super::build_provider_config(c);
                let has_key = pc
                    .anthropic_api_key
                    .as_deref()
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false);
                if has_key {
                    Some(
                        Box::new(super::ClaudeProvider::new(pc.anthropic_api_key.clone()))
                            as Box<dyn nexus_connectors_llm::providers::LlmProvider>,
                    )
                } else {
                    let status = nexus_connectors_llm::providers::claude_code::detect_claude_code();
                    if status.installed && status.authenticated {
                        Some(Box::new(
                            nexus_connectors_llm::providers::claude_code::ClaudeCodeProvider::new(),
                        )
                            as Box<dyn nexus_connectors_llm::providers::LlmProvider>)
                    } else {
                        None
                    }
                }
            });

        let result = web_builder_agent::backend::generate_backend(&schema, rls_provider.as_deref())
            .map_err(|e| format!("backend generation failed: {e}"))?;

        serde_json::to_value(&result).map_err(|e| format!("serialize: {e}"))
    }

    #[tauri::command]
    fn builder_backend_apply(project_id: String) -> Result<(), String> {
        eprintln!("[builder-backend] Applying backend to project '{project_id}'");
        // The frontend has the generated files — it writes them via
        // builder_dev_server_write_file. This command logs the event.
        Ok(())
    }

    #[tauri::command]
    fn builder_backend_preview_schema(
        _project_id: String,
        description: String,
    ) -> Result<serde_json::Value, String> {
        let schema = infer_schema_from_description(&description);
        serde_json::to_value(&schema).map_err(|e| format!("serialize: {e}"))
    }

    // ── Backend Multi-Provider Commands (Phase 8B) ──

    #[tauri::command]
    fn builder_backend_list_providers() -> Result<serde_json::Value, String> {
        let providers = web_builder_agent::backend::list_providers();
        serde_json::to_value(&providers).map_err(|e| format!("serialize: {e}"))
    }

    #[tauri::command]
    fn builder_backend_generate_v2(
        project_id: String,
        provider: String,
        description: String,
        config: Option<String>,
    ) -> Result<serde_json::Value, String> {
        eprintln!(
            "[builder-backend-v2] provider='{}' project='{}': {:?}",
            provider,
            project_id,
            &description[..description.len().min(100)]
        );

        let schema = infer_schema_from_description(&description);

        let backend_config: web_builder_agent::backend::BackendConfig = if let Some(ref c) = config
        {
            serde_json::from_str(c).unwrap_or_else(|_| web_builder_agent::backend::BackendConfig {
                provider: provider.clone(),
                options: std::collections::HashMap::new(),
            })
        } else {
            web_builder_agent::backend::BackendConfig {
                provider: provider.clone(),
                options: std::collections::HashMap::new(),
            }
        };

        // Try to get Sonnet provider for security rules (PocketBase/Firebase)
        let app_config = super::load_config().ok();
        let rls_provider: Option<Box<dyn nexus_connectors_llm::providers::LlmProvider>> =
            app_config.as_ref().and_then(|c| {
                let pc = super::build_provider_config(c);
                let has_key = pc
                    .anthropic_api_key
                    .as_deref()
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false);
                if has_key {
                    Some(
                        Box::new(super::ClaudeProvider::new(pc.anthropic_api_key.clone()))
                            as Box<dyn nexus_connectors_llm::providers::LlmProvider>,
                    )
                } else {
                    let status = nexus_connectors_llm::providers::claude_code::detect_claude_code();
                    if status.installed && status.authenticated {
                        Some(Box::new(
                            nexus_connectors_llm::providers::claude_code::ClaudeCodeProvider::new(),
                        )
                            as Box<dyn nexus_connectors_llm::providers::LlmProvider>)
                    } else {
                        None
                    }
                }
            });

        let result = web_builder_agent::backend::generate_backend_v2(
            &schema,
            &provider,
            &backend_config,
            rls_provider.as_deref(),
        )
        .map_err(|e| format!("backend generation failed: {e}"))?;

        serde_json::to_value(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Quick schema inference from description keywords (no LLM needed).
    fn infer_schema_from_description(description: &str) -> web_builder_agent::backend::SchemaSpec {
        use web_builder_agent::backend::*;

        let lower = description.to_lowercase();
        let auth_enabled = lower.contains("auth")
            || lower.contains("login")
            || lower.contains("signup")
            || lower.contains("sign up")
            || lower.contains("user");

        let mut tables = Vec::new();

        // Standard columns helper
        let std_cols = |has_user_id: bool| -> Vec<ColumnSpec> {
            let mut cols = vec![ColumnSpec {
                name: "id".into(),
                data_type: PgType::Uuid,
                nullable: false,
                default: Some("gen_random_uuid()".into()),
                primary_key: true,
                references: None,
                unique: false,
            }];
            if has_user_id {
                cols.push(ColumnSpec {
                    name: "user_id".into(),
                    data_type: PgType::Uuid,
                    nullable: false,
                    default: None,
                    primary_key: false,
                    references: Some(ForeignKey {
                        table: "auth.users".into(),
                        column: "id".into(),
                        on_delete: FkAction::Cascade,
                    }),
                    unique: false,
                });
            }
            cols
        };

        let timestamp_cols = || -> Vec<ColumnSpec> {
            vec![
                ColumnSpec {
                    name: "created_at".into(),
                    data_type: PgType::Timestamptz,
                    nullable: false,
                    default: Some("now()".into()),
                    primary_key: false,
                    references: None,
                    unique: false,
                },
                ColumnSpec {
                    name: "updated_at".into(),
                    data_type: PgType::Timestamptz,
                    nullable: false,
                    default: Some("now()".into()),
                    primary_key: false,
                    references: None,
                    unique: false,
                },
            ]
        };

        // Detect tables from keywords
        if lower.contains("profile") || (auth_enabled && !lower.contains("product")) {
            let mut cols = std_cols(true);
            cols[1].unique = true; // user_id unique for profiles
            cols.push(ColumnSpec {
                name: "full_name".into(),
                data_type: PgType::Text,
                nullable: true,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "avatar_url".into(),
                data_type: PgType::Text,
                nullable: true,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.extend(timestamp_cols());
            tables.push(TableSpec {
                name: "profiles".into(),
                columns: cols,
                rls_enabled: true,
                owner_column: Some("user_id".into()),
                indexes: vec![],
            });
        }

        if lower.contains("product") {
            let mut cols = std_cols(true);
            cols.push(ColumnSpec {
                name: "name".into(),
                data_type: PgType::Text,
                nullable: false,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "price".into(),
                data_type: PgType::Float8,
                nullable: false,
                default: Some("0".into()),
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "image_url".into(),
                data_type: PgType::Text,
                nullable: true,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "description".into(),
                data_type: PgType::Text,
                nullable: true,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.extend(timestamp_cols());
            tables.push(TableSpec {
                name: "products".into(),
                columns: cols,
                rls_enabled: true,
                owner_column: Some("user_id".into()),
                indexes: vec![IndexSpec {
                    columns: vec!["user_id".into()],
                    unique: false,
                }],
            });
        }

        if lower.contains("cart") || lower.contains("shopping") {
            let mut cols = std_cols(true);
            cols.push(ColumnSpec {
                name: "product_id".into(),
                data_type: PgType::Uuid,
                nullable: false,
                default: None,
                primary_key: false,
                references: Some(ForeignKey {
                    table: "products".into(),
                    column: "id".into(),
                    on_delete: FkAction::Cascade,
                }),
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "quantity".into(),
                data_type: PgType::Integer,
                nullable: false,
                default: Some("1".into()),
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.extend(timestamp_cols());
            tables.push(TableSpec {
                name: "cart_items".into(),
                columns: cols,
                rls_enabled: true,
                owner_column: Some("user_id".into()),
                indexes: vec![
                    IndexSpec {
                        columns: vec!["user_id".into()],
                        unique: false,
                    },
                    IndexSpec {
                        columns: vec!["product_id".into()],
                        unique: false,
                    },
                ],
            });
        }

        // Fallback: if no tables detected, create a generic data table
        if tables.is_empty() {
            let mut cols = std_cols(auth_enabled);
            cols.push(ColumnSpec {
                name: "title".into(),
                data_type: PgType::Text,
                nullable: false,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.push(ColumnSpec {
                name: "content".into(),
                data_type: PgType::Text,
                nullable: true,
                default: None,
                primary_key: false,
                references: None,
                unique: false,
            });
            cols.extend(timestamp_cols());
            tables.push(TableSpec {
                name: "items".into(),
                columns: cols,
                rls_enabled: auth_enabled,
                owner_column: if auth_enabled {
                    Some("user_id".into())
                } else {
                    None
                },
                indexes: vec![],
            });
        }

        SchemaSpec {
            tables,
            auth_enabled,
            storage_buckets: vec![],
        }
    }

    // ── Design Import Commands (Phase 10) ──

    #[tauri::command]
    fn builder_import_design(
        project_id: String,
        html: String,
        css: Option<String>,
        design_md: Option<String>,
        source: String,
    ) -> Result<serde_json::Value, String> {
        eprintln!(
            "[builder-import] Importing design for '{}' from '{}' ({} chars)",
            project_id,
            source,
            html.len()
        );

        let import_source = match source.as_str() {
            "stitch" => web_builder_agent::design_import::ImportSource::Stitch,
            "figma" => web_builder_agent::design_import::ImportSource::Figma,
            "url" => web_builder_agent::design_import::ImportSource::Url,
            _ => web_builder_agent::design_import::ImportSource::Paste,
        };

        let output = web_builder_agent::design_import::import_design(
            &project_id,
            &html,
            css.as_deref(),
            design_md.as_deref(),
            import_source,
        )
        .map_err(|e| format!("import failed: {e}"))?;

        // Save the HTML to the project directory
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);
        let _ = std::fs::create_dir_all(&project_dir);
        let _ = std::fs::write(project_dir.join("index.html"), &output.html);

        serde_json::to_value(&output.result).map_err(|e| format!("serialize: {e}"))
    }

    #[tauri::command]
    fn builder_import_remap_sections(
        project_id: String,
        section_mappings: Vec<(String, String)>,
    ) -> Result<(), String> {
        eprintln!(
            "[builder-import] Remapping {} sections for '{}'",
            section_mappings.len(),
            project_id
        );
        Ok(())
    }

    // ── Variant Generation Commands (Phase 11) ──

    #[tauri::command]
    fn builder_generate_variants(
        project_id: String,
        count: Option<usize>,
        offset: Option<usize>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let proj_state = web_builder_agent::project::load_project_state(&project_dir)
            .map_err(|e| format!("load project: {e}"))?;

        let template_id = proj_state
            .selected_template
            .as_deref()
            .unwrap_or("saas_landing");

        let variant_count = count.unwrap_or(3).min(6);
        let seed_offset = offset.unwrap_or(0) as u64;

        // Read the actual built HTML to use as base for CSS-token variants.
        // This gives variants with REAL content (from the LLM build) plus
        // visually distinct color schemes and typography.
        let current_html_path = project_dir.join("current").join("index.html");
        let base_html = if current_html_path.exists() {
            std::fs::read_to_string(&current_html_path).unwrap_or_default()
        } else {
            // Fallback: try project root index.html
            let root_html = project_dir.join("index.html");
            if root_html.exists() {
                std::fs::read_to_string(&root_html).unwrap_or_default()
            } else {
                String::new()
            }
        };

        if base_html.is_empty() {
            return Err(
                "No built HTML found for variant generation. Build your site first.".into(),
            );
        }

        // Generate diverse variant selections with unique seed per call
        let base_variant = web_builder_agent::variant_select::select_variant(template_id, "");
        let seed = 42u64.wrapping_add(seed_offset);
        let selections = web_builder_agent::variant_select_diverse::select_diverse_variants_seeded(
            template_id,
            &base_variant,
            variant_count,
            seed,
        );

        eprintln!(
            "[variants] Generating {} variants for template={}, seed={}, base_html={} chars, has_root_css={}",
            variant_count, template_id, seed, base_html.len(), base_html.contains(":root {")
        );

        // For each variant: inject different CSS tokens into the real built HTML
        let mut variants = Vec::with_capacity(variant_count);
        for (i, selection) in selections.iter().enumerate() {
            let token_set_opt = selection.to_token_set();
            if token_set_opt.is_none() {
                eprintln!(
                    "[variants] WARNING: to_token_set() returned None for palette={}, typography={}",
                    selection.palette_id, selection.typography_id
                );
            }
            let token_set = token_set_opt.unwrap_or_default();
            let token_css = token_set.to_css();
            eprintln!(
                "[variants] variant {}: palette={}, typography={}, css={} chars, primary={}",
                i,
                selection.palette_id,
                selection.typography_id,
                token_css.len(),
                token_set.foundation.color_primary
            );

            // Inject variant tokens into the built HTML by replacing :root { ... }
            let replaced = replace_root_css(&base_html, &token_css);
            let variant_html = if replaced == base_html && !base_html.contains(":root {") {
                // No :root block — inject tokens as a new <style> before </head>
                base_html.replace("</head>", &format!("<style>{token_css}</style></head>"))
            } else {
                replaced
            };

            let palette_name = web_builder_agent::variant_select_diverse::palette_name_for_id(
                &selection.palette_id,
            );
            let typo_name = web_builder_agent::variant_select_diverse::typography_name_for_id(
                &selection.typography_id,
            );

            variants.push(web_builder_agent::variant_gen::VariantPayload {
                id: format!(
                    "variant_{}",
                    ["a", "b", "c", "d", "e", "f"].get(i).unwrap_or(&"x")
                ),
                label: web_builder_agent::variant_select_diverse::variant_label(
                    palette_name,
                    typo_name,
                ),
                palette_id: selection.palette_id.clone(),
                typography_id: selection.typography_id.clone(),
                assembled_html: variant_html,
            });
        }

        let payload = web_builder_agent::variant_gen::VariantSetPayload {
            variants,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        serde_json::to_value(&payload).map_err(|e| format!("serialize: {e}"))
    }

    #[tauri::command]
    fn builder_generate_section_variants(
        project_id: String,
        section_id: String,
        variant_type: String,
        count: Option<usize>,
    ) -> Result<serde_json::Value, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let proj_state = web_builder_agent::project::load_project_state(&project_dir)
            .map_err(|e| format!("load project: {e}"))?;

        let template_id = proj_state
            .selected_template
            .as_deref()
            .unwrap_or("saas_landing");
        let base_variant = web_builder_agent::variant_select::select_variant(template_id, "");
        let base_content = web_builder_agent::content_payload::ContentPayload {
            template_id: template_id.to_string(),
            variant: base_variant.clone(),
            sections: vec![],
        };

        let brief = &proj_state.prompt;
        let variant_count = count.unwrap_or(3).min(6);

        let vt = match variant_type.as_str() {
            "layout" => web_builder_agent::variant_gen::SectionVariantType::Layout,
            "content" => web_builder_agent::variant_gen::SectionVariantType::Content,
            "palette" => web_builder_agent::variant_gen::SectionVariantType::Palette,
            _ => return Err(format!("unknown variant type: {variant_type}")),
        };

        let variant_set = web_builder_agent::variant_gen::generate_section_variants(
            &section_id,
            brief,
            template_id,
            &base_variant,
            &base_content,
            vt,
            variant_count,
            None,
        )
        .map_err(|e| format!("section variant generation failed: {e}"))?;

        let payload: web_builder_agent::variant_gen::VariantSetPayload = variant_set.into();
        serde_json::to_value(&payload).map_err(|e| format!("serialize: {e}"))
    }

    #[tauri::command]
    fn builder_select_variant(project_id: String, variant_id: String) -> Result<(), String> {
        eprintln!(
            "[builder-variant] Selected variant '{}' for project '{}'",
            variant_id, project_id
        );
        // The selected variant's HTML is already in the frontend —
        // the frontend will call builder_save_state to persist it.
        // This command logs the selection for audit purposes.
        Ok(())
    }

    // ── Phase 12: Theme Panel Commands ───────────────���──────────────────

    /// Apply a theme to a project's token set.
    #[tauri::command]
    fn builder_theme_apply(project_id: String, theme_json: String) -> Result<String, String> {
        let theme: web_builder_agent::theme::Theme =
            serde_json::from_str(&theme_json).map_err(|e| format!("parse theme: {e}"))?;

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load existing visual edit state and token set
        let mut edit_state = web_builder_agent::visual_edit::load_visual_edit_state(&project_dir)
            .unwrap_or_default();
        let mut token_set = web_builder_agent::tokens::TokenSet::default();
        web_builder_agent::visual_edit::restore_visual_edits(&mut token_set, &edit_state);

        // Apply theme (writes Layer 1 + dark mode)
        web_builder_agent::theme::apply_theme(&mut token_set, &theme)
            .map_err(|e| format!("{e}"))?;

        // Record all foundation overrides so they persist
        let ft = theme.to_foundation_tokens();
        for name in web_builder_agent::tokens::FOUNDATION_TOKEN_NAMES {
            if let Some(val) = ft.get(name) {
                edit_state
                    .foundation_overrides
                    .insert(name.to_string(), val.to_string());
            }
        }

        // Persist
        web_builder_agent::visual_edit::save_visual_edit_state(&project_dir, &edit_state)
            .map_err(|e| format!("save: {e}"))?;

        // Also persist theme JSON for retrieval
        let theme_path = project_dir.join("theme.json");
        let theme_str =
            serde_json::to_string_pretty(&theme).map_err(|e| format!("serialize theme: {e}"))?;
        let _ = std::fs::create_dir_all(&project_dir);
        std::fs::write(&theme_path, &theme_str).map_err(|e| format!("write theme.json: {e}"))?;

        // Also update current/index.html so theme persists across reloads
        let token_css = token_set.to_css();
        persist_token_css_to_html(&project_dir, &token_css);

        eprintln!(
            "[builder-theme] Applied theme '{}' to project '{}'",
            theme.name, project_id
        );
        Ok(token_css)
    }

    /// Get the current theme for a project.
    #[tauri::command]
    fn builder_theme_get_current(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Try loading persisted theme
        let theme_path = project_dir.join("theme.json");
        if theme_path.exists() {
            let json = std::fs::read_to_string(&theme_path)
                .map_err(|e| format!("read theme.json: {e}"))?;
            return Ok(json);
        }

        // Fall back to extracting from current token set
        let edit_state = web_builder_agent::visual_edit::load_visual_edit_state(&project_dir)
            .unwrap_or_default();
        let mut token_set = web_builder_agent::tokens::TokenSet::default();
        web_builder_agent::visual_edit::restore_visual_edits(&mut token_set, &edit_state);
        let theme = web_builder_agent::theme::extract_theme(&token_set);
        serde_json::to_string(&theme).map_err(|e| format!("serialize: {e}"))
    }

    /// Extract a theme from a URL (HTTPS only).
    #[tauri::command]
    async fn builder_theme_extract_from_url(url: String) -> Result<String, String> {
        let theme = web_builder_agent::theme_extract::extract_theme_from_url(&url)
            .await
            .map_err(|e| format!("{e}"))?;
        serde_json::to_string(&theme).map_err(|e| format!("serialize: {e}"))
    }

    /// Export the current theme in a specified format.
    #[tauri::command]
    fn builder_theme_export(project_id: String, format: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Get current theme
        let edit_state = web_builder_agent::visual_edit::load_visual_edit_state(&project_dir)
            .unwrap_or_default();
        let mut token_set = web_builder_agent::tokens::TokenSet::default();
        web_builder_agent::visual_edit::restore_visual_edits(&mut token_set, &edit_state);
        let theme = web_builder_agent::theme::extract_theme(&token_set);

        match format.as_str() {
            "css" => Ok(theme.to_css_variables()),
            "tailwind" => Ok(theme.to_tailwind_config()),
            "design_md" => Ok(theme.to_design_md()),
            "dtcg" => Ok(theme.to_dtcg_json()),
            _ => Err(format!("unknown export format: {format}")),
        }
    }

    /// Import a theme from content in a specified format.
    #[tauri::command]
    fn builder_theme_import(content: String, format: String) -> Result<String, String> {
        let theme = match format.as_str() {
            "design_md" => web_builder_agent::theme::Theme::from_design_md(&content)
                .map_err(|e| format!("{e}"))?,
            "dtcg" | "json" => web_builder_agent::theme::Theme::from_dtcg_json(&content)
                .map_err(|e| format!("{e}"))?,
            _ => return Err(format!("unknown import format: {format}")),
        };
        serde_json::to_string(&theme).map_err(|e| format!("serialize: {e}"))
    }

    /// List available preset themes.
    #[tauri::command]
    fn builder_theme_list_presets() -> Result<String, String> {
        let presets = web_builder_agent::theme_presets::get_preset_info_list();
        serde_json::to_string(&presets).map_err(|e| format!("serialize: {e}"))
    }

    /// Get a specific preset theme by name.
    #[tauri::command]
    fn builder_theme_get_preset(name: String) -> Result<String, String> {
        let presets = web_builder_agent::theme_presets::get_preset_themes();
        let theme = presets
            .into_iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("preset not found: {name}"))?;
        serde_json::to_string(&theme).map_err(|e| format!("serialize: {e}"))
    }

    // ── Phase 13: Image Generation Commands ────────────────────────────

    /// Check which image generation tiers are available.
    #[tauri::command]
    async fn builder_image_gen_status() -> Result<String, String> {
        let config = web_builder_agent::image_gen::ImageGenConfig::default();
        let status = web_builder_agent::image_gen::check_status(&config).await;
        serde_json::to_string(&status).map_err(|e| format!("serialize: {e}"))
    }

    /// Generate a single image for a specific slot.
    #[tauri::command]
    async fn builder_generate_image(
        project_id: String,
        slot_name: String,
        section_id: String,
        prompt: Option<String>,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let image_type = web_builder_agent::image_gen::infer_image_type(&slot_name);
        let aspect_ratio = web_builder_agent::image_gen::AspectRatio::from_image_type(image_type);

        let request = web_builder_agent::image_gen::ImageRequest {
            prompt: prompt.unwrap_or_else(|| format!("Image for {slot_name}")),
            slot_name: slot_name.clone(),
            section_id,
            image_type,
            aspect_ratio,
        };

        let config = web_builder_agent::image_gen::ImageGenConfig::default();
        let theme = web_builder_agent::image_gen::ThemeColors::default();

        let result =
            web_builder_agent::image_gen::generate_image(&request, &project_dir, &config, &theme)
                .await;

        eprintln!(
            "[builder-image] Generated image for slot '{}' via {} (${:.4})",
            slot_name, result.generation_method, result.cost
        );

        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Generate images for all ImagePrompt slots in a project.
    #[tauri::command]
    async fn builder_generate_all_images(
        project_id: String,
        _app: tauri::AppHandle,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        // Load the project state to get the template ID
        let state = web_builder_agent::project::load_project_state(&project_dir)
            .map_err(|e| format!("failed to load project state: {e}"))?;
        let template_id = state.selected_template.as_deref().unwrap_or("saas_landing");
        let schema = web_builder_agent::slot_schema::get_template_schema(template_id)
            .ok_or_else(|| format!("unknown template: {template_id}"))?;

        // Build a minimal content payload from latest checkpoint
        let checkpoint_dir = project_dir.join("current");
        let html_path = checkpoint_dir.join("index.html");
        if !html_path.exists() {
            return Err("no build output found — run a build first".into());
        }

        let config = web_builder_agent::image_gen::ImageGenConfig::default();
        let theme = web_builder_agent::image_gen::ThemeColors::default();

        // Collect all ImagePrompt slots from the schema and generate placeholders
        let mut results = Vec::new();
        let mut slot_count = 0usize;
        let mut total_cost = 0.0f64;

        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.slot_type == web_builder_agent::slot_schema::SlotType::ImagePrompt {
                    slot_count += 1;
                    let image_type = web_builder_agent::image_gen::infer_image_type(slot_name);
                    let request = web_builder_agent::image_gen::ImageRequest {
                        prompt: format!("Image for {} in {}", slot_name, section.section_id),
                        slot_name: slot_name.to_string(),
                        section_id: section.section_id.clone(),
                        image_type,
                        aspect_ratio: web_builder_agent::image_gen::AspectRatio::from_image_type(
                            image_type,
                        ),
                    };

                    let result = web_builder_agent::image_gen::generate_image(
                        &request,
                        &project_dir,
                        &config,
                        &theme,
                    )
                    .await;

                    total_cost += result.cost;
                    results.push(result);
                }
            }
        }

        eprintln!(
            "[builder-image] Generated {} images for project '{}' (total cost: ${:.4})",
            slot_count, project_id, total_cost
        );

        serde_json::to_string(&results).map_err(|e| format!("serialize: {e}"))
    }

    // ── Phase 15: Enterprise Trust Pack Commands ───────────────────────

    /// Generate a complete Trust Pack for a project.
    #[tauri::command]
    fn builder_generate_trust_pack(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let output_dir = project_dir.clone();
        let result = web_builder_agent::trust_pack::generate_trust_pack(&project_dir, &output_dir)
            .map_err(|e| format!("trust pack failed: {e}"))?;

        eprintln!(
            "[builder-trust] Generated trust pack for '{}': {} files, signed={}",
            project_id, result.total_files, result.signed
        );

        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Get the audit trail for a project.
    #[tauri::command]
    fn builder_get_audit_trail(
        project_id: String,
        filter: Option<String>,
        search: Option<String>,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let state = web_builder_agent::project::load_project_state(&project_dir)
            .unwrap_or_else(|_| web_builder_agent::project::create_project(&project_id, ""));

        let mut events =
            web_builder_agent::trust_pack::audit_trail::collect_audit_trail(&project_dir, &state);

        // Apply filter
        if let Some(ref filter_type) = filter {
            let event_type: Option<web_builder_agent::trust_pack::audit_trail::AuditEventType> =
                serde_json::from_str(&format!("\"{filter_type}\"")).ok();
            if let Some(et) = event_type {
                events = web_builder_agent::trust_pack::audit_trail::filter_by_type(&events, &et);
            }
        }

        // Apply search
        if let Some(ref query) = search {
            events = web_builder_agent::trust_pack::audit_trail::search_events(&events, query);
        }

        serde_json::to_string(&events).map_err(|e| format!("serialize: {e}"))
    }

    /// Export audit trail as CSV or JSON.
    #[tauri::command]
    fn builder_export_audit_trail(project_id: String, format: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let state = web_builder_agent::project::load_project_state(&project_dir)
            .unwrap_or_else(|_| web_builder_agent::project::create_project(&project_id, ""));

        let events =
            web_builder_agent::trust_pack::audit_trail::collect_audit_trail(&project_dir, &state);

        match format.as_str() {
            "csv" => Ok(web_builder_agent::trust_pack::audit_trail::export_csv(
                &events,
            )),
            "json" => Ok(web_builder_agent::trust_pack::audit_trail::export_json(
                &events,
            )),
            _ => Err(format!("unknown format: {format}")),
        }
    }

    /// Verify a build manifest's Ed25519 signature.
    #[tauri::command]
    fn builder_verify_manifest(manifest_json: String) -> Result<bool, String> {
        let manifest: web_builder_agent::trust_pack::build_manifest::BuildManifest =
            serde_json::from_str(&manifest_json).map_err(|e| format!("parse manifest: {e}"))?;

        web_builder_agent::trust_pack::build_manifest::verify_manifest(&manifest)
            .map_err(|e| format!("verify: {e}"))
    }

    // ── Phase 7B: Deploy History Commands ────────────────────────────────

    /// Get the full deploy history for a project.
    #[tauri::command]
    fn builder_deploy_history(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);
        let history = web_builder_agent::deploy::history::load_history(&project_dir);
        serde_json::to_string(&history.all_newest_first()).map_err(|e| format!("serialize: {e}"))
    }

    /// Compute diff between two deploys.
    #[tauri::command]
    fn builder_deploy_diff(
        project_id: String,
        from_id: String,
        to_id: String,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);
        let history = web_builder_agent::deploy::history::load_history(&project_dir);
        let diff = history
            .diff(&from_id, &to_id)
            .ok_or_else(|| "deploy entries not found".to_string())?;
        serde_json::to_string(&diff).map_err(|e| format!("serialize: {e}"))
    }

    /// Rollback to any previous deploy by history entry ID.
    #[tauri::command]
    async fn builder_deploy_rollback_to(
        project_id: String,
        entry_id: String,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);

        let mut history = web_builder_agent::deploy::history::load_history(&project_dir);

        let target = history
            .get(&entry_id)
            .ok_or_else(|| format!("entry not found: {entry_id}"))?
            .clone();

        // Load credentials
        let creds = web_builder_agent::deploy::credentials::load_credentials(&target.provider)
            .map_err(|e| format!("credentials: {e}"))?
            .ok_or_else(|| format!("No credentials for {}", target.provider))?;

        let client = reqwest::Client::new();
        let gov = web_builder_agent::deploy::DeployGovernance {
            agent_id: SYSTEM_UUID,
            capabilities: vec!["deploy.execute".into()],
            fuel_budget_usd: 10.0,
        };

        // Rollback via provider
        let result = match target.provider.as_str() {
            "netlify" => web_builder_agent::deploy::netlify::rollback(
                &target.site_id,
                &target.deploy_id,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            "cloudflare" => web_builder_agent::deploy::cloudflare::rollback(
                &target.site_id,
                &target.deploy_id,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            "vercel" => web_builder_agent::deploy::vercel::rollback(
                &target.site_id,
                &target.deploy_id,
                &creds,
                &client,
                &gov,
            )
            .await
            .map_err(|e| format!("rollback: {e}"))?,
            other => return Err(format!("Unknown provider: {other}")),
        };

        // Update history
        let current_id = history.current().map(|e| e.id.clone());
        if let Some(from) = current_id {
            history.record_rollback(&from, &entry_id);
        }
        let _ = web_builder_agent::deploy::history::save_history(&project_dir, &history);

        serde_json::to_string(&serde_json::json!({
            "deploy_id": result.deploy_id,
            "url": result.url,
            "provider": result.provider,
        }))
        .map_err(|e| format!("serialize: {e}"))
    }

    /// Generate a QR code SVG for a URL.
    #[tauri::command]
    fn builder_deploy_qr_code(url: String) -> Result<String, String> {
        web_builder_agent::deploy::qr::generate_qr_svg(&url, 200).map_err(|e| format!("{e}"))
    }

    /// Get share info for the current live deploy.
    #[tauri::command]
    fn builder_deploy_share_info(project_id: String) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);
        let history = web_builder_agent::deploy::history::load_history(&project_dir);
        let live = history
            .current()
            .ok_or_else(|| "No live deploy found".to_string())?;

        let qr_svg =
            web_builder_agent::deploy::qr::generate_qr_svg(&live.url, 200).unwrap_or_default();

        let info = serde_json::json!({
            "url": live.url,
            "qr_svg": qr_svg,
            "provider": live.provider,
            "deployed_at": live.timestamp,
            "build_hash": live.build_hash,
            "is_current": true,
        });
        serde_json::to_string(&info).map_err(|e| format!("serialize: {e}"))
    }

    /// Check deploy drift (current build vs live deploy).
    #[tauri::command]
    fn builder_deploy_drift(
        project_id: String,
        current_build_hash: String,
    ) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let project_dir = std::path::PathBuf::from(home)
            .join(".nexus")
            .join("builds")
            .join(&project_id);
        let history = web_builder_agent::deploy::history::load_history(&project_dir);
        let drift =
            web_builder_agent::deploy::history::check_deploy_drift(&current_build_hash, &history);
        serde_json::to_string(&drift).map_err(|e| format!("serialize: {e}"))
    }

    // ── Phase 16: Self-Improving Builder Commands ──────���─────────────────

    /// Get the current improvement status.
    #[tauri::command]
    fn builder_improvement_status() -> Result<String, String> {
        let store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;
        let status = web_builder_agent::self_improve::compute_status(&store);
        serde_json::to_string(&status).map_err(|e| format!("serialize: {e}"))
    }

    /// Run analysis on all completed projects.
    #[tauri::command]
    fn builder_improvement_run_analysis() -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let builds_dir = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("builds");

        let mut store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;

        // Collect metrics from all completed projects
        let metrics = web_builder_agent::self_improve::observer::collect_all_metrics(&builds_dir);
        store.metrics = metrics;

        // Aggregate and analyze
        let agg = web_builder_agent::self_improve::metrics::aggregate_metrics(&store.metrics);
        let analysis = web_builder_agent::self_improve::analyzer::analyze(
            &agg,
            web_builder_agent::self_improve::analyzer::DEFAULT_MIN_SAMPLE_SIZE,
        );

        // Generate proposals for new opportunities
        let proposals = web_builder_agent::self_improve::proposer::generate_proposals(
            &analysis,
            &store.defaults,
        );
        for p in proposals {
            if !store
                .proposals
                .iter()
                .any(|existing| existing.opportunity_id == p.opportunity_id)
            {
                store.proposals.push(p);
            }
        }

        web_builder_agent::self_improve::store::save_store(&store)
            .map_err(|e| format!("save store: {e}"))?;

        serde_json::to_string(&analysis).map_err(|e| format!("serialize: {e}"))
    }

    /// Get all proposals.
    #[tauri::command]
    fn builder_improvement_get_proposals() -> Result<String, String> {
        let store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;
        serde_json::to_string(&store.proposals).map_err(|e| format!("serialize: {e}"))
    }

    /// Validate a proposal by ID.
    #[tauri::command]
    fn builder_improvement_validate_proposal(proposal_id: String) -> Result<String, String> {
        let mut store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;

        let proposal = store
            .proposals
            .iter()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| format!("proposal not found: {proposal_id}"))?
            .clone();

        let avg_quality = 85u32; // baseline estimate
        let avg_conversion = 75u32;
        let result = web_builder_agent::self_improve::validator::validate_proposal(
            &proposal,
            avg_quality,
            avg_conversion,
        )
        .map_err(|e| format!("validate: {e}"))?;

        // Update proposal status
        if let Some(p) = store.proposals.iter_mut().find(|p| p.id == proposal_id) {
            p.status = if result.passed {
                web_builder_agent::self_improve::proposer::ProposalStatus::Validated
            } else {
                web_builder_agent::self_improve::proposer::ProposalStatus::ValidationFailed
            };
        }
        web_builder_agent::self_improve::store::save_store(&store)
            .map_err(|e| format!("save store: {e}"))?;

        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Apply a validated proposal.
    #[tauri::command]
    fn builder_improvement_apply_proposal(proposal_id: String) -> Result<String, String> {
        let mut store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;

        let proposal = store
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| format!("proposal not found: {proposal_id}"))?;

        let result =
            web_builder_agent::self_improve::applier::apply_proposal(proposal, &mut store.defaults)
                .map_err(|e| format!("apply: {e}"))?;

        // Save rollback snapshot
        store
            .rollback_snapshots
            .insert(proposal_id.clone(), result.previous_state.clone());
        store.version += 1;

        web_builder_agent::self_improve::store::save_store(&store)
            .map_err(|e| format!("save store: {e}"))?;

        serde_json::to_string(&result).map_err(|e| format!("serialize: {e}"))
    }

    /// Roll back a previously applied proposal.
    #[tauri::command]
    fn builder_improvement_rollback_proposal(proposal_id: String) -> Result<String, String> {
        let mut store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;

        let snapshot = store
            .rollback_snapshots
            .get(&proposal_id)
            .ok_or_else(|| format!("no rollback snapshot for: {proposal_id}"))?
            .clone();

        let proposal = store
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| format!("proposal not found: {proposal_id}"))?;

        web_builder_agent::self_improve::applier::rollback_proposal(
            proposal,
            &mut store.defaults,
            &snapshot,
        )
        .map_err(|e| format!("rollback: {e}"))?;

        store.version += 1;
        web_builder_agent::self_improve::store::save_store(&store)
            .map_err(|e| format!("save store: {e}"))?;

        Ok("rolled back".into())
    }

    /// Reset all improvements to factory defaults.
    #[tauri::command]
    fn builder_improvement_reset_defaults() -> Result<String, String> {
        let mut store = web_builder_agent::self_improve::store::load_store()
            .map_err(|e| format!("load store: {e}"))?;

        web_builder_agent::self_improve::applier::reset_defaults(&mut store.defaults);
        // Mark all applied proposals as rolled back
        for p in &mut store.proposals {
            if p.status == web_builder_agent::self_improve::proposer::ProposalStatus::Applied {
                p.status = web_builder_agent::self_improve::proposer::ProposalStatus::RolledBack;
            }
        }
        store.version += 1;
        store.rollback_snapshots.clear();

        web_builder_agent::self_improve::store::save_store(&store)
            .map_err(|e| format!("save store: {e}"))?;

        Ok("reset to factory defaults".into())
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

    /// TEMPORARY DIAGNOSTIC: emit a synthetic agent-cognitive-cycle event from
    /// inside a Tauri command, using the window injected by Tauri itself.
    /// Bypasses BackendEventBridge to test if direct window.emit reaches the
    /// frontend listener.
    #[tauri::command]
    async fn test_emit_event(window: tauri::WebviewWindow) -> Result<(), String> {
        eprintln!(
            "[TEST] test_emit_event called, window label: {}",
            window.label()
        );
        window
            .emit(
                "agent-cognitive-cycle",
                serde_json::json!({
                    "agent_id": "test-agent-id",
                    "phase": "test",
                    "steps_executed": 1,
                    "fuel_consumed": 0.0,
                    "should_continue": true,
                    "blocked_reason": null,
                    "steps": []
                }),
            )
            .map_err(|e| {
                eprintln!("[TEST] emit failed: {}", e);
                e.to_string()
            })?;
        eprintln!("[TEST] emit succeeded from test command");
        Ok(())
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

    #[tauri::command]
    fn self_improve_get_envelope(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_envelope(state.inner(), agent_id)
    }

    #[tauri::command]
    fn self_improve_get_guardian_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_guardian_status(state.inner())
    }

    #[tauri::command]
    fn self_improve_force_baseline(
        state: tauri::State<'_, AppState>,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_force_baseline(state.inner())
    }

    #[tauri::command]
    fn self_improve_promote_baseline(state: tauri::State<'_, AppState>) -> Result<(), String> {
        commands::self_improvement::self_improve_promote_baseline(state.inner())
    }

    #[tauri::command]
    fn self_improve_get_report(
        state: tauri::State<'_, AppState>,
        days: u32,
    ) -> Result<serde_json::Value, String> {
        commands::self_improvement::self_improve_get_report(state.inner(), days)
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

    #[tauri::command]
    fn log_frontend_error(message: String, stack: String, component_stack: String) {
        eprintln!("[FRONTEND ERROR] {message}");
        if !stack.is_empty() {
            eprintln!("[FRONTEND STACK] {stack}");
        }
        if !component_stack.is_empty() {
            eprintln!("[COMPONENT STACK] {component_stack}");
        }
        // Also append to a log file for post-mortem debugging
        if let Some(home) = dirs::home_dir() {
            let log_dir = home.join(".nexus");
            let _ = std::fs::create_dir_all(&log_dir);
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("frontend_errors.log"))
            {
                use std::io::Write;
                let _ = writeln!(
                    file,
                    "[{}] {}\n{}\n{}\n---",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    message,
                    stack,
                    component_stack
                );
            }
        }
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
                            SYSTEM_UUID,
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
            .manage(AppState::new())
            .manage(nx_bridge::init_nx_state().expect("Failed to initialize Nexus Code bridge"));

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

            // Auto-detect enabled LLM providers (non-blocking).
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match commands::chat_llm::auto_detect_enabled_providers() {
                        Ok(providers) => {
                            let _ = app_handle.emit("llm-providers-ready", &providers);
                            eprintln!("[startup] LLM provider auto-detect complete");
                        }
                        Err(e) => {
                            eprintln!("[startup] LLM provider auto-detect failed: {e}");
                        }
                    }
                });
            }

            // Defer heavy agent loading so the window appears immediately.
            let state_clone = state.inner().clone();
            let _runner_clone = state.schedule_runner.clone();
            tauri::async_runtime::spawn(async move {
                state_clone.load_agents_deferred();

                // DISABLED: Auto-starting agent schedules from persisted state.
                // This was registering cron jobs for all previously-running agents,
                // causing cognitive loops to fire without user action.
                // state_clone.initialize_startup_schedules();

                // DISABLED: Background schedule runner + agent auto-seeding.
                // The runner continuously spawned cognitive loops for persisted
                // agents (via seed_manifests_to_runner → execute_agent_goal →
                // spawn_cognitive_loop), consuming CPU/RAM without user consent.
                // Agents now only run when explicitly started by the user.
                //
                // To re-enable, uncomment the following three lines:
                // seed_manifests_to_runner(&state_clone);
                // eprintln!("[startup] launching background schedule runner");
                // runner_clone.run().await;
                eprintln!(
                    "[startup] background schedule runner DISABLED — agents run on-demand only"
                );
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
                detect_claude_code_cli,
                trigger_claude_code_login,
                detect_codex_cli,
                trigger_codex_cli_login,
                load_llm_provider_settings,
                save_llm_provider_settings,
                detect_cli_provider,
                auto_detect_all_enabled,
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
                conduct_build_streaming,
                read_build_file,
                builder_get_budget,
                builder_set_budget,
                builder_set_remaining,
                builder_list_projects,
                builder_load_project,
                builder_delete_project,
                builder_get_history,
                builder_record_build,
                builder_get_available_models,
                builder_get_model_config,
                builder_save_model_config,
                builder_reset_model_config,
                builder_get_model_choices,
                builder_check_cli_auth,
                builder_authenticate_cli,
                builder_read_preview,
                builder_list_checkpoints,
                builder_rollback,
                builder_init_checkpoint,
                builder_iterate,
                builder_generate_plan,
                builder_load_plan,
                builder_archive_project,
                builder_unarchive_project,
                builder_export_project,
                builder_save_state,
                builder_load_state,
                builder_visual_edit_token,
                builder_visual_edit_text,
                builder_scaffold_build,
                builder_dev_server_start,
                builder_dev_server_stop,
                builder_dev_server_status,
                builder_dev_server_write_file,
                builder_deploy,
                builder_deploy_rollback,
                builder_deploy_store_credentials,
                builder_deploy_check_credentials,
                builder_deploy_list_sites,
                builder_build_static,
                builder_quality_check,
                builder_quality_auto_fix,
                builder_quality_auto_fix_all,
                builder_conversion_check,
                builder_conversion_auto_fix,
                builder_collab_start_hosting,
                builder_collab_join,
                builder_collab_leave,
                builder_collab_invite,
                builder_collab_set_role,
                builder_collab_add_comment,
                builder_collab_get_comments,
                builder_collab_resolve_comment,
                builder_backend_connect,
                builder_backend_generate,
                builder_backend_apply,
                builder_backend_preview_schema,
                builder_backend_list_providers,
                builder_backend_generate_v2,
                builder_import_design,
                builder_import_remap_sections,
                builder_generate_variants,
                builder_generate_section_variants,
                builder_select_variant,
                builder_theme_apply,
                builder_theme_get_current,
                builder_theme_extract_from_url,
                builder_theme_export,
                builder_theme_import,
                builder_theme_list_presets,
                builder_theme_get_preset,
                builder_image_gen_status,
                builder_generate_image,
                builder_generate_all_images,
                builder_generate_trust_pack,
                builder_get_audit_trail,
                builder_export_audit_trail,
                builder_verify_manifest,
                builder_deploy_history,
                builder_deploy_diff,
                builder_deploy_rollback_to,
                builder_deploy_qr_code,
                builder_deploy_share_info,
                builder_deploy_drift,
                builder_improvement_status,
                builder_improvement_run_analysis,
                builder_improvement_get_proposals,
                builder_improvement_validate_proposal,
                builder_improvement_apply_proposal,
                builder_improvement_rollback_proposal,
                builder_improvement_reset_defaults,
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
                test_emit_event,
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
                self_improve_get_envelope,
                self_improve_get_guardian_status,
                self_improve_force_baseline,
                self_improve_promote_baseline,
                self_improve_get_report,
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
                // Nexus Code (nx) Bridge
                nx_bridge::commands::nx_status,
                nx_bridge::commands::nx_chat,
                nx_bridge::commands::nx_chat_cancel,
                nx_bridge::commands::nx_consent_respond,
                nx_bridge::commands::nx_tool,
                nx_bridge::commands::nx_doctor,
                nx_bridge::commands::nx_providers,
                nx_bridge::commands::nx_tools,
                nx_bridge::commands::nx_session_save,
                nx_bridge::commands::nx_session_list,
                nx_bridge::commands::nx_switch_provider,
                // Computer Use commands
                nx_bridge::commands::nx_computer_use_screenshot,
                nx_bridge::commands::nx_computer_use_status,
                nx_bridge::commands::nx_agent_run,
                nx_bridge::commands::nx_agent_approve,
                nx_bridge::commands::nx_app_grants,
                nx_bridge::commands::nx_learned_patterns,
                nx_bridge::commands::nx_learning_stats,
                log_frontend_error,
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
