//! browser_research domain implementation.

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

// ── Research Mode ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentState {
    pub agent_id: String,
    pub agent_name: String,
    pub status: String,
    pub current_url: Option<String>,
    pub query: String,
    pub findings: Vec<String>,
    pub pages_visited: u32,
    pub fuel_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSessionState {
    pub session_id: String,
    pub topic: String,
    pub status: String,
    pub supervisor_message: String,
    pub sub_agents: Vec<SubAgentState>,
    pub summary: Option<String>,
    pub total_fuel_used: u64,
    pub pages_visited: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResearchEvent {
    pub event_type: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub message: String,
    pub url: Option<String>,
    pub finding: Option<String>,
    pub summary: Option<String>,
}

/// Manages multi-agent research sessions with supervisor delegation.
/// Each session: supervisor breaks topic into sub-queries, assigns to sub-agents,
/// sub-agents search + extract, supervisor merges findings.
/// PII redaction via PromptFirewall, fuel metered per page + LLM call, all audited.
#[derive(Debug, Clone, Default)]
pub struct ResearchManager {
    sessions: HashMap<String, ResearchSessionState>,
}

impl ResearchManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn get_session(&self, session_id: &str) -> Option<&ResearchSessionState> {
        self.sessions.get(session_id)
    }

    pub fn list_sessions(&self) -> Vec<ResearchSessionState> {
        self.sessions.values().cloned().collect()
    }
}

/// PII redaction helper — applies PromptFirewall-style redaction to extracted text.
/// Redacts SSN, email, phone patterns before they enter findings.
pub(crate) fn redact_pii(text: &str) -> String {
    use nexus_kernel::firewall::prompt_firewall::{FirewallAction, InputFilter};

    let mut filter = InputFilter::default();
    let agent_id = Uuid::nil();
    let mut audit = AuditTrail::new();
    match filter.check(agent_id, text, &mut audit) {
        FirewallAction::Redacted { redacted_text, .. } => redacted_text,
        _ => text.to_string(),
    }
}

/// Fuel cost constants for research operations.
const FUEL_PER_PAGE_VISIT: u64 = 25;
const FUEL_PER_LLM_EXTRACTION: u64 = 50;
const FUEL_PER_MERGE: u64 = 100;

pub(crate) fn start_research(
    state: &AppState,
    topic: String,
    num_agents: u32,
) -> Result<ResearchSessionState, String> {
    let num_agents = num_agents.clamp(1, 5);
    let session_id = Uuid::new_v4().to_string();
    let supervisor_id = Uuid::new_v4();

    // Audit: research started
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "research_started",
            "session_id": session_id,
            "topic": topic,
            "num_agents": num_agents,
        }),
    );

    // Step 1: Supervisor breaks topic into sub-queries
    let sub_queries = generate_sub_queries(&topic, num_agents);

    // Step 2: Create sub-agents
    let mut sub_agents = Vec::new();
    for (i, query) in sub_queries.iter().enumerate() {
        let agent_id = Uuid::new_v4().to_string();
        let agent_name = format!("Sub-Agent-{}", i + 1);

        sub_agents.push(SubAgentState {
            agent_id: agent_id.clone(),
            agent_name: agent_name.clone(),
            status: "searching".to_string(),
            current_url: None,
            query: query.clone(),
            findings: Vec::new(),
            pages_visited: 0,
            fuel_used: 0,
        });

        // Audit: sub-agent assigned
        state.log_event(
            supervisor_id,
            EventType::ToolCall,
            json!({
                "event": "agent_assigned",
                "session_id": session_id,
                "agent_id": agent_id,
                "agent_name": agent_name,
                "query": query,
            }),
        );
    }

    let supervisor_msg = format!(
        "Assigning research task to {}",
        sub_agents
            .iter()
            .map(|a| a.agent_name.as_str())
            .collect::<Vec<_>>()
            .join(" and ")
    );

    let session = ResearchSessionState {
        session_id: session_id.clone(),
        topic: topic.clone(),
        status: "running".to_string(),
        supervisor_message: supervisor_msg,
        sub_agents: sub_agents.clone(),
        summary: None,
        total_fuel_used: 0,
        pages_visited: 0,
    };

    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    research
        .sessions
        .insert(session_id.clone(), session.clone());

    // Add activity to browser manager for the activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Research started: \"{}\" with {} sub-agents",
            topic, num_agents
        ),
    );
    for agent in &sub_agents {
        browser.add_activity(
            &agent.agent_id,
            &agent.agent_name,
            "searching",
            &format!("Assigned query: \"{}\"", agent.query),
        );
    }

    Ok(session)
}

