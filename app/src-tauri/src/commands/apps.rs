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
    capture_and_store_screen, ComputerControlEngine, InputControlStatus, ScreenRegion,
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
use std::path::{Path, PathBuf};
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

/// The Nexus state directory under the validated identity home (P0-002C5B).
pub(crate) fn nexus_data_dir() -> Result<PathBuf, String> {
    nexus_kernel::identity_home::nexus_state_dir().map_err(|e| e.to_string())
}

// ── Caller identifiers (P0-002C5B) ────────────────────────────────────
//
// A caller identifier is never a path. Notes and projects accept only the
// identifier grammar; email messages map any provider id to a deterministic
// storage stem; providers and platforms come from explicit allowlists checked
// before any token file is named. A refusal records only the operation, the
// outcome and a reason class, never the rejected value.

/// Longest note or project identifier.
const MAX_STORE_ID_BYTES: usize = 128;

fn deny(state: &AppState, action: &str, reason: &str) -> String {
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": action, "outcome": "denied", "reason": reason}),
    );
    format!("{action}: {}", reason.replace('_', " "))
}

/// The JSON file for a caller identifier beneath a store directory. Note and
/// project ids are backend-grammar storage identifiers (the interface issues
/// `n-<millis>` and `default`): lowercase only, so no second spelling of an
/// id can name the same file on a case-insensitive filesystem.
fn identified_file(
    state: &AppState,
    action: &str,
    dir: &Path,
    id: &str,
) -> Result<PathBuf, String> {
    nexus_kernel::governed_path::validate_storage_identifier(id, MAX_STORE_ID_BYTES)
        .map_err(|_| deny(state, action, "invalid_identifier"))?;
    stored_file(state, action, dir, id)
}

/// The JSON file for a storage stem beneath a store directory (P0-002C5C).
/// A stored file whose name differs from it only by letter case, such as one
/// written before C5B when stems were raw caller ids, is refused rather than
/// selected: on a case-insensitive filesystem it would be the same file.
/// Nothing searches for a similar name.
fn stored_file(state: &AppState, action: &str, dir: &Path, stem: &str) -> Result<PathBuf, String> {
    let name = format!("{stem}.json");
    nexus_kernel::governed_path::case_exact_entry(dir, &name).map_err(|denied| {
        let reason = match denied {
            nexus_kernel::governed_path::PathDenied::CaseAlias => "stored_name_differs_by_case",
            _ => "store_unavailable",
        };
        deny(state, action, reason)
    })?;
    Ok(dir.join(name))
}

/// Every JSON document in a store directory, whatever its file name: stored
/// files, including legacy ones, stay listed even when no identifier selects
/// them.
fn json_documents(dir: &Path) -> Result<Vec<serde_json::Value>, String> {
    let mut documents = Vec::new();
    if dir.exists() {
        let read_dir = std::fs::read_dir(dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(document) = serde_json::from_str::<serde_json::Value>(&content) {
                    documents.push(document);
                }
            }
        }
    }
    Ok(documents)
}

/// The supported email providers.
fn email_provider(state: &AppState, action: &str, provider: &str) -> Result<&'static str, String> {
    match provider {
        "gmail" => Ok("gmail"),
        "outlook" => Ok("outlook"),
        _ => Err(deny(state, action, "unsupported_provider")),
    }
}

// ── API Client ────────────────────────────────────────────────────────
// Final Gate item B: the interface's HTTP client (`api_client_request`)
// sent a request of governed shape to a URL the caller chose and returned
// the response. A URL is not an egress grant, so the command is closed in
// `lib.rs` and nothing here sends a request. Saved collections stay.

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

/// Saves the API Client collections (Final Gate item H). The file is
/// plaintext and no approved secret store exists, so collections that hold
/// an authentication secret are refused and nothing is written; the file
/// already stored is left as it was. Collections that are not JSON cannot be
/// checked and are refused too.
pub(crate) fn api_client_save_collections(data_json: String) -> Result<(), String> {
    save_api_collections_to(data_json, api_collections_path)
}

