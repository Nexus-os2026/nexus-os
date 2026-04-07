//! model_hub domain implementation.

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

// ── RAG Pipeline Commands ──

pub(crate) fn format_from_extension(path: &str) -> Result<SupportedFormat, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "txt" | "text" | "log" | "csv" => Ok(SupportedFormat::PlainText),
        "md" | "markdown" => Ok(SupportedFormat::Markdown),
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "toml" | "yaml"
        | "yml" | "json" | "html" | "css" | "sh" | "bash" | "sql" | "rb" | "swift" | "kt" => {
            Ok(SupportedFormat::Code)
        }
        _ => Err(format!(
            "unsupported file extension '.{ext}'. Supported: .txt, .md, .rs, .py, .js, .ts, .go, .java, .c, .cpp, .toml, .yaml, .json, .html, .css, .sh, .sql"
        )),
    }
}

pub(crate) fn index_document(state: &AppState, file_path: String) -> Result<String, String> {
    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("failed to read file: {e}"))?;

    let format = format_from_extension(&file_path)?;

    let provider = get_configured_provider();

    // Detect embedding dimension from the provider on first ingest.
    // Probe with a short text to discover the actual dimension.
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    if rag.documents.is_empty() {
        if let Ok(probe) = provider.embed(&["dimension probe"], &rag.config.embedding_model) {
            if let Some(first) = probe.embeddings.first() {
                let detected = first.len();
                if detected != rag.config.embedding_dimension {
                    eprintln!(
                        "[nexus-rag] embedding dimension changed: {} -> {} (provider: {}). Recreating vector store.",
                        rag.config.embedding_dimension, detected, provider.name()
                    );
                    rag.config.embedding_dimension = detected;
                    rag.vector_store =
                        nexus_connectors_llm::vector_store::VectorStore::new(detected);
                }
            }
        }
    } else {
        // Documents already indexed — warn if dimension would change
        if let Ok(probe) = provider.embed(&["dimension probe"], &rag.config.embedding_model) {
            if let Some(first) = probe.embeddings.first() {
                let detected = first.len();
                if detected != rag.config.embedding_dimension {
                    eprintln!(
                        "[nexus-rag] WARNING: provider {} produces {}-dim embeddings but store uses {}. Re-index to switch.",
                        provider.name(), detected, rag.config.embedding_dimension
                    );
                }
            }
        }
    }

    let mut redaction = state
        .redaction_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    let doc = rag
        .ingest_document(&content, &file_path, format, &provider, &mut redaction)
        .map_err(|e| format!("ingest failed: {e}"))?;

    drop(rag);
    drop(redaction);

    state.log_event(
        Uuid::new_v4(),
        EventType::ToolCall,
        json!({
            "event": "rag.ingest",
            "file_path": file_path,
            "format": doc.format,
            "chunk_count": doc.chunk_count,
            "provider": provider.name(),
        }),
    );

    serde_json::to_string(&doc).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn search_documents(
    state: &AppState,
    query: String,
    top_k: Option<u32>,
) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());

    if let Some(k) = top_k {
        rag.config.top_k = k as usize;
    }

    let provider = get_configured_provider();
    let results = rag
        .query(&query, &provider)
        .map_err(|e| format!("search failed: {e}"))?;

    // SearchResult doesn't derive Serialize, so convert manually.
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "chunk_id": r.chunk_id,
                "doc_path": r.doc_path,
                "chunk_index": r.chunk_index,
                "content": r.content,
                "score": r.score,
            })
        })
        .collect();

    serde_json::to_string(&rows).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn chat_with_documents(state: &AppState, question: String) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let provider = get_configured_provider();

    let results = rag
        .query(&question, &provider)
        .map_err(|e| format!("query failed: {e}"))?;

    let prompt = rag.build_rag_prompt(&question, &results);
    let chunk_count = results.len();

    let sources: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "doc_path": r.doc_path,
                "chunk_index": r.chunk_index,
                "score": r.score,
            })
        })
        .collect();

    drop(rag);

    let model = get_default_model();
    let provider_name = provider.name().to_string();

    // Call the real LLM with the assembled RAG prompt.
    let response = match provider.query(&prompt, 1024, &model) {
        Ok(llm_resp) => {
            state.log_event(
                Uuid::new_v4(),
                EventType::LlmCall,
                json!({
                    "event": "rag.chat",
                    "question_len": question.len(),
                    "chunk_count": chunk_count,
                    "provider": provider_name,
                    "model": llm_resp.model_name,
                    "tokens": llm_resp.token_count,
                }),
            );

            json!({
                "answer": llm_resp.output_text,
                "sources": sources,
                "model": format!("{}/{}", provider_name, llm_resp.model_name),
                "tokens": llm_resp.token_count,
            })
        }
        Err(e) => {
            eprintln!("[nexus-rag] LLM query failed, returning raw prompt: {e}");

            state.log_event(
                Uuid::new_v4(),
                EventType::ToolCall,
                json!({
                    "event": "rag.chat_fallback",
                    "question_len": question.len(),
                    "chunk_count": chunk_count,
                    "error": e.to_string(),
                }),
            );

            json!({
                "answer": prompt,
                "sources": sources,
                "model": format!("{}/fallback", provider_name),
                "tokens": 0,
                "fallback": true,
                "error": format!("LLM query failed: {e}. Returning raw RAG prompt."),
            })
        }
    };

    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn list_indexed_documents(state: &AppState) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let docs = rag.list_documents();
    serde_json::to_string(docs).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn remove_indexed_document(
    state: &AppState,
    doc_path: String,
) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let removed = rag.remove_document(&doc_path);
    let response = json!({
        "removed": removed,
        "path": doc_path,
    });
    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_document_governance(
    state: &AppState,
    doc_path: String,
) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let doc = rag
        .documents
        .iter()
        .find(|d| d.path == doc_path)
        .ok_or_else(|| format!("document not found: {doc_path}"))?;
    serde_json::to_string(&doc.governance).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_semantic_map(state: &AppState) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let points = rag.vector_store.get_2d_projection();
    serde_json::to_string(&points).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_document_access_log(
    state: &AppState,
    doc_path: String,
) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let entries: Vec<_> = rag.get_document_access_log(&doc_path);
    serde_json::to_string(&entries).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_active_llm_provider(_state: &AppState) -> Result<String, String> {
    let provider = get_configured_provider();
    let provider_name = provider.name().to_string();
    let model = get_default_model();

    let (status, message) = if provider_name == "mock" {
        (
            "no_provider_available",
            "Install Ollama or configure an API key".to_string(),
        )
    } else {
        ("connected", format!("Using {provider_name}"))
    };

    // Determine embedding model from RAG config default
    let embedding_model = if provider_name == "ollama" {
        "nomic-embed-text".to_string()
    } else {
        "all-minilm".to_string()
    };

    let response = json!({
        "provider": provider_name,
        "model": model,
        "embedding_model": embedding_model,
        "status": status,
        "message": message,
    });

    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

// ── Model Hub Commands ──────────────────────────────────────────────────────

pub(crate) fn search_models(
    state: &AppState,
    query: String,
    limit: Option<u32>,
) -> Result<String, String> {
    let limit = limit.unwrap_or(20) as usize;
    state.log_event(
        AgentId::nil(),
        EventType::ToolCall,
        json!({"operation": "model_hub.search", "query": &query, "limit": limit}),
    );
    let result = model_hub::search_huggingface(&query, limit)?;
    serde_json::to_string(&result).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_model_info(_state: &AppState, model_id: String) -> Result<String, String> {
    let info = model_hub::get_model_details(&model_id)?;
    serde_json::to_string(&info).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn check_model_compatibility(
    _state: &AppState,
    file_size_bytes: u64,
) -> Result<String, String> {
    let compat = model_hub::check_compatibility(file_size_bytes);
    serde_json::to_string(&compat).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn list_local_models(state: &AppState) -> Result<String, String> {
    let mut registry = state
        .model_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.discover();
    let models = registry.available_models().to_vec();
    serde_json::to_string(&models).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn delete_local_model(state: &AppState, model_id: String) -> Result<String, String> {
    let mut registry = state
        .model_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.discover();

    let model_dir = match registry.find_model(&model_id) {
        Some(config) => config.model_path.clone(),
        None => {
            return serde_json::to_string(
                &json!({"deleted": false, "model_id": &model_id, "error": "model not found"}),
            )
            .map_err(|e| format!("Serialization error: {}", e));
        }
    };

    // Safety: only delete within the models directory
    let models_root = registry.models_dir().clone();
    if !model_dir.starts_with(&models_root) {
        return Err("refusing to delete path outside models directory".to_string());
    }

    drop(registry); // unlock before filesystem operation

    match std::fs::remove_dir_all(&model_dir) {
        Ok(()) => {
            state.log_event(
                AgentId::nil(),
                EventType::ToolCall,
                json!({"operation": "model_hub.delete", "model_id": &model_id, "path": model_dir.display().to_string()}),
            );
            serde_json::to_string(&json!({"deleted": true, "model_id": &model_id}))
                .map_err(|e| format!("Serialization error: {}", e))
        }
        Err(e) => serde_json::to_string(
            &json!({"deleted": false, "model_id": &model_id, "error": e.to_string()}),
        )
        .map_err(|e| format!("Serialization error: {}", e)),
    }
}

pub(crate) fn get_system_specs() -> Result<String, String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let cpu_cores = sys.cpus().len();

    serde_json::to_string(&json!({
        "total_ram_mb": sys.total_memory() / (1024 * 1024),
        "available_ram_mb": sys.available_memory() / (1024 * 1024),
        "cpu_name": cpu_name,
        "cpu_cores": cpu_cores,
    }))
    .map_err(|e| format!("Serialization error: {}", e))
}

pub(crate) fn get_live_system_metrics(state: &AppState) -> Result<String, String> {
    use sysinfo::{Disks, System};

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let cpu_cores = sys.cpus().len();

    let per_core_usage: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_avg = if cpu_cores > 0 {
        per_core_usage.iter().sum::<f32>() / cpu_cores as f32
    } else {
        0.0
    };

    let total_ram = sys.total_memory();
    let used_ram = sys.used_memory();
    let available_ram = sys.available_memory();

    let uptime = System::uptime();
    let process_count = sys.processes().len();

    // Disk usage for ~/.nexus/ directory
    let nexus_dir = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".nexus"))
        .unwrap_or_default();
    let nexus_disk_bytes: u64 = if nexus_dir.exists() {
        fn dir_size(path: &std::path::Path) -> u64 {
            let mut total = 0u64;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        total += dir_size(&p);
                    } else if let Ok(meta) = p.metadata() {
                        total += meta.len();
                    }
                }
            }
            total
        }
        dir_size(&nexus_dir)
    } else {
        0
    };

    // Total disk info from sysinfo
    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_available) = disks
        .list()
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| (d.total_space(), d.available_space()))
        .unwrap_or_else(|| {
            disks
                .list()
                .first()
                .map(|d| (d.total_space(), d.available_space()))
                .unwrap_or((0, 0))
        });

    // Per-agent fuel from Supervisor
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let meta_guard = match state.meta.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let statuses = supervisor.health_check();
    let mut agent_fuel = Vec::new();
    for status in &statuses {
        let name = meta_guard
            .get(&status.id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| status.id.to_string());
        let (fuel_budget, fuel_used) = if let Some(report) = supervisor.fuel_audit_report(status.id)
        {
            (report.cap_units, report.spent_units)
        } else {
            let budget = supervisor
                .get_agent(status.id)
                .map(|h| h.manifest.fuel_budget)
                .unwrap_or(0);
            let remaining = status.remaining_fuel;
            (budget, budget.saturating_sub(remaining))
        };
        agent_fuel.push(json!({
            "id": status.id.to_string(),
            "name": name,
            "state": status.state.to_string(),
            "fuel_budget": fuel_budget,
            "fuel_used": fuel_used,
            "remaining_fuel": status.remaining_fuel,
        }));
    }

    serde_json::to_string(&json!({
        "cpu_name": cpu_name,
        "cpu_cores": cpu_cores,
        "cpu_avg": (cpu_avg * 10.0).round() / 10.0,
        "per_core_usage": per_core_usage,
        "total_ram": total_ram,
        "used_ram": used_ram,
        "available_ram": available_ram,
        "uptime_secs": uptime,
        "process_count": process_count,
        "nexus_disk_bytes": nexus_disk_bytes,
        "disk_total": disk_total,
        "disk_available": disk_available,
        "agents": agent_fuel,
    }))
    .map_err(|e| format!("Serialization error: {}", e))
}

