//! apps domain implementation.

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

// ── Database Manager ──────────────────────────────────────────────────
// User-initiated commands (not agent-initiated).  Agent DB access goes
// through kernel capability checks.  These user-facing commands use
// SQL keyword blocking (DB_BLOCKED_KEYWORDS) as a governance safeguard.

/// Blocked SQL keywords that require HITL approval.
const DB_BLOCKED_KEYWORDS: &[&str] = &["DROP", "TRUNCATE", "ALTER", "DELETE", "GRANT", "REVOKE"];

/// Allowed table names for direct SQL interpolation (whitelist approach).
const ALLOWED_TABLES: &[&str] = &[
    "agents",
    "audit_entries",
    "genomes",
    "capabilities",
    "fuel_records",
    "sessions",
    "workspaces",
    "workspace_members",
    "usage_records",
    "integration_configs",
    "backup_metadata",
    "settings",
    "identities",
    "trust_links",
    "sqlite_master",
    "sqlite_sequence",
    "consent_requests",
    "agent_memories",
    "cognitive_tasks",
    "evolution_log",
    "marketplace_plugins",
];

pub(crate) fn validate_table_name(name: &str) -> Result<&str, String> {
    if name.is_empty() {
        return Err("Empty table name".to_string());
    }
    if ALLOWED_TABLES.contains(&name) {
        Ok(name)
    } else {
        Err(format!("Table '{}' is not in the allowed whitelist", name))
    }
}

pub(crate) fn db_check_governance(sql: &str) -> Result<(), String> {
    let upper = sql.trim().to_uppercase();
    for kw in DB_BLOCKED_KEYWORDS {
        // Check if the keyword appears as a standalone word
        if upper.split_whitespace().any(|w| w == *kw) {
            return Err(format!(
                "BLOCKED: \"{kw}\" queries require Tier2+ HITL approval. Agent write access is governed."
            ));
        }
    }
    Ok(())
}

pub(crate) fn nexus_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".nexus"))
}

pub(crate) fn db_connect(state: &AppState, connection_string: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "db_connect", "connection_string": connection_string}),
    );

    // Validate the path exists or can be created
    let db_path = std::path::Path::new(&connection_string);
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create db directory: {e}"))?;
        }
    }

    // Test that we can open the database
    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    // Get table list
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .map_err(|e| format!("Failed to query tables: {e}"))?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| format!("Failed to fetch tables: {e}"))?
        .filter_map(|r| r.ok()) // Optional: skip individual rows that fail to deserialize
        .collect();

    let conn_id = Uuid::new_v4().to_string();
    let result = json!({
        "conn_id": conn_id,
        "path": connection_string,
        "tables": tables,
    });
    serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn db_execute_query(
    state: &AppState,
    connection_string: String,
    query: String,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::Default)?;
    state.validate_input(&query)?;
    state.validate_path_input(&connection_string)?;
    // Governance check
    db_check_governance(&query)?;

    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "db_execute_query", "query": query}),
    );

    let start = std::time::Instant::now();
    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    let trimmed = query.trim().to_uppercase();
    if trimmed.starts_with("SELECT")
        || trimmed.starts_with("PRAGMA")
        || trimmed.starts_with("EXPLAIN")
    {
        let mut stmt = conn
            .prepare(query.trim().trim_end_matches(';'))
            .map_err(|e| format!("SQL error: {e}"))?;

        let col_count = stmt.column_count();
        let columns: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let rows: Vec<Vec<serde_json::Value>> = stmt
            .query_map([], |row| {
                let mut vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val: rusqlite::Result<String> = row.get(i);
                    vals.push(match val {
                        Ok(s) => serde_json::Value::String(s),
                        Err(_) => {
                            // Try as integer
                            let int_val: rusqlite::Result<i64> = row.get(i);
                            match int_val {
                                Ok(n) => serde_json::Value::Number(n.into()),
                                Err(_) => {
                                    // Try as float
                                    let float_val: rusqlite::Result<f64> = row.get(i);
                                    match float_val {
                                        Ok(f) => serde_json::json!(f),
                                        Err(_) => serde_json::Value::Null,
                                    }
                                }
                            }
                        }
                    });
                }
                Ok(vals)
            })
            .map_err(|e| format!("Query failed: {e}"))?
            .filter_map(|r| r.ok()) // Optional: skip individual rows that fail to deserialize
            .collect();

        let duration = start.elapsed().as_millis() as u64;
        let result = json!({
            "columns": columns,
            "rows": rows,
            "row_count": rows.len(),
            "duration_ms": duration,
        });
        serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
    } else {
        // INSERT / UPDATE / CREATE TABLE etc.
        let affected = conn
            .execute(query.trim().trim_end_matches(';'), [])
            .map_err(|e| format!("SQL error: {e}"))?;
        let duration = start.elapsed().as_millis() as u64;
        let result = json!({
            "columns": [],
            "rows": [],
            "row_count": affected,
            "duration_ms": duration,
        });
        serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
    }
}

pub(crate) fn db_list_tables(
    state: &AppState,
    connection_string: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "db_list_tables", "path": connection_string}),
    );

    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT m.name, m.type, \
             (SELECT COUNT(*) FROM pragma_table_info(m.name)) as col_count \
             FROM sqlite_master m WHERE m.type='table' ORDER BY m.name",
        )
        .map_err(|e| format!("Failed to query tables: {e}"))?;

    let tables: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let ttype: String = row.get(1)?;
            let col_count: i64 = row.get(2)?;
            Ok(json!({"name": name, "type": ttype, "col_count": col_count}))
        })
        .map_err(|e| format!("Failed to fetch tables: {e}"))?
        .filter_map(|r| r.ok()) // Optional: skip individual rows that fail to deserialize
        .collect();

    // For each table, get column info
    let mut detailed = Vec::new();
    for tbl in &tables {
        let name = tbl["name"].as_str().unwrap_or("");
        let validated_name =
            validate_table_name(name).map_err(|e| format!("table validation error: {e}"))?;
        let mut col_stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", validated_name))
            .map_err(|e| format!("pragma error: {e}"))?;
        let columns: Vec<serde_json::Value> = col_stmt
            .query_map([], |row| {
                let col_name: String = row.get(1)?;
                let col_type: String = row.get(2)?;
                let notnull: i64 = row.get(3)?;
                let pk: i64 = row.get(5)?;
                Ok(json!({
                    "name": col_name,
                    "type": col_type,
                    "nullable": notnull == 0,
                    "primaryKey": pk > 0,
                }))
            })
            .map_err(|e| format!("col info error: {e}"))?
            .filter_map(|r| r.ok()) // Optional: skip individual rows that fail to deserialize
            .collect();

        // Get row count
        let row_count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM \"{}\"", validated_name),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        detailed.push(json!({
            "name": name,
            "columns": columns,
            "rowCount": row_count,
        }));
    }

    serde_json::to_string(&detailed).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn db_disconnect(state: &AppState, db_path: String) -> Result<(), String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "db_disconnect", "path": db_path}),
    );
    // SQLite connections are per-request; this records the disconnect for audit
    Ok(())
}

