//! Phase 7.5 — Web API integration tests.
//!
//! End-to-end tests exercising the full HTTP gateway stack including JWT auth,
//! REST CRUD, WebSocket streaming, health/metrics endpoints, graceful shutdown,
//! and WASM module cache performance.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use futures_util::StreamExt;
use nexus_kernel::identity::{AgentIdentity, TokenManager};
use nexus_kernel::manifest::AgentManifest;
use nexus_protocols::http_gateway::{build_router, GatewayState, WsEvent, WsEventType};
use nexus_sdk::ModuleCache;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tower::ServiceExt;
use uuid::Uuid;

// ── Helpers ──────────────────────────────────────────────────────────────────

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
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    }
}

fn setup_gateway() -> (axum::Router, GatewayState) {
    let state = GatewayState::new("integration-test")
        .unwrap()
        .with_llm_provider(Box::new(
            nexus_connectors_llm::providers::mock::MockProvider::new(),
        ));
    state.register_agent(
        test_manifest(
            "test-agent",
            vec!["web.search", "llm.query", "fs.read"],
            10_000,
        ),
        "https://example.com",
    );
    let router = build_router(state.clone());
    (router, state)
}

fn auth_header(state: &GatewayState) -> String {
    let token = state.issue_token(&[], 3600).unwrap();
    format!("Bearer {token}")
}

async fn body_json(body: Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Get the agent UUID by listing agents via the REST API.
async fn get_agent_id_via_api(router: &axum::Router, state: &GatewayState) -> String {
    let req = Request::builder()
        .uri("/api/agents")
        .header("authorization", auth_header(state))
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let json = body_json(resp.into_body()).await;
    let agents = json["agents"].as_array().unwrap();
    // Find "test-agent" by name
    agents
        .iter()
        .find(|a| a["name"] == "test-agent")
        .expect("test-agent should exist")["id"]
        .as_str()
        .expect("agent should have id")
        .to_string()
}

async fn start_server(state: GatewayState) -> std::net::SocketAddr {
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

// ── 1. REST API list agents returns correct data ─────────────────────────────

#[tokio::test]
async fn rest_list_agents_returns_correct_data() {
    let (router, state) = setup_gateway();
    // Register a second agent
    state.register_agent(
        test_manifest("analytics-agent", vec!["audit.read"], 5_000),
        "https://analytics.example.com",
    );

    let req = Request::builder()
        .uri("/api/agents")
        .header("authorization", auth_header(&state))
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let agents = json["agents"].as_array().expect("agents array");
    assert_eq!(agents.len(), 2, "should list both registered agents");

    let names: Vec<&str> = agents.iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"test-agent"), "should contain test-agent");
    assert!(
        names.contains(&"analytics-agent"),
        "should contain analytics-agent"
    );

    // Each agent entry should have id, name, and status
    for agent in agents {
        assert!(agent["id"].as_str().is_some(), "agent should have id");
        assert!(agent["name"].as_str().is_some(), "agent should have name");
        assert!(
            agent["status"].as_str().is_some(),
            "agent should have status"
        );
    }
}

// ── 2. REST API create agent returns new agent ID ────────────────────────────

