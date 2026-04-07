//! Proposer — generates concrete improvement proposals from analysis opportunities.
//!
//! One proposal per opportunity. Each specifies before/after values and is always reversible.

use crate::self_improve::analyzer::{AnalysisResult, ImprovementTarget, Opportunity};
use crate::self_improve::mod_types::SystemDefaults;
use serde::{Deserialize, Serialize};

// ─── Types ─────────────────────────────────────────────────────────────────

/// A concrete improvement proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub opportunity_id: String,
    pub target: ImprovementTarget,
    pub description: String,
    pub before_value: String,
    pub after_value: String,
    pub evidence_summary: String,
    pub confidence: f32,
    pub reversible: bool,
    pub auto_apply: bool,
    pub status: ProposalStatus,
}

/// Lifecycle status of a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Validated,
    ValidationFailed,
    Applied,
    Rejected,
    RolledBack,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Validated => write!(f, "Validated"),
            Self::ValidationFailed => write!(f, "ValidationFailed"),
            Self::Applied => write!(f, "Applied"),
            Self::Rejected => write!(f, "Rejected"),
            Self::RolledBack => write!(f, "RolledBack"),
        }
    }
}

// ─── Proposal Generation ───────────────────────────────────────────────────

/// Generate one proposal per opportunity, referencing current system defaults for before/after.
pub fn generate_proposals(
    analysis: &AnalysisResult,
    current_defaults: &SystemDefaults,
) -> Vec<Proposal> {
    let mut proposals = Vec::new();
    let mut prop_id = 1u32;

    for opp in &analysis.opportunities {
        if let Some(proposal) =
            proposal_for_opportunity(opp, current_defaults, prop_id, analysis.confidence)
        {
            proposals.push(proposal);
            prop_id += 1;
        }
    }

    proposals
}

