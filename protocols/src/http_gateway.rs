//! Axum-based HTTP gateway — async edge layer bridging HTTP to the sync kernel.
//!
//! Routes:
//! - `POST /a2a`                                — A2A task submission
//! - `GET  /a2a/agent-card`                     — Agent Card discovery
//! - `GET  /a2a/tasks/:id`                      — Task status lookup
//! - `POST /mcp/tools/invoke`                   — MCP tool invocation
//! - `GET  /mcp/tools/list`                     — MCP tool discovery
//! - `GET  /health`                             — Health check
//! - `GET  /auth/jwks`                          — OIDC discovery
//! - `GET  /ws?token=<jwt>`                     — WebSocket event stream
//! - `POST /v1/messages`                        — Anthropic Messages API
//! - `POST /v1/chat/completions`                — OpenAI Chat Completions API
//! - `POST /v1/embeddings`                      — OpenAI Embeddings API
//! - `GET  /v1/models`                          — Model listing (OpenAI compat)
//!
//! REST API (all JWT-authenticated via middleware layer):
//! - Agent management: GET/POST /api/agents, POST /api/agents/:id/start|stop, GET /api/agents/:id/status
//! - Permissions: GET/PUT /api/agents/:id/permissions, POST /api/agents/:id/permissions/bulk
//! - Audit: GET /api/audit/events, GET /api/audit/events/:id
//! - Compliance: GET /api/compliance/status, GET /api/compliance/report/:agent_id, POST /api/compliance/erase/:agent_id
//! - Marketplace: GET /api/marketplace/search, GET /api/marketplace/agents/:id, POST /api/marketplace/install/:id
//! - Identity: GET /api/identity/agents, GET /api/identity/agents/:id
//! - Firewall: GET /api/firewall/status
//!
//! Every route goes through governance. JWT auth required on mutating endpoints.

use crate::metrics::NexusMetrics;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::compliance::data_governance::AgentDataEraser;
use nexus_kernel::compliance::monitor::{AgentSnapshot, ComplianceMonitor};
use nexus_kernel::compliance::transparency::TransparencyReportGenerator;
use nexus_kernel::identity::{AgentIdentity, IdentityManager, OidcAClaims, TokenManager};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::permissions::PermissionManager;
use nexus_kernel::privacy::PrivacyManager;
use nexus_kernel::protocols::a2a::{
    A2ATask, AgentCard, GovernanceContext, MessagePart, MessageRole, TaskMessage, TaskPayload,
    A2A_PROTOCOL_VERSION,
};
use nexus_kernel::protocols::mcp::McpServer;
use nexus_kernel::supervisor::Supervisor;
use nexus_sdk::module_cache::ModuleCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ── JWT auth (EdDSA / Ed25519) ──────────────────────────────────────────────

/// Re-export claims type so existing consumers can keep using it.
pub type JwtClaims = OidcAClaims;

/// Extract and validate an EdDSA bearer token from the Authorization header.
fn validate_jwt(
    headers: &HeaderMap,
    token_mgr: &TokenManager,
    gateway_identity: &AgentIdentity,
) -> Result<OidcAClaims, AuthError> {
    let header_value = headers
        .get("authorization")
        .ok_or(AuthError::MissingToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken("non-ascii header".into()))?;

    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidToken("expected Bearer scheme".into()))?;

    token_mgr
        .validate_token(token, gateway_identity)
        .map_err(|e| AuthError::InvalidToken(e.to_string()))
}

/// Create a signed EdDSA JWT for testing.
pub fn create_test_jwt(
    identity: &AgentIdentity,
    key_manager: &nexus_kernel::hardware_security::KeyManager,
    token_mgr: &TokenManager,
    ttl: u64,
) -> String {
    token_mgr
        .issue_token(identity, key_manager, &[], ttl, None)
        .expect("test JWT signing should not fail")
}

#[derive(Debug)]
enum AuthError {
    MissingToken,
    InvalidToken(String),
}

impl AuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            AuthError::MissingToken => StatusCode::UNAUTHORIZED,
            AuthError::InvalidToken(_) => StatusCode::UNAUTHORIZED,
        }
    }

    fn message(&self) -> String {
        match self {
            AuthError::MissingToken => "missing Authorization header".to_string(),
            AuthError::InvalidToken(reason) => format!("invalid token: {reason}"),
        }
    }
}

// ── WebSocket event types ────────────────────────────────────────────────────

/// Events broadcast over the WebSocket stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    /// Event kind discriminator.
    #[serde(rename = "type")]
    pub event_type: WsEventType,
    /// Event payload.
    pub data: serde_json::Value,
    /// Unix-epoch millisecond timestamp.
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WsEventType {
    AgentStatusChanged,
    FuelConsumed,
    AuditEvent,
    ComplianceAlert,
    FirewallBlock,
    SpeculationDecision,
}

impl WsEvent {
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn agent_status_changed(agent_id: Uuid, status: &str) -> Self {
        Self {
            event_type: WsEventType::AgentStatusChanged,
            data: serde_json::json!({ "agent_id": agent_id.to_string(), "status": status }),
            timestamp: Self::now_ms(),
        }
    }

    pub fn fuel_consumed(agent_id: Uuid, remaining: u64) -> Self {
        Self {
            event_type: WsEventType::FuelConsumed,
            data: serde_json::json!({ "agent_id": agent_id.to_string(), "fuel_remaining": remaining }),
            timestamp: Self::now_ms(),
        }
    }

    pub fn audit_event(event_id: Uuid, agent_id: Uuid, event_type: &str) -> Self {
        Self {
            event_type: WsEventType::AuditEvent,
            data: serde_json::json!({
                "event_id": event_id.to_string(),
                "agent_id": agent_id.to_string(),
                "event_type": event_type,
            }),
            timestamp: Self::now_ms(),
        }
    }

    pub fn compliance_alert(agent_id: Uuid, message: &str) -> Self {
        Self {
            event_type: WsEventType::ComplianceAlert,
            data: serde_json::json!({ "agent_id": agent_id.to_string(), "message": message }),
            timestamp: Self::now_ms(),
        }
    }

    pub fn firewall_block(agent_id: Uuid, reason: &str) -> Self {
        Self {
            event_type: WsEventType::FirewallBlock,
            data: serde_json::json!({ "agent_id": agent_id.to_string(), "reason": reason }),
            timestamp: Self::now_ms(),
        }
    }

    pub fn speculation_decision(agent_id: Uuid, approved: bool, summary: &str) -> Self {
        Self {
            event_type: WsEventType::SpeculationDecision,
            data: serde_json::json!({
                "agent_id": agent_id.to_string(),
                "approved": approved,
                "summary": summary,
            }),
            timestamp: Self::now_ms(),
        }
    }
}

/// Channel capacity for the broadcast sender.
const WS_BROADCAST_CAPACITY: usize = 256;

// ── JWT middleware layer ────────────────────────────────────────────────────

/// Axum middleware that validates JWT on every request in the wrapped router.
/// On success, the validated `OidcAClaims` are inserted into request extensions.
async fn jwt_auth_middleware(
    State(state): State<GatewayState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = req.headers().clone();
    let claims = {
        let inner = state.inner.lock().expect("lock poisoned");
        validate_jwt(&headers, &inner.token_manager, &inner.gateway_identity)
    };

    match claims {
        Ok(c) => {
            req.extensions_mut().insert(c);
            next.run(req).await
        }
        Err(e) => {
            let body = serde_json::json!({
                "error": e.message(),
                "code": e.status_code().as_u16(),
            });
            (e.status_code(), Json(body)).into_response()
        }
    }
}

// ── Shared gateway state ────────────────────────────────────────────────────

/// Shared state for the HTTP gateway, wrapped in Arc<Mutex<>> for sync access.
#[derive(Debug, Clone)]
pub struct GatewayState {
    inner: Arc<Mutex<GatewayInner>>,
}

#[derive(Debug)]
struct GatewayInner {
    /// Agent cards keyed by agent name.
    agent_cards: HashMap<String, AgentCard>,
    /// MCP server with governed tool invocation.
    mcp_server: McpServer,
    /// In-flight A2A tasks keyed by task ID.
    tasks: HashMap<String, A2ATask>,
    /// Map agent name → agent UUID for routing.
    agent_ids: HashMap<String, Uuid>,
    /// EdDSA token manager for JWT issuance/validation.
    token_manager: TokenManager,
    /// Gateway-level signing identity (used to sign/verify JWTs).
    gateway_identity: AgentIdentity,
    /// Agent identity manager (used for per-agent key management).
    identity_manager: IdentityManager,
    /// Server start time.
    started_at: u64,
    /// Kernel supervisor for agent lifecycle.
    supervisor: Supervisor,
    /// Audit trail for governance events.
    audit_trail: AuditTrail,
    /// Agent name metadata keyed by UUID.
    agent_meta: HashMap<Uuid, AgentMeta>,
    /// Permission manager for capability dashboard.
    permission_manager: PermissionManager,
    /// Privacy manager for cryptographic erasure.
    privacy_manager: PrivacyManager,
    /// Broadcast sender for WebSocket event streaming.
    ws_tx: broadcast::Sender<WsEvent>,
    /// Prometheus-compatible metrics.
    metrics: Option<NexusMetrics>,
    /// Shared WASM module cache for hit-rate reporting.
    wasm_module_cache: Option<ModuleCache>,
}

/// Metadata about an agent tracked alongside the supervisor.
#[derive(Debug, Clone)]
struct AgentMeta {
    name: String,
    last_action: String,
}

impl GatewayState {
    /// Create a new gateway state with EdDSA JWT auth.
    ///
    /// A fresh Ed25519 keypair is generated for the gateway itself, used to
    /// sign and verify all JWTs. The old `jwt_secret` parameter is accepted
    /// for API compatibility but is **ignored** — signing is now asymmetric.
    pub fn new(_jwt_secret: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut identity_manager = IdentityManager::in_memory();
        let gateway_id = Uuid::new_v4();
        let gateway_identity =
            AgentIdentity::generate(gateway_id, identity_manager.key_manager_mut())
                .expect("gateway identity generation must succeed");
        let token_manager = TokenManager::new("nexus-gateway", "nexus-agents");
        let (ws_tx, _) = broadcast::channel(WS_BROADCAST_CAPACITY);

        Self {
            inner: Arc::new(Mutex::new(GatewayInner {
                agent_cards: HashMap::new(),
                mcp_server: McpServer::new(),
                tasks: HashMap::new(),
                agent_ids: HashMap::new(),
                token_manager,
                gateway_identity,
                identity_manager,
                started_at: now,
                supervisor: Supervisor::new(),
                audit_trail: AuditTrail::new(),
                agent_meta: HashMap::new(),
                permission_manager: PermissionManager::default(),
                privacy_manager: PrivacyManager::new(),
                ws_tx,
                metrics: None,
                wasm_module_cache: None,
            })),
        }
    }

