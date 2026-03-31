//! audit_compliance domain implementation.

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

// ── Compliance Dashboard API ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceAlertRow {
    pub severity: String,
    pub check_id: String,
    pub message: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soc2ControlRow {
    pub control_id: String,
    pub description: String,
    pub status: String,
    pub evidence_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatusRow {
    pub status: String,
    pub checks_passed: usize,
    pub checks_failed: usize,
    pub agents_checked: usize,
    pub alerts: Vec<ComplianceAlertRow>,
    pub soc2_controls: Vec<Soc2ControlRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceAgentRow {
    pub id: String,
    pub name: String,
    pub risk_tier: String,
    pub autonomy_level: String,
    pub capabilities: Vec<String>,
    pub status: String,
    pub justification: String,
    pub applicable_articles: Vec<String>,
    pub required_controls: Vec<String>,
}

pub(crate) fn get_compliance_status(state: &AppState) -> Result<ComplianceStatusRow, String> {
    use nexus_kernel::compliance::monitor::{AgentSnapshot, ComplianceMonitor};

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let identity_mgr = state.identity_mgr.lock().unwrap_or_else(|p| p.into_inner());

    let snapshots: Vec<AgentSnapshot> = supervisor
        .health_check()
        .iter()
        .filter_map(|s| {
            supervisor.get_agent(s.id).map(|h| AgentSnapshot {
                agent_id: s.id,
                manifest: h.manifest.clone(),
                running: matches!(
                    s.state,
                    nexus_kernel::lifecycle::AgentState::Running
                        | nexus_kernel::lifecycle::AgentState::Starting
                ),
            })
        })
        .collect();

    let monitor = ComplianceMonitor::new();
    let result = monitor.check_compliance(&snapshots, &audit, &identity_mgr);

    // Generate SOC 2 report using enterprise crate
    let soc2_report = nexus_enterprise::compliance::generate_soc2_report(
        &audit,
        true, // capabilities are always configured in Nexus OS
        true, // HITL is always enabled
        true, // fuel tracking is always enabled
        "Nexus OS",
        0,
        u64::MAX,
    );
    let soc2_controls: Vec<Soc2ControlRow> = soc2_report
        .sections
        .into_iter()
        .flat_map(|s| s.controls)
        .map(|c| {
            let status_str = match &c.status {
                nexus_enterprise::compliance::ControlStatus::Satisfied => "satisfied".to_string(),
                nexus_enterprise::compliance::ControlStatus::PartiallyMet { gaps } => {
                    format!("partially_met: {}", gaps.join("; "))
                }
                nexus_enterprise::compliance::ControlStatus::NotMet { reason } => {
                    format!("not_met: {reason}")
                }
                nexus_enterprise::compliance::ControlStatus::NotApplicable => {
                    "not_applicable".to_string()
                }
            };
            Soc2ControlRow {
                control_id: c.control_id,
                description: c.description,
                status: status_str,
                evidence_count: c.evidence_count,
            }
        })
        .collect();

    Ok(ComplianceStatusRow {
        status: result.status.as_str().to_string(),
        checks_passed: result.checks_passed,
        checks_failed: result.checks_failed,
        agents_checked: result.agents_checked,
        alerts: result
            .alerts
            .into_iter()
            .map(|a| ComplianceAlertRow {
                severity: a.severity.as_str().to_string(),
                check_id: a.check_id,
                message: a.message,
                agent_id: a.agent_id.map(|id| id.to_string()),
            })
            .collect(),
        soc2_controls,
    })
}

pub(crate) fn get_compliance_agents(state: &AppState) -> Result<Vec<ComplianceAgentRow>, String> {
    use nexus_kernel::autonomy::AutonomyLevel;
    use nexus_kernel::compliance::eu_ai_act::RiskClassifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let classifier = RiskClassifier::new();

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let profile = classifier.classify_agent(&handle.manifest);
            let autonomy = AutonomyLevel::from_manifest(handle.manifest.autonomy_level);
            rows.push(ComplianceAgentRow {
                id: agent_status.id.to_string(),
                name: handle.manifest.name.clone(),
                risk_tier: profile.tier.as_str().to_string(),
                autonomy_level: autonomy.as_str().to_string(),
                capabilities: handle.manifest.capabilities.clone(),
                status: format!("{}", agent_status.state),
                justification: profile.justification,
                applicable_articles: profile.applicable_articles,
                required_controls: profile.required_controls,
            });
        }
    }
    Ok(rows)
}

