//! chat_llm domain implementation.

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
    groq::GROQ_MODELS, nvidia::NVIDIA_MODELS, openrouter::OPENROUTER_MODELS, ClaudeProvider,
    DeepSeekProvider, GeminiProvider, GroqProvider, LlmProvider, NvidiaProvider, OllamaProvider,
    OpenAiProvider, OpenRouterProvider,
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

// ── Chat Pipeline: Complexity Detection + Auto-Routing ───────────────────

/// Detect message complexity using a fast heuristic (no LLM call).
/// Falls back to keyword analysis for speed — the LLM call is reserved for
/// the actual response, not the classification.
pub(crate) fn detect_complexity(message: &str) -> ComplexityLevel {
    let lower = message.to_lowercase();

    // Project indicators: user wants a complete product built
    let project_keywords = [
        "build me",
        "build a",
        "create a full",
        "create an app",
        "create a saas",
        "create a platform",
        "build an app",
        "build a saas",
        "build a platform",
        "full stack",
        "fullstack",
        "complete app",
        "complete system",
        "entire app",
        "from scratch",
        "production ready",
        "mvp of",
        "mvp for",
        "startup",
        "build a website",
        "build a dashboard",
        "build me a",
        "make me a",
        "develop a",
        "create a complete",
        "design and build",
        "end to end",
        "with authentication",
        "with stripe",
        "with payments",
        "with database",
        "multi-page",
        "landing page with",
    ];

    // Strong project signals: message is long AND contains project keywords
    let has_project_keyword = project_keywords.iter().any(|kw| lower.contains(kw));
    let is_long_request = message.len() > 100;

    if has_project_keyword && (is_long_request || lower.contains("app") || lower.contains("saas")) {
        return ComplexityLevel::ComplexProject;
    }

    // Also detect: multi-feature requests (lists with "and", commas, numbers)
    if has_project_keyword {
        let feature_count_signals = [", ", " and ", "1.", "2.", "- "];
        let multi_feature = feature_count_signals
            .iter()
            .filter(|s| lower.contains(**s))
            .count();
        if multi_feature >= 2 {
            return ComplexityLevel::ComplexProject;
        }
    }

    // Question indicators
    let question_starters = [
        "what is",
        "what are",
        "how do",
        "how does",
        "why ",
        "explain",
        "tell me about",
        "can you explain",
        "what's the",
        "is it",
        "are there",
        "define ",
        "describe ",
    ];
    let is_question =
        lower.ends_with('?') || question_starters.iter().any(|qs| lower.starts_with(qs));

    if is_question {
        return ComplexityLevel::SimpleQuestion;
    }

    // Task indicators: user wants something done
    let task_keywords = [
        "write a",
        "write me",
        "fix ",
        "debug ",
        "refactor ",
        "review ",
        "analyze ",
        "convert ",
        "translate ",
        "optimize ",
        "implement ",
        "add a",
        "remove ",
        "update ",
        "modify ",
        "change ",
        "generate ",
        "create a function",
        "create a class",
        "create a test",
        "write code",
        "code for",
    ];

    if task_keywords.iter().any(|kw| lower.contains(kw)) {
        return ComplexityLevel::SmallTask;
    }

    // Default: treat as question (safest — goes to normal LLM)
    ComplexityLevel::SimpleQuestion
}

/// Auto-select the best agent for a request using the heuristic categorizer
/// and the routing learner's historical data.
pub(crate) fn auto_select_agent(
    message: &str,
    routing_learner: &nexus_kernel::self_improve::RoutingLearner,
) -> (String, String) {
    let category = nexus_kernel::self_improve::RoutingLearner::categorize_heuristic(message);

    // Check if the routing learner has learned a better agent for this category
    if let Some(learned_agent) = routing_learner.recommend_agent(&category) {
        return (learned_agent, category);
    }

    // Fallback: static mapping of categories to default agents
    let agent_id = match category.as_str() {
        "code" => "nexus-forge",
        "security" => "nexus-aegis",
        "research" => "nexus-scholar",
        "design" => "nexus-architect",
        "devops" => "nexus-devops",
        "data" => "nexus-datasmith",
        "writing" => "nexus-herald",
        "planning" => "nexus-architect",
        "testing" => "nexus-sentinel",
        _ => "nexus-nexus",
    };
    (agent_id.to_string(), category)
}

