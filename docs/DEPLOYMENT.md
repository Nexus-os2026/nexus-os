# Deployment Guide

> Docker, Kubernetes, and air-gapped deployment for Nexus OS.

## Platform Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| OS | Linux (x86_64), macOS (ARM64/x86_64), Windows (x86_64) | Ubuntu 22.04+ / macOS 14+ |
| Rust | 1.82+ | Latest stable |
| Node.js | 18+ | 22 LTS |
| RAM | 4 GB | 16 GB |
| Disk | 2 GB | 20 GB (models + audit logs) |
| CPU | 2 cores | 8+ cores |
| GPU | None (CPU inference) | CUDA-capable for local LLM |

---

## Quick Start (Docker)

### Prerequisites

- Docker 24+ and Docker Compose v2
- 4 GB RAM minimum

### One-command start

```bash
docker compose up -d
```

Access the UI at **http://localhost:8080**

### With Ollama (local LLM inference)

```bash
docker compose --profile with-ollama up -d
```

### HA mode (PostgreSQL + 2 replicas)

```bash
docker compose --profile ha up -d
```

### Verify

```bash
curl http://localhost:8080/health
```

### Stop

```bash
docker compose down
```

---

## Kubernetes (Helm)

### Prerequisites

- kubectl configured with cluster access
- Helm 3.x

### Install

```bash
helm install nexus-os helm/nexus-os/
```

### With custom values

```bash
helm install nexus-os helm/nexus-os/ -f my-values.yaml
```

### Common overrides

```bash
# Enable ingress
helm install nexus-os helm/nexus-os/ \
  --set ingress.enabled=true \
  --set ingress.hosts[0].host=nexus.mycompany.com \
  --set ingress.hosts[0].paths[0].path=/ \
  --set ingress.hosts[0].paths[0].pathType=Prefix

# Enable Prometheus monitoring
helm install nexus-os helm/nexus-os/ \
  --set serviceMonitor.enabled=true

# Scale with HPA
helm install nexus-os helm/nexus-os/ \
  --set replicaCount=3 \
  --set autoscaling.enabled=true

# With Ollama sidecar
helm install nexus-os helm/nexus-os/ \
  --set ollama.enabled=true

# PostgreSQL backend for HA
helm install nexus-os helm/nexus-os/ \
  --set database.backend=postgres \
  --set database.postgresUrl="postgres://nexus:secret@postgres:5432/nexus"
```

### Upgrade

```bash
helm upgrade nexus-os helm/nexus-os/
```

### Port-forward (no ingress)

```bash
kubectl port-forward svc/nexus-os 8080:8080
```

---

## Air-Gapped Deployment

For environments without internet access.

### 1. Pre-pull and save images

```bash
# On a machine with internet
docker pull ghcr.io/nexus-os2026/nexus-os:10.5.0
docker pull ollama/ollama:latest  # if needed

docker save ghcr.io/nexus-os2026/nexus-os:10.5.0 -o nexus-os.tar
docker save ollama/ollama:latest -o ollama.tar
```

### 2. Transfer and load

```bash
# On the air-gapped machine
docker load -i nexus-os.tar
docker load -i ollama.tar
```

### 3. Bundle Helm chart

```bash
# On a machine with internet
helm package helm/nexus-os/

# Transfer nexus-os-1.1.0.tgz to the air-gapped environment
```

### 4. Install from local archive

```bash
helm install nexus-os nexus-os-1.1.0.tgz \
  --set image.pullPolicy=Never \
  --set ollama.enabled=true
```

### 5. Configure local LLM (no internet required)

With Ollama loaded, pull a model on the air-gapped machine from a pre-downloaded model file or serve models already bundled in the Ollama volume.

---

## Headless Server (Binary)

For running without Docker or Kubernetes.

### Build

```bash
cargo build --release -p nexus-protocols --bin nexus-server
```

### Run

```bash
./target/release/nexus-server start
```

### CLI server (alternative)

```bash
cargo build --release -p nexus-server
./target/release/nexus-server --port 3000 --mcp-port 3001 --a2a-port 3002
```