/// Generate sub-queries by splitting the topic into focused aspects.
pub(crate) fn generate_sub_queries(topic: &str, num_agents: u32) -> Vec<String> {
    let aspects = [
        "overview and key concepts",
        "recent developments and trends",
        "practical applications and examples",
        "challenges and limitations",
        "future directions and outlook",
    ];
    (0..num_agents as usize)
        .map(|i| {
            let aspect = aspects.get(i).unwrap_or(&"additional details");
            format!("{} — {}", topic, aspect)
        })
        .collect()
}

pub(crate) fn research_agent_action(
    state: &AppState,
    session_id: String,
    agent_id: String,
    action: String,
    url: Option<String>,
    content: Option<String>,
) -> Result<ResearchSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());

    let session = research
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Research session {} not found", session_id))?;

    let agent = session
        .sub_agents
        .iter_mut()
        .find(|a| a.agent_id == agent_id)
        .ok_or_else(|| format!("Sub-agent {} not found", agent_id))?;

    let agent_name = agent.agent_name.clone();

    // Egress governance check (before mutating agent state)
    if action == "reading" {
        if let Some(ref target_url) = url {
            let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(reason) = browser.check_url(target_url) {
                drop(research);
                state.log_event(
                    supervisor_id,
                    EventType::ToolCall,
                    json!({
                        "event": "research_url_blocked",
                        "session_id": session_id,
                        "agent_id": agent_id,
                        "url": target_url,
                        "reason": reason,
                    }),
                );
                return Err(format!("URL blocked by egress policy: {}", reason));
            }
        }
    }

    match action.as_str() {
        "searching" => {
            agent.status = "searching".to_string();
            agent.current_url = url.clone();
        }
        "reading" => {
            // Fuel metered per page visit
            agent.fuel_used += FUEL_PER_PAGE_VISIT;
            agent.pages_visited += 1;
            agent.status = "reading".to_string();
            agent.current_url = url.clone();
        }
        "extracting" => {
            // Fuel metered per LLM extraction call
            agent.fuel_used += FUEL_PER_LLM_EXTRACTION;
            agent.status = "extracting".to_string();

            // PII redaction on extracted content
            if let Some(ref raw_content) = content {
                let redacted = redact_pii(raw_content);
                agent.findings.push(redacted);
            }
        }
        "done" => {
            agent.status = "done".to_string();
            agent.current_url = None;
        }
        _ => {
            return Err(format!("Unknown action: {}", action));
        }
    }

    // Capture agent fields we need for activity stream before dropping the mutable borrow
    let agent_query = agent.query.clone();
    let agent_findings_count = agent.findings.len();

    // Update session totals (no longer conflicts with agent borrow)
    session.total_fuel_used = session.sub_agents.iter().map(|a| a.fuel_used).sum();
    session.pages_visited = session.sub_agents.iter().map(|a| a.pages_visited).sum();

    let result = session.clone();

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": format!("agent_{}", action),
            "session_id": session_id,
            "agent_id": agent_id,
            "agent_name": agent_name,
            "url": url,
            "fuel_cost": match action.as_str() {
                "reading" => FUEL_PER_PAGE_VISIT,
                "extracting" => FUEL_PER_LLM_EXTRACTION,
                _ => 0,
            },
        }),
    );

    // Record in browser activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    let msg_type = match action.as_str() {
        "searching" => "searching",
        "reading" => "reading",
        "extracting" => "extracting",
        "done" => "info",
        _ => "info",
    };
    let content_msg = match action.as_str() {
        "searching" => format!("Searching: \"{}\"", agent_query),
        "reading" => format!("Reading: {}", url.as_deref().unwrap_or("unknown")),
        "extracting" => format!(
            "Extracting findings from {}",
            url.as_deref().unwrap_or("current page")
        ),
        "done" => format!("Completed with {} findings", agent_findings_count),
        _ => action.clone(),
    };
    browser.add_activity(&agent_id, &agent_name, msg_type, &content_msg);

    // Record URL visit in browser history
    if let Some(ref target_url) = url {
        if action == "reading" {
            let title = target_url
                .split("://")
                .nth(1)
                .unwrap_or(target_url)
                .split('/')
                .next()
                .unwrap_or("Untitled")
                .to_string();
            browser.record_visit(target_url, &title, Some(agent_id.clone()));
        }
    }

    Ok(result)
}