/// Load the system prompt for a given agent from its genome file on disk.
pub(crate) fn load_agent_system_prompt(agent_name: &str) -> Option<String> {
    let genome_path = format!("agents/genomes/{agent_name}.json");
    // Optional: genome file may not exist for dynamically created agents
    let genome_json = std::fs::read_to_string(&genome_path).ok()?;
    // Optional: genome JSON may be malformed or from an older schema version
    let genome: nexus_kernel::genome::AgentGenome = serde_json::from_str(&genome_json).ok()?;
    let prompt = genome.genes.personality.system_prompt.clone();
    if prompt.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

/// Return the default chat/completion model from config (or `"mock-1"`).
pub(crate) fn get_default_model() -> String {
    load_config()
        .map(|c| {
            let m = c.llm.default_model.trim().to_string();
            if m.is_empty() {
                "mock-1".to_string()
            } else {
                m
            }
        })
        .unwrap_or_else(|_| "mock-1".to_string())
}

pub(crate) fn send_chat(
    state: &AppState,
    message: String,
    model_id: Option<String>,
    agent_name: Option<String>,
) -> Result<ChatResponse, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::LlmRequest)?;
    state.validate_input(&message)?;
    // ── Pipeline Step 0: Check if user is approving a pending project ──
    {
        let mut conv_state = state
            .chat_conversation_state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if conv_state.awaiting_approval {
            let lower = message.to_lowercase();
            let is_approval = lower.contains("yes")
                || lower.contains("build")
                || lower.contains("go ahead")
                || lower.contains("start")
                || lower.contains("do it")
                || lower.contains("approve")
                || lower == "y";
            if is_approval {
                conv_state.awaiting_approval = false;
                let plan = conv_state.last_project_plan.take().unwrap_or_default();
                conv_state.active_project = Some(plan.clone());

                // Return an autopilot-activated acknowledgment.
                // The frontend will show progress events as they stream in.
                let ack = format!(
                    "\u{1f680} **Autopilot activated!** Building your project now...\n\n\
                     I'll update you as I progress. You can keep chatting \u{2014} I'm multitasking.\n\n\
                     ---\n\n\
                     **Plan summary:**\n{plan}"
                );
                return Ok(ChatResponse {
                    text: ack,
                    model: "autopilot".to_string(),
                    token_count: 0,
                    cost: 0.0,
                    latency_ms: 0,
                });
            }
            // Not an approval — user is asking something else, clear the pending state
            conv_state.awaiting_approval = false;
        }
    }

    // ── Pipeline Step 1: Complexity Detection ──
    let complexity = detect_complexity(&message);

    // ── Pipeline Step 2: Auto-routing (determine effective agent) ──
    let (effective_agent_name, routing_prefix) = if agent_name.is_some() {
        // User explicitly selected an agent — use it directly
        (agent_name.clone(), String::new())
    } else {
        match complexity {
            ComplexityLevel::ComplexProject => {
                // For complex projects, we generate a plan instead of routing to an agent
                (Some("nexus-architect".to_string()), String::new())
            }
            ComplexityLevel::SmallTask | ComplexityLevel::SimpleQuestion => {
                let learner = state
                    .routing_learner
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                let (routed_agent, category) = auto_select_agent(&message, &learner);
                let prefix =
                    format!("\u{1f916} *Routing to **{routed_agent}** ({category})...*\n\n");
                eprintln!("auto-route: {category} → {routed_agent}");
                (Some(routed_agent), prefix)
            }
        }
    };

    // ── Pipeline Step 3: ComplexProject → generate plan, set awaiting_approval ──
    if complexity == ComplexityLevel::ComplexProject && agent_name.is_none() {
        // Build a project-planning prompt and send it through the normal LLM path.
        // The response becomes the plan; we set awaiting_approval so the next "yes"
        // triggers autopilot.
        let plan_prompt = format!(
            "You are a project planner for an AI operating system called Nexus OS.\n\
             The user wants: \"{message}\"\n\n\
             Create a project plan. Present it in PLAIN ENGLISH:\n\
             1. **Project name**\n\
             2. **What will be built** (features list)\n\
             3. **Estimated build time** (in minutes)\n\
             4. **What agents will work on it** (pick from: nexus-forge for code, \
                nexus-architect for design, nexus-aegis for security, nexus-scholar for research, \
                nexus-sentinel for testing, nexus-devops for deployment)\n\
             5. Ask 1-2 simple clarifying questions if anything is ambiguous\n\n\
             Format the plan nicely with markdown. End with:\n\
             '**Ready to build? Type YES to start, or tell me what you\\'d like to change.**'"
        );

        let config = load_config().map_err(agent_error)?;
        let provider_config = build_provider_config(&config);
        let (provider, model_name) = if let Some(ref full_model) = model_id {
            provider_from_prefixed_model(full_model, &provider_config)?
        } else {
            let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
            let m = if config.llm.default_model.trim().is_empty() {
                "mock-1".to_string()
            } else {
                config.llm.default_model.clone()
            };
            (provider, m)
        };

        let mut gateway = GovernedLlmGateway::new(provider);
        gateway.set_skip_output_firewall(true);
        let mut capabilities = HashSet::new();
        capabilities.insert("llm.query".to_string());
        let plan_agent_id = Uuid::new_v4();
        let mut context = AgentRuntimeContext {
            agent_id: plan_agent_id,
            capabilities,
            fuel_remaining: 50_000,
        };

        let response = gateway
            .query(&mut context, &plan_prompt, 4096, &model_name)
            .map_err(agent_error)?;

        let plan_text = response.output_text.clone();

        // Set conversation state to awaiting approval
        {
            let mut conv_state = state
                .chat_conversation_state
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            conv_state.last_project_plan = Some(plan_text.clone());
            conv_state.awaiting_approval = true;
        }

        let oracle = gateway.oracle_events().last();
        let header =
            "\u{1f680} **This looks like a project!** Let me put together a plan for you.\n\n---\n\n";
        return Ok(ChatResponse {
            text: format!("{header}{plan_text}"),
            model: response.model_name,
            token_count: response.token_count,
            cost: oracle.map(|value| value.cost).unwrap_or(0.0),
            latency_ms: oracle.map(|value| value.latency_ms).unwrap_or(0),
        });
    }

    // ── Pipeline Step 4: Normal chat flow (with agent system prompt if routed) ──
    let config = load_config().map_err(agent_error)?;
    let provider_config = build_provider_config(&config);

    // Determine provider and model from prefixed string (e.g. "anthropic/claude-sonnet-4-20250514")
    let (provider, model_name) = if let Some(ref full_model) = model_id {
        provider_from_prefixed_model(full_model, &provider_config)?
    } else {
        let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
        let m = if config.llm.default_model.trim().is_empty() {
            "mock-1".to_string()
        } else {
            config.llm.default_model.clone()
        };
        (provider, m)
    };

    let mut gateway = GovernedLlmGateway::new(provider);
    // User-facing chat: the human asked the question and is reading the
    // response on their own screen — skip the output exfiltration filter.
    gateway.set_skip_output_firewall(true);

    let mut capabilities = HashSet::new();
    capabilities.insert("llm.query".to_string());
    let agent_id = Uuid::new_v4();
    let agent_id_str = agent_id.to_string();
    let mut context = AgentRuntimeContext {
        agent_id,
        capabilities,
        fuel_remaining: 50_000,
    };

    // ── Consciousness: pre-flight modification ──
    let consciousness_mod = {
        let mut engine = state
            .consciousness
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        engine.on_agent_new_task(&agent_id_str, &message, 0.5);
        engine.get_llm_modification(&agent_id_str)
    };
    let max_tokens = (2048_f64 * consciousness_mod.max_tokens_multiplier) as u32;

    // Build the effective prompt: agent system prompt + consciousness hint + message
    let agent_system_prompt = effective_agent_name
        .as_deref()
        .and_then(load_agent_system_prompt);

    let effective_prompt = {
        let mut parts = Vec::new();
        if let Some(ref sys_prompt) = agent_system_prompt {
            parts.push(format!(
                "[Agent System Prompt]\n{sys_prompt}\n[End Agent System Prompt]"
            ));
        }
        if let Some(ref suffix) = consciousness_mod.system_prompt_suffix {
            parts.push(format!("[System hint: {suffix}]"));
        }
        parts.push(message.clone());
        parts.join("\n\n")
    };

    let response = gateway
        .query(
            &mut context,
            effective_prompt.as_str(),
            max_tokens,
            &model_name,
        )
        .map_err(|e| {
            // ── Consciousness: record failure ──
            let mut engine = state
                .consciousness
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            engine.on_agent_task_failure(&agent_id_str, &e.to_string());
            agent_error(e)
        })?;

    // ── Consciousness: record success + tokens ──
    {
        let mut engine = state
            .consciousness
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        engine.on_agent_task_success(&agent_id_str);
        engine.on_agent_tokens(&agent_id_str, response.token_count as u64);
    }

    let oracle = gateway.oracle_events().last();

    let payload = json!({
        "event": "send_chat",
        "model": response.model_name,
        "provider": model_id.as_deref().and_then(|m| m.split_once('/')).map(|(p, _)| p).unwrap_or("auto"),
        "token_count": response.token_count,
        "cost": oracle.map(|value| value.cost).unwrap_or(0.0),
        "latency_ms": oracle.map(|value| value.latency_ms).unwrap_or(0),
        "consciousness": consciousness_mod.reason,
        "complexity": format!("{complexity:?}"),
        "routed_agent": effective_agent_name.as_deref().unwrap_or("none"),
    });
    state.log_event(context.agent_id, EventType::LlmCall, payload);

    // ── Dream Forge: auto-queue dream tasks from this interaction ──
    {
        let consciousness_snapshot = {
            let mut c_engine = state
                .consciousness
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            c_engine.get_agent_state_snapshot(&agent_id_str)
        };
        let interaction = nexus_kernel::dreams::auto_queue::ChatInteraction {
            user_message: message.clone(),
            agent_response: response.output_text.clone(),
            was_error: false,
            error_message: None,
            topic_detected: None,
            token_count: response.token_count as u64,
        };
        let mut d_engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
        nexus_kernel::dreams::queue_dreams_from_interaction(
            &mut d_engine.scheduler,
            &agent_id_str,
            &consciousness_snapshot,
            &interaction,
        );
    }

    // ── Auto-Evolution: background score + evolve ──
    let evo_agent = effective_agent_name.or(agent_name);
    if let Some(ref evo_name) = evo_agent {
        let auto_evo = state.auto_evolution.clone();
        let routing_learner = state.routing_learner.clone();
        let evo_agent_name = evo_name.clone();
        let evo_user_msg = message.clone();
        let evo_agent_resp = response.output_text.clone();
        #[cfg(all(
            feature = "tauri-runtime",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        let evo_app_handle = state
            .app_handle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        std::thread::spawn(move || {
            let llm = GatewayPlannerLlm;
            let score_val =
                auto_evo.score_and_record(&evo_agent_name, &evo_user_msg, &evo_agent_resp, &llm);

            // Feed routing learner with the score
            {
                let category =
                    nexus_kernel::self_improve::RoutingLearner::categorize_heuristic(&evo_user_msg);
                let outcome = nexus_kernel::self_improve::RoutingOutcome {
                    request_summary: evo_user_msg.chars().take(100).collect::<String>(),
                    request_category: category,
                    agent_id: evo_agent_name.clone(),
                    score: score_val,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                if let Ok(mut learner) = routing_learner.lock() {
                    learner.record(outcome);
                }
            }

            if auto_evo.should_evolve(&evo_agent_name) {
                // Load genome from disk if available
                let genome_path = format!("agents/genomes/{evo_agent_name}.json");
                if let Ok(genome_json) = std::fs::read_to_string(&genome_path) {
                    if let Ok(genome) =
                        serde_json::from_str::<nexus_kernel::genome::AgentGenome>(&genome_json)
                    {
                        let result = auto_evo.attempt_evolution(&evo_agent_name, &genome, &llm);
                        if result.improved {
                            // Apply and persist the evolved genome
                            if let Some(evolved) = auto_evo.apply_evolution(&genome, &llm) {
                                if let Ok(json) = serde_json::to_string_pretty(&evolved) {
                                    // Best-effort: persist evolved genome to disk; failure is non-fatal
                                    let _ = std::fs::write(&genome_path, json);
                                }
                            }
                            // Emit event to frontend
                            #[cfg(all(
                                feature = "tauri-runtime",
                                any(
                                    target_os = "windows",
                                    target_os = "macos",
                                    target_os = "linux"
                                )
                            ))]
                            if let Some(ref app) = evo_app_handle {
                                // Best-effort: notify frontend of evolution; UI will refresh on next poll
                                let _ = app.emit(
                                    "agent-evolved",
                                    json!({
                                        "agent_id": evo_agent_name,
                                        "old_score": result.old_score,
                                        "new_score": result.new_score,
                                    }),
                                );
                            }
                            eprintln!(
                                "auto-evolution: {} improved {:.1} → {:.1}",
                                evo_agent_name, result.old_score, result.new_score
                            );
                        }
                    }
                }
            }
        });
    }

    // ── Consciousness: adapt response to user mood ──
    let user_adaptation = {
        let engine = state
            .consciousness
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        engine.get_user_behavior().adapt_response()
    };

    // If the empathic engine has a proactive message and confidence is high enough,
    // prepend it to the response.
    let final_text = {
        let mut text = String::new();
        // Add routing prefix (auto-selected agent notification)
        if !routing_prefix.is_empty() {
            text.push_str(&routing_prefix);
        }
        // Add consciousness adaptation hint
        if let Some(ref hint) = user_adaptation.message {
            text.push_str(hint);
            text.push_str("\n\n");
        }
        text.push_str(&response.output_text);
        text
    };

    Ok(ChatResponse {
        text: final_text,
        model: response.model_name,
        token_count: response.token_count,
        cost: oracle.map(|value| value.cost).unwrap_or(0.0),
        latency_ms: oracle.map(|value| value.latency_ms).unwrap_or(0),
    })
}

// ── Auto-Evolution Tauri API ──────────────────────────────────────────────

pub(crate) fn get_agent_performance(
    state: &AppState,
    agent_id: String,
) -> Result<nexus_kernel::genome::AgentPerformanceTracker, String> {
    state
        .auto_evolution
        .get_tracker(&agent_id)
        .ok_or_else(|| format!("No performance data for agent {agent_id}"))
}

pub(crate) fn get_auto_evolution_log(
    state: &AppState,
    agent_id: String,
    limit: u32,
) -> Result<Vec<nexus_kernel::genome::EvolutionEvent>, String> {
    Ok(state.auto_evolution.get_evolution_log(&agent_id, limit))
}

pub(crate) fn set_auto_evolution_config(
    state: &AppState,
    agent_id: String,
    enabled: bool,
    threshold: f64,
    cooldown_seconds: u64,
) -> Result<(), String> {
    state.auto_evolution.set_evolution_config(
        &agent_id,
        AutoEvolveConfig {
            enabled,
            threshold,
            cooldown_seconds,
        },
    );
    Ok(())
}

pub(crate) fn force_evolve_agent(
    state: &AppState,
    agent_id: String,
) -> Result<nexus_kernel::genome::EvolutionResult, String> {
    let genome_path = format!("agents/genomes/{agent_id}.json");
    let genome_json = std::fs::read_to_string(&genome_path)
        .map_err(|e| format!("Cannot read genome for {agent_id}: {e}"))?;
    let genome: nexus_kernel::genome::AgentGenome =
        serde_json::from_str(&genome_json).map_err(|e| format!("Invalid genome JSON: {e}"))?;
    let llm = GatewayPlannerLlm;
    let result = state.auto_evolution.force_evolve(&agent_id, &genome, &llm);
    if result.improved {
        if let Some(evolved) = state.auto_evolution.apply_evolution(&genome, &llm) {
            if let Ok(json) = serde_json::to_string_pretty(&evolved) {
                // Best-effort: persist force-evolved genome to disk; failure is non-fatal
                let _ = std::fs::write(&genome_path, json);
            }
        }
    }
    Ok(result)
}