fn proposal_for_opportunity(
    opp: &Opportunity,
    defaults: &SystemDefaults,
    prop_id: u32,
    confidence: f32,
) -> Option<Proposal> {
    match &opp.target {
        ImprovementTarget::DefaultPalette {
            template_id,
            palette_name,
        } => {
            let current = defaults
                .palette_rankings
                .get(template_id)
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_else(|| "(default)".into());
            Some(Proposal {
                id: format!("prop-{prop_id:04}"),
                opportunity_id: opp.id.clone(),
                target: opp.target.clone(),
                description: opp.description.clone(),
                before_value: current,
                after_value: palette_name.clone(),
                evidence_summary: format!(
                    "{}: {:.0}% (n={})",
                    opp.evidence.metric,
                    opp.evidence.value * 100.0,
                    opp.evidence.sample_size
                ),
                confidence,
                reversible: true,
                auto_apply: false,
                status: ProposalStatus::Pending,
            })
        }
        ImprovementTarget::DefaultTypography {
            template_id,
            typography_name,
        } => {
            let current = defaults
                .typography_rankings
                .get(template_id)
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_else(|| "modern".into());
            Some(Proposal {
                id: format!("prop-{prop_id:04}"),
                opportunity_id: opp.id.clone(),
                target: opp.target.clone(),
                description: opp.description.clone(),
                before_value: current,
                after_value: typography_name.clone(),
                evidence_summary: format!(
                    "{}: {:.0}% (n={})",
                    opp.evidence.metric,
                    opp.evidence.value * 100.0,
                    opp.evidence.sample_size
                ),
                confidence,
                reversible: true,
                auto_apply: false,
                status: ProposalStatus::Pending,
            })
        }
        ImprovementTarget::DefaultLayout {
            section_id,
            layout_name,
            ..
        } => {
            let current = defaults
                .layout_rankings
                .values()
                .find_map(|sections| sections.get(section_id).and_then(|v| v.first()).cloned())
                .unwrap_or_else(|| "(default)".into());
            Some(Proposal {
                id: format!("prop-{prop_id:04}"),
                opportunity_id: opp.id.clone(),
                target: opp.target.clone(),
                description: opp.description.clone(),
                before_value: current,
                after_value: layout_name.clone(),
                evidence_summary: format!(
                    "{}: {:.0}% (n={})",
                    opp.evidence.metric,
                    opp.evidence.value * 100.0,
                    opp.evidence.sample_size
                ),
                confidence,
                reversible: true,
                auto_apply: false,
                status: ProposalStatus::Pending,
            })
        }
        ImprovementTarget::ContentPromptHint { hint } => {
            let current_hints = defaults.content_prompt_hints.join("; ");
            let before = if current_hints.is_empty() {
                "(none)".into()
            } else {
                current_hints
            };
            Some(Proposal {
                id: format!("prop-{prop_id:04}"),
                opportunity_id: opp.id.clone(),
                target: opp.target.clone(),
                description: opp.description.clone(),
                before_value: before,
                after_value: hint.clone(),
                evidence_summary: format!(
                    "{}: {:.0}% (n={})",
                    opp.evidence.metric,
                    opp.evidence.value * 100.0,
                    opp.evidence.sample_size
                ),
                confidence,
                reversible: true,
                auto_apply: false,
                status: ProposalStatus::Pending,
            })
        }
        ImprovementTarget::SlotConstraint {
            slot_name: _,
            new_max_chars,
            ..
        } => Some(Proposal {
            id: format!("prop-{prop_id:04}"),
            opportunity_id: opp.id.clone(),
            target: opp.target.clone(),
            description: opp.description.clone(),
            before_value: "(current schema default)".into(),
            after_value: format!("max_chars={new_max_chars}"),
            evidence_summary: format!(
                "{}: {:.0}% (n={})",
                opp.evidence.metric,
                opp.evidence.value * 100.0,
                opp.evidence.sample_size
            ),
            confidence,
            reversible: true,
            auto_apply: false,
            status: ProposalStatus::Pending,
        }),
        ImprovementTarget::VariantDiversityWeight { factor, new_weight } => {
            let current = defaults
                .variant_diversity_weights
                .get(factor)
                .map(|w| format!("{w:.2}"))
                .unwrap_or_else(|| "1.00".into());
            Some(Proposal {
                id: format!("prop-{prop_id:04}"),
                opportunity_id: opp.id.clone(),
                target: opp.target.clone(),
                description: opp.description.clone(),
                before_value: current,
                after_value: format!("{new_weight:.2}"),
                evidence_summary: format!(
                    "{}: {:.0}% (n={})",
                    opp.evidence.metric,
                    opp.evidence.value * 100.0,
                    opp.evidence.sample_size
                ),
                confidence,
                reversible: true,
                auto_apply: false,
                status: ProposalStatus::Pending,
            })
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improve::analyzer::{Evidence, Impact, Opportunity};

    fn mock_analysis(opportunities: Vec<Opportunity>) -> AnalysisResult {
        AnalysisResult {
            sample_size: 10,
            confidence: 0.8,
            opportunities,
        }
    }

    fn palette_opportunity() -> Opportunity {
        Opportunity {
            id: "opp-0001".into(),
            target: ImprovementTarget::DefaultPalette {
                template_id: "saas_landing".into(),
                palette_name: "saas_midnight".into(),
            },
            description: "72% retention for midnight".into(),
            evidence: Evidence {
                metric: "palette_retention_rate".into(),
                value: 0.72,
                sample_size: 10,
                threshold: 0.60,
            },
            estimated_impact: Impact::High,
        }
    }

    #[test]
    fn test_generates_one_proposal_per_opportunity() {
        let analysis = mock_analysis(vec![
            palette_opportunity(),
            Opportunity {
                id: "opp-0002".into(),
                target: ImprovementTarget::ContentPromptHint {
                    hint: "Use numbers in headlines".into(),
                },
                description: "hint test".into(),
                evidence: Evidence {
                    metric: "section_edit_rate".into(),
                    value: 0.55,
                    sample_size: 10,
                    threshold: 0.50,
                },
                estimated_impact: Impact::Medium,
            },
            Opportunity {
                id: "opp-0003".into(),
                target: ImprovementTarget::DefaultTypography {
                    template_id: "saas_landing".into(),
                    typography_name: "editorial".into(),
                },
                description: "typo test".into(),
                evidence: Evidence {
                    metric: "typography_retention_rate".into(),
                    value: 0.65,
                    sample_size: 10,
                    threshold: 0.60,
                },
                estimated_impact: Impact::Medium,
            },
        ]);
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        assert_eq!(proposals.len(), 3);
    }

    #[test]
    fn test_proposal_has_before_after() {
        let analysis = mock_analysis(vec![palette_opportunity()]);
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        assert_eq!(proposals.len(), 1);
        let p = &proposals[0];
        assert!(!p.before_value.is_empty());
        assert!(!p.after_value.is_empty());
    }

    #[test]
    fn test_proposal_always_reversible() {
        let analysis = mock_analysis(vec![palette_opportunity()]);
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        for p in &proposals {
            assert!(p.reversible, "proposal {} should be reversible", p.id);
        }
    }

    #[test]
    fn test_proposal_auto_apply_off_by_default() {
        let analysis = mock_analysis(vec![palette_opportunity()]);
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        for p in &proposals {
            assert!(!p.auto_apply, "proposal {} should not auto-apply", p.id);
        }
    }
}