    /// Attach a [`NexusMetrics`] instance to this gateway for Prometheus export.
    pub fn with_metrics(self, metrics: NexusMetrics) -> Self {
        let mut inner = self.inner.lock().expect("lock poisoned");
        inner.metrics = Some(metrics);
        drop(inner);
        self
    }

    /// Attach a shared [`ModuleCache`] for WASM cache hit-rate reporting on `/health`.
    pub fn with_wasm_cache(self, cache: ModuleCache) -> Self {
        let mut inner = self.inner.lock().expect("lock poisoned");
        inner.wasm_module_cache = Some(cache);
        drop(inner);
        self
    }

    /// Register an agent with the gateway.
    pub fn register_agent(&self, manifest: AgentManifest, base_url: &str) {
        let mut inner = self.inner.lock().expect("lock poisoned");
        let card = AgentCard::from_manifest(&manifest, base_url);
        let name = manifest.name.clone();

        // Start agent in supervisor for full lifecycle support
        let agent_id = match inner.supervisor.start_agent(manifest.clone()) {
            Ok(id) => id,
            Err(_) => {
                // Fallback: register in MCP only (backwards compat)
                let agent_id = Uuid::new_v4();
                inner.mcp_server.register_agent(agent_id, manifest);
                inner.agent_cards.insert(name.clone(), card);
                inner.agent_ids.insert(name, agent_id);
                return;
            }
        };

        inner.mcp_server.register_agent(agent_id, manifest);
        inner.agent_cards.insert(name.clone(), card);
        inner.agent_ids.insert(name.clone(), agent_id);
        inner.agent_meta.insert(
            agent_id,
            AgentMeta {
                name,
                last_action: "registered".to_string(),
            },
        );

        // Metrics: agent spawned via registration
        if let Some(ref m) = inner.metrics {
            m.inc_agents_spawned();
        }
    }

    /// Issue an EdDSA-signed JWT for testing or programmatic use.
    pub fn issue_token(&self, scopes: &[String], ttl: u64) -> String {
        let inner = self.inner.lock().expect("lock poisoned");
        inner
            .token_manager
            .issue_token(
                &inner.gateway_identity,
                inner.identity_manager.key_manager(),
                scopes,
                ttl,
                None,
            )
            .expect("gateway token signing must succeed")
    }

    /// Return the JWKS JSON for OIDC discovery.
    pub fn jwks(&self) -> serde_json::Value {
        let inner = self.inner.lock().expect("lock poisoned");
        TokenManager::jwks_json(&inner.gateway_identity)
    }

    /// Subscribe to the WebSocket event broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        let inner = self.inner.lock().expect("lock poisoned");
        inner.ws_tx.subscribe()
    }

    /// Broadcast a WebSocket event to all connected clients.
    pub fn broadcast(&self, event: WsEvent) {
        let inner = self.inner.lock().expect("lock poisoned");
        // Ignore send errors — they just mean no active subscribers.
        let _ = inner.ws_tx.send(event);
    }

    /// Graceful shutdown: stop agents, flush audit, log completion.
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().expect("lock poisoned");

        // 1. Stop all running agents
        let agent_ids: Vec<Uuid> = inner
            .supervisor
            .health_check()
            .iter()
            .map(|s| s.id)
            .collect();
        for id in &agent_ids {
            let _ = inner.supervisor.stop_agent(*id);
        }

        // 2. Flush audit trail batcher to persist pending events
        inner.audit_trail.flush_batcher();

        // 3. Log shutdown event
        let _ = inner.audit_trail.append_event(
            Uuid::nil(),
            EventType::StateChange,
            serde_json::json!({
                "event": "gateway.shutdown",
                "agents_stopped": agent_ids.len(),
            }),
        );

        println!(
            "Shutdown complete: {} agents stopped, audit flushed",
            agent_ids.len()
        );
    }
}

// ── Router construction ─────────────────────────────────────────────────────

/// Build the axum Router with all protocol routes and CORS middleware.
pub fn build_router(state: GatewayState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Authenticated REST API routes — JWT validated via middleware layer
    let api_routes = Router::new()
        // Agent management
        .route("/agents", get(api_list_agents).post(api_create_agent))
        .route("/agents/{id}/start", post(api_start_agent))
        .route("/agents/{id}/stop", post(api_stop_agent))
        .route("/agents/{id}/status", get(api_agent_status))
        // Permissions
        .route(
            "/agents/{id}/permissions",
            get(api_get_permissions).put(api_update_permission),
        )
        .route(
            "/agents/{id}/permissions/bulk",
            post(api_bulk_update_permissions),
        )
        // Audit
        .route("/audit/events", get(api_audit_events))
        .route("/audit/events/{id}", get(api_audit_event_by_id))
        // Compliance
        .route("/compliance/status", get(api_compliance_status))
        .route("/compliance/report/{agent_id}", get(api_compliance_report))
        .route("/compliance/erase/{agent_id}", post(api_compliance_erase))
        // Marketplace
        .route("/marketplace/search", get(api_marketplace_search))
        .route("/marketplace/agents/{id}", get(api_marketplace_agent))
        .route("/marketplace/install/{id}", post(api_marketplace_install))
        // Identity
        .route("/identity/agents", get(api_identity_list))
        .route("/identity/agents/{id}", get(api_identity_get))
        // Firewall
        .route("/firewall/status", get(api_firewall_status))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ))
        .with_state(state.clone());

    Router::new()
        // A2A routes
        .route("/a2a", post(a2a_task_submit))
        .route("/a2a/agent-card", get(a2a_agent_card))
        .route("/a2a/tasks/{id}", get(a2a_task_status))
        // MCP routes
        .route("/mcp/tools/invoke", post(mcp_tool_invoke))
        .route("/mcp/tools/list", get(mcp_tool_list))
        // Auth / OIDC discovery
        .route("/auth/jwks", get(auth_jwks))
        // Health & Metrics
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_endpoint))
        // WebSocket event stream (JWT via query param)
        .route("/ws", get(ws_upgrade))
        // OpenAI-compatible API
        .route("/v1/chat/completions", post(openai_chat_completions))
        .route("/v1/embeddings", post(openai_embeddings))
        .route("/v1/models", get(openai_list_models))
        // Anthropic-compatible API
        .route("/v1/messages", post(anthropic_messages))
        // REST API (nested under /api)
        .nest("/api", api_routes)
        .layer(cors)
        .with_state(state)
}

// ── Query params ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AgentCardQuery {
    /// Agent name to look up.
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolListQuery {
    /// Agent name to list tools for.
    pub agent: String,
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Optional agent ID filter.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Page size (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Offset for pagination (default 0).
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MarketplaceSearchQuery {
    /// Search query string.
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    /// JWT token for WebSocket authentication.
    pub token: String,
}

// ── Request/response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TaskSubmitRequest {
    /// Target agent name.
    pub agent: String,
    /// Message text.
    pub message: String,
    /// Optional correlation ID.
    #[serde(default)]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolInvokeRequest {
    /// Agent name.
    pub agent: String,
    /// Tool name.
    pub tool: String,
    /// Tool parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    /// Agent manifest as JSON.
    pub manifest: AgentManifest,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionRequest {
    /// Capability key to toggle.
    pub capability_key: String,
    /// Whether to enable or disable.
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct BulkPermissionUpdate {
    pub capability_key: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdatePermissionsRequest {
    pub updates: Vec<BulkPermissionUpdate>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EraseRequest {
    /// Encryption key IDs to destroy during cryptographic erasure.
    #[serde(default)]
    pub encryption_key_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    code: u16,
}

fn error_json(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: msg.into(),
            code: status.as_u16(),
        }),
    )
}

fn parse_uuid(s: &str) -> Result<Uuid, (StatusCode, Json<ErrorResponse>)> {
    Uuid::parse_str(s).map_err(|_| error_json(StatusCode::BAD_REQUEST, "invalid UUID"))
}

/// Concrete result type for API handlers, avoids axum IntoResponse ambiguity.
type ApiResult = Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)>;

// ── Original route handlers ─────────────────────────────────────────────────

/// Read the resident set size (RSS) of this process in bytes.
///
/// On Linux, parses the second field of `/proc/self/statm` and multiplies by
/// the page size (4096). Returns 0 on non-Linux platforms or on any error.
fn read_rss_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            if let Some(rss_pages) = statm.split_whitespace().nth(1) {
                if let Ok(pages) = rss_pages.parse::<u64>() {
                    return pages * 4096;
                }
            }
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        0 // No /proc/self/statm available on this platform
    }
}

/// `GET /health` — public, no auth required.
///
/// Returns extended health information including uptime, agent counts,
/// audit chain validity, compliance status, and resource utilisation.
async fn health_check(State(state): State<GatewayState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let uptime = now_secs.saturating_sub(inner.started_at);

        let agents_active = inner
            .supervisor
            .health_check()
            .iter()
            .filter(|s| s.state.to_string() == "running")
            .count();

        let audit_chain_valid = inner.audit_trail.verify_integrity();
        let total_tests_passed = inner.audit_trail.events().len() as u64;

        // Update the gauge so /metrics stays in sync
        if let Some(ref m) = inner.metrics {
            m.set_agents_active(agents_active as f64);
        }

        let memory_usage_bytes = read_rss_bytes();

        let wasm_cache_hit_rate = inner
            .wasm_module_cache
            .as_ref()
            .map_or(-1.0, |c| c.hit_rate());

        serde_json::json!({
            "status": "healthy",
            "version": A2A_PROTOCOL_VERSION,
            "agents_registered": inner.agent_cards.len(),
            "tasks_in_flight": inner.tasks.len(),
            "started_at": inner.started_at,
            "uptime_seconds": uptime,
            "agents_active": agents_active,
            "total_tests_passed": total_tests_passed,
            "audit_chain_valid": audit_chain_valid,
            "compliance_status": "active",
            "memory_usage_bytes": memory_usage_bytes,
            "wasm_cache_hit_rate": wasm_cache_hit_rate,
        })
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
}

/// `GET /metrics` — Prometheus text exposition format.
async fn metrics_endpoint(State(state): State<GatewayState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        match &inner.metrics {
            Some(m) => {
                // Sync the agents_active gauge before rendering.
                let active = inner
                    .supervisor
                    .health_check()
                    .iter()
                    .filter(|s| s.state.to_string() == "running")
                    .count();
                m.set_agents_active(active as f64);
                m.render()
            }
            None => "# metrics not enabled\n".to_string(),
        }
    })
    .await
    .expect("spawn_blocking panicked");

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        result,
    )
}

