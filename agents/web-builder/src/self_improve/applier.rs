//! Applier — applies validated improvements to SystemDefaults or rolls them back.
//!
//! Invariant #3: Every improvement is reversible.
//! Invariant #4: Only proposals with status == Validated can be applied.

use crate::self_improve::analyzer::ImprovementTarget;
use crate::self_improve::mod_types::SystemDefaults;
use crate::self_improve::proposer::{Proposal, ProposalStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Types ─────────────────────────────────────────────────────────────────

/// Result of applying a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub applied: bool,
    pub previous_state: String,
}

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("proposal not validated: status={0}")]
    NotValidated(String),
    #[error("proposal already applied: {0}")]
    AlreadyApplied(String),
    #[error("rollback failed: proposal '{0}' not found or not applied")]
    RollbackNotFound(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

// ─── Apply ─────────────────────────────────────────────────────────────────

/// Apply a validated proposal to SystemDefaults.
///
/// Returns the previous state (serialized) for rollback.
/// Invariant #4: proposal.status must be Validated.
pub fn apply_proposal(
    proposal: &mut Proposal,
    defaults: &mut SystemDefaults,
) -> Result<ApplyResult, ApplyError> {
    if proposal.status != ProposalStatus::Validated {
        return Err(ApplyError::NotValidated(format!(
            "{}: status is {}",
            proposal.id, proposal.status
        )));
    }

    // Snapshot current state for rollback
    let previous_state =
        serde_json::to_string(defaults).map_err(|e| ApplyError::Serialization(e.to_string()))?;

    match &proposal.target {
        ImprovementTarget::DefaultPalette {
            template_id,
            palette_name,
        } => {
            let rankings = defaults
                .palette_rankings
                .entry(template_id.clone())
                .or_default();
            // Move the preferred palette to the front
            rankings.retain(|p| p != palette_name);
            rankings.insert(0, palette_name.clone());
        }
        ImprovementTarget::DefaultTypography {
            template_id,
            typography_name,
        } => {
            let rankings = defaults
                .typography_rankings
                .entry(template_id.clone())
                .or_default();
            rankings.retain(|t| t != typography_name);
            rankings.insert(0, typography_name.clone());
        }
        ImprovementTarget::DefaultLayout {
            template_id,
            section_id,
            layout_name,
        } => {
            let template_layouts = defaults
                .layout_rankings
                .entry(template_id.clone())
                .or_default();
            let section_rankings = template_layouts.entry(section_id.clone()).or_default();
            section_rankings.retain(|l| l != layout_name);
            section_rankings.insert(0, layout_name.clone());
        }
        ImprovementTarget::ContentPromptHint { hint } => {
            if !defaults.content_prompt_hints.contains(hint) {
                defaults.content_prompt_hints.push(hint.clone());
            }
        }
        ImprovementTarget::SlotConstraint {
            template_id,
            section_id,
            slot_name,
            new_max_chars,
        } => {
            let key = format!("{template_id}.{section_id}.{slot_name}");
            defaults.slot_adjustments.insert(
                key,
                crate::self_improve::mod_types::SlotAdjustment {
                    max_chars: Some(*new_max_chars),
                },
            );
        }
        ImprovementTarget::VariantDiversityWeight { factor, new_weight } => {
            defaults
                .variant_diversity_weights
                .insert(factor.clone(), *new_weight);
        }
    }

    proposal.status = ProposalStatus::Applied;

    Ok(ApplyResult {
        applied: true,
        previous_state,
    })
}

/// Roll back a proposal by restoring the previous SystemDefaults state.
pub fn rollback_proposal(
    proposal: &mut Proposal,
    defaults: &mut SystemDefaults,
    previous_state: &str,
) -> Result<(), ApplyError> {
    if proposal.status != ProposalStatus::Applied {
        return Err(ApplyError::RollbackNotFound(proposal.id.clone()));
    }

    let restored: SystemDefaults = serde_json::from_str(previous_state)
        .map_err(|e| ApplyError::Serialization(e.to_string()))?;
    *defaults = restored;
    proposal.status = ProposalStatus::RolledBack;

    Ok(())
}

/// Reset all improvements, returning SystemDefaults to factory state.
pub fn reset_defaults(defaults: &mut SystemDefaults) {
    *defaults = SystemDefaults::default();
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn validated_palette_proposal() -> Proposal {
        Proposal {
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
            status: ProposalStatus::Validated,
        }
    }

    #[test]
    fn test_apply_changes_defaults() {
        let mut proposal = validated_palette_proposal();
        let mut defaults = SystemDefaults::default();
        let result = apply_proposal(&mut proposal, &mut defaults).unwrap();
        assert!(result.applied);
        let rankings = defaults.palette_rankings.get("saas_landing").unwrap();
        assert_eq!(rankings[0], "saas_midnight");
        assert_eq!(proposal.status, ProposalStatus::Applied);
    }

    #[test]
    fn test_rollback_restores_previous() {
        let mut proposal = validated_palette_proposal();
        let mut defaults = SystemDefaults::default();
        let result = apply_proposal(&mut proposal, &mut defaults).unwrap();

        // Now rollback
        rollback_proposal(&mut proposal, &mut defaults, &result.previous_state).unwrap();
        assert_eq!(proposal.status, ProposalStatus::RolledBack);
        assert!(defaults.palette_rankings.is_empty());
    }

    #[test]
    fn test_reset_clears_all() {
        let mut defaults = SystemDefaults::default();
        defaults
            .palette_rankings
            .insert("saas_landing".into(), vec!["saas_midnight".into()]);
        defaults.content_prompt_hints.push("Use numbers".into());
        reset_defaults(&mut defaults);
        assert!(defaults.palette_rankings.is_empty());
        assert!(defaults.content_prompt_hints.is_empty());
    }

    #[test]
    fn test_apply_is_idempotent() {
        let mut proposal = validated_palette_proposal();
        let mut defaults = SystemDefaults::default();
        apply_proposal(&mut proposal, &mut defaults).unwrap();

        // Try to apply again — should fail because status is now Applied
        let mut proposal2 = validated_palette_proposal();
        proposal2.status = ProposalStatus::Validated;
        apply_proposal(&mut proposal2, &mut defaults).unwrap();

        let rankings = defaults.palette_rankings.get("saas_landing").unwrap();
        // Should only have one entry for saas_midnight, not duplicated
        assert_eq!(rankings.iter().filter(|r| *r == "saas_midnight").count(), 1);
    }

    #[test]
    fn test_apply_rejects_non_validated() {
        let mut proposal = validated_palette_proposal();
        proposal.status = ProposalStatus::Pending;
        let mut defaults = SystemDefaults::default();
        let result = apply_proposal(&mut proposal, &mut defaults);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_content_prompt_hint() {
        let mut proposal = Proposal {
            id: "prop-0002".into(),
            opportunity_id: "opp-0002".into(),
            target: ImprovementTarget::ContentPromptHint {
                hint: "Use specific numbers in headlines".into(),
            },
            description: "test".into(),
            before_value: "(none)".into(),
            after_value: "Use specific numbers in headlines".into(),
            evidence_summary: "65%".into(),
            confidence: 0.8,
            reversible: true,
            auto_apply: false,
            status: ProposalStatus::Validated,
        };
        let mut defaults = SystemDefaults::default();
        apply_proposal(&mut proposal, &mut defaults).unwrap();
        assert!(defaults
            .content_prompt_hints
            .contains(&"Use specific numbers in headlines".to_string()));
    }

    #[test]
    fn test_apply_slot_constraint() {
        let mut proposal = Proposal {
            id: "prop-0003".into(),
            opportunity_id: "opp-0003".into(),
            target: ImprovementTarget::SlotConstraint {
                template_id: "saas_landing".into(),
                section_id: "hero".into(),
                slot_name: "headline".into(),
                new_max_chars: 90,
            },
            description: "test".into(),
            before_value: "80".into(),
            after_value: "max_chars=90".into(),
            evidence_summary: "20% truncation".into(),
            confidence: 0.8,
            reversible: true,
            auto_apply: false,
            status: ProposalStatus::Validated,
        };
        let mut defaults = SystemDefaults::default();
        apply_proposal(&mut proposal, &mut defaults).unwrap();
        let adj = defaults
            .slot_adjustments
            .get("saas_landing.hero.headline")
            .unwrap();
        assert_eq!(adj.max_chars, Some(90));
    }
}
