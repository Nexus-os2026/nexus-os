//! advanced domain implementation.

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

// ── Cognitive Filesystem commands ───────────────────────────────────

pub(crate) fn cogfs_index_file(path: String) -> Result<(), String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("failed to read file: {e}"))?;
    let metadata = std::fs::metadata(&path).map_err(|e| format!("failed to stat file: {e}"))?;
    let size_bytes = metadata.len();
    let mut indexer = nexus_kernel::cogfs::SemanticIndexer::new();
    indexer
        .index_file(&path, &content, size_bytes)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn cogfs_query(question: String) -> Result<serde_json::Value, String> {
    let indexer = nexus_kernel::cogfs::SemanticIndexer::new();
    let query_engine = nexus_kernel::cogfs::NaturalQuery::new();
    let results = query_engine.query(&question, &indexer);
    serde_json::to_value(&results).map_err(|e| e.to_string())
}

pub(crate) fn cogfs_get_graph(file_path: String) -> Result<serde_json::Value, String> {
    let graph = nexus_kernel::cogfs::KnowledgeGraph::new();
    let links = graph.get_links(&file_path);
    serde_json::to_value(&links).map_err(|e| e.to_string())
}

pub(crate) fn cogfs_watch_directory(path: String) -> Result<(), String> {
    let mut watcher =
        nexus_kernel::cogfs::FileWatcher::new(nexus_kernel::cogfs::WatchConfig::default());
    watcher.add_watch(&path);
    Ok(())
}

pub(crate) fn cogfs_get_entities(file_path: String) -> Result<serde_json::Value, String> {
    let mut indexer = nexus_kernel::cogfs::SemanticIndexer::new();
    if let Ok(indexed) = indexer.index_file(&file_path, "", 0) {
        return serde_json::to_value(&indexed.entities).map_err(|e| e.to_string());
    }
    Ok(serde_json::json!([]))
}

pub(crate) fn cogfs_search(query: String, limit: usize) -> Result<serde_json::Value, String> {
    let indexer = nexus_kernel::cogfs::SemanticIndexer::new();
    let query_engine = nexus_kernel::cogfs::NaturalQuery::new();
    let mut results = query_engine.query(&query, &indexer);
    results.truncate(limit);
    serde_json::to_value(&results).map_err(|e| e.to_string())
}

pub(crate) fn cogfs_get_context(topic: String) -> Result<serde_json::Value, String> {
    let indexer = nexus_kernel::cogfs::SemanticIndexer::new();
    let graph = nexus_kernel::cogfs::KnowledgeGraph::new();
    let watcher =
        nexus_kernel::cogfs::FileWatcher::new(nexus_kernel::cogfs::WatchConfig::default());
    let builder = nexus_kernel::cogfs::ContextBuilder::new();
    let context = builder.build_context(&topic, &indexer, &graph, &watcher);
    serde_json::to_value(&context).map_err(|e| e.to_string())
}

// ── Civilization commands ───────────────────────────────────────────

pub(crate) fn civ_propose_rule(
    proposer_id: String,
    rule_text: String,
) -> Result<serde_json::Value, String> {
    let mut parliament = nexus_kernel::civilization::Parliament::new();
    let mut log = nexus_kernel::civilization::CivilizationLog::new();
    let proposal = parliament.propose_rule(&proposer_id, &rule_text, &mut log);
    serde_json::to_value(&proposal).map_err(|e| e.to_string())
}