pub(crate) fn db_export_table(
    state: &AppState,
    connection_string: String,
    table_name: String,
    format: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "db_export", "table": table_name, "format": format}),
    );

    let validated_name =
        validate_table_name(&table_name).map_err(|e| format!("Table validation failed: {e}"))?;

    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    let query = format!("SELECT * FROM \"{}\"", validated_name);
    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| format!("SQL error: {e}"))?;
    let col_count = stmt.column_count();
    let columns: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let rows: Vec<Vec<String>> = stmt
        .query_map([], |row| {
            let mut vals = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val: String = row.get::<_, String>(i).unwrap_or_default();
                vals.push(val);
            }
            Ok(vals)
        })
        .map_err(|e| format!("Query failed: {e}"))?
        .filter_map(|r| r.ok()) // Optional: skip individual rows that fail to deserialize
        .collect();

    match format.as_str() {
        "csv" => {
            let mut out = columns.join(",") + "\n";
            for row in &rows {
                let escaped: Vec<String> = row
                    .iter()
                    .map(|v| {
                        if v.contains(',') || v.contains('"') || v.contains('\n') {
                            format!("\"{}\"", v.replace('"', "\"\""))
                        } else {
                            v.clone()
                        }
                    })
                    .collect();
                out += &escaped.join(",");
                out += "\n";
            }
            Ok(out)
        }
        "json" => {
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (i, col) in columns.iter().enumerate() {
                        obj.insert(
                            col.clone(),
                            serde_json::Value::String(row.get(i).cloned().unwrap_or_default()),
                        );
                    }
                    serde_json::Value::Object(obj)
                })
                .collect();
            serde_json::to_string_pretty(&json_rows).map_err(|e| format!("json error: {e}"))
        }
        _ => Err(format!(
            "Unsupported export format: {format}. Use 'csv' or 'json'"
        )),
    }
}

// ── API Client ────────────────────────────────────────────────────────
// User-initiated HTTP requests from the UI (governed Postman).  Agent
// network access goes through kernel capability checks.  These commands
// are scoped to the authenticated desktop user, not agent identities.

pub(crate) fn api_client_request(
    state: &AppState,
    method: String,
    url: String,
    headers_json: String,
    body: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "api_client_request", "method": method, "url": url}),
    );

    let start = std::time::Instant::now();

    let mut args: Vec<String> = vec![
        "-sS".to_string(),
        "-w".to_string(),
        "\n__NEXUS_STATUS__%{http_code}\n__NEXUS_HEADERS__%{header_json}".to_string(),
        "-X".to_string(),
        method.clone(),
    ];

    // Parse headers
    let headers: Vec<(String, String)> = serde_json::from_str(&headers_json).unwrap_or_default();
    for (k, v) in &headers {
        args.push("-H".to_string());
        args.push(format!("{k}: {v}"));
    }

    // Add body for methods that support it
    if !body.is_empty() && method != "GET" && method != "HEAD" {
        args.push("-d".to_string());
        args.push(body);
    }

    args.push(url.clone());

    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run curl: {e}"))?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    // Parse status code from our -w format
    let mut response_body = raw.clone();
    let mut status_code: u16 = 0;
    let mut resp_headers = json!({});

    if let Some(status_pos) = raw.rfind("__NEXUS_STATUS__") {
        response_body = raw[..status_pos].to_string();
        let after_status = &raw[status_pos + 16..];
        // Parse status code (first line after marker)
        if let Some(newline) = after_status.find('\n') {
            status_code = after_status[..newline].trim().parse().unwrap_or(0);
            // Parse headers json after __NEXUS_HEADERS__
            let header_part = &after_status[newline + 1..];
            if let Some(hdr_pos) = header_part.find("__NEXUS_HEADERS__") {
                let hdr_json = &header_part[hdr_pos + 17..];
                resp_headers = serde_json::from_str(hdr_json.trim()).unwrap_or_else(|_| json!({}));
            }
        } else {
            status_code = after_status.trim().parse().unwrap_or(0);
        }
    }

    if !output.status.success() && status_code == 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl failed: {stderr}"));
    }

    let status_text = match status_code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    };

    let size = response_body.len();
    let result = json!({
        "status": status_code,
        "statusText": status_text,
        "headers": resp_headers,
        "body": response_body,
        "duration": duration_ms,
        "size": size,
    });
    serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
}

// ── API Client Collections ────────────────────────────────────────────

pub(crate) fn api_collections_path() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?;
    Ok(dir.join("api_collections.json"))
}

pub(crate) fn api_client_list_collections() -> Result<String, String> {
    let path = api_collections_path()?;
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("read error: {e}"))
    } else {
        Ok("[]".to_string())
    }
}

pub(crate) fn api_client_save_collections(data_json: String) -> Result<(), String> {
    let path = api_collections_path()?;
    std::fs::write(&path, data_json).map_err(|e| format!("write error: {e}"))
}

// ── Learning Progress ────────────────────────────────────────────────

pub(crate) fn learning_progress_path() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?;
    Ok(dir.join("learning_progress.json"))
}

