//! Analyzer — identifies patterns and improvement opportunities from aggregate metrics.
//!
//! All analysis is deterministic (no LLM). Threshold-based rules produce
//! `Opportunity` values that the Proposer converts into concrete proposals.

use crate::self_improve::metrics::AggregateMetrics;
use serde::{Deserialize, Serialize};

// ─── Types ─────────────────────────────────────────────────────────────────

/// Result of running the analyzer on aggregate metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub opportunities: Vec<Opportunity>,
    pub sample_size: usize,
    pub confidence: f32,
}

/// A single improvement opportunity identified by the analyzer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: String,
    pub target: ImprovementTarget,
    pub description: String,
    pub evidence: Evidence,
    pub estimated_impact: Impact,
}

/// What the improvement would change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImprovementTarget {
    DefaultPalette {
        template_id: String,
        palette_name: String,
    },
    DefaultTypography {
        template_id: String,
        typography_name: String,
    },
    DefaultLayout {
        template_id: String,
        section_id: String,
        layout_name: String,
    },
    ContentPromptHint {
        hint: String,
    },
    SlotConstraint {
        template_id: String,
        section_id: String,
        slot_name: String,
        new_max_chars: usize,
    },
    VariantDiversityWeight {
        factor: String,
        new_weight: f32,
    },
}

/// Evidence supporting an opportunity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub metric: String,
    pub value: f64,
    pub sample_size: usize,
    pub threshold: f64,
}

/// Estimated impact level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Impact {
    High,
    Medium,
    Low,
}

// ─── Analysis ──────────────────────────────────────────────────────────────

/// Default minimum sample size before any opportunities are proposed.
pub const DEFAULT_MIN_SAMPLE_SIZE: usize = 5;

/// Palette retention threshold (60%).
const PALETTE_RETENTION_THRESHOLD: f64 = 0.60;

/// Typography retention threshold (60%).
const TYPOGRAPHY_RETENTION_THRESHOLD: f64 = 0.60;

/// Section edit rate threshold — above this means the section default needs improvement (50%).
const SECTION_EDIT_THRESHOLD: f64 = 0.50;

/// Layout selection rate threshold (60%).
const LAYOUT_SELECTION_THRESHOLD: f64 = 0.60;