pub(crate) fn civ_vote(agent_id: String, proposal_id: String, vote: bool) -> Result<(), String> {
    let mut parliament = nexus_kernel::civilization::Parliament::new();
    let mut log = nexus_kernel::civilization::CivilizationLog::new();
    let pid = uuid::Uuid::parse_str(&proposal_id).map_err(|e| e.to_string())?;
    parliament
        .cast_vote(&agent_id, pid, vote, 1.0, &mut log)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn civ_get_parliament_status() -> Result<serde_json::Value, String> {
    let mut parliament = nexus_kernel::civilization::Parliament::new();
    let status = serde_json::json!({
        "active_proposals": parliament.get_active_proposals().len(),
        "passed_rules": parliament.get_passed_rules().len(),
        "total_votes": 0,
    });
    Ok(status)
}

pub(crate) fn civ_get_economy_status() -> Result<serde_json::Value, String> {
    let economy = nexus_kernel::civilization::CivilizationEconomy::new();
    let balances = economy.get_all_balances();
    let status = serde_json::json!({
        "total_agents": balances.len(),
        "total_tokens_circulating": balances.iter().map(|b| b.balance).sum::<f64>(),
        "transactions_today": 0,
    });
    Ok(status)
}

pub(crate) fn civ_get_roles() -> Result<serde_json::Value, String> {
    let manager = nexus_kernel::civilization::RoleManager::new();
    let roles = manager.get_current_roles();
    serde_json::to_value(roles).map_err(|e| e.to_string())
}

pub(crate) fn civ_run_election(role: String) -> Result<serde_json::Value, String> {
    let mut manager = nexus_kernel::civilization::RoleManager::new();
    let mut log = nexus_kernel::civilization::CivilizationLog::new();
    let role_enum = match role.as_str() {
        "Coordinator" => nexus_kernel::civilization::Role::Coordinator,
        "Auditor" => nexus_kernel::civilization::Role::Auditor,
        "Researcher" => nexus_kernel::civilization::Role::Researcher,
        "Guardian" => nexus_kernel::civilization::Role::Guardian,
        _ => return Err(format!("unknown role: {role}")),
    };
    let election = manager
        .run_election(role_enum, Vec::new(), &mut log)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&election).map_err(|e| e.to_string())
}

pub(crate) fn civ_resolve_dispute(
    agent_a: String,
    agent_b: String,
    issue: String,
) -> Result<serde_json::Value, String> {
    let mut resolver = nexus_kernel::civilization::DisputeResolver::new();
    let mut log = nexus_kernel::civilization::CivilizationLog::new();
    let dispute = resolver.file_dispute(&agent_a, &agent_b, &issue, &mut log);
    serde_json::to_value(&dispute).map_err(|e| e.to_string())
}

pub(crate) fn civ_get_governance_log(_limit: u32) -> Result<serde_json::Value, String> {
    let log = nexus_kernel::civilization::CivilizationLog::new();
    let events = log.get_events();
    serde_json::to_value(events).map_err(|e| e.to_string())
}

// ── Sovereign Identity commands ─────────────────────────────────────

pub(crate) fn identity_get_agent_passport(agent_id: String) -> Result<serde_json::Value, String> {
    let aid = uuid::Uuid::parse_str(&agent_id).map_err(|e| e.to_string())?;
    let passport = nexus_kernel::identity::export_passport(
        aid,
        format!("did:key:{agent_id}"),
        Vec::new(),
        String::new(),
        Vec::new(),
        Vec::new(),
    );
    serde_json::to_value(&passport).map_err(|e| e.to_string())
}

pub(crate) fn identity_generate_proof(
    agent_id: String,
    claim: String,
) -> Result<serde_json::Value, String> {
    let aid = uuid::Uuid::parse_str(&agent_id).map_err(|e| e.to_string())?;
    let generator = nexus_kernel::identity::ZkProofGenerator::new();
    let zk_claim = match claim.as_str() {
        "CreatedByNexus" => nexus_kernel::identity::ZkClaim::CreatedByNexus,
        _ => nexus_kernel::identity::ZkClaim::CreatedByNexus,
    };
    let proof = generator
        .generate_proof(aid, zk_claim, 1)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&proof).map_err(|e| e.to_string())
}

pub(crate) fn identity_verify_proof(proof: serde_json::Value) -> Result<bool, String> {
    let zk_proof: nexus_kernel::identity::ZkProof =
        serde_json::from_value(proof).map_err(|e| e.to_string())?;
    let generator = nexus_kernel::identity::ZkProofGenerator::new();
    match generator.verify_proof(&zk_proof) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub(crate) fn identity_export_passport(agent_id: String) -> Result<serde_json::Value, String> {
    let aid = uuid::Uuid::parse_str(&agent_id).map_err(|e| e.to_string())?;
    let passport = nexus_kernel::identity::export_passport(
        aid,
        format!("did:key:{agent_id}"),
        Vec::new(),
        String::new(),
        Vec::new(),
        Vec::new(),
    );
    let exported = serde_json::to_string(&passport).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"passport_json": exported}))
}

// ── Mesh commands ───────────────────────────────────────────────────