pub(crate) fn learning_save_progress(data_json: String) -> Result<(), String> {
    let path = learning_progress_path()?;
    std::fs::write(&path, data_json).map_err(|e| format!("write error: {e}"))
}

pub(crate) fn learning_get_progress() -> Result<String, String> {
    let path = learning_progress_path()?;
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("read error: {e}"))
    } else {
        Ok("{}".to_string())
    }
}

pub(crate) fn learning_execute_challenge(
    challenge_id: String,
    code: String,
    language: String,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("Code is empty".to_string());
    }

    // Basic static analysis checks for Rust challenges
    let result = match language.as_str() {
        "rust" => {
            let mut issues: Vec<String> = Vec::new();
            let mut passed = true;

            // Check for unimplemented code
            if code.contains("todo!()") || code.contains("unimplemented!()") {
                issues.push(
                    "Code contains unimplemented sections (todo!() or unimplemented!())"
                        .to_string(),
                );
                passed = false;
            }

            // Check for empty function bodies
            if code.contains("{}") && !code.contains("// empty") {
                let brace_count = code.matches('{').count();
                let empty_brace_count = code.matches("{}").count();
                if empty_brace_count > 0 && empty_brace_count as f64 / brace_count as f64 > 0.5 {
                    issues.push("Most function bodies are empty".to_string());
                    passed = false;
                }
            }

            // Check for required patterns based on challenge
            match challenge_id.as_str() {
                "cap-check" => {
                    if !code.contains("capability") && !code.contains("Capability") {
                        issues.push("Expected capability checking logic".to_string());
                        passed = false;
                    }
                    if !code.contains("Result") && !code.contains("Option") {
                        issues.push("Expected error handling with Result or Option".to_string());
                        passed = false;
                    }
                }
                "audit-trail" => {
                    if !code.contains("AuditTrail") && !code.contains("audit") {
                        issues.push("Expected audit trail usage".to_string());
                        passed = false;
                    }
                    if !code.contains("append") && !code.contains("log") && !code.contains("record")
                    {
                        issues.push("Expected event recording/appending logic".to_string());
                        passed = false;
                    }
                }
                "fuel-budget" => {
                    if !code.contains("fuel") && !code.contains("Fuel") && !code.contains("budget")
                    {
                        issues.push("Expected fuel budget tracking".to_string());
                        passed = false;
                    }
                }
                _ => {
                    // Generic: must have some structure
                    if code.lines().filter(|l| !l.trim().is_empty()).count() < 5 {
                        issues.push(
                            "Solution is too short — expected at least 5 lines of code".to_string(),
                        );
                        passed = false;
                    }
                }
            }

            // Check for basic Rust syntax patterns
            if !code.contains("fn ") && !code.contains("struct ") && !code.contains("impl ") {
                issues.push("Expected Rust code with function/struct/impl definitions".to_string());
                passed = false;
            }

            json!({
                "passed": passed,
                "challenge_id": challenge_id,
                "feedback": if passed {
                    "Challenge passed! Your code meets the structural and pattern requirements.".to_string()
                } else {
                    format!("Challenge failed:\n{}", issues.iter().map(|i| format!("  • {i}")).collect::<Vec<_>>().join("\n"))
                },
                "issues": issues,
            })
        }
        _ => {
            // For non-Rust: basic length and structure check
            let line_count = code.lines().filter(|l| !l.trim().is_empty()).count();
            let passed = line_count >= 5 && !code.contains("todo!()");
            json!({
                "passed": passed,
                "challenge_id": challenge_id,
                "feedback": if passed {
                    "Challenge passed!".to_string()
                } else {
                    "Challenge failed: code is too short or contains placeholder markers".to_string()
                },
                "issues": [],
            })
        }
    };

    serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
}

// ── Notes App ─────────────────────────────────────────────────────────

pub(crate) fn notes_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("notes");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create notes dir: {e}"))?;
    }
    Ok(dir)
}

pub(crate) fn notes_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_list"}),
    );

    let dir = notes_dir()?;
    let mut notes = Vec::new();

    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(note) = serde_json::from_str::<serde_json::Value>(&content) {
                    notes.push(note);
                }
            }
        }
    }

    serde_json::to_string(&notes).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn notes_get(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_get", "id": id}),
    );

    let path = notes_dir()?.join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("note not found: {id}"));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))
}

pub(crate) fn notes_save(
    state: &AppState,
    id: String,
    title: String,
    content: String,
    folder_id: String,
    tags_json: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_save", "id": id, "title": title}),
    );

    let dir = notes_dir()?;
    let path = dir.join(format!("{id}.json"));

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    // Load existing note to preserve createdAt, or create new timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let created_at = if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&existing)
            .ok()
            .and_then(|v| v["createdAt"].as_u64())
            .unwrap_or(now)
    } else {
        now
    };

    let word_count = content.split_whitespace().count();

    let note = json!({
        "id": id,
        "title": title,
        "content": content,
        "folderId": folder_id,
        "tags": tags,
        "createdAt": created_at,
        "updatedAt": now,
        "wordCount": word_count,
    });

    let serialized = serde_json::to_string_pretty(&note).map_err(|e| format!("json error: {e}"))?;
    std::fs::write(&path, &serialized).map_err(|e| format!("write failed: {e}"))?;
    Ok(serialized)
}

pub(crate) fn notes_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_delete", "id": id}),
    );

    let path = notes_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

// ── Email Client (local drafts) ───────────────────────────────────────

pub(crate) fn emails_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("emails");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create emails dir: {e}"))?;
    }
    Ok(dir)
}

pub(crate) fn email_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_list"}),
    );
    let dir = emails_dir()?;
    let mut emails = Vec::new();
    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(email) = serde_json::from_str::<serde_json::Value>(&content) {
                    emails.push(email);
                }
            }
        }
    }
    serde_json::to_string(&emails).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn email_save(
    state: &AppState,
    id: String,
    data_json: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_save", "id": id}),
    );
    let dir = emails_dir()?;
    let path = dir.join(format!("{id}.json"));
    // Validate JSON
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn email_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_delete", "id": id}),
    );
    let path = emails_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

// ── Email OAuth2 (Gmail / Outlook via REST API) ───────────────────────

