//! Metric types and aggregation for the self-improvement loop.
//!
//! `ProjectMetrics` captures what happened in a single completed project.
//! `AggregateMetrics` summarises patterns across many projects.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Per-Project Metrics ───────────────────────────────────────────────────

/// Metrics collected from a single completed project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub project_id: String,
    pub template_id: String,
    /// Which palette/typography/layout/motion the project used.
    pub palette_id: String,
    pub typography_id: String,
    pub layout_selections: HashMap<String, String>,
    /// Did the user change the default palette after generation?
    pub palette_was_changed: bool,
    /// Did the user change the default typography after generation?
    pub typography_was_changed: bool,
    /// Sections that were visually edited (section_id → edit count).
    pub section_edits: HashMap<String, u32>,
    /// Foundation tokens that were manually changed.
    pub tokens_changed: Vec<String>,
    /// Quality score from the quality critic (0-100).
    pub quality_score: u32,
    /// Conversion score from the conversion critic (0-100).
    pub conversion_score: u32,
    /// How many build iterations before the user was satisfied.
    pub iteration_count: u32,
    /// Seconds from first build to deploy/export (if available).
    pub time_to_deploy_seconds: Option<u64>,
    /// If variants were generated, which variant index was picked.
    pub variant_selected_index: Option<usize>,
    /// Number of auto-fixes the user accepted.
    pub auto_fixes_applied: usize,
    /// Total build cost in USD.
    pub build_cost: f64,
    /// ISO-8601 timestamp of project completion.
    pub completed_at: String,
}

// ─── Section Edit Detail ───────────────────────────────────────────────────

/// Detailed edit info for a single section in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionEdit {
    pub section_id: String,
    pub edit_count: u32,
    pub tokens_modified: Vec<String>,
}

// ─── Aggregate Metrics ─────────────────────────────────────────────────────

/// Aggregated metrics across all completed projects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateMetrics {
    /// Total completed projects analysed.
    pub total_projects: usize,
    /// Per-template project count.
    pub projects_per_template: HashMap<String, usize>,
    /// Per-template palette retention: template_id → (palette_id → retention_rate 0.0-1.0).
    pub palette_retention: HashMap<String, HashMap<String, f64>>,
    /// Per-template typography retention: template_id → (typo_id → retention_rate).
    pub typography_retention: HashMap<String, HashMap<String, f64>>,
    /// Section edit frequency: section_id → fraction of projects that edited it.
    pub section_edit_rate: HashMap<String, f64>,
    /// Average quality score per template.
    pub avg_quality_by_template: HashMap<String, f64>,
    /// Average conversion score per template.
    pub avg_conversion_by_template: HashMap<String, f64>,
    /// Average iteration count per template.
    pub avg_iterations_by_template: HashMap<String, f64>,
    /// Layout selection frequency: section_id → (layout_id → selection_rate).
    pub layout_selection_rate: HashMap<String, HashMap<String, f64>>,
}