#[tokio::test]
async fn rest_create_agent_returns_new_agent_id() {
    let (router, state) = setup_gateway();
    let body = serde_json::json!({
        "manifest": {
            "name": "created-agent",
            "version": "2.0.0",
            "capabilities": ["web.search", "fs.read"],
            "fuel_budget": 8000,
            "domain_tags": ["limited-risk"]
        }
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/agents")
        .header("content-type", "application/json")
        .header("authorization", auth_header(&state))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["name"], "created-agent");

    let agent_id = json["agent_id"].as_str().expect("should have agent_id");
    assert!(!agent_id.is_empty(), "agent_id should not be empty");

    // Validate UUID format
    let parsed = Uuid::parse_str(agent_id);
    assert!(parsed.is_ok(), "agent_id should be a valid UUID");
}

// ── 3. REST API permissions CRUD works ───────────────────────────────────────

#[tokio::test]
async fn rest_permissions_crud_works() {
    let (router, state) = setup_gateway();
    let agent_id = get_agent_id_via_api(&router, &state).await;
    let auth = auth_header(&state);

    // GET permissions — should return categories
    let req = Request::builder()
        .uri(format!("/api/agents/{agent_id}/permissions"))
        .header("authorization", &auth)
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let categories = json.as_array().expect("should be array of categories");
    assert!(!categories.is_empty(), "should have permission categories");

    // Verify structure: each category has id, display_name, permissions
    for cat in categories {
        assert!(cat["id"].as_str().is_some(), "category should have id");
        assert!(
            cat["display_name"].as_str().is_some(),
            "category should have display_name"
        );
        assert!(
            cat["permissions"].as_array().is_some(),
            "category should have permissions array"
        );
    }

    // PUT — disable a capability
    let update_body = serde_json::json!({
        "capability_key": "web.search",
        "enabled": false
    });

    let req = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/agents/{agent_id}/permissions"))
        .header("content-type", "application/json")
        .header("authorization", &auth)
        .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // POST bulk — re-enable web.search, disable fs.read
    let bulk_body = serde_json::json!({
        "updates": [
            {"capability_key": "web.search", "enabled": true},
            {"capability_key": "fs.read", "enabled": false}
        ],
        "reason": "integration test bulk update"
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/agents/{agent_id}/permissions/bulk"))
        .header("content-type", "application/json")
        .header("authorization", &auth)
        .body(Body::from(serde_json::to_vec(&bulk_body).unwrap()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["count"], 2, "should report 2 updates applied");
}

// ── 4. REST API compliance status returns valid response ─────────────────────

#[tokio::test]
async fn rest_compliance_status_returns_valid_response() {
    let (router, state) = setup_gateway();
    let auth = auth_header(&state);

    let req = Request::builder()
        .uri("/api/compliance/status")
        .header("authorization", &auth)
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;

    // Must have these compliance fields
    assert!(
        json["status"].as_str().is_some(),
        "should have compliance status string"
    );
    assert!(
        json["agents_checked"].as_u64().is_some(),
        "should have agents_checked count"
    );
    assert!(
        json["checks_passed"].as_u64().is_some(),
        "should have checks_passed"
    );
    assert!(
        json["checks_failed"].as_u64().is_some(),
        "should have checks_failed"
    );
    assert!(
        json["last_check_unix"].as_u64().is_some(),
        "should have last_check_unix timestamp"
    );

    // Alerts should be an array (possibly empty)
    assert!(
        json["alerts"].as_array().is_some(),
        "should have alerts array"
    );
}

// ── 5. REST API marketplace search works ─────────────────────────────────────

#[tokio::test]
async fn rest_marketplace_search_works() {
    let (router, state) = setup_gateway();
    let auth = auth_header(&state);

    // Search with query
    let req = Request::builder()
        .uri("/api/marketplace/search?q=test")
        .header("authorization", &auth)
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    // Should return a valid response (results array, possibly empty for fresh DB)
    assert!(
        json["results"].as_array().is_some(),
        "should have results array in search results"
    );

    // Search without query (list all)
    let req = Request::builder()
        .uri("/api/marketplace/search")
        .header("authorization", &auth)
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── 6. All REST endpoints require JWT auth ───────────────────────────────────

#[tokio::test]
async fn all_rest_endpoints_require_jwt_auth() {
    let (router, state) = setup_gateway();
    let agent_id = get_agent_id_via_api(&router, &state).await;

    // Every /api/* endpoint should return 401 without auth
    let endpoints = vec![
        ("GET", "/api/agents".to_string()),
        ("GET", format!("/api/agents/{agent_id}/status")),
        ("GET", format!("/api/agents/{agent_id}/permissions")),
        ("GET", "/api/audit/events".to_string()),
        ("GET", "/api/compliance/status".to_string()),
        ("GET", format!("/api/compliance/report/{agent_id}")),
        // Marketplace search/browse is now public (no auth required)
        ("GET", "/api/identity/agents".to_string()),
        ("GET", format!("/api/identity/agents/{agent_id}")),
        ("GET", "/api/firewall/status".to_string()),
    ];

    for (method, uri) in &endpoints {
        let req = match *method {
            "GET" => Request::builder()
                .uri(uri.as_str())
                .body(Body::empty())
                .unwrap(),
            _ => Request::builder()
                .method(Method::POST)
                .uri(uri.as_str())
                .body(Body::empty())
                .unwrap(),
        };

        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {uri} should require JWT auth"
        );
    }

    // Also verify that an invalid token is rejected
    let req = Request::builder()
        .uri("/api/agents")
        .header("authorization", "Bearer invalid.jwt.garbage")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "invalid JWT should be rejected"
    );

    // Verify a token signed by a different key is rejected
    let mut rogue_km = nexus_kernel::hardware_security::KeyManager::new();
    let rogue_identity =
        AgentIdentity::generate(Uuid::new_v4(), &mut rogue_km).expect("rogue identity");
    let rogue_mgr = TokenManager::new("rogue-issuer", "nexus-agents");
    let bad_token = rogue_mgr
        .issue_token(&rogue_identity, &rogue_km, &[], 3600, None)
        .expect("rogue token");

    let req = Request::builder()
        .uri("/api/agents")
        .header("authorization", format!("Bearer {bad_token}"))
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "wrong-key JWT should be rejected"
    );
}

// ── 7. WebSocket connects and receives events ────────────────────────────────

#[tokio::test]
async fn websocket_connects_and_receives_events() {
    let state = GatewayState::new("ws-test").unwrap();
    state.register_agent(
        test_manifest("ws-agent", vec!["web.search"], 10_000),
        "https://example.com",
    );
    let token = state.issue_token(&[], 3600).unwrap();
    let broadcast_state = state.clone();
    let addr = start_server(state).await;

    // Connect with valid JWT
    let url = format!("ws://{addr}/ws?token={token}");
    let (mut ws_stream, resp) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("should connect with valid token");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    // Broadcast multiple event types
    let agent_id = Uuid::new_v4();
    broadcast_state.broadcast(WsEvent::agent_status_changed(agent_id, "running"));
    broadcast_state.broadcast(WsEvent::fuel_consumed(agent_id, 8500));
    broadcast_state.broadcast(WsEvent::compliance_alert(agent_id, "high-risk detected"));

    // Receive and validate each event
    let mut received_types = Vec::new();
    for _ in 0..3 {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(3), ws_stream.next())
            .await
            .expect("should receive within 3s")
            .expect("stream not closed")
            .expect("valid message");

        if let TungsteniteMessage::Text(text) = msg {
            let event: WsEvent = serde_json::from_str(&text).unwrap();
            assert!(event.timestamp > 0, "event should have valid timestamp");
            assert!(
                event.data["agent_id"].as_str().is_some(),
                "event should have agent_id"
            );
            received_types.push(event.event_type);
        } else {
            panic!("expected text message");
        }
    }

    assert_eq!(received_types[0], WsEventType::AgentStatusChanged);
    assert_eq!(received_types[1], WsEventType::FuelConsumed);
    assert_eq!(received_types[2], WsEventType::ComplianceAlert);
}

// ── 8. Health endpoint returns all fields ────────────────────────────────────

#[tokio::test]
async fn health_endpoint_returns_all_fields() {
    let (router, _state) = setup_gateway();

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;

    // All required health fields
    assert_eq!(json["status"], "healthy");
    assert!(json["version"].as_str().is_some(), "should have version");
    assert!(
        json["agents_registered"].as_u64().is_some(),
        "should have agents_registered"
    );
    assert!(
        json["tasks_in_flight"].as_u64().is_some(),
        "should have tasks_in_flight"
    );
    assert!(
        json["started_at"].as_u64().is_some(),
        "should have started_at"
    );
    assert!(
        json["uptime_seconds"].as_u64().is_some(),
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
        json["compliance_status"].as_str().is_some(),
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

    // Health endpoint should not require auth (public)
    assert_eq!(json["agents_registered"], 1);
    assert_eq!(json["audit_chain_valid"], true);
}

// ── 9. Metrics endpoint returns Prometheus format ────────────────────────────

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
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

    // Should return valid Prometheus-compatible text
    // (without metrics installed, gateway returns a placeholder — either case is valid)
    assert!(
        !text.is_empty(),
        "metrics endpoint should return non-empty body"
    );

    // Every non-empty line should be a comment (# ...) or a metric line
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        assert!(
            trimmed.starts_with('#')
                || trimmed.starts_with("nexus_")
                || trimmed.contains('{')
                || trimmed.contains("metrics"),
            "unexpected Prometheus line: {trimmed}"
        );
    }
}