// ---------------------------------------------------------------------------
// Time Machine commands
// ---------------------------------------------------------------------------

pub(crate) fn time_machine_list_checkpoints(state: &AppState) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let checkpoints = supervisor.time_machine().list_checkpoints();
    let summaries: Vec<serde_json::Value> = checkpoints
        .iter()
        .map(|cp| checkpoint_summary_json(state, cp))
        .collect();
    serde_json::to_string(&summaries).map_err(|e| e.to_string())
}

pub(crate) fn time_machine_get_checkpoint(state: &AppState, id: String) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let cp = supervisor
        .time_machine()
        .get_checkpoint(&id)
        .ok_or_else(|| format!("checkpoint not found: {id}"))?;
    serde_json::to_string(&checkpoint_summary_json(state, cp)).map_err(|e| e.to_string())
}

pub(crate) fn time_machine_create_checkpoint(
    state: &AppState,
    label: String,
) -> Result<String, String> {
    let builder = {
        let supervisor = match state.supervisor.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        supervisor.time_machine().begin_checkpoint(&label, None)
    };
    let cp = builder.build();
    let id = commit_time_machine_checkpoint(state, cp)?;

    state.log_event(
        SYSTEM_UUID,
        nexus_kernel::audit::EventType::StateChange,
        json!({ "action": "time_machine.checkpoint_created", "checkpoint_id": id, "label": label }),
    );
    Ok(id)
}