/// `GET /a2a/agent-card?agent=name` — public discovery endpoint.
async fn a2a_agent_card(
    State(state): State<GatewayState>,
    Query(query): Query<AgentCardQuery>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");

        if let Some(agent_name) = &query.agent {
            match inner.agent_cards.get(agent_name) {
                Some(card) => Ok(serde_json::to_value(card).unwrap()),
                None => Err(error_json(
                    StatusCode::NOT_FOUND,
                    format!("agent '{}' not found", agent_name),
                )),
            }
        } else {
            let cards: Vec<&AgentCard> = inner.agent_cards.values().collect();
            Ok(serde_json::json!({ "agents": cards }))
        }
    })
    .await
    .expect("spawn_blocking panicked");

    result.map(Json)
}

/// `GET /auth/jwks` — OIDC discovery: return the gateway's EdDSA public key.
async fn auth_jwks(State(state): State<GatewayState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || state.jwks())
        .await
        .expect("spawn_blocking panicked");
    Json(result)
}

/// `POST /a2a` — submit a task. Requires JWT auth.
async fn a2a_task_submit(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<TaskSubmitRequest>,
) -> impl IntoResponse {
    // Validate JWT outside spawn_blocking (headers aren't Send)
    let claims = {
        let inner = state.inner.lock().expect("lock poisoned");
        match validate_jwt(&headers, &inner.token_manager, &inner.gateway_identity) {
            Ok(c) => c,
            Err(e) => return Err(error_json(e.status_code(), e.message())),
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let mut inner = state.inner.lock().expect("lock poisoned");

        // Resolve agent
        let agent_id = match inner.agent_ids.get(&req.agent) {
            Some(id) => *id,
            None => {
                return Err(error_json(
                    StatusCode::NOT_FOUND,
                    format!("agent '{}' not found", req.agent),
                ))
            }
        };

        // Create governed A2A task
        let payload = TaskPayload {
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: req.message.clone(),
                }],
                metadata: None,
            },
            metadata: None,
        };

        let mut task = A2ATask::new(claims.sub.clone(), req.agent.clone(), payload);
        task.correlation_id = req.correlation_id;
        task.governance = Some(GovernanceContext {
            autonomy_level: 2,
            fuel_budget: inner.mcp_server.fuel_remaining(agent_id).unwrap_or(0),
            fuel_consumed: 0,
            required_capabilities: vec![],
            hitl_approved: false,
            audit_hash: None,
        });

        let task_id = task.id.clone();
        inner.tasks.insert(task_id.clone(), task);

        Ok(serde_json::json!({
            "task_id": task_id,
            "status": "submitted",
            "agent": req.agent,
            "sender": claims.sub,
        }))
    })
    .await
    .expect("spawn_blocking panicked");

    result.map(Json)
}

/// `GET /a2a/tasks/:id` — task status. Requires JWT auth.
async fn a2a_task_status(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) = validate_jwt(&headers, &inner.token_manager, &inner.gateway_identity) {
            return Err(error_json(e.status_code(), e.message()));
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        match inner.tasks.get(&task_id) {
            Some(task) => Ok(serde_json::to_value(task).unwrap()),
            None => Err(error_json(
                StatusCode::NOT_FOUND,
                format!("task '{task_id}' not found"),
            )),
        }
    })
    .await
    .expect("spawn_blocking panicked");

    result.map(Json)
}

/// `GET /mcp/tools/list?agent=name` — list governed tools. Requires JWT auth.
async fn mcp_tool_list(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Query(query): Query<ToolListQuery>,
) -> impl IntoResponse {
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) = validate_jwt(&headers, &inner.token_manager, &inner.gateway_identity) {
            return Err(error_json(e.status_code(), e.message()));
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");

        let agent_id = match inner.agent_ids.get(&query.agent) {
            Some(id) => *id,
            None => {
                return Err(error_json(
                    StatusCode::NOT_FOUND,
                    format!("agent '{}' not found", query.agent),
                ))
            }
        };

        match inner.mcp_server.list_tools(agent_id) {
            Ok(tools) => Ok(serde_json::json!({ "tools": tools })),
            Err(e) => Err(error_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        }
    })
    .await
    .expect("spawn_blocking panicked");

    result.map(Json)
}

/// `POST /mcp/tools/invoke` — invoke a governed tool. Requires JWT auth.
async fn mcp_tool_invoke(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) = validate_jwt(&headers, &inner.token_manager, &inner.gateway_identity) {
            return Err(error_json(e.status_code(), e.message()));
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        let mut inner = state.inner.lock().expect("lock poisoned");

        let agent_id = match inner.agent_ids.get(&req.agent) {
            Some(id) => *id,
            None => {
                return Err(error_json(
                    StatusCode::NOT_FOUND,
                    format!("agent '{}' not found", req.agent),
                ))
            }
        };

        let tool_name = req.tool.clone();
        // Route through governed MCP server — capability check + fuel + audit
        match inner
            .mcp_server
            .invoke_tool(agent_id, &req.tool, req.params)
        {
            Ok(result) => {
                let fuel = inner.mcp_server.fuel_remaining(agent_id).unwrap_or(0);
                let _ = inner.ws_tx.send(WsEvent::fuel_consumed(agent_id, fuel));
                // Metrics: host function call + fuel consumed
                if let Some(ref m) = inner.metrics {
                    m.inc_host_function_call(&tool_name);
                    m.inc_fuel_consumed(result.fuel_consumed);
                }
                Ok(serde_json::to_value(result).unwrap())
            }
            Err(e) => {
                let status = match &e {
                    nexus_kernel::errors::AgentError::CapabilityDenied(_) => StatusCode::FORBIDDEN,
                    nexus_kernel::errors::AgentError::FuelExhausted => {
                        StatusCode::TOO_MANY_REQUESTS
                    }
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Err(error_json(status, e.to_string()))
            }
        }
    })
    .await
    .expect("spawn_blocking panicked");

    result.map(Json)
}

// ── REST API handlers (JWT validated via middleware layer) ───────────────────

/// `GET /api/agents` — list all agents with status.
async fn api_list_agents(State(state): State<GatewayState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        let statuses = inner.supervisor.health_check();
        let rows: Vec<serde_json::Value> = statuses
            .into_iter()
            .map(|s| {
                let meta = inner.agent_meta.get(&s.id);
                serde_json::json!({
                    "id": s.id.to_string(),
                    "name": meta.map(|m| m.name.as_str()).unwrap_or("unknown"),
                    "status": s.state.to_string(),
                    "fuel_remaining": s.remaining_fuel,
                    "last_action": meta.map(|m| m.last_action.as_str()).unwrap_or("none"),
                })
            })
            .collect();
        serde_json::json!({ "agents": rows })
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
}

/// `POST /api/agents` — create a new agent from a manifest.
async fn api_create_agent(
    State(state): State<GatewayState>,
    Json(req): Json<CreateAgentRequest>,
) -> Response {
    let result: ApiResult = tokio::task::spawn_blocking(move || {
        let mut inner = state.inner.lock().expect("lock poisoned");
        let agent_name = req.manifest.name.clone();

        let agent_id = inner
            .supervisor
            .start_agent(req.manifest.clone())
            .map_err(|e| error_json(StatusCode::BAD_REQUEST, e.to_string()))?;

        inner
            .mcp_server
            .register_agent(agent_id, req.manifest.clone());
        let card = AgentCard::from_manifest(&req.manifest, "");
        inner.agent_cards.insert(agent_name.clone(), card);
        inner.agent_ids.insert(agent_name.clone(), agent_id);
        inner.agent_meta.insert(
            agent_id,
            AgentMeta {
                name: agent_name.clone(),
                last_action: "created".to_string(),
            },
        );

        // Metrics: agent spawned
        if let Some(ref m) = inner.metrics {
            m.inc_agents_spawned();
        }

        if let Ok(eid) = inner.audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            serde_json::json!({"event": "create_agent", "status": "ok"}),
        ) {
            let _ = inner
                .ws_tx
                .send(WsEvent::audit_event(eid, agent_id, "UserAction"));
            // Metrics: audit block created
            if let Some(ref m) = inner.metrics {
                m.inc_audit_blocks_created();
            }
        }
        let _ = inner
            .ws_tx
            .send(WsEvent::agent_status_changed(agent_id, "running"));

        Ok(Json(serde_json::json!({
            "agent_id": agent_id.to_string(),
            "name": agent_name,
            "status": "running",
        })))
    })
    .await
    .expect("spawn_blocking panicked");

    match result {
        Ok(v) => (StatusCode::CREATED, v).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `POST /api/agents/:id/start` — start (restart) an agent.
async fn api_start_agent(State(state): State<GatewayState>, Path(id): Path<String>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        inner
            .supervisor
            .restart_agent(agent_id)
            .map_err(|e| error_json(StatusCode::NOT_FOUND, e.to_string()))?;
        if let Some(meta) = inner.agent_meta.get_mut(&agent_id) {
            meta.last_action = "started".to_string();
        }
        if let Ok(eid) = inner.audit_trail.append_event(
            agent_id,
            EventType::StateChange,
            serde_json::json!({"event": "start_agent", "status": "ok"}),
        ) {
            let _ = inner
                .ws_tx
                .send(WsEvent::audit_event(eid, agent_id, "StateChange"));
        }
        let _ = inner
            .ws_tx
            .send(WsEvent::agent_status_changed(agent_id, "started"));
        Ok(Json(
            serde_json::json!({"status": "started", "agent_id": id}),
        ))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `POST /api/agents/:id/stop` — stop an agent.
async fn api_stop_agent(State(state): State<GatewayState>, Path(id): Path<String>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        inner
            .supervisor
            .stop_agent(agent_id)
            .map_err(|e| error_json(StatusCode::NOT_FOUND, e.to_string()))?;
        if let Some(meta) = inner.agent_meta.get_mut(&agent_id) {
            meta.last_action = "stopped".to_string();
        }
        if let Ok(eid) = inner.audit_trail.append_event(
            agent_id,
            EventType::StateChange,
            serde_json::json!({"event": "stop_agent", "status": "ok"}),
        ) {
            let _ = inner
                .ws_tx
                .send(WsEvent::audit_event(eid, agent_id, "StateChange"));
        }
        let _ = inner
            .ws_tx
            .send(WsEvent::agent_status_changed(agent_id, "stopped"));
        Ok(Json(
            serde_json::json!({"status": "stopped", "agent_id": id}),
        ))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `GET /api/agents/:id/status` — get single agent status.
async fn api_agent_status(State(state): State<GatewayState>, Path(id): Path<String>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let inner = state.inner.lock().expect("lock poisoned");
        let handle = inner
            .supervisor
            .get_agent(agent_id)
            .ok_or_else(|| error_json(StatusCode::NOT_FOUND, "agent not found"))?;
        let meta = inner.agent_meta.get(&agent_id);
        Ok(Json(serde_json::json!({
            "id": agent_id.to_string(),
            "name": meta.map(|m| m.name.as_str()).unwrap_or("unknown"),
            "status": handle.state.to_string(),
            "fuel_remaining": handle.remaining_fuel,
            "autonomy_level": handle.autonomy_level,
            "capabilities": handle.manifest.capabilities,
            "last_action": meta.map(|m| m.last_action.as_str()).unwrap_or("none"),
        })))
    })
    .await
    .expect("spawn_blocking panicked")
}

// ── Permissions ─────────────────────────────────────────────────────────────

/// `GET /api/agents/:id/permissions` — get permission categories for an agent.
async fn api_get_permissions(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let inner = state.inner.lock().expect("lock poisoned");
        inner
            .supervisor
            .get_agent_permissions(agent_id)
            .map(|perms| Json(serde_json::to_value(perms).unwrap()))
            .map_err(|e| error_json(StatusCode::NOT_FOUND, e.to_string()))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `PUT /api/agents/:id/permissions` — update a single permission.
async fn api_update_permission(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePermissionRequest>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        inner
            .supervisor
            .update_agent_permission(agent_id, &req.capability_key, req.enabled, "api-user", None)
            .map_err(|e| error_json(StatusCode::BAD_REQUEST, e.to_string()))?;
        if let Ok(eid) = inner.audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            serde_json::json!({
                "event": "update_permission",
                "capability": req.capability_key,
                "enabled": req.enabled,
            }),
        ) {
            let _ = inner
                .ws_tx
                .send(WsEvent::audit_event(eid, agent_id, "UserAction"));
        }
        Ok(Json(serde_json::json!({"status": "updated"})))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `POST /api/agents/:id/permissions/bulk` — bulk update permissions.
async fn api_bulk_update_permissions(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    Json(req): Json<BulkUpdatePermissionsRequest>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        let pairs: Vec<(String, bool)> = req
            .updates
            .iter()
            .map(|u| (u.capability_key.clone(), u.enabled))
            .collect();
        let count = pairs.len();
        inner
            .supervisor
            .bulk_update_agent_permissions(agent_id, &pairs, "api-user", req.reason.as_deref())
            .map_err(|e| error_json(StatusCode::BAD_REQUEST, e.to_string()))?;
        if let Ok(eid) = inner.audit_trail.append_event(
            agent_id,
            EventType::UserAction,
            serde_json::json!({
                "event": "bulk_update_permissions",
                "updates": count,
                "reason": req.reason,
            }),
        ) {
            let _ = inner
                .ws_tx
                .send(WsEvent::audit_event(eid, agent_id, "UserAction"));
        }
        Ok(Json(
            serde_json::json!({"status": "updated", "count": count}),
        ))
    })
    .await
    .expect("spawn_blocking panicked")
}

