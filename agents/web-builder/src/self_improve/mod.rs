//! Self-Improving Builder — governed 5-stage improvement loop.
//!
//! Stages: Observer → Analyzer → Proposer → Validator → Applier
//!
//! ## Five Hard Invariants
//!
//! 1. **Governance kernel is immutable** — the loop cannot modify governance, audit, signing, or itself.
//! 2. **Every improvement is logged** — audit trail records proposals, validations, and applications.
//! 3. **Every improvement is reversible** — rollback restores previous state.
//! 4. **Validation required** — Applier rejects proposals whose status != Validated.
//! 5. **Human override** — auto_apply is off by default; rejected proposals cannot be auto-applied.

pub mod analyzer;
pub mod applier;
pub mod metrics;
pub mod observer;
pub mod proposer;
pub mod store;
pub mod validator;

/// Re-export core types under a private name so sibling modules can reference
/// `mod_types::SystemDefaults` without circular `use super::*` issues.
pub mod mod_types {
    pub use super::SlotAdjustment;
    pub use super::SystemDefaults;
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── SystemDefaults ────────────────────────────────────────────────────────

/// Transparent overlay on the builder's hardcoded defaults.
///
/// If empty, every system behaves exactly as before self-improvement was added.
/// Existing code checks these rankings as an overlay — the first entry in a
/// ranking list becomes the new default for that template.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemDefaults {
    /// template_id → ordered palette IDs (first = new default).
    pub palette_rankings: HashMap<String, Vec<String>>,
    /// template_id → ordered typography IDs (first = new default).
    pub typography_rankings: HashMap<String, Vec<String>>,
    /// template_id → section_id → ordered layout IDs.
    pub layout_rankings: HashMap<String, HashMap<String, Vec<String>>>,
    /// Additional hints appended to the content generation prompt.
    pub content_prompt_hints: Vec<String>,
    /// Slot constraint overrides keyed by "template.section.slot".
    pub slot_adjustments: HashMap<String, SlotAdjustment>,
    /// Diversity factor → weight override.
    pub variant_diversity_weights: HashMap<String, f32>,
}

/// Override for a single slot constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAdjustment {
    pub max_chars: Option<usize>,
}

// ─── ImprovementStatus (for Tauri) ─────────────────────────────────────────

/// Summary status returned by the Tauri command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementStatus {
    pub projects_analyzed: usize,
    pub proposals_pending: usize,
    pub proposals_applied: usize,
    pub proposals_rejected: usize,
    pub proposals_rolled_back: usize,
    pub defaults_modified: usize,
    pub store_version: u32,
}

/// Compute a summary status from the store.
pub fn compute_status(store: &store::ImprovementStore) -> ImprovementStatus {
    use proposer::ProposalStatus;
    let pending = store
        .proposals
        .iter()
        .filter(|p| p.status == ProposalStatus::Pending || p.status == ProposalStatus::Validated)
        .count();
    let applied = store
        .proposals
        .iter()
        .filter(|p| p.status == ProposalStatus::Applied)
        .count();
    let rejected = store
        .proposals
        .iter()
        .filter(|p| {
            p.status == ProposalStatus::Rejected || p.status == ProposalStatus::ValidationFailed
        })
        .count();
    let rolled_back = store
        .proposals
        .iter()
        .filter(|p| p.status == ProposalStatus::RolledBack)
        .count();

    let defaults_modified = (!store.defaults.palette_rankings.is_empty()) as usize
        + (!store.defaults.typography_rankings.is_empty()) as usize
        + (!store.defaults.layout_rankings.is_empty()) as usize
        + (!store.defaults.content_prompt_hints.is_empty()) as usize
        + (!store.defaults.slot_adjustments.is_empty()) as usize
        + (!store.defaults.variant_diversity_weights.is_empty()) as usize;

    ImprovementStatus {
        projects_analyzed: store.metrics.len(),
        proposals_pending: pending,
        proposals_applied: applied,
        proposals_rejected: rejected,
        proposals_rolled_back: rolled_back,
        defaults_modified,
        store_version: store.version,
    }
}

// ─── Full Loop ─────────────────────────────────────────────────────────────

/// Load the system defaults (from store). Returns empty defaults if no store exists.
pub fn load_system_defaults() -> SystemDefaults {
    store::load_store().map(|s| s.defaults).unwrap_or_default()
}

// ─── Governance Invariant Helpers ──────────────────────────────────────────

/// Forbidden target patterns — the improvement loop CANNOT modify these.
const FORBIDDEN_TARGETS: &[&str] = &[
    "governance",
    "audit_trail",
    "signing",
    "crypto",
    "trust_pack",
    "self_improve",
    "invariant",
    "security",
    "rls",
    "csp",
];

