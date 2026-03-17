//! Hot patcher — applies tested patches to the system with health checks and
//! rollback support.  For kernel code, patches are saved as signed records and
//! applied on next restart.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::patch::{Patch, PatchStatus};
use super::SelfRewriteError;

/// A record of an applied patch with health check status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedPatch {
    pub patch_id: Uuid,
    pub applied_at: u64,
    pub health_check_passed: bool,
    pub rollback_available: bool,
    pub benchmark_before: f64,
    pub benchmark_after: f64,
    /// SHA-256 signature of the patch content for integrity verification.
    pub content_hash: String,
}

/// Applies patches to the running system.
#[derive(Debug, Clone)]
pub struct HotPatcher {
    /// Applied patches in order of application.
    applied_patches: Vec<AppliedPatch>,
    /// Pending patches awaiting restart.
    pending_restart: Vec<Patch>,
    /// Health check interval in seconds (default 300 = 5 minutes).
    health_check_interval_secs: u64,
}

impl HotPatcher {
    pub fn new() -> Self {
        Self {
            applied_patches: Vec::new(),
            pending_restart: Vec::new(),
            health_check_interval_secs: 300,
        }
    }

    pub fn with_health_check_interval(interval_secs: u64) -> Self {
        Self {
            applied_patches: Vec::new(),
            pending_restart: Vec::new(),
            health_check_interval_secs: interval_secs,
        }
    }

    /// Apply a tested and approved patch.
    ///
    /// # Safety requirements
    /// - Patch must have status `Approved` (HITL approval received)
    /// - Patch must have `requires_approval = true`
    /// - Kernel patches are queued for next restart
    pub fn apply_patch(
        &mut self,
        mut patch: Patch,
        benchmark_before: f64,
        benchmark_after: f64,
    ) -> Result<AppliedPatch, SelfRewriteError> {
        // Triple safety check
        if !patch.requires_approval {
            return Err(SelfRewriteError::HitlApprovalRequired(patch.id.to_string()));
        }

        if patch.status != PatchStatus::Approved {
            return Err(SelfRewriteError::PatchApplicationFailed(format!(
                "patch {} must be approved before application (current status: {:?})",
                patch.id, patch.status
            )));
        }

        // Check for duplicate application
        if self
            .applied_patches
            .iter()
            .any(|ap| ap.patch_id == patch.id)
        {
            return Err(SelfRewriteError::PatchAlreadyApplied(patch.id.to_string()));
        }

        // Compute content hash for integrity
        let content_hash = Self::hash_content(&patch.optimized_code);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // For kernel code: queue for restart rather than live patching
        let is_kernel = patch.target_file.contains("kernel/");
        if is_kernel {
            self.pending_restart.push(patch.clone());
        }

        patch.status = PatchStatus::Applied;

        let applied = AppliedPatch {
            patch_id: patch.id,
            applied_at: now,
            health_check_passed: false, // Will be set after health check
            rollback_available: true,
            benchmark_before,
            benchmark_after,
            content_hash,
        };

        self.applied_patches.push(applied.clone());
        Ok(applied)
    }

    /// Schedule a health check for the most recently applied patch.
    /// Returns the health check interval in seconds.
    pub fn schedule_health_check(&self) -> u64 {
        self.health_check_interval_secs
    }

    /// Mark a patch's health check as passed or failed.
    pub fn update_health_check(
        &mut self,
        patch_id: Uuid,
        passed: bool,
    ) -> Result<(), SelfRewriteError> {
        let applied = self
            .applied_patches
            .iter_mut()
            .find(|ap| ap.patch_id == patch_id)
            .ok_or_else(|| SelfRewriteError::PatchNotFound(patch_id.to_string()))?;

        applied.health_check_passed = passed;
        if passed {
            // Once health check passes, rollback is still available but less
            // likely needed.
            applied.rollback_available = true;
        }
        Ok(())
    }

    /// Get all applied patches.
    pub fn get_applied_patches(&self) -> &[AppliedPatch] {
        &self.applied_patches
    }

    /// Get patches pending restart.
    pub fn get_pending_restart(&self) -> &[Patch] {
        &self.pending_restart
    }

    /// Verify the integrity of an applied patch by recomputing its hash.
    pub fn verify_integrity(
        &self,
        patch_id: Uuid,
        current_code: &str,
    ) -> Result<bool, SelfRewriteError> {
        let applied = self
            .applied_patches
            .iter()
            .find(|ap| ap.patch_id == patch_id)
            .ok_or_else(|| SelfRewriteError::PatchNotFound(patch_id.to_string()))?;

        let current_hash = Self::hash_content(current_code);
        Ok(current_hash == applied.content_hash)
    }

