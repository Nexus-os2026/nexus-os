//! Regulatory compliance mapping — cross-reference requirements across EU AI Act,
//! NIST AI RMF, and ISO 42001, with gap analysis and scoring.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Types ───────────────────────────────────────────────────────────────

/// Supported regulatory frameworks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Framework {
    EuAiAct,
    NistAiRmf,
    Iso42001,
}

/// Compliance status of a requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceStatus {
    Met,
    Partial,
    NotMet,
    NotApplicable,
}

/// A single regulatory requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub req_id: String,
    pub framework: Framework,
    pub title: String,
    pub description: String,
    pub status: ComplianceStatus,
    pub evidence: Option<String>,
}

/// A compliance gap identified during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGap {
    pub req_id: String,
    pub framework: Framework,
    pub title: String,
    pub status: ComplianceStatus,
    pub remediation: String,
}

/// Full compliance report across one or more frameworks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub report_id: String,
    pub frameworks: Vec<Framework>,
    pub scores: HashMap<String, f64>,
    pub total_requirements: usize,
    pub met_count: usize,
    pub partial_count: usize,
    pub not_met_count: usize,
    pub gaps: Vec<ComplianceGap>,
    pub generated_at: u64,
}

// ── Engine ──────────────────────────────────────────────────────────────

/// Compliance crosswalk engine mapping requirements across regulatory frameworks.
pub struct ComplianceCrosswalk {
    requirements: Vec<Requirement>,
}

impl Default for ComplianceCrosswalk {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplianceCrosswalk {
    /// Create a new crosswalk pre-populated with framework requirements.
    pub fn new() -> Self {
        let mut cw = Self {
            requirements: Vec::new(),
        };
        cw.populate_eu_ai_act();
        cw.populate_nist_ai_rmf();
        cw.populate_iso_42001();
        cw
    }

    fn populate_eu_ai_act(&mut self) {
        let reqs = vec![
            (
                "EU-AI-6",
                "Risk Classification",
                "Classify AI systems by risk level",
            ),
            (
                "EU-AI-9",
                "Risk Management System",
                "Establish and maintain a risk management system",
            ),
            (
                "EU-AI-10",
                "Data Governance",
                "Ensure training data quality and governance",
            ),
            (
                "EU-AI-11",
                "Technical Documentation",
                "Maintain comprehensive technical documentation",
            ),
            (
                "EU-AI-13",
                "Transparency",
                "Ensure transparency of AI system operation",
            ),
            (
                "EU-AI-14",
                "Human Oversight",
                "Enable effective human oversight mechanisms",
            ),
            (
                "EU-AI-15",
                "Accuracy & Robustness",
                "Ensure accuracy, robustness, and cybersecurity",
            ),
            (
                "EU-AI-52",
                "Record Keeping",
                "Maintain logs for traceability",
            ),
        ];

        for (id, title, desc) in reqs {
            self.requirements.push(Requirement {
                req_id: id.to_string(),
                framework: Framework::EuAiAct,
                title: title.to_string(),
                description: desc.to_string(),
                status: ComplianceStatus::NotMet,
                evidence: None,
            });
        }
    }

    fn populate_nist_ai_rmf(&mut self) {
        let reqs = vec![
            (
                "NIST-GOV-1",
                "Governance Policies",
                "Establish AI governance policies and procedures",
            ),
            (
                "NIST-GOV-2",
                "Accountability",
                "Define roles and responsibilities for AI risk management",
            ),
            (
                "NIST-MAP-1",
                "Context Mapping",
                "Map AI system context, capabilities, and limitations",
            ),
            (
                "NIST-MAP-2",
                "Stakeholder Impact",
                "Identify and assess impacts on stakeholders",
            ),
            (
                "NIST-MEA-1",
                "Performance Metrics",
                "Define and track AI performance metrics",
            ),
            (
                "NIST-MEA-2",
                "Bias Measurement",
                "Measure and monitor for bias and fairness",
            ),
            (
                "NIST-MAN-1",
                "Risk Treatment",
                "Implement risk treatment and mitigation strategies",
            ),
            (
                "NIST-MAN-2",
                "Incident Response",
                "Establish AI incident response procedures",
            ),
        ];

        for (id, title, desc) in reqs {
            self.requirements.push(Requirement {
                req_id: id.to_string(),
                framework: Framework::NistAiRmf,
                title: title.to_string(),
                description: desc.to_string(),
                status: ComplianceStatus::NotMet,
                evidence: None,
            });
        }
    }

    fn populate_iso_42001(&mut self) {
        let reqs = vec![
            (
                "ISO-4.1",
                "Context of Organization",
                "Understand the organization and its context",
            ),
            (
                "ISO-5.1",
                "Leadership Commitment",
                "Demonstrate leadership commitment to AI management",
            ),
            (
                "ISO-6.1",
                "Risk Assessment",
                "Plan actions to address risks and opportunities",
            ),
            (
                "ISO-7.1",
                "Resource Management",
                "Determine and provide resources for the AI management system",
            ),
            (
                "ISO-8.1",
                "Operational Planning",
                "Plan, implement, and control AI processes",
            ),
            (
                "ISO-9.1",
                "Performance Evaluation",
                "Monitor, measure, analyze, and evaluate AI performance",
            ),
            (
                "ISO-10.1",
                "Continual Improvement",
                "Continually improve the AI management system",
            ),
        ];

        for (id, title, desc) in reqs {
            self.requirements.push(Requirement {
                req_id: id.to_string(),
                framework: Framework::Iso42001,
                title: title.to_string(),
                description: desc.to_string(),
                status: ComplianceStatus::NotMet,
                evidence: None,
            });
        }
    }