/// [`api_client_save_collections`] with the file location injected. The
/// secret check runs before `path` is resolved, so a refused save neither
/// resolves nor writes the file (tests pass a temporary location, so no test
/// can reach a real home even if the check regresses).
fn save_api_collections_to(
    data_json: String,
    path: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<(), String> {
    refuse_api_client_secrets(&data_json)?;
    let path = path()?;
    std::fs::write(&path, data_json).map_err(|e| format!("write error: {e}"))
}

/// The API Client's authentication secret fields: the bearer token, the basic
/// password and the API key value. Field names match in any letter case.
const API_CLIENT_SECRET_FIELDS: &[&str] = &["authToken", "authPass", "authKeyValue"];

/// Headers that carry credentials: the standard ones and common API-key
/// headers, matched in any letter case. Secrets typed into other headers,
/// parameters, URLs or bodies are user content and are not detected.
const API_CLIENT_CREDENTIAL_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "x-api-key",
    "api-key",
    "x-auth-token",
    "private-token",
];

/// Refuses collections that hold a non-empty authentication secret, with a
/// bounded reason that echoes nothing.
fn refuse_api_client_secrets(data_json: &str) -> Result<(), String> {
    let collections: serde_json::Value = serde_json::from_str(data_json)
        .map_err(|_| "api_client_save_collections: collections must be JSON".to_string())?;
    if holds_api_client_secret(&collections) {
        return Err(
            "api_client_save_collections: authentication secrets are not stored in Phase Zero; clear the token, password and API key values to save"
                .to_string(),
        );
    }
    Ok(())
}

/// Whether any object in `value` holds a non-empty secret field, or is a
/// credential header entry (`{"key": "Authorization", "value": "..."}`) with
/// a non-empty value. Field and header names match in any letter case, and
/// every spelling counts: an object is a credential header entry when any
/// of its `key` fields names a credential header and any of its `value`
/// fields is non-empty, so a decoy spelling cannot hide the real one.
fn holds_api_client_secret(value: &serde_json::Value) -> bool {
    let filled = |field: &serde_json::Value| match field {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.is_empty(),
        _ => true,
    };
    let named = |name: &str, candidates: &[&str]| {
        candidates
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name.trim()))
    };
    match value {
        serde_json::Value::Object(map) => {
            let names_credential_header = map.iter().any(|(name, field)| {
                named(name, &["key"])
                    && field
                        .as_str()
                        .is_some_and(|header| named(header, API_CLIENT_CREDENTIAL_HEADERS))
            });
            let holds_value = map
                .iter()
                .any(|(name, field)| named(name, &["value"]) && filled(field));
            let credential_header = names_credential_header && holds_value;
            credential_header
                || map.iter().any(|(key, field)| {
                    (named(key, API_CLIENT_SECRET_FIELDS) && filled(field))
                        || holds_api_client_secret(field)
                })
        }
        serde_json::Value::Array(items) => items.iter().any(holds_api_client_secret),
        _ => false,
    }
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

    let notes = json_documents(&notes_dir()?)?;
    serde_json::to_string(&notes).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn notes_get(state: &AppState, id: String) -> Result<String, String> {
    let path = identified_file(state, "notes_get", &notes_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_get", "id": id}),
    );

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
    let path = identified_file(state, "notes_save", &notes_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_save", "id": id, "title": title}),
    );

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
    let path = identified_file(state, "notes_delete", &notes_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "notes_delete", "id": id}),
    );

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
    let emails = json_documents(&emails_dir()?)?;
    serde_json::to_string(&emails).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn email_save(
    state: &AppState,
    id: String,
    data_json: String,
) -> Result<String, String> {
    // P0-002C5B: provider message ids may hold `/`, `=` or anything else, so
    // they map to a deterministic storage stem and are never a path.
    let stem = nexus_kernel::governed_path::storage_stem(&id);
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_save", "id": stem}),
    );
    let path = stored_file(state, "email_save", &emails_dir()?, &stem)?;
    // Validate JSON
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn email_delete(state: &AppState, id: String) -> Result<String, String> {
    let stem = nexus_kernel::governed_path::storage_stem(&id);
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_delete", "id": stem}),
    );
    let path = stored_file(state, "email_delete", &emails_dir()?, &stem)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

