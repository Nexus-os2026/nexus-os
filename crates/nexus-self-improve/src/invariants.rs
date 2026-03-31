//! # 10 Hard Invariants
//!
//! These invariants are enforced at the kernel level and CANNOT be overridden.
//! Every improvement proposal must pass ALL 10 before it can be applied.

use crate::types::{ImprovementProposal, InvariantId, ProposedChange};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The 10 hard invariants that govern self-improvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardInvariant {
    /// #1: Cannot modify kernel governance code.
    GovernanceKernelImmutable,
    /// #2: Hash chain must remain valid.
    AuditTrailIntegrity,
    /// #3: Cannot lower consent tier requirements.
    HitlGatesCannotWeaken,
    /// #4: Cannot grant self new capabilities.
    CapabilitiesCannotExpand,
    /// #5: Operates within fuel budget.
    FuelLimitsEnforced,
    /// #6: Ed25519 identity cannot change.
    CryptoIdentityImmutable,
    /// #7: Every change has a checkpoint.
    AllChangesReversible,
    /// #8: All tests pass before and after.
    TestSuiteGreen,
    /// #9: Tier3 HITL for all changes.
    HitlApprovalRequired,
    /// #10: Cannot modify self-improvement pipeline.
    SelfProtected,
}

impl HardInvariant {
    /// Returns all 10 invariants in order.
    pub fn all() -> &'static [HardInvariant] {
        &[
            Self::GovernanceKernelImmutable,
            Self::AuditTrailIntegrity,
            Self::HitlGatesCannotWeaken,
            Self::CapabilitiesCannotExpand,
            Self::FuelLimitsEnforced,
            Self::CryptoIdentityImmutable,
            Self::AllChangesReversible,
            Self::TestSuiteGreen,
            Self::HitlApprovalRequired,
            Self::SelfProtected,
        ]
    }

    /// Returns the numeric ID (1-10).
    pub fn id(&self) -> InvariantId {
        InvariantId(match self {
            Self::GovernanceKernelImmutable => 1,
            Self::AuditTrailIntegrity => 2,
            Self::HitlGatesCannotWeaken => 3,
            Self::CapabilitiesCannotExpand => 4,
            Self::FuelLimitsEnforced => 5,
            Self::CryptoIdentityImmutable => 6,
            Self::AllChangesReversible => 7,
            Self::TestSuiteGreen => 8,
            Self::HitlApprovalRequired => 9,
            Self::SelfProtected => 10,
        })
    }

    /// Check this invariant against a proposal and system state.
    pub fn check(
        &self,
        proposal: &ImprovementProposal,
        state: &InvariantCheckState,
    ) -> Result<(), InvariantViolation> {
        match self {
            Self::GovernanceKernelImmutable => check_governance_immutable(proposal),
            Self::AuditTrailIntegrity => check_audit_integrity(state),
            Self::HitlGatesCannotWeaken => check_hitl_gates(proposal),
            Self::CapabilitiesCannotExpand => check_capabilities(proposal),
            Self::FuelLimitsEnforced => check_fuel_limits(proposal, state),
            Self::CryptoIdentityImmutable => check_crypto_identity(proposal),
            Self::AllChangesReversible => check_reversible(proposal),
            Self::TestSuiteGreen => check_test_suite(state),
            Self::HitlApprovalRequired => check_hitl_required(state),
            Self::SelfProtected => check_self_protected(proposal),
        }
    }
}

impl fmt::Display for HardInvariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GovernanceKernelImmutable => write!(f, "#1 Governance kernel immutable"),
            Self::AuditTrailIntegrity => write!(f, "#2 Audit trail integrity"),
            Self::HitlGatesCannotWeaken => write!(f, "#3 HITL gates cannot weaken"),
            Self::CapabilitiesCannotExpand => write!(f, "#4 Capabilities cannot expand"),
            Self::FuelLimitsEnforced => write!(f, "#5 Fuel limits enforced"),
            Self::CryptoIdentityImmutable => write!(f, "#6 Crypto identity immutable"),
            Self::AllChangesReversible => write!(f, "#7 All changes reversible"),
            Self::TestSuiteGreen => write!(f, "#8 Test suite green"),
            Self::HitlApprovalRequired => write!(f, "#9 HITL approval required"),
            Self::SelfProtected => write!(f, "#10 Self-improvement pipeline protected"),
        }
    }
}

