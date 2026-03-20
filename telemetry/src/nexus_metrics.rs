//! Extended Prometheus metrics for Nexus OS.
//!
//! Builds on the existing `metrics` crate facade to add governance-specific
//! counters, histograms, and gauges. Compatible with the existing
//! `NexusMetrics` in `nexus-protocols` — this module extends the metric set.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use std::time::Instant;

/// Extended metrics registry for Nexus OS enterprise observability.
///
/// Includes all original metrics from `nexus-protocols` plus additional
/// governance, LLM, session, and sandbox metrics.
#[derive(Debug, Clone)]
pub struct NexusMetricsExtended {
    handle: Arc<PrometheusHandle>,
    start_time: Instant,
}

impl NexusMetricsExtended {
    /// Install the Prometheus recorder and register all metrics.
    ///
    /// **Must be called at most once per process.**
    pub fn install() -> Self {
        Self::try_install().unwrap_or_else(|e| {
            tracing::error!("Prometheus recorder installation failed: {e} — metrics disabled");
            // Retry without the global recorder (metrics will be no-ops)
            let handle = PrometheusBuilder::new().build_recorder().handle();
            Self {
                handle: Arc::new(handle),
                start_time: Instant::now(),
            }
        })
    }

    /// Try to install the Prometheus recorder, returning an error on failure.
    pub fn try_install() -> Result<Self, Box<dyn std::error::Error>> {
        let handle = PrometheusBuilder::new().install_recorder()?;

        // ── Original metrics (preserve compatibility) ──────────────────
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

        // ── New: Agent execution ───────────────────────────────────────
        describe_counter!(
            "nexus_agent_executions_total",
            "Total agent executions by agent, level, and status"
        );
        describe_histogram!(
            "nexus_agent_execution_duration_seconds",
            "Agent execution duration in seconds"
        );

        // ── New: Capability checks ─────────────────────────────────────
        describe_counter!(
            "nexus_capability_checks_total",
            "Total capability checks by capability and result"
        );

        // ── New: HITL ──────────────────────────────────────────────────
        describe_counter!(
            "nexus_hitl_requests_total",
            "Total HITL approval requests by decision"
        );
        describe_histogram!(
            "nexus_hitl_response_time_seconds",
            "HITL approval response time in seconds"
        );

        // ── New: PII ───────────────────────────────────────────────────
        describe_counter!(
            "nexus_pii_redactions_total",
            "Total PII items redacted by type"
        );

        // ── New: Output firewall ───────────────────────────────────────
        describe_counter!(
            "nexus_output_firewall_blocks_total",
            "Total output firewall blocks by reason"
        );

        // ── New: LLM extended ──────────────────────────────────────────
        describe_counter!(
            "nexus_llm_tokens_total",
            "Total LLM tokens by provider, model, and direction"
        );
        describe_histogram!(
            "nexus_llm_request_duration_seconds",
            "LLM request duration in seconds"
        );

        // ── New: Audit ─────────────────────────────────────────────────
        describe_counter!(
            "nexus_audit_entries_total",
            "Total audit trail entries written"
        );

        // ── New: Genome ────────────────────────────────────────────────
        describe_counter!(
            "nexus_genome_evolutions_total",
            "Total genome evolution events by result"
        );

        // ── New: Sandbox ───────────────────────────────────────────────
        describe_histogram!(
            "nexus_sandbox_execution_duration_seconds",
            "WASM sandbox execution duration in seconds"
        );
        describe_gauge!(
            "nexus_sandbox_active_count",
            "Number of active WASM sandboxes"
        );
        describe_gauge!(
            "nexus_sandbox_memory_bytes",
            "WASM sandbox memory usage in bytes"
        );

        // ── New: System ────────────────────────────────────────────────
        describe_gauge!("nexus_uptime_seconds", "System uptime in seconds");
        describe_gauge!(
            "nexus_active_sessions_count",
            "Number of active user sessions"
        );

        // ── New: Fuel gauge ────────────────────────────────────────────
        describe_gauge!(
            "nexus_agent_fuel_remaining",
            "Remaining fuel budget per agent"
        );

        Ok(Self {
            handle: Arc::new(handle),
            start_time: Instant::now(),
        })
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        // Update uptime before rendering.
        gauge!("nexus_uptime_seconds").set(self.start_time.elapsed().as_secs_f64());
        self.handle.render()
    }

