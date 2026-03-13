//! AI fairness auditing — impact assessments, disparity computation, and bias detection.
//!
//! Provides tools to assess the fairness of agent actions across different
//! population groups, compute statistical parity differences, and flag
//! potential biases before they cause harm.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Types ───────────────────────────────────────────────────────────────

/// Risk level for a fairness impact assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FairnessRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Context for a fairness assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairnessContext {
    /// Population groups potentially affected.
    pub affected_groups: Vec<String>,
    /// Domain of the action (e.g., "hiring", "lending", "content_moderation").
    pub domain: String,
    /// Whether the action involves automated decision-making.
    pub automated_decision: bool,
    /// Whether the outcome affects resource allocation.
    pub resource_allocation: bool,
}

/// Result of a fairness impact assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub assessment_id: String,
    pub action: String,
    pub affected_groups: Vec<String>,
    pub risk_level: FairnessRiskLevel,
    pub mitigation_steps: Vec<String>,
    pub disparity_score: Option<f64>,
    pub assessed_at: u64,
}

/// Outcome data for a specific group, used for disparity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupOutcome {
    pub group_name: String,
    /// Positive outcome rate for this group (0.0 to 1.0).
    pub positive_rate: f64,
    /// Sample size for this group.
    pub sample_size: u64,
}

/// Alert raised when bias is detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiasAlert {
    pub alert_id: String,
    pub disparity: f64,
    pub threshold: f64,
    pub advantaged_group: String,
    pub disadvantaged_group: String,
    pub recommendation: String,
    pub raised_at: u64,
}

/// Summary statistics for fairness assessments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairnessSummary {
    pub total_assessments: usize,
    pub by_risk_level: Vec<(FairnessRiskLevel, usize)>,
    pub total_bias_alerts: usize,
    pub average_disparity: Option<f64>,
}

// ── Engine ──────────────────────────────────────────────────────────────

/// Fairness auditing engine.
pub struct FairnessEngine {
    assessments: Vec<ImpactAssessment>,
    alerts: Vec<BiasAlert>,
    /// Default disparity threshold for bias detection.
    default_threshold: f64,
}

impl Default for FairnessEngine {
    fn default() -> Self {
        Self::new(0.1)
    }
}

impl FairnessEngine {
    /// Create a new engine with the given default disparity threshold.
    pub fn new(default_threshold: f64) -> Self {
        Self {
            assessments: Vec::new(),
            alerts: Vec::new(),
            default_threshold,
        }
    }

    /// Assess the fairness impact of an action given its context.
    pub fn assess_impact(&mut self, action: &str, context: &FairnessContext) -> ImpactAssessment {
        let risk_level = Self::determine_risk_level(context);
        let mitigation_steps = Self::suggest_mitigations(&risk_level, context);

        let assessment = ImpactAssessment {
            assessment_id: Uuid::new_v4().to_string(),
            action: action.to_string(),
            affected_groups: context.affected_groups.clone(),
            risk_level,
            mitigation_steps,
            disparity_score: None,
            assessed_at: now_secs(),
        };

        self.assessments.push(assessment.clone());
        assessment
    }

    /// Determine risk level based on context factors.
    fn determine_risk_level(context: &FairnessContext) -> FairnessRiskLevel {
        let mut risk_score = 0u32;

        // More affected groups → higher risk.
        if context.affected_groups.len() > 3 {
            risk_score += 2;
        } else if !context.affected_groups.is_empty() {
            risk_score += 1;
        }

        // Automated decisions are riskier.
        if context.automated_decision {
            risk_score += 2;
        }

        // Resource allocation increases risk.
        if context.resource_allocation {
            risk_score += 2;
        }

        // High-risk domains.
        let high_risk_domains = ["hiring", "lending", "criminal_justice", "healthcare"];
        if high_risk_domains.contains(&context.domain.as_str()) {
            risk_score += 2;
        }

        match risk_score {
            0..=1 => FairnessRiskLevel::Low,
            2..=3 => FairnessRiskLevel::Medium,
            4..=5 => FairnessRiskLevel::High,
            _ => FairnessRiskLevel::Critical,
        }
    }

    /// Suggest mitigation steps based on risk level and context.
    fn suggest_mitigations(
        risk_level: &FairnessRiskLevel,
        context: &FairnessContext,
    ) -> Vec<String> {
        let mut steps = Vec::new();

        match risk_level {
            FairnessRiskLevel::Low => {
                steps.push("Monitor outcomes for potential disparities".to_string());
            }
            FairnessRiskLevel::Medium => {
                steps.push("Conduct periodic disparity analysis".to_string());
                steps.push("Document decision criteria".to_string());
            }
            FairnessRiskLevel::High => {
                steps.push("Implement regular bias audits".to_string());
                steps.push("Require human review for edge cases".to_string());
                steps.push("Establish appeal mechanisms".to_string());
            }
            FairnessRiskLevel::Critical => {
                steps.push("Mandatory human-in-the-loop for all decisions".to_string());
                steps.push("Independent third-party audit required".to_string());
                steps.push("Continuous monitoring with automated alerts".to_string());
                steps.push("Establish transparent appeal process".to_string());
            }
        }

        if context.automated_decision {
            steps.push("Ensure explainability of automated decisions".to_string());
        }

        if context.resource_allocation {
            steps.push("Verify equitable resource distribution".to_string());
        }

        steps
    }

