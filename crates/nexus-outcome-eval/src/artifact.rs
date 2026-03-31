//! Compliance-ready outcome artifact generation and verification.

use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::types::*;

/// Generates and verifies compliance-ready outcome artifacts.
pub struct OutcomeArtifactGenerator;

impl OutcomeArtifactGenerator {
    /// Generate a complete outcome artifact combining assessment + context.
    pub fn generate(
        assessment: &OutcomeAssessment,
        spec: &OutcomeSpec,
        action_log: Vec<serde_json::Value>,
        memory_entries_created: usize,
        rollbacks_performed: usize,
        governance_events: Vec<serde_json::Value>,
    ) -> OutcomeArtifact {
        let mut artifact = OutcomeArtifact {
            assessment: assessment.clone(),
            spec: spec.clone(),
            action_log,
            memory_entries_created,
            rollbacks_performed,
            governance_events,
            generated_at: Utc::now(),
            artifact_hash: String::new(),
        };
        artifact.artifact_hash = Self::compute_hash(&artifact);
        artifact
    }

    /// Generate a human-readable report from an artifact.
    pub fn generate_report(artifact: &OutcomeArtifact) -> String {
        let a = &artifact.assessment;
        let s = &artifact.spec;

        let verdict_icon = match a.verdict {
            OutcomeVerdict::Success => "\u{2705}",
            OutcomeVerdict::PartialSuccess => "\u{26A0}\u{FE0F}",
            OutcomeVerdict::Failure => "\u{274C}",
            OutcomeVerdict::PendingReview => "\u{23F3}",
            OutcomeVerdict::Inconclusive => "\u{2753}",
        };

        let mut lines = Vec::new();

        lines.push("\u{2550}".repeat(55));
        lines.push("NEXUS OS \u{2014} OUTCOME EVALUATION REPORT".to_string());
        lines.push("\u{2550}".repeat(55));
        lines.push(String::new());
        lines.push(format!("Task:      {}", a.task_id));
        lines.push(format!("Agent:     {}", a.agent_id));
        lines.push(format!("Goal:      {}", s.goal_description));
        lines.push(format!(
            "Verdict:   {verdict_icon} {} (score: {:.2})",
            a.verdict, a.score
        ));
        lines.push(format!(
            "Evaluated: {}",
            a.evaluated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        lines.push(String::new());

        // Criteria results
        lines.push("CRITERIA RESULTS".to_string());
        lines.push("\u{2500}".repeat(55));
        for cr in &a.criteria_results {
            let icon = if cr.passed { "\u{2705}" } else { "\u{274C}" };
            lines.push(format!(
                "{icon} {} \u{2014} Score: {:.2}",
                cr.criterion_description, cr.score
            ));
            lines.push(format!("   Evidence: {}", cr.evidence));
        }
        lines.push(String::new());

        // Constraint results
        if !a.constraint_results.is_empty() {
            lines.push("CONSTRAINT RESULTS".to_string());
            lines.push("\u{2500}".repeat(55));
            for cr in &a.constraint_results {
                let icon = if cr.violated { "\u{274C}" } else { "\u{2705}" };
                let status = if cr.violated {
                    "VIOLATED"
                } else {
                    "Not violated"
                };
                lines.push(format!(
                    "{icon} {} \u{2014} {status}",
                    cr.constraint_description
                ));
                lines.push(format!("   Evidence: {}", cr.evidence));
            }
            lines.push(String::new());
        }

        // Execution summary
        lines.push("EXECUTION SUMMARY".to_string());
        lines.push("\u{2500}".repeat(55));
        lines.push(format!(
            "Actions taken:            {}",
            artifact.action_log.len()
        ));
        lines.push(format!(
            "Memory entries created:    {}",
            artifact.memory_entries_created
        ));
        lines.push(format!(
            "Rollbacks performed:      {}",
            artifact.rollbacks_performed
        ));
        lines.push(format!(
            "Governance events:        {}",
            artifact.governance_events.len()
        ));
        lines.push(format!(
            "Evaluation duration:      {}ms",
            a.evaluation_duration_ms
        ));
        lines.push(String::new());

        // Audit
        lines.push("AUDIT".to_string());
        lines.push("\u{2500}".repeat(55));
        lines.push(format!("Artifact hash:     {}", artifact.artifact_hash));
        lines.push(format!("Assessment hash:   {}", a.audit_hash));
        let integrity = Self::verify_integrity(artifact);
        let integrity_icon = if integrity { "\u{2705}" } else { "\u{274C}" };
        lines.push(format!(
            "Tamper-evident:    {integrity_icon} {}",
            if integrity { "Verified" } else { "FAILED" }
        ));
        lines.push(String::new());
        lines.push("\u{2550}".repeat(55));

        lines.join("\n")
    }

    /// Verify artifact integrity by recomputing hash.
    pub fn verify_integrity(artifact: &OutcomeArtifact) -> bool {
        let expected = Self::compute_hash(artifact);
        expected == artifact.artifact_hash
    }

    /// Compute the SHA-256 hash of an artifact (excluding the hash field itself).
    fn compute_hash(artifact: &OutcomeArtifact) -> String {
        let data = serde_json::json!({
            "assessment_id": artifact.assessment.id.to_string(),
            "spec_id": artifact.spec.id.to_string(),
            "verdict": artifact.assessment.verdict,
            "score": artifact.assessment.score,
            "action_count": artifact.action_log.len(),
            "memory_entries": artifact.memory_entries_created,
            "rollbacks": artifact.rollbacks_performed,
            "governance_count": artifact.governance_events.len(),
            "generated_at": artifact.generated_at.to_rfc3339(),
        });
        let serialized = serde_json::to_string(&data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
    }
}