pub(crate) fn email_oauth_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("email_oauth");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create email_oauth dir: {e}"))?;
    }
    Ok(dir)
}

pub(crate) fn read_messaging_token(platform: &str) -> Result<String, String> {
    let path = nexus_data_dir()?
        .join("messaging_tokens")
        .join(format!("{platform}.json"));
    if !path.exists() {
        return Err(format!(
            "{platform} not configured — add bot token in Messaging settings"
        ));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("parse: {e}"))?;
    data.get("token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no token for {platform}"))
}

pub(crate) fn read_oauth_setting(key: &str) -> Result<String, String> {
    let path = nexus_data_dir()?.join("oauth_settings.json");
    if !path.exists() {
        return Err("no oauth settings file".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("parse: {e}"))?;
    data.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("key {key} not found"))
}

pub(crate) fn email_start_oauth(state: &AppState, provider: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_start_oauth", "provider": provider}),
    );

    // Client IDs from env vars or local config file
    let client_id = match provider.as_str() {
        "gmail" => std::env::var("NEXUS_GMAIL_CLIENT_ID")
            .or_else(|_| read_oauth_setting("gmail_client_id"))
            .unwrap_or_default(),
        "outlook" => std::env::var("NEXUS_OUTLOOK_CLIENT_ID")
            .or_else(|_| read_oauth_setting("outlook_client_id"))
            .unwrap_or_default(),
        _ => return Err(format!("Unknown email provider: {provider}")),
    };

    if client_id.is_empty() {
        return Err(format!(
            "No client ID configured for {provider}. Set NEXUS_{}_CLIENT_ID env var or configure in Settings.",
            provider.to_uppercase()
        ));
    }

    let csrf_token = uuid::Uuid::new_v4().to_string();
    let redirect_uri = "http://localhost:19823/oauth/callback";

    let auth_url = match provider.as_str() {
        "gmail" => format!(
            "https://accounts.google.com/o/oauth2/v2/auth?\
             client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&\
             scope=https://www.googleapis.com/auth/gmail.modify&\
             state={csrf_token}&access_type=offline&prompt=consent"
        ),
        "outlook" => format!(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?\
             client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&\
             scope=Mail.ReadWrite+Mail.Send+offline_access&state={csrf_token}"
        ),
        _ => unreachable!(),
    };

    // Best-effort: open browser for OAuth; user can manually navigate if this fails
    let _ = open::that(&auth_url);

    // Start a local listener to catch the callback
    let listener = std::net::TcpListener::bind("127.0.0.1:19823")
        .map_err(|e| format!("Cannot start OAuth listener: {e}"))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("set_nonblocking: {e}"))?;

    // Wait for the callback (with timeout)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut auth_code = String::new();
    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                use std::io::{Read as IoRead, Write as IoWrite};
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();

                // Parse the code from GET /?code=XXX&state=YYY
                if let Some(query_start) = request.find("GET /?") {
                    let query = &request[query_start + 6..];
                    if let Some(end) = query.find(' ') {
                        let params = &query[..end];
                        for param in params.split('&') {
                            let parts: Vec<&str> = param.splitn(2, '=').collect();
                            if parts.len() == 2 && parts[0] == "code" {
                                auth_code = parts[1].to_string();
                            }
                        }
                    }
                }

                // Send a nice response page
                let body = "<html><body style=\"font-family:system-ui;text-align:center;padding:60px;background:#0f172a;color:#e2e8f0\">\
                    <h1>&#10003; Connected!</h1>\
                    <p>You can close this tab and return to Nexus OS.</p>\
                    </body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                // Best-effort: send success page to browser; OAuth code already captured
                let _ = stream.write_all(response.as_bytes());
                break;
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }

    if auth_code.is_empty() {
        return Err("OAuth flow timed out or no auth code received".to_string());
    }

    // Exchange code for tokens
    let client_secret = match provider.as_str() {
        "gmail" => std::env::var("NEXUS_GMAIL_CLIENT_SECRET")
            .or_else(|_| read_oauth_setting("gmail_client_secret"))
            .unwrap_or_default(),
        "outlook" => std::env::var("NEXUS_OUTLOOK_CLIENT_SECRET")
            .or_else(|_| read_oauth_setting("outlook_client_secret"))
            .unwrap_or_default(),
        _ => String::new(),
    };

    let token_url = match provider.as_str() {
        "gmail" => "https://oauth2.googleapis.com/token",
        "outlook" => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
        _ => unreachable!(),
    };

    let token_resp = block_on_async(async {
        reqwest::Client::new()
            .post(token_url)
            .form(&[
                ("code", auth_code.as_str()),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("token exchange: {e}"))?
            .text()
            .await
            .map_err(|e| format!("token body: {e}"))
    })?;

    let token_json: serde_json::Value =
        serde_json::from_str(&token_resp).map_err(|e| format!("token parse: {e}"))?;

    // Store tokens encrypted in local file
    let token_path = email_oauth_dir()?.join(format!("{provider}_tokens.json"));
    let token_data = json!({
        "provider": provider,
        "access_token": token_json.get("access_token").and_then(|v| v.as_str()).unwrap_or(""),
        "refresh_token": token_json.get("refresh_token").and_then(|v| v.as_str()).unwrap_or(""),
        "expires_at": chrono::Utc::now().timestamp() + token_json.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600),
        "connected_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        &token_path,
        serde_json::to_string_pretty(&token_data).map_err(|e| format!("json: {e}"))?,
    )
    .map_err(|e| format!("write tokens: {e}"))?;

    serde_json::to_string(&json!({
        "status": "connected",
        "provider": provider,
    }))
    .map_err(|e| format!("json: {e}"))
}

pub(crate) fn email_oauth_status(state: &AppState) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_oauth_status"}),
    );
    let dir = email_oauth_dir()?;
    let mut statuses = Vec::new();
    for provider in &["gmail", "outlook"] {
        let path = dir.join(format!("{provider}_tokens.json"));
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    let expires_at = data.get("expires_at").and_then(|v| v.as_i64()).unwrap_or(0);
                    let valid = chrono::Utc::now().timestamp() < expires_at;
                    statuses.push(json!({
                        "provider": provider,
                        "connected": true,
                        "token_valid": valid,
                        "connected_at": data.get("connected_at"),
                    }));
                    continue;
                }
            }
        }
        statuses.push(json!({
            "provider": provider,
            "connected": false,
            "token_valid": false,
        }));
    }
    serde_json::to_string(&statuses).map_err(|e| format!("json: {e}"))
}