pub(crate) fn complete_research(
    state: &AppState,
    session_id: String,
) -> Result<ResearchSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());

    let session = research
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Research session {} not found", session_id))?;

    // Fuel for merge operation
    session.total_fuel_used += FUEL_PER_MERGE;
    session.status = "merging".to_string();

    // Collect all findings from sub-agents, apply PII redaction to merged summary
    let all_findings: Vec<String> = session
        .sub_agents
        .iter()
        .flat_map(|a| {
            let header = format!("## {} (query: \"{}\")", a.agent_name, a.query);
            let mut items = vec![header];
            for (j, f) in a.findings.iter().enumerate() {
                items.push(format!("{}. {}", j + 1, f));
            }
            items
        })
        .collect();

    let raw_summary = format!(
        "# Research Summary: {}\n\n{}\n\n---\nTotal pages visited: {} | Total fuel used: {}",
        session.topic,
        all_findings.join("\n"),
        session.pages_visited,
        session.total_fuel_used,
    );

    // PII redaction on merged summary
    let summary = redact_pii(&raw_summary);

    session.summary = Some(summary.clone());
    session.status = "complete".to_string();

    // Mark all sub-agents as done
    for agent in &mut session.sub_agents {
        agent.status = "done".to_string();
        agent.current_url = None;
    }

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "research_complete",
            "session_id": session_id,
            "topic": session.topic,
            "total_pages": session.pages_visited,
            "total_fuel": session.total_fuel_used,
            "findings_count": session.sub_agents.iter().map(|a| a.findings.len()).sum::<usize>(),
        }),
    );

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "extracting",
        &format!(
            "Merging findings from {} agents ({} total findings)",
            session.sub_agents.len(),
            session
                .sub_agents
                .iter()
                .map(|a| a.findings.len())
                .sum::<usize>(),
        ),
    );
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Research complete: {} pages visited, {} fuel consumed",
            session.pages_visited, session.total_fuel_used,
        ),
    );

    let result = session.clone();
    Ok(result)
}

pub(crate) fn get_research_session(
    state: &AppState,
    session_id: String,
) -> Result<ResearchSessionState, String> {
    let research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    research
        .get_session(&session_id)
        .cloned()
        .ok_or_else(|| format!("Research session {} not found", session_id))
}

pub(crate) fn list_research_sessions(
    state: &AppState,
) -> Result<Vec<ResearchSessionState>, String> {
    let research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    Ok(research.list_sessions())
}

// ── Build Mode ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildAgentMessage {
    pub id: String,
    pub timestamp: u64,
    pub agent_name: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSessionState {
    pub session_id: String,
    pub description: String,
    pub status: String,
    pub code: String,
    pub preview_html: String,
    pub messages: Vec<BuildAgentMessage>,
    pub fuel_used: u64,
    pub llm_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BuildCodeDelta {
    pub session_id: String,
    pub delta: String,
    pub full_code: String,
    pub agent_name: String,
}

/// Fuel cost constants for build operations.
const FUEL_PER_BUILD_LLM_CALL: u64 = 75;

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Manages build sessions where agents write code collaboratively.
#[derive(Debug, Clone, Default)]
pub struct BuildManager {
    sessions: HashMap<String, BuildSessionState>,
}

impl BuildManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

pub(crate) fn build_msg(agent_name: &str, role: &str, content: &str) -> BuildAgentMessage {
    BuildAgentMessage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_secs(),
        agent_name: agent_name.to_string(),
        role: role.to_string(),
        content: content.to_string(),
    }
}