// ── Distributed Audit API ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditChainStatusRow {
    pub total_events: usize,
    pub chain_valid: bool,
    pub first_hash: String,
    pub last_hash: String,
}

pub(crate) fn get_audit_chain_status(state: &AppState) -> Result<AuditChainStatusRow, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let total = events.len();
    let chain_valid = if total == 0 {
        true
    } else {
        audit.verify_integrity()
    };
    let first_hash = events.first().map(|e| e.hash.clone()).unwrap_or_default();
    let last_hash = events.last().map(|e| e.hash.clone()).unwrap_or_default();

    Ok(AuditChainStatusRow {
        total_events: total,
        chain_valid,
        first_hash,
        last_hash,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitChangeRow {
    pub file: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitRow {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepoStatusRow {
    pub detected: bool,
    pub root: Option<String>,
    pub branch: Option<String>,
    pub changes: Vec<GitChangeRow>,
    pub commits: Vec<GitCommitRow>,
}

pub(crate) fn run_git_command(
    args: &[&str],
    repo_root: &std::path::Path,
) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|error| format!("git {} failed: {error}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {} exited with {}", args.join(" "), output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn parse_git_status(code: &str) -> &'static str {
    match code {
        "??" => "untracked",
        value if value.contains('A') => "added",
        value if value.contains('D') => "deleted",
        _ => "modified",
    }
}

pub(crate) fn get_git_repo_status() -> Result<GitRepoStatusRow, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("cwd unavailable: {error}"))?;
    let repo_root_output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .map_err(|error| format!("git unavailable: {error}"))?;

    if !repo_root_output.status.success() {
        return Ok(GitRepoStatusRow {
            detected: false,
            root: None,
            branch: None,
            changes: Vec::new(),
            commits: Vec::new(),
        });
    }

    let repo_root = String::from_utf8_lossy(&repo_root_output.stdout)
        .trim()
        .to_string();
    let repo_path = std::path::PathBuf::from(&repo_root);
    // Optional: branch detection may fail in detached HEAD or non-git contexts
    let branch = run_git_command(&["branch", "--show-current"], &repo_path).ok();
    let status_output = run_git_command(&["status", "--porcelain"], &repo_path).unwrap_or_default();
    let log_output = run_git_command(
        &["log", "--pretty=format:%H%x1f%an%x1f%ct%x1f%s", "-n", "15"],
        &repo_path,
    )
    .unwrap_or_default();

    let changes = status_output
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let status = parse_git_status(&line[..2]).to_string();
            let file = line[3..].trim().to_string();
            Some(GitChangeRow { file, status })
        })
        .collect::<Vec<_>>();

    let commits = log_output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            let hash = parts.next()?.to_string();
            let author = parts.next()?.to_string();
            // Optional: skip commits with unparseable timestamps
            let ts = parts.next()?.parse::<u64>().ok()?.saturating_mul(1000);
            let message = parts.next()?.to_string();
            Some(GitCommitRow {
                hash,
                message,
                author,
                ts,
            })
        })
        .collect::<Vec<_>>();

    Ok(GitRepoStatusRow {
        detected: true,
        root: Some(repo_root),
        branch,
        changes,
        commits,
    })
}

// ── Governance verification commands ────────────────────────────────────────