/// Run deterministic analysis on aggregate metrics.
///
/// Returns empty opportunities if sample size < `min_sample_size`.
pub fn analyze(metrics: &AggregateMetrics, min_sample_size: usize) -> AnalysisResult {
    if metrics.total_projects < min_sample_size {
        return AnalysisResult {
            opportunities: vec![],
            sample_size: metrics.total_projects,
            confidence: 0.0,
        };
    }

    let mut opportunities = Vec::new();
    let mut opp_id = 1u32;

    // ── Palette preference ─────────────────────────────────────────────
    for (template_id, palette_rates) in &metrics.palette_retention {
        let template_count = metrics
            .projects_per_template
            .get(template_id)
            .copied()
            .unwrap_or(0);
        if template_count < min_sample_size {
            continue;
        }
        // Find highest-retained palette
        if let Some((best_palette, &best_rate)) = palette_rates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            if best_rate >= PALETTE_RETENTION_THRESHOLD {
                opportunities.push(Opportunity {
                    id: format!("opp-{opp_id:04}"),
                    target: ImprovementTarget::DefaultPalette {
                        template_id: template_id.clone(),
                        palette_name: best_palette.clone(),
                    },
                    description: format!(
                        "{template_id} users prefer '{best_palette}' palette ({:.0}% retention)",
                        best_rate * 100.0
                    ),
                    evidence: Evidence {
                        metric: "palette_retention_rate".into(),
                        value: best_rate,
                        sample_size: template_count,
                        threshold: PALETTE_RETENTION_THRESHOLD,
                    },
                    estimated_impact: Impact::High,
                });
                opp_id += 1;
            }
        }
    }

    // ── Typography preference ──────────────────────────────────────────
    for (template_id, typo_rates) in &metrics.typography_retention {
        let template_count = metrics
            .projects_per_template
            .get(template_id)
            .copied()
            .unwrap_or(0);
        if template_count < min_sample_size {
            continue;
        }
        if let Some((best_typo, &best_rate)) = typo_rates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            if best_rate >= TYPOGRAPHY_RETENTION_THRESHOLD {
                opportunities.push(Opportunity {
                    id: format!("opp-{opp_id:04}"),
                    target: ImprovementTarget::DefaultTypography {
                        template_id: template_id.clone(),
                        typography_name: best_typo.clone(),
                    },
                    description: format!(
                        "{template_id} users prefer '{best_typo}' typography ({:.0}% retention)",
                        best_rate * 100.0
                    ),
                    evidence: Evidence {
                        metric: "typography_retention_rate".into(),
                        value: best_rate,
                        sample_size: template_count,
                        threshold: TYPOGRAPHY_RETENTION_THRESHOLD,
                    },
                    estimated_impact: Impact::Medium,
                });
                opp_id += 1;
            }
        }
    }

    // ── Section edit frequency (high edits = bad default) ──────────────
    for (section_id, &rate) in &metrics.section_edit_rate {
        if rate >= SECTION_EDIT_THRESHOLD {
            opportunities.push(Opportunity {
                id: format!("opp-{opp_id:04}"),
                target: ImprovementTarget::ContentPromptHint {
                    hint: format!("Improve default content for '{section_id}' section — users edit it {:.0}% of the time", rate * 100.0),
                },
                description: format!(
                    "'{section_id}' section is edited in {:.0}% of projects — default may need improvement",
                    rate * 100.0
                ),
                evidence: Evidence {
                    metric: "section_edit_rate".into(),
                    value: rate,
                    sample_size: metrics.total_projects,
                    threshold: SECTION_EDIT_THRESHOLD,
                },
                estimated_impact: Impact::Medium,
            });
            opp_id += 1;
        }
    }

    // ── Layout variant preference ──────────────────────────────────────
    for (section_id, layout_rates) in &metrics.layout_selection_rate {
        if let Some((best_layout, &best_rate)) = layout_rates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            if best_rate >= LAYOUT_SELECTION_THRESHOLD {
                opportunities.push(Opportunity {
                    id: format!("opp-{opp_id:04}"),
                    target: ImprovementTarget::DefaultLayout {
                        template_id: String::new(), // section-level, template determined at proposal time
                        section_id: section_id.clone(),
                        layout_name: best_layout.clone(),
                    },
                    description: format!(
                        "'{best_layout}' is selected {:.0}% of the time for '{section_id}'",
                        best_rate * 100.0
                    ),
                    evidence: Evidence {
                        metric: "layout_selection_rate".into(),
                        value: best_rate,
                        sample_size: metrics.total_projects,
                        threshold: LAYOUT_SELECTION_THRESHOLD,
                    },
                    estimated_impact: Impact::Medium,
                });
                opp_id += 1;
            }
        }
    }

    let _ = opp_id;

    // Confidence scales with sample size (caps at 1.0 around 20 projects)
    let confidence = (metrics.total_projects as f32 / 20.0).min(1.0);

    AnalysisResult {
        opportunities,
        sample_size: metrics.total_projects,
        confidence,
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improve::metrics::AggregateMetrics;
    use std::collections::HashMap;

    fn base_metrics(total: usize) -> AggregateMetrics {
        let mut agg = AggregateMetrics::default();
        agg.total_projects = total;
        agg.projects_per_template
            .insert("saas_landing".into(), total);
        agg
    }

    #[test]
    fn test_min_sample_size_respected() {
        let agg = base_metrics(3);
        let result = analyze(&agg, 5);
        assert!(result.opportunities.is_empty());
        assert_eq!(result.sample_size, 3);
    }

    #[test]
    fn test_identifies_palette_preference() {
        let mut agg = base_metrics(10);
        let mut palette_rates = HashMap::new();
        palette_rates.insert("saas_midnight".into(), 0.72);
        palette_rates.insert("saas_ocean".into(), 0.15);
        agg.palette_retention
            .insert("saas_landing".into(), palette_rates);

        let result = analyze(&agg, 5);
        assert!(!result.opportunities.is_empty());
        let palette_opp = result
            .opportunities
            .iter()
            .find(|o| matches!(&o.target, ImprovementTarget::DefaultPalette { .. }));
        assert!(palette_opp.is_some());
        let opp = palette_opp.unwrap();
        assert!(opp.description.contains("72%"));
        assert_eq!(opp.estimated_impact, Impact::High);
    }

    #[test]
    fn test_identifies_high_edit_section() {
        let mut agg = base_metrics(10);
        agg.section_edit_rate.insert("hero".into(), 0.65);

        let result = analyze(&agg, 5);
        let section_opp = result
            .opportunities
            .iter()
            .find(|o| matches!(&o.target, ImprovementTarget::ContentPromptHint { .. }));
        assert!(section_opp.is_some());
        assert!(section_opp.unwrap().description.contains("hero"));
    }

    #[test]
    fn test_confidence_scales_with_sample() {
        let r5 = analyze(&base_metrics(5), 5);
        let r20 = analyze(&base_metrics(20), 5);
        assert!(r5.confidence < r20.confidence);
        assert!((r20.confidence - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_no_opportunities_when_data_uniform() {
        let mut agg = base_metrics(10);
        // All palettes equally used — no clear winner
        let mut palette_rates = HashMap::new();
        palette_rates.insert("saas_midnight".into(), 0.25);
        palette_rates.insert("saas_ocean".into(), 0.25);
        palette_rates.insert("saas_neon".into(), 0.25);
        palette_rates.insert("saas_nature".into(), 0.25);
        agg.palette_retention
            .insert("saas_landing".into(), palette_rates);

        let result = analyze(&agg, 5);
        let palette_opps: Vec<_> = result
            .opportunities
            .iter()
            .filter(|o| matches!(&o.target, ImprovementTarget::DefaultPalette { .. }))
            .collect();
        assert!(
            palette_opps.is_empty(),
            "uniform data should produce no palette preference"
        );
    }

    #[test]
    fn test_identifies_layout_preference() {
        let mut agg = base_metrics(10);
        let mut layout_rates = HashMap::new();
        layout_rates.insert("split_image".into(), 0.70);
        layout_rates.insert("centered".into(), 0.30);
        agg.layout_selection_rate
            .insert("hero".into(), layout_rates);

        let result = analyze(&agg, 5);
        let layout_opp = result
            .opportunities
            .iter()
            .find(|o| matches!(&o.target, ImprovementTarget::DefaultLayout { .. }));
        assert!(layout_opp.is_some());
    }
}
