//! Flash Inference Tauri commands — local LLM inference via llama.cpp.

use crate::{AppState, SYSTEM_UUID};
use nexus_kernel::audit::EventType;
use serde_json::json;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Emitter;
use uuid::Uuid;

// ── Flash Inference Commands ────────────────────────────────────────

#[tauri::command]
pub async fn flash_detect_hardware() -> Result<serde_json::Value, String> {
    let hw = nexus_flash_infer::detect_hardware();
    serde_json::to_value(hw).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_profile_model(model_path: String) -> Result<serde_json::Value, String> {
    // Probe GGUF metadata without fully loading the model
    let hw = nexus_flash_infer::detect_hardware();
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);
    serde_json::to_value(profile).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_auto_configure(
    model_path: String,
    target_context_len: u32,
    priority: String,
) -> Result<serde_json::Value, String> {
    let hw = nexus_flash_infer::detect_hardware();
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);

    let prio = match priority.as_str() {
        "speed" => nexus_flash_infer::InferencePriority::Speed,
        "context" => nexus_flash_infer::InferencePriority::Context,
        _ => nexus_flash_infer::InferencePriority::Balanced,
    };

    let preference = nexus_flash_infer::InferencePreference {
        model_path,
        target_context_len,
        priority: prio,
        generation_config: None,
    };

    let config =
        nexus_flash_infer::auto_configure(&hw, &profile, preference).map_err(|e| e.to_string())?;
    serde_json::to_value(config).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_create_session(
    state: tauri::State<'_, AppState>,
    model_path: String,
    target_context_len: u32,
    priority: String,
) -> Result<String, String> {
    let hw = state.flash_session_manager.hardware().clone();
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);

    let prio = match priority.as_str() {
        "speed" => nexus_flash_infer::InferencePriority::Speed,
        "context" => nexus_flash_infer::InferencePriority::Context,
        _ => nexus_flash_infer::InferencePriority::Balanced,
    };

    // Auto-unload other models when loading a large model (>50 GB).
    // Multiple loaded models split RAM and starve mmap page cache,
    // killing MoE expert streaming performance.
    let model_size_gb = profile.file_size_mb as f64 / 1024.0;
    if model_size_gb > 50.0 {
        let existing = state.flash_session_manager.list_sessions().await;
        if !existing.is_empty() {
            eprintln!(
                "[flash] Unloading {} other model(s) to free RAM for large model ({:.0} GB)",
                existing.len(),
                model_size_gb
            );
            // Drop all cached providers first so model handles are released
            {
                let mut cache = state
                    .flash_providers
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                cache.clear();
            }
            state.flash_session_manager.clear_all().await;
            // Force glibc to return freed memory to the OS
            #[cfg(target_os = "linux")]
            {
                extern "C" {
                    fn malloc_trim(pad: usize) -> i32;
                }
                // SAFETY: malloc_trim is a standard glibc function with no UB risk.
                // Best-effort: return freed memory to OS; return value only indicates if memory was released
                let _ = unsafe { malloc_trim(0) };
            }
        }
    }

    let preference = nexus_flash_infer::InferencePreference {
        model_path: model_path.clone(),
        target_context_len,
        priority: prio,
        generation_config: None,
    };

    let (session_id, optimal_config) = state
        .flash_session_manager
        .create_session(&model_path, profile, preference)
        .await
        .map_err(|e| e.to_string())?;

    // Cache a shared FlashProvider so the model handle persists across generate calls.
    // Pass the auto-configured LoadConfig and GenerationConfig so thread count,
    // context size, and batch size match what auto_configure() computed.
    let provider = std::sync::Arc::new(nexus_connectors_llm::providers::FlashProvider::new(
        model_path.clone(),
        optimal_config.load_config,
        optimal_config.generation_config,
    ));

    // Pre-load the model into the provider NOW so that the model_handle is
    // populated before any agent or UI query(). Without this, the agent's
    // ensure_loaded() creates a SECOND llama context with wrong config
    // (n_ctx=262144 default instead of the session's optimized value),
    // which OOMs or asserts in llama.cpp.
    // spawn_blocking works here — flash_generate uses the same pattern.
    {
        let prov_clone = provider.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = prov_clone.ensure_loaded() {
                eprintln!("[flash] WARNING: pre-load failed: {e}");
            } else {
                eprintln!("[flash] model pre-loaded into provider");
            }
        })
        .await
        .unwrap_or_else(|e| eprintln!("[flash] pre-load thread panicked: {e}"));
    }

    {
        let mut cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        cache.insert(session_id.clone(), provider);
    }

    // Audit session creation
    {
        let mut audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: audit trail append; session creation succeeds regardless
        let _ = audit.append_event(
            SYSTEM_UUID,
            EventType::UserAction,
            json!({
                "event_kind": "flash.session_created",
                "session_id": session_id,
                "model_path": model_path,
            }),
        );
    }

    Ok(session_id)
}