pub(crate) fn get_config() -> Result<NexusConfig, String> {
    load_config().map_err(agent_error)
}

pub(crate) fn save_config(config: NexusConfig) -> Result<(), String> {
    save_nexus_config(&config).map_err(agent_error)
}

pub(crate) fn transcribe_push_to_talk() -> Result<String, String> {
    let voice_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../voice");
    if !voice_dir.exists() {
        return Ok("voice runtime unavailable".to_string());
    }

    let output = Command::new("python3")
        .arg("-c")
        .arg(
            "from stt import FasterWhisperSTT; model=FasterWhisperSTT().model; print(f'push-to-talk via {model}')",
        )
        .current_dir(&voice_dir)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let text = String::from_utf8_lossy(&result.stdout).trim().to_string();
            if text.is_empty() {
                Ok("push-to-talk ready".to_string())
            } else {
                Ok(text)
            }
        }
        _ => Ok("push-to-talk captured audio".to_string()),
    }
}

/// Returns tray status data for the system tray indicator and Mission Control.
pub(crate) fn tray_status(state: &AppState) -> Result<TrayStatus, String> {
    let agents = list_agents(state)?;
    let running_agents = agents
        .iter()
        .filter(|agent| agent.status == "Running")
        .count();
    Ok(TrayStatus {
        running_agents,
        menu_items: vec![
            "Show Dashboard".to_string(),
            "Start Voice".to_string(),
            "Quit".to_string(),
        ],
    })
}

pub(crate) fn update_last_action(state: &AppState, agent_id: AgentId, action: &str) {
    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&agent_id) {
        meta.last_action = action.to_string();
    }
}

pub(crate) fn event_to_row(event: &AuditEvent) -> AuditRow {
    AuditRow {
        event_id: event.event_id.to_string(),
        timestamp: event.timestamp,
        agent_id: event.agent_id.to_string(),
        event_type: format!("{:?}", event.event_type),
        payload: event.payload.clone(),
        hash: event.hash.clone(),
        previous_hash: event.previous_hash.clone(),
    }
}

pub(crate) fn parse_agent_id(value: &str) -> Result<AgentId, String> {
    uuid::Uuid::parse_str(value).map_err(|error| format!("invalid agent_id: {error}"))
}

pub(crate) fn display_agent_state(state: &str) -> String {
    match state.trim().to_ascii_lowercase().as_str() {
        "created" => "Created".to_string(),
        "starting" => "Starting".to_string(),
        "running" => "Running".to_string(),
        "paused" => "Paused".to_string(),
        "stopping" => "Stopping".to_string(),
        "stopped" => "Stopped".to_string(),
        "destroyed" => "Destroyed".to_string(),
        "" => "Stopped".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => "Stopped".to_string(),
            }
        }
    }
}

pub(crate) fn agent_error(error: AgentError) -> String {
    error.to_string()
}

impl AppState {
    #[allow(dead_code)]
    pub(crate) fn load_prebuilt_agents(&self) {
        eprintln!("Loading prebuilt agents...");
        let manifest_paths = list_prebuilt_manifest_paths();
        let prebuilt_names = manifest_paths
            .iter()
            // Optional: skip manifest files that cannot be read (e.g. permission or encoding errors)
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .filter_map(|manifest_json| manifest_name_from_json(&manifest_json))
            .collect::<HashSet<_>>();
        self.prune_stale_test_agents(&prebuilt_names);
        eprintln!("prebuilt: discovered {} manifest(s)", manifest_paths.len());
        let marketplace_registry = match open_marketplace_registry() {
            Ok(registry) => Some(registry),
            Err(error) => {
                eprintln!("marketplace seed: failed to open registry: {error}");
                None
            }
        };
        let marketplace_author_key = nexus_crypto::CryptoIdentity::from_bytes(
            nexus_crypto::SignatureAlgorithm::Ed25519,
            &MARKETPLACE_SEED_KEY,
        )
        .unwrap_or_else(|e| panic!("marketplace seed key: {e}"));
        let mut existing_names = match self.db.list_agents() {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|row| manifest_name_from_json(&row.manifest_json))
                .collect::<HashSet<String>>(),
            Err(error) => {
                eprintln!("prebuilt: failed to load existing agents from DB: {error}");
                HashSet::new()
            }
        };

        for path in manifest_paths {
            let manifest_json = match std::fs::read_to_string(&path) {
                Ok(contents) => contents,
                Err(error) => {
                    eprintln!("prebuilt: failed to read {}: {error}", path.display());
                    continue;
                }
            };

            let manifest = match parse_agent_manifest_json(&manifest_json) {
                Ok(manifest) => manifest,
                Err(error) => {
                    eprintln!("prebuilt: failed to parse {}: {error}", path.display());
                    continue;
                }
            };
            let manifest_description = parse_manifest_description(&manifest_json);

            if existing_names.contains(&manifest.name)
                || database_contains_agent_name(self.db.as_ref(), &manifest.name)
            {
                continue;
            }

            let mut supervisor = match self.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let agent_id = match supervisor.start_agent(manifest.clone()) {
                Ok(agent_id) => agent_id,
                Err(error) => {
                    eprintln!("prebuilt: failed to register {}: {error}", manifest.name);
                    continue;
                }
            };
            // Best-effort: stop prebuilt agent after registration; it will be started on demand
            let _ = supervisor.stop_agent(agent_id);
            let agent_name = manifest.name.clone();

            if let Err(error) = self.db.save_agent(
                &agent_id.to_string(),
                &manifest_json,
                "stopped",
                manifest.autonomy_level.unwrap_or(0),
                "native",
            ) {
                eprintln!("prebuilt: failed to persist {}: {error}", agent_id);
                continue;
            }

            let mut meta = match self.meta.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            meta.insert(
                agent_id,
                AgentMeta {
                    name: agent_name,
                    last_action: "prebuilt".to_string(),
                },
            );

            if let Some(registry) = marketplace_registry.as_ref() {
                publish_prebuilt_manifest_to_marketplace(
                    registry,
                    &marketplace_author_key,
                    &manifest,
                    &manifest_description,
                );
            }

            existing_names.insert(manifest.name);
        }

        match self.db.list_agents() {
            Ok(rows) => eprintln!("prebuilt: database now tracks {} agent(s)", rows.len()),
            Err(error) => eprintln!("prebuilt: failed to query DB after load: {error}"),
        }
    }

    fn prune_stale_test_agents(&self, prebuilt_names: &HashSet<String>) {
        let rows = match self.db.list_agents() {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("prebuilt: failed to inspect DB for stale test agents: {error}");
                return;
            }
        };

        for row in rows {
            let Some(name) = manifest_name_from_json(&row.manifest_json) else {
                continue;
            };

            if row.parent_agent_id.is_some()
                || prebuilt_names.contains(&name)
                || !is_known_test_agent_name(&name)
            {
                continue;
            }

            match self.db.delete_agent(&row.id) {
                Ok(()) => eprintln!("prebuilt: removed stale test agent '{name}' ({})", row.id),
                Err(error) => eprintln!(
                    "prebuilt: failed removing stale test agent '{name}' ({}): {error}",
                    row.id
                ),
            }
        }
    }
}

#[allow(dead_code)]
const MARKETPLACE_SEED_KEY: [u8; 32] = [7; 32];

#[allow(dead_code)]
pub(crate) fn publish_prebuilt_manifest_to_marketplace(
    registry: &nexus_marketplace::sqlite_registry::SqliteRegistry,
    author_key: &nexus_crypto::CryptoIdentity,
    manifest: &AgentManifest,
    manifest_description: &str,
) {
    let package_name = sanitize_prebuilt_marketplace_name(&manifest.name);
    let already_published = match registry.search(&package_name) {
        Ok(results) => results.iter().any(|agent| agent.name == package_name),
        Err(_) => false,
    };
    if already_published {
        return;
    }

    let manifest_toml = match build_marketplace_manifest_toml(manifest, &package_name) {
        Ok(manifest_toml) => manifest_toml,
        Err(error) => {
            eprintln!(
                "marketplace seed: failed to format {} manifest: {error}",
                manifest.name
            );
            return;
        }
    };

    let mut tags = vec![
        "prebuilt".to_string(),
        "nexus-os".to_string(),
        "automated-agent".to_string(),
    ];
    if let Some(level) = manifest.autonomy_level {
        tags.push(format!("l{level}"));
    }
    if manifest.schedule.is_some() {
        tags.push("scheduled".to_string());
    }

    let metadata = nexus_marketplace::package::PackageMetadata {
        name: package_name,
        version: manifest.version.clone(),
        description: manifest_description.to_string(),
        capabilities: manifest.capabilities.clone(),
        tags,
        author_id: "nexus-os".to_string(),
    };

    let unsigned_bundle = match nexus_marketplace::package::create_unsigned_bundle(
        &manifest_toml,
        &format!("// prebuilt agent manifest for {}", manifest.name),
        metadata,
        "local://agents/prebuilt",
        "nexus-desktop-backend",
    ) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!(
                "marketplace seed: failed to build {} bundle: {error}",
                manifest.name
            );
            return;
        }
    };

    if let Err(error) = nexus_marketplace::verification_pipeline::verified_publish_sqlite(
        registry,
        unsigned_bundle,
        author_key,
    ) {
        eprintln!(
            "marketplace seed: failed to publish {}: {error}",
            manifest.name
        );
    }
}