// ── OAuth2 loopback callback (P0-002C5C) ──────────────────────────────
//
// A sign-in flow opens the provider's consent page and waits on a fixed
// loopback port for the provider's redirect. Any local process, or any page
// the browser renders, can also reach that port, so a request counts only if
// it is `GET /oauth/callback?…` and carries this flow's unguessable `state`.
// Anything else is answered and ignored: it neither ends the flow nor
// supplies the authorization code.
//
// Final Gate item H: the email and integration sign-in flows are closed,
// because their only product was plaintext token files and no approved
// secret store exists. No production code runs a flow, so these helpers are
// compiled for their P0-002C5C tests only, as the reviewed callback handling
// for any future flow backed by an approved store.

/// How long one loopback connection may take to send its request line.
#[cfg(test)]
const OAUTH_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// Largest request head read from one loopback connection.
#[cfg(test)]
const OAUTH_MAX_REQUEST_BYTES: usize = 8 * 1024;
/// Pause between polls of the nonblocking listener.
#[cfg(test)]
const OAUTH_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// A configured client ID in the form providers issue: letters, digits, `.`,
/// `-` and `_`. It can then add no parameter to the authorization URL and no
/// quoting to the command that opens the browser.
#[cfg(test)]
fn oauth_client_id(client_id: String) -> Result<String, String> {
    let well_formed = !client_id.is_empty()
        && client_id.len() <= 256
        && client_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if well_formed {
        Ok(client_id)
    } else {
        Err("The configured OAuth client ID has an unexpected form.".to_string())
    }
}

/// What one loopback request means for the waiting flow.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum OAuthCallback {
    /// This flow's redirect, carrying an authorization code.
    Code(String),
    /// This flow's redirect, reporting that the provider refused.
    Refused,
    /// Anything else: another method or path, a missing, repeated or wrong
    /// `state`, or no usable code.
    Unrelated,
}

/// Classify a request by its request line alone.
#[cfg(test)]
fn oauth_callback(request: &str, expected_state: &str) -> OAuthCallback {
    let line = request.split("\r\n").next().unwrap_or_default();
    let mut words = line.split(' ');
    let (Some("GET"), Some(target), Some(version), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return OAuthCallback::Unrelated;
    };
    let Some(("/oauth/callback", query)) = target.split_once('?') else {
        return OAuthCallback::Unrelated;
    };
    if !version.starts_with("HTTP/") {
        return OAuthCallback::Unrelated;
    }
    let Ok(parsed) = reqwest::Url::parse(&format!("http://127.0.0.1/?{query}")) else {
        return OAuthCallback::Unrelated;
    };
    let (mut state, mut code, mut error) = (None, None, None);
    for (key, value) in parsed.query_pairs() {
        let slot = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut error,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return OAuthCallback::Unrelated;
        }
    }
    if state.as_deref() != Some(expected_state) {
        return OAuthCallback::Unrelated;
    }
    match (code, error) {
        (Some(code), None) if !code.is_empty() => OAuthCallback::Code(code),
        (None, Some(_)) => OAuthCallback::Refused,
        _ => OAuthCallback::Unrelated,
    }
}

/// Read one request head, bounded in size and by the per-connection timeout.
/// Only its request line is used.
#[cfg(test)]
fn read_oauth_request(stream: &mut std::net::TcpStream, deadline: std::time::Instant) -> String {
    use std::io::Read as IoRead;
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    let timeout = OAUTH_REQUEST_TIMEOUT.min(remaining);
    // Accepted sockets inherit nonblocking mode on some platforms.
    if timeout.is_zero()
        || stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(timeout)).is_err()
        || stream
            .set_write_timeout(Some(OAUTH_REQUEST_TIMEOUT))
            .is_err()
    {
        return String::new();
    }
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    // Read the whole head, so no unread request bytes turn the close into a reset.
    while head.len() < OAUTH_MAX_REQUEST_BYTES && !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    head.truncate(OAUTH_MAX_REQUEST_BYTES);
    String::from_utf8_lossy(&head).into_owned()
}