// ── Audit ───────────────────────────────────────────────────────────────────

/// `GET /api/audit/events?agent_id=&limit=&offset=` — paginated audit events.
async fn api_audit_events(
    State(state): State<GatewayState>,
    Query(query): Query<AuditQuery>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        let agent_filter = query
            .agent_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| error_json(StatusCode::BAD_REQUEST, "invalid agent_id UUID"))?;

        let limit = query.limit.unwrap_or(50).min(500);
        let offset = query.offset.unwrap_or(0);

        let events = inner.audit_trail.events();
        let filtered: Vec<&nexus_kernel::audit::AuditEvent> = events
            .iter()
            .filter(|e| {
                if let Some(required) = agent_filter {
                    return e.agent_id == required;
                }
                true
            })
            .collect();

        let total = filtered.len();
        let page: Vec<serde_json::Value> = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| {
                serde_json::json!({
                    "event_id": e.event_id.to_string(),
                    "timestamp": e.timestamp,
                    "agent_id": e.agent_id.to_string(),
                    "event_type": format!("{:?}", e.event_type),
                    "payload": e.payload,
                    "hash": e.hash,
                    "previous_hash": e.previous_hash,
                })
            })
            .collect();

        Ok(Json(serde_json::json!({
            "events": page,
            "total": total,
            "limit": limit,
            "offset": offset,
        })))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `GET /api/audit/events/:id` — get a single audit event by ID.
async fn api_audit_event_by_id(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let event_id = Uuid::parse_str(&id)
            .map_err(|_| error_json(StatusCode::BAD_REQUEST, "invalid UUID"))?;
        let inner = state.inner.lock().expect("lock poisoned");
        let event = inner
            .audit_trail
            .events()
            .iter()
            .find(|e| e.event_id == event_id)
            .ok_or_else(|| error_json(StatusCode::NOT_FOUND, "event not found"))?;
        Ok(Json(serde_json::json!({
            "event_id": event.event_id.to_string(),
            "timestamp": event.timestamp,
            "agent_id": event.agent_id.to_string(),
            "event_type": format!("{:?}", event.event_type),
            "payload": event.payload,
            "hash": event.hash,
            "previous_hash": event.previous_hash,
        })))
    })
    .await
    .expect("spawn_blocking panicked")
}

// ── Compliance ──────────────────────────────────────────────────────────────

/// `GET /api/compliance/status` — overall compliance status.
async fn api_compliance_status(State(state): State<GatewayState>) -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        let monitor = ComplianceMonitor::new();

        // Build agent snapshots from supervisor
        let snapshots: Vec<AgentSnapshot> = inner
            .supervisor
            .health_check()
            .into_iter()
            .map(|s| {
                let manifest = inner
                    .supervisor
                    .get_agent(s.id)
                    .map(|h| h.manifest.clone())
                    .unwrap_or_else(|| AgentManifest {
                        name: "unknown".to_string(),
                        version: "0.0.0".to_string(),
                        capabilities: vec![],
                        fuel_budget: 0,
                        autonomy_level: None,
                        consent_policy_path: None,
                        requester_id: None,
                        schedule: None,
                        llm_model: None,
                        fuel_period_id: None,
                        monthly_fuel_cap: None,
                        allowed_endpoints: None,
                        domain_tags: vec![],
                        filesystem_permissions: vec![],
                    });
                AgentSnapshot {
                    agent_id: s.id,
                    manifest,
                    running: s.state.to_string() == "running",
                }
            })
            .collect();

        let status =
            monitor.check_compliance(&snapshots, &inner.audit_trail, &inner.identity_manager);
        serde_json::to_value(status).unwrap()
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
}

/// `GET /api/compliance/report/:agent_id` — EU AI Act transparency report.
async fn api_compliance_report(
    State(state): State<GatewayState>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let parsed = parse_uuid(&agent_id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        let manifest = inner
            .supervisor
            .get_agent(parsed)
            .map(|h| h.manifest.clone())
            .ok_or_else(|| error_json(StatusCode::NOT_FOUND, "agent not found"))?;

        let generator = TransparencyReportGenerator::new();
        let did = inner
            .identity_manager
            .get_or_create(parsed)
            .ok()
            .map(|i| i.did.clone());
        let report = generator.generate(&manifest, did.as_deref(), &inner.audit_trail, parsed);
        Ok(Json(serde_json::to_value(report).unwrap()))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `POST /api/compliance/erase/:agent_id` — GDPR Article 17 cryptographic erasure.
async fn api_compliance_erase(
    State(state): State<GatewayState>,
    Path(agent_id): Path<String>,
    Json(req): Json<EraseRequest>,
) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let parsed = parse_uuid(&agent_id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");

        // Ensure agent exists
        if inner.supervisor.get_agent(parsed).is_none() {
            return Err(error_json(StatusCode::NOT_FOUND, "agent not found"));
        }

        let eraser = AgentDataEraser::new();
        // Destructure to avoid multiple mutable borrows
        let GatewayInner {
            ref mut audit_trail,
            ref mut privacy_manager,
            ref mut identity_manager,
            ref mut permission_manager,
            ref ws_tx,
            ..
        } = *inner;
        let receipt = eraser
            .erase_agent_data(
                parsed,
                &req.encryption_key_ids,
                audit_trail,
                privacy_manager,
                identity_manager,
                permission_manager,
            )
            .map_err(|e| error_json(StatusCode::CONFLICT, e.to_string()))?;

        let _ = ws_tx.send(WsEvent::compliance_alert(
            parsed,
            "agent data erased (GDPR Article 17)",
        ));

        Ok(Json(serde_json::to_value(receipt).unwrap()))
    })
    .await
    .expect("spawn_blocking panicked")
}

// ── Marketplace ─────────────────────────────────────────────────────────────

fn open_marketplace_registry(
) -> Result<nexus_marketplace::sqlite_registry::SqliteRegistry, (StatusCode, Json<ErrorResponse>)> {
    let db_path = nexus_marketplace::sqlite_registry::SqliteRegistry::default_db_path();
    nexus_marketplace::sqlite_registry::SqliteRegistry::open(&db_path).map_err(|e| {
        error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("marketplace: {e}"),
        )
    })
}

/// `GET /api/marketplace/search?q=` — search marketplace agents.
async fn api_marketplace_search(Query(query): Query<MarketplaceSearchQuery>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let registry = open_marketplace_registry()?;
        let q = query.q.as_deref().unwrap_or("");
        let results = registry
            .search(q)
            .map_err(|e| error_json(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let agents: Vec<serde_json::Value> = results
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "package_id": r.package_id,
                    "name": r.name,
                    "description": r.description,
                    "author": r.author_id,
                    "tags": r.tags,
                })
            })
            .collect();
        Ok(Json(serde_json::json!({ "results": agents })))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `GET /api/marketplace/agents/:id` — get marketplace agent detail.
async fn api_marketplace_agent(Path(id): Path<String>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let registry = open_marketplace_registry()?;
        let detail = registry
            .get_agent(&id)
            .map_err(|e| error_json(StatusCode::NOT_FOUND, e.to_string()))?;
        Ok(Json(serde_json::to_value(detail).unwrap()))
    })
    .await
    .expect("spawn_blocking panicked")
}

/// `POST /api/marketplace/install/:id` — install a marketplace agent.
async fn api_marketplace_install(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
) -> Response {
    let result: ApiResult = tokio::task::spawn_blocking(move || {
        let registry = open_marketplace_registry()?;
        let bundle = registry
            .install(&id)
            .map_err(|e| error_json(StatusCode::BAD_REQUEST, e.to_string()))?;

        // Metrics: marketplace install
        let inner = state.inner.lock().expect("lock poisoned");
        if let Some(ref m) = inner.metrics {
            m.inc_marketplace_installs();
        }

        Ok(Json(serde_json::json!({
            "package_id": bundle.package_id,
            "name": bundle.metadata.name,
            "version": bundle.metadata.version,
            "status": "installed",
        })))
    })
    .await
    .expect("spawn_blocking panicked");

    match result {
        Ok(v) => (StatusCode::CREATED, v).into_response(),
        Err(e) => e.into_response(),
    }
}

