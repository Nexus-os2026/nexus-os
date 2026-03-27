use serde::{Deserialize, Serialize};

use crate::artifacts::{ArtifactContent, ProjectArtifact};
use crate::pipeline::PipelineStage;

pub struct QualityGate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResult {
    pub stage: PipelineStage,
    pub passed: bool,
    pub score: f64,
    pub checks: Vec<GateCheck>,
    pub blocking_issues: Vec<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheck {
    pub name: String,
    pub passed: bool,
    pub details: String,
}

impl QualityGate {
    pub fn evaluate(artifact: &ProjectArtifact) -> QualityGateResult {
        let checks = match &artifact.content {
            ArtifactContent::Requirements {
                user_stories,
                acceptance_criteria,
                constraints,
                ..
            } => vec![
                GateCheck {
                    name: "Has user stories".into(),
                    passed: !user_stories.is_empty(),
                    details: format!("{} user stories", user_stories.len()),
                },
                GateCheck {
                    name: "Has acceptance criteria".into(),
                    passed: !acceptance_criteria.is_empty(),
                    details: format!("{} criteria", acceptance_criteria.len()),
                },
                GateCheck {
                    name: "Has constraints".into(),
                    passed: !constraints.is_empty(),
                    details: format!("{} constraints", constraints.len()),
                },
            ],
            ArtifactContent::Architecture {
                components,
                risks,
                technology_choices,
                ..
            } => vec![
                GateCheck {
                    name: "Has components".into(),
                    passed: !components.is_empty(),
                    details: format!("{} components", components.len()),
                },
                GateCheck {
                    name: "Risks identified".into(),
                    passed: !risks.is_empty(),
                    details: format!("{} risks", risks.len()),
                },
                GateCheck {
                    name: "Technology choices documented".into(),
                    passed: !technology_choices.is_empty(),
                    details: format!("{} decisions", technology_choices.len()),
                },
            ],
            ArtifactContent::SourceCode { files, .. } => {
                let total_lines: u32 = files.iter().map(|f| f.lines).sum();
                vec![
                    GateCheck {
                        name: "Has source files".into(),
                        passed: !files.is_empty(),
                        details: format!("{} files", files.len()),
                    },
                    GateCheck {
                        name: "Non-trivial code".into(),
                        passed: total_lines >= 10,
                        details: format!("{} total lines", total_lines),
                    },
                ]
            }
            ArtifactContent::TestResults {
                total,
                passed,
                failed,
                coverage_percent,
                ..
            } => vec![
                GateCheck {
                    name: "Tests exist".into(),
                    passed: *total > 0,
                    details: format!("{} tests", total),
                },
                GateCheck {
                    name: "All tests pass".into(),
                    passed: *failed == 0,
                    details: format!("{}/{} passed", passed, total),
                },
                GateCheck {
                    name: "Coverage threshold".into(),
                    passed: coverage_percent.unwrap_or(0.0) >= 60.0,
                    details: format!("{:.1}% coverage", coverage_percent.unwrap_or(0.0)),
                },
            ],
            ArtifactContent::ReviewOutcome {
                approved,
                requested_changes,
                ..
            } => vec![GateCheck {
                name: "Review approved".into(),
                passed: *approved,
                details: if *approved {
                    "Approved".into()
                } else {
                    format!("{} changes requested", requested_changes.len())
                },
            }],
            ArtifactContent::DeploymentResult { success, .. } => vec![GateCheck {
                name: "Deployment successful".into(),
                passed: *success,
                details: if *success {
                    "Deployed".into()
                } else {
                    "Failed".into()
                },
            }],
            ArtifactContent::VerificationReport {
                all_checks_passed,
                checks,
            } => vec![GateCheck {
                name: "All verifications pass".into(),
                passed: *all_checks_passed,
                details: format!(
                    "{}/{} checks passed",
                    checks.iter().filter(|c| c.passed).count(),
                    checks.len()
                ),
            }],
            ArtifactContent::Text(t) => vec![GateCheck {
                name: "Content exists".into(),
                passed: !t.is_empty(),
                details: format!("{} chars", t.len()),
            }],
        };

        let passed_count = checks.iter().filter(|c| c.passed).count();
        let total = checks.len();
        let score = if total > 0 {
            passed_count as f64 / total as f64
        } else {
            0.0
        };
        let all_passed = checks.iter().all(|c| c.passed);
        let blocking = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| format!("{}: {}", c.name, c.details))
            .collect();

        QualityGateResult {
            stage: artifact.stage,
            passed: all_passed,
            score,
            checks,
            blocking_issues: blocking,
            timestamp: epoch_now(),
        }
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::*;

    fn make_req_artifact(
        stories: &[&str],
        criteria: &[&str],
        constraints: &[&str],
    ) -> ProjectArtifact {
        ProjectArtifact {
            id: "a1".into(),
            project_id: "p1".into(),
            artifact_type: "requirements_spec".into(),
            stage: PipelineStage::Requirements,
            produced_by: "pm-1".into(),
            content: ArtifactContent::Requirements {
                user_stories: stories.iter().map(|s| s.to_string()).collect(),
                acceptance_criteria: criteria.iter().map(|s| s.to_string()).collect(),
                constraints: constraints.iter().map(|s| s.to_string()).collect(),
                priorities: Vec::new(),
            },
            quality_score: None,
            created_at: 0,
            version: 1,
        }
    }

    #[test]
    fn test_artifact_quality_gate_pass() {
        let artifact = make_req_artifact(
            &["As user I want X"],
            &["Given A when B then C"],
            &["Max 100ms"],
        );
        let result = QualityGate::evaluate(&artifact);
        assert!(result.passed);
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_artifact_quality_gate_fail() {
        let artifact = make_req_artifact(&[], &[], &[]);
        let result = QualityGate::evaluate(&artifact);
        assert!(!result.passed);
        assert!(!result.blocking_issues.is_empty());
    }
}