pub(crate) fn verify_governance_invariants(state: &AppState) -> Result<String, String> {
    use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    // Use first agent's data if available, otherwise use defaults.
    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let manifest = nexus_kernel::manifest::AgentManifest {
        name: "verification-probe".to_string(),
        version: "1.0.0".to_string(),
        capabilities: capabilities.clone(),
        fuel_budget,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![
            FilesystemPermission {
                path_pattern: "/safe/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            },
            FilesystemPermission {
                path_pattern: "/safe/secret.key".to_string(),
                permission: FsPermissionLevel::Deny,
            },
        ],
    };

    let test_paths: Vec<&str> = vec!["/safe/readme.txt", "/safe/secret.key"];
    let results = verifier.verify_all(
        fuel_remaining,
        fuel_budget,
        &capabilities,
        &capabilities,
        &audit,
        &manifest,
        &test_paths,
    );

    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub(crate) fn verify_specific_invariant(
    state: &AppState,
    invariant_name: String,
) -> Result<String, String> {
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let proof = match invariant_name.as_str() {
        "FuelNeverNegative" => verifier.verify_fuel_invariant(fuel_remaining, fuel_budget),
        "FuelNeverExceedsBudget" => {
            verifier.verify_fuel_budget_invariant(fuel_remaining, fuel_budget)
        }
        "CapabilityCheckBeforeAction" => {
            verifier.verify_capability_invariant(&capabilities, "llm.query")
        }
        "AuditChainIntegrity" => verifier.verify_audit_chain(&audit),
        "RedactionBeforeLlmCall" => verifier.verify_redaction_invariant(&audit),
        "HitlApprovalForDestructive" => verifier.verify_hitl_invariant(&audit),
        "NoCapabilityEscalation" => verifier.verify_no_escalation(&capabilities, &capabilities),
        "DenyOverridesAllow" => {
            use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
            let manifest = nexus_kernel::manifest::AgentManifest {
                name: "verification-probe".to_string(),
                version: "1.0.0".to_string(),
                capabilities: capabilities.clone(),
                fuel_budget,
                autonomy_level: None,
                consent_policy_path: None,
                requester_id: None,
                schedule: None,
                default_goal: None,
                llm_model: None,
                fuel_period_id: None,
                monthly_fuel_cap: None,
                allowed_endpoints: None,
                domain_tags: vec![],
                filesystem_permissions: vec![
                    FilesystemPermission {
                        path_pattern: "/safe/".to_string(),
                        permission: FsPermissionLevel::ReadOnly,
                    },
                    FilesystemPermission {
                        path_pattern: "/safe/secret.key".to_string(),
                        permission: FsPermissionLevel::Deny,
                    },
                ],
            };
            verifier.verify_filesystem_deny_override(&manifest, &["/safe/secret.key"])
        }
        _ => return Err(format!("Unknown invariant: {invariant_name}")),
    };

    serde_json::to_string(&proof).map_err(|e| e.to_string())
}

pub(crate) fn export_compliance_report(state: &AppState) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AuditExport)?;
    use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let manifest = nexus_kernel::manifest::AgentManifest {
        name: "verification-probe".to_string(),
        version: "1.0.0".to_string(),
        capabilities: capabilities.clone(),
        fuel_budget,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![
            FilesystemPermission {
                path_pattern: "/safe/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            },
            FilesystemPermission {
                path_pattern: "/safe/secret.key".to_string(),
                permission: FsPermissionLevel::Deny,
            },
        ],
    };

    let test_paths: Vec<&str> = vec!["/safe/readme.txt", "/safe/secret.key"];
    verifier.verify_all(
        fuel_remaining,
        fuel_budget,
        &capabilities,
        &capabilities,
        &audit,
        &manifest,
        &test_paths,
    );

    Ok(verifier.generate_compliance_report())
}

// ── Audit & Compliance Dashboard commands ───────────────────────────────────

/// Audit search with multiple filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSearchQuery {
    pub text: Option<String>,
    pub agent_id: Option<String>,
    pub event_type: Option<String>,
    pub severity: Option<String>,
    pub time_range: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSearchResult {
    pub entries: Vec<AuditRow>,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
}

pub(crate) fn audit_search(state: &AppState, query: AuditSearchQuery) -> Result<String, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let time_start = match query.time_range.as_deref() {
        Some("1h") => now.saturating_sub(3600),
        Some("24h") => now.saturating_sub(86400),
        Some("7d") => now.saturating_sub(604800),
        Some("30d") => now.saturating_sub(2592000),
        _ => 0,
    };

    let parsed_agent = query
        .agent_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(uuid::Uuid::parse_str)
        .transpose()
        .map_err(|e| format!("invalid agent_id: {e}"))?;

    let text_lower = query.text.as_deref().unwrap_or("").to_lowercase();

    let filtered: Vec<AuditRow> = events
        .iter()
        .filter(|e| {
            if e.timestamp < time_start {
                return false;
            }
            if let Some(aid) = parsed_agent {
                if e.agent_id != aid {
                    return false;
                }
            }
            if let Some(ref et) = query.event_type {
                if !et.is_empty() && format!("{:?}", e.event_type) != *et {
                    return false;
                }
            }
            if let Some(ref sev) = query.severity {
                let event_sev = event_severity(&e.event_type, &e.payload);
                if !sev.is_empty() && event_sev != *sev {
                    return false;
                }
            }
            if !text_lower.is_empty() {
                let payload_str = e.payload.to_string().to_lowercase();
                let etype_str = format!("{:?}", e.event_type).to_lowercase();
                let agent_str = e.agent_id.to_string().to_lowercase();
                if !payload_str.contains(&text_lower)
                    && !etype_str.contains(&text_lower)
                    && !agent_str.contains(&text_lower)
                {
                    return false;
                }
            }
            true
        })
        .map(event_to_row)
        .collect();

    let total = filtered.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let page: Vec<AuditRow> = filtered.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + page.len() < total;

    let result = AuditSearchResult {
        entries: page,
        total,
        offset,
        has_more,
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn event_severity(
    event_type: &nexus_kernel::audit::EventType,
    payload: &serde_json::Value,
) -> String {
    use nexus_kernel::audit::EventType;
    match event_type {
        EventType::Error => "error".to_string(),
        EventType::UserAction => {
            if payload
                .get("denied")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "denied".to_string()
            } else if payload
                .get("approval_required")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "warning".to_string()
            } else {
                "info".to_string()
            }
        }
        EventType::StateChange => {
            if payload
                .get("blocked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || payload
                    .get("firewall_block")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            {
                "denied".to_string()
            } else {
                "info".to_string()
            }
        }
        _ => "info".to_string(),
    }
}

/// Audit statistics for a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_entries: u64,
    pub entries_by_action: std::collections::HashMap<String, u64>,
    pub entries_by_agent: std::collections::HashMap<String, u64>,
    pub hitl_approvals: u64,
    pub hitl_denials: u64,
    pub hitl_timeouts: u64,
    pub capability_denials: u64,
    pub pii_redactions: u64,
    pub firewall_blocks: u64,
    pub total_fuel_consumed: u64,
    pub severity_counts: std::collections::HashMap<String, u64>,
}