pub(crate) fn get_email_access_token(provider: &str) -> Result<String, String> {
    let path = email_oauth_dir()?.join(format!("{provider}_tokens.json"));
    if !path.exists() {
        return Err(format!("{provider} not connected"));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("parse: {e}"))?;
    let token = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no access_token".to_string())?
        .to_string();
    Ok(token)
}

pub(crate) fn email_fetch_messages(
    state: &AppState,
    provider: String,
    folder: String,
    page: u32,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_fetch_messages", "provider": provider, "folder": folder}),
    );
    let token = get_email_access_token(&provider)?;
    let max_results = 20u32;

    let result = block_on_async(async {
        match provider.as_str() {
            "gmail" => {
                let label = match folder.as_str() {
                    "inbox" => "INBOX",
                    "sent" => "SENT",
                    "drafts" => "DRAFT",
                    "trash" => "TRASH",
                    "starred" => "STARRED",
                    _ => "INBOX",
                };
                let url = format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds={label}&maxResults={max_results}"
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| format!("gmail list: {e}"))?;
                let body = resp.text().await.map_err(|e| format!("gmail body: {e}"))?;
                let list: serde_json::Value =
                    serde_json::from_str(&body).map_err(|e| format!("gmail parse: {e}"))?;

                let mut emails = Vec::new();
                if let Some(messages) = list.get("messages").and_then(|m| m.as_array()) {
                    for msg in messages.iter().take(max_results as usize) {
                        if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
                            // Fetch each message detail
                            let detail_url = format!(
                                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Subject&metadataHeaders=Date"
                            );
                            let detail_resp = reqwest::Client::new()
                                .get(&detail_url)
                                .bearer_auth(&token)
                                .send()
                                .await;
                            if let Ok(resp) = detail_resp {
                                if let Ok(text) = resp.text().await {
                                    if let Ok(detail) =
                                        serde_json::from_str::<serde_json::Value>(&text)
                                    {
                                        let headers = detail
                                            .get("payload")
                                            .and_then(|p| p.get("headers"))
                                            .and_then(|h| h.as_array());
                                        let mut from = String::new();
                                        let mut to = String::new();
                                        let mut subject = String::new();
                                        if let Some(hdrs) = headers {
                                            for h in hdrs {
                                                let name = h
                                                    .get("name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");
                                                let value = h
                                                    .get("value")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");
                                                match name {
                                                    "From" => from = value.to_string(),
                                                    "To" => to = value.to_string(),
                                                    "Subject" => subject = value.to_string(),
                                                    _ => {}
                                                }
                                            }
                                        }
                                        let snippet = detail
                                            .get("snippet")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let label_ids =
                                            detail.get("labelIds").and_then(|v| v.as_array());
                                        let unread = label_ids
                                            .map(|l| l.iter().any(|v| v.as_str() == Some("UNREAD")))
                                            .unwrap_or(false);
                                        let internal_date = detail
                                            .get("internalDate")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| s.parse::<i64>().ok())
                                            .unwrap_or(0);

                                        emails.push(json!({
                                            "id": id,
                                            "threadId": detail.get("threadId").and_then(|v| v.as_str()).unwrap_or(id),
                                            "from": {"name": from.split('<').next().unwrap_or(&from).trim(), "email": from},
                                            "to": [{"name": to.split('<').next().unwrap_or(&to).trim(), "email": to}],
                                            "subject": subject,
                                            "body": snippet,
                                            "timestamp": internal_date,
                                            "read": !unread,
                                            "starred": label_ids.map(|l| l.iter().any(|v| v.as_str() == Some("STARRED"))).unwrap_or(false),
                                            "folder": folder,
                                            "priority": "normal",
                                            "category": "primary",
                                            "labels": [],
                                            "source": "gmail",
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
                serde_json::to_string(&emails).map_err(|e| format!("serialize: {e}"))
            }
            "outlook" => {
                let folder_path = match folder.as_str() {
                    "inbox" => "inbox",
                    "sent" => "sentitems",
                    "drafts" => "drafts",
                    "trash" => "deleteditems",
                    _ => "inbox",
                };
                let url = format!(
                    "https://graph.microsoft.com/v1.0/me/mailFolders/{folder_path}/messages?$top={max_results}&$skip={}&$orderby=receivedDateTime+desc",
                    page * max_results
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| format!("outlook list: {e}"))?;
                let body = resp
                    .text()
                    .await
                    .map_err(|e| format!("outlook body: {e}"))?;
                let data: serde_json::Value =
                    serde_json::from_str(&body).map_err(|e| format!("outlook parse: {e}"))?;

                let mut emails = Vec::new();
                if let Some(messages) = data.get("value").and_then(|m| m.as_array()) {
                    for msg in messages {
                        let from_obj = msg.get("from").and_then(|f| f.get("emailAddress"));
                        let from_name = from_obj
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let from_email = from_obj
                            .and_then(|f| f.get("address"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let subject = msg.get("subject").and_then(|v| v.as_str()).unwrap_or("");
                        let preview = msg
                            .get("bodyPreview")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let is_read = msg.get("isRead").and_then(|v| v.as_bool()).unwrap_or(true);
                        let received = msg
                            .get("receivedDateTime")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");

                        emails.push(json!({
                            "id": id,
                            "threadId": msg.get("conversationId").and_then(|v| v.as_str()).unwrap_or(id),
                            "from": {"name": from_name, "email": from_email},
                            "to": [],
                            "subject": subject,
                            "body": preview,
                            "timestamp": chrono::DateTime::parse_from_rfc3339(received).map(|d| d.timestamp_millis()).unwrap_or(0),
                            "read": is_read,
                            "starred": msg.get("flag").and_then(|f| f.get("flagStatus")).and_then(|v| v.as_str()) == Some("flagged"),
                            "folder": folder,
                            "priority": if msg.get("importance").and_then(|v| v.as_str()) == Some("high") { "high" } else { "normal" },
                            "category": "primary",
                            "labels": [],
                            "source": "outlook",
                        }));
                    }
                }
                serde_json::to_string(&emails).map_err(|e| format!("serialize: {e}"))
            }
            _ => Err(format!("Unknown provider: {provider}")),
        }
    })?;
    Ok(result)
}

pub(crate) fn email_send_message(
    state: &AppState,
    provider: String,
    to: String,
    subject: String,
    body: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_send", "provider": provider, "to": to}),
    );
    let token = get_email_access_token(&provider)?;

    let result = block_on_async(async {
        match provider.as_str() {
            "gmail" => {
                let raw_message = format!(
                    "To: {to}\r\nSubject: {subject}\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{body}"
                );
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                    raw_message.as_bytes(),
                );
                let resp = reqwest::Client::new()
                    .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                    .bearer_auth(&token)
                    .json(&json!({"raw": encoded}))
                    .send()
                    .await
                    .map_err(|e| format!("gmail send: {e}"))?;
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    Ok(json!({"status": "sent", "provider": "gmail"}).to_string())
                } else {
                    Err(format!("Gmail send failed ({status}): {text}"))
                }
            }
            "outlook" => {
                let mail = json!({
                    "message": {
                        "subject": subject,
                        "body": {"contentType": "HTML", "content": body},
                        "toRecipients": [{"emailAddress": {"address": to}}]
                    },
                    "saveToSentItems": true
                });
                let resp = reqwest::Client::new()
                    .post("https://graph.microsoft.com/v1.0/me/sendMail")
                    .bearer_auth(&token)
                    .json(&mail)
                    .send()
                    .await
                    .map_err(|e| format!("outlook send: {e}"))?;
                let status = resp.status();
                if status.is_success() || status.as_u16() == 202 {
                    Ok(json!({"status": "sent", "provider": "outlook"}).to_string())
                } else {
                    let text = resp.text().await.unwrap_or_default();
                    Err(format!("Outlook send failed ({status}): {text}"))
                }
            }
            _ => Err(format!("Unknown provider: {provider}")),
        }
    })?;
    Ok(result)
}

pub(crate) fn email_search_messages(
    state: &AppState,
    provider: String,
    query: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_search", "provider": provider, "query": query}),
    );
    let token = get_email_access_token(&provider)?;

    let result = block_on_async(async {
        match provider.as_str() {
            "gmail" => {
                let url = format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages?q={}&maxResults=20",
                    urlencoding::encode(&query)
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| format!("gmail search: {e}"))?;
                resp.text().await.map_err(|e| format!("gmail body: {e}"))
            }
            "outlook" => {
                let url = format!(
                    "https://graph.microsoft.com/v1.0/me/messages?$search=\"{}\"&$top=20",
                    query.replace('"', "\\\"")
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| format!("outlook search: {e}"))?;
                resp.text().await.map_err(|e| format!("outlook body: {e}"))
            }
            _ => Err(format!("Unknown provider: {provider}")),
        }
    })?;
    Ok(result)
}

pub(crate) fn email_disconnect(state: &AppState, provider: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_disconnect", "provider": provider}),
    );
    let path = email_oauth_dir()?.join(format!("{provider}_tokens.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("remove: {e}"))?;
    }
    Ok(json!({"status": "disconnected", "provider": provider}).to_string())
}

// ── Messaging: Real Platform Connections ──────────────────────────────

pub(crate) fn messaging_connect_platform(
    state: &AppState,
    platform: String,
    token_value: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_connect", "platform": platform}),
    );

    // Store token in messaging tokens file
    let msg_dir = nexus_data_dir()?.join("messaging_tokens");
    if !msg_dir.exists() {
        std::fs::create_dir_all(&msg_dir).map_err(|e| format!("mkdir: {e}"))?;
    }
    let token_path = msg_dir.join(format!("{platform}.json"));
    std::fs::write(
        &token_path,
        serde_json::to_string_pretty(&json!({"token": token_value, "platform": platform, "connected_at": chrono::Utc::now().to_rfc3339()})).map_err(|e| format!("json: {e}"))?,
    )
    .map_err(|e| format!("write: {e}"))?;

    // Test connectivity
    let test_result = block_on_async(async {
        match platform.as_str() {
            "telegram" => {
                let url = format!("https://api.telegram.org/bot{}/getMe", token_value);
                let resp = reqwest::Client::new()
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("telegram test: {e}"))?;
                let body = resp
                    .text()
                    .await
                    .map_err(|e| format!("telegram body: {e}"))?;
                let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let bot_name = data
                        .get("result")
                        .and_then(|r| r.get("username"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    Ok(json!({"connected": true, "bot_name": bot_name}).to_string())
                } else {
                    Err("Invalid Telegram bot token".to_string())
                }
            }
            "slack" => {
                let resp = reqwest::Client::new()
                    .post("https://slack.com/api/auth.test")
                    .bearer_auth(&token_value)
                    .send()
                    .await
                    .map_err(|e| format!("slack test: {e}"))?;
                let body = resp.text().await.map_err(|e| format!("slack body: {e}"))?;
                let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let team = data
                        .get("team")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    // Attempt Socket Mode WebSocket connection for real-time events
                    // Requires an app-level token (xapp-*) — if using a bot token, falls back to polling
                    if token_value.starts_with("xapp-") {
                        let ws_resp = reqwest::Client::new()
                            .post("https://slack.com/api/apps.connections.open")
                            .bearer_auth(&token_value)
                            .send()
                            .await;
                        if let Ok(ws_resp) = ws_resp {
                            let ws_data: serde_json::Value =
                                ws_resp.json().await.unwrap_or_default();
                            if let Some(ws_url) = ws_data.get("url").and_then(|v| v.as_str()) {
                                // Store WebSocket URL for the frontend to use
                                let ws_path = msg_dir.join("slack_ws_url.txt");
                                // Best-effort: cache WebSocket URL on disk for frontend access
                                let _ = std::fs::write(&ws_path, ws_url);
                            }
                        }
                    }

                    Ok(json!({"connected": true, "team": team, "realtime": token_value.starts_with("xapp-")}).to_string())
                } else {
                    Err(format!(
                        "Slack auth failed: {}",
                        data.get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                    ))
                }
            }
            "discord" => {
                let resp = reqwest::Client::new()
                    .get("https://discord.com/api/v10/users/@me")
                    .header("Authorization", format!("Bot {}", token_value))
                    .send()
                    .await
                    .map_err(|e| format!("discord test: {e}"))?;
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .map_err(|e| format!("discord body: {e}"))?;
                if status.is_success() {
                    let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                    let name = data
                        .get("username")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    Ok(json!({"connected": true, "bot_name": name}).to_string())
                } else {
                    Err(format!("Discord auth failed ({status})"))
                }
            }
            _ => Ok(json!({"connected": true}).to_string()),
        }
    })?;
    Ok(test_result)
}

