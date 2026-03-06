# Deployment Guide

> Single-node and cluster deployment for Nexus OS.

## Platform Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| OS | Linux (x86_64), macOS (ARM64/x86_64), Windows (x86_64) | Ubuntu 22.04+ / macOS 14+ |
| Rust | 1.75+ | Latest stable |
| Node.js | 18+ | 20 LTS |
| RAM | 4 GB | 16 GB |
| Disk | 2 GB | 20 GB (models + audit logs) |
| CPU | 2 cores | 8+ cores |
| GPU | None (CPU inference) | CUDA-capable for local LLM |

## Single Node Setup

### 1. Build from Source

```bash
git clone https://github.com/nexai-lang/nexus-os.git
cd nexus-os

# Build all workspace crates
cargo build --workspace --release

# Build the desktop UI
cd app && npm ci && npm run build && cd ..
```

### 2. Verify the Build

```bash
# Run all tests
cargo test --workspace --all-features

# Check formatting and lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### 3. Run the Desktop App

```bash
cd app
npm run tauri dev
```

### 4. Run via CLI

```bash
# List registered agents
cargo run -p nexus-cli -- agent list

# Check audit trail integrity
cargo run -p nexus-cli -- audit verify

# View system status
cargo run -p nexus-cli -- cluster status
```

## Configuration Reference

### Agent Manifest (`manifest.toml`)

```toml
name = "my-agent"              # Required. 3-64 chars, alphanumeric + hyphens
version = "1.0.0"              # Required. Semver
capabilities = ["llm.query"]   # Required. From capability registry
fuel_budget = 10000            # Required. 1 - 1,000,000
autonomy_level = 2             # Optional. 0-5, default 0
schedule = "*/10 * * * *"      # Optional. Cron expression
llm_model = "claude-sonnet-4-6"  # Optional. Model identifier
consent_policy_path = "consent.toml"  # Optional. Path to consent policy
fuel_period_id = "2024-Q1"     # Optional. Budget period identifier
monthly_fuel_cap = 50000       # Optional. Monthly fuel limit
```

### Autonomy Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| 0 | Inert | Disabled agents |
| 1 | Suggest | Research assistants |
| 2 | Act-with-approval | Code generation, content creation |
| 3 | Act-then-report | Monitoring, alerting |
| 4 | Autonomous-bounded | CI/CD automation |
| 5 | Full autonomy | Self-improvement (restricted) |

### Capability Registry

```
web.search      - Search the web
web.read        - Read web pages
llm.query       - Query language models
fs.read         - Read filesystem
fs.write        - Write filesystem
process.exec    - Execute processes
social.post     - Post to social platforms
social.x.post   - Post to X/Twitter
social.x.read   - Read from X/Twitter
messaging.send  - Send messages
audit.read      - Read audit events
```

## Cluster Setup

### Architecture

A Nexus OS cluster consists of multiple nodes communicating over TCP with length-prefix framed messages. The cluster provides:

- **Audit replication** - All audit events are replicated across nodes
- **Quorum voting** - Governance decisions require majority agreement
- **Membership** - SWIM-style failure detection with heartbeats

### Node Configuration

Each node requires:

1. A unique node ID
2. A bind address for incoming connections
3. Seed node addresses for cluster join

### Starting the First Node

```bash
cargo run -p nexus-cli -- cluster status
```

The first node bootstraps as the primary with quorum authority.

### Joining Additional Nodes

```bash
# From the second machine
cargo run -p nexus-cli -- cluster join --seed 10.0.1.10:9090
```

### Cluster Commands

```bash
# View cluster health
nexus cluster status

# Join a cluster
nexus cluster join --seed <address>

# Leave the cluster gracefully
nexus cluster leave
```

### Connection Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `connect_timeout_secs` | 5 | TCP connection timeout |
| `read_timeout_secs` | 10 | Read timeout for framed messages |
| `max_retries` | 5 | Maximum reconnection attempts |
| `base_retry_delay_ms` | 1000 | Initial retry backoff |
| `max_retry_delay_ms` | 30000 | Maximum retry backoff |

Retry delay follows exponential backoff: `min(base * 2^count, max_delay)`.

### Quorum

Quorum is reached when `active_nodes >= ceil(total_nodes / 2)`. Without quorum, governance decisions are deferred until quorum is restored.

### Wire Message Types

| Type | Purpose |
|------|---------|
| Heartbeat | Node liveness detection |
| AuditSync | Replicate audit events |
| QuorumPropose | Propose a governance decision |
| QuorumVote | Vote on a proposal |
| ReplicationFull | Full state sync for new nodes |
| ReplicationDelta | Incremental state updates |
| AuthChallenge | Node authentication challenge |
| AuthResponse | Authentication response |

## Monitoring

### Audit Trail

```bash
# Show recent audit events
nexus audit show

# Verify hash-chain integrity
nexus audit verify

# Export audit log
nexus audit export --format json

# Check federation sync status
nexus audit federation-status
```

### Compliance

```bash
# Generate SOC 2 compliance report
nexus compliance report

# Check current compliance status
nexus compliance status
```

### Benchmarks

```bash
# Run performance benchmarks
nexus benchmark run

# View benchmark report
nexus benchmark report
```

## Production Checklist

- [ ] Build with `--release` flag
- [ ] Set appropriate autonomy levels for each agent
- [ ] Configure fuel budgets with monthly caps
- [ ] Enable audit log rotation and backup
- [ ] Set up cluster with at least 3 nodes for quorum
- [ ] Verify all compliance controls are satisfied
- [ ] Review agent capabilities - grant minimum required
- [ ] Test kill gates for emergency shutdown
- [ ] Configure HITL approval queues for L1/L2 agents
- [ ] Set up monitoring for fuel burn anomalies