    /// Compute statistical parity difference across groups.
    ///
    /// Returns the maximum difference between any two groups' positive outcome rates.
    /// A value of 0.0 indicates perfect parity; higher values indicate more disparity.
    pub fn compute_disparity(outcomes: &[GroupOutcome]) -> f64 {
        if outcomes.len() < 2 {
            return 0.0;
        }

        let mut max_rate = f64::MIN;
        let mut min_rate = f64::MAX;

        for outcome in outcomes {
            if outcome.positive_rate > max_rate {
                max_rate = outcome.positive_rate;
            }
            if outcome.positive_rate < min_rate {
                min_rate = outcome.positive_rate;
            }
        }

        (max_rate - min_rate).abs()
    }

    /// Flag bias if the disparity exceeds the given threshold.
    ///
    /// Returns a `BiasAlert` if the disparity is above the threshold, `None` otherwise.
    pub fn flag_bias(
        &mut self,
        outcomes: &[GroupOutcome],
        threshold: Option<f64>,
    ) -> Option<BiasAlert> {
        let threshold = threshold.unwrap_or(self.default_threshold);
        let disparity = Self::compute_disparity(outcomes);

        if disparity <= threshold || outcomes.len() < 2 {
            return None;
        }

        // Find advantaged and disadvantaged groups.
        let advantaged = outcomes
            .iter()
            .max_by(|a, b| {
                a.positive_rate
                    .partial_cmp(&b.positive_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|g| g.group_name.clone())
            .unwrap_or_default();

        let disadvantaged = outcomes
            .iter()
            .min_by(|a, b| {
                a.positive_rate
                    .partial_cmp(&b.positive_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|g| g.group_name.clone())
            .unwrap_or_default();

        let alert = BiasAlert {
            alert_id: Uuid::new_v4().to_string(),
            disparity,
            threshold,
            advantaged_group: advantaged,
            disadvantaged_group: disadvantaged,
            recommendation: format!(
                "Disparity of {disparity:.3} exceeds threshold {threshold:.3}. \
                 Investigate causes and implement corrective measures."
            ),
            raised_at: now_secs(),
        };

        self.alerts.push(alert.clone());
        Some(alert)
    }

    /// Get all assessment history.
    pub fn get_assessment_history(&self) -> &[ImpactAssessment] {
        &self.assessments
    }

    /// Get all bias alerts.
    pub fn get_alerts(&self) -> &[BiasAlert] {
        &self.alerts
    }

    /// Get summary statistics.
    pub fn summary(&self) -> FairnessSummary {
        let total_assessments = self.assessments.len();

        let count_by_level = |level: &FairnessRiskLevel| -> usize {
            self.assessments
                .iter()
                .filter(|a| &a.risk_level == level)
                .count()
        };

        let by_risk_level = vec![
            (
                FairnessRiskLevel::Low,
                count_by_level(&FairnessRiskLevel::Low),
            ),
            (
                FairnessRiskLevel::Medium,
                count_by_level(&FairnessRiskLevel::Medium),
            ),
            (
                FairnessRiskLevel::High,
                count_by_level(&FairnessRiskLevel::High),
            ),
            (
                FairnessRiskLevel::Critical,
                count_by_level(&FairnessRiskLevel::Critical),
            ),
        ];

        let disparities: Vec<f64> = self
            .assessments
            .iter()
            .filter_map(|a| a.disparity_score)
            .collect();

        let average_disparity = if disparities.is_empty() {
            None
        } else {
            Some(disparities.iter().sum::<f64>() / disparities.len() as f64)
        };

        FairnessSummary {
            total_assessments,
            by_risk_level,
            total_bias_alerts: self.alerts.len(),
            average_disparity,
        }
    }

    /// Total number of assessments performed.
    pub fn assessment_count(&self) -> usize {
        self.assessments.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hiring_context() -> FairnessContext {
        FairnessContext {
            affected_groups: vec!["group_a".into(), "group_b".into()],
            domain: "hiring".to_string(),
            automated_decision: true,
            resource_allocation: true,
        }
    }

    fn low_risk_context() -> FairnessContext {
        FairnessContext {
            affected_groups: vec![],
            domain: "content_display".to_string(),
            automated_decision: false,
            resource_allocation: false,
        }
    }

    #[test]
    fn test_assess_impact_high_risk() {
        let mut engine = FairnessEngine::new(0.1);
        let assessment = engine.assess_impact("screen_candidates", &hiring_context());

        assert_eq!(assessment.action, "screen_candidates");
        assert!(matches!(
            assessment.risk_level,
            FairnessRiskLevel::High | FairnessRiskLevel::Critical
        ));
        assert!(!assessment.mitigation_steps.is_empty());
        assert!(!assessment.assessment_id.is_empty());
    }

    #[test]
    fn test_assess_impact_low_risk() {
        let mut engine = FairnessEngine::new(0.1);
        let assessment = engine.assess_impact("display_page", &low_risk_context());

        assert_eq!(assessment.risk_level, FairnessRiskLevel::Low);
    }

    #[test]
    fn test_compute_disparity_equal() {
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.8,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.8,
                sample_size: 100,
            },
        ];
        let disparity = FairnessEngine::compute_disparity(&outcomes);
        assert!(disparity.abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_disparity_unequal() {
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.9,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.6,
                sample_size: 100,
            },
        ];
        let disparity = FairnessEngine::compute_disparity(&outcomes);
        assert!((disparity - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_disparity_single_group() {
        let outcomes = vec![GroupOutcome {
            group_name: "A".into(),
            positive_rate: 0.8,
            sample_size: 100,
        }];
        assert!(FairnessEngine::compute_disparity(&outcomes).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_disparity_empty() {
        assert!(FairnessEngine::compute_disparity(&[]).abs() < f64::EPSILON);
    }

    #[test]
    fn test_flag_bias_above_threshold() {
        let mut engine = FairnessEngine::new(0.1);
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.9,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.5,
                sample_size: 100,
            },
        ];

        let alert = engine.flag_bias(&outcomes, None);
        assert!(alert.is_some());

        let alert = alert.unwrap();
        assert!((alert.disparity - 0.4).abs() < f64::EPSILON);
        assert_eq!(alert.advantaged_group, "A");
        assert_eq!(alert.disadvantaged_group, "B");
        assert!(!alert.recommendation.is_empty());
    }

    #[test]
    fn test_flag_bias_below_threshold() {
        let mut engine = FairnessEngine::new(0.1);
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.81,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.79,
                sample_size: 100,
            },
        ];

        let alert = engine.flag_bias(&outcomes, None);
        assert!(alert.is_none());
    }

    #[test]
    fn test_flag_bias_custom_threshold() {
        let mut engine = FairnessEngine::new(0.1);
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.85,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.80,
                sample_size: 100,
            },
        ];

        // Below default 0.1 → no alert.
        assert!(engine.flag_bias(&outcomes, None).is_none());
        // With lower threshold → alert.
        assert!(engine.flag_bias(&outcomes, Some(0.01)).is_some());
    }

    #[test]
    fn test_flag_bias_multiple_groups() {
        let mut engine = FairnessEngine::new(0.1);
        let outcomes = vec![
            GroupOutcome {
                group_name: "A".into(),
                positive_rate: 0.95,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "B".into(),
                positive_rate: 0.70,
                sample_size: 100,
            },
            GroupOutcome {
                group_name: "C".into(),
                positive_rate: 0.50,
                sample_size: 100,
            },
        ];

        let alert = engine.flag_bias(&outcomes, None).unwrap();
        assert_eq!(alert.advantaged_group, "A");
        assert_eq!(alert.disadvantaged_group, "C");
        assert!((alert.disparity - 0.45).abs() < f64::EPSILON);
    }

    #[test]
    fn test_assessment_history() {
        let mut engine = FairnessEngine::new(0.1);
        engine.assess_impact("action_1", &low_risk_context());
        engine.assess_impact("action_2", &hiring_context());

        assert_eq!(engine.assessment_count(), 2);
        assert_eq!(engine.get_assessment_history().len(), 2);
        assert_eq!(engine.get_assessment_history()[0].action, "action_1");
    }

    #[test]
    fn test_summary() {
        let mut engine = FairnessEngine::new(0.1);
        engine.assess_impact("a1", &low_risk_context());
        engine.assess_impact("a2", &hiring_context());

        let outcomes = vec![
            GroupOutcome {
                group_name: "X".into(),
                positive_rate: 0.9,
                sample_size: 50,
            },
            GroupOutcome {
                group_name: "Y".into(),
                positive_rate: 0.3,
                sample_size: 50,
            },
        ];
        engine.flag_bias(&outcomes, None);

        let summary = engine.summary();
        assert_eq!(summary.total_assessments, 2);
        assert_eq!(summary.total_bias_alerts, 1);
    }

    #[test]
    fn test_mitigation_includes_explainability() {
        let mut engine = FairnessEngine::new(0.1);
        let ctx = FairnessContext {
            affected_groups: vec!["g1".into()],
            domain: "general".to_string(),
            automated_decision: true,
            resource_allocation: false,
        };
        let assessment = engine.assess_impact("auto_decide", &ctx);
        assert!(assessment
            .mitigation_steps
            .iter()
            .any(|s| s.contains("explainability")));
    }

    #[test]
    fn test_default_engine() {
        let engine = FairnessEngine::default();
        assert_eq!(engine.assessment_count(), 0);
        assert!((engine.default_threshold - 0.1).abs() < f64::EPSILON);
    }
}