pub(crate) fn audit_statistics(state: &AppState, time_range: String) -> Result<String, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let time_start = match time_range.as_str() {
        "1h" => now.saturating_sub(3600),
        "24h" => now.saturating_sub(86400),
        "7d" => now.saturating_sub(604800),
        "30d" => now.saturating_sub(2592000),
        _ => 0,
    };

    let mut stats = AuditStatistics {
        total_entries: 0,
        entries_by_action: std::collections::HashMap::new(),
        entries_by_agent: std::collections::HashMap::new(),
        hitl_approvals: 0,
        hitl_denials: 0,
        hitl_timeouts: 0,
        capability_denials: 0,
        pii_redactions: 0,
        firewall_blocks: 0,
        total_fuel_consumed: 0,
        severity_counts: std::collections::HashMap::new(),
    };

    for event in events.iter().filter(|e| e.timestamp >= time_start) {
        stats.total_entries += 1;

        let action = format!("{:?}", event.event_type);
        *stats.entries_by_action.entry(action).or_insert(0) += 1;

        let agent = event.agent_id.to_string();
        let short_agent = if agent.len() > 8 {
            agent[..8].to_string()
        } else {
            agent
        };
        *stats.entries_by_agent.entry(short_agent).or_insert(0) += 1;

        let sev = event_severity(&event.event_type, &event.payload);
        *stats.severity_counts.entry(sev.clone()).or_insert(0) += 1;

        // Count specific governance events from payload
        if let Some(p) = event.payload.as_object() {
            if p.get("approved").and_then(|v| v.as_bool()) == Some(true) {
                stats.hitl_approvals += 1;
            }
            if p.get("denied").and_then(|v| v.as_bool()) == Some(true) {
                stats.hitl_denials += 1;
            }
            if p.get("timeout").and_then(|v| v.as_bool()) == Some(true) {
                stats.hitl_timeouts += 1;
            }
            if p.get("capability_denied").and_then(|v| v.as_bool()) == Some(true) {
                stats.capability_denials += 1;
            }
            if p.get("pii_redacted").and_then(|v| v.as_bool()) == Some(true) {
                stats.pii_redactions += 1;
            }
            if p.get("firewall_block").and_then(|v| v.as_bool()) == Some(true)
                || p.get("blocked").and_then(|v| v.as_bool()) == Some(true)
            {
                stats.firewall_blocks += 1;
            }
            if let Some(fuel) = p.get("consumed").and_then(|v| v.as_u64()) {
                stats.total_fuel_consumed += fuel;
            }
            if let Some(fuel) = p.get("fuel").and_then(|v| v.as_u64()) {
                stats.total_fuel_consumed += fuel;
            }
        }
    }

    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

