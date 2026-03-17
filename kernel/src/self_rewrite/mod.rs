//! Self-Rewriting Kernel — detects performance bottlenecks, generates optimized
//! patches, tests them in a sandbox, and applies with automatic rollback.
//!
//! # Safety
//!
//! ALL patches require HITL approval before application.  Triple safety checks:
//! 1. Syntax validation (no `unsafe`, basic Rust checks)
//! 2. Sandboxed test run (all tests must pass)
//! 3. Post-apply health monitoring with automatic rollback

pub mod analyzer;
pub mod patch;
pub mod patcher;
pub mod profiler;
pub mod rollback;
pub mod tester;

pub use analyzer::{AnalysisResult, CodeAnalyzer, CodeIssue, FunctionAnalysis, IssueSeverity};
pub use patch::{Patch, PatchDiff, PatchGenerator, PatchStatus};
pub use patcher::{AppliedPatch, HotPatcher};
pub use profiler::{Bottleneck, BottleneckSeverity, PerformanceMetric, PerformanceProfiler};
pub use rollback::{RollbackEngine, RollbackEvent};
pub use tester::{PatchTester, TestRun};

/// Errors for the self-rewrite subsystem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum SelfRewriteError {
    #[error("profiling failed: {0}")]
    ProfilingFailed(String),

    #[error("code analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("patch generation failed: {0}")]
    PatchGenerationFailed(String),

    #[error("patch validation failed: {0}")]
    PatchValidationFailed(String),

    #[error("patch testing failed: {0}")]
    PatchTestingFailed(String),

    #[error("patch application failed: {0}")]
    PatchApplicationFailed(String),

    #[error("rollback failed: {0}")]
    RollbackFailed(String),

    #[error("HITL approval required — patch {0} cannot be applied without human approval")]
    HitlApprovalRequired(String),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("patch not found: {0}")]
    PatchNotFound(String),

    #[error("patch already applied: {0}")]
    PatchAlreadyApplied(String),

    #[error("health check failed after patch: {0}")]
    HealthCheckFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = SelfRewriteError::HitlApprovalRequired("abc-123".into());
        assert!(err.to_string().contains("abc-123"));
        assert!(err.to_string().contains("HITL"));
    }

    #[test]
    fn error_serialization_roundtrip() {
        let err = SelfRewriteError::PatchGenerationFailed("timeout".into());
        let json = serde_json::to_string(&err).unwrap();
        let back: SelfRewriteError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }
}