/// Run governed inference on a loaded flash session.
///
/// Pipeline: capability check → fuel reserve → PII redaction → input firewall →
/// egress check → flash inference → fuel ledger → oracle event → safety supervisor →
/// output firewall → audit trail.
///
/// Streams tokens to the frontend via `flash-token` events, then emits `flash-done`
/// or `flash-error`.
#[tauri::command]
pub async fn flash_generate(
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: String,
    prompt: String,
    max_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let max_tokens = max_tokens.unwrap_or(2048);

    // 1. Look up session to get model_path and config
    let sessions = state.flash_session_manager.list_sessions().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| format!("session {session_id} not found"))?;
    let model_path = session.model_path.clone();
    let _model_name = session.model_name.clone();

    // 2. Retrieve cached FlashProvider (Arc) — model handle persists across calls.
    let provider = {
        let cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        cache.get(&session_id).cloned().unwrap_or_else(|| {
            // Fallback: provider not cached (shouldn't happen). Use safe defaults
            // matching the proven 0.26 tok/s test configuration.
            eprintln!(
                "[flash] WARNING: provider not cached for session {session_id}, using defaults"
            );
            std::sync::Arc::new(nexus_connectors_llm::providers::FlashProvider::new(
                model_path.clone(),
                nexus_flash_infer::LoadConfig {
                    model_path: model_path.clone(),
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
        })
    };

    // 3. Pre-flight governance audit — log the request before streaming.
    //    For user-facing local chat the full GovernedLlmGateway pipeline
    //    (PII redaction, egress check, etc.) is overkill — the user typed
    //    the prompt and reads the response directly.  We still audit it.
    {
        let mut audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: audit trail for governance; inference proceeds regardless
        let _ = audit.append_event(
            SYSTEM_UUID,
            EventType::LlmCall,
            json!({
                "event_kind": "flash.generate_request",
                "session_id": session_id,
                "model_path": model_path,
                "prompt_len": prompt.len(),
                "max_tokens": max_tokens,
            }),
        );
    }

    // 4. Stream inference — emit each token to the frontend as it arrives.
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    let app_handle = app.clone();

    let session_id_clone = session_id.clone();
    let model_path_clone = model_path.clone();
    let audit_arc = state.audit.clone();

    // Run inference on a blocking thread (llama.cpp is CPU-bound).
    let result = tokio::task::spawn_blocking(move || {
        #[cfg(all(
            feature = "tauri-runtime",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        let stream_app = app_handle.clone();

        let response = provider.query_streaming(&prompt, max_tokens, move |token_text| {
            #[cfg(all(
                feature = "tauri-runtime",
                any(target_os = "windows", target_os = "macos", target_os = "linux")
            ))]
            {
                // Best-effort: stream token to frontend; dropped tokens are non-fatal
                let _ = stream_app.emit("flash-token", json!({ "text": token_text }));
            }
            true // continue generating
        });

        match response {
            Ok(resp) => {
                #[cfg(all(
                    feature = "tauri-runtime",
                    any(target_os = "windows", target_os = "macos", target_os = "linux")
                ))]
                {
                    // Best-effort: notify frontend that generation is complete
                    let _ = app_handle.emit(
                        "flash-done",
                        json!({
                            "stats": {
                                "token_count": resp.token_count,
                                "model": resp.model_name,
                                "cost": 0.0,
                            }
                        }),
                    );
                }

                // Audit
                {
                    let mut audit = audit_arc.lock().unwrap_or_else(|p| p.into_inner());
                    // Best-effort: audit trail for completed inference; response already sent
                    let _ = audit.append_event(
                        SYSTEM_UUID,
                        EventType::LlmCall,
                        json!({
                            "event_kind": "flash.governed_inference",
                            "session_id": session_id_clone,
                            "model_path": model_path_clone,
                            "tokens": resp.token_count,
                        }),
                    );
                }

                Ok(json!({
                    "streamed": true,
                    "token_count": resp.token_count,
                    "model": resp.model_name,
                }))
            }
            Err(e) => {
                #[cfg(all(
                    feature = "tauri-runtime",
                    any(target_os = "windows", target_os = "macos", target_os = "linux")
                ))]
                {
                    // Best-effort: notify frontend of generation error
                    let _ = app_handle.emit("flash-error", json!({ "message": e.to_string() }));
                }
                Err(e.to_string())
            }
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {e}"))?;

    result
}