/// An invariant check failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantViolation {
    pub invariant: HardInvariant,
    pub reason: String,
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invariant violation {}: {}", self.invariant, self.reason)
    }
}

/// State required for invariant checking.
#[derive(Debug, Clone)]
pub struct InvariantCheckState {
    pub audit_chain_valid: bool,
    pub test_suite_passing: bool,
    pub hitl_approved: bool,
    pub fuel_remaining: u64,
    pub fuel_budget: u64,
}

impl Default for InvariantCheckState {
    fn default() -> Self {
        Self {
            audit_chain_valid: true,
            test_suite_passing: true,
            hitl_approved: false,
            fuel_remaining: 10_000,
            fuel_budget: 10_000,
        }
    }
}

/// Paths that are protected by invariant #1 (GovernanceKernelImmutable).
const PROTECTED_GOVERNANCE_PATHS: &[&str] = &[
    "kernel/src/permissions.rs",
    "kernel/src/consent.rs",
    "kernel/src/autonomy.rs",
    "kernel/src/firewall/",
    "kernel/src/owasp_defenses.rs",
    "kernel/src/checkpoint.rs",
    "kernel/src/supervisor.rs",
    "kernel/src/audit/",
    "kernel/src/identity/",
    "kernel/src/hardware_security/",
    "kernel/src/policy_engine/",
    "kernel/src/resource_limiter",
];

/// Paths protected by invariant #10 (SelfProtected).
const PROTECTED_SELF_PATHS: &[&str] = &["crates/nexus-self-improve/", "kernel/src/self_improve"];

/// Paths that could weaken HITL gates.
const HITL_GATE_PATHS: &[&str] = &["kernel/src/consent.rs", "kernel/src/autonomy.rs"];

/// Paths that could modify crypto identity.
const CRYPTO_IDENTITY_PATHS: &[&str] = &[
    "kernel/src/identity/",
    "kernel/src/hardware_security/",
    "crates/nexus-crypto/",
];

// ── Individual invariant checks ─────────────────────────────────────

fn check_governance_immutable(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    for file in proposal.change.touched_files() {
        if PROTECTED_GOVERNANCE_PATHS
            .iter()
            .any(|p| file.starts_with(p))
        {
            return Err(InvariantViolation {
                invariant: HardInvariant::GovernanceKernelImmutable,
                reason: format!("proposal touches protected governance path: {file}"),
            });
        }
    }
    Ok(())
}

fn check_audit_integrity(state: &InvariantCheckState) -> Result<(), InvariantViolation> {
    if !state.audit_chain_valid {
        return Err(InvariantViolation {
            invariant: HardInvariant::AuditTrailIntegrity,
            reason: "audit hash chain is invalid — cannot apply improvements to a corrupted system"
                .into(),
        });
    }
    Ok(())
}

fn check_hitl_gates(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    // Policy updates must not lower HITL tier requirements
    if let ProposedChange::PolicyUpdate {
        new_policy_cedar, ..
    } = &proposal.change
    {
        let lower = new_policy_cedar.to_lowercase();
        if lower.contains("tier0") && lower.contains("self_mutation") {
            return Err(InvariantViolation {
                invariant: HardInvariant::HitlGatesCannotWeaken,
                reason: "policy would lower HITL tier for self-mutation operations".into(),
            });
        }
    }
    // Code patches must not touch HITL gate code
    for file in proposal.change.touched_files() {
        if HITL_GATE_PATHS.iter().any(|p| file.starts_with(p)) {
            return Err(InvariantViolation {
                invariant: HardInvariant::HitlGatesCannotWeaken,
                reason: format!("proposal touches HITL gate code: {file}"),
            });
        }
    }
    Ok(())
}