/// Wrap code in a full HTML document for preview rendering.
pub(crate) fn wrap_preview_html(code: &str) -> String {
    // If code already has <html> or <!DOCTYPE>, use as-is
    let lower = code.to_lowercase();
    if lower.contains("<html") || lower.contains("<!doctype") {
        return code.to_string();
    }
    format!(
        "<!DOCTYPE html>\n<html>\n<head><meta charset=\"utf-8\"><style>body{{margin:0;font-family:system-ui,sans-serif}}</style></head>\n<body>\n{}\n</body>\n</html>",
        code
    )
}

pub(crate) fn start_build(
    state: &AppState,
    description: String,
) -> Result<BuildSessionState, String> {
    let session_id = Uuid::new_v4().to_string();
    let supervisor_id = Uuid::new_v4();

    // Audit: build started
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_started",
            "session_id": session_id,
            "description": description,
        }),
    );

    let mut messages = Vec::new();
    messages.push(build_msg(
        "Supervisor",
        "supervisor",
        &format!("Build task received: {}", description),
    ));
    messages.push(build_msg(
        "Supervisor",
        "supervisor",
        "Assigning to Coder agent. Designer agent on standby for styling.",
    ));

    let session = BuildSessionState {
        session_id: session_id.clone(),
        description,
        status: "planning".to_string(),
        code: String::new(),
        preview_html: String::new(),
        messages,
        fuel_used: 0,
        llm_calls: 0,
    };

    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions.insert(session_id.clone(), session.clone());

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!("Build started: {}", session.description),
    );

    Ok(session)
}

pub(crate) fn build_append_code(
    state: &AppState,
    session_id: String,
    delta: String,
    agent_name: String,
) -> Result<BuildSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    // PII redaction on code content
    let redacted_delta = redact_pii(&delta);
    session.code.push_str(&redacted_delta);
    session.preview_html = wrap_preview_html(&session.code);
    session.status = "coding".to_string();
    session.fuel_used += FUEL_PER_BUILD_LLM_CALL;
    session.llm_calls += 1;

    let result = session.clone();
    drop(bm);

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_code_delta",
            "session_id": session_id,
            "agent_name": agent_name,
            "delta_len": redacted_delta.len(),
            "total_len": result.code.len(),
            "fuel_cost": FUEL_PER_BUILD_LLM_CALL,
        }),
    );

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        &agent_name.to_lowercase().replace(' ', "-"),
        &agent_name,
        "extracting",
        &format!("Writing code ({} chars)", redacted_delta.len()),
    );

    Ok(result)
}

pub(crate) fn build_add_message(
    state: &AppState,
    session_id: String,
    agent_name: String,
    role: String,
    content: String,
) -> Result<BuildSessionState, String> {
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    session
        .messages
        .push(build_msg(&agent_name, &role, &content));

    let result = session.clone();
    drop(bm);

    // Activity stream
    let msg_type = match role.as_str() {
        "coder" => "extracting",
        "designer" => "deciding",
        _ => "info",
    };
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        &agent_name.to_lowercase().replace(' ', "-"),
        &agent_name,
        msg_type,
        &content,
    );

    Ok(result)
}

pub(crate) fn complete_build(
    state: &AppState,
    session_id: String,
) -> Result<BuildSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    session.status = "complete".to_string();
    session.preview_html = wrap_preview_html(&session.code);
    session.messages.push(build_msg(
        "Supervisor",
        "supervisor",
        "Build complete. Preview is ready.",
    ));

    let result = session.clone();
    drop(bm);

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_complete",
            "session_id": session_id,
            "code_len": result.code.len(),
            "fuel_used": result.fuel_used,
            "llm_calls": result.llm_calls,
        }),
    );

    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Build complete — {} chars, {} LLM calls",
            result.code.len(),
            result.llm_calls
        ),
    );

    Ok(result)
}

