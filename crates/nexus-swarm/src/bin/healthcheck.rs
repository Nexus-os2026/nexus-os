//! `nexus-swarm-healthcheck` — probe every configured swarm provider and
//! print a status table.
//!
//! Exit code:
//! - `0` when every **required** provider reports `Ok`.
//! - Non-zero when any required provider is not `Ok`.
//!
//! Required set: `ollama` + `codex-cli`. Optional providers failing only
//! emit a warning row but do not affect the exit code.

use nexus_swarm::events::ProviderHealthStatus;
use nexus_swarm::provider::Provider;
use nexus_swarm::providers::{
    AnthropicProvider, CodexCliProvider, HuggingFaceProvider, OllamaSwarmProvider,
    OpenAiSwarmProvider, OpenRouterSwarmProvider,
};
use std::sync::Arc;
use std::time::Duration;

const REQUIRED: &[&str] = &["ollama", "codex-cli"];
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let providers: Vec<Arc<dyn Provider>> = vec![
        Arc::new(OllamaSwarmProvider::from_env()),
        Arc::new(CodexCliProvider::new()),
        Arc::new(OpenAiSwarmProvider::new()),
        Arc::new(AnthropicProvider::new()),
        Arc::new(OpenRouterSwarmProvider::new()),
        Arc::new(HuggingFaceProvider::new()),
    ];

    let mut futures = Vec::new();
    for p in &providers {
        let p = Arc::clone(p);
        futures.push(tokio::spawn(async move {
            match tokio::time::timeout(PROBE_TIMEOUT, p.health_check()).await {
                Ok(h) => h,
                Err(_) => nexus_swarm::events::ProviderHealth {
                    provider_id: p.id().to_string(),
                    status: ProviderHealthStatus::Unhealthy,
                    latency_ms: None,
                    models: vec![],
                    notes: format!("timeout after {:?}", PROBE_TIMEOUT),
                    checked_at_secs: 0,
                },
            }
        }));
    }

    let mut results = Vec::with_capacity(futures.len());
    for f in futures {
        match f.await {
            Ok(h) => results.push(h),
            Err(e) => {
                eprintln!("probe join error: {e}");
            }
        }
    }

    println!(
        "{:<14} {:<8} {:<9} {:<7} Notes",
        "Provider", "Status", "Latency", "Models"
    );
    println!("{}", "-".repeat(80));
    for h in &results {
        let status = match h.status {
            ProviderHealthStatus::Ok => "OK",
            ProviderHealthStatus::Degraded => "DEGR",
            ProviderHealthStatus::Unhealthy => "FAIL",
        };
        let latency = h
            .latency_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "—".into());
        let models_count = h.models.len();
        let notes = truncate(&h.notes, 50);
        println!(
            "{:<14} {:<8} {:<9} {:<7} {}",
            h.provider_id, status, latency, models_count, notes
        );
        if models_count > 0 && models_count <= 8 {
            println!("               └─ {}", h.models.join(", "));
        }
    }

    let mut required_ok = true;
    for req in REQUIRED {
        let ok = results
            .iter()
            .any(|h| h.provider_id == *req && h.status == ProviderHealthStatus::Ok);
        if !ok {
            eprintln!("required provider `{req}` is not OK");
            required_ok = false;
        }
    }
    std::process::exit(if required_ok { 0 } else { 1 });
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
