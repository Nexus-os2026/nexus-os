//! Axum-based HTTP gateway — async edge layer bridging HTTP to the sync kernel.
//!
//! Routes:
//! - `POST /a2a`              — A2A task submission
//! - `GET  /a2a/agent-card`   — Agent Card discovery
//! - `GET  /a2a/tasks/:id`    — Task status lookup
//! - `POST /mcp/tools/invoke` — MCP tool invocation
//! - `GET  /mcp/tools/list`   — MCP tool discovery
//! - `GET  /health`           — Health check
//!
//! Every route goes through governance. JWT auth required on mutating endpoints.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::protocols::a2a::{
    A2ATask, AgentCard, GovernanceContext, MessagePart, MessageRole, TaskMessage, TaskPayload,
    A2A_PROTOCOL_VERSION,
};
use nexus_kernel::protocols::mcp::McpServer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ── JWT auth ────────────────────────────────────────────────────────────────

/// JWT validation config.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// HMAC secret for HS256 validation.
    pub secret: String,
}

/// Claims embedded in a JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (caller identity).
    pub sub: String,
    /// Expiration (unix timestamp).
    pub exp: u64,
    /// Issued at (unix timestamp).
    #[serde(default)]
    pub iat: u64,
}

/// Validate a JWT bearer token from the Authorization header.
fn validate_jwt(headers: &HeaderMap, config: &JwtConfig) -> Result<JwtClaims, AuthError> {
    let header_value = headers
        .get("authorization")
        .ok_or(AuthError::MissingToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken("non-ascii header".into()))?;

    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidToken("expected Bearer scheme".into()))?;

    let key = jsonwebtoken::DecodingKey::from_secret(config.secret.as_bytes());
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);

    let token_data = jsonwebtoken::decode::<JwtClaims>(token, &key, &validation)
        .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

    Ok(token_data.claims)
}

/// Create a signed JWT for testing.
pub fn create_test_jwt(sub: &str, secret: &str, exp_secs_from_now: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = JwtClaims {
        sub: sub.to_string(),
        exp: now + exp_secs_from_now,
        iat: now,
    };

    let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key).unwrap()
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
    /// JWT configuration.
    jwt_config: JwtConfig,
    /// Server start time.
    started_at: u64,
}

impl GatewayState {
    /// Create a new gateway state with JWT config.
    pub fn new(jwt_secret: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            inner: Arc::new(Mutex::new(GatewayInner {
                agent_cards: HashMap::new(),
                mcp_server: McpServer::new(),
                tasks: HashMap::new(),
                agent_ids: HashMap::new(),
                jwt_config: JwtConfig {
                    secret: jwt_secret.into(),
                },
                started_at: now,
            })),
        }
    }

    /// Register an agent with the gateway.
    pub fn register_agent(&self, manifest: AgentManifest, base_url: &str) {
        let mut inner = self.inner.lock().expect("lock poisoned");
        let agent_id = Uuid::new_v4();
        let card = AgentCard::from_manifest(&manifest, base_url);
        let name = manifest.name.clone();
        inner.mcp_server.register_agent(agent_id, manifest);
        inner.agent_cards.insert(name.clone(), card);
        inner.agent_ids.insert(name, agent_id);
    }
}

// ── Router construction ─────────────────────────────────────────────────────

/// Build the axum Router with all protocol routes and CORS middleware.
pub fn build_router(state: GatewayState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // A2A routes
        .route("/a2a", post(a2a_task_submit))
        .route("/a2a/agent-card", get(a2a_agent_card))
        .route("/a2a/tasks/{id}", get(a2a_task_status))
        // MCP routes
        .route("/mcp/tools/invoke", post(mcp_tool_invoke))
        .route("/mcp/tools/list", get(mcp_tool_list))
        // Health
        .route("/health", get(health_check))
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

// ── Route handlers ──────────────────────────────────────────────────────────

/// `GET /health` — public, no auth required.
async fn health_check(State(state): State<GatewayState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let inner = state.inner.lock().expect("lock poisoned");
        serde_json::json!({
            "status": "healthy",
            "version": A2A_PROTOCOL_VERSION,
            "agents_registered": inner.agent_cards.len(),
            "tasks_in_flight": inner.tasks.len(),
            "started_at": inner.started_at,
        })
    })
    .await
    .expect("spawn_blocking panicked");

    Json(result)
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

