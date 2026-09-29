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

/// The supported messaging platforms.
fn messaging_platform(
    state: &AppState,
    action: &str,
    platform: &str,
) -> Result<&'static str, String> {
    match platform {
        "telegram" => Ok("telegram"),
        "discord" => Ok("discord"),
        "slack" => Ok("slack"),
        _ => Err(deny(state, action, "unsupported_platform")),
    }
}

/// A Discord channel id placed in an API URL path: a snowflake of digits.
fn discord_snowflake(state: &AppState, action: &str, value: &str) -> Result<(), String> {
    if (1..=20).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(deny(state, action, "invalid_channel"))
    }
}

/// A Telegram bot token placed in an API URL path: `<digits>:<token>`, with no
/// URL syntax.
fn telegram_token_ok(token: &str) -> bool {
    token.split_once(':').is_some_and(|(bot, secret)| {
        (1..=20).contains(&bot.len())
            && bot.bytes().all(|b| b.is_ascii_digit())
            && (1..=128).contains(&secret.len())
            && secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    })
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
    refuse_api_client_secrets(&data_json)?;
    let path = api_collections_path()?;
    std::fs::write(&path, data_json).map_err(|e| format!("write error: {e}"))
}

/// The API Client's authentication secret fields: the bearer token, the basic
/// password and the API key value.
const API_CLIENT_SECRET_FIELDS: &[&str] = &["authToken", "authPass", "authKeyValue"];

/// Standard HTTP headers that carry credentials. Secrets typed into other
/// headers, parameters, URLs or bodies are user content and are not detected.
const API_CLIENT_CREDENTIAL_HEADERS: &[&str] = &["authorization", "proxy-authorization", "cookie"];

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

/// Whether any object in `value` holds a non-empty secret field, or a
/// credential header entry (`{"key": "Authorization", "value": "..."}`) with
/// a non-empty value.
fn holds_api_client_secret(value: &serde_json::Value) -> bool {
    let filled = |field: &serde_json::Value| match field {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.is_empty(),
        _ => true,
    };
    match value {
        serde_json::Value::Object(map) => {
            let credential_header = map
                .get("key")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|key| {
                    API_CLIENT_CREDENTIAL_HEADERS
                        .contains(&key.trim().to_ascii_lowercase().as_str())
                })
                && map.get("value").is_some_and(filled);
            credential_header
                || map.iter().any(|(key, field)| {
                    (API_CLIENT_SECRET_FIELDS.contains(&key.as_str()) && filled(field))
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

/// The bot token the configuration stores for a supported platform, or an
/// empty string when none is stored.
fn stored_messaging_token(platform: &'static str) -> Result<String, String> {
    let stored = load_config().map_err(|e| format!("config: {e}"))?.messaging;
    Ok(match platform {
        "telegram" => stored.telegram_bot_token,
        "discord" => stored.discord_bot_token,
        "slack" => stored.slack_bot_token,
        _ => String::new(),
    })
}

/// The bot token for a supported platform (Final Gate item H). The
/// configuration is the token store: its token is used when it holds one.
/// Otherwise a token file written by an earlier version is read as it is,
/// never rewritten or removed.
pub(crate) fn read_messaging_token(platform: &'static str) -> Result<String, String> {
    if let Ok(token) = stored_messaging_token(platform) {
        if !token.is_empty() {
            return Ok(token);
        }
    }
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

pub(crate) fn get_email_access_token(provider: &'static str) -> Result<String, String> {
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
    let known = email_provider(state, "email_fetch_messages", &provider)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_fetch_messages", "provider": known, "folder": folder}),
    );
    let token = get_email_access_token(known)?;
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

/// A value that fits on one message header line: no CR, LF or NUL.
fn email_header_ok(value: &str) -> bool {
    !value.chars().any(|c| matches!(c, '\r' | '\n' | '\0'))
}

pub(crate) fn email_send_message(
    state: &AppState,
    provider: String,
    to: String,
    subject: String,
    body: String,
) -> Result<String, String> {
    let known = email_provider(state, "email_send", &provider)?;
    // P0-002C5C: the recipient and subject become message header lines, so
    // neither may carry a line break (which would add headers such as Bcc).
    if !email_header_ok(&to) || !email_header_ok(&subject) {
        return Err(deny(state, "email_send", "invalid_header"));
    }
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_send", "provider": known, "to": to}),
    );
    let token = get_email_access_token(known)?;

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
    let known = email_provider(state, "email_search", &provider)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "email_search", "provider": known, "query": query}),
    );
    let token = get_email_access_token(known)?;

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

/// Connect a messaging platform with its stored bot token (Final Gate item
/// H).
///
/// The configuration is the token store: a new token is saved there by the
/// settings save, which needs the operator configuration key (item A).
/// Connecting accepts only the stored token, which the interface sees as
/// [`STORED_SECRET`]; any other value is refused before anything is read,
/// written or sent. No token file is written: the plaintext copy under
/// `messaging_tokens` and the cached Slack socket URL are gone.
pub(crate) fn messaging_connect_platform(
    state: &AppState,
    platform: String,
    token_value: String,
) -> Result<String, String> {
    let known = messaging_platform(state, "messaging_connect", &platform)?;
    if token_value != STORED_SECRET {
        return Err(deny(
            state,
            "messaging_connect",
            "token_must_be_saved_first",
        ));
    }
    let token_value = stored_messaging_token(known)?;
    if token_value.is_empty() {
        return Err(deny(state, "messaging_connect", "no_stored_token"));
    }
    if known == "telegram" && !telegram_token_ok(&token_value) {
        return Err(deny(state, "messaging_connect", "invalid_token"));
    }
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_connect", "platform": known}),
    );

    block_on_async(check_messaging_connectivity(
        known,
        &token_value,
        &MESSAGING_ENDPOINTS,
    ))
}

/// Where the connectivity check sends its one request.
struct MessagingEndpoints<'a> {
    /// Base URL; the bot token goes in the path.
    telegram: &'a str,
    /// `auth.test` URL; the token goes in the Authorization header.
    slack: &'a str,
    /// `users/@me` URL; the token goes in the Authorization header.
    discord: &'a str,
}