#[allow(dead_code)]
pub(crate) fn list_prebuilt_manifest_paths() -> Vec<PathBuf> {
    let Some(prebuilt_dir) = resolve_prebuilt_manifest_dir() else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(&prebuilt_dir) else {
        return Vec::new();
    };

    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(crate) fn resolve_prebuilt_manifest_dir() -> Option<PathBuf> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut candidate_dirs = vec![workspace_root.join("agents/prebuilt")];

    if let Ok(current_dir) = std::env::current_dir() {
        candidate_dirs.push(current_dir.join("agents/prebuilt"));
        if let Some(parent) = current_dir.parent() {
            candidate_dirs.push(parent.join("agents/prebuilt"));
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors() {
            candidate_dirs.push(ancestor.join("agents/prebuilt"));
        }
    }

    let mut seen = HashSet::new();
    for candidate in candidate_dirs {
        let normalized = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        if !seen.insert(normalized.clone()) {
            continue;
        }
        if normalized.is_dir() {
            eprintln!(
                "prebuilt: using manifest directory {}",
                normalized.display()
            );
            return Some(normalized);
        }
    }

    eprintln!(
        "prebuilt: failed to locate agents/prebuilt from current_dir={} manifest_dir={}",
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string()),
        env!("CARGO_MANIFEST_DIR"),
    );
    None
}

#[derive(serde::Serialize)]
#[allow(dead_code)]
pub(crate) struct MarketplaceManifestToml<'a> {
    name: &'a str,
    version: &'a str,
    capabilities: &'a [String],
    fuel_budget: u64,
    autonomy_level: Option<u8>,
    schedule: Option<&'a str>,
    default_goal: Option<&'a str>,
    llm_model: Option<&'a str>,
}

#[allow(dead_code)]
pub(crate) fn sanitize_prebuilt_marketplace_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "prebuilt-agent".to_string()
    } else {
        sanitized
    }
}

#[allow(dead_code)]
pub(crate) fn build_marketplace_manifest_toml(
    manifest: &AgentManifest,
    package_name: &str,
) -> Result<String, toml::ser::Error> {
    let toml_manifest = MarketplaceManifestToml {
        name: package_name,
        version: &manifest.version,
        capabilities: &manifest.capabilities,
        fuel_budget: manifest.fuel_budget,
        autonomy_level: manifest.autonomy_level,
        schedule: manifest.schedule.as_deref(),
        default_goal: manifest.default_goal.as_deref(),
        llm_model: manifest.llm_model.as_deref(),
    };
    toml::to_string(&toml_manifest)
}

#[allow(dead_code)]
pub(crate) fn parse_manifest_description(manifest_json: &str) -> String {
    // Optional: malformed JSON falls through to default description below
    serde_json::from_str::<Value>(manifest_json)
        .ok()
        .and_then(|value| {
            value
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Nexus prebuilt agent".to_string())
}

#[cfg_attr(test, allow(dead_code))]
const LEGACY_DB_CLEANUP_FLAG: &str = ".db_cleanup_prebuilt_v1";

pub(crate) fn database_contains_agent_name(db: &NexusDatabase, name: &str) -> bool {
    db.list_agents()
        .map(|rows| {
            rows.into_iter().any(|row| {
                manifest_name_from_json(&row.manifest_json)
                    .map(|existing| existing == name)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn legacy_db_cleanup_flag_path(db_path: &std::path::Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(LEGACY_DB_CLEANUP_FLAG)
}

pub(crate) fn cleanup_legacy_agent_db_if_needed(
    db_path: &std::path::Path,
    flag_path: &std::path::Path,
) {
    if flag_path.exists() {
        return;
    }

    if let Some(parent) = flag_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!(
                "startup cleanup: failed to prepare {}: {error}",
                parent.display()
            );
            return;
        }
    }

    if db_path.exists() {
        match std::fs::remove_file(db_path) {
            Ok(()) => eprintln!(
                "startup cleanup: removed stale database {}",
                db_path.display()
            ),
            Err(error) => {
                eprintln!(
                    "startup cleanup: failed removing {}: {error}",
                    db_path.display()
                );
                return;
            }
        }
    }

    if let Err(error) = std::fs::write(flag_path, "prebuilt-cleanup-complete\n") {
        eprintln!(
            "startup cleanup: failed writing {}: {error}",
            flag_path.display()
        );
    }
}

#[cfg(not(test))]
pub(crate) fn maybe_cleanup_legacy_agent_db() {
    if std::env::var("NEXUS_DB_PATH").is_ok() {
        return;
    }

    let db_path = NexusDatabase::default_db_path();
    let flag_path = legacy_db_cleanup_flag_path(&db_path);
    cleanup_legacy_agent_db_if_needed(&db_path, &flag_path);
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonAgentManifest {
    #[serde(flatten)]
    pub(crate) manifest: AgentManifest,
    #[allow(dead_code)]
    pub(crate) description: Option<String>,
}

pub(crate) fn parse_agent_manifest_json(manifest_json: &str) -> Result<AgentManifest, String> {
    let json_manifest: JsonAgentManifest = serde_json::from_str(manifest_json)
        .map_err(|error| format!("invalid manifest JSON: {error}"))?;
    let manifest_toml = toml::to_string(&json_manifest.manifest)
        .map_err(|error| format!("failed to serialize manifest for validation: {error}"))?;
    parse_manifest(&manifest_toml).map_err(agent_error)
}

pub(crate) fn find_manifest(state: &AppState, agent_id: &str) -> Option<AgentManifest> {
    let rows = match state.db.list_agents() {
        Ok(rows) => rows,
        Err(_) => return None,
    };

    rows.iter()
        .find(|row| row.id == agent_id)
        // Optional: manifest JSON may be corrupted or from older schema version
        .and_then(|row| serde_json::from_str::<AgentManifest>(&row.manifest_json).ok())
}

#[allow(dead_code)]
pub(crate) fn manifest_name_from_json(manifest_json: &str) -> Option<String> {
    // Optional: returns None if JSON is malformed rather than propagating parse error
    serde_json::from_str::<Value>(manifest_json)
        .ok()?
        .get("name")
        .and_then(|value| value.as_str())
        .map(std::string::ToString::to_string)
        .filter(|name| !name.trim().is_empty())
}

pub(crate) fn is_known_test_agent_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "a-agent" | "b-agent" | "c-agent" | "my-social-poster"
    )
}

pub(crate) fn extract_manifest_description(manifest_json: &str) -> Option<String> {
    // Optional: returns None if JSON is malformed rather than propagating parse error
    serde_json::from_str::<Value>(manifest_json)
        .ok()?
        .get("description")
        .and_then(|value| value.as_str())
        .map(|desc| desc.trim().to_string())
        .filter(|desc| !desc.is_empty())
}

pub(crate) fn find_manifest_description(state: &AppState, agent_id: &str) -> Option<String> {
    // Optional: returns None if DB query fails rather than propagating error
    let rows = state.db.list_agents().ok()?;
    rows.iter()
        .find(|row| row.id == agent_id)
        .and_then(|row| extract_manifest_description(&row.manifest_json))
}

pub(crate) fn goal_with_manifest_context(
    agent_id: &str,
    goal: &str,
    description: Option<&str>,
) -> String {
    let goal = if goal.trim().is_empty() {
        description
            .and_then(|value| {
                let first_sentence = value
                    .split('.')
                    .next()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())?;
                Some(format!("Work autonomously on: {first_sentence}"))
            })
            .unwrap_or_else(|| "Execute scheduled task".to_string())
    } else {
        goal.to_string()
    };

    match description {
        Some(description) if !description.trim().is_empty() => {
            format!("{goal}\n\nAgent Manifest Instructions:\n{description}\nFor Agent: {agent_id}")
        }
        _ => goal.to_string(),
    }
}

pub(crate) fn create_warden_consent_request(
    state: &AppState,
    agent_id: &str,
    agent_name: &str,
    action: &PlannedAction,
    reason: &str,
) -> Result<String, String> {
    let consent_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let summary = format!(
        "Warden blocked {} for {}",
        format_hitl_action_summary(action),
        agent_name
    );
    let operation_json = json!({
        "summary": summary,
        "fuel_cost": 0.0,
        "side_effects": [format_hitl_action_summary(action)],
        "warden_reason": reason,
        "goal_id": state
            .cognitive_runtime
            .get_agent_status(agent_id)
            .and_then(|status| status.active_goal.map(|goal| goal.id))
    });
    let row = nexus_persistence::ConsentRow {
        id: consent_id.clone(),
        agent_id: agent_id.to_string(),
        operation_type: "warden_review".to_string(),
        operation_json: operation_json.to_string(),
        hitl_tier: "Tier2".to_string(),
        status: "pending".to_string(),
        created_at: now,
        resolved_at: None,
        resolved_by: None,
    };
    state
        .db
        .enqueue_consent(&row)
        .map_err(|e| format!("db error: {e}"))?;
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    if let Some(app) = state.app_handle() {
        let notification = consent_row_to_notification(&row, agent_name);
        // Best-effort: notify frontend of pending consent; UI will see it on next poll
        let _ = app.emit("consent-request-pending", notification);
    }
    state.log_event(
        Uuid::parse_str(agent_id).unwrap_or_default(),
        EventType::StateChange,
        json!({
            "action": "warden_decision",
            "decision": "NO",
            "agent_name": agent_name,
            "consent_id": consent_id,
            "reason": reason,
        }),
    );
    Ok(consent_id)
}

pub(crate) fn register_manifest_schedule(
    state: &AppState,
    agent_id: &str,
    schedule: Option<&str>,
    default_goal: Option<&str>,
    manifest_description: Option<&str>,
) {
    let Some(cron_expr) = schedule else {
        return;
    };

    let goal = goal_with_manifest_context(
        agent_id,
        default_goal.unwrap_or("Execute scheduled task"),
        manifest_description,
    );

    if let Err(error) = state
        .agent_scheduler
        .register_agent(agent_id, cron_expr, &goal)
    {
        eprintln!("scheduler: failed to register {agent_id}: {error}");
    }
}

/// Seed the ScheduleRunner's ScheduleStore from agent manifests that have schedule + default_goal.
/// This bridges prebuilt agents with the background scheduler on startup.
pub(crate) fn seed_manifests_to_runner(state: &AppState) {
    let agents = match state.db.list_agents() {
        Ok(rows) => rows,
        Err(_) => return,
    };

    for row in agents {
        if !row.was_running {
            continue;
        }
        let Ok(json_manifest) = serde_json::from_str::<serde_json::Value>(&row.manifest_json)
        else {
            continue;
        };

        let schedule = json_manifest
            .get("schedule")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let default_goal = json_manifest
            .get("default_goal")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let fuel_budget = json_manifest
            .get("fuel_budget")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000);
        let name = json_manifest
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        if let (Some(cron), Some(goal)) = (schedule, default_goal) {
            state
                .schedule_runner
                .seed_from_agent(&row.id, &name, &cron, &goal, fuel_budget);
        }
    }
}

// ── Setup Wizard Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HardwareInfo {
    pub gpu: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub detected_at: String,
    pub tier: String,
    pub recommended_primary: String,
    pub recommended_fast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaStatus {
    pub connected: bool,
    pub base_url: String,
    pub models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetupResult {
    pub hardware: HardwareInfo,
    pub ollama: OllamaStatus,
    pub config_saved: bool,
}

// ── Setup Wizard Functions ──

pub(crate) fn detect_hardware() -> Result<HardwareInfo, String> {
    let hw = HardwareProfile::detect();
    let tier = hw.recommended_tier();
    Ok(HardwareInfo {
        gpu: hw.gpu,
        vram_mb: hw.vram_mb,
        ram_mb: hw.ram_mb,
        detected_at: hw.detected_at,
        tier: tier.label().to_string(),
        recommended_primary: tier.primary_model().to_string(),
        recommended_fast: tier.fast_model().to_string(),
    })
}

pub(crate) fn check_ollama(base_url: Option<String>) -> Result<OllamaStatus, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);

    let connected = provider.health_check().unwrap_or(false);
    let models = if connected {
        provider
            .list_models()
            .unwrap_or_default()
            .into_iter()
            .map(|m| OllamaModelInfo {
                name: m.name,
                size: m.size,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(OllamaStatus {
        connected,
        base_url: url,
        models,
    })
}

pub(crate) fn pull_ollama_model(
    model_name: String,
    base_url: Option<String>,
) -> Result<String, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);
    provider
        .pull_model(&model_name, |_status, _completed, _total| {})
        .map_err(|e| e.to_string())
}

/// Pull a model with throttled progress events (max ~3/sec).
/// The callback is only invoked every 300ms for progress updates,
/// but always fires immediately for "success" and error statuses.
pub(crate) fn pull_ollama_model_throttled<F>(
    model_name: String,
    base_url: Option<String>,
    mut on_event: F,
) -> Result<String, String>
where
    F: FnMut(ModelPullProgress),
{
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);
    let model_id = model_name.clone();
    let mut last_emit = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap_or_else(std::time::Instant::now);

    provider
        .pull_model(&model_name, |status, completed, total| {
            // Always emit terminal statuses immediately
            if status == "success" || status.contains("error") {
                let percent = if status == "success" { 100 } else { 0 };
                on_event(ModelPullProgress {
                    model: model_id.clone(),
                    status: status.to_string(),
                    percent,
                    completed_bytes: completed,
                    total_bytes: total,
                    error: if status.contains("error") {
                        Some(status.to_string())
                    } else {
                        None
                    },
                });
                return;
            }

            // Throttle: skip if <300ms since last emit
            let now = std::time::Instant::now();
            if now.duration_since(last_emit).as_millis() < 300 {
                return;
            }
            last_emit = now;

            let percent = if total > 0 {
                ((completed as f64 / total as f64) * 100.0).round() as u32
            } else {
                0
            };

            on_event(ModelPullProgress {
                model: model_id.clone(),
                status: status.to_string(),
                percent: percent.min(99),
                completed_bytes: completed,
                total_bytes: total,
                error: None,
            });
        })
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPullProgress {
    pub model: String,
    pub status: String,
    pub percent: u32,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Ensure Ollama server is running. Returns true if already running or started.
pub(crate) fn ensure_ollama(base_url: Option<String>) -> Result<bool, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);

    // Check if already running
    if provider.health_check().unwrap_or(false) {
        return Ok(true);
    }

    // Try to start ollama serve in the background
    let started = Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    if started.is_err() {
        return Err(
            "Ollama is not installed. Please install it from https://ollama.ai".to_string(),
        );
    }

    // Wait up to 8 seconds for it to come online
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if provider.health_check().unwrap_or(false) {
            return Ok(true);
        }
    }

    Err("Ollama was started but did not respond within 8 seconds".to_string())
}

