use crate::http_gateway::{build_router, GatewayState};
use crate::metrics::NexusMetrics;
use tokio::net::TcpListener;

pub async fn run_from_args<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let args = args.into_iter().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("start") => run_server().await,
        Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown command '{other}'. Usage: nexus-server start"
        )),
    }
}

/// Server operating mode — drives which subsystems are initialised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMode {
    /// Headless server (no UI, intended for Docker/K8s).
    Server,
    /// Desktop mode with Tauri (default when running the desktop app).
    Desktop,
    /// Hybrid: server with optional desktop UI attachment.
    Hybrid,
}

impl ServerMode {
    pub fn from_env() -> Self {
        match std::env::var("NEXUS_MODE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "desktop" => Self::Desktop,
            "hybrid" => Self::Hybrid,
            _ => Self::Server,
        }
    }
}

async fn run_server() -> Result<(), String> {
    let mode = ServerMode::from_env();
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "nexus-default-secret".to_string());

    // 1. Create gateway state.
    let state =
        GatewayState::new(&jwt_secret).map_err(|e| format!("failed to initialize gateway: {e}"))?;

    // 2. Install Prometheus metrics.
    let metrics = NexusMetrics::install().map_err(|e| format!("failed to install metrics: {e}"))?;
    let state = state.with_metrics(metrics);

    // 3. Build router.
    let shutdown_state = state.clone();
    let ready_state = state.clone();
    let router = build_router(state);

    // 4. Bind listener.
    let http_addr = std::env::var("NEXUS_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = TcpListener::bind(&http_addr)
        .await
        .map_err(|error| format!("failed to bind {http_addr}: {error}"))?;

    let instance_id = ready_state.instance_id();
    println!("Nexus OS gateway listening on {http_addr} (mode={mode:?}, instance={instance_id})");

    // 5. Mark ready — readiness probe will now return 200.
    ready_state.set_ready();

    // 6. Serve with graceful shutdown.
    let shutdown_timeout = std::env::var("NEXUS_SHUTDOWN_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(match mode {
            // Server mode gets a longer drain period for in-flight agent work.
            ServerMode::Server | ServerMode::Hybrid => 30u64,
            ServerMode::Desktop => 5,
        });

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(shutdown_state, shutdown_timeout))
        .await
        .map_err(|error| format!("server error: {error}"))?;

    println!("Nexus OS gateway exited");
    Ok(())
}

async fn shutdown_signal(state: GatewayState, timeout_secs: u64) {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => println!("Received Ctrl-C, shutting down..."),
                    _ = sigterm.recv() => println!("Received SIGTERM, shutting down..."),
                }
            }
            Err(e) => {
                eprintln!("Failed to register SIGTERM handler: {e}, falling back to Ctrl-C only");
                if let Err(e) = ctrl_c.await {
                    eprintln!("Failed to listen for Ctrl-C: {e}");
                }
                println!("Shutting down...");
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = ctrl_c.await {
            eprintln!("Failed to listen for Ctrl-C: {e}");
        }
        println!("Received Ctrl-C, shutting down...");
    }

    let shutdown = tokio::task::spawn_blocking(move || {
        state.shutdown();
    });

    let deadline = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), shutdown);
    match deadline.await {
        Ok(Ok(())) => println!("Graceful shutdown complete"),
        _ => {
            eprintln!("Shutdown timed out after {timeout_secs} seconds, forcing exit");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "\
Nexus OS Server — Governed AI Agent Runtime

USAGE:
    nexus-server start

ENVIRONMENT:
    NEXUS_MODE                  server | desktop | hybrid (default: server)
    NEXUS_HTTP_ADDR             Listen address (default: 0.0.0.0:8080)
    NEXUS_CORS_ORIGINS          Comma-separated origins or \"*\"
    NEXUS_FRONTEND_DIST         Path to frontend dist/ (default: app/dist)
    NEXUS_CONFIG_PATH           Config directory (default: /data/config)
    NEXUS_SHUTDOWN_TIMEOUT_SECS Graceful shutdown timeout (default: 30)
    JWT_SECRET                  Legacy JWT secret (ignored — EdDSA used)
    HOSTNAME                    Instance ID for HA (default: random UUID)
    DATABASE_URL                PostgreSQL URL for server mode (optional)
    OLLAMA_URL                  Ollama endpoint (default: http://localhost:11434)"
    );
}