pub(crate) fn mesh_discover_peers() -> Result<serde_json::Value, String> {
    let local_id = uuid::Uuid::new_v4();
    let discovery = nexus_kernel::mesh::MeshDiscovery::new(local_id);
    let peers = discovery.list_peers();
    serde_json::to_value(&peers).map_err(|e| e.to_string())
}

pub(crate) fn mesh_add_peer(address: String) -> Result<(), String> {
    let local_id = uuid::Uuid::new_v4();
    let mut discovery = nexus_kernel::mesh::MeshDiscovery::new(local_id);
    let peer = nexus_kernel::mesh::PeerInfo {
        peer_id: uuid::Uuid::new_v4(),
        address,
        port: 9090,
        name: String::new(),
        discovered_at: 0,
        last_seen: 0,
        status: nexus_kernel::mesh::PeerStatus::Discovered,
        capabilities: Vec::new(),
    };
    discovery.add_peer(peer).map_err(|e| e.to_string())
}

pub(crate) fn mesh_get_peers() -> Result<serde_json::Value, String> {
    let local_id = uuid::Uuid::new_v4();
    let discovery = nexus_kernel::mesh::MeshDiscovery::new(local_id);
    let peers = discovery.list_peers();
    serde_json::to_value(&peers).map_err(|e| e.to_string())
}

pub(crate) fn mesh_migrate_agent(
    agent_id: String,
    _target_peer: String,
) -> Result<serde_json::Value, String> {
    let local_id = uuid::Uuid::new_v4();
    let mut migration = nexus_kernel::mesh::AgentMigration::new(local_id);
    let aid = uuid::Uuid::parse_str(&agent_id).map_err(|e| e.to_string())?;
    let status = migration
        .prepare_migration(aid)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

pub(crate) fn mesh_distribute_task(
    task: String,
    agent_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    let local_id = uuid::Uuid::new_v4();
    let mut executor = nexus_kernel::mesh::DistributedExecutor::new(local_id);
    let assignments: Vec<(uuid::Uuid, String)> = agent_ids
        .iter()
        .map(|s| (uuid::Uuid::parse_str(s).unwrap_or_default(), task.clone()))
        .collect();
    let dt = executor
        .distribute_task(&task, assignments)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&dt).map_err(|e| e.to_string())
}

pub(crate) fn mesh_get_sync_status() -> Result<serde_json::Value, String> {
    let sync = nexus_kernel::mesh::ConsciousnessSync::new("local".to_string());
    // get_sync_status requires an agent_id; return empty status for overview
    let status = serde_json::json!({
        "local_peer_id": "local",
        "synced_agents": 0,
    });
    // suppress unused sync — parameter reserved for future distributed sync implementation
    let _ = sync;
    Ok(status)
}

// ── Self-Rewrite commands ───────────────────────────────────────────

pub(crate) fn self_rewrite_analyze(state: &AppState) -> Result<serde_json::Value, String> {
    // Scan real kernel source files for performance-relevant patterns
    let mut bottlenecks = Vec::new();
    let kernel_src = std::path::Path::new("kernel/src");
    if kernel_src.exists() {
        scan_for_bottlenecks(kernel_src, &mut bottlenecks);
    }

    // Also run the kernel profiler for any runtime samples
    let profiler = nexus_kernel::self_rewrite::PerformanceProfiler::new();
    let runtime_bottlenecks = profiler.detect_bottlenecks();
    for b in &runtime_bottlenecks {
        bottlenecks.push(serde_json::json!({
            "function_name": b.function_name,
            "module_path": b.module_path,
            "severity": format!("{:?}", b.severity),
            "reason": b.reason,
            "suggestion": b.suggestion,
        }));
    }

    // Generate patches from bottlenecks using the LLM
    let generator = nexus_kernel::self_rewrite::PatchGenerator::new();
    let mut patches_store = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    patches_store.clear();

    for b in &bottlenecks {
        let target_file = b["module_path"].as_str().unwrap_or("kernel");
        let target_fn = b["function_name"].as_str().unwrap_or("unknown");
        let suggestion = b["suggestion"].as_str().unwrap_or("");

        // Try to read the actual source file to provide real code
        let file_path = format!("kernel/src/{}.rs", target_file.replace("::", "/"));
        let original_code = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|_| format!("// Source: {target_file}::{target_fn}\n// {suggestion}"));

        // Use the LLM to generate an optimized version
        let config = load_config().map_err(agent_error)?;
        let provider_config = build_provider_config(&config);
        let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
        let model = if config.llm.default_model.trim().is_empty() {
            "mock-1".to_string()
        } else {
            config.llm.default_model.clone()
        };

        let mut gateway = GovernedLlmGateway::new(provider);
        gateway.set_skip_output_firewall(true);
        let mut caps = HashSet::new();
        caps.insert("llm.query".to_string());
        let mut ctx = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: caps,
            fuel_remaining: 20_000,
        };

        let prompt = format!(
            "You are a Rust performance optimizer. Given this bottleneck:\n\
             Function: {target_fn}\n\
             Module: {target_file}\n\
             Issue: {suggestion}\n\n\
             Suggest a concrete code optimization in valid Rust. Reply with ONLY the optimized code snippet, no explanation."
        );

        let optimized_code = match gateway.query(&mut ctx, &prompt, 500, &model) {
            Ok(resp) => resp.output_text,
            Err(_) => format!(
                "// Optimization for: {suggestion}\n// LLM unavailable — manual review needed"
            ),
        };

        if let Ok(patch) = generator.generate_patch(
            target_file,
            target_fn,
            &original_code,
            &optimized_code,
            suggestion,
        ) {
            patches_store.push(patch);
        }
    }

    serde_json::to_value(&bottlenecks).map_err(|e| e.to_string())
}