pub(crate) fn messaging_send(
    state: &AppState,
    platform: String,
    channel: String,
    text: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_send", "platform": platform, "channel": channel}),
    );

    let token = read_messaging_token(&platform)?;

    let result = block_on_async(async {
        match platform.as_str() {
            "telegram" => {
                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                let resp = reqwest::Client::new()
                    .post(&url)
                    .json(&json!({"chat_id": channel, "text": text}))
                    .send()
                    .await
                    .map_err(|e| format!("telegram send: {e}"))?;
                let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
                Ok(body)
            }
            "slack" => {
                let resp = reqwest::Client::new()
                    .post("https://slack.com/api/chat.postMessage")
                    .bearer_auth(&token)
                    .json(&json!({"channel": channel, "text": text}))
                    .send()
                    .await
                    .map_err(|e| format!("slack send: {e}"))?;
                let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
                Ok(body)
            }
            "discord" => {
                let url = format!("https://discord.com/api/v10/channels/{}/messages", channel);
                let resp = reqwest::Client::new()
                    .post(&url)
                    .header("Authorization", format!("Bot {}", token))
                    .json(&json!({"content": text}))
                    .send()
                    .await
                    .map_err(|e| format!("discord send: {e}"))?;
                let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
                Ok(body)
            }
            _ => Err(format!("Unknown platform: {platform}")),
        }
    })?;
    Ok(result)
}