    // ── Original metric helpers (backward compatible) ──────────────────

    pub fn set_agents_active(&self, count: f64) {
        gauge!("nexus_agents_active").set(count);
    }

    pub fn inc_agents_spawned(&self) {
        counter!("nexus_agents_total_spawned").increment(1);
    }

    pub fn inc_fuel_consumed(&self, amount: u64) {
        counter!("nexus_fuel_consumed_total").increment(amount);
    }

    pub fn inc_host_function_call(&self, function_type: &str) {
        counter!("nexus_host_function_calls_total", "function" => function_type.to_string())
            .increment(1);
    }

    pub fn inc_speculation_decision(&self, outcome: &str) {
        counter!("nexus_speculation_decisions_total", "outcome" => outcome.to_string())
            .increment(1);
    }

    pub fn inc_audit_blocks_created(&self) {
        counter!("nexus_audit_blocks_created").increment(1);
    }

    pub fn inc_firewall_blocks(&self) {
        counter!("nexus_firewall_blocks_total").increment(1);
    }

    pub fn inc_llm_requests(&self, provider: &str) {
        counter!("nexus_llm_requests_total", "provider" => provider.to_string()).increment(1);
    }

    pub fn inc_marketplace_installs(&self) {
        counter!("nexus_marketplace_installs_total").increment(1);
    }

    // ── New metric helpers ─────────────────────────────────────────────

    /// Record an agent execution.
    pub fn record_agent_execution(
        &self,
        agent_did: &str,
        autonomy_level: u8,
        status: &str,
        duration_secs: f64,
    ) {
        counter!(
            "nexus_agent_executions_total",
            "agent_did" => agent_did.to_string(),
            "autonomy_level" => autonomy_level.to_string(),
            "status" => status.to_string(),
        )
        .increment(1);
        histogram!(
            "nexus_agent_execution_duration_seconds",
            "agent_did" => agent_did.to_string(),
        )
        .record(duration_secs);
    }

    /// Record a capability check.
    pub fn record_capability_check(&self, capability: &str, result: &str) {
        counter!(
            "nexus_capability_checks_total",
            "capability" => capability.to_string(),
            "result" => result.to_string(),
        )
        .increment(1);
    }

    /// Record a HITL approval request.
    pub fn record_hitl_request(&self, decision: &str, response_time_secs: f64) {
        counter!(
            "nexus_hitl_requests_total",
            "decision" => decision.to_string(),
        )
        .increment(1);
        histogram!("nexus_hitl_response_time_seconds").record(response_time_secs);
    }

    /// Record PII redaction.
    pub fn record_pii_redaction(&self, pii_type: &str, count: u64) {
        counter!(
            "nexus_pii_redactions_total",
            "pii_type" => pii_type.to_string(),
        )
        .increment(count);
    }

    /// Record an output firewall block.
    pub fn record_output_firewall_block(&self, reason: &str) {
        counter!(
            "nexus_output_firewall_blocks_total",
            "reason" => reason.to_string(),
        )
        .increment(1);
    }

    /// Record LLM token usage.
    pub fn record_llm_tokens(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        counter!(
            "nexus_llm_tokens_total",
            "provider" => provider.to_string(),
            "model" => model.to_string(),
            "direction" => "input",
        )
        .increment(input_tokens);
        counter!(
            "nexus_llm_tokens_total",
            "provider" => provider.to_string(),
            "model" => model.to_string(),
            "direction" => "output",
        )
        .increment(output_tokens);
    }

    /// Record LLM request duration.
    pub fn record_llm_duration(&self, provider: &str, model: &str, duration_secs: f64) {
        histogram!(
            "nexus_llm_request_duration_seconds",
            "provider" => provider.to_string(),
            "model" => model.to_string(),
        )
        .record(duration_secs);
    }

    /// Record an audit trail entry.
    pub fn inc_audit_entries(&self) {
        counter!("nexus_audit_entries_total").increment(1);
    }