pub(crate) fn time_machine_undo(state: &AppState) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (cp, non_file_actions) = supervisor
        .time_machine_mut()
        .undo()
        .map_err(|e| e.to_string())?;

    let files_restored: Vec<String> = cp
        .changes
        .iter()
        .filter_map(|c| match c {
            nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    let agents_affected: Vec<String> = non_file_actions
        .iter()
        .filter_map(|a| match a {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        })
        .collect();
    let actions_applied = files_restored.len() + non_file_actions.len();

    drop(supervisor);
    apply_non_file_undo_actions(state, &non_file_actions);

    state.log_event(
        SYSTEM_UUID,
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.undo",
            "checkpoint_id": cp.id,
            "label": cp.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": cp.id,
        "label": cp.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn time_machine_undo_checkpoint(state: &AppState, id: String) -> Result<String, String> {
    let mut files_restored = Vec::new();
    let mut agents_affected = Vec::new();
    let mut actions_applied = 0usize;
    let selected_checkpoint = {
        let supervisor = match state.supervisor.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        supervisor
            .time_machine()
            .get_checkpoint(&id)
            .cloned()
            .ok_or_else(|| format!("checkpoint not found: {id}"))?
    };

    loop {
        let latest_active = {
            let supervisor = match state.supervisor.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            supervisor
                .time_machine()
                .list_checkpoints()
                .iter()
                .rev()
                .find(|cp| !cp.undone)
                .cloned()
        };
        let Some(latest_active) = latest_active else {
            break;
        };
        if latest_active.id == id {
            break;
        }

        let (cp, non_file_actions) = {
            let mut supervisor = match state.supervisor.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            supervisor
                .time_machine_mut()
                .undo()
                .map_err(|e| e.to_string())?
        };
        let current_files: Vec<String> = cp
            .changes
            .iter()
            .filter_map(|c| match c {
                nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
                | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
                | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect();
        let current_agents: Vec<String> = non_file_actions
            .iter()
            .filter_map(|a| match a {
                nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                    Some(agent_id.clone())
                }
                _ => None,
            })
            .collect();
        actions_applied += current_files.len() + non_file_actions.len();
        files_restored.extend(current_files);
        agents_affected.extend(current_agents);
        apply_non_file_undo_actions(state, &non_file_actions);
    }

    state.log_event(
        SYSTEM_UUID,
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.undo_checkpoint",
            "checkpoint_id": selected_checkpoint.id,
            "label": selected_checkpoint.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": selected_checkpoint.id,
        "label": selected_checkpoint.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn time_machine_redo(state: &AppState) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (cp, non_file_actions) = supervisor
        .time_machine_mut()
        .redo()
        .map_err(|e| e.to_string())?;

    let files_restored: Vec<String> = cp
        .changes
        .iter()
        .filter_map(|c| match c {
            nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    let agents_affected: Vec<String> = non_file_actions
        .iter()
        .filter_map(|a| match a {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        })
        .collect();
    let actions_applied = files_restored.len() + non_file_actions.len();

    drop(supervisor);
    apply_non_file_undo_actions(state, &non_file_actions);

    state.log_event(
        SYSTEM_UUID,
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.redo",
            "checkpoint_id": cp.id,
            "label": cp.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": cp.id,
        "label": cp.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn time_machine_get_diff(state: &AppState, id: String) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let cp = supervisor
        .time_machine()
        .get_checkpoint(&id)
        .ok_or_else(|| format!("checkpoint not found: {id}"))?;

    let diffs: Vec<serde_json::Value> = cp
        .changes
        .iter()
        .map(|entry| match entry {
            nexus_kernel::time_machine::ChangeEntry::FileWrite {
                path,
                before,
                after,
            } => json!({
                "path": path,
                "change_type": "modify",
                "size_before": before.as_ref().map(|b| b.len()).unwrap_or(0),
                "size_after": after.len(),
            }),
            nexus_kernel::time_machine::ChangeEntry::FileCreate { path, after } => json!({
                "path": path,
                "change_type": "create",
                "size_before": 0,
                "size_after": after.len(),
            }),
            nexus_kernel::time_machine::ChangeEntry::FileDelete { path, before } => json!({
                "path": path,
                "change_type": "delete",
                "size_before": before.len(),
                "size_after": 0,
            }),
            nexus_kernel::time_machine::ChangeEntry::AgentStateChange {
                agent_id,
                field,
                before,
                after,
            } => json!({
                "path": format!("agent://{agent_id}/{field}"),
                "change_type": "modify",
                "before_value": before,
                "after_value": after,
            }),
            nexus_kernel::time_machine::ChangeEntry::ConfigChange { key, before, after } => json!({
                "path": format!("config://{key}"),
                "change_type": "modify",
                "before_value": before,
                "after_value": after,
            }),
        })
        .collect();
    serde_json::to_string(&diffs).map_err(|e| e.to_string())
}

pub(crate) fn time_machine_what_if(
    state: &AppState,
    id: String,
    variable_key: String,
    variable_value: String,
) -> Result<String, String> {
    let rewind_raw = time_machine_undo_checkpoint(state, id.clone())?;
    let rewind_result: serde_json::Value =
        serde_json::from_str(&rewind_raw).map_err(|e| e.to_string())?;

    if let Some(path) = variable_key.strip_prefix("agent://") {
        if let Some((agent_id, field)) = path.split_once('/') {
            if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
                let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(handle) = supervisor.get_agent_mut(agent_uuid) {
                    match field {
                        "fuel_remaining" => {
                            let fuel = variable_value.parse::<u64>().map_err(|e| e.to_string())?;
                            handle.remaining_fuel = fuel;
                        }
                        "status" => {
                            handle.state = parse_agent_state(&variable_value)
                                .ok_or_else(|| format!("invalid agent status: {variable_value}"))?;
                        }
                        _ => {}
                    }
                }
            }
        }
    } else if variable_key == "governance.enable_warden_review" {
        let mut config = load_config().map_err(agent_error)?;
        config.governance.enable_warden_review =
            matches!(variable_value.as_str(), "true" | "1" | "yes" | "on");
        save_nexus_config(&config).map_err(agent_error)?;
    }

    let mut replayed = 0usize;
    while time_machine_redo(state).is_ok() {
        replayed += 1;
    }

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "action": "time_machine.what_if",
            "checkpoint_id": id,
            "variable_key": variable_key,
            "variable_value": variable_value,
            "replayed_checkpoints": replayed,
        }),
    );

    serde_json::to_string(&json!({
        "rewind": rewind_result,
        "replayed_checkpoints": replayed,
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn checkpoint_summary_json(
    state: &AppState,
    checkpoint: &nexus_kernel::time_machine::Checkpoint,
) -> serde_json::Value {
    let agent_name = checkpoint.agent_id.as_deref().and_then(|agent_id| {
        // Optional: agent_id may not be a valid UUID
        let uuid = Uuid::parse_str(agent_id).ok()?;
        state
            .meta
            .lock()
            .ok() // Optional: skip name lookup if meta mutex is poisoned
            .and_then(|meta| meta.get(&uuid).map(|entry| entry.name.clone()))
    });
    let action = checkpoint
        .changes
        .iter()
        .find_map(|change| match change {
            nexus_kernel::time_machine::ChangeEntry::ConfigChange { key, after, .. }
                if key == "action" =>
            {
                after.as_str().map(str::to_string)
            }
            _ => None,
        })
        .unwrap_or_else(|| checkpoint.label.clone());
    let state_hash = checkpoint
        .changes
        .iter()
        .find_map(|change| match change {
            nexus_kernel::time_machine::ChangeEntry::ConfigChange { key, after, .. }
                if key == "state_hash" =>
            {
                after.as_str().map(str::to_string)
            }
            _ => None,
        })
        .unwrap_or_else(|| format!("{:x}", sha2::Sha256::digest(checkpoint.label.as_bytes())));

    json!({
        "id": checkpoint.id,
        "label": checkpoint.label,
        "timestamp": checkpoint.timestamp,
        "agent_id": checkpoint.agent_id,
        "agent_name": agent_name,
        "action": action,
        "state_hash": state_hash,
        "change_count": checkpoint.changes.len(),
        "undone": checkpoint.undone,
    })
}

// ── Nexus Link (peer-to-peer model sharing) ─────────────────────────────

pub(crate) fn nexus_link_status(state: &AppState) -> Result<String, String> {
    let link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let local_model_count = link.get_local_models().unwrap_or_default().len();
    let result = json!({
        "device_id": link.device_id(),
        "device_name": link.device_name(),
        "sharing_enabled": link.sharing_enabled(),
        "peer_count": link.list_peers().len(),
        "local_model_count": local_model_count,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn nexus_link_toggle_sharing(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    if enabled {
        link.enable_sharing();
    } else {
        link.disable_sharing();
    }
    let result = json!({ "sharing_enabled": link.sharing_enabled() });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn nexus_link_add_peer(
    state: &AppState,
    address: String,
    name: String,
) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let peer = link.add_peer(&address, &name);
    serde_json::to_string(&peer).map_err(|e| e.to_string())
}

pub(crate) fn nexus_link_remove_peer(
    state: &AppState,
    device_id: String,
) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let removed = link.remove_peer(&device_id);
    let result = json!({ "removed": removed });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn nexus_link_list_peers(state: &AppState) -> Result<String, String> {
    let link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(link.list_peers()).map_err(|e| e.to_string())
}

// ── Evolution engine (self-improving agent strategies) ───────────────────

pub(crate) fn evolution_get_status(state: &AppState) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    let result = json!({
        "enabled": engine.config().enabled,
        "total_strategies": engine.total_strategies(),
        "active_agents": engine.active_agent_count(),
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn evolution_register_strategy(
    state: &AppState,
    agent_id: String,
    name: String,
    parameters: String,
) -> Result<String, String> {
    let params: serde_json::Value =
        serde_json::from_str(&parameters).map_err(|e| format!("Invalid parameters JSON: {e}"))?;
    let strategy = Strategy {
        id: uuid::Uuid::new_v4().to_string(),
        version: 1,
        agent_id,
        name,
        parameters: params,
        score: 0.0,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        parent_id: None,
    };
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    engine.register_strategy(strategy.clone())?;
    serde_json::to_string(&strategy).map_err(|e| e.to_string())
}

pub(crate) fn evolution_evolve_once(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    // Simple scoring: count non-null fields in parameters
    let result = engine.evolve_once(&agent_id, MutationType::ParameterTweak, |s| {
        let param_count = s.parameters.as_object().map(|o| o.len()).unwrap_or(0);
        (param_count as f64 * 0.1).min(1.0)
    })?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn evolution_get_history(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    match engine.get_history(&agent_id) {
        Some(history) => serde_json::to_string(history).map_err(|e| e.to_string()),
        None => Ok(json!({
            "agent_id": agent_id,
            "total_generations": 0,
            "total_improvements": 0,
            "total_regressions": 0,
            "current_best_score": 0.0,
            "results": []
        })
        .to_string()),
    }
}

pub(crate) fn evolution_rollback(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    let strategy = engine.rollback(&agent_id)?;
    serde_json::to_string(&strategy).map_err(|e| e.to_string())
}

pub(crate) fn evolution_get_active_strategy(
    state: &AppState,
    agent_id: String,
) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    match engine.get_active_strategy(&agent_id) {
        Some(strategy) => serde_json::to_string(strategy).map_err(|e| e.to_string()),
        None => Err(format!("No active strategy for agent {agent_id}")),
    }
}

// ── Agent DNA / Genome system ────────────────────────────────────────────

pub(crate) fn genome_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/genomes")
}

pub(crate) fn load_genome(agent_id: &str) -> Result<AgentGenome, String> {
    let path = genome_dir().join(format!("{agent_id}.genome.json"));
    if path.exists() {
        let raw = std::fs::read_to_string(&path).map_err(|e| format!("read genome: {e}"))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse genome: {e}"))
    } else {
        // Generate from manifest on the fly
        generate_genome_for(agent_id)
    }
}

pub(crate) fn generate_genome_for(agent_id: &str) -> Result<AgentGenome, String> {
    for path in list_prebuilt_manifest_paths() {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(manifest) = serde_json::from_str::<GenomeJsonManifest>(&raw) {
            if manifest.name == agent_id {
                let genome = genome_from_manifest(&manifest);
                // Best-effort: cache generated genome to disk for future use
                let _ = save_genome(&genome);
                return Ok(genome);
            }
        }
    }
    Err(format!(
        "Agent '{agent_id}' not found in prebuilt manifests"
    ))
}

pub(crate) fn save_genome(genome: &AgentGenome) -> Result<(), String> {
    let dir = genome_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create genome dir: {e}"))?;
    let path = dir.join(format!("{}.genome.json", genome.agent_id));
    let json =
        serde_json::to_string_pretty(genome).map_err(|e| format!("serialize genome: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write genome: {e}"))
}

pub(crate) fn get_agent_genome(_state: &AppState, agent_id: String) -> Result<String, String> {
    let genome = load_genome(&agent_id)?;
    serde_json::to_string_pretty(&genome).map_err(|e| e.to_string())
}

pub(crate) fn breed_agents(
    state: &AppState,
    parent_a: String,
    parent_b: String,
) -> Result<String, String> {
    let genome_a = load_genome(&parent_a)?;
    let genome_b = load_genome(&parent_b)?;

    let mut offspring = crossover(&genome_a, &genome_b);

    // Use LLM to breed the system prompts (if a real provider is available)
    let bred_prompt = breed_system_prompts_via_llm(
        &genome_a.genes.personality.system_prompt,
        &genome_b.genes.personality.system_prompt,
    );
    set_offspring_prompt(&mut offspring, bred_prompt);

    // Save the offspring genome
    save_genome(&offspring)?;

    // Register the offspring as a real agent in the supervisor
    let manifest_json = json!({
        "name": offspring.agent_id,
        "version": offspring.genome_version,
        "description": offspring.genes.personality.system_prompt,
        "capabilities": offspring.genes.capabilities.tools,
        "autonomy_level": offspring.genes.autonomy.level,
        "fuel_budget": 10000,
    });
    // Best-effort: register bred offspring as a new agent; breeding result is logged regardless
    let _ = create_agent(state, manifest_json.to_string());

    state.log_event(
        uuid::Uuid::new_v4(),
        EventType::UserAction,
        json!({
            "event": "agent_bred",
            "parent_a": parent_a,
            "parent_b": parent_b,
            "offspring": offspring.agent_id,
            "generation": offspring.generation,
        }),
    );

    serde_json::to_string_pretty(&offspring).map_err(|e| e.to_string())
}

pub(crate) fn mutate_agent(_state: &AppState, agent_id: String) -> Result<String, String> {
    let genome = load_genome(&agent_id)?;
    let child = mutate(&genome);
    save_genome(&child)?;
    serde_json::to_string_pretty(&child).map_err(|e| e.to_string())
}

pub(crate) fn get_agent_lineage(_state: &AppState, agent_id: String) -> Result<String, String> {
    let genome = load_genome(&agent_id)?;
    let mut lineage = Vec::new();

    // Load each ancestor genome
    for ancestor_id in &genome.genes.evolution.lineage {
        if let Ok(ancestor) = load_genome(ancestor_id) {
            lineage.push(ancestor);
        }
    }

    // Add the current agent
    lineage.push(genome);

    serde_json::to_string_pretty(&lineage).map_err(|e| e.to_string())
}

pub(crate) fn generate_all_genomes(_state: &AppState) -> Result<String, String> {
    let mut generated = 0;
    let mut errors = 0;

    for path in list_prebuilt_manifest_paths() {
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let manifest = match serde_json::from_str::<GenomeJsonManifest>(&raw) {
            Ok(m) => m,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let genome = genome_from_manifest(&manifest);
        if save_genome(&genome).is_ok() {
            generated += 1;
        } else {
            errors += 1;
        }
    }

    Ok(json!({
        "generated": generated,
        "errors": errors,
    })
    .to_string())
}
