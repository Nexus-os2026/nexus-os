//! Prometheus-compatible metrics for Nexus OS.
//!
//! Uses the `metrics` crate facade with a Prometheus exporter recorder.
//! All counters/gauges are registered at construction time and can be
//! incremented from any component that holds a reference to [`NexusMetrics`].

use metrics::{counter, describe_counter, describe_gauge, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;

/// Central metrics registry for Nexus OS.
///
/// Wraps a [`PrometheusHandle`] that can render the Prometheus text exposition
/// format on demand via [`render`](NexusMetrics::render).
#[derive(Debug, Clone)]
pub struct NexusMetrics {
    handle: Arc<PrometheusHandle>,
}

impl NexusMetrics {
    /// Create a new metrics instance and install the Prometheus recorder.
    ///
    /// **Must be called at most once per process** (the `metrics` crate only
    /// supports a single global recorder).
    pub fn install() -> Self {
        let builder = PrometheusBuilder::new();
        let handle = builder
            .install_recorder()
            .expect("failed to install Prometheus recorder");

        // Describe all metrics up-front so /metrics always has HELP/TYPE lines.
        describe_gauge!("nexus_agents_active", "Number of currently active agents");
        describe_counter!(
            "nexus_agents_total_spawned",
            "Total number of agents spawned"
        );
        describe_counter!(
            "nexus_fuel_consumed_total",
            "Total fuel units consumed across all agents"
        );
        describe_counter!(
            "nexus_host_function_calls_total",
            "Total host function calls by function type"
        );
        describe_counter!(
            "nexus_speculation_decisions_total",
            "Total speculative execution decisions by outcome"
        );
        describe_counter!(
            "nexus_audit_blocks_created",
            "Total audit blocks created in the hash chain"
        );
        describe_counter!(
            "nexus_firewall_blocks_total",
            "Total prompts blocked by the prompt firewall"
        );
        describe_counter!("nexus_llm_requests_total", "Total LLM requests by provider");
        describe_counter!(
            "nexus_marketplace_installs_total",
            "Total marketplace agent installs"
        );

        Self {
            handle: Arc::new(handle),
        }
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        self.handle.render()
    }

    // ── Gauge helpers ────────────────────────────────────────────────────

    /// Set the number of currently active agents.
    pub fn set_agents_active(&self, count: f64) {
        gauge!("nexus_agents_active").set(count);
    }

    // ── Counter helpers ──────────────────────────────────────────────────

    /// Record that a new agent was spawned.
    pub fn inc_agents_spawned(&self) {
        counter!("nexus_agents_total_spawned").increment(1);
    }

    /// Record fuel consumption.
    pub fn inc_fuel_consumed(&self, amount: u64) {
        counter!("nexus_fuel_consumed_total").increment(amount);
    }

    /// Record a host function call, keyed by function type.
    pub fn inc_host_function_call(&self, function_type: &str) {
        counter!("nexus_host_function_calls_total", "function" => function_type.to_string())
            .increment(1);
    }

    /// Record a speculation decision (commit, rollback, or review).
    pub fn inc_speculation_decision(&self, outcome: &str) {
        counter!("nexus_speculation_decisions_total", "outcome" => outcome.to_string())
            .increment(1);
    }

    /// Record an audit block creation.
    pub fn inc_audit_blocks_created(&self) {
        counter!("nexus_audit_blocks_created").increment(1);
    }

    /// Record a firewall block event.
    pub fn inc_firewall_blocks(&self) {
        counter!("nexus_firewall_blocks_total").increment(1);
    }

    /// Record an LLM request, keyed by provider.
    pub fn inc_llm_requests(&self, provider: &str) {
        counter!("nexus_llm_requests_total", "provider" => provider.to_string()).increment(1);
    }

    /// Record a marketplace install.
    pub fn inc_marketplace_installs(&self) {
        counter!("nexus_marketplace_installs_total").increment(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: Because the `metrics` crate uses a global recorder, we can only
    // install it once per process. Tests that need the recorder should use
    // a single shared instance via `std::sync::OnceLock`.
    use std::sync::OnceLock;

    fn shared_metrics() -> &'static NexusMetrics {
        static METRICS: OnceLock<NexusMetrics> = OnceLock::new();
        METRICS.get_or_init(NexusMetrics::install)
    }

    #[test]
    fn metrics_increment_correctly() {
        let m = shared_metrics();

        // Increment various counters
        m.inc_agents_spawned();
        m.inc_agents_spawned();
        m.inc_fuel_consumed(100);
        m.inc_fuel_consumed(50);
        m.inc_host_function_call("wasm_invoke");
        m.inc_host_function_call("wasm_invoke");
        m.inc_host_function_call("memory_alloc");
        m.inc_speculation_decision("commit");
        m.inc_speculation_decision("rollback");
        m.inc_speculation_decision("review");
        m.inc_audit_blocks_created();
        m.inc_firewall_blocks();
        m.inc_firewall_blocks();
        m.inc_llm_requests("claude");
        m.inc_llm_requests("local-slm");
        m.inc_marketplace_installs();
        m.set_agents_active(3.0);

        let output = m.render();

        // Verify Prometheus text format contains our metrics
        assert!(
            output.contains("nexus_agents_total_spawned"),
            "missing agents_total_spawned"
        );
        assert!(
            output.contains("nexus_fuel_consumed_total"),
            "missing fuel_consumed_total"
        );
        assert!(
            output.contains("nexus_host_function_calls_total"),
            "missing host_function_calls"
        );
        assert!(
            output.contains("nexus_speculation_decisions_total"),
            "missing speculation_decisions"
        );
        assert!(
            output.contains("nexus_audit_blocks_created"),
            "missing audit_blocks_created"
        );
        assert!(
            output.contains("nexus_firewall_blocks_total"),
            "missing firewall_blocks"
        );
        assert!(
            output.contains("nexus_llm_requests_total"),
            "missing llm_requests"
        );
        assert!(
            output.contains("nexus_marketplace_installs_total"),
            "missing marketplace_installs"
        );
        assert!(
            output.contains("nexus_agents_active"),
            "missing agents_active"
        );
    }

    #[test]
    fn render_returns_valid_prometheus_format() {
        let m = shared_metrics();
        m.inc_agents_spawned();

        let output = m.render();

        // Prometheus text format: lines are either comments (# ...) or metric lines
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            assert!(
                trimmed.starts_with('#') || trimmed.starts_with("nexus_") || trimmed.contains('{'),
                "unexpected Prometheus line: {trimmed}"
            );
        }
    }
}