const MESSAGING_ENDPOINTS: MessagingEndpoints<'static> = MessagingEndpoints {
    telegram: "https://api.telegram.org",
    slack: "https://slack.com/api/auth.test",
    discord: "https://discord.com/api/v10/users/@me",
};

/// Tests a stored bot token against its platform. A transport or body error
/// is reported without its URL (`without_url`): Telegram carries the token in
/// the URL path, and an error returns to the interface (Final Gate item H).
async fn check_messaging_connectivity(
    platform: &'static str,
    token_value: &str,
    endpoints: &MessagingEndpoints<'_>,
) -> Result<String, String> {
    match platform {
        "telegram" => {
            let url = format!("{}/bot{}/getMe", endpoints.telegram, token_value);
            let resp = reqwest::Client::new()
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("telegram test: {}", e.without_url()))?;
            let body = resp
                .text()
                .await
                .map_err(|e| format!("telegram body: {}", e.without_url()))?;
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
                .post(endpoints.slack)
                .bearer_auth(token_value)
                .send()
                .await
                .map_err(|e| format!("slack test: {}", e.without_url()))?;
            let body = resp
                .text()
                .await
                .map_err(|e| format!("slack body: {}", e.without_url()))?;
            let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                let team = data
                    .get("team")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Final Gate item H: no Socket Mode URL is requested or
                // cached on disk; `realtime` only reports an app-level
                // (xapp-*) token.
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
                .get(endpoints.discord)
                .header("Authorization", format!("Bot {}", token_value))
                .send()
                .await
                .map_err(|e| format!("discord test: {}", e.without_url()))?;
            let status = resp.status();
            let body = resp
                .text()
                .await
                .map_err(|e| format!("discord body: {}", e.without_url()))?;
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
}

/// Final Gate item C (redaction): a messaging transport error without the
/// request URL. reqwest names the URL in its errors, and a Telegram URL
/// carries the stored bot token in its path, so an error returned to the
/// interface as given would hand it the stored token. The URL is dropped
/// from every messaging error, whatever the platform.
pub(crate) fn messaging_transport_error(context: &str, error: reqwest::Error) -> String {
    // reqwest's own message for a timeout does not say so.
    let timed_out = if error.is_timeout() {
        " (timed out)"
    } else {
        ""
    };
    format!("{context}: {}{timed_out}", error.without_url())
}

/// Final Gate resource bound: the longest one messaging request may take,
/// from connecting until its whole body is read (reqwest's async client
/// applies its timeout to the whole request). Telegram's poll asks the
/// server to hold the request for up to 5 s.
pub(crate) const MESSAGING_REQUEST_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);

