//! `nexus-telemetry` — OpenTelemetry-compatible instrumentation for Nexus OS.
//!
//! Provides enterprise observability via:
//! - **Prometheus metrics** (extended from existing `nexus-protocols` metrics)
//! - **Structured spans** for agent execution, capability checks, HITL gates,
//!   LLM requests, PII redaction, and audit writes
//! - **Structured logging** via `tracing` with JSON or pretty output
//! - **Health/readiness** endpoint data structures
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐   ┌──────────────┐   ┌───────────────┐
//! │  Kernel      │──▶│ nexus-       │──▶│ Prometheus     │
//! │  Agents      │   │ telemetry    │   │ Grafana        │
//! │  LLM Router  │   │              │──▶│ Datadog/Splunk │
//! │  Audit       │   │  spans       │   │ ELK            │
//! └─────────────┘   │  metrics     │   └───────────────┘
//!                    │  logging     │
//!                    │  health      │
//!                    └──────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use nexus_telemetry::{TelemetryConfig, NexusMetricsExtended, init_logging};
//!
//! // Initialize logging (call once at startup)
//! let config = TelemetryConfig::server();
//! init_logging(&config);
//!
//! // Install metrics (call once at startup)
//! let metrics = NexusMetricsExtended::install();
//!
//! // Record metrics throughout the application
//! metrics.record_agent_execution("did:key:z6MkAgent1", 2, "ok", 1.5);
//! metrics.record_capability_check("llm.invoke", "granted");
//! metrics.record_llm_tokens("claude", "sonnet", 500, 200);
//!
//! // Render Prometheus exposition format
//! let output = metrics.render();
//! ```

pub mod config;
pub mod health;
pub mod logging;
pub mod nexus_metrics;
pub mod spans;

// Re-exports for convenience.
pub use config::{LogFormat, TelemetryConfig};
pub use health::{subsystem, HealthResponse, HealthStatus, ReadinessResponse, SubsystemStatus};
pub use logging::init_logging;
pub use nexus_metrics::NexusMetricsExtended;
pub use spans::{
    emit_agent_execution, emit_llm_request, span_to_attributes, AgentExecutionSpan, AuditWriteSpan,
    CapabilityCheckSpan, CheckResult, FuelCheckSpan, HitlDecision, HitlGateSpan, LlmRequestSpan,
    PiiRedactionSpan, SandboxSpan,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn re_exports_accessible() {
        let config = TelemetryConfig::default();
        assert!(!config.service_name.is_empty());
        assert!(matches!(HealthStatus::Healthy, HealthStatus::Healthy));
        assert!(matches!(CheckResult::Granted, CheckResult::Granted));
        assert!(matches!(HitlDecision::Approved, HitlDecision::Approved));
        let sub = subsystem("test", true, None);
        assert_eq!(sub.name, "test");
        assert!(sub.ready);
    }

    #[test]
    fn config_to_logging_integration() {
        let config = TelemetryConfig::server();
        assert_eq!(config.log_format, LogFormat::Json);
        // init_logging may fail if subscriber is already set (from other tests),
        // but it should not panic.
        let _ = init_logging(&config);
    }
}