/// Check if ollama binary is available on PATH.
pub(crate) fn is_ollama_installed() -> bool {
    Command::new("ollama")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Delete a model from Ollama.
pub(crate) fn delete_ollama_model(
    model_name: String,
    base_url: Option<String>,
) -> Result<(), String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let endpoint = format!("{}/api/delete", url.trim_end_matches('/'));
    let body = json!({ "name": model_name });
    let encoded = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let output = Command::new("curl")
        .args(["-sS", "-X", "DELETE"])
        .arg("-H")
        .arg("content-type: application/json")
        .arg("-d")
        .arg(&encoded)
        .arg(&endpoint)
        .output()
        .map_err(|e| format!("Failed to run curl: {e}"))?;

    if !output.status.success() {
        return Err("Failed to delete model".to_string());
    }
    Ok(())
}

/// Available model entry for the model browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModel {
    pub id: String,
    pub name: String,
    pub size_gb: f64,
    pub context: String,
    pub capabilities: Vec<String>,
    pub recommended: bool,
    pub tag: String,
    pub installed: bool,
    pub description: String,
}

/// List all Qwen 3.5 models with hardware-aware recommendations.
pub(crate) fn list_available_models() -> Result<Vec<AvailableModel>, String> {
    let hw = HardwareProfile::detect();
    let vram = hw.vram_mb;
    let ram = hw.ram_mb;

    let provider = OllamaProvider::new("http://localhost:11434");
    let installed_names: Vec<String> = provider
        .list_models()
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name)
        .collect();

    let has = |id: &str| installed_names.iter().any(|n| n == id);

    let mut models = vec![
        AvailableModel {
            id: "qwen3.5:0.8b".into(),
            name: "Qwen 3.5 0.8B".into(),
            size_gb: 1.0,
            context: "256K".into(),
            capabilities: vec!["Text".into(), "Vision".into(), "Tools".into()],
            recommended: false,
            tag: "Ultra-light".into(),
            installed: has("qwen3.5:0.8b"),
            description: "Smallest model — runs on anything".into(),
        },
        AvailableModel {
            id: "qwen3.5:2b".into(),
            name: "Qwen 3.5 2B".into(),
            size_gb: 2.7,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Lightweight".into(),
            installed: has("qwen3.5:2b"),
            description: "Fast responses, great for quick tasks".into(),
        },
        AvailableModel {
            id: "qwen3.5:4b".into(),
            name: "Qwen 3.5 4B".into(),
            size_gb: 3.4,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Balanced".into(),
            installed: has("qwen3.5:4b"),
            description: "Good balance of speed and quality".into(),
        },
        AvailableModel {
            id: "qwen3.5:9b".into(),
            name: "Qwen 3.5 9B".into(),
            size_gb: 6.6,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Recommended".into(),
            installed: has("qwen3.5:9b"),
            description: "Best quality for consumer GPUs".into(),
        },
        AvailableModel {
            id: "qwen3.5:27b".into(),
            name: "Qwen 3.5 27B".into(),
            size_gb: 17.0,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "High-end".into(),
            installed: has("qwen3.5:27b"),
            description: "Premium quality — needs 24GB+ VRAM or 32GB+ RAM".into(),
        },
        AvailableModel {
            id: "qwen3.5:35b".into(),
            name: "Qwen 3.5 35B MoE".into(),
            size_gb: 24.0,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "MoE — only 3B active".into(),
            installed: has("qwen3.5:35b"),
            description: "35B total but only 3B active — fast with enough RAM".into(),
        },
    ];

    // Mark recommendations based on hardware
    for m in &mut models {
        match m.id.as_str() {
            "qwen3.5:9b" if vram >= 8000 => {
                m.recommended = true;
                m.tag = format!("Recommended for your {}MB VRAM", vram);
                m.description = format!("Best model for your {}MB VRAM — fits perfectly", vram);
            }
            "qwen3.5:4b" if (4000..8000).contains(&vram) => {
                m.recommended = true;
                m.tag = "Recommended for your GPU".into();
            }
            "qwen3.5:4b" if vram >= 8000 => {
                m.tag = "Fast companion".into();
                m.description = "Use alongside 9B for quick background tasks".into();
            }
            "qwen3.5:35b" if ram >= 48000 => {
                m.recommended = true;
                m.tag = format!("Bonus — your {}GB RAM enables this", ram / 1024);
                m.description = "MoE model with GPU+RAM offload — premium quality".into();
            }
            "qwen3.5:27b" if vram >= 20000 => {
                m.recommended = true;
                m.tag = "Best for your GPU".into();
            }
            "qwen3.5:27b" if vram < 20000 && ram < 32000 => {
                m.tag = "Too large for your system".into();
            }
            _ => {}
        }
    }

    // Add already-installed non-qwen3.5 models
    for name in &installed_names {
        if !models.iter().any(|m| m.id == *name) {
            models.push(AvailableModel {
                id: name.clone(),
                name: name.replace([':', '-'], " "),
                size_gb: 0.0,
                context: "varies".into(),
                capabilities: vec!["Text".into()],
                recommended: false,
                tag: "Already installed".into(),
                installed: true,
                description: "Previously downloaded model".into(),
            });
        }
    }

    Ok(models)
}

// ── Multi-Provider Model Listing & API Key Management ───────────────

/// A model entry with provider information for the multi-provider model picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub local: bool,
    pub requires_key: bool,
    pub size_gb: Option<f64>,
    pub installed: bool,
}