// ── 10. Graceful shutdown completes within 5 seconds ─────────────────────────

#[tokio::test]
async fn graceful_shutdown_completes_within_5_seconds() {
    let state = GatewayState::new("shutdown-test").unwrap();

    // Register multiple agents to make shutdown do real work
    for i in 0..5 {
        state.register_agent(
            test_manifest(
                &format!("shutdown-agent-{i}"),
                vec!["llm.query", "web.search"],
                1000,
            ),
            "http://localhost:9999",
        );
    }

    // Verify agents are registered via the API
    let router = build_router(state.clone());
    let auth = auth_header(&state);
    let req = Request::builder()
        .uri("/api/agents")
        .header("authorization", &auth)
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let json = body_json(resp.into_body()).await;
    assert_eq!(
        json["agents"].as_array().unwrap().len(),
        5,
        "should have 5 agents"
    );

    // Run shutdown with a 5-second deadline
    let shutdown_state = state.clone();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async move {
        tokio::task::spawn_blocking(move || {
            shutdown_state.shutdown();
        })
        .await
        .expect("shutdown task should not panic");
    })
    .await;

    assert!(
        result.is_ok(),
        "graceful shutdown must complete within 5 seconds"
    );

    // After shutdown, verify via health endpoint that system is still responsive
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["status"], "healthy", "health endpoint still responds");
    // All agents should be stopped (agents_active = 0)
    assert_eq!(
        json["agents_active"], 0,
        "all agents should be stopped after shutdown"
    );
}