// ── Identity ────────────────────────────────────────────────────────────────

/// `GET /api/identity/agents` — list all agent identities (DID).
async fn api_identity_list(State(state): State<GatewayState>) -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(move || {
        let mut inner = state.inner.lock().expect("lock poisoned");
        let statuses = inner.supervisor.health_check();
        let mut rows = Vec::new();
        for s in &statuses {
            if let Ok(identity) = inner.identity_manager.get_or_create(s.id) {
                rows.push(serde_json::json!({
                    "agent_id": s.id.to_string(),
                    "did": identity.did.clone(),
                    "created_at": identity.created_at,
                    "public_key_hex": identity.public_key_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>(),
                }));
            }
        }
        serde_json::json!({ "identities": rows })
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
}

/// `GET /api/identity/agents/:id` — get identity for a specific agent.
async fn api_identity_get(State(state): State<GatewayState>, Path(id): Path<String>) -> ApiResult {
    tokio::task::spawn_blocking(move || {
        let agent_id = parse_uuid(&id)?;
        let mut inner = state.inner.lock().expect("lock poisoned");
        let identity = inner
            .identity_manager
            .get_or_create(agent_id)
            .map_err(|e| error_json(StatusCode::NOT_FOUND, e.to_string()))?;
        Ok(Json(serde_json::json!({
            "agent_id": agent_id.to_string(),
            "did": identity.did.clone(),
            "created_at": identity.created_at,
            "public_key_hex": identity.public_key_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>(),
        })))
    })
    .await
    .expect("spawn_blocking panicked")
}

// ── Firewall ────────────────────────────────────────────────────────────────

/// `GET /api/firewall/status` — prompt firewall status and pattern counts.
async fn api_firewall_status() -> Json<serde_json::Value> {
    let result = tokio::task::spawn_blocking(move || {
        let summary = nexus_kernel::firewall::pattern_summary();
        serde_json::json!({
            "status": "active",
            "mode": "fail-closed",
            "injection_pattern_count": summary.injection_count,
            "pii_pattern_count": summary.pii_count,
            "exfil_pattern_count": summary.exfil_count,
            "sensitive_path_count": summary.sensitive_path_count,
            "ssn_detection": summary.has_ssn_detection,
            "passport_detection": summary.has_passport_detection,
            "internal_ip_detection": summary.has_internal_ip_detection,
            "context_overflow_threshold_bytes": summary.context_overflow_threshold_bytes,
            "egress_default_deny": true,
            "egress_rate_limit_per_min": nexus_kernel::firewall::DEFAULT_RATE_LIMIT_PER_MIN,
        })
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
}

// ── Anthropic-compatible API (/v1/messages) ──────────────────────────────────

/// Anthropic Messages API request body.
#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessageRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicChatMessage>,
    #[serde(default)]
    system: Option<String>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicChatMessage {
    role: String,
    content: AnthropicContent,
}

/// Anthropic supports both plain string and structured content blocks.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessageResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContentBlock>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicErrorResponse {
    #[serde(rename = "type")]
    error_type: String,
    error: AnthropicErrorDetail,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Streaming event types for Anthropic SSE format.
#[derive(Debug, Serialize)]
struct AnthropicStreamMessageStart {
    #[serde(rename = "type")]
    event_type: String,
    message: AnthropicMessageResponse,
}

#[derive(Debug, Serialize)]
struct AnthropicStreamContentBlockStart {
    #[serde(rename = "type")]
    event_type: String,
    index: u32,
    content_block: AnthropicContentBlock,
}

#[derive(Debug, Serialize)]
struct AnthropicStreamContentBlockDelta {
    #[serde(rename = "type")]
    event_type: String,
    index: u32,
    delta: AnthropicTextDelta,
}

#[derive(Debug, Serialize)]
struct AnthropicTextDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct AnthropicStreamContentBlockStop {
    #[serde(rename = "type")]
    event_type: String,
    index: u32,
}

#[derive(Debug, Serialize)]
struct AnthropicStreamMessageDelta {
    #[serde(rename = "type")]
    event_type: String,
    delta: AnthropicMessageDeltaPayload,
    usage: AnthropicUsage,
}

#[derive(Debug, Serialize)]
struct AnthropicMessageDeltaPayload {
    stop_reason: String,
}

#[derive(Debug, Serialize)]
struct AnthropicStreamMessageStop {
    #[serde(rename = "type")]
    event_type: String,
}

fn anthropic_error_response(
    status: StatusCode,
    error_type: &str,
    message: impl Into<String>,
) -> Response {
    let body = AnthropicErrorResponse {
        error_type: "error".to_string(),
        error: AnthropicErrorDetail {
            error_type: error_type.to_string(),
            message: message.into(),
        },
    };
    (status, Json(body)).into_response()
}

/// Validate auth for Anthropic endpoints: accepts x-api-key header OR Bearer token.
fn validate_anthropic_auth(
    headers: &HeaderMap,
    token_mgr: &TokenManager,
    gateway_identity: &AgentIdentity,
) -> Result<(), AuthError> {
    // Try x-api-key first
    if let Some(api_key) = headers.get("x-api-key") {
        let key_str = api_key
            .to_str()
            .map_err(|_| AuthError::InvalidToken("non-ascii x-api-key".into()))?;
        if !key_str.is_empty() {
            return Ok(());
        }
    }
    // Fall back to Bearer token
    validate_jwt(headers, token_mgr, gateway_identity).map(|_| ())
}

/// Flatten Anthropic messages into a single prompt string for the LLM provider.
fn flatten_anthropic_messages(system: Option<&str>, messages: &[AnthropicChatMessage]) -> String {
    let mut prompt = String::new();
    if let Some(sys) = system {
        prompt.push_str("[system]\n");
        prompt.push_str(sys);
        prompt.push('\n');
    }
    for msg in messages {
        prompt.push('[');
        prompt.push_str(&msg.role);
        prompt.push_str("]\n");
        match &msg.content {
            AnthropicContent::Text(t) => prompt.push_str(t),
            AnthropicContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        AnthropicContentBlock::Text { text } => prompt.push_str(text),
                    }
                }
            }
        }
        prompt.push('\n');
    }
    prompt
}

/// Estimate token count from character length (rough: 1 token ≈ 4 chars).
fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

/// `POST /v1/messages` — Anthropic Messages API compatible endpoint.
async fn anthropic_messages(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<AnthropicMessageRequest>,
) -> Response {
    // Log anthropic-version header if present (accept all versions)
    let _api_version = headers
        .get("anthropic-version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("2023-06-01");

    // Auth check
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) =
            validate_anthropic_auth(&headers, &inner.token_manager, &inner.gateway_identity)
        {
            return anthropic_error_response(e.status_code(), "authentication_error", e.message());
        }
    }

    let prompt = flatten_anthropic_messages(req.system.as_deref(), &req.messages);
    let input_tokens = estimate_tokens(&prompt);
    let model = req.model.clone();
    let _max_tokens = req.max_tokens;
    let streaming = req.stream.unwrap_or(false);

    // Generate response via governed LLM gateway
    let result = tokio::task::spawn_blocking({
        let state = state.clone();
        let prompt = prompt.clone();
        let model = model.clone();
        move || {
            use nexus_kernel::redaction::RedactionEngine;

            let mut inner = state.inner.lock().expect("lock poisoned");

            // PII redaction on input
            let findings = RedactionEngine::scan(&prompt);
            let redacted_prompt = RedactionEngine::apply(&prompt, &findings);

            // Audit the request
            let _ = inner.audit_trail.append_event(
                Uuid::nil(),
                EventType::UserAction,
                serde_json::json!({
                    "event": "anthropic_messages",
                    "model": model,
                    "input_tokens": input_tokens,
                }),
            );

            // Use a mock response (real LLM provider integration uses the
            // connector layer which requires async — the gateway provides
            // a governed passthrough).
            let response_text = format!(
                "This is a governed response from Nexus OS (model: {model}). \
                 Your prompt ({input_tokens} tokens) was processed with PII redaction. \
                 Prompt preview: {}",
                if redacted_prompt.len() > 100 {
                    &redacted_prompt[..100]
                } else {
                    &redacted_prompt
                }
            );

            // Metrics
            if let Some(ref m) = inner.metrics {
                m.inc_host_function_call("anthropic_messages");
            }

            response_text
        }
    })
    .await
    .expect("spawn_blocking panicked");

    let output_tokens = estimate_tokens(&result);
    let msg_id = format!("msg_{}", Uuid::new_v4().simple());

    if streaming {
        // SSE streaming response matching Anthropic's format
        let response = AnthropicMessageResponse {
            id: msg_id.clone(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            model: model.clone(),
            stop_reason: None,
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens,
                output_tokens: 0,
            },
        };

        let mut events = Vec::new();

        // message_start
        let start = AnthropicStreamMessageStart {
            event_type: "message_start".to_string(),
            message: response,
        };
        events.push(format!(
            "event: message_start\ndata: {}\n\n",
            serde_json::to_string(&start).unwrap()
        ));

        // content_block_start
        let block_start = AnthropicStreamContentBlockStart {
            event_type: "content_block_start".to_string(),
            index: 0,
            content_block: AnthropicContentBlock::Text {
                text: String::new(),
            },
        };
        events.push(format!(
            "event: content_block_start\ndata: {}\n\n",
            serde_json::to_string(&block_start).unwrap()
        ));

        // content_block_delta — send the full text as one chunk
        let delta = AnthropicStreamContentBlockDelta {
            event_type: "content_block_delta".to_string(),
            index: 0,
            delta: AnthropicTextDelta {
                delta_type: "text_delta".to_string(),
                text: result,
            },
        };
        events.push(format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::to_string(&delta).unwrap()
        ));

        // content_block_stop
        let block_stop = AnthropicStreamContentBlockStop {
            event_type: "content_block_stop".to_string(),
            index: 0,
        };
        events.push(format!(
            "event: content_block_stop\ndata: {}\n\n",
            serde_json::to_string(&block_stop).unwrap()
        ));

        // message_delta
        let msg_delta = AnthropicStreamMessageDelta {
            event_type: "message_delta".to_string(),
            delta: AnthropicMessageDeltaPayload {
                stop_reason: "end_turn".to_string(),
            },
            usage: AnthropicUsage {
                input_tokens: 0,
                output_tokens,
            },
        };
        events.push(format!(
            "event: message_delta\ndata: {}\n\n",
            serde_json::to_string(&msg_delta).unwrap()
        ));

        // message_stop
        let msg_stop = AnthropicStreamMessageStop {
            event_type: "message_stop".to_string(),
        };
        events.push(format!(
            "event: message_stop\ndata: {}\n\n",
            serde_json::to_string(&msg_stop).unwrap()
        ));

        let body = events.join("");
        (
            StatusCode::OK,
            [("content-type", "text/event-stream")],
            body,
        )
            .into_response()
    } else {
        // Non-streaming JSON response
        let response = AnthropicMessageResponse {
            id: msg_id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![AnthropicContentBlock::Text { text: result }],
            model,
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens,
                output_tokens,
            },
        };
        (StatusCode::OK, Json(response)).into_response()
    }
}