    /// Get all requirements for a given framework.
    pub fn get_requirements(&self, framework: &Framework) -> Vec<&Requirement> {
        self.requirements
            .iter()
            .filter(|r| &r.framework == framework)
            .collect()
    }

    /// Check the compliance status of a specific requirement.
    pub fn check_requirement(&self, req_id: &str) -> Option<ComplianceStatus> {
        self.requirements
            .iter()
            .find(|r| r.req_id == req_id)
            .map(|r| r.status.clone())
    }

    /// Update the status of a requirement, optionally with evidence.
    pub fn update_status(
        &mut self,
        req_id: &str,
        status: ComplianceStatus,
        evidence: Option<String>,
    ) -> bool {
        if let Some(req) = self.requirements.iter_mut().find(|r| r.req_id == req_id) {
            req.status = status;
            req.evidence = evidence;
            true
        } else {
            false
        }
    }

    /// Perform gap analysis for a framework — returns unmet or partially met requirements.
    pub fn gap_analysis(&self, framework: &Framework) -> Vec<ComplianceGap> {
        self.requirements
            .iter()
            .filter(|r| {
                &r.framework == framework
                    && matches!(
                        r.status,
                        ComplianceStatus::NotMet | ComplianceStatus::Partial
                    )
            })
            .map(|r| {
                let remediation = match r.status {
                    ComplianceStatus::NotMet => {
                        format!("Implement controls for: {}", r.title)
                    }
                    ComplianceStatus::Partial => {
                        format!("Complete implementation of: {}", r.title)
                    }
                    _ => String::new(),
                };
                ComplianceGap {
                    req_id: r.req_id.clone(),
                    framework: r.framework.clone(),
                    title: r.title.clone(),
                    status: r.status.clone(),
                    remediation,
                }
            })
            .collect()
    }

    /// Compute compliance score for a framework as percentage of Met requirements.
    ///
    /// NotApplicable requirements are excluded from the denominator.
    pub fn compute_score(&self, framework: &Framework) -> f64 {
        let applicable: Vec<&Requirement> = self
            .requirements
            .iter()
            .filter(|r| &r.framework == framework && r.status != ComplianceStatus::NotApplicable)
            .collect();

        if applicable.is_empty() {
            return 0.0;
        }

        let met = applicable
            .iter()
            .filter(|r| r.status == ComplianceStatus::Met)
            .count();

        met as f64 / applicable.len() as f64
    }

    /// Generate a comprehensive compliance report across multiple frameworks.
    pub fn generate_report(&self, frameworks: &[Framework]) -> ComplianceReport {
        let mut scores = HashMap::new();
        let mut total = 0usize;
        let mut met = 0usize;
        let mut partial = 0usize;
        let mut not_met = 0usize;
        let mut gaps = Vec::new();

        for fw in frameworks {
            let fw_key = format!("{fw:?}");
            scores.insert(fw_key, self.compute_score(fw));

            for req in self.requirements.iter().filter(|r| &r.framework == fw) {
                total += 1;
                match req.status {
                    ComplianceStatus::Met => met += 1,
                    ComplianceStatus::Partial => partial += 1,
                    ComplianceStatus::NotMet => not_met += 1,
                    ComplianceStatus::NotApplicable => {}
                }
            }

            gaps.extend(self.gap_analysis(fw));
        }

        ComplianceReport {
            report_id: Uuid::new_v4().to_string(),
            frameworks: frameworks.to_vec(),
            scores,
            total_requirements: total,
            met_count: met,
            partial_count: partial,
            not_met_count: not_met,
            gaps,
            generated_at: now_secs(),
        }
    }