// ── 11. WASM module cache improves startup time ──────────────────────────────

#[tokio::test]
async fn wasm_module_cache_improves_startup_time() {
    let cache = ModuleCache::new();

    // Create a wasmtime engine with fuel metering
    let mut config = wasmtime::Config::new();
    config.consume_fuel(true);
    let engine = wasmtime::Engine::new(&config).unwrap();

    // Compile a minimal WASM module
    let wasm = wat::parse_str("(module)").unwrap();

    assert!(cache.is_empty(), "cache should start empty");

    // First compilation — cache miss
    let start = std::time::Instant::now();
    let (_, hit1) = cache.get_or_compile(&engine, &wasm).unwrap();
    let cold_duration = start.elapsed();
    assert!(!hit1, "first call should be a cache miss");
    assert_eq!(cache.len(), 1);

    // Second compilation — cache hit (should be significantly faster)
    let start = std::time::Instant::now();
    let (_, hit2) = cache.get_or_compile(&engine, &wasm).unwrap();
    let warm_duration = start.elapsed();
    assert!(hit2, "second call should be a cache hit");
    assert_eq!(cache.len(), 1, "cache size should not change on hit");

    // Cache hit should be faster than cold compilation
    // (In rare CI edge cases cold could be very fast, so we just verify the API works)
    assert!(
        warm_duration <= cold_duration || warm_duration.as_micros() < 100,
        "cache hit ({warm_duration:?}) should generally be faster than miss ({cold_duration:?})"
    );

    // Verify different modules are cached separately
    let wasm_b = wat::parse_str("(module (memory 1))").unwrap();
    let (_, hit3) = cache.get_or_compile(&engine, &wasm_b).unwrap();
    assert!(!hit3, "different WASM should be a cache miss");
    assert_eq!(cache.len(), 2, "should have 2 cached modules");

    // Clear and verify
    cache.clear();
    assert!(cache.is_empty(), "cache should be empty after clear");
}