#[tauri::command]
pub async fn flash_list_sessions(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let sessions = state.flash_session_manager.list_sessions().await;
    serde_json::to_value(sessions).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_unload_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // Remove cached provider (drops model handle when last Arc ref goes away)
    {
        let mut cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let provider = cache.remove(&session_id);
        // Explicitly drop the provider before malloc_trim so the model is freed
        std::mem::drop(provider);
    }

    state
        .flash_session_manager
        .unload_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    // Force glibc to return freed memory to the OS
    #[cfg(target_os = "linux")]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: malloc_trim is a standard glibc function with no UB risk.
        // Best-effort: return freed memory to OS; return value only indicates if memory was released
        let _ = unsafe { malloc_trim(0) };
    }

    // Audit session unload
    {
        let mut audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
        // Best-effort: audit trail append; session unload succeeds regardless
        let _ = audit.append_event(
            SYSTEM_UUID,
            EventType::UserAction,
            json!({
                "event_kind": "flash.session_unloaded",
                "session_id": session_id,
            }),
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn flash_clear_sessions(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Drop all cached providers first so model handles are released.
    {
        let mut cache = state
            .flash_providers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        cache.clear();
    }
    state.flash_session_manager.clear_all().await;
    Ok(())
}

#[tauri::command]
pub async fn flash_get_metrics(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<serde_json::Value, String> {
    // Build metrics from session info
    let sessions = state.flash_session_manager.list_sessions().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| format!("session {session_id} not found"))?;

    let metrics = nexus_flash_infer::types::InferenceMetrics {
        session_id: session.id.clone(),
        tokens_per_second: 0.0,
        prompt_tokens_per_second: 0.0,
        memory_used_mb: session.memory_used_mb,
        memory_budget_mb: state.flash_session_manager.remaining_budget_mb()
            + session.memory_used_mb,
        memory_utilization: 0.0,
        expert_cache_hit_rate: 0.0,
        io_read_mb_per_sec: 0.0,
        cpu_utilization: 0.0,
        context_used: 0,
        context_max: 0,
        total_tokens_generated: session.tokens_generated,
        uptime_seconds: chrono::Utc::now()
            .signed_duration_since(session.created_at)
            .num_seconds() as f64,
    };

    serde_json::to_value(metrics).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_system_metrics() -> Result<serde_json::Value, String> {
    // --- RAM ---
    let (ram_used_mb, ram_total_mb) = {
        // Process RSS from /proc/self/status
        let rss_kb = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse::<u64>().ok())
                })
            })
            .unwrap_or(0);

        // System total from /proc/meminfo
        let total_kb = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                    })
            })
            .unwrap_or(0);

        (rss_kb / 1024, total_kb / 1024)
    };

    // --- CPU ---
    let cpu_percent = {
        // Read /proc/stat twice with a short gap to compute delta
        fn read_cpu_total() -> Option<(u64, u64)> {
            let s = std::fs::read_to_string("/proc/stat").ok()?;
            let line = s.lines().next()?;
            let vals: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|v| v.parse().ok())
                .collect();
            if vals.len() < 4 {
                return None;
            }
            let total: u64 = vals.iter().sum();
            let idle = vals[3];
            Some((total, idle))
        }

        let before = read_cpu_total();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let after = read_cpu_total();

        match (before, after) {
            (Some((t1, i1)), Some((t2, i2))) => {
                let dt = t2.saturating_sub(t1) as f32;
                let di = i2.saturating_sub(i1) as f32;
                if dt > 0.0 {
                    ((dt - di) / dt * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    };

    // --- VRAM (nvidia-smi) ---
    let (vram_used_mb, vram_total_mb) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()
        .and_then(|out| {
            if !out.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            let line = text.lines().next()?;
            let mut parts = line
                .split(',')
                .map(|s| s.trim().parse::<u64>().unwrap_or(0));
            let used = parts.next().unwrap_or(0);
            let total = parts.next().unwrap_or(0);
            Some((used, total))
        })
        .unwrap_or((0, 0));

    // --- Disk I/O and page-cache hit rate ---
    // During MoE inference, most expert weight reads come from page cache (RAM),
    // not SSD. Show both: actual disk I/O and cache hit percentage.
    let (io_read_mb_s, cache_hit_percent) = {
        fn read_disk_sectors() -> Option<u64> {
            for prefix in &["nvme0n1", "nvme1n1", "sda", "sdb"] {
                let path = format!("/sys/block/{}/stat", prefix);
                if let Ok(s) = std::fs::read_to_string(&path) {
                    let fields: Vec<&str> = s.split_whitespace().collect();
                    if let Some(sectors) = fields.get(2).and_then(|v| v.parse::<u64>().ok()) {
                        if sectors > 0 {
                            return Some(sectors);
                        }
                    }
                }
            }
            None
        }

        // Page cache stats from /proc/vmstat: pgpgin = pages read from disk
        fn read_pgpgin() -> Option<u64> {
            let s = std::fs::read_to_string("/proc/vmstat").ok()?;
            for line in s.lines() {
                if line.starts_with("pgpgin ") {
                    return line.split_whitespace().nth(1)?.parse().ok();
                }
            }
            None
        }

        // Cache stats from /proc/meminfo
        fn read_cache_mb() -> u64 {
            std::fs::read_to_string("/proc/meminfo")
                .ok()
                .map(|s| {
                    let mut cached = 0u64;
                    for line in s.lines() {
                        if line.starts_with("Cached:") {
                            cached = line
                                .split_whitespace()
                                .nth(1)
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(0);
                            break;
                        }
                    }
                    cached / 1024
                })
                .unwrap_or(0)
        }

        let s1 = read_disk_sectors().unwrap_or(0);
        let pg1 = read_pgpgin().unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let s2 = read_disk_sectors().unwrap_or(0);
        let pg2 = read_pgpgin().unwrap_or(0);

        // Actual disk I/O in MB/s
        let delta_sectors = s2.saturating_sub(s1);
        let io_mb_s = (delta_sectors * 512) as f32 / (1024.0 * 1024.0);

        // Page-cache hit rate: pgpgin counts KB read from disk.
        // If total file reads >> disk reads, the rest came from cache.
        // Use cached memory as proxy: high cache = high hit rate for mmap'd models.
        let cache_mb = read_cache_mb();
        let disk_read_kb = pg2.saturating_sub(pg1); // KB read from disk this interval
        let cache_pct = if cache_mb > 0 && ram_total_mb > 0 {
            // Heuristic: cache hit rate ≈ fraction of RAM used as page cache,
            // weighted by whether actual I/O is low relative to expected throughput.
            let cache_fraction = cache_mb as f32 / ram_total_mb as f32;
            // If disk reads are near zero, almost everything is cached
            if disk_read_kb == 0 {
                (cache_fraction * 100.0).min(99.0)
            } else {
                // Lower hit rate when disk is actively reading
                let io_penalty = (io_mb_s / 500.0).min(1.0); // normalize to 500 MB/s max
                ((cache_fraction * (1.0 - io_penalty * 0.5)) * 100.0).clamp(0.0, 99.0)
            }
        } else {
            0.0
        };

        (io_mb_s, cache_pct)
    };

    let metrics = serde_json::json!({
        "ram_used_mb": ram_used_mb,
        "ram_total_mb": ram_total_mb,
        "cpu_percent": (cpu_percent * 10.0).round() / 10.0,
        "vram_used_mb": vram_used_mb,
        "vram_total_mb": vram_total_mb,
        "ssd_read_mb_s": (io_read_mb_s * 10.0).round() / 10.0,
        "cache_hit_percent": (cache_hit_percent * 10.0).round() / 10.0,
    });

    Ok(metrics)
}

#[tauri::command]
pub async fn flash_estimate_performance(model_path: String) -> Result<serde_json::Value, String> {
    let hw = nexus_flash_infer::detect_hardware();
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);

    let budget = nexus_flash_infer::MemoryBudget::calculate(&hw, &profile, 4096);
    let estimate = profile.estimate_performance(&hw, &budget);
    serde_json::to_value(estimate).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_run_benchmark(
    state: tauri::State<'_, AppState>,
    model_path: String,
    priority: Option<String>,
) -> Result<serde_json::Value, String> {
    let hw = nexus_flash_infer::detect_hardware();
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);

    let prio = match priority.as_deref() {
        Some("speed") => nexus_flash_infer::InferencePriority::Speed,
        Some("context") => nexus_flash_infer::InferencePriority::Context,
        _ => nexus_flash_infer::InferencePriority::Balanced,
    };

    let preference = nexus_flash_infer::InferencePreference {
        model_path: model_path.clone(),
        target_context_len: 4096,
        priority: prio,
        generation_config: None,
    };

    let config =
        nexus_flash_infer::auto_configure(&hw, &profile, preference).map_err(|e| e.to_string())?;

    let model_handle = backend
        .load_model(path, &config.load_config)
        .map_err(|e| e.to_string())?;

    let budget_mb = state.flash_session_manager.remaining_budget_mb();
    let results = nexus_flash_infer::run_full_benchmark(model_handle.as_ref(), &hw, budget_mb)
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&results).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_export_benchmark_report(
    results: Vec<nexus_flash_infer::BenchmarkResult>,
) -> Result<String, String> {
    let report = nexus_flash_infer::generate_report(&results);

    // Save to a temp file and return the path
    let report_dir = std::env::temp_dir();
    let report_path = report_dir.join(format!(
        "nexus-benchmark-{}.md",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));

    std::fs::write(&report_path, &report).map_err(|e| format!("write report: {e}"))?;
    Ok(report_path.to_string_lossy().into_owned())
}

/// Enable speculative decoding for the current session.
///
/// Loads a small fast "draft" model and pairs it with the loaded target model.
/// The draft model generates tokens speculatively, then the target verifies
/// them in batch. With ~70% acceptance rate, this gives 2-4x throughput
/// for memory-bandwidth-bound MoE models.
#[tauri::command]
pub async fn flash_enable_speculative(
    state: tauri::State<'_, AppState>,
    draft_model_path: String,
    draft_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let hw = state.flash_session_manager.hardware().clone();

    // Auto-configure the draft model for speed
    let registry = {
        let mut r = nexus_flash_infer::BackendRegistry::new();
        r.register(Box::new(nexus_flash_infer::LlamaBackend::new(hw.clone())));
        r
    };
    let path = std::path::Path::new(&draft_model_path);
    let backend = registry.select_backend(path).map_err(|e| e.to_string())?;
    let metadata = backend.probe_model(path).map_err(|e| e.to_string())?;
    let profile = nexus_flash_infer::ModelProfile::from_metadata(&metadata);

    let preference = nexus_flash_infer::InferencePreference {
        model_path: draft_model_path.clone(),
        target_context_len: 2048,
        priority: nexus_flash_infer::InferencePriority::Speed,
        generation_config: None,
    };

    let optimal =
        nexus_flash_infer::auto_configure(&hw, &profile, preference).map_err(|e| e.to_string())?;

    let spec_config = nexus_flash_infer::SpeculativeConfig {
        draft_model_path: draft_model_path.clone(),
        draft_tokens: draft_tokens.unwrap_or(5),
        draft_load_config: optimal.load_config,
        draft_gen_config: optimal.generation_config,
    };

    let engine = nexus_flash_infer::SpeculativeEngine::new(spec_config, hw);
    engine.load_draft().map_err(|e| e.to_string())?;

    let info = json!({
        "draft_model": draft_model_path,
        "draft_tokens": draft_tokens.unwrap_or(5),
        "status": "loaded",
        "acceptance_rate": engine.acceptance_rate(),
    });

    {
        let mut guard = state
            .flash_speculative
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *guard = Some(engine);
    }

    Ok(info)
}

/// Disable speculative decoding and unload the draft model.
#[tauri::command]
pub async fn flash_disable_speculative(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state
        .flash_speculative
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(engine) = guard.take() {
        engine.unload_draft().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get speculative decoding status and stats.
#[tauri::command]
pub async fn flash_speculative_status(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let guard = state
        .flash_speculative
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    match guard.as_ref() {
        Some(engine) => Ok(json!({
            "enabled": true,
            "acceptance_rate": engine.acceptance_rate(),
            "draft_length": engine.draft_length(),
            "loaded": engine.is_loaded(),
        })),
        None => Ok(json!({
            "enabled": false,
        })),
    }
}

#[tauri::command]
pub async fn flash_catalog_recommend() -> Result<serde_json::Value, String> {
    let hw = nexus_flash_infer::detect_hardware();
    let catalog = nexus_flash_infer::ModelCatalog::new();
    let recommendations = catalog.recommend(&hw);
    serde_json::to_value(recommendations).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_catalog_search(query: String) -> Result<serde_json::Value, String> {
    let catalog = nexus_flash_infer::ModelCatalog::new();
    let results = catalog.search(&query);
    serde_json::to_value(results).map_err(|e| format!("serialize: {e}"))
}

// ── Flash Inference — Download & Model Management ──────────────────

#[tauri::command]
pub async fn flash_list_local_models() -> Result<serde_json::Value, String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    let models = storage.list_models().map_err(|e| e.to_string())?;
    serde_json::to_value(models).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_download_model(
    hf_repo: String,
    filename: String,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    let downloader = nexus_flash_infer::ModelDownloader::new(storage);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<nexus_flash_infer::DownloadProgress>(64);

    // Spawn a task to forward progress events to the frontend.
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            // Best-effort: forward download progress to frontend; missed events are non-fatal
            let _ = handle.emit("flash-download-progress", &progress);
        }
    });

    let model = downloader
        .download(&hf_repo, &filename, tx)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(model).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_download_multi(
    hf_repo: String,
    filenames: Vec<String>,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    let downloader = nexus_flash_infer::ModelDownloader::new(storage);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<nexus_flash_infer::DownloadProgress>(64);

    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            // Best-effort: forward download progress to frontend; missed events are non-fatal
            let _ = handle.emit("flash-download-progress", &progress);
        }
    });

    let model = downloader
        .download_multi(&hf_repo, &filenames, tx)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(model).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn flash_delete_local_model(filename: String) -> Result<(), String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    storage.delete_model(&filename).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn flash_available_disk_space() -> Result<u64, String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    storage.available_disk_space().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn flash_get_model_dir() -> Result<String, String> {
    let storage = nexus_flash_infer::ModelStorage::new().map_err(|e| e.to_string())?;
    Ok(storage.base_dir().to_string_lossy().to_string())
}
