# PROMPT: OpenTelemetry Instrumentation for Nexus OS

## Context
Nexus OS needs enterprise observability via OpenTelemetry (traces, metrics, logs) to integrate with Prometheus, Grafana, Datadog, Splunk, and ELK stacks. The existing Prometheus metrics endpoint (port 9090) should be preserved and extended.

## Objective
Create a `nexus-telemetry` crate that instruments the governance kernel, agent execution, LLM routing, and audit system with OpenTelemetry.

## Implementation Steps

### Step 1: Create nexus-telemetry crate

```bash
cd crates
cargo new nexus-telemetry --lib
```

**Dependencies:**
```toml
[dependencies]
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["tonic"] }
opentelemetry-prometheus = "0.27"
prometheus = "0.13"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.27"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

### Step 2: Telemetry configuration

```rust
pub struct TelemetryConfig {
    pub enabled: bool,
    pub otlp_endpoint: String,           // e.g., "http://otel-collector:4317"
    pub service_name: String,            // "nexus-os"
    pub sample_rate: f64,                // 0.0–1.0
    pub metrics_export_interval_secs: u64,
    pub log_format: LogFormat,           // Json | Pretty
}
```

### Step 3: Instrument these critical paths

**Agent execution span:**
```
nexus.agent.execute
├── agent_did: String
├── autonomy_level: u8
├── task_type: String
├── nexus.capability.check
│   ├── capability: String
│   └── result: "granted" | "denied"
├── nexus.fuel.check
│   ├── fuel_required: u64
│   ├── fuel_remaining: u64
│   └── result: "sufficient" | "exhausted"
├── nexus.hitl.gate (optional)
│   ├── reason: String
│   ├── decision: "approved" | "denied" | "timeout"
│   └── response_time_ms: u64
├── nexus.sandbox.execute
│   ├── wasm_fuel_consumed: u64
│   ├── memory_peak_bytes: u64
│   └── duration_ms: u64
├── nexus.llm.request (optional)
│   ├── provider: String
│   ├── model: String
│   ├── tokens_input: u64
│   ├── tokens_output: u64
│   └── latency_ms: u64
├── nexus.pii.redaction
│   ├── items_detected: u64
│   └── items_redacted: u64
└── nexus.audit.write
    ├── entry_id: String
    └── chain_length: u64
```

### Step 4: Metrics (extend existing Prometheus)

Add these metrics to the existing Prometheus endpoint:

```rust
// Counters
nexus_agent_executions_total{agent_did, autonomy_level, status}
nexus_capability_checks_total{capability, result}
nexus_hitl_requests_total{decision}
nexus_pii_redactions_total{pii_type}
nexus_output_firewall_blocks_total{reason}
nexus_llm_requests_total{provider, model, status}
nexus_llm_tokens_total{provider, model, direction}  // input/output
nexus_audit_entries_total
nexus_genome_evolutions_total{genome_id, result}

// Histograms
nexus_agent_execution_duration_seconds{agent_did}
nexus_hitl_response_time_seconds
nexus_llm_request_duration_seconds{provider, model}
nexus_sandbox_execution_duration_seconds{agent_did}

// Gauges
nexus_agent_fuel_remaining{agent_did}
nexus_sandbox_active_count
nexus_sandbox_memory_bytes{agent_did}
nexus_uptime_seconds
nexus_active_sessions_count
```

### Step 5: Structured logging

Replace any existing `println!` or `log::` calls with `tracing::` macros:

```rust
tracing::info!(
    agent_did = %did,
    capability = %cap,
    fuel_consumed = fuel,
    "Agent execution completed"
);
```

Configure JSON output for server mode, pretty output for desktop mode.

### Step 6: Health and readiness endpoints

Add to the REST API:
- `GET /health` → 200 if system is running
- `GET /ready` → 200 if all subsystems initialized
- `GET /metrics` → Prometheus exposition format

### Step 7: Grafana dashboard JSON

Create `monitoring/grafana/nexus-os-dashboard.json` with panels for:
- Agent execution heatmap (by autonomy level)
- Fuel consumption trends (top 10 agents)
- HITL approval rates (approved vs denied vs timeout)
- LLM provider latency comparison
- Audit trail growth rate
- PII redaction frequency
- System resource utilization

## Integration Points

Add `nexus-telemetry` as a dependency to:
- `nexus-kernel` (capability checks, fuel metering)
- `nexus-sandbox` (WASM execution metrics)
- `nexus-hitl` (approval timing)
- `nexus-audit` (write metrics)
- `nexus-pii` (redaction counts)
- `nexus-conductor` (orchestration spans)
- LLM router (provider metrics)

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`. Do NOT run workspace-wide tests.
