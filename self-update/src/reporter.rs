use crate::analyzer::BugReport;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub trait TelemetrySink {
    fn send(&mut self, payload: Value);
}

#[derive(Debug, Default)]
pub struct InMemoryTelemetrySink {
    sent: Vec<Value>,
}

impl InMemoryTelemetrySink {
    pub fn sent(&self) -> &[Value] {
        &self.sent
    }
}

impl TelemetrySink for InMemoryTelemetrySink {
    fn send(&mut self, payload: Value) {
        self.sent.push(payload);
    }
}

pub struct AutoReporter<S: TelemetrySink> {
    telemetry_opt_in: bool,
    sink: S,
}

impl<S: TelemetrySink> AutoReporter<S> {
    pub fn new(sink: S) -> Self {
        Self {
            telemetry_opt_in: false,
            sink,
        }
    }

    pub fn telemetry_opt_in(&self) -> bool {
        self.telemetry_opt_in
    }

    pub fn set_telemetry_opt_in(&mut self, enabled: bool) {
        self.telemetry_opt_in = enabled;
    }

    pub fn format_bug_report(&self, report: &BugReport) -> String {
        let steps = report
            .steps_to_reproduce
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {}", index + 1, step))
            .collect::<Vec<_>>()
            .join("\n");
        let excerpt = if report.audit_trail_excerpt.is_empty() {
            "- none".to_string()
        } else {
            report
                .audit_trail_excerpt
                .iter()
                .map(|line| format!("- {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "Bug Report {}\nSeverity: {}\nRoot Cause: {}\n\nSteps to Reproduce:\n{}\n\nExpected:\n{}\n\nActual:\n{}\n\nAudit Trail Excerpt:\n{}",
            report.report_id,
            report.severity,
            report.root_cause,
            steps,
            report.expected_behavior,
            report.actual_behavior,
            excerpt
        )
    }

    pub fn submit_report(&mut self, report: &BugReport) -> String {
        let formatted = self.format_bug_report(report);
        if self.telemetry_opt_in {
            self.sink.send(anonymized_payload(report));
        }
        formatted
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }
}

fn anonymized_payload(report: &BugReport) -> Value {
    let agent_hash = sha256_hex(report.agent_id.as_str());
    let report_hash = sha256_hex(report.report_id.as_str());
    json!({
        "report_hash": report_hash,
        "agent_hash": agent_hash,
        "severity": report.severity,
        "root_cause": report.root_cause,
        "steps_count": report.steps_to_reproduce.len(),
        "audit_excerpt_count": report.audit_trail_excerpt.len(),
    })
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{AutoReporter, InMemoryTelemetrySink};
    use crate::analyzer::BugReport;

    fn sample_report() -> BugReport {
        BugReport {
            report_id: "bug-123".to_string(),
            agent_id: "agent-sensitive-id".to_string(),
            severity: "high".to_string(),
            root_cause: "Capability denied".to_string(),
            steps_to_reproduce: vec![
                "Create agent".to_string(),
                "Trigger tool call".to_string(),
                "Observe crash".to_string(),
            ],
            expected_behavior: "Graceful recovery".to_string(),
            actual_behavior: "Process terminated".to_string(),
            audit_trail_excerpt: vec!["[1] error ...".to_string()],
        }
    }

    #[test]
    fn test_telemetry_opt_in() {
        let mut reporter = AutoReporter::new(InMemoryTelemetrySink::default());
        let report = sample_report();

        let _ = reporter.submit_report(&report);
        assert!(!reporter.telemetry_opt_in());
        assert_eq!(reporter.sink().sent().len(), 0);

        reporter.set_telemetry_opt_in(true);
        let _ = reporter.submit_report(&report);
        assert_eq!(reporter.sink().sent().len(), 1);

        let sent = &reporter.sink().sent()[0];
        assert!(sent.get("agent_hash").is_some());
        assert!(sent.get("report_hash").is_some());
        assert!(sent.get("agent_id").is_none());
        assert!(sent.get("report_id").is_none());
    }
}