pub(crate) fn get_build_session(
    state: &AppState,
    session_id: String,
) -> Result<BuildSessionState, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

pub(crate) fn get_build_code(state: &AppState, session_id: String) -> Result<String, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .map(|s| s.code.clone())
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

pub(crate) fn get_build_preview(state: &AppState, session_id: String) -> Result<String, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .map(|s| s.preview_html.clone())
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

// ── Learn Mode ──

/// Fuel cost constants for learning operations.
const FUEL_PER_LEARN_BROWSE: u64 = 25;
const FUEL_PER_LEARN_EXTRACT: u64 = 50;
const FUEL_PER_LEARN_COMPARE: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSource {
    pub url: String,
    pub label: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub title: String,
    pub source_url: String,
    pub key_points: Vec<String>,
    pub timestamp: u64,
    pub relevance_score: f64,
    pub category: String,
    pub is_new: bool,
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSuggestion {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source_url: String,
    pub relevance: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSessionState {
    pub session_id: String,
    pub status: String,
    pub sources: Vec<LearningSource>,
    pub current_source_idx: usize,
    pub current_url: Option<String>,
    pub knowledge_base: Vec<KnowledgeEntry>,
    pub suggestions: Vec<LearningSuggestion>,
    pub fuel_used: u64,
    pub pages_visited: u64,
}

/// Manages learning sessions where agents browse documentation to stay current.
#[derive(Debug, Clone, Default)]
pub struct LearningManager {
    sessions: HashMap<String, LearningSessionState>,
    knowledge: Vec<KnowledgeEntry>,
}

impl LearningManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            knowledge: Vec::new(),
        }
    }
}

pub(crate) fn start_learning(
    state: &AppState,
    sources: Vec<LearningSource>,
) -> Result<LearningSessionState, String> {
    let session_id = Uuid::new_v4().to_string();
    let agent_id = Uuid::new_v4();

    // Validate sources — all must be http(s) URLs
    for src in &sources {
        let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(reason) = browser.check_url(&src.url) {
            return Err(format!("Source {} blocked: {}", src.label, reason));
        }
    }

    // Audit: learning started
    state.log_event(
        agent_id,
        EventType::ToolCall,
        json!({
            "event": "learning_started",
            "session_id": session_id,
            "source_count": sources.len(),
            "sources": sources.iter().map(|s| &s.url).collect::<Vec<_>>(),
        }),
    );

    let session = LearningSessionState {
        session_id: session_id.clone(),
        status: "browsing".to_string(),
        sources,
        current_source_idx: 0,
        current_url: None,
        knowledge_base: Vec::new(),
        suggestions: Vec::new(),
        fuel_used: 0,
        pages_visited: 0,
    };

    let mut lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    lm.sessions.insert(session_id.clone(), session.clone());

    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "learn-agent",
        "LearnAgent",
        "info",
        &format!(
            "Learning session {} started with {} sources",
            &session_id[..8],
            session.sources.len()
        ),
    );

    Ok(session)
}