/// Compute aggregate metrics from a slice of project metrics.
pub fn aggregate_metrics(all: &[ProjectMetrics]) -> AggregateMetrics {
    if all.is_empty() {
        return AggregateMetrics::default();
    }

    let total = all.len();
    let mut agg = AggregateMetrics {
        total_projects: total,
        ..Default::default()
    };

    // ── Per-template counts ────────────────────────────────────────────────
    for m in all {
        *agg.projects_per_template
            .entry(m.template_id.clone())
            .or_insert(0) += 1;
    }

    // ── Palette retention (per template) ──────────────────────────────────
    // A palette is "retained" if the user did NOT change it.
    for (tid, count) in &agg.projects_per_template {
        let template_projects: Vec<&ProjectMetrics> =
            all.iter().filter(|m| m.template_id == *tid).collect();
        let n = *count as f64;
        let mut palette_counts: HashMap<String, usize> = HashMap::new();
        for m in &template_projects {
            if !m.palette_was_changed {
                *palette_counts.entry(m.palette_id.clone()).or_insert(0) += 1;
            }
        }
        let rates: HashMap<String, f64> = palette_counts
            .into_iter()
            .map(|(pid, c)| (pid, c as f64 / n))
            .collect();
        agg.palette_retention.insert(tid.clone(), rates);
    }

    // ── Typography retention (per template) ───────────────────────────────
    for (tid, count) in &agg.projects_per_template {
        let template_projects: Vec<&ProjectMetrics> =
            all.iter().filter(|m| m.template_id == *tid).collect();
        let n = *count as f64;
        let mut typo_counts: HashMap<String, usize> = HashMap::new();
        for m in &template_projects {
            if !m.typography_was_changed {
                *typo_counts.entry(m.typography_id.clone()).or_insert(0) += 1;
            }
        }
        let rates: HashMap<String, f64> = typo_counts
            .into_iter()
            .map(|(tid, c)| (tid, c as f64 / n))
            .collect();
        agg.typography_retention.insert(tid.clone(), rates);
    }

    // ── Section edit rate ─────────────────────────────────────────────────
    let mut section_edited_count: HashMap<String, usize> = HashMap::new();
    for m in all {
        for sid in m.section_edits.keys() {
            *section_edited_count.entry(sid.clone()).or_insert(0) += 1;
        }
    }
    agg.section_edit_rate = section_edited_count
        .into_iter()
        .map(|(sid, c)| (sid, c as f64 / total as f64))
        .collect();

    // ── Quality/conversion/iteration averages per template ────────────────
    for (tid, count) in &agg.projects_per_template {
        let tp: Vec<&ProjectMetrics> = all.iter().filter(|m| m.template_id == *tid).collect();
        let n = *count as f64;
        let avg_q = tp.iter().map(|m| m.quality_score as f64).sum::<f64>() / n;
        let avg_c = tp.iter().map(|m| m.conversion_score as f64).sum::<f64>() / n;
        let avg_i = tp.iter().map(|m| m.iteration_count as f64).sum::<f64>() / n;
        agg.avg_quality_by_template.insert(tid.clone(), avg_q);
        agg.avg_conversion_by_template.insert(tid.clone(), avg_c);
        agg.avg_iterations_by_template.insert(tid.clone(), avg_i);
    }

    // ── Layout selection rate ─────────────────────────────────────────────
    let mut layout_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut layout_totals: HashMap<String, usize> = HashMap::new();
    for m in all {
        for (sid, lid) in &m.layout_selections {
            *layout_counts
                .entry(sid.clone())
                .or_default()
                .entry(lid.clone())
                .or_insert(0) += 1;
            *layout_totals.entry(sid.clone()).or_insert(0) += 1;
        }
    }
    for (sid, counts) in layout_counts {
        let total_for_section = *layout_totals.get(&sid).unwrap_or(&1) as f64;
        let rates: HashMap<String, f64> = counts
            .into_iter()
            .map(|(lid, c)| (lid, c as f64 / total_for_section))
            .collect();
        agg.layout_selection_rate.insert(sid, rates);
    }

    agg
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_metrics(
        template_id: &str,
        palette_id: &str,
        palette_changed: bool,
        quality: u32,
        conversion: u32,
    ) -> ProjectMetrics {
        ProjectMetrics {
            project_id: uuid::Uuid::new_v4().to_string(),
            template_id: template_id.into(),
            palette_id: palette_id.into(),
            typography_id: "modern".into(),
            layout_selections: HashMap::new(),
            palette_was_changed: palette_changed,
            typography_was_changed: false,
            section_edits: HashMap::new(),
            tokens_changed: vec![],
            quality_score: quality,
            conversion_score: conversion,
            iteration_count: 2,
            time_to_deploy_seconds: None,
            variant_selected_index: None,
            auto_fixes_applied: 0,
            build_cost: 0.0,
            completed_at: "2026-04-05T12:00:00Z".into(),
        }
    }

    #[test]
    fn test_aggregate_counts_templates() {
        let metrics = vec![
            mock_metrics("saas_landing", "saas_midnight", false, 90, 80),
            mock_metrics("saas_landing", "saas_midnight", false, 85, 75),
            mock_metrics("portfolio", "port_monochrome", false, 88, 70),
        ];
        let agg = aggregate_metrics(&metrics);
        assert_eq!(agg.total_projects, 3);
        assert_eq!(agg.projects_per_template.get("saas_landing"), Some(&2));
        assert_eq!(agg.projects_per_template.get("portfolio"), Some(&1));
    }

    #[test]
    fn test_aggregate_palette_retention() {
        let metrics = vec![
            mock_metrics("saas_landing", "saas_midnight", false, 90, 80),
            mock_metrics("saas_landing", "saas_midnight", false, 85, 75),
            mock_metrics("saas_landing", "saas_ocean", true, 88, 70), // changed
        ];
        let agg = aggregate_metrics(&metrics);
        let saas_retention = agg.palette_retention.get("saas_landing").unwrap();
        // 2 out of 3 kept midnight
        let midnight_rate = saas_retention.get("saas_midnight").unwrap();
        assert!((midnight_rate - 2.0 / 3.0).abs() < 0.01);
        // ocean was changed, so not retained
        assert!(saas_retention.get("saas_ocean").is_none());
    }

    #[test]
    fn test_aggregate_section_edit_rate() {
        let mut m1 = mock_metrics("saas_landing", "saas_midnight", false, 90, 80);
        m1.section_edits.insert("hero".into(), 3);
        let mut m2 = mock_metrics("saas_landing", "saas_midnight", false, 85, 75);
        m2.section_edits.insert("hero".into(), 1);
        let m3 = mock_metrics("saas_landing", "saas_midnight", false, 88, 70);
        // m3 doesn't edit hero

        let agg = aggregate_metrics(&[m1, m2, m3]);
        let hero_rate = agg.section_edit_rate.get("hero").unwrap();
        assert!((hero_rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_aggregate_empty_input() {
        let agg = aggregate_metrics(&[]);
        assert_eq!(agg.total_projects, 0);
        assert!(agg.projects_per_template.is_empty());
    }

    #[test]
    fn test_aggregate_quality_averages() {
        let metrics = vec![
            mock_metrics("saas_landing", "saas_midnight", false, 90, 80),
            mock_metrics("saas_landing", "saas_midnight", false, 80, 60),
        ];
        let agg = aggregate_metrics(&metrics);
        let avg_q = agg.avg_quality_by_template.get("saas_landing").unwrap();
        assert!((avg_q - 85.0).abs() < 0.01);
        let avg_c = agg.avg_conversion_by_template.get("saas_landing").unwrap();
        assert!((avg_c - 70.0).abs() < 0.01);
    }
}