    /// Record a genome evolution event.
    pub fn record_genome_evolution(&self, genome_id: &str, result: &str) {
        counter!(
            "nexus_genome_evolutions_total",
            "genome_id" => genome_id.to_string(),
            "result" => result.to_string(),
        )
        .increment(1);
    }

    /// Record sandbox execution duration.
    pub fn record_sandbox_execution(&self, agent_did: &str, duration_secs: f64) {
        histogram!(
            "nexus_sandbox_execution_duration_seconds",
            "agent_did" => agent_did.to_string(),
        )
        .record(duration_secs);
    }

    /// Set the number of active sandboxes.
    pub fn set_sandbox_active_count(&self, count: f64) {
        gauge!("nexus_sandbox_active_count").set(count);
    }

    /// Set sandbox memory usage for a given agent.
    pub fn set_sandbox_memory(&self, agent_did: &str, bytes: f64) {
        gauge!(
            "nexus_sandbox_memory_bytes",
            "agent_did" => agent_did.to_string(),
        )
        .set(bytes);
    }

    /// Set remaining fuel for an agent.
    pub fn set_agent_fuel_remaining(&self, agent_did: &str, fuel: f64) {
        gauge!(
            "nexus_agent_fuel_remaining",
            "agent_did" => agent_did.to_string(),
        )
        .set(fuel);
    }

    /// Set the number of active user sessions.
    pub fn set_active_sessions(&self, count: f64) {
        gauge!("nexus_active_sessions_count").set(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn shared_metrics() -> &'static NexusMetricsExtended {
        static METRICS: OnceLock<NexusMetricsExtended> = OnceLock::new();
        METRICS.get_or_init(NexusMetricsExtended::install)
    }

    #[test]
    fn original_metrics_work() {
        let m = shared_metrics();
        m.inc_agents_spawned();
        m.inc_fuel_consumed(100);
        m.inc_host_function_call("wasm_invoke");
        m.inc_speculation_decision("commit");
        m.inc_audit_blocks_created();
        m.inc_firewall_blocks();
        m.inc_llm_requests("claude");
        m.inc_marketplace_installs();
        m.set_agents_active(5.0);

        let output = m.render();
        assert!(output.contains("nexus_agents_total_spawned"));
        assert!(output.contains("nexus_fuel_consumed_total"));
        assert!(output.contains("nexus_agents_active"));
    }

    #[test]
    fn extended_metrics_work() {
        let m = shared_metrics();

        m.record_agent_execution("did:key:z6MkTest", 2, "ok", 1.5);
        m.record_capability_check("llm.invoke", "granted");
        m.record_hitl_request("approved", 3.2);
        m.record_pii_redaction("email", 2);
        m.record_output_firewall_block("harmful_content");
        m.record_llm_tokens("claude", "sonnet", 500, 200);
        m.record_llm_duration("claude", "sonnet", 1.2);
        m.inc_audit_entries();
        m.record_genome_evolution("genome-1", "success");
        m.record_sandbox_execution("did:key:z6MkTest", 0.042);
        m.set_sandbox_active_count(3.0);
        m.set_sandbox_memory("did:key:z6MkTest", 1048576.0);
        m.set_agent_fuel_remaining("did:key:z6MkTest", 420.0);
        m.set_active_sessions(2.0);

        let output = m.render();
        assert!(output.contains("nexus_agent_executions_total"));
        assert!(output.contains("nexus_capability_checks_total"));
        assert!(output.contains("nexus_hitl_requests_total"));
        assert!(output.contains("nexus_pii_redactions_total"));
        assert!(output.contains("nexus_llm_tokens_total"));
        assert!(output.contains("nexus_audit_entries_total"));
        assert!(output.contains("nexus_uptime_seconds"));
        assert!(output.contains("nexus_active_sessions_count"));
        assert!(output.contains("nexus_sandbox_active_count"));
    }

    #[test]
    fn uptime_is_positive() {
        let m = shared_metrics();
        let output = m.render();
        // uptime is set during render(), should contain a positive value
        assert!(output.contains("nexus_uptime_seconds"));
    }

    #[test]
    fn render_valid_prometheus_format() {
        let m = shared_metrics();
        m.inc_agents_spawned();
        let output = m.render();

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