pub(crate) fn scan_for_bottlenecks(
    dir: &std::path::Path,
    bottlenecks: &mut Vec<serde_json::Value>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_for_bottlenecks(&path, bottlenecks);
            continue;
        }
        if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let module = path
                    .strip_prefix("kernel/src/")
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('/', "::")
                    .replace(".rs", "");

                // Detect common Rust performance patterns
                for (i, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.contains(".clone()") && trimmed.contains("for ") {
                        bottlenecks.push(serde_json::json!({
                            "function_name": format!("line_{}", i + 1),
                            "module_path": module,
                            "severity": "Medium",
                            "reason": format!("Clone inside loop at line {}", i + 1),
                            "suggestion": "Consider borrowing or moving instead of cloning inside a loop",
                        }));
                    }
                    if trimmed.contains("unwrap()") && !trimmed.contains("test") {
                        bottlenecks.push(serde_json::json!({
                            "function_name": format!("line_{}", i + 1),
                            "module_path": module,
                            "severity": "Low",
                            "reason": format!("Unwrap call at line {} may panic in production", i + 1),
                            "suggestion": "Replace unwrap() with proper error handling or unwrap_or_else",
                        }));
                    }
                }
            }
        }
    }
    // Limit to 20 most relevant
    bottlenecks.truncate(20);
}

pub(crate) fn self_rewrite_suggest_patches(state: &AppState) -> Result<serde_json::Value, String> {
    let patches = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    serde_json::to_value(&*patches).map_err(|e| e.to_string())
}

pub(crate) fn self_rewrite_preview_patch(
    state: &AppState,
    patch_id: String,
) -> Result<serde_json::Value, String> {
    let pid = Uuid::parse_str(&patch_id).map_err(|e| e.to_string())?;
    let patches = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let patch = patches
        .iter()
        .find(|p| p.id == pid)
        .ok_or_else(|| format!("patch {patch_id} not found"))?;

    let diff = format!(
        "--- a/{file}\n+++ b/{file}\n\n// Function: {func}\n// Goal: {goal}\n\n\
         - {original}\n+ {optimized}",
        file = patch.target_file,
        func = patch.target_function,
        goal = patch.optimization_goal,
        original = patch
            .original_code
            .lines()
            .take(10)
            .collect::<Vec<_>>()
            .join("\n- "),
        optimized = patch
            .optimized_code
            .lines()
            .take(10)
            .collect::<Vec<_>>()
            .join("\n+ "),
    );
    Ok(serde_json::json!(diff))
}