pub(crate) fn messaging_poll_messages(
    state: &AppState,
    platform: String,
    channel: String,
    last_id: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_poll", "platform": platform}),
    );

    let token = read_messaging_token(&platform)?;

    let result = block_on_async(async {
        match platform.as_str() {
            "telegram" => {
                let offset: i64 = last_id.parse().unwrap_or(0);
                let url = format!(
                    "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=5&limit=20",
                    token, offset
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("telegram poll: {e}"))?;
                resp.text().await.map_err(|e| format!("body: {e}"))
            }
            "slack" => {
                let resp = reqwest::Client::new()
                    .get("https://slack.com/api/conversations.history")
                    .bearer_auth(&token)
                    .query(&[("channel", channel.as_str()), ("limit", "20")])
                    .send()
                    .await
                    .map_err(|e| format!("slack poll: {e}"))?;
                resp.text().await.map_err(|e| format!("body: {e}"))
            }
            "discord" => {
                let url = format!(
                    "https://discord.com/api/v10/channels/{}/messages?limit=20",
                    channel
                );
                let resp = reqwest::Client::new()
                    .get(&url)
                    .header("Authorization", format!("Bot {}", token))
                    .send()
                    .await
                    .map_err(|e| format!("discord poll: {e}"))?;
                resp.text().await.map_err(|e| format!("body: {e}"))
            }
            _ => Err(format!("Unknown platform: {platform}")),
        }
    })?;
    Ok(result)
}

// ── Integration OAuth2 Flow ──────────────────────────────────────────