/// Check whether an improvement target references a forbidden module (Invariant #1).
pub fn is_governance_target(target: &analyzer::ImprovementTarget) -> bool {
    let target_str = format!("{target:?}").to_lowercase();
    FORBIDDEN_TARGETS.iter().any(|f| target_str.contains(f))
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improve::analyzer::{
        AnalysisResult, Evidence, Impact, ImprovementTarget, Opportunity,
    };
    use crate::self_improve::applier::{apply_proposal, reset_defaults};
    use crate::self_improve::metrics::{aggregate_metrics, ProjectMetrics};
    use crate::self_improve::proposer::{generate_proposals, Proposal, ProposalStatus};
    use crate::self_improve::validator::validate_proposal;
    use std::collections::HashMap;

    // ─── Governance Invariant Tests ────────────────────────────────────

    #[test]
    fn test_invariant_governance_kernel_immutable() {
        // No improvement target should reference governance modules
        let targets = vec![
            ImprovementTarget::DefaultPalette {
                template_id: "saas_landing".into(),
                palette_name: "midnight".into(),
            },
            ImprovementTarget::ContentPromptHint {
                hint: "Use numbers".into(),
            },
            ImprovementTarget::SlotConstraint {
                template_id: "saas_landing".into(),
                section_id: "hero".into(),
                slot_name: "headline".into(),
                new_max_chars: 90,
            },
        ];
        for target in &targets {
            assert!(
                !is_governance_target(target),
                "legitimate target wrongly classified as governance: {target:?}"
            );
        }

        // Hypothetical governance targets should be blocked
        let bad_target = ImprovementTarget::ContentPromptHint {
            hint: "modify audit_trail signing".into(),
        };
        assert!(
            is_governance_target(&bad_target),
            "governance target should be blocked"
        );
    }

    #[test]
    fn test_invariant_all_proposals_reversible() {
        let analysis = AnalysisResult {
            sample_size: 10,
            confidence: 0.8,
            opportunities: vec![Opportunity {
                id: "opp-0001".into(),
                target: ImprovementTarget::DefaultPalette {
                    template_id: "saas_landing".into(),
                    palette_name: "saas_midnight".into(),
                },
                description: "test".into(),
                evidence: Evidence {
                    metric: "palette_retention_rate".into(),
                    value: 0.72,
                    sample_size: 10,
                    threshold: 0.60,
                },
                estimated_impact: Impact::High,
            }],
        };
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        for p in &proposals {
            assert!(p.reversible, "all proposals must be reversible");
        }
    }

    #[test]
    fn test_invariant_validation_required() {
        // Applier must reject non-Validated proposals
        let mut proposal = Proposal {
            id: "prop-0001".into(),
            opportunity_id: "opp-0001".into(),
            target: ImprovementTarget::DefaultPalette {
                template_id: "saas_landing".into(),
                palette_name: "saas_midnight".into(),
            },
            description: "test".into(),
            before_value: "(default)".into(),
            after_value: "saas_midnight".into(),
            evidence_summary: "72%".into(),
            confidence: 0.8,
            reversible: true,
            auto_apply: false,
            status: ProposalStatus::Pending,
        };
        let mut defaults = SystemDefaults::default();
        let result = apply_proposal(&mut proposal, &mut defaults);
        assert!(result.is_err(), "Pending proposals must not be applied");
    }

    #[test]
    fn test_invariant_human_override() {
        // Rejected proposals should not be auto-applicable
        let analysis = AnalysisResult {
            sample_size: 10,
            confidence: 0.8,
            opportunities: vec![Opportunity {
                id: "opp-0001".into(),
                target: ImprovementTarget::DefaultPalette {
                    template_id: "saas_landing".into(),
                    palette_name: "saas_midnight".into(),
                },
                description: "test".into(),
                evidence: Evidence {
                    metric: "palette_retention_rate".into(),
                    value: 0.72,
                    sample_size: 10,
                    threshold: 0.60,
                },
                estimated_impact: Impact::High,
            }],
        };
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        for p in &proposals {
            assert!(!p.auto_apply, "auto_apply must be off by default");
        }
    }

    #[test]
    fn test_invariant_all_proposals_logged() {
        // Every proposal has an id and status — the store persists them all
        let analysis = AnalysisResult {
            sample_size: 10,
            confidence: 0.8,
            opportunities: vec![Opportunity {
                id: "opp-0001".into(),
                target: ImprovementTarget::DefaultPalette {
                    template_id: "saas_landing".into(),
                    palette_name: "saas_midnight".into(),
                },
                description: "test".into(),
                evidence: Evidence {
                    metric: "palette_retention_rate".into(),
                    value: 0.72,
                    sample_size: 10,
                    threshold: 0.60,
                },
                estimated_impact: Impact::High,
            }],
        };
        let defaults = SystemDefaults::default();
        let proposals = generate_proposals(&analysis, &defaults);
        for p in &proposals {
            assert!(!p.id.is_empty(), "proposal must have an id");
            assert!(!p.opportunity_id.is_empty(), "must link to opportunity");
            assert!(!p.evidence_summary.is_empty(), "must have evidence");
        }
    }

    // ─── Integration: Full Loop ────────────────────────────────────────

    #[test]
    fn test_full_improvement_loop() {
        // 1. Create 5+ mock project metrics
        let metrics: Vec<ProjectMetrics> = (0..7)
            .map(|i| {
                let palette_changed = i >= 5; // 5 out of 7 keep midnight
                ProjectMetrics {
                    project_id: format!("proj-{i}"),
                    template_id: "saas_landing".into(),
                    palette_id: "saas_midnight".into(),
                    typography_id: "modern".into(),
                    layout_selections: HashMap::new(),
                    palette_was_changed: palette_changed,
                    typography_was_changed: false,
                    section_edits: HashMap::new(),
                    tokens_changed: vec![],
                    quality_score: 85,
                    conversion_score: 75,
                    iteration_count: 2,
                    time_to_deploy_seconds: None,
                    variant_selected_index: None,
                    auto_fixes_applied: 0,
                    build_cost: 0.0,
                    completed_at: "2026-04-05T12:00:00Z".into(),
                }
            })
            .collect();

        // 2. Aggregate
        let agg = aggregate_metrics(&metrics);
        assert_eq!(agg.total_projects, 7);

        // 3. Analyze
        let analysis = analyzer::analyze(&agg, 5);
        assert!(
            !analysis.opportunities.is_empty(),
            "should find palette preference"
        );

        // 4. Propose
        let mut defaults = SystemDefaults::default();
        let mut proposals = generate_proposals(&analysis, &defaults);
        assert!(!proposals.is_empty());

        // 5. Validate
        for p in &proposals {
            let result = validate_proposal(p, 85, 75).unwrap();
            assert!(result.passed);
        }

        // 6. Apply
        proposals[0].status = ProposalStatus::Validated;
        let result = apply_proposal(&mut proposals[0], &mut defaults).unwrap();
        assert!(result.applied);
        assert!(!defaults.palette_rankings.is_empty());
    }

    #[test]
    fn test_factory_reset_restores_original_behavior() {
        let mut defaults = SystemDefaults::default();
        defaults
            .palette_rankings
            .insert("saas_landing".into(), vec!["saas_midnight".into()]);
        defaults.content_prompt_hints.push("Use numbers".into());
        defaults.slot_adjustments.insert(
            "saas_landing.hero.headline".into(),
            SlotAdjustment {
                max_chars: Some(90),
            },
        );

        reset_defaults(&mut defaults);

        assert!(defaults.palette_rankings.is_empty());
        assert!(defaults.content_prompt_hints.is_empty());
        assert!(defaults.slot_adjustments.is_empty());
        assert!(defaults.typography_rankings.is_empty());
        assert!(defaults.layout_rankings.is_empty());
        assert!(defaults.variant_diversity_weights.is_empty());
    }

    #[test]
    fn test_system_defaults_empty_is_noop() {
        // Empty SystemDefaults should equal Default::default()
        let defaults = SystemDefaults::default();
        assert!(defaults.palette_rankings.is_empty());
        assert!(defaults.typography_rankings.is_empty());
        assert!(defaults.layout_rankings.is_empty());
        assert!(defaults.content_prompt_hints.is_empty());
        assert!(defaults.slot_adjustments.is_empty());
        assert!(defaults.variant_diversity_weights.is_empty());
    }

    #[test]
    fn test_compute_status() {
        let mut s = store::ImprovementStore::default();
        s.version = 2;
        s.metrics = vec![]; // mock empty
        s.proposals.push(Proposal {
            id: "p1".into(),
            opportunity_id: "o1".into(),
            target: ImprovementTarget::DefaultPalette {
                template_id: "t".into(),
                palette_name: "p".into(),
            },
            description: "t".into(),
            before_value: "a".into(),
            after_value: "b".into(),
            evidence_summary: "x".into(),
            confidence: 0.8,
            reversible: true,
            auto_apply: false,
            status: ProposalStatus::Applied,
        });
        let status = compute_status(&s);
        assert_eq!(status.proposals_applied, 1);
        assert_eq!(status.store_version, 2);
    }
}