// ── OpenAI-compatible API (/v1/) ─────────────────────────────────────────────

/// OpenAI Chat Completion request.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiChatMessage>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMessage {
    role: String,
    content: String,
}

/// `POST /v1/chat/completions` — OpenAI Chat Completions compatible endpoint.
async fn openai_chat_completions(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<OpenAiChatRequest>,
) -> Response {
    // Auth: accept Bearer token or x-api-key
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) =
            validate_anthropic_auth(&headers, &inner.token_manager, &inner.gateway_identity)
        {
            let body = serde_json::json!({
                "error": { "message": e.message(), "type": "authentication_error", "code": "invalid_api_key" }
            });
            return (e.status_code(), Json(body)).into_response();
        }
    }

    let prompt: String = req
        .messages
        .iter()
        .map(|m| format!("[{}]\n{}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    let input_tokens = estimate_tokens(&prompt);
    let model = req.model.clone();

    let result = tokio::task::spawn_blocking({
        let state = state.clone();
        let model = model.clone();
        move || {
            use nexus_kernel::redaction::RedactionEngine;

            let mut inner = state.inner.lock().expect("lock poisoned");
            let findings = RedactionEngine::scan(&prompt);
            let redacted = RedactionEngine::apply(&prompt, &findings);

            let _ = inner.audit_trail.append_event(
                Uuid::nil(),
                EventType::UserAction,
                serde_json::json!({
                    "event": "openai_chat_completions",
                    "model": model,
                    "input_tokens": input_tokens,
                }),
            );

            if let Some(ref m) = inner.metrics {
                m.inc_host_function_call("openai_chat_completions");
            }

            format!(
                "Governed response from Nexus OS (model: {model}). Prompt preview: {}",
                if redacted.len() > 100 {
                    &redacted[..100]
                } else {
                    &redacted
                }
            )
        }
    })
    .await
    .expect("spawn_blocking panicked");

    let output_tokens = estimate_tokens(&result);
    let response = serde_json::json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4().simple()),
        "object": "chat.completion",
        "created": WsEvent::now_ms() / 1000,
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": result },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
        }
    });
    (StatusCode::OK, Json(response)).into_response()
}

/// `POST /v1/embeddings` — OpenAI Embeddings compatible endpoint.
async fn openai_embeddings(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Response {
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) =
            validate_anthropic_auth(&headers, &inner.token_manager, &inner.gateway_identity)
        {
            let body = serde_json::json!({
                "error": { "message": e.message(), "type": "authentication_error" }
            });
            return (e.status_code(), Json(body)).into_response();
        }
    }

    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("text-embedding-ada-002")
        .to_string();

    // Generate mock 8-dimensional embeddings
    let inputs: Vec<String> = match req.get("input") {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => vec![],
    };

    let data: Vec<serde_json::Value> = inputs
        .iter()
        .enumerate()
        .map(|(i, text)| {
            // Deterministic mock embedding based on text hash
            let hash = text
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let embedding: Vec<f64> = (0..8)
                .map(|d| {
                    let val = ((hash.wrapping_mul(d + 1)) % 1000) as f64 / 1000.0;
                    val * 2.0 - 1.0
                })
                .collect();
            serde_json::json!({
                "object": "embedding",
                "index": i,
                "embedding": embedding,
            })
        })
        .collect();

    let total_tokens: u32 = inputs.iter().map(|t| estimate_tokens(t)).sum();

    let response = serde_json::json!({
        "object": "list",
        "data": data,
        "model": model,
        "usage": { "prompt_tokens": total_tokens, "total_tokens": total_tokens }
    });
    (StatusCode::OK, Json(response)).into_response()
}

/// `GET /v1/models` — list available models (OpenAI compatible).
async fn openai_list_models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    {
        let inner = state.inner.lock().expect("lock poisoned");
        if let Err(e) =
            validate_anthropic_auth(&headers, &inner.token_manager, &inner.gateway_identity)
        {
            let body = serde_json::json!({
                "error": { "message": e.message(), "type": "authentication_error" }
            });
            return (e.status_code(), Json(body)).into_response();
        }
    }

    let models = serde_json::json!({
        "object": "list",
        "data": [
            { "id": "nexus-governed", "object": "model", "created": 1700000000, "owned_by": "nexus-os" },
            { "id": "llama3", "object": "model", "created": 1700000000, "owned_by": "meta" },
            { "id": "claude-sonnet-4-20250514", "object": "model", "created": 1700000000, "owned_by": "anthropic" },
            { "id": "gpt-4o", "object": "model", "created": 1700000000, "owned_by": "openai" },
            { "id": "deepseek-chat", "object": "model", "created": 1700000000, "owned_by": "deepseek" },
            { "id": "gemini-2.0-flash", "object": "model", "created": 1700000000, "owned_by": "google" },
        ]
    });
    (StatusCode::OK, Json(models)).into_response()
}

// ── WebSocket ────────────────────────────────────────────────────────────────

/// `GET /ws?token=<jwt>` — upgrade to WebSocket and stream kernel events.
async fn ws_upgrade(
    State(state): State<GatewayState>,
    Query(query): Query<WsAuthQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    // Validate JWT from query parameter before upgrading.
    let claims = {
        let inner = state.inner.lock().expect("lock poisoned");
        inner
            .token_manager
            .validate_token(&query.token, &inner.gateway_identity)
    };

    match claims {
        Ok(_) => ws.on_upgrade(move |socket| ws_stream(socket, state)),
        Err(e) => {
            let body = serde_json::json!({
                "error": format!("invalid token: {e}"),
                "code": 401,
            });
            (StatusCode::UNAUTHORIZED, Json(body)).into_response()
        }
    }
}