---

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NEXUS_HTTP_ADDR` | `0.0.0.0:8080` | HTTP listen address |
| `NEXUS_FRONTEND_DIST` | `/opt/nexus/app/dist` | Path to React frontend build |
| `NEXUS_CONFIG_PATH` | `/data/config` | Configuration directory |
| `NEXUS_MODE` | `server` | Runtime mode |
| `NEXUS_LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |
| `NEXUS_SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout |
| `NEXUS_CORS_ORIGINS` | `*` | Allowed CORS origins |
| `JWT_SECRET` | (required) | JWT signing secret |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama API endpoint |
| `DATABASE_URL` | (sqlite) | PostgreSQL connection URL for HA mode |

### Helm Values

| Value | Default | Description |
|-------|---------|-------------|
| `replicaCount` | `2` | Number of pod replicas |
| `image.repository` | `registry.gitlab.com/nexaiceo/nexus-os` | Container image |
| `image.tag` | `""` (appVersion) | Image tag |
| `service.type` | `ClusterIP` | Service type |
| `service.port` | `8080` | HTTP port |
| `service.metricsPort` | `9090` | Metrics port |
| `ingress.enabled` | `false` | Enable ingress |
| `persistence.enabled` | `true` | Enable PVC |
| `persistence.size` | `10Gi` | PVC size |
| `autoscaling.enabled` | `true` | Enable HPA |
| `autoscaling.minReplicas` | `2` | Minimum replicas |
| `autoscaling.maxReplicas` | `10` | Maximum replicas |
| `ollama.enabled` | `false` | Enable Ollama sidecar |
| `serviceMonitor.enabled` | `false` | Enable Prometheus ServiceMonitor |
| `backup.enabled` | `false` | Enable backup CronJob |
| `database.backend` | `sqlite` | Database backend (sqlite/postgres) |

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Server health + version |
| GET | `/status` | Agent count, uptime, providers |
| GET | `/api/v1/agents` | List all agents |
| POST | `/api/v1/agents/{id}/run` | Execute an agent task |
| GET | `/api/v1/agents/{id}/status` | Agent execution status |
| GET | `/api/v1/audit` | Query audit trail |
| GET | `/mcp/tools/list` | MCP tool listing |
| POST | `/mcp/tools/invoke` | MCP tool invocation |
| POST | `/a2a` | A2A task submission |
| GET | `/a2a/agent-card` | A2A agent discovery |

---

## Monitoring & Observability

### Prometheus Metrics

Metrics are exposed on port **9090** at `/metrics`.

Enable the Prometheus ServiceMonitor:

```bash
helm install nexus-os helm/nexus-os/ --set serviceMonitor.enabled=true
```

### Grafana Dashboard

Pre-built dashboards are in `monitoring/grafana/`.

### Health Check

The `/health` endpoint returns:

```json
{
  "status": "healthy",
  "version": "10.5.0",
  "agents_registered": 5,
  "tasks_in_flight": 2,
  "uptime_secs": 3600,
  "audit_valid": true,
  "memory_usage_bytes": 104857600,
  "wasm_cache_hit_rate": 0.85
}
```

---

## Security Hardening

### Container Security

The default Dockerfile and Helm chart enforce:
- Non-root user (UID 1000)
- Read-only root filesystem
- All capabilities dropped
- No privilege escalation

### Network Policies

Restrict traffic to only required ports:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: nexus-os
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: nexus-os
  policyTypes: [Ingress, Egress]
  ingress:
    - ports:
        - port: 8080
        - port: 9090
  egress:
    - ports:
        - port: 443   # LLM API calls
        - port: 11434  # Ollama
```

### Secret Management

For production, use external secret stores:

```bash
# Use existing Kubernetes secret for JWT
helm install nexus-os helm/nexus-os/ \
  --set auth.existingSecret=my-jwt-secret \
  --set auth.existingSecretKey=jwt-secret

# Use existing secret for database URL
helm install nexus-os helm/nexus-os/ \
  --set database.existingSecret=my-db-secret \
  --set database.existingSecretKey=database-url
```

### TLS Termination

Use ingress with TLS:

```yaml
ingress:
  enabled: true
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: nexus.mycompany.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: nexus-tls
      hosts:
        - nexus.mycompany.com
```

---

## Production Checklist

- [ ] Change `JWT_SECRET` from default
- [ ] Enable TLS via ingress or load balancer
- [ ] Set resource limits appropriate to workload
- [ ] Enable persistence with appropriate storage class
- [ ] Configure backup CronJob for audit data
- [ ] Enable ServiceMonitor for Prometheus
- [ ] Use `existingSecret` for all credentials
- [ ] Set appropriate autonomy levels for each agent
- [ ] Configure fuel budgets with monthly caps
- [ ] Review agent capabilities — grant minimum required
- [ ] Test kill gates for emergency shutdown
- [ ] Set up alerts for fuel burn anomalies