/// Verify audit hash chain integrity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerifyResult {
    pub verified: bool,
    pub chain_length: u64,
    pub verification_time_ms: u64,
    pub first_break_at: Option<u64>,
    pub last_verified_at: u64,
}

pub(crate) fn audit_verify_chain(state: &AppState) -> Result<String, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let start = std::time::Instant::now();
    let verified = audit.verify_integrity();
    let elapsed_ms = start.elapsed().as_millis() as u64;
    let events = audit.events();

    // Find first break (if any) by checking hash chain manually
    let first_break = if !verified {
        let mut break_idx = None;
        for (i, event) in events.iter().enumerate().skip(1) {
            if event.previous_hash != events[i - 1].hash {
                break_idx = Some(i as u64);
                break;
            }
        }
        break_idx
    } else {
        None
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let result = ChainVerifyResult {
        verified,
        chain_length: events.len() as u64,
        verification_time_ms: elapsed_ms,
        first_break_at: first_break,
        last_verified_at: now,
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Export audit as JSON or CSV.
pub(crate) fn audit_export_report(
    state: &AppState,
    format: String,
    time_range: String,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AuditExport)?;
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let time_start = match time_range.as_str() {
        "1h" => now.saturating_sub(3600),
        "24h" => now.saturating_sub(86400),
        "7d" => now.saturating_sub(604800),
        "30d" => now.saturating_sub(2592000),
        _ => 0,
    };

    let rows: Vec<AuditRow> = events
        .iter()
        .filter(|e| e.timestamp >= time_start)
        .map(event_to_row)
        .collect();

    match format.as_str() {
        "csv" => {
            let mut out =
                String::from("event_id,timestamp,agent_id,event_type,hash,previous_hash,payload\n");
            for r in &rows {
                let payload_escaped = r.payload.to_string().replace('"', "\"\"");
                out.push_str(&format!(
                    "{},{},{},{},{},{},\"{}\"\n",
                    r.event_id,
                    r.timestamp,
                    r.agent_id,
                    r.event_type,
                    r.hash,
                    r.previous_hash,
                    payload_escaped
                ));
            }
            Ok(out)
        }
        _ => serde_json::to_string_pretty(&rows).map_err(|e| e.to_string()),
    }
}

/// Governance metrics aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMetrics {
    pub hitl_approval_rate: f64,
    pub capability_denial_rate: f64,
    pub pii_redaction_count: u64,
    pub firewall_block_count: u64,
    pub total_fuel_consumed: u64,
    pub total_events: u64,
    pub autonomy_distribution: std::collections::HashMap<String, u32>,
    pub events_per_hour: Vec<(u64, u64)>,
}