    /// Remove a patch from the applied list (after rollback).
    pub fn mark_reverted(&mut self, patch_id: Uuid) -> Result<(), SelfRewriteError> {
        let idx = self
            .applied_patches
            .iter()
            .position(|ap| ap.patch_id == patch_id)
            .ok_or_else(|| SelfRewriteError::PatchNotFound(patch_id.to_string()))?;

        self.applied_patches.remove(idx);

        // Also remove from pending restart if present
        self.pending_restart.retain(|p| p.id != patch_id);

        Ok(())
    }

    /// Compute SHA-256 hash of content.
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        hex_encode(&result)
    }
}

impl Default for HotPatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_rewrite::patch::PatchGenerator;

    fn make_approved_patch() -> Patch {
        let gen = PatchGenerator::new();
        let mut patch = gen
            .generate_patch(
                "src/lib.rs",
                "process",
                "fn process() { let v = Vec::new(); }",
                "fn process() { let v = Vec::with_capacity(64); }",
                "preallocate",
            )
            .unwrap();
        patch = gen.validate_syntax(patch).unwrap();
        patch.status = PatchStatus::Approved;
        patch
    }

    #[test]
    fn apply_approved_patch() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch_id = patch.id;

        let applied = patcher.apply_patch(patch, 50.0, 30.0).unwrap();
        assert_eq!(applied.patch_id, patch_id);
        assert!(applied.rollback_available);
        assert!(!applied.health_check_passed); // Not yet checked
        assert!(!applied.content_hash.is_empty());
    }

    #[test]
    fn reject_unapproved_patch() {
        let mut patcher = HotPatcher::new();
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "fn f() { return; }", "opt")
            .unwrap();
        // Status is Generated, not Approved
        let err = patcher.apply_patch(patch, 10.0, 5.0).unwrap_err();
        assert!(err.to_string().contains("approved"));
    }

    #[test]
    fn reject_hitl_bypass() {
        let mut patcher = HotPatcher::new();
        let mut patch = make_approved_patch();
        patch.requires_approval = false;

        let err = patcher.apply_patch(patch, 10.0, 5.0).unwrap_err();
        assert!(matches!(err, SelfRewriteError::HitlApprovalRequired(_)));
    }

    #[test]
    fn reject_duplicate_application() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch2 = patch.clone();

        patcher.apply_patch(patch, 50.0, 30.0).unwrap();
        let err = patcher.apply_patch(patch2, 50.0, 30.0).unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchAlreadyApplied(_)));
    }

    #[test]
    fn health_check_update() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch_id = patch.id;

        patcher.apply_patch(patch, 50.0, 30.0).unwrap();

        assert!(!patcher.applied_patches[0].health_check_passed);
        patcher.update_health_check(patch_id, true).unwrap();
        assert!(patcher.applied_patches[0].health_check_passed);
    }

    #[test]
    fn verify_integrity_pass() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch_id = patch.id;
        let code = patch.optimized_code.clone();

        patcher.apply_patch(patch, 50.0, 30.0).unwrap();
        assert!(patcher.verify_integrity(patch_id, &code).unwrap());
    }

    #[test]
    fn verify_integrity_fail_on_tampering() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch_id = patch.id;

        patcher.apply_patch(patch, 50.0, 30.0).unwrap();
        assert!(!patcher.verify_integrity(patch_id, "tampered code").unwrap());
    }

    #[test]
    fn mark_reverted_removes_patch() {
        let mut patcher = HotPatcher::new();
        let patch = make_approved_patch();
        let patch_id = patch.id;

        patcher.apply_patch(patch, 50.0, 30.0).unwrap();
        assert_eq!(patcher.get_applied_patches().len(), 1);

        patcher.mark_reverted(patch_id).unwrap();
        assert_eq!(patcher.get_applied_patches().len(), 0);
    }

    #[test]
    fn kernel_patch_queued_for_restart() {
        let mut patcher = HotPatcher::new();
        let gen = PatchGenerator::new();
        let mut patch = gen
            .generate_patch(
                "kernel/src/lib.rs",
                "init",
                "fn init() { }",
                "fn init() { return; }",
                "opt",
            )
            .unwrap();
        patch = gen.validate_syntax(patch).unwrap();
        patch.status = PatchStatus::Approved;

        patcher.apply_patch(patch, 10.0, 5.0).unwrap();
        assert_eq!(patcher.get_pending_restart().len(), 1);
    }

    #[test]
    fn applied_patch_serialization() {
        let applied = AppliedPatch {
            patch_id: Uuid::new_v4(),
            applied_at: 1700000000,
            health_check_passed: true,
            rollback_available: true,
            benchmark_before: 50.0,
            benchmark_after: 30.0,
            content_hash: "abc123".into(),
        };
        let json = serde_json::to_string(&applied).unwrap();
        let back: AppliedPatch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch_id, applied.patch_id);
        assert!(back.health_check_passed);
    }
}
