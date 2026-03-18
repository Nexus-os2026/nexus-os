# PROMPT: Server Mode + Docker + Helm + HA/DR for Nexus OS

## Context
Nexus OS needs a headless server mode for enterprise deployment via Docker and Kubernetes, with horizontal scaling and high availability.

## Objective
1. Create a `nexus-os-server` binary (headless, no Tauri UI)
2. Create Dockerfile and docker-compose.yml
3. Create Helm chart for Kubernetes
4. Implement basic horizontal scaling via shared state
5. Implement health checks and graceful shutdown

## Part 1: Server Mode Binary

### Step 1: Create server entry point

Create `src/bin/nexus-os-server.rs`:

```rust
use axum::{Router, routing::get, routing::post};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load configuration from config.toml / env vars
    // 2. Initialize nexus-kernel
    // 3. Initialize nexus-auth (OIDC)
    // 4. Initialize nexus-telemetry (OpenTelemetry)
    // 5. Build Axum router with all REST endpoints
    // 6. Start HTTP server
    // 7. Register graceful shutdown handler (SIGTERM, SIGINT)
    
    let app = Router::new()
        // Health
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        
        // Agent API
        .route("/api/v1/agents", get(list_agents).post(deploy_agent))
        .route("/api/v1/agents/:did/execute", post(execute_agent))
        .route("/api/v1/agents/:did/stop", post(stop_agent))
        .route("/api/v1/agents/:did/status", get(agent_status))
        
        // Governance API
        .route("/api/v1/hitl/pending", get(hitl_pending))
        .route("/api/v1/hitl/:id/approve", post(hitl_approve))
        .route("/api/v1/hitl/:id/deny", post(hitl_deny))
        
        // Audit API
        .route("/api/v1/audit", get(audit_query))
        .route("/api/v1/audit/verify", post(audit_verify))
        .route("/api/v1/audit/export", get(audit_export))
        
        // Workspace API
        .route("/api/v1/workspaces", get(list_workspaces).post(create_workspace))
        
        // Admin API
        .route("/api/v1/admin/users", get(admin_users))
        .route("/api/v1/admin/fleet", get(admin_fleet))
        .route("/api/v1/admin/policies", get(admin_policies).put(update_policies))
        
        // Metrics
        .route("/metrics", get(prometheus_metrics))
        
        // Auth middleware applied to all /api routes
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("Graceful shutdown initiated");
    // 1. Stop accepting new requests
    // 2. Wait for in-flight agent executions (with timeout)
    // 3. Flush audit trail
    // 4. Flush telemetry
    // 5. Close database connections
}
```

### Step 2: Dependencies

Add to Cargo.toml:
```toml
[dependencies]
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
hyper = "1"
```

## Part 2: Docker

### Step 3: Dockerfile

Create `Dockerfile` in repo root:

```dockerfile
# Build stage
FROM rust:1.75-slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/
RUN cargo build --release --bin nexus-os-server

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*
RUN useradd -r -s /bin/false nexus
COPY --from=builder /app/target/release/nexus-os-server /usr/local/bin/
RUN chmod +x /usr/local/bin/nexus-os-server
USER nexus
EXPOSE 8080 9090
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1
ENTRYPOINT ["nexus-os-server"]
```

### Step 4: Docker Compose

Create `docker-compose.yml` (already drafted in ENTERPRISE_DEPLOYMENT.md — make it real and tested).

### Step 5: .dockerignore

```
target/
frontend/node_modules/
.git/
*.md
docs/
assets/
```

## Part 3: Helm Chart

### Step 6: Create Helm chart structure

```
helm/nexus-os/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── _helpers.tpl
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── ingress.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── hpa.yaml                    # Horizontal Pod Autoscaler
│   ├── pdb.yaml                    # Pod Disruption Budget
│   ├── serviceaccount.yaml
│   ├── servicemonitor.yaml         # Prometheus ServiceMonitor
│   └── cronjob-backup.yaml         # Scheduled backups
└── README.md
```