    /// Total number of requirements tracked.
    pub fn requirement_count(&self) -> usize {
        self.requirements.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepopulated_requirements() {
        let cw = ComplianceCrosswalk::new();
        assert_eq!(cw.get_requirements(&Framework::EuAiAct).len(), 8);
        assert_eq!(cw.get_requirements(&Framework::NistAiRmf).len(), 8);
        assert_eq!(cw.get_requirements(&Framework::Iso42001).len(), 7);
        assert_eq!(cw.requirement_count(), 23);
    }

    #[test]
    fn test_check_requirement() {
        let cw = ComplianceCrosswalk::new();
        assert_eq!(
            cw.check_requirement("EU-AI-6"),
            Some(ComplianceStatus::NotMet)
        );
        assert_eq!(
            cw.check_requirement("NIST-GOV-1"),
            Some(ComplianceStatus::NotMet)
        );
        assert_eq!(
            cw.check_requirement("ISO-4.1"),
            Some(ComplianceStatus::NotMet)
        );
        assert_eq!(cw.check_requirement("NONEXISTENT"), None);
    }

    #[test]
    fn test_update_status() {
        let mut cw = ComplianceCrosswalk::new();
        let updated = cw.update_status(
            "EU-AI-6",
            ComplianceStatus::Met,
            Some("Risk assessment done".into()),
        );
        assert!(updated);
        assert_eq!(cw.check_requirement("EU-AI-6"), Some(ComplianceStatus::Met));

        let not_found = cw.update_status("FAKE-1", ComplianceStatus::Met, None);
        assert!(!not_found);
    }

    #[test]
    fn test_gap_analysis_all_unmet() {
        let cw = ComplianceCrosswalk::new();
        let gaps = cw.gap_analysis(&Framework::EuAiAct);
        assert_eq!(gaps.len(), 8); // All start as NotMet.
        assert!(gaps.iter().all(|g| g.status == ComplianceStatus::NotMet));
    }

    #[test]
    fn test_gap_analysis_some_met() {
        let mut cw = ComplianceCrosswalk::new();
        cw.update_status("EU-AI-6", ComplianceStatus::Met, None);
        cw.update_status("EU-AI-9", ComplianceStatus::Partial, None);
        cw.update_status("EU-AI-10", ComplianceStatus::NotApplicable, None);

        let gaps = cw.gap_analysis(&Framework::EuAiAct);
        // 8 total - 1 Met - 1 NotApplicable = 6 gaps (5 NotMet + 1 Partial).
        assert_eq!(gaps.len(), 6);
        assert!(gaps
            .iter()
            .any(|g| g.req_id == "EU-AI-9" && g.status == ComplianceStatus::Partial));
        assert!(!gaps.iter().any(|g| g.req_id == "EU-AI-6"));
    }

    #[test]
    fn test_compute_score_all_unmet() {
        let cw = ComplianceCrosswalk::new();
        let score = cw.compute_score(&Framework::EuAiAct);
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_all_met() {
        let mut cw = ComplianceCrosswalk::new();
        for req in cw
            .get_requirements(&Framework::Iso42001)
            .iter()
            .map(|r| r.req_id.clone())
            .collect::<Vec<_>>()
        {
            cw.update_status(&req, ComplianceStatus::Met, None);
        }
        let score = cw.compute_score(&Framework::Iso42001);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_excludes_not_applicable() {
        let mut cw = ComplianceCrosswalk::new();
        let ids: Vec<String> = cw
            .get_requirements(&Framework::Iso42001)
            .iter()
            .map(|r| r.req_id.clone())
            .collect();

        // Mark 3 as Met, 2 as NotApplicable, rest as NotMet.
        for (i, id) in ids.iter().enumerate() {
            if i < 3 {
                cw.update_status(id, ComplianceStatus::Met, None);
            } else if i < 5 {
                cw.update_status(id, ComplianceStatus::NotApplicable, None);
            }
        }

        // 3 Met out of 5 applicable (7 total - 2 N/A) = 0.6.
        let score = cw.compute_score(&Framework::Iso42001);
        assert!((score - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_generate_report_single_framework() {
        let cw = ComplianceCrosswalk::new();
        let report = cw.generate_report(&[Framework::EuAiAct]);

        assert_eq!(report.frameworks, vec![Framework::EuAiAct]);
        assert_eq!(report.total_requirements, 8);
        assert_eq!(report.not_met_count, 8);
        assert_eq!(report.met_count, 0);
        assert_eq!(report.gaps.len(), 8);
        assert!(!report.report_id.is_empty());
        assert!(report.scores.contains_key("EuAiAct"));
    }

    #[test]
    fn test_generate_report_multiple_frameworks() {
        let mut cw = ComplianceCrosswalk::new();
        cw.update_status("EU-AI-6", ComplianceStatus::Met, None);
        cw.update_status("NIST-GOV-1", ComplianceStatus::Met, None);

        let report = cw.generate_report(&[Framework::EuAiAct, Framework::NistAiRmf]);

        assert_eq!(report.frameworks.len(), 2);
        assert_eq!(report.total_requirements, 16); // 8 + 8.
        assert_eq!(report.met_count, 2);
        assert_eq!(report.not_met_count, 14);
        assert_eq!(report.gaps.len(), 14);
    }

    #[test]
    fn test_gap_remediation_messages() {
        let mut cw = ComplianceCrosswalk::new();
        cw.update_status("EU-AI-6", ComplianceStatus::Partial, None);

        let gaps = cw.gap_analysis(&Framework::EuAiAct);
        let partial_gap = gaps.iter().find(|g| g.req_id == "EU-AI-6").unwrap();
        assert!(partial_gap.remediation.contains("Complete implementation"));

        let not_met_gap = gaps.iter().find(|g| g.req_id == "EU-AI-9").unwrap();
        assert!(not_met_gap.remediation.contains("Implement controls"));
    }

    #[test]
    fn test_default_crosswalk() {
        let cw = ComplianceCrosswalk::default();
        assert_eq!(cw.requirement_count(), 23);
    }
}