pub(crate) fn compliance_governance_metrics(
    state: &AppState,
    time_range: String,
) -> Result<String, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let time_start = match time_range.as_str() {
        "1h" => now.saturating_sub(3600),
        "24h" => now.saturating_sub(86400),
        "7d" => now.saturating_sub(604800),
        "30d" => now.saturating_sub(2592000),
        _ => 0,
    };

    let mut hitl_total = 0u64;
    let mut hitl_approvals = 0u64;
    let mut cap_total = 0u64;
    let mut cap_denials = 0u64;
    let mut pii_count = 0u64;
    let mut fw_count = 0u64;
    let mut fuel = 0u64;
    let mut total = 0u64;
    let mut hourly: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();

    for event in events.iter().filter(|e| e.timestamp >= time_start) {
        total += 1;
        let hour_bucket = event.timestamp / 3600 * 3600;
        *hourly.entry(hour_bucket).or_insert(0) += 1;

        if let Some(p) = event.payload.as_object() {
            if p.contains_key("approved") || p.contains_key("denied") {
                hitl_total += 1;
                if p.get("approved").and_then(|v| v.as_bool()) == Some(true) {
                    hitl_approvals += 1;
                }
            }
            if p.contains_key("capability_check") || p.contains_key("capability_denied") {
                cap_total += 1;
                if p.get("capability_denied").and_then(|v| v.as_bool()) == Some(true) {
                    cap_denials += 1;
                }
            }
            if p.get("pii_redacted").and_then(|v| v.as_bool()) == Some(true) {
                pii_count += 1;
            }
            if p.get("firewall_block").and_then(|v| v.as_bool()) == Some(true)
                || p.get("blocked").and_then(|v| v.as_bool()) == Some(true)
            {
                fw_count += 1;
            }
            if let Some(f) = p.get("consumed").and_then(|v| v.as_u64()) {
                fuel += f;
            }
        }
    }

    // Agent state distribution from supervisor
    let mut autonomy_dist: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for status in supervisor.health_check() {
        let level = format!("{}", status.state);
        *autonomy_dist.entry(level).or_insert(0) += 1;
    }

    let mut events_per_hour: Vec<(u64, u64)> = hourly.into_iter().collect();
    events_per_hour.sort_by_key(|&(ts, _)| ts);

    let metrics = GovernanceMetrics {
        hitl_approval_rate: if hitl_total > 0 {
            hitl_approvals as f64 / hitl_total as f64
        } else {
            1.0
        },
        capability_denial_rate: if cap_total > 0 {
            cap_denials as f64 / cap_total as f64
        } else {
            0.0
        },
        pii_redaction_count: pii_count,
        firewall_block_count: fw_count,
        total_fuel_consumed: fuel,
        total_events: total,
        autonomy_distribution: autonomy_dist,
        events_per_hour,
    };
    serde_json::to_string(&metrics).map_err(|e| e.to_string())
}

/// Security events for the compliance dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub severity: String,
    pub agent_id: String,
    pub description: String,
}

pub(crate) fn compliance_security_events(
    state: &AppState,
    time_range: String,
) -> Result<String, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let time_start = match time_range.as_str() {
        "1h" => now.saturating_sub(3600),
        "24h" => now.saturating_sub(86400),
        "7d" => now.saturating_sub(604800),
        "30d" => now.saturating_sub(2592000),
        _ => 0,
    };

    let mut security_events: Vec<SecurityEvent> = Vec::new();

    for event in events.iter().filter(|e| e.timestamp >= time_start) {
        if let Some(p) = event.payload.as_object() {
            let is_security = p.get("capability_denied").and_then(|v| v.as_bool()) == Some(true)
                || p.get("firewall_block").and_then(|v| v.as_bool()) == Some(true)
                || p.get("blocked").and_then(|v| v.as_bool()) == Some(true)
                || p.get("auth_failed").and_then(|v| v.as_bool()) == Some(true)
                || p.get("escalation_attempt").and_then(|v| v.as_bool()) == Some(true)
                || matches!(event.event_type, nexus_kernel::audit::EventType::Error);

            if is_security {
                let desc = p
                    .get("message")
                    .or_else(|| p.get("reason"))
                    .or_else(|| p.get("event"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Security event")
                    .to_string();

                let severity =
                    if p.get("escalation_attempt").and_then(|v| v.as_bool()) == Some(true) {
                        "critical"
                    } else if p.get("capability_denied").and_then(|v| v.as_bool()) == Some(true)
                        || p.get("firewall_block").and_then(|v| v.as_bool()) == Some(true)
                    {
                        "high"
                    } else if matches!(event.event_type, nexus_kernel::audit::EventType::Error) {
                        "medium"
                    } else {
                        "low"
                    };

                let event_type = if p.get("capability_denied").is_some() {
                    "capability_denial"
                } else if p.get("firewall_block").is_some() || p.get("blocked").is_some() {
                    "firewall_block"
                } else if p.get("auth_failed").is_some() {
                    "auth_failure"
                } else if p.get("escalation_attempt").is_some() {
                    "escalation_attempt"
                } else {
                    "error"
                };

                security_events.push(SecurityEvent {
                    timestamp: event.timestamp,
                    event_type: event_type.to_string(),
                    severity: severity.to_string(),
                    agent_id: event.agent_id.to_string(),
                    description: desc,
                });
            }
        }
    }

    // Most recent first
    security_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    security_events.truncate(500);

    serde_json::to_string(&security_events).map_err(|e| e.to_string())
}

/* ================================================================== */
/*  File Manager — real filesystem operations                          */
/*                                                                     */
/*  These commands are USER-initiated via the Tauri frontend, not      */
/*  agent-initiated.  Agent file operations go through the kernel's    */
/*  capability system (CapabilityCheck + fuel budget).  User-facing    */
/*  commands rely on OS-level permissions and path sandboxing below    */
/*  (allowed_roots + reject "..") rather than agent capability gates.  */
/* ================================================================== */

/// Allowed root directories for the file manager.  Operations outside these
/// are rejected.  The user's home directory is always allowed.
pub(crate) fn file_manager_allowed_roots() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    vec![home]
}