/// `POST /a2a` — submit a task. Requires JWT auth.
async fn a2a_task_submit(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(req): Json<TaskSubmitRequest>,
) -> impl IntoResponse {
    // Validate JWT outside spawn_blocking (headers aren't Send)
    let jwt_secret = {
        let inner = state.inner.lock().expect("lock poisoned");
        inner.jwt_config.clone()
    };
    let claims = match validate_jwt(&headers, &jwt_secret) {
        Ok(c) => c,
        Err(e) => return Err(error_json(e.status_code(), e.message())),
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
    let jwt_secret = {
        let inner = state.inner.lock().expect("lock poisoned");
        inner.jwt_config.clone()
    };
    if let Err(e) = validate_jwt(&headers, &jwt_secret) {
        return Err(error_json(e.status_code(), e.message()));
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
    let jwt_secret = {
        let inner = state.inner.lock().expect("lock poisoned");
        inner.jwt_config.clone()
    };
    if let Err(e) = validate_jwt(&headers, &jwt_secret) {
        return Err(error_json(e.status_code(), e.message()));
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
    let jwt_secret = {
        let inner = state.inner.lock().expect("lock poisoned");
        inner.jwt_config.clone()
    };
    if let Err(e) = validate_jwt(&headers, &jwt_secret) {
        return Err(error_json(e.status_code(), e.message()));
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

        // Route through governed MCP server — capability check + fuel + audit
        match inner
            .mcp_server
            .invoke_tool(agent_id, &req.tool, req.params)
        {
            Ok(result) => Ok(serde_json::to_value(result).unwrap()),
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use nexus_kernel::protocols::a2a::TaskStatus;
    use tower::ServiceExt;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-validation";

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
        }
    }

    fn setup_gateway() -> (Router, GatewayState) {
        let state = GatewayState::new(TEST_SECRET);
        state.register_agent(
            test_manifest("test-agent", vec!["web.search", "llm.query"], 10_000),
            "https://example.com",
        );
        let router = build_router(state.clone());
        (router, state)
    }

    fn auth_header() -> String {
        let token = create_test_jwt("test-user", TEST_SECRET, 3600);
        format!("Bearer {token}")
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
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
        let body = serde_json::json!({
            "agent": "test-agent",
            "message": "Hello, agent!"
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/a2a")
            .header("content-type", "application/json")
            .header("authorization", auth_header())
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "submitted");
        assert_eq!(json["agent"], "test-agent");
        assert_eq!(json["sender"], "test-user");

        let task_id = json["task_id"].as_str().unwrap();
        assert!(!task_id.is_empty());

        // Verify task is stored
        let inner = state.inner.lock().unwrap();
        let task = inner.tasks.get(task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Submitted);
        assert_eq!(task.sender, "test-user");
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
    async fn task_submission_wrong_secret_rejected() {
        let (router, _) = setup_gateway();
        let bad_token = create_test_jwt("attacker", "wrong-secret", 3600);
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
        let state = GatewayState::new(TEST_SECRET);
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

        let router = build_router(state);
        let req = Request::builder()
            .uri(format!("/a2a/tasks/{task_id}"))
            .header("authorization", auth_header())
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
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/a2a/tasks/nonexistent-id")
            .header("authorization", auth_header())
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
        let (router, _) = setup_gateway();
        let req = Request::builder()
            .uri("/mcp/tools/list?agent=test-agent")
            .header("authorization", auth_header())
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
        let (router, _) = setup_gateway();
        let body = serde_json::json!({
            "agent": "test-agent",
            "tool": "web_search",
            "params": {"query": "rust async"}
        });

        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp/tools/invoke")
            .header("content-type", "application/json")
            .header("authorization", auth_header())
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
        let (router, _) = setup_gateway();
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
            .header("authorization", auth_header())
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
}