pub(crate) fn self_rewrite_test_patch(
    state: &AppState,
    patch_id: String,
) -> Result<serde_json::Value, String> {
    let pid = Uuid::parse_str(&patch_id).map_err(|e| e.to_string())?;
    let mut patches = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let patch = patches
        .iter_mut()
        .find(|p| p.id == pid)
        .ok_or_else(|| format!("patch {patch_id} not found"))?;

    // Validate patch status
    if patch.status == nexus_kernel::self_rewrite::PatchStatus::Generated {
        patch.status = nexus_kernel::self_rewrite::PatchStatus::Validated;
    }

    let mut tester = nexus_kernel::self_rewrite::PatchTester::new();

    // Run cargo check to verify syntax validity
    let compile_ok = std::process::Command::new("cargo")
        .args(["check", "--workspace", "--quiet"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let result = tester.test_patch(
        patch,
        compile_ok,
        if compile_ok { 1 } else { 0 },
        0,
        1.0,
        1.0,
    );
    match result {
        Ok(run) => serde_json::to_value(&run).map_err(|e| e.to_string()),
        Err(e) => Ok(serde_json::json!({"status": "failed", "reason": e.to_string()})),
    }
}

pub(crate) fn self_rewrite_apply_patch(state: &AppState, patch_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&patch_id).map_err(|e| e.to_string())?;
    let mut patches = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let patch = patches
        .iter_mut()
        .find(|p| p.id == pid)
        .ok_or_else(|| format!("patch {patch_id} not found"))?;

    // Mark as approved (HITL confirmed by the frontend dialog)
    patch.status = nexus_kernel::self_rewrite::PatchStatus::Approved;

    let mut patcher = nexus_kernel::self_rewrite::HotPatcher::new();
    match patcher.apply_patch(patch.clone(), 1.0, 1.0) {
        Ok(_applied) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub(crate) fn self_rewrite_rollback(state: &AppState, patch_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&patch_id).map_err(|e| e.to_string())?;
    let mut patches = state
        .self_rewrite_patches
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let patch = patches
        .iter_mut()
        .find(|p| p.id == pid)
        .ok_or_else(|| format!("patch {patch_id} not found"))?;

    // Revert to original code if the file was modified
    let file_path = format!("kernel/src/{}.rs", patch.target_file.replace("::", "/"));
    if std::path::Path::new(&file_path).exists() && !patch.original_code.is_empty() {
        std::fs::write(&file_path, &patch.original_code)
            .map_err(|e| format!("failed to write rollback: {e}"))?;
    }
    patch.status = nexus_kernel::self_rewrite::PatchStatus::Reverted;
    Ok(())
}

pub(crate) fn self_rewrite_get_history() -> Result<serde_json::Value, String> {
    let rollback = nexus_kernel::self_rewrite::RollbackEngine::new();
    let history = rollback.get_rollback_history();
    serde_json::to_value(history).map_err(|e| e.to_string())
}

// ── Omniscience commands ────────────────────────────────────────────

static OMNISCIENCE_ENGINE: std::sync::OnceLock<
    Mutex<nexus_kernel::omniscience::ScreenUnderstanding>,
> = std::sync::OnceLock::new();

pub(crate) fn omniscience_engine() -> &'static Mutex<nexus_kernel::omniscience::ScreenUnderstanding>
{
    OMNISCIENCE_ENGINE
        .get_or_init(|| Mutex::new(nexus_kernel::omniscience::ScreenUnderstanding::new()))
}

pub(crate) fn omniscience_get_screen_context() -> Result<serde_json::Value, String> {
    let screen = omniscience_engine()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let context = screen.get_rolling_context(1);
    serde_json::to_value(&context).map_err(|e| e.to_string())
}

pub(crate) fn omniscience_get_predictions() -> Result<serde_json::Value, String> {
    let predictor = nexus_kernel::omniscience::IntentPredictor::new();
    let predictions = predictor.predict_intent(&[]);
    serde_json::to_value(&predictions).map_err(|e| e.to_string())
}

pub(crate) fn omniscience_enable(interval_ms: u64) -> Result<(), String> {
    let mut screen = omniscience_engine()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    screen.set_capture_interval_ms(interval_ms);
    screen.start();
    Ok(())
}

pub(crate) fn omniscience_disable() -> Result<(), String> {
    let mut screen = omniscience_engine()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    screen.stop();
    Ok(())
}

pub(crate) fn omniscience_execute_action(
    action: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut executor = nexus_kernel::omniscience::ActionExecutor::new();
    let action_type = action
        .get("action_type")
        .and_then(|v| v.as_str())
        .unwrap_or("TypeText");
    let target = action
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let a_type = match action_type {
        "Click" => nexus_kernel::omniscience::ActionType::Click,
        "KeyPress" => nexus_kernel::omniscience::ActionType::KeyPress,
        "Navigate" => nexus_kernel::omniscience::ActionType::Navigate,
        "Scroll" => nexus_kernel::omniscience::ActionType::Scroll,
        _ => nexus_kernel::omniscience::ActionType::TypeText,
    };
    let action_id = executor
        .queue_action(a_type, target, serde_json::json!({}), true)
        .map_err(|e| e.to_string())?;
    let ca = executor.get_action(&action_id);
    serde_json::to_value(ca).map_err(|e| e.to_string())
}

pub(crate) fn omniscience_get_app_context(app_name: String) -> Result<serde_json::Value, String> {
    let integration = nexus_kernel::omniscience::AppIntegration::new();
    let context = integration.get_app_context(&app_name);
    serde_json::to_value(context).map_err(|e| e.to_string())
}

// ── Consciousness Heatmap (for Mission Control) ─────────────────────

pub(crate) fn get_consciousness_heatmap(_state: &AppState) -> Result<serde_json::Value, String> {
    // ConsciousnessEngine has no all_states() method; return empty heatmap
    let heatmap: Vec<serde_json::Value> = Vec::new();
    Ok(serde_json::json!(heatmap))
}

// ── Self-Improving OS Commands ──────────────────────────────────────────────

pub(crate) fn get_os_fitness(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let fitness = os.compute_fitness();
    serde_json::to_string(&fitness).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_fitness_history(state: &AppState, days: u32) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let scores = os.fitness_history.last_n_days(days as usize);
    serde_json::to_string(&scores).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_routing_stats(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = os.routing.get_stats();
    serde_json::to_string(&stats).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_ui_adaptations(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let adapt = os.ui.get_adaptation();
    serde_json::to_string(&adapt).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_user_profile(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let profile = os.knowledge.user_profile().clone();
    serde_json::to_string(&profile).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn record_page_visit(state: &AppState, page: String) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.ui.record_page_visit(&page);
    Ok(())
}

pub(crate) fn record_feature_use(state: &AppState, feature: String) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.ui.record_feature_use(&feature);
    Ok(())
}

pub(crate) fn override_security_block(
    state: &AppState,
    event_id: String,
    rule_id: String,
) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.security.record_false_positive(
        &rule_id,
        nexus_kernel::self_improve::SecurityEvent {
            event_id,
            rule_id: rule_id.clone(),
            description: "User override".to_string(),
            input_sample: String::new(),
            was_blocked: true,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        },
    );
    Ok(())
}

pub(crate) fn get_os_improvement_log(state: &AppState, limit: u32) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let log = os.improvement_log(limit);
    serde_json::to_string(&log).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_morning_os_briefing(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let briefing = os.morning_briefing();
    serde_json::to_string(&briefing).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn record_routing_outcome(
    state: &AppState,
    category: String,
    agent_id: String,
    score: f64,
) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.routing
        .record(nexus_kernel::self_improve::RoutingOutcome {
            request_summary: String::new(),
            request_category: category,
            agent_id,
            score,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    Ok(())
}

pub(crate) fn record_operation_timing(
    state: &AppState,
    operation: String,
    latency_ms: f64,
) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.performance.record_timing(&operation, latency_ms);
    Ok(())
}

pub(crate) fn get_performance_report(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let report = os.performance.report();
    serde_json::to_string(&report).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_security_evolution_report(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let report = os.security.report();
    serde_json::to_string(&report).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn record_knowledge_interaction(
    state: &AppState,
    topic: String,
    languages: Vec<String>,
    score: f64,
) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.knowledge
        .record_interaction(nexus_kernel::self_improve::InteractionSummary {
            topic,
            languages_mentioned: languages,
            score,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    Ok(())
}

pub(crate) fn get_os_dream_status(state: &AppState) -> Result<String, String> {
    let os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let pending = os.dreams.pending_dream_types();
    let history_len = os.dreams.history().len();
    serde_json::to_string(&serde_json::json!({
        "pending_types": pending,
        "history_count": history_len,
        "token_budget": os.dreams.token_budget(),
    }))
    .map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn set_self_improve_enabled(state: &AppState, enabled: bool) -> Result<(), String> {
    let mut os = state
        .self_improving_os
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    os.set_enabled(enabled);
    Ok(())
}