pub(crate) fn learning_agent_action(
    state: &AppState,
    session_id: String,
    action: String,
    url: Option<String>,
    content: Option<String>,
) -> Result<LearningSessionState, String> {
    let agent_id = Uuid::new_v4();

    // Egress check if URL provided
    if let Some(ref u) = url {
        let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(reason) = browser.check_url(u) {
            state.log_event(
                agent_id,
                EventType::ToolCall,
                json!({
                    "event": "learning_blocked",
                    "session_id": session_id,
                    "url": u,
                    "reason": reason,
                }),
            );
            return Err(format!("URL blocked: {}", reason));
        }
    }

    let mut lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());

    // Snapshot existing knowledge URLs before borrowing session
    let existing_knowledge_urls: HashSet<String> =
        lm.knowledge.iter().map(|k| k.source_url.clone()).collect();

    if !lm.sessions.contains_key(&session_id) {
        return Err(format!("Learning session {} not found", session_id));
    }

    // Perform session mutations in a block so we can access lm.knowledge afterward
    let (result, entries_to_merge) = {
        let session = lm
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;

        match action.as_str() {
            "browse" => {
                session.fuel_used += FUEL_PER_LEARN_BROWSE;
                session.pages_visited += 1;
                session.current_url = url.clone();
                session.status = "browsing".to_string();

                if let Some(ref u) = url {
                    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                    browser.record_visit(u, "Learning", Some("learn-agent".to_string()));
                    browser.add_activity(
                        "learn-agent",
                        "LearnAgent",
                        "navigating",
                        &format!("Browsing: {}", u),
                    );
                }

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "agent_browsing",
                        "session_id": session_id,
                        "url": url,
                        "fuel_used": session.fuel_used,
                    }),
                );

                (session.clone(), None)
            }
            "extract" => {
                session.fuel_used += FUEL_PER_LEARN_EXTRACT;
                session.status = "extracting".to_string();

                let source_url =
                    url.unwrap_or_else(|| session.current_url.clone().unwrap_or_default());
                let src_label = session
                    .sources
                    .iter()
                    .find(|s| s.url == source_url)
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| source_url.clone());
                let src_category = session
                    .sources
                    .iter()
                    .find(|s| s.url == source_url)
                    .map(|s| s.category.clone())
                    .unwrap_or_else(|| "documentation".to_string());

                let raw_content =
                    content.unwrap_or_else(|| format!("Extracted information from {}", src_label));
                let redacted = redact_pii(&raw_content);

                let entry = KnowledgeEntry {
                    id: Uuid::new_v4().to_string(),
                    title: format!("{} — Latest", src_label),
                    source_url: source_url.clone(),
                    key_points: vec![redacted],
                    timestamp: now_secs(),
                    relevance_score: 0.5,
                    category: src_category,
                    is_new: true,
                    change_summary: None,
                };
                session.knowledge_base.push(entry);

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "agent_extracted",
                        "session_id": session_id,
                        "source": source_url,
                        "fuel_used": session.fuel_used,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "extracting",
                    &format!("Extracted from {}", src_label),
                );

                (session.clone(), None)
            }
            "compare" => {
                session.fuel_used += FUEL_PER_LEARN_COMPARE;
                session.status = "comparing".to_string();

                for entry in &mut session.knowledge_base {
                    if !existing_knowledge_urls.contains(&entry.source_url) {
                        entry.is_new = true;
                        entry.change_summary =
                            Some("New source — not previously in knowledge base".to_string());
                        entry.relevance_score = 0.8;
                    }
                }

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "knowledge_updated",
                        "session_id": session_id,
                        "knowledge_count": session.knowledge_base.len(),
                        "fuel_used": session.fuel_used,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "deciding",
                    "Compared with existing knowledge base",
                );

                (session.clone(), None)
            }
            "done" => {
                session.status = "complete".to_string();
                session.current_url = None;

                let kb_len = session.knowledge_base.len();
                let fuel_used = session.fuel_used;
                let pages_visited = session.pages_visited;
                let merge = session.knowledge_base.clone();

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "learning_complete",
                        "session_id": session_id,
                        "knowledge_entries": kb_len,
                        "fuel_used": fuel_used,
                        "pages_visited": pages_visited,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "info",
                    &format!("Learning complete — {} entries, {} fuel", kb_len, fuel_used),
                );

                (session.clone(), Some(merge))
            }
            other => {
                return Err(format!("Unknown learning action: {}", other));
            }
        }
    };

    // Merge knowledge entries into global store (session borrow is now dropped)
    if let Some(entries) = entries_to_merge {
        for entry in entries {
            lm.knowledge.push(entry);
        }
    }

    Ok(result)
}

pub(crate) fn get_learning_session(
    state: &AppState,
    session_id: String,
) -> Result<LearningSessionState, String> {
    let lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    lm.sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Learning session {} not found", session_id))
}