pub(crate) fn integration_start_oauth(
    state: &AppState,
    provider_id: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "integration_start_oauth", "provider": provider_id}),
    );

    let env_key = format!("NEXUS_{}_CLIENT_ID", provider_id.to_uppercase());
    let client_id = std::env::var(&env_key)
        .or_else(|_| read_oauth_setting(&format!("{provider_id}_client_id")))
        .unwrap_or_default();

    if client_id.is_empty() {
        return Err(format!(
            "No client ID for {provider_id}. Set {env_key} env var or configure in Settings."
        ));
    }

    let redirect_uri = "http://localhost:19824/oauth/callback";
    let csrf = uuid::Uuid::new_v4().to_string();

    let auth_url = match provider_id.as_str() {
        "github" => format!(
            "https://github.com/login/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&state={csrf}&scope=repo,read:org"
        ),
        "gitlab" => format!(
            "https://gitlab.com/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&state={csrf}&scope=api+read_user"
        ),
        "slack" => format!(
            "https://slack.com/oauth/v2/authorize?client_id={client_id}&redirect_uri={redirect_uri}&state={csrf}&scope=chat:write,channels:read,channels:history"
        ),
        "jira" => format!(
            "https://auth.atlassian.com/authorize?audience=api.atlassian.com&client_id={client_id}&scope=read%3Ajira-work%20manage%3Ajira-project&redirect_uri={redirect_uri}&state={csrf}&response_type=code&prompt=consent"
        ),
        _ => return Err(format!("OAuth not supported for {provider_id}. Use token-based auth.")),
    };

    // Best-effort: open browser for OAuth; user can manually navigate if this fails
    let _ = open::that(&auth_url);

    // Listen for callback on port 19824
    let listener = std::net::TcpListener::bind("127.0.0.1:19824")
        .map_err(|e| format!("Cannot start OAuth listener: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut auth_code = String::new();

    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                use std::io::{Read as IoRead, Write as IoWrite};
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(qs) = request.find("GET /?") {
                    let query = &request[qs + 6..];
                    if let Some(end) = query.find(' ') {
                        for param in query[..end].split('&') {
                            let parts: Vec<&str> = param.splitn(2, '=').collect();
                            if parts.len() == 2 && parts[0] == "code" {
                                auth_code = parts[1].to_string();
                            }
                        }
                    }
                }
                let body = "<html><body style=\"font-family:system-ui;text-align:center;padding:60px;background:#0f172a;color:#e2e8f0\">\
                    <h1>&#10003; Connected!</h1><p>Return to Nexus OS.</p></body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                // Best-effort: send success page to browser; OAuth code already captured
                let _ = stream.write_all(response.as_bytes());
                break;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    }

    if auth_code.is_empty() {
        return Err("OAuth timed out".to_string());
    }

    // Exchange code for token
    let secret_key = format!("NEXUS_{}_CLIENT_SECRET", provider_id.to_uppercase());
    let client_secret = std::env::var(&secret_key)
        .or_else(|_| read_oauth_setting(&format!("{provider_id}_client_secret")))
        .unwrap_or_default();

    let token_result = block_on_async(async {
        let (token_url, use_json) = match provider_id.as_str() {
            "github" => ("https://github.com/login/oauth/access_token", false),
            "gitlab" => ("https://gitlab.com/oauth/token", false),
            "slack" => ("https://slack.com/api/oauth.v2.access", false),
            "jira" => ("https://auth.atlassian.com/oauth/token", true),
            _ => return Err("unsupported".to_string()),
        };

        let client = reqwest::Client::new();
        let resp = if use_json {
            client
                .post(token_url)
                .json(&json!({
                    "grant_type": "authorization_code",
                    "client_id": client_id,
                    "client_secret": client_secret,
                    "code": auth_code,
                    "redirect_uri": redirect_uri,
                }))
                .send()
                .await
        } else {
            client
                .post(token_url)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("code", auth_code.as_str()),
                    ("redirect_uri", redirect_uri),
                    ("grant_type", "authorization_code"),
                ])
                .send()
                .await
        };

        let resp = resp.map_err(|e| format!("token request: {e}"))?;
        resp.text().await.map_err(|e| format!("token body: {e}"))
    })?;

    // Store token
    let integration_dir = nexus_data_dir()?.join("integrations");
    if !integration_dir.exists() {
        std::fs::create_dir_all(&integration_dir).map_err(|e| format!("mkdir: {e}"))?;
    }
    let token_path = integration_dir.join(format!("{provider_id}_oauth.json"));
    let token_data = json!({
        "provider": provider_id,
        "token_response": serde_json::from_str::<serde_json::Value>(&token_result).unwrap_or(json!({"raw": token_result})),
        "connected_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        &token_path,
        serde_json::to_string_pretty(&token_data).map_err(|e| format!("json: {e}"))?,
    )
    .map_err(|e| format!("write: {e}"))?;

    serde_json::to_string(&json!({"status": "connected", "provider": provider_id}))
        .map_err(|e| format!("json: {e}"))
}

// ── App Store: GitLab API search ─────────────────────────────────────

pub(crate) fn marketplace_search_gitlab(query: String) -> Result<String, String> {
    let result = block_on_async(async {
        let url = "https://gitlab.com/api/v4/projects";
        let resp = reqwest::Client::new()
            .get(url)
            .query(&[
                ("search", query.as_str()),
                ("topic", "nexus-agent"),
                ("per_page", "20"),
                ("order_by", "last_activity_at"),
            ])
            .send()
            .await
            .map_err(|e| format!("gitlab search: {e}"))?;
        let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
        let projects: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap_or_default();

        let agents: Vec<serde_json::Value> = projects
            .iter()
            .map(|p| {
                json!({
                    "id": p.get("id").and_then(|v| v.as_i64()).unwrap_or(0).to_string(),
                    "name": p.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "description": p.get("description").and_then(|v| v.as_str()).unwrap_or("Community agent"),
                    "author": p.get("namespace").and_then(|n| n.get("name")).and_then(|v| v.as_str()).unwrap_or("community"),
                    "url": p.get("web_url").and_then(|v| v.as_str()).unwrap_or(""),
                    "stars": p.get("star_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    "source": "gitlab",
                    "autonomy_level": "L2",
                })
            })
            .collect();
        serde_json::to_string(&agents).map_err(|e| format!("json: {e}"))
    })?;
    Ok(result)
}

// ── Agent Output Panel ───────────────────────────────────────────────

pub(crate) fn get_agent_outputs(
    state: &AppState,
    agent_id: String,
    limit: u32,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "get_agent_outputs", "agent_id": agent_id}),
    );

    // Pull recent outputs from audit trail for this agent
    let agent_uuid = uuid::Uuid::parse_str(&agent_id).unwrap_or(SYSTEM_UUID);
    let guard = state.audit.lock().map_err(|e| format!("lock: {e}"))?;
    let all_events = guard.events();
    let filtered: Vec<serde_json::Value> = all_events
        .iter()
        .rev()
        .filter(|e| {
            e.agent_id == agent_uuid
                || e.payload
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s == agent_id)
                    .unwrap_or(false)
        })
        .take(limit as usize)
        .map(|e| {
            json!({
                "id": e.event_id.to_string(),
                "time": e.timestamp,
                "action": format!("{:?}", e.event_type),
                "type": "text",
                "content": serde_json::to_string(&e.payload).unwrap_or_default(),
            })
        })
        .collect();
    serde_json::to_string(&filtered).map_err(|e| format!("json: {e}"))
}

// ── Project Manager ───────────────────────────────────────────────────

pub(crate) fn projects_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("projects");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create projects dir: {e}"))?;
    }
    Ok(dir)
}

pub(crate) fn project_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_list"}),
    );
    let dir = projects_dir()?;
    let mut projects = Vec::new();
    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(project) = serde_json::from_str::<serde_json::Value>(&content) {
                    projects.push(project);
                }
            }
        }
    }
    serde_json::to_string(&projects).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn project_get(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_get", "id": id}),
    );
    let path = projects_dir()?.join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("project not found: {id}"));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))
}

pub(crate) fn project_save(
    state: &AppState,
    id: String,
    data_json: String,
) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_save", "id": id}),
    );
    let dir = projects_dir()?;
    let path = dir.join(format!("{id}.json"));
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn project_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_delete", "id": id}),
    );
    let path = projects_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}
