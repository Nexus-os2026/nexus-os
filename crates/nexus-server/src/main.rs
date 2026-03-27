use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use clap::Parser;
use serde_json::json;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "nexus-server", about = "Nexus OS headless server", version)]
struct Args {
    /// HTTP gateway port
    #[arg(long, default_value = "3000")]
    port: u16,

    /// MCP server port (0 to disable)
    #[arg(long, default_value = "3001")]
    mcp_port: u16,

    /// A2A server port (0 to disable)
    #[arg(long, default_value = "3002")]
    a2a_port: u16,

    /// Data directory for SQLite, audit trail, agent genomes
    #[arg(long, default_value = "./nexus-data")]
    data_dir: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

/// Shared server state.
struct ServerState {
    started_at: std::time::Instant,
    data_dir: String,
    mcp_server: nexus_mcp::McpServer,
    agent_count: u64,
    requests_served: std::sync::atomic::AtomicU64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    tracing::info!(
        "Nexus OS Server v{} starting on port {}",
        env!("CARGO_PKG_VERSION"),
        args.port
    );

    // Create data directory
    std::fs::create_dir_all(&args.data_dir)?;
    tracing::info!("Data directory: {}", args.data_dir);

    // Initialize MCP server with default Nexus OS tools
    let mcp_server = nexus_mcp::McpServer::new();
    tracing::info!(
        "MCP server initialized with {} tools",
        mcp_server
            .handle_request(&nexus_mcp::JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: json!(0),
                method: "tools/list".into(),
                params: json!({}),
            })
            .result
            .and_then(|r| r.get("tools").and_then(|t| t.as_array()).map(|a| a.len()))
            .unwrap_or(0)
    );

    let state = Arc::new(ServerState {
        started_at: std::time::Instant::now(),
        data_dir: args.data_dir.clone(),
        mcp_server,
        agent_count: 0,
        requests_served: std::sync::atomic::AtomicU64::new(0),
    });

    // Build HTTP router
    let app = axum::Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route("/api/v1/agents", get(agents_list_handler))
        .route("/api/v1/agents/{id}/run", post(agent_run_handler))
        .route("/api/v1/agents/{id}/status", get(agent_status_handler))
        .route("/api/v1/audit", get(audit_handler))
        .route("/mcp/tools/list", get(mcp_tools_list_handler))
        .route("/mcp/tools/invoke", post(mcp_tools_invoke_handler))
        .route("/mcp/handle", post(mcp_raw_handler))
        .route("/a2a", post(a2a_handler))
        .route("/a2a/agent-card", get(a2a_agent_card_handler))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state.clone());

    // Start main HTTP server
    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP API listening on {}", addr);

    // Start MCP server on separate port if enabled
    if args.mcp_port > 0 {
        let mcp_state = state.clone();
        let mcp_addr = format!("0.0.0.0:{}", args.mcp_port);
        tokio::spawn(async move {
            let mcp_app = axum::Router::new()
                .route("/", post(mcp_raw_handler))
                .route("/tools/list", get(mcp_tools_list_handler))
                .route("/tools/invoke", post(mcp_tools_invoke_handler))
                .with_state(mcp_state);
            let listener = tokio::net::TcpListener::bind(&mcp_addr).await.unwrap();
            tracing::info!("MCP server listening on {}", mcp_addr);
            axum::serve(listener, mcp_app).await.unwrap();
        });
    }

    // Start A2A server on separate port if enabled
    if args.a2a_port > 0 {
        let a2a_state = state.clone();
        let a2a_addr = format!("0.0.0.0:{}", args.a2a_port);
        tokio::spawn(async move {
            let a2a_app = axum::Router::new()
                .route("/", post(a2a_handler))
                .route("/agent-card", get(a2a_agent_card_handler))
                .with_state(a2a_state);
            let listener = tokio::net::TcpListener::bind(&a2a_addr).await.unwrap();
            tracing::info!("A2A server listening on {}", a2a_addr);
            axum::serve(listener, a2a_app).await.unwrap();
        });
    }

    tracing::info!("Nexus OS Server ready. Press Ctrl+C to stop.");

    // Serve until shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Nexus OS Server stopped.");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Shutdown signal received");
}

// ── Route Handlers ──────────────────────────────────────────────────────────

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "nexus-os-server"
    }))
}

async fn status_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let uptime = state.started_at.elapsed().as_secs();
    let requests = state
        .requests_served
        .load(std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "uptime_seconds": uptime,
        "agent_count": state.agent_count,
        "requests_served": requests,
        "data_dir": state.data_dir,
        "mcp_tools": 7,
        "llm_providers": ["ollama", "groq", "nvidia", "openai", "anthropic", "google", "deepseek", "openrouter"]
    }))
}

async fn agents_list_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "agents": [],
        "count": state.agent_count
    }))
}

async fn agent_run_handler(
    State(state): State<Arc<ServerState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let task = body
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("(no task)");
    Json(json!({
        "agent_id": id,
        "task": task,
        "status": "submitted",
        "message": "Task submitted. Poll /api/v1/agents/{id}/status for progress."
    }))
}

async fn agent_status_handler(
    State(state): State<Arc<ServerState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "agent_id": id,
        "status": "idle",
        "last_task": null
    }))
}

async fn audit_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "events": [],
        "count": 0
    }))
}

async fn mcp_tools_list_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let resp = state.mcp_server.handle_request(&nexus_mcp::JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: json!(1),
        method: "tools/list".into(),
        params: json!({}),
    });
    Json(resp.result.unwrap_or(json!({"tools": []})))
}

async fn mcp_tools_invoke_handler(
    State(state): State<Arc<ServerState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = body.get("arguments").cloned().unwrap_or(json!({}));
    let resp = state.mcp_server.handle_request(&nexus_mcp::JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: json!(1),
        method: "tools/call".into(),
        params: json!({"name": name, "arguments": arguments}),
    });
    Json(resp.result.unwrap_or(json!({"error": "Tool call failed"})))
}

async fn mcp_raw_handler(State(state): State<Arc<ServerState>>, body: String) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let response = state.mcp_server.handle_raw(&body);
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        response,
    )
}

async fn a2a_handler(
    State(state): State<Arc<ServerState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    state
        .requests_served
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "jsonrpc": "2.0",
        "id": body.get("id").cloned().unwrap_or(json!(null)),
        "result": {
            "id": uuid::Uuid::new_v4().to_string(),
            "status": { "state": "submitted" }
        }
    }))
}

async fn a2a_agent_card_handler() -> impl IntoResponse {
    Json(json!({
        "name": "nexus-os",
        "description": "Nexus OS Governed AI Agent Operating System",
        "url": "http://localhost:3002",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "streaming": false,
            "pushNotifications": false
        },
        "skills": [
            {"id": "agent_run", "name": "Agent Run", "description": "Execute a task through a governed agent"},
            {"id": "governance_check", "name": "Governance Check", "description": "Check action against governance policy"},
            {"id": "web_search", "name": "Web Search", "description": "Search the web"},
            {"id": "code_generation", "name": "Code Generation", "description": "Generate code with governed agents"}
        ],
        "authentication": {
            "schemes": ["bearer"]
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args_parse() {
        let args = Args::try_parse_from(["nexus-server", "--port", "8080"]).unwrap();
        assert_eq!(args.port, 8080);
        assert_eq!(args.mcp_port, 3001);
        assert_eq!(args.data_dir, "./nexus-data");
    }

    #[test]
    fn test_cli_defaults() {
        let args = Args::try_parse_from(["nexus-server"]).unwrap();
        assert_eq!(args.port, 3000);
        assert_eq!(args.log_level, "info");
    }
}