/// Final Gate resource bound: the most response bytes one messaging request
/// reads; a page of 20 messages is far smaller. A longer response is refused,
/// whether or not it declares its length.
pub(crate) const MAX_MESSAGING_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// The client every messaging send and poll uses, bounded in total time.
pub(crate) fn messaging_client() -> Result<reqwest::Client, String> {
    messaging_client_with(MESSAGING_REQUEST_TIMEOUT)
}

pub(crate) fn messaging_client_with(
    timeout: std::time::Duration,
) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| messaging_transport_error("client", e))
}

/// A messaging response's body: at most [`MAX_MESSAGING_RESPONSE_BYTES`].
pub(crate) async fn messaging_body(response: reqwest::Response) -> Result<String, String> {
    messaging_body_bounded(response, MAX_MESSAGING_RESPONSE_BYTES).await
}

/// At most `max` bytes of a response body, read chunk by chunk; decoded as
/// `text()` did (lossy UTF-8). Errors name no URL.
pub(crate) async fn messaging_body_bounded(
    mut response: reqwest::Response,
    max: usize,
) -> Result<String, String> {
    let too_large = || format!("body: the response is larger than {max} bytes");
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| messaging_transport_error("body", e))?
    {
        if body.len() + chunk.len() > max {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

pub(crate) fn messaging_send(
    state: &AppState,
    platform: String,
    channel: String,
    text: String,
) -> Result<String, String> {
    let known = messaging_platform(state, "messaging_send", &platform)?;
    if known == "discord" {
        discord_snowflake(state, "messaging_send", &channel)?;
    }
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_send", "platform": known, "channel": channel}),
    );

    let token = read_messaging_token(known)?;
    if known == "telegram" && !telegram_token_ok(&token) {
        return Err(deny(state, "messaging_send", "invalid_token"));
    }

    let result = block_on_async(async {
        match platform.as_str() {
            "telegram" => {
                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                let resp = messaging_client()?
                    .post(&url)
                    .json(&json!({"chat_id": channel, "text": text}))
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("telegram send", e))?;
                messaging_body(resp).await
            }
            "slack" => {
                let resp = messaging_client()?
                    .post("https://slack.com/api/chat.postMessage")
                    .bearer_auth(&token)
                    .json(&json!({"channel": channel, "text": text}))
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("slack send", e))?;
                messaging_body(resp).await
            }
            "discord" => {
                let url = format!("https://discord.com/api/v10/channels/{}/messages", channel);
                let resp = messaging_client()?
                    .post(&url)
                    .header("Authorization", format!("Bot {}", token))
                    .json(&json!({"content": text}))
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("discord send", e))?;
                messaging_body(resp).await
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
    let known = messaging_platform(state, "messaging_poll", &platform)?;
    if known == "discord" {
        discord_snowflake(state, "messaging_poll", &channel)?;
    }
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "messaging_poll", "platform": known}),
    );

    let token = read_messaging_token(known)?;
    if known == "telegram" && !telegram_token_ok(&token) {
        return Err(deny(state, "messaging_poll", "invalid_token"));
    }

    let result = block_on_async(async {
        match platform.as_str() {
            "telegram" => {
                let offset: i64 = last_id.parse().unwrap_or(0);
                let url = format!(
                    "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=5&limit=20",
                    token, offset
                );
                let resp = messaging_client()?
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("telegram poll", e))?;
                messaging_body(resp).await
            }
            "slack" => {
                let resp = messaging_client()?
                    .get("https://slack.com/api/conversations.history")
                    .bearer_auth(&token)
                    .query(&[("channel", channel.as_str()), ("limit", "20")])
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("slack poll", e))?;
                messaging_body(resp).await
            }
            "discord" => {
                let url = format!(
                    "https://discord.com/api/v10/channels/{}/messages?limit=20",
                    channel
                );
                let resp = messaging_client()?
                    .get(&url)
                    .header("Authorization", format!("Bot {}", token))
                    .send()
                    .await
                    .map_err(|e| messaging_transport_error("discord poll", e))?;
                messaging_body(resp).await
            }
            _ => Err(format!("Unknown platform: {platform}")),
        }
    })?;
    Ok(result)
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