fn check_capabilities(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    // Config changes must not add new capabilities
    if let ProposedChange::ConfigChange { key, .. } = &proposal.change {
        if key.contains("capabilities") || key.contains("permissions") {
            return Err(InvariantViolation {
                invariant: HardInvariant::CapabilitiesCannotExpand,
                reason: format!("config change targets capability/permission key: {key}"),
            });
        }
    }
    Ok(())
}

fn check_fuel_limits(
    proposal: &ImprovementProposal,
    state: &InvariantCheckState,
) -> Result<(), InvariantViolation> {
    if proposal.fuel_cost > state.fuel_remaining {
        return Err(InvariantViolation {
            invariant: HardInvariant::FuelLimitsEnforced,
            reason: format!(
                "proposal costs {} fuel but only {} remaining",
                proposal.fuel_cost, state.fuel_remaining
            ),
        });
    }
    Ok(())
}

fn check_crypto_identity(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    for file in proposal.change.touched_files() {
        if CRYPTO_IDENTITY_PATHS.iter().any(|p| file.starts_with(p)) {
            return Err(InvariantViolation {
                invariant: HardInvariant::CryptoIdentityImmutable,
                reason: format!("proposal touches crypto identity code: {file}"),
            });
        }
    }
    Ok(())
}

fn check_reversible(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    if proposal.rollback_plan.steps.is_empty() {
        return Err(InvariantViolation {
            invariant: HardInvariant::AllChangesReversible,
            reason: "proposal has no rollback steps — all changes must be reversible".into(),
        });
    }
    // checkpoint_id must be non-nil (zero UUID means not yet created)
    if proposal.rollback_plan.checkpoint_id.is_nil() {
        return Err(InvariantViolation {
            invariant: HardInvariant::AllChangesReversible,
            reason: "rollback plan has no checkpoint_id — checkpoint must be pre-allocated".into(),
        });
    }
    Ok(())
}

fn check_test_suite(state: &InvariantCheckState) -> Result<(), InvariantViolation> {
    if !state.test_suite_passing {
        return Err(InvariantViolation {
            invariant: HardInvariant::TestSuiteGreen,
            reason: "test suite is not passing — cannot apply improvements to a broken system"
                .into(),
        });
    }
    Ok(())
}

fn check_hitl_required(state: &InvariantCheckState) -> Result<(), InvariantViolation> {
    if !state.hitl_approved {
        return Err(InvariantViolation {
            invariant: HardInvariant::HitlApprovalRequired,
            reason: "Tier3 HITL approval has not been granted".into(),
        });
    }
    Ok(())
}

fn check_self_protected(proposal: &ImprovementProposal) -> Result<(), InvariantViolation> {
    for file in proposal.change.touched_files() {
        if PROTECTED_SELF_PATHS.iter().any(|p| file.starts_with(p)) {
            return Err(InvariantViolation {
                invariant: HardInvariant::SelfProtected,
                reason: format!("proposal touches self-improvement pipeline code: {file}"),
            });
        }
    }
    Ok(())
}