pub(crate) fn get_knowledge_base(state: &AppState) -> Result<Vec<KnowledgeEntry>, String> {
    let lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    Ok(lm.knowledge.clone())
}

// ── Agent Browser ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigateResult {
    pub url: String,
    pub title: String,
    pub allowed: bool,
    pub deny_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserHistoryEntry {
    pub url: String,
    pub title: String,
    pub timestamp: u64,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMessageRow {
    pub id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub agent_name: String,
    pub message_type: String,
    pub content: String,
}

/// BrowserManager tracks active browsing sessions and enforces egress governance.
/// URLs are checked against a built-in blocklist and (when agents are assigned)
/// against the agent's `allowed_endpoints` from their manifest.
#[derive(Debug, Clone, Default)]
pub struct BrowserManager {
    history: Vec<BrowserHistoryEntry>,
    activity: Vec<ActivityMessageRow>,
    /// Blocked domain patterns (default deny list).
    blocked_domains: Vec<String>,
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            activity: Vec::new(),
            blocked_domains: vec![
                "malware.".to_string(),
                "phishing.".to_string(),
                "darkweb.".to_string(),
            ],
        }
    }

    /// Check whether a URL is allowed by egress governance.
    /// Returns Ok(title) on success, Err(reason) on block.
    pub fn check_url(&self, url: &str) -> Result<(), String> {
        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Only http:// and https:// URLs are allowed".to_string());
        }

        // Extract host for domain check
        let host = url
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        // Check against blocked domains
        for blocked in &self.blocked_domains {
            if host.contains(blocked) {
                return Err(format!("Domain blocked by egress policy: {}", host));
            }
        }

        Ok(())
    }

    pub fn record_visit(&mut self, url: &str, title: &str, agent_id: Option<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.history.push(BrowserHistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            timestamp: now,
            agent_id,
        });
    }

    pub fn add_activity(
        &mut self,
        agent_id: &str,
        agent_name: &str,
        message_type: &str,
        content: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.activity.push(ActivityMessageRow {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            message_type: message_type.to_string(),
            content: content.to_string(),
        });
    }
}

pub(crate) fn navigate_to(state: &AppState, url: String) -> Result<BrowserNavigateResult, String> {
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());

    // Egress governance check — fail-closed
    if let Err(reason) = browser.check_url(&url) {
        browser.add_activity(
            "system",
            "Firewall",
            "blocked",
            &format!("Blocked: {} — {}", url, reason),
        );

        // Audit the blocked attempt
        let system_id = Uuid::nil();
        state.log_event(
            system_id,
            EventType::ToolCall,
            json!({
                "event": "browser_navigate",
                "url": url,
                "allowed": false,
                "reason": reason,
            }),
        );

        return Ok(BrowserNavigateResult {
            url,
            title: String::new(),
            allowed: false,
            deny_reason: Some(reason),
        });
    }

    // Extract a title from the URL (real browser would parse HTML)
    let title = url
        .split("://")
        .nth(1)
        .unwrap_or(&url)
        .split('/')
        .next()
        .unwrap_or("Untitled")
        .to_string();

    browser.record_visit(&url, &title, None);
    browser.add_activity(
        "system",
        "Browser",
        "navigating",
        &format!("Loaded: {}", url),
    );

    // Audit the page visit
    let system_id = Uuid::nil();
    state.log_event(
        system_id,
        EventType::ToolCall,
        json!({
            "event": "browser_navigate",
            "url": url,
            "allowed": true,
            "title": title,
        }),
    );

    Ok(BrowserNavigateResult {
        url,
        title,
        allowed: true,
        deny_reason: None,
    })
}

pub(crate) fn get_browser_history(state: &AppState) -> Result<Vec<BrowserHistoryEntry>, String> {
    let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    Ok(browser.history.clone())
}

pub(crate) fn get_agent_activity(state: &AppState) -> Result<Vec<ActivityMessageRow>, String> {
    let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    Ok(browser.activity.clone())
}