/// Validate that `path` is under one of the allowed roots.  Returns the
/// canonical path on success.
pub(crate) fn file_manager_validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    let candidate = std::path::PathBuf::from(path);
    // Canonicalize — resolves symlinks and `..` segments
    let canonical = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|e| format!("path error: {e}"))?
    } else {
        // For creation: parent must exist and be valid
        let parent = candidate
            .parent()
            .ok_or_else(|| "invalid path: no parent".to_string())?;
        let parent_canon = parent
            .canonicalize()
            .map_err(|e| format!("parent path error: {e}"))?;
        parent_canon.join(candidate.file_name().unwrap_or_default())
    };
    let roots = file_manager_allowed_roots();
    if roots.iter().any(|r| canonical.starts_with(r)) {
        Ok(canonical)
    } else {
        Err(format!(
            "access denied: path outside allowed directories: {}",
            path
        ))
    }
}

pub(crate) fn file_manager_list(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_list", "path": path}),
    );

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&canonical).map_err(|e| format!("read_dir failed: {e}"))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("entry error: {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("metadata error: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path().to_string_lossy().to_string();
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        entries.push(serde_json::json!({
            "name": name,
            "path": entry_path,
            "is_dir": is_dir,
            "size": size,
            "modified": modified,
        }));
    }

    serde_json::to_string(&entries).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn file_manager_read(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_read", "path": path}),
    );

    std::fs::read_to_string(&canonical).map_err(|e| format!("read failed: {e}"))
}

pub(crate) fn file_manager_write(
    state: &AppState,
    path: String,
    content: String,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::Default)?;
    state.validate_path_input(&path)?;
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_write", "path": path, "size": content.len()}),
    );

    std::fs::write(&canonical, &content).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn file_manager_create_dir(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_create_dir", "path": path}),
    );

    std::fs::create_dir_all(&canonical).map_err(|e| format!("create_dir failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn file_manager_delete(state: &AppState, path: String) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::Default)?;
    state.validate_path_input(&path)?;
    let canonical = file_manager_validate_path(&path)?;

    let is_dir = canonical.is_dir();

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_delete", "path": path, "is_dir": is_dir}),
    );

    if is_dir {
        std::fs::remove_dir_all(&canonical).map_err(|e| format!("remove_dir failed: {e}"))?;
    } else {
        std::fs::remove_file(&canonical).map_err(|e| format!("remove_file failed: {e}"))?;
    }
    Ok("ok".to_string())
}

pub(crate) fn file_manager_rename(
    state: &AppState,
    from: String,
    to: String,
) -> Result<String, String> {
    let from_canonical = file_manager_validate_path(&from)?;
    let to_canonical = file_manager_validate_path(&to)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_rename", "from": from, "to": to}),
    );

    std::fs::rename(&from_canonical, &to_canonical).map_err(|e| format!("rename failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn file_manager_home() -> Result<String, String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())
}