### Step 7: Key Helm templates

**deployment.yaml** highlights:
```yaml
spec:
  replicas: {{ .Values.replicaCount }}
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 1
  template:
    spec:
      containers:
        - name: nexus-os
          image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"
          ports:
            - containerPort: 8080  # API
            - containerPort: 9090  # Metrics
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 30
          readinessProbe:
            httpGet:
              path: /ready
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          resources:
            requests:
              cpu: {{ .Values.resources.requests.cpu }}
              memory: {{ .Values.resources.requests.memory }}
            limits:
              cpu: {{ .Values.resources.limits.cpu }}
              memory: {{ .Values.resources.limits.memory }}
          volumeMounts:
            - name: data
              mountPath: /data
            - name: config
              mountPath: /etc/nexus-os
```

**hpa.yaml:**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: {{ include "nexus-os.fullname" . }}
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: {{ include "nexus-os.fullname" . }}
  minReplicas: {{ .Values.autoscaling.minReplicas }}
  maxReplicas: {{ .Values.autoscaling.maxReplicas }}
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
```

**pdb.yaml:**
```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: {{ include "nexus-os.fullname" . }}
spec:
  minAvailable: {{ .Values.highAvailability.minAvailable | default 1 }}
  selector:
    matchLabels:
      app: {{ include "nexus-os.name" . }}
```

## Part 4: High Availability

### Step 8: Shared state for multi-instance

For multi-replica server mode, state must be shared:

**Option A (Simple):** Shared PostgreSQL database
- Replace SQLite with PostgreSQL for server mode
- Connection pooling via `deadpool-postgres` or `sqlx`
- All instances read/write to same database

**Option B (Desktop-compatible):** SQLite with Litestream replication
- Keep SQLite for simplicity
- Use Litestream for real-time replication to S3/GCS
- Read replicas for horizontal read scaling

Recommend **Option A** for server mode, **SQLite** for desktop mode. The data layer should abstract this:

```rust
pub trait DataStore: Send + Sync {
    async fn write_audit(&self, entry: AuditEntry) -> Result<(), DataError>;
    async fn query_audit(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>, DataError>;
    // ... all data operations
}

pub struct SqliteStore { /* desktop mode */ }
pub struct PostgresStore { /* server mode */ }
```

### Step 9: Leader election for singleton tasks

Some operations should only run on one instance:
- Scheduled backups
- Genome evolution cycles
- Audit chain verification

Use Kubernetes lease-based leader election or a simple database lock:

```rust
pub struct LeaderElection {
    pub async fn try_acquire(&self, task: &str, ttl: Duration) -> Result<bool, Error>;
    pub async fn release(&self, task: &str) -> Result<(), Error>;
    pub async fn renew(&self, task: &str, ttl: Duration) -> Result<(), Error>;
}
```

### Step 10: Graceful degradation

If a replica fails:
- Other replicas continue serving requests
- PodDisruptionBudget ensures minimum availability
- Failed agent executions are retried by the Conductor
- Audit chain maintains integrity (each entry includes instance_id)

## Part 5: Configuration

```toml
[server]
mode = "server"           # "desktop" | "server" | "hybrid"
host = "0.0.0.0"
port = 8080
metrics_port = 9090

[database]
backend = "postgres"       # "sqlite" | "postgres"
postgres_url_env = "DATABASE_URL"
pool_size = 20
pool_timeout_seconds = 30

[ha]
leader_election = true
leader_lease_ttl_seconds = 30
instance_id_env = "HOSTNAME"   # K8s pod name
```

## Testing
- Unit test: Health/readiness endpoints
- Unit test: Graceful shutdown completes in-flight requests
- Unit test: DataStore trait abstraction (SQLite + Postgres)
- Integration test: Docker build succeeds
- Integration test: Helm template renders valid YAML (`helm template`)

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.