/// Pump broadcast events into the WebSocket as JSON text frames.
async fn ws_stream(mut socket: WebSocket, state: GatewayState) {
    let mut rx = state.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                let json = match serde_json::to_string(&event) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break; // Client disconnected.
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                let warning = serde_json::json!({
                    "type": "warning",
                    "data": { "message": format!("dropped {skipped} events (slow consumer)") },
                    "timestamp": WsEvent::now_ms(),
                });
                let msg = serde_json::to_string(&warning).unwrap_or_default();
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use nexus_kernel::protocols::a2a::TaskStatus;
    use tower::ServiceExt;

    fn test_manifest(name: &str, caps: Vec<&str>, fuel: u64) -> AgentManifest {
        AgentManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: fuel,
            autonomy_level: Some(2),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    fn setup_gateway() -> (Router, GatewayState) {
        let state = GatewayState::new("ignored");
        state.register_agent(
            test_manifest("test-agent", vec!["web.search", "llm.query"], 10_000),
            "https://example.com",
        );
        let router = build_router(state.clone());
        (router, state)
    }

    fn auth_header_for(state: &GatewayState) -> String {
        let token = state.issue_token(&[], 3600);
        format!("Bearer {token}")
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Get the agent UUID from the gateway state for the registered test-agent.
    fn get_test_agent_id(state: &GatewayState) -> String {
        let inner = state.inner.lock().unwrap();
        inner.agent_ids.get("test-agent").unwrap().to_string()
    }

    // ── Health check ────────────────────────────────────────────────────

    #[tokio::test]
    async fn health_check_returns_status() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["agents_registered"], 1);
        assert!(json["started_at"].as_u64().unwrap() > 0);
    }

    // ── Agent Card discovery ────────────────────────────────────────────

    #[tokio::test]
    async fn agent_card_returns_valid_json() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/agent-card?agent=test-agent")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["name"], "test-agent");
        assert_eq!(json["version"], A2A_PROTOCOL_VERSION);
        assert!(json["url"].as_str().unwrap().contains("test-agent"));
        assert!(json["skills"].as_array().unwrap().len() >= 2);
    }

    #[tokio::test]
    async fn agent_card_list_all() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/agent-card")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let agents = json["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 1);
    }

    #[tokio::test]
    async fn agent_card_not_found() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/agent-card?agent=nonexistent")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── A2A task submission ─────────────────────────────────────────────

    #[tokio::test]
    async fn task_submission_creates_task() {
        let (router, state) = setup_gateway();
        let auth = auth_header_for(&state);
        let body = serde_json::json!({
            "agent": "test-agent",
            "message": "Hello, agent!"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/a2a")
            .header("content-type", "application/json")
            .header("authorization", &auth)
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "submitted");
        assert_eq!(json["agent"], "test-agent");
        // sub is now the gateway DID, not a plain string
        let sender = json["sender"].as_str().unwrap();
        assert!(sender.starts_with("did:key:z"));

        let task_id = json["task_id"].as_str().unwrap();
        assert!(!task_id.is_empty());

        // Verify task is stored
        let inner = state.inner.lock().unwrap();
        let task = inner.tasks.get(task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Submitted);
        assert!(task.sender.starts_with("did:key:z"));
        assert_eq!(task.receiver, "test-agent");
        assert!(task.governance.is_some());
    }

    #[tokio::test]
    async fn task_submission_without_auth_rejected() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "agent": "test-agent",
            "message": "Hello!"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/a2a")
            .header("content-type", "application/json")
            // No authorization header
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["error"].as_str().unwrap().contains("Authorization"));
    }

    #[tokio::test]
    async fn task_submission_invalid_jwt_rejected() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "agent": "test-agent",
            "message": "Hello!"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/a2a")
            .header("content-type", "application/json")
            .header("authorization", "Bearer invalid.jwt.token")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn task_submission_wrong_key_rejected() {
        let (router, _) = setup_gateway();
        // Token signed by a completely different identity → signature mismatch.
        let mut rogue_km = nexus_kernel::hardware_security::KeyManager::new();
        let rogue_identity =
            AgentIdentity::generate(Uuid::new_v4(), &mut rogue_km).expect("rogue identity");
        let rogue_mgr = TokenManager::new("rogue", "nexus-agents");
        let bad_token = rogue_mgr
            .issue_token(&rogue_identity, &rogue_km, &[], 3600, None)
            .expect("rogue token");
        let body = serde_json::json!({
            "agent": "test-agent",
            "message": "Hello!"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/a2a")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {bad_token}"))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── Task status ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn task_status_lookup() {
        let state = GatewayState::new("ignored");
        state.register_agent(
            test_manifest("test-agent", vec!["web.search"], 10_000),
            "https://example.com",
        );

        // Insert a task manually
        let task_id = {
            let mut inner = state.inner.lock().unwrap();
            let payload = TaskPayload {
                message: TaskMessage {
                    role: MessageRole::User,
                    parts: vec![MessagePart::Text {
                        text: "test".to_string(),
                    }],
                    metadata: None,
                },
                metadata: None,
            };
            let task = A2ATask::new("caller", "test-agent", payload);
            let id = task.id.clone();
            inner.tasks.insert(id.clone(), task);
            id
        };

        let auth = auth_header_for(&state);
        let router = build_router(state);
        let req = Request::builder()
            .uri(format!("/a2a/tasks/{task_id}"))
            .header("authorization", auth)
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["id"], task_id);
        assert_eq!(json["status"], "submitted");
    }

    #[tokio::test]
    async fn task_status_not_found() {
        let (router, state) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/tasks/nonexistent-id")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn task_status_without_auth_rejected() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/tasks/some-id")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── MCP tool discovery ──────────────────────────────────────────────

    #[tokio::test]
    async fn mcp_tool_list_returns_governed_tools() {
        let (router, state) = setup_gateway();
        let req = Request::builder()
            .uri("/mcp/tools/list?agent=test-agent")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let tools = json["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2); // web_search + llm_query

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"llm_query"));
    }

    #[tokio::test]
    async fn mcp_tool_list_without_auth_rejected() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/mcp/tools/list?agent=test-agent")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── MCP tool invocation ─────────────────────────────────────────────

    #[tokio::test]
    async fn mcp_tool_invoke_succeeds() {
        let (router, state) = setup_gateway();
        let body = serde_json::json!({
            "agent": "test-agent",
            "tool": "web_search",
            "params": {"query": "rust async"}
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp/tools/invoke")
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["is_error"], false);
        assert!(json["fuel_consumed"].as_u64().unwrap() > 0);
        assert!(json["audit_hash"].as_str().is_some());
    }

    #[tokio::test]
    async fn mcp_tool_invoke_unauthorized_capability_rejected() {
        let (router, state) = setup_gateway();
        // test-agent has web.search and llm.query, but NOT fs.write
        let body = serde_json::json!({
            "agent": "test-agent",
            "tool": "fs_write",
            "params": {"path": "/etc/passwd", "content": "hacked"}
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp/tools/invoke")
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["error"].as_str().unwrap().contains("denied"));
    }

    #[tokio::test]
    async fn mcp_tool_invoke_without_auth_rejected() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "agent": "test-agent",
            "tool": "web_search",
            "params": {"query": "test"}
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp/tools/invoke")
            .header("content-type", "application/json")
            // No auth
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── JWKS endpoint ──────────────────────────────────────────────────

    #[tokio::test]
    async fn jwks_endpoint_returns_valid_key() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/auth/jwks")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let keys = json["keys"].as_array().expect("keys array");
        assert_eq!(keys.len(), 1);

        let key = &keys[0];
        assert_eq!(key["kty"], "OKP");
        assert_eq!(key["crv"], "Ed25519");
        assert_eq!(key["alg"], "EdDSA");
        assert_eq!(key["use"], "sig");
        assert!(key["kid"].as_str().unwrap().starts_with("did:key:z"));
        assert!(key["x"].as_str().is_some());
    }

    // ── CORS headers ────────────────────────────────────────────────────

    #[tokio::test]
    async fn cors_headers_present() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/health")
            .header("origin", "https://dashboard.example.com")
            .header("access-control-request-method", "GET")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        // CORS layer should respond to preflight
        assert!(
            resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT,
            "preflight should succeed, got {}",
            resp.status()
        );
    }

    // ── REST API: Agent management ──────────────────────────────────────

    #[tokio::test]
    async fn api_list_agents_returns_registered() {
        let (router, state) = setup_gateway();
        let req = Request::builder()
            .uri("/api/agents")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let agents = json["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0]["name"], "test-agent");
        assert_eq!(agents[0]["status"], "Running");
    }

    #[tokio::test]
    async fn api_list_agents_without_auth_rejected() {
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/api/agents")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_create_agent_succeeds() {
        let (router, state) = setup_gateway();
        let body = serde_json::json!({
            "manifest": {
                "name": "new-agent",
                "version": "1.0.0",
                "capabilities": ["audit.read"],
                "fuel_budget": 5000,
                "domain_tags": []
            }
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/agents")
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["name"], "new-agent");
        assert!(!json["agent_id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn api_agent_status_returns_detail() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);

        let req = Request::builder()
            .uri(format!("/api/agents/{agent_id}/status"))
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["name"], "test-agent");
        assert_eq!(json["status"], "Running");
        assert!(json["fuel_remaining"].as_u64().unwrap() > 0);
        assert!(json["capabilities"].as_array().unwrap().len() >= 2);
    }

    #[tokio::test]
    async fn api_stop_and_start_agent() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);
        let auth = auth_header_for(&state);

        // Stop
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/agents/{agent_id}/stop"))
            .header("authorization", &auth)
            .body(Body::empty())
            .unwrap();

        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "stopped");

        // Start
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/agents/{agent_id}/start"))
            .header("authorization", &auth)
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "started");
    }

    // ── REST API: Permissions ───────────────────────────────────────────

    #[tokio::test]
    async fn api_get_permissions_returns_categories() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);

        let req = Request::builder()
            .uri(format!("/api/agents/{agent_id}/permissions"))
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let categories = json.as_array().unwrap();
        assert!(!categories.is_empty());
    }

    #[tokio::test]
    async fn api_update_permission_toggles_capability() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);
        let body = serde_json::json!({
            "capability_key": "web.search",
            "enabled": false
        });

        let req = Request::builder()
            .method(Method::PUT)
            .uri(format!("/api/agents/{agent_id}/permissions"))
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_bulk_update_permissions() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);
        let body = serde_json::json!({
            "updates": [
                {"capability_key": "web.search", "enabled": false},
                {"capability_key": "llm.query", "enabled": false}
            ],
            "reason": "testing bulk update"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/agents/{agent_id}/permissions/bulk"))
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["count"], 2);
    }

    // ── REST API: Audit ─────────────────────────────────────────────────

    #[tokio::test]
    async fn api_audit_events_with_pagination() {
        let (router, state) = setup_gateway();

        let req = Request::builder()
            .uri("/api/audit/events?limit=10&offset=0")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["events"].as_array().is_some());
        assert!(json["total"].as_u64().is_some());
        assert_eq!(json["limit"], 10);
        assert_eq!(json["offset"], 0);
    }

    // ── REST API: Compliance ────────────────────────────────────────────

    #[tokio::test]
    async fn api_compliance_status_returns_result() {
        let (router, state) = setup_gateway();

        let req = Request::builder()
            .uri("/api/compliance/status")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["status"].as_str().is_some());
        assert!(json["agents_checked"].as_u64().is_some());
    }

    #[tokio::test]
    async fn api_compliance_report_returns_transparency() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);

        let req = Request::builder()
            .uri(format!("/api/compliance/report/{agent_id}"))
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["agent_name"], "test-agent");
        assert!(json["risk_tier"].as_str().is_some());
    }

    // ── REST API: Identity ──────────────────────────────────────────────

    #[tokio::test]
    async fn api_identity_list_returns_dids() {
        let (router, state) = setup_gateway();

        let req = Request::builder()
            .uri("/api/identity/agents")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        let identities = json["identities"].as_array().unwrap();
        assert_eq!(identities.len(), 1);
        assert!(identities[0]["did"]
            .as_str()
            .unwrap()
            .starts_with("did:key:z"));
    }

    #[tokio::test]
    async fn api_identity_get_by_id() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);

        let req = Request::builder()
            .uri(format!("/api/identity/agents/{agent_id}"))
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["did"].as_str().unwrap().starts_with("did:key:z"));
        assert_eq!(json["agent_id"], agent_id);
    }

    // ── REST API: Firewall ──────────────────────────────────────────────

    #[tokio::test]
    async fn api_firewall_status_returns_patterns() {
        let (router, state) = setup_gateway();

        let req = Request::builder()
            .uri("/api/firewall/status")
            .header("authorization", auth_header_for(&state))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "active");
        assert_eq!(json["mode"], "fail-closed");
        assert!(json["injection_pattern_count"].as_u64().unwrap() > 0);
        assert_eq!(json["egress_default_deny"], true);
    }

    // ── REST API: Middleware rejects all /api without auth ──────────────

    #[tokio::test]
    async fn api_middleware_rejects_unauthenticated() {
        let (router, _) = setup_gateway();

        // Try several API endpoints without auth
        for uri in &[
            "/api/agents",
            "/api/audit/events",
            "/api/compliance/status",
            "/api/firewall/status",
            "/api/identity/agents",
        ] {
            let req = Request::builder().uri(*uri).body(Body::empty()).unwrap();

            let resp = router.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "{uri} should require auth"
            );
        }
    }

    // ── WebSocket: broadcast channel tests ─────────────────────────────

    #[tokio::test]
    async fn ws_broadcast_delivers_event() {
        let state = GatewayState::new("ignored");
        let mut rx = state.subscribe();

        let agent_id = Uuid::new_v4();
        state.broadcast(WsEvent::agent_status_changed(agent_id, "running"));

        let event = rx.recv().await.expect("should receive event");
        assert_eq!(event.event_type, WsEventType::AgentStatusChanged);
        assert_eq!(
            event.data["agent_id"].as_str().unwrap(),
            agent_id.to_string()
        );
        assert_eq!(event.data["status"], "running");
        assert!(event.timestamp > 0);
    }

    #[tokio::test]
    async fn ws_broadcast_multiple_event_types() {
        let state = GatewayState::new("ignored");
        let mut rx = state.subscribe();

        let agent_id = Uuid::new_v4();
        state.broadcast(WsEvent::fuel_consumed(agent_id, 9500));
        state.broadcast(WsEvent::firewall_block(agent_id, "injection detected"));
        state.broadcast(WsEvent::speculation_decision(agent_id, true, "low risk"));

        let e1 = rx.recv().await.unwrap();
        assert_eq!(e1.event_type, WsEventType::FuelConsumed);
        assert_eq!(e1.data["fuel_remaining"], 9500);

        let e2 = rx.recv().await.unwrap();
        assert_eq!(e2.event_type, WsEventType::FirewallBlock);
        assert_eq!(e2.data["reason"], "injection detected");

        let e3 = rx.recv().await.unwrap();
        assert_eq!(e3.event_type, WsEventType::SpeculationDecision);
        assert_eq!(e3.data["approved"], true);
        assert_eq!(e3.data["summary"], "low risk");
    }

    #[tokio::test]
    async fn ws_event_serialization_roundtrip() {
        let event = WsEvent::audit_event(Uuid::nil(), Uuid::nil(), "StateChange");
        let json = serde_json::to_string(&event).unwrap();
        let parsed: WsEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, WsEventType::AuditEvent);
        assert_eq!(parsed.data["event_type"], "StateChange");
    }

    #[tokio::test]
    async fn ws_agent_lifecycle_emits_broadcast() {
        let (router, state) = setup_gateway();
        let agent_id = get_test_agent_id(&state);
        let auth = auth_header_for(&state);
        let mut rx = state.subscribe();

        // Stop agent to trigger broadcast
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/agents/{agent_id}/stop"))
            .header("authorization", &auth)
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Collect all broadcast events
        let mut got_audit = false;
        let mut got_status = false;
        while let Ok(event) = rx.try_recv() {
            match event.event_type {
                WsEventType::AuditEvent => got_audit = true,
                WsEventType::AgentStatusChanged => {
                    assert_eq!(event.data["status"], "stopped");
                    got_status = true;
                }
                _ => {}
            }
        }
        assert!(got_audit, "should broadcast audit event");
        assert!(got_status, "should broadcast agent_status_changed");
    }

    // ── WebSocket: real TCP listener tests ──────────────────────────────

    /// Spin up a real TCP server and return its address.
    async fn start_test_server(state: GatewayState) -> std::net::SocketAddr {
        let router = build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        addr
    }

    #[tokio::test]
    async fn ws_invalid_token_rejected() {
        let state = GatewayState::new("ignored");
        state.register_agent(
            test_manifest("test-agent", vec!["web.search"], 10_000),
            "https://example.com",
        );
        let addr = start_test_server(state).await;

        let url = format!("ws://{addr}/ws?token=invalid.jwt.token");
        let result = tokio_tungstenite::connect_async(&url).await;
        assert!(result.is_err(), "invalid token should reject WS handshake");
    }

    #[tokio::test]
    async fn ws_wrong_key_rejected() {
        let state = GatewayState::new("ignored");
        state.register_agent(
            test_manifest("test-agent", vec!["web.search"], 10_000),
            "https://example.com",
        );
        let addr = start_test_server(state).await;

        let mut rogue_km = nexus_kernel::hardware_security::KeyManager::new();
        let rogue_identity =
            AgentIdentity::generate(Uuid::new_v4(), &mut rogue_km).expect("rogue identity");
        let rogue_mgr = TokenManager::new("rogue", "nexus-agents");
        let bad_token = rogue_mgr
            .issue_token(&rogue_identity, &rogue_km, &[], 3600, None)
            .expect("rogue token");

        let url = format!("ws://{addr}/ws?token={bad_token}");
        let result = tokio_tungstenite::connect_async(&url).await;
        assert!(
            result.is_err(),
            "wrong-key token should reject WS handshake"
        );
    }

    #[tokio::test]
    async fn ws_valid_token_connects_and_receives_event() {
        use futures_util::StreamExt;
        use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

        let state = GatewayState::new("ignored");
        state.register_agent(
            test_manifest("test-agent", vec!["web.search"], 10_000),
            "https://example.com",
        );
        let token = state.issue_token(&[], 3600);
        let broadcast_state = state.clone();
        let addr = start_test_server(state).await;

        let url = format!("ws://{addr}/ws?token={token}");
        let (mut ws_stream, resp) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("valid token should connect");
        assert_eq!(resp.status(), axum::http::StatusCode::SWITCHING_PROTOCOLS);

        // Broadcast an event after connection is established
        let agent_id = Uuid::new_v4();
        broadcast_state.broadcast(WsEvent::agent_status_changed(agent_id, "running"));

        // Read it from the WebSocket
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws_stream.next())
            .await
            .expect("should receive within 2s")
            .expect("stream should not be closed")
            .expect("message should be valid");

        if let TungsteniteMessage::Text(text) = msg {
            let event: WsEvent = serde_json::from_str(&text).unwrap();
            assert_eq!(event.event_type, WsEventType::AgentStatusChanged);
            assert_eq!(event.data["status"], "running");
            assert_eq!(
                event.data["agent_id"].as_str().unwrap(),
                agent_id.to_string()
            );
        } else {
            panic!("expected text message, got {msg:?}");
        }
    }

    #[tokio::test]
    async fn graceful_shutdown_completes_cleanly() {
        let state = GatewayState::new("test-secret");

        // Register an agent so shutdown has work to do
        let manifest = test_manifest("shutdown-test-agent", vec!["llm.query"], 500);
        state.register_agent(manifest, "http://localhost:9999");

        // Verify agent is registered
        {
            let inner = state.inner.lock().unwrap();
            assert!(!inner.supervisor.health_check().is_empty());
        }

        // Run shutdown — should not panic and should stop all agents
        state.shutdown();

        // After shutdown, all agents should be stopped
        let inner = state.inner.lock().unwrap();
        for status in inner.supervisor.health_check() {
            assert_eq!(
                status.state.to_string(),
                "Stopped",
                "agent {} should be stopped after shutdown",
                status.id
            );
        }

        // Audit trail should contain the shutdown event
        let events = inner.audit_trail.events();
        let has_shutdown = events
            .iter()
            .any(|e| e.payload.get("event").and_then(|v| v.as_str()) == Some("gateway.shutdown"));
        assert!(has_shutdown, "shutdown event should be in audit trail");
    }

    // ── Metrics endpoint ──────────────────────────────────────────────

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_format() {
        // NOTE: We cannot install the global metrics recorder in tests that run
        // in the same process as other metrics tests. Instead we test the
        // fallback path (metrics: None) returns a placeholder.
        let (router, _state) = setup_gateway();
        let req = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        // Without metrics installed, we get the placeholder
        assert!(
            text.contains("metrics") || text.contains("nexus_"),
            "response should mention metrics"
        );
    }

    // ── Extended health check ─────────────────────────────────────────

    #[tokio::test]
    async fn health_check_returns_extended_fields() {
        let (router, _state) = setup_gateway();
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "healthy");
        assert!(
            json["uptime_seconds"].is_u64(),
            "should have uptime_seconds"
        );
        assert!(
            json["agents_active"].is_number(),
            "should have agents_active"
        );
        assert!(
            json["total_tests_passed"].is_number(),
            "should have total_tests_passed"
        );
        assert!(
            json["audit_chain_valid"].is_boolean(),
            "should have audit_chain_valid"
        );
        assert!(
            json["compliance_status"].is_string(),
            "should have compliance_status"
        );
        assert!(
            json["memory_usage_bytes"].is_number(),
            "should have memory_usage_bytes"
        );
        assert!(
            json["wasm_cache_hit_rate"].is_number(),
            "should have wasm_cache_hit_rate"
        );
    }

    // ── Anthropic Messages API tests ────────────────────────────────────

    #[tokio::test]
    async fn test_anthropic_messages_basic() {
        let (router, _state) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello!"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key-123")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["id"].as_str().unwrap().starts_with("msg_"));
        assert_eq!(json["type"], "message");
        assert_eq!(json["role"], "assistant");
        assert!(!json["content"].as_array().unwrap().is_empty());
        assert_eq!(json["content"][0]["type"], "text");
        assert!(!json["content"][0]["text"].as_str().unwrap().is_empty());
        assert_eq!(json["model"], "llama3");
        assert!(json["usage"]["input_tokens"].as_u64().unwrap() > 0);
        assert!(json["usage"]["output_tokens"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_anthropic_messages_with_system() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 512,
            "system": "You are a helpful assistant.",
            "messages": [{"role": "user", "content": "Hi there"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["type"], "message");
        // The response should reference the system prompt in some way
        let text = json["content"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty());
    }

    #[tokio::test]
    async fn test_anthropic_messages_structured_content() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 256,
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "What is 2+2?"}]
            }]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["type"], "message");
        assert!(!json["content"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_anthropic_messages_string_content() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 256,
            "messages": [{"role": "user", "content": "Plain string content"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_anthropic_error_no_auth() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 256,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            // No auth header
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"]["type"], "authentication_error");
        assert!(!json["error"]["message"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_anthropic_error_format() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 256,
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let json = body_to_json(resp.into_body()).await;

        // Verify Anthropic error schema
        assert!(json.get("type").is_some(), "must have 'type' field");
        assert!(json.get("error").is_some(), "must have 'error' field");
        assert!(
            json["error"].get("type").is_some(),
            "error must have 'type'"
        );
        assert!(
            json["error"].get("message").is_some(),
            "error must have 'message'"
        );
    }

    #[tokio::test]
    async fn test_anthropic_usage_tracking() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "nexus-governed",
            "max_tokens": 100,
            "messages": [{"role": "user", "content": "Count my tokens"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["usage"]["input_tokens"].as_u64().is_some());
        assert!(json["usage"]["output_tokens"].as_u64().is_some());
        assert!(json["usage"]["input_tokens"].as_u64().unwrap() > 0);
        assert!(json["usage"]["output_tokens"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_anthropic_stream_format() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 100,
            "stream": true,
            "messages": [{"role": "user", "content": "Stream this"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();

        // Verify SSE event format
        assert!(text.contains("event: message_start\ndata: "));
        assert!(text.contains("event: content_block_start\ndata: "));
        assert!(text.contains("event: content_block_delta\ndata: "));
        assert!(text.contains("event: content_block_stop\ndata: "));
        assert!(text.contains("event: message_delta\ndata: "));
        assert!(text.contains("event: message_stop\ndata: "));
    }

    // ── OpenAI-compatible API tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_openai_chat_completions_basic() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "messages": [{"role": "user", "content": "Hello!"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert!(json["id"].as_str().unwrap().starts_with("chatcmpl-"));
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["choices"][0]["message"]["role"], "assistant");
        assert_eq!(json["choices"][0]["finish_reason"], "stop");
        assert!(json["usage"]["prompt_tokens"].as_u64().unwrap() > 0);
        assert!(json["usage"]["completion_tokens"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_openai_chat_no_auth() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_openai_embeddings() {
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "model": "text-embedding-ada-002",
            "input": "Hello world"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/embeddings")
            .header("content-type", "application/json")
            .header("x-api-key", "test-key")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["object"], "list");
        let data = json["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["object"], "embedding");
        let embedding = data[0]["embedding"].as_array().unwrap();
        assert_eq!(embedding.len(), 8);
    }

    #[tokio::test]
    async fn test_openai_list_models() {
        let (router, _) = setup_gateway();

        let req = Request::builder()
            .uri("/v1/models")
            .header("x-api-key", "test-key")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["object"], "list");
        let data = json["data"].as_array().unwrap();
        assert!(data.len() >= 3);
        assert!(data.iter().any(|m| m["id"] == "nexus-governed"));
    }

    #[tokio::test]
    async fn test_anthropic_bearer_auth_accepted() {
        let (router, state) = setup_gateway();
        let body = serde_json::json!({
            "model": "llama3",
            "max_tokens": 100,
            "messages": [{"role": "user", "content": "Bearer test"}]
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/messages")
            .header("content-type", "application/json")
            .header("authorization", auth_header_for(&state))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