/// Validate ALL 10 invariants against a proposal. Returns `Ok(())` if all pass,
/// or `Err(violations)` with every failed invariant.
pub fn validate_all_invariants(
    proposal: &ImprovementProposal,
    state: &InvariantCheckState,
) -> Result<(), Vec<InvariantViolation>> {
    let violations: Vec<InvariantViolation> = HardInvariant::all()
        .iter()
        .filter_map(|inv| inv.check(proposal, state).err())
        .collect();

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProposedChange, RollbackPlan, RollbackStep, SafetyProof};
    use uuid::Uuid;

    fn make_proposal(change: ProposedChange) -> ImprovementProposal {
        ImprovementProposal {
            id: Uuid::new_v4(),
            opportunity_id: Uuid::new_v4(),
            domain: change.domain(),
            description: "test proposal".into(),
            change,
            rollback_plan: RollbackPlan {
                checkpoint_id: Uuid::new_v4(),
                steps: vec![RollbackStep {
                    description: "revert".into(),
                    action: serde_json::json!({"revert": true}),
                }],
                estimated_rollback_time_ms: 100,
                automatic: true,
            },
            expected_tests: vec![],
            proof: None,
            generated_by: "test".into(),
            fuel_cost: 100,
        }
    }

    fn passing_state() -> InvariantCheckState {
        InvariantCheckState {
            audit_chain_valid: true,
            test_suite_passing: true,
            hitl_approved: true,
            fuel_remaining: 10_000,
            fuel_budget: 10_000,
        }
    }

    fn safe_config_change() -> ProposedChange {
        ProposedChange::ConfigChange {
            key: "agent.response_timeout_ms".into(),
            old_value: serde_json::json!(5000),
            new_value: serde_json::json!(3000),
            justification: "reduce latency".into(),
        }
    }

    // ── Invariant #1: Governance kernel immutable ───────────────────

    #[test]
    fn test_invariant_governance_kernel_immutable_blocks_kernel_patch() {
        let change = ProposedChange::CodePatch {
            target_file: "kernel/src/permissions.rs".into(),
            diff: "- old\n+ new".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: crate::types::ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "abc".into(),
            },
        };
        let proposal = make_proposal(change);
        let result = HardInvariant::GovernanceKernelImmutable.check(&proposal, &passing_state());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .reason
            .contains("protected governance path"));
    }

    #[test]
    fn test_invariant_governance_allows_non_kernel_changes() {
        let proposal = make_proposal(safe_config_change());
        let result = HardInvariant::GovernanceKernelImmutable.check(&proposal, &passing_state());
        assert!(result.is_ok());
    }

    // ── Invariant #2: Audit trail integrity ─────────────────────────

    #[test]
    fn test_invariant_audit_integrity_fails_when_chain_invalid() {
        let proposal = make_proposal(safe_config_change());
        let mut state = passing_state();
        state.audit_chain_valid = false;
        let result = HardInvariant::AuditTrailIntegrity.check(&proposal, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_invariant_audit_integrity_passes_when_chain_valid() {
        let proposal = make_proposal(safe_config_change());
        let result = HardInvariant::AuditTrailIntegrity.check(&proposal, &passing_state());
        assert!(result.is_ok());
    }

    // ── Invariant #3: HITL gates cannot weaken ──────────────────────

    #[test]
    fn test_invariant_hitl_gates_blocks_consent_code_patch() {
        let change = ProposedChange::CodePatch {
            target_file: "kernel/src/consent.rs".into(),
            diff: "weaken".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: crate::types::ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "abc".into(),
            },
        };
        let proposal = make_proposal(change);
        let result = HardInvariant::HitlGatesCannotWeaken.check(&proposal, &passing_state());
        assert!(result.is_err());
    }

    // ── Invariant #4: Capabilities cannot expand ────────────────────

    #[test]
    fn test_invariant_capabilities_blocks_capability_config() {
        let change = ProposedChange::ConfigChange {
            key: "agent.capabilities.new_cap".into(),
            old_value: serde_json::json!(null),
            new_value: serde_json::json!(true),
            justification: "add capability".into(),
        };
        let proposal = make_proposal(change);
        let result = HardInvariant::CapabilitiesCannotExpand.check(&proposal, &passing_state());
        assert!(result.is_err());
    }

    // ── Invariant #5: Fuel limits enforced ──────────────────────────

    #[test]
    fn test_invariant_fuel_limits_blocks_over_budget() {
        let proposal = make_proposal(safe_config_change());
        let state = InvariantCheckState {
            fuel_remaining: 50,
            ..passing_state()
        };
        let result = HardInvariant::FuelLimitsEnforced.check(&proposal, &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("fuel"));
    }

    #[test]
    fn test_invariant_fuel_limits_passes_within_budget() {
        let proposal = make_proposal(safe_config_change());
        let result = HardInvariant::FuelLimitsEnforced.check(&proposal, &passing_state());
        assert!(result.is_ok());
    }

    // ── Invariant #6: Crypto identity immutable ─────────────────────

    #[test]
    fn test_invariant_crypto_identity_blocks_identity_patch() {
        let change = ProposedChange::CodePatch {
            target_file: "kernel/src/identity/agent_identity.rs".into(),
            diff: "modify".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: crate::types::ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "abc".into(),
            },
        };
        let proposal = make_proposal(change);
        let result = HardInvariant::CryptoIdentityImmutable.check(&proposal, &passing_state());
        assert!(result.is_err());
    }

    // ── Invariant #7: All changes reversible ────────────────────────

    #[test]
    fn test_invariant_reversible_blocks_empty_rollback() {
        let mut proposal = make_proposal(safe_config_change());
        proposal.rollback_plan.steps.clear();
        let result = HardInvariant::AllChangesReversible.check(&proposal, &passing_state());
        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("no rollback steps"));
    }

    #[test]
    fn test_invariant_reversible_blocks_nil_checkpoint() {
        let mut proposal = make_proposal(safe_config_change());
        proposal.rollback_plan.checkpoint_id = Uuid::nil();
        let result = HardInvariant::AllChangesReversible.check(&proposal, &passing_state());
        assert!(result.is_err());
    }

    // ── Invariant #8: Test suite green ──────────────────────────────

    #[test]
    fn test_invariant_test_suite_blocks_when_failing() {
        let proposal = make_proposal(safe_config_change());
        let state = InvariantCheckState {
            test_suite_passing: false,
            ..passing_state()
        };
        let result = HardInvariant::TestSuiteGreen.check(&proposal, &state);
        assert!(result.is_err());
    }

    // ── Invariant #9: HITL approval required ────────────────────────

    #[test]
    fn test_invariant_hitl_required_blocks_without_approval() {
        let proposal = make_proposal(safe_config_change());
        let state = InvariantCheckState {
            hitl_approved: false,
            ..passing_state()
        };
        let result = HardInvariant::HitlApprovalRequired.check(&proposal, &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().reason.contains("Tier3"));
    }

    // ── Invariant #10: Self-protected ───────────────────────────────

    #[test]
    fn test_invariant_self_protected_blocks_pipeline_patch() {
        let change = ProposedChange::CodePatch {
            target_file: "crates/nexus-self-improve/src/invariants.rs".into(),
            diff: "weaken".into(),
            proof: SafetyProof {
                invariants_checked: vec![],
                proof_method: crate::types::ProofMethod::TypeCheck,
                verifier_version: "1.0".into(),
                proof_hash: "abc".into(),
            },
        };
        let proposal = make_proposal(change);
        let result = HardInvariant::SelfProtected.check(&proposal, &passing_state());
        assert!(result.is_err());
    }

    // ── Validate all invariants ─────────────────────────────────────

    #[test]
    fn test_validate_all_passes_for_safe_proposal() {
        let proposal = make_proposal(safe_config_change());
        let result = validate_all_invariants(&proposal, &passing_state());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_all_returns_multiple_violations() {
        let proposal = make_proposal(safe_config_change());
        let state = InvariantCheckState {
            audit_chain_valid: false,
            test_suite_passing: false,
            hitl_approved: false,
            ..passing_state()
        };
        let result = validate_all_invariants(&proposal, &state);
        assert!(result.is_err());
        let violations = result.unwrap_err();
        assert!(violations.len() >= 3);
    }

    #[test]
    fn test_all_invariants_returns_10() {
        assert_eq!(HardInvariant::all().len(), 10);
    }
}