/// Wait on `listener` until `deadline` for this flow's provider redirect and
/// return its authorization code.
#[cfg(test)]
fn await_oauth_code(
    listener: &std::net::TcpListener,
    expected_state: &str,
    deadline: std::time::Instant,
) -> Result<String, String> {
    use std::io::Write as IoWrite;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("OAuth listener: {e}"))?;
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err("OAuth flow timed out or no auth code received".to_string());
        }
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(_) => {
                std::thread::sleep(OAUTH_POLL_INTERVAL.min(deadline - now));
                continue;
            }
        };
        let outcome = oauth_callback(&read_oauth_request(&mut stream, deadline), expected_state);
        let (status, message) = match outcome {
            OAuthCallback::Code(_) => (
                "200 OK",
                "&#10003; Connected! You can close this tab and return to Nexus OS.",
            ),
            OAuthCallback::Refused => (
                "200 OK",
                "Access was not granted. You can close this tab and return to Nexus OS.",
            ),
            OAuthCallback::Unrelated => (
                "400 Bad Request",
                "This is not the sign-in Nexus OS is waiting for.",
            ),
        };
        let body = format!(
            "<html><body style=\"font-family:system-ui;text-align:center;padding:60px;background:#0f172a;color:#e2e8f0\"><h1>{message}</h1></body></html>"
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        // Best-effort: the page only tells the browser what happened.
        let _ = stream.write_all(response.as_bytes());
        match outcome {
            OAuthCallback::Code(code) => return Ok(code),
            OAuthCallback::Refused => return Err("The provider did not grant access.".to_string()),
            OAuthCallback::Unrelated => {}
        }
    }
}

// ── Email OAuth2 (Gmail / Outlook via REST API) ───────────────────────

pub(crate) fn email_oauth_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("email_oauth");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create email_oauth dir: {e}"))?;
    }
    Ok(dir)
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

pub(crate) fn email_disconnect(state: &AppState, provider: String) -> Result<String, String> {
    let known = email_provider(state, "email_disconnect", &provider)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_disconnect", "provider": known}),
    );
    let path = email_oauth_dir()?.join(format!("{known}_tokens.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("remove: {e}"))?;
    }
    Ok(json!({"status": "disconnected", "provider": provider}).to_string())
}

// ── Messaging: Real Platform Connections ──────────────────────────────

// ── App Store: GitLab API search ─────────────────────────────────────

/// The marketplace search's one fixed endpoint.
const GITLAB_PROJECTS_URL: &str = "https://gitlab.com/api/v4/projects";

/// The longest one marketplace search may take, from connecting until its
/// whole answer is read.
const GITLAB_SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// The most bytes of a marketplace search answer read into memory.
const MAX_GITLAB_SEARCH_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn marketplace_search_gitlab(query: String) -> Result<String, String> {
    block_on_async(search_gitlab(GITLAB_PROJECTS_URL, &query))
}

/// Phase Three G-INV-6: the marketplace search is a fixed-host read with no
/// credential. It follows no redirect (a redirect could name any
/// destination), sends no Referer, ends within `GITLAB_SEARCH_TIMEOUT` and
/// reads at most `MAX_GITLAB_SEARCH_BYTES`; the caller's text is only a
/// query value.
async fn search_gitlab(url: &str, query: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(GITLAB_SEARCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .build()
        .map_err(|e| format!("gitlab search: {}", e.without_url()))?;
    let mut resp = client
        .get(url)
        .query(&[
            ("search", query),
            ("topic", "nexus-agent"),
            ("per_page", "20"),
            ("order_by", "last_activity_at"),
        ])
        .send()
        .await
        .map_err(|e| format!("gitlab search: {e}"))?;
    let too_large =
        || format!("gitlab search: the answer is larger than {MAX_GITLAB_SEARCH_BYTES} bytes");
    if resp
        .content_length()
        .is_some_and(|length| length > MAX_GITLAB_SEARCH_BYTES as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("body: {e}"))? {
        if chunk.len() > MAX_GITLAB_SEARCH_BYTES - body.len() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    let projects: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap_or_default();

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
    let projects = json_documents(&projects_dir()?)?;
    serde_json::to_string(&projects).map_err(|e| format!("json error: {e}"))
}

pub(crate) fn project_get(state: &AppState, id: String) -> Result<String, String> {
    let path = identified_file(state, "project_get", &projects_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_get", "id": id}),
    );
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
    let path = identified_file(state, "project_save", &projects_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_save", "id": id}),
    );
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub(crate) fn project_delete(state: &AppState, id: String) -> Result<String, String> {
    let path = identified_file(state, "project_delete", &projects_dir()?, &id)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "project_delete", "id": id}),
    );
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

#[cfg(test)]
mod tests;