/// Provider status entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub ollama: bool,
    pub anthropic: bool,
    pub openai: bool,
    pub deepseek: bool,
    pub gemini: bool,
    pub nvidia: bool,
    pub groq: bool,
}

/// List models from ALL configured providers (Ollama + cloud).
pub(crate) fn list_provider_models() -> Result<Vec<ProviderModel>, String> {
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);
    let mut models = Vec::new();

    // ── Ollama (local) ──
    let ollama_url = prov_config
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let ollama = OllamaProvider::new(&ollama_url);
    if let Ok(ollama_models) = ollama.list_models() {
        for m in ollama_models {
            models.push(ProviderModel {
                id: format!("ollama/{}", m.name),
                name: m.name.clone(),
                provider: "ollama".into(),
                local: true,
                requires_key: false,
                size_gb: Some(m.size as f64 / 1_073_741_824.0),
                installed: true,
            });
        }
    }

    // ── Anthropic (Claude) ──
    if has_provider_key(&prov_config.anthropic_api_key) {
        for (id, name) in [
            ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
            ("claude-haiku-4-5", "Claude Haiku 4.5"),
            ("claude-opus-4-6", "Claude Opus 4.6"),
        ] {
            models.push(ProviderModel {
                id: format!("anthropic/{id}"),
                name: name.into(),
                provider: "anthropic".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── OpenAI ──
    if has_provider_key(&prov_config.openai_api_key) {
        for (id, name) in [
            ("gpt-4.1-nano", "GPT-4.1 Nano"),
            ("gpt-4.1-mini", "GPT-4.1 Mini"),
            ("gpt-4.1", "GPT-4.1"),
            ("gpt-5-mini", "GPT-5 Mini"),
            ("gpt-5", "GPT-5"),
            ("gpt-4o", "GPT-4o"),
            ("gpt-4o-mini", "GPT-4o Mini"),
            ("o3-mini", "o3 Mini"),
        ] {
            models.push(ProviderModel {
                id: format!("openai/{id}"),
                name: name.into(),
                provider: "openai".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── DeepSeek ──
    if has_provider_key(&prov_config.deepseek_api_key) {
        for (id, name) in [
            ("deepseek-chat", "DeepSeek Chat"),
            ("deepseek-coder", "DeepSeek Coder"),
            ("deepseek-reasoner", "DeepSeek Reasoner"),
        ] {
            models.push(ProviderModel {
                id: format!("deepseek/{id}"),
                name: name.into(),
                provider: "deepseek".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── Gemini ──
    if has_provider_key(&prov_config.gemini_api_key) {
        for (id, name) in [
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("gemini-2.5-flash", "Gemini 2.5 Flash"),
        ] {
            models.push(ProviderModel {
                id: format!("google/{id}"),
                name: name.into(),
                provider: "google".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── NVIDIA NIM ──
    if has_provider_key(&prov_config.nvidia_api_key) {
        for (id, name) in NVIDIA_MODELS.iter().copied() {
            models.push(ProviderModel {
                id: format!("nvidia/{id}"),
                name: name.into(),
                provider: "nvidia".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── Groq ──
    if has_provider_key(&prov_config.groq_api_key) {
        for (id, name) in GROQ_MODELS.iter().copied() {
            models.push(ProviderModel {
                id: format!("groq/{id}"),
                name: name.into(),
                provider: "groq".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    // ── OpenRouter (free models available!) ──
    if has_provider_key(&prov_config.openrouter_api_key) {
        for (id, name) in OPENROUTER_MODELS.iter().copied() {
            models.push(ProviderModel {
                id: format!("openrouter/{id}"),
                name: name.into(),
                provider: "openrouter".into(),
                local: false,
                requires_key: true,
                size_gb: None,
                installed: true,
            });
        }
    }

    Ok(models)
}

pub(crate) fn has_provider_key(key: &Option<String>) -> bool {
    key.as_deref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
}

/// Check which providers have API keys configured.
pub(crate) fn get_provider_status() -> Result<ProviderStatus, String> {
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);

    // Check Ollama reachability
    let ollama_url = prov_config
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let ollama = OllamaProvider::new(&ollama_url);
    let ollama_ok = ollama.health_check().unwrap_or(false);

    Ok(ProviderStatus {
        ollama: ollama_ok,
        anthropic: has_provider_key(&prov_config.anthropic_api_key),
        openai: has_provider_key(&prov_config.openai_api_key),
        deepseek: has_provider_key(&prov_config.deepseek_api_key),
        gemini: has_provider_key(&prov_config.gemini_api_key),
        nvidia: has_provider_key(&prov_config.nvidia_api_key),
        groq: has_provider_key(&prov_config.groq_api_key),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableProvider {
    pub id: String,
    pub name: String,
    pub status: String,
    pub available: bool,
    pub model: Option<String>,
    pub models: Vec<String>,
    /// File paths corresponding to each entry in `models` (Flash only).
    /// For non-Flash providers this is empty.
    pub model_paths: Vec<String>,
}

/// Returns a list of all LLM providers with their live status.
/// Used by the ProviderSelector component on the Agents page.
pub(crate) async fn get_available_providers(
    state: &AppState,
) -> Result<Vec<AvailableProvider>, String> {
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);
    let mut providers = Vec::new();

    // ── Flash Inference ──
    // 1. Check session manager for loaded models (primary: model is in memory)
    // 2. Check provider cache (secondary: FlashProvider instances)
    // 3. Scan local .gguf files (tertiary: models on disk, can be loaded)
    {
        let sessions = state.flash_session_manager.list_sessions().await;
        let active_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    nexus_flash_infer::SessionStatus::Ready
                        | nexus_flash_infer::SessionStatus::Generating
                        | nexus_flash_infer::SessionStatus::Idle
                        | nexus_flash_infer::SessionStatus::Loading
                )
            })
            .collect();

        // Also check the provider cache (populated during flash_generate calls)
        let cached_provider_count = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len();

        // Scan local .gguf model files on disk
        let local_models: Vec<nexus_flash_infer::LocalModel> =
            nexus_flash_infer::ModelStorage::new()
                .and_then(|s| s.list_models())
                .unwrap_or_default();
        // Build name→path lookup from local models
        let local_model_names: Vec<String> = local_models.iter().map(|m| m.name.clone()).collect();
        let local_model_paths: Vec<String> =
            local_models.iter().map(|m| m.file_path.clone()).collect();

        // Helper: build parallel model_paths vec aligned with models vec
        let build_paths = |models: &[String]| -> Vec<String> {
            models
                .iter()
                .map(|name| {
                    // Look up file path for this model name; also check active sessions
                    local_models
                        .iter()
                        .find(|lm| &lm.name == name)
                        .map(|lm| lm.file_path.clone())
                        .or_else(|| {
                            active_sessions
                                .iter()
                                .find(|s| &s.model_name == name)
                                .map(|s| s.model_path.clone())
                        })
                        .unwrap_or_default()
                })
                .collect()
        };

        if !active_sessions.is_empty() {
            // Sessions loaded in memory — Flash is ready
            let session_models: Vec<String> = active_sessions
                .iter()
                .map(|s| s.model_name.clone())
                .collect();
            let first = session_models.first().cloned();
            let is_busy = active_sessions
                .iter()
                .any(|s| matches!(s.status, nexus_flash_infer::SessionStatus::Generating));
            // Merge: session models + local models (dedup)
            let mut all_models = session_models.clone();
            for name in &local_model_names {
                if !all_models.contains(name) {
                    all_models.push(name.clone());
                }
            }
            let paths = build_paths(&all_models);
            providers.push(AvailableProvider {
                id: "flash".into(),
                name: "Flash Inference".into(),
                status: if is_busy {
                    "busy".into()
                } else {
                    "ready".into()
                },
                available: true,
                model: first,
                models: all_models,
                model_paths: paths,
            });
        } else if cached_provider_count > 0 {
            providers.push(AvailableProvider {
                id: "flash".into(),
                name: "Flash Inference".into(),
                status: "ready".into(),
                available: true,
                model: local_model_names.first().cloned(),
                model_paths: local_model_paths.clone(),
                models: local_model_names,
            });
        } else if !local_models.is_empty() {
            // .gguf files on disk — selectable, user can load from here
            providers.push(AvailableProvider {
                id: "flash".into(),
                name: "Flash Inference".into(),
                status: "models_on_disk".into(),
                available: true,
                model: local_model_names.first().cloned(),
                model_paths: local_model_paths,
                models: local_model_names,
            });
        } else {
            providers.push(AvailableProvider {
                id: "flash".into(),
                name: "Flash Inference".into(),
                status: "no_model".into(),
                available: false,
                model: None,
                models: vec![],
                model_paths: vec![],
            });
        }
    }

    // ── Ollama ──
    {
        let ollama_url = prov_config
            .ollama_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let ollama = OllamaProvider::new(&ollama_url);
        let reachable = ollama.health_check().unwrap_or(false);
        if reachable {
            let models: Vec<String> = ollama
                .list_models()
                .unwrap_or_default()
                .into_iter()
                .map(|m| m.name)
                .collect();
            let first = models.first().cloned();
            providers.push(AvailableProvider {
                id: "ollama".into(),
                name: "Ollama".into(),
                status: "running".into(),
                available: !models.is_empty(),
                model: first,
                models,
                model_paths: vec![],
            });
        } else {
            providers.push(AvailableProvider {
                id: "ollama".into(),
                name: "Ollama".into(),
                status: "stopped".into(),
                available: false,
                model: None,
                models: vec![],
                model_paths: vec![],
            });
        }
    }

    // ── Cloud providers ──
    let cloud_providers = [
        ("groq", "Groq", &prov_config.groq_api_key),
        ("deepseek", "DeepSeek", &prov_config.deepseek_api_key),
        ("openai", "OpenAI", &prov_config.openai_api_key),
        ("gemini", "Google Gemini", &prov_config.gemini_api_key),
        ("nvidia", "NVIDIA NIM", &prov_config.nvidia_api_key),
        ("anthropic", "Anthropic", &prov_config.anthropic_api_key),
    ];
    for (id, name, key) in &cloud_providers {
        let configured = has_provider_key(key);
        providers.push(AvailableProvider {
            id: id.to_string(),
            name: name.to_string(),
            status: if configured {
                "configured".into()
            } else {
                "not_configured".into()
            },
            available: configured,
            model: None,
            models: vec![],
            model_paths: vec![],
        });
    }

    Ok(providers)
}

/// Save an API key for a provider into `~/.nexus/config.toml` and set the
/// environment variable for the current session.
pub(crate) fn save_provider_api_key(provider: String, api_key: String) -> Result<(), String> {
    let mut config = load_config().map_err(agent_error)?;

    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => {
            config.llm.anthropic_api_key = api_key.clone();
            std::env::set_var("ANTHROPIC_API_KEY", &api_key);
        }
        "openai" => {
            config.llm.openai_api_key = api_key.clone();
            std::env::set_var("OPENAI_API_KEY", &api_key);
        }
        "deepseek" => {
            config.llm.deepseek_api_key = api_key.clone();
            std::env::set_var("DEEPSEEK_API_KEY", &api_key);
        }
        "gemini" | "google" => {
            config.llm.gemini_api_key = api_key.clone();
            std::env::set_var("GEMINI_API_KEY", &api_key);
        }
        "nvidia" | "nvidia-nim" | "nim" => {
            config.llm.nvidia_api_key = api_key.clone();
            std::env::set_var("NVIDIA_NIM_API_KEY", &api_key);
        }
        "groq" => {
            std::env::set_var("GROQ_API_KEY", &api_key);
        }
        _ => return Err(format!("Unknown provider: {provider}")),
    }

    save_nexus_config(&config).map_err(agent_error)
}

/// Create an LLM provider from a "provider/model" string.
/// Returns `(Box<dyn LlmProvider>, model_name)`.
pub(crate) fn provider_from_prefixed_model(
    full_model: &str,
    prov_config: &ProviderSelectionConfig,
) -> Result<(Box<dyn LlmProvider>, String), String> {
    if let Some((provider_prefix, model_name)) = full_model.split_once('/') {
        let provider: Box<dyn LlmProvider> = match provider_prefix {
            "ollama" => {
                let url = prov_config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string());
                Box::new(OllamaProvider::new(&url))
            }
            "anthropic" => Box::new(ClaudeProvider::new(prov_config.anthropic_api_key.clone())),
            "openai" => Box::new(OpenAiProvider::new(prov_config.openai_api_key.clone())),
            "deepseek" => Box::new(DeepSeekProvider::new(prov_config.deepseek_api_key.clone())),
            "google" | "gemini" => {
                Box::new(GeminiProvider::new(prov_config.gemini_api_key.clone()))
            }
            "nvidia" | "nvidia-nim" | "nim" => {
                Box::new(NvidiaProvider::new(prov_config.nvidia_api_key.clone()))
            }
            "groq" => Box::new(GroqProvider::new(prov_config.groq_api_key.clone())),
            "openrouter" => Box::new(OpenRouterProvider::new(
                prov_config.openrouter_api_key.clone(),
            )),
            #[cfg(feature = "flash-infer")]
            #[allow(unexpected_cfgs)]
            "flash" => {
                // model_name is the path to the GGUF file.
                // Use auto-configured settings for the model.
                Box::new(nexus_connectors_llm::providers::FlashProvider::new(
                    model_name.to_string(),
                    nexus_flash_infer::LoadConfig {
                        model_path: model_name.to_string(),
                        n_threads: Some(8),
                        n_ctx: 2048,
                        n_batch: 512,
                        ..Default::default()
                    },
                    nexus_flash_infer::GenerationConfig {
                        n_ctx: 2048,
                        n_batch: 512,
                        n_threads: Some(8),
                        ..nexus_flash_infer::GenerationConfig::fast()
                    },
                ))
            }
            _ => return Err(format!("Unknown provider prefix: {provider_prefix}")),
        };
        Ok((provider, model_name.to_string()))
    } else {
        // No prefix — use legacy select_provider behavior
        let provider = select_provider(prov_config).map_err(|e| e.to_string())?;
        Ok((provider, full_model.to_string()))
    }
}

/// Stream a chat completion through Ollama with governance enforcement.
///
/// Pre-flight: PII redaction + prompt firewall on the last user message.
/// Post-flight: audit event with token count and model.
/// The `on_token` callback is called with each token for streaming.
pub(crate) fn chat_with_ollama_streaming<F>(
    state: &AppState,
    messages: Vec<serde_json::Value>,
    model: String,
    base_url: Option<String>,
    mut on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    state.check_rate(nexus_kernel::rate_limit::RateCategory::LlmRequest)?;
    let config = load_config().map_err(|e| e.to_string())?;
    let url = base_url.unwrap_or_else(|| {
        let cfg_url = config.llm.ollama_url.trim();
        if cfg_url.is_empty() {
            "http://localhost:11434".to_string()
        } else {
            cfg_url.to_string()
        }
    });
    let provider = OllamaProvider::new(&url);

    // Ensure Ollama is running first
    if !provider.health_check().unwrap_or(false) {
        return Err(format!(
            "Ollama is not running at {url}. Start it with: ollama serve"
        ));
    }

    // Governance pre-flight: redact PII and check firewall on last user message
    let chat_agent_id = Uuid::new_v4();
    let mut governed_messages = messages.clone();
    if let Some(last_user) = governed_messages
        .iter_mut()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
    {
        if let Some(content) = last_user.get("content").and_then(|c| c.as_str()) {
            // PII redaction
            let mut redaction_engine =
                nexus_kernel::redaction::RedactionEngine::new(Default::default());
            let result = redaction_engine.process_prompt(
                "llm.chat_stream",
                "strict",
                vec![chat_agent_id.to_string()],
                content,
            );
            let redacted = result.outbound_prompt.clone();

            // Prompt firewall check
            let mut input_filter = nexus_kernel::firewall::prompt_firewall::InputFilter::new();
            let mut audit = match state.audit.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            match input_filter.check(chat_agent_id, &redacted, &mut audit) {
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Block { reason } => {
                    return Err(format!("Prompt blocked by firewall: {reason}"));
                }
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Redacted {
                    redacted_text,
                    ..
                } => {
                    *last_user = json!({"role": "user", "content": redacted_text});
                }
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Allow => {
                    // Use the PII-redacted version even if firewall allows
                    if result.summary.total_findings > 0 {
                        *last_user = json!({"role": "user", "content": redacted});
                    }
                }
            }
        }
    }

    let started = std::time::Instant::now();
    let result = provider
        .chat_stream(&governed_messages, &model, |token| {
            on_token(token);
        })
        .map_err(|e| e.to_string())?;
    let latency_ms = started.elapsed().as_millis() as u64;

    // Post-flight audit
    state.log_event(
        chat_agent_id,
        EventType::LlmCall,
        json!({
            "event": "chat_stream",
            "model": model,
            "provider": "ollama",
            "response_length": result.len(),
            "latency_ms": latency_ms,
            "governance": "firewall+redaction",
        }),
    );

    Ok(result)
}

/// Save agent-to-model assignment in config.
pub(crate) fn set_agent_model(agent: String, model: String) -> Result<(), String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    let entry = config.agents.entry(agent).or_insert(AgentLlmConfig {
        model: String::new(),
        temperature: 0.7,
        max_tokens: 4096,
    });
    entry.model = model;
    save_nexus_config(&config).map_err(|e| e.to_string())
}

// ── LLM Provider Management ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderStatusEntry {
    pub name: String,
    pub available: bool,
    pub is_paid: bool,
    pub reason: String,
    pub latency_ms: Option<u64>,
    pub error_hint: Option<String>,
    pub setup_command: Option<String>,
    pub models_installed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatusResponse {
    pub active_provider: String,
    pub providers: Vec<LlmProviderStatusEntry>,
    pub governance_warning: Option<String>,
    pub has_any_provider: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendation {
    pub provider_type: String,
    pub display_name: String,
    pub reason: String,
    pub setup_command: Option<String>,
    pub cost_info: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendations {
    pub ram_mb: u64,
    pub gpu: String,
    pub can_run_local: bool,
    pub recommendations: Vec<LlmRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageStats {
    pub provider_name: String,
    pub total_queries: u64,
    pub total_tokens: u64,
    pub estimated_cost_dollars: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionResult {
    pub provider: String,
    pub success: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub model_used: Option<String>,
}

pub(crate) fn key_present(opt: &Option<String>) -> bool {
    opt.as_deref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
}

/// Smart Ollama status detection: diagnose connection refused, not installed, no models.
pub(crate) fn check_ollama_smart(url: &str) -> LlmProviderStatusEntry {
    let provider = OllamaProvider::new(url);
    let start = std::time::Instant::now();
    let health = provider.health_check();
    let latency = start.elapsed().as_millis() as u64;

    match health {
        Ok(true) => {
            // Connected! Check how many models are installed.
            let models = provider.list_models().unwrap_or_default();
            if models.is_empty() {
                // Ollama running but no models — detect system RAM for recommendation.
                let sys = sysinfo::System::new_all();
                let ram_mb = sys.total_memory() / (1024 * 1024);
                let (suggestion, cmd) = if ram_mb < 8_000 {
                    ("phi3:mini (2.7B, ~1.6GB)", "ollama pull phi3:mini")
                } else if ram_mb < 16_000 {
                    ("llama3:8b (8B, ~4.7GB)", "ollama pull llama3:8b")
                } else if ram_mb < 32_000 {
                    ("llama3:70b-q4 or mixtral:8x7b", "ollama pull mixtral:8x7b")
                } else {
                    ("llama3:70b", "ollama pull llama3:70b")
                };
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: format!(
                        "Ollama is running but has no models. Based on your system ({ram_mb} MB RAM), try: {suggestion}"
                    ),
                    latency_ms: Some(latency),
                    error_hint: Some("No models installed".to_string()),
                    setup_command: Some(cmd.to_string()),
                    models_installed: Some(0),
                }
            } else {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: true,
                    is_paid: false,
                    reason: format!(
                        "connected to {url} ({} model{})",
                        models.len(),
                        if models.len() == 1 { "" } else { "s" }
                    ),
                    latency_ms: Some(latency),
                    error_hint: None,
                    setup_command: None,
                    models_installed: Some(models.len() as u32),
                }
            }
        }
        _ => {
            // Not reachable. Detect whether Ollama binary exists.
            let ollama_installed = Command::new("which")
                .arg("ollama")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !ollama_installed {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: "Ollama not found on this system. Download it from https://ollama.com"
                        .to_string(),
                    latency_ms: None,
                    error_hint: Some("Not installed".to_string()),
                    setup_command: Some(
                        "curl -fsSL https://ollama.com/install.sh | sh".to_string(),
                    ),
                    models_installed: None,
                }
            } else {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: "Ollama is not running. Start it with: ollama serve".to_string(),
                    latency_ms: None,
                    error_hint: Some("Not running".to_string()),
                    setup_command: Some("ollama serve".to_string()),
                    models_installed: None,
                }
            }
        }
    }
}

pub(crate) fn cloud_provider_entry(
    name: &str,
    has_key: bool,
    cost_info: &str,
) -> LlmProviderStatusEntry {
    LlmProviderStatusEntry {
        name: name.to_string(),
        available: has_key,
        is_paid: true,
        reason: if has_key {
            "API key configured".to_string()
        } else {
            format!("no API key configured ({cost_info})")
        },
        latency_ms: None,
        error_hint: if has_key {
            None
        } else {
            Some("No API key".to_string())
        },
        setup_command: None,
        models_installed: None,
    }
}

/// Check which LLM providers are configured, reachable, and active.
pub(crate) fn check_llm_status() -> Result<LlmStatusResponse, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let prov_config = build_provider_config(&config);
    let active = select_provider(&prov_config).map_err(|e| e.to_string())?;
    let active_name = active.name().to_string();

    let mut providers = Vec::new();

    // Ollama — local, free, smart diagnostics
    let ollama_url = prov_config
        .ollama_url
        .as_deref()
        .unwrap_or("http://localhost:11434");
    providers.push(check_ollama_smart(ollama_url));

    // OpenAI
    providers.push(cloud_provider_entry(
        "openai",
        key_present(&prov_config.openai_api_key),
        "~$5/M tokens",
    ));

    // DeepSeek
    providers.push(cloud_provider_entry(
        "deepseek",
        key_present(&prov_config.deepseek_api_key),
        "~$0.14/M tokens, cheapest cloud option",
    ));

    // Gemini
    providers.push(cloud_provider_entry(
        "gemini",
        key_present(&prov_config.gemini_api_key),
        "~$3.50/M tokens",
    ));

    // Claude / Anthropic
    {
        let has_key = key_present(&prov_config.anthropic_api_key);
        let available = has_key;
        let reason = if !has_key {
            "no API key configured (~$3/M tokens)".to_string()
        } else {
            "API key configured".to_string()
        };
        providers.push(LlmProviderStatusEntry {
            name: "claude".to_string(),
            available,
            is_paid: true,
            reason,
            latency_ms: None,
            error_hint: if !has_key {
                Some("No API key".to_string())
            } else {
                None
            },
            setup_command: None,
            models_installed: None,
        });
    }

    // NVIDIA NIM
    providers.push(cloud_provider_entry(
        "nvidia",
        key_present(&prov_config.nvidia_api_key),
        "free 1000 credits, access frontier models via NIM",
    ));

    // Mock — always available
    providers.push(LlmProviderStatusEntry {
        name: "mock".to_string(),
        available: true,
        is_paid: false,
        reason: "built-in fallback".to_string(),
        latency_ms: None,
        error_hint: None,
        setup_command: None,
        models_installed: None,
    });

    let has_real = providers.iter().any(|p| p.available && p.name != "mock");

    // Governance warning: if no local provider, warn about cloud governance
    let governance_warning = if !providers.iter().any(|p| p.available && p.name == "ollama") {
        if has_real {
            Some(
                "Governance tasks are using cloud LLM. For maximum privacy, install a local model."
                    .to_string(),
            )
        } else {
            Some("Governance features limited. Configure an LLM provider in Settings.".to_string())
        }
    } else {
        None
    };

    Ok(LlmStatusResponse {
        active_provider: active_name,
        providers,
        governance_warning,
        has_any_provider: has_real,
    })
}

/// Get system-appropriate LLM recommendations.
pub(crate) fn get_llm_recommendations() -> Result<LlmRecommendations, String> {
    let sys = sysinfo::System::new_all();
    let ram_mb = sys.total_memory() / (1024 * 1024);

    // Try to detect GPU name from sysinfo cpus (basic heuristic)
    let gpu = "auto-detect".to_string();
    let can_run_local = ram_mb >= 8_000;

    let mut recs = Vec::new();

    // Local recommendations based on RAM
    if ram_mb < 8_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (phi3:mini)".to_string(),
            reason: format!("Your system has {ram_mb} MB RAM. phi3:mini is the lightest option."),
            setup_command: Some("ollama pull phi3:mini".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: false,
        });
    } else if ram_mb < 16_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (llama3:8b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. llama3:8b is a great balance of quality and speed."
            ),
            setup_command: Some("ollama pull llama3:8b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    } else if ram_mb < 32_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (mixtral:8x7b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. mixtral:8x7b offers excellent quality."
            ),
            setup_command: Some("ollama pull mixtral:8x7b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    } else {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (llama3:70b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. llama3:70b is the most capable local model."
            ),
            setup_command: Some("ollama pull llama3:70b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    }

    // Cloud recommendations — always show
    recs.push(LlmRecommendation {
        provider_type: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        reason: "Cheapest cloud option with strong coding performance.".to_string(),
        setup_command: None,
        cost_info: "~$0.14/M tokens".to_string(),
        recommended: !can_run_local,
    });

    recs.push(LlmRecommendation {
        provider_type: "openai".to_string(),
        display_name: "OpenAI (GPT-4o)".to_string(),
        reason: "Industry standard with broad capabilities.".to_string(),
        setup_command: None,
        cost_info: "~$5/M tokens".to_string(),
        recommended: false,
    });

    recs.push(LlmRecommendation {
        provider_type: "gemini".to_string(),
        display_name: "Google Gemini".to_string(),
        reason: "Strong multimodal capabilities and competitive pricing.".to_string(),
        setup_command: None,
        cost_info: "~$3.50/M tokens".to_string(),
        recommended: false,
    });

    recs.push(LlmRecommendation {
        provider_type: "claude".to_string(),
        display_name: "Anthropic Claude".to_string(),
        reason: "Best for reasoning and safety-conscious tasks.".to_string(),
        setup_command: None,
        cost_info: "~$3/M tokens".to_string(),
        recommended: false,
    });

    Ok(LlmRecommendations {
        ram_mb,
        gpu,
        can_run_local,
        recommendations: recs,
    })
}

/// Set the LLM provider assignment for a specific agent.
pub(crate) fn set_agent_llm_provider(
    agent_id: String,
    provider_id: String,
    local_only: bool,
    budget_dollars: u32,
    budget_tokens: u64,
) -> Result<(), String> {
    let mut config = load_config().map_err(agent_error)?;
    let assignment = nexus_kernel::config::AgentLlmAssignment {
        provider_id,
        local_only,
        budget_dollars,
        budget_tokens,
    };
    config.agent_llm_assignments.insert(agent_id, assignment);
    save_nexus_config(&config).map_err(agent_error)
}

/// Get provider usage stats (from audit trail oracle events).
pub(crate) fn get_provider_usage_stats(
    state: &AppState,
) -> Result<Vec<ProviderUsageStats>, String> {
    let audit = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let events = audit.events();

    // Aggregate by provider from LlmCall audit events
    let mut stats: HashMap<String, (u64, u64, f64)> = HashMap::new();
    for event in events {
        if event.event_type == EventType::LlmCall {
            let provider = event
                .payload
                .get("provider")
                .or_else(|| event.payload.get("model"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let tokens = event
                .payload
                .get("token_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cost = event
                .payload
                .get("cost")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let entry = stats.entry(provider).or_insert((0, 0, 0.0));
            entry.0 += 1;
            entry.1 += tokens;
            entry.2 += cost;
        }
    }

    let result = stats
        .into_iter()
        .map(|(name, (queries, tokens, cost))| ProviderUsageStats {
            provider_name: name,
            total_queries: queries,
            total_tokens: tokens,
            estimated_cost_dollars: cost,
        })
        .collect();

    Ok(result)
}

/// Test connection to a specific provider by sending a simple prompt.
pub(crate) fn test_llm_connection(provider_name: String) -> Result<TestConnectionResult, String> {
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);

    let mut test_config = prov_config.clone();
    test_config.provider = Some(provider_name.clone());
    let provider = select_provider(&test_config).map_err(|e| e.to_string())?;

    // Pick an appropriate test model for the provider
    let test_model = match provider_name.as_str() {
        "nvidia" => "meta/llama-3.1-8b-instruct".to_string(),
        "deepseek" => "deepseek-chat".to_string(),
        "gemini" => "gemini-2.5-flash".to_string(),
        "anthropic" | "claude" => "claude-sonnet-4-20250514".to_string(),
        "openai" => "gpt-4o-mini".to_string(),
        "openrouter" => "qwen/qwen3.6-plus:free".to_string(),
        _ => config.llm.default_model.clone(),
    };

    let start = std::time::Instant::now();
    let result = provider.query("Reply with exactly: ok", 10, &test_model);
    let latency = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => Ok(TestConnectionResult {
            provider: provider_name,
            success: true,
            latency_ms: latency,
            error: None,
            model_used: Some(response.model_name),
        }),
        Err(e) => Ok(TestConnectionResult {
            provider: provider_name,
            success: false,
            latency_ms: latency,
            error: Some(e.to_string()),
            model_used: None,
        }),
    }
}
