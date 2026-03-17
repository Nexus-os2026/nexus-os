//! Sandboxed patch testing — runs tests and benchmarks against patches before
//! they can be approved.  ALL tests must pass; ANY failure rejects the patch.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::patch::{Patch, PatchStatus};
use super::SelfRewriteError;

/// Result of running tests against a patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRun {
    pub patch_id: Uuid,
    pub compile_success: bool,
    pub tests_passed: u32,
    pub tests_failed: u32,
    pub benchmark_before: f64,
    pub benchmark_after: f64,
    pub improvement_pct: f64,
    pub timestamp: u64,
}

/// Sandboxed patch tester.
#[derive(Debug, Clone)]
pub struct PatchTester {
    /// Minimum improvement percentage required for a patch to pass benchmarks.
    min_improvement_pct: f64,
    /// Test history.
    test_runs: Vec<TestRun>,
}

impl PatchTester {
    pub fn new() -> Self {
        Self {
            min_improvement_pct: 0.0, // Any non-regression is acceptable
            test_runs: Vec::new(),
        }
    }

    pub fn with_min_improvement(min_improvement_pct: f64) -> Self {
        Self {
            min_improvement_pct,
            test_runs: Vec::new(),
        }
    }

    /// Test a patch in a sandboxed environment.
    ///
    /// In production this would:
    /// 1. Copy the target file to a temp directory
    /// 2. Apply the patch
    /// 3. Run `cargo check` to verify compilation
    /// 4. Run `cargo test` to verify correctness
    /// 5. Run benchmarks to measure improvement
    ///
    /// Here we simulate the process by validating the patch structure and
    /// running the provided test/benchmark closures.
    pub fn test_patch(
        &mut self,
        patch: &mut Patch,
        compile_success: bool,
        tests_passed: u32,
        tests_failed: u32,
        benchmark_before: f64,
        benchmark_after: f64,
    ) -> Result<TestRun, SelfRewriteError> {
        if patch.status != PatchStatus::Validated {
            return Err(SelfRewriteError::PatchTestingFailed(format!(
                "patch {} must be validated before testing (current status: {:?})",
                patch.id, patch.status
            )));
        }

        patch.status = PatchStatus::Testing;

        let improvement_pct = if benchmark_before > 0.0 {
            ((benchmark_before - benchmark_after) / benchmark_before) * 100.0
        } else {
            0.0
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let test_run = TestRun {
            patch_id: patch.id,
            compile_success,
            tests_passed,
            tests_failed,
            benchmark_before,
            benchmark_after,
            improvement_pct,
            timestamp: now,
        };

        // ALL tests must pass.  ANY failure = patch rejected.
        if !compile_success {
            patch.status = PatchStatus::Rejected;
            self.test_runs.push(test_run.clone());
            return Err(SelfRewriteError::PatchTestingFailed(format!(
                "patch {} failed to compile",
                patch.id
            )));
        }

        if tests_failed > 0 {
            patch.status = PatchStatus::Rejected;
            self.test_runs.push(test_run.clone());
            return Err(SelfRewriteError::PatchTestingFailed(format!(
                "patch {} failed {} tests ({} passed, {} failed)",
                patch.id, tests_failed, tests_passed, tests_failed
            )));
        }

        // Check for performance regression
        if improvement_pct < self.min_improvement_pct {
            patch.status = PatchStatus::Rejected;
            self.test_runs.push(test_run.clone());
            return Err(SelfRewriteError::PatchTestingFailed(format!(
                "patch {} improvement {improvement_pct:.1}% below minimum {:.1}%",
                patch.id, self.min_improvement_pct
            )));
        }

        patch.status = PatchStatus::Tested;
        self.test_runs.push(test_run.clone());
        Ok(test_run)
    }

    /// Run benchmarks by comparing before/after durations.
    /// Returns the improvement percentage (positive = faster).
    pub fn run_benchmarks(&self, benchmark_before: f64, benchmark_after: f64) -> f64 {
        if benchmark_before <= 0.0 {
            return 0.0;
        }
        ((benchmark_before - benchmark_after) / benchmark_before) * 100.0
    }

    /// Get all test runs for a given patch.
    pub fn get_test_runs(&self, patch_id: Uuid) -> Vec<&TestRun> {
        self.test_runs
            .iter()
            .filter(|r| r.patch_id == patch_id)
            .collect()
    }

    /// Get the full test history.
    pub fn get_history(&self) -> &[TestRun] {
        &self.test_runs
    }
}

impl Default for PatchTester {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_rewrite::patch::PatchGenerator;

    fn make_validated_patch() -> Patch {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch(
                "src/lib.rs",
                "process",
                "fn process() { let v = Vec::new(); }",
                "fn process() { let v = Vec::with_capacity(64); }",
                "preallocate",
            )
            .unwrap();
        gen.validate_syntax(patch).unwrap()
    }

    #[test]
    fn test_passing_patch() {
        let mut tester = PatchTester::new();
        let mut patch = make_validated_patch();

        let result = tester
            .test_patch(&mut patch, true, 10, 0, 50.0, 30.0)
            .unwrap();

        assert!(result.compile_success);
        assert_eq!(result.tests_passed, 10);
        assert_eq!(result.tests_failed, 0);
        assert!(result.improvement_pct > 0.0);
        assert_eq!(patch.status, PatchStatus::Tested);
    }

    #[test]
    fn test_compile_failure_rejects() {
        let mut tester = PatchTester::new();
        let mut patch = make_validated_patch();

        let err = tester
            .test_patch(&mut patch, false, 0, 0, 50.0, 50.0)
            .unwrap_err();
        assert!(err.to_string().contains("compile"));
        assert_eq!(patch.status, PatchStatus::Rejected);
    }

    #[test]
    fn test_any_failure_rejects() {
        let mut tester = PatchTester::new();
        let mut patch = make_validated_patch();

        let err = tester
            .test_patch(&mut patch, true, 9, 1, 50.0, 30.0)
            .unwrap_err();
        assert!(err.to_string().contains("failed 1 tests"));
        assert_eq!(patch.status, PatchStatus::Rejected);
    }

    #[test]
    fn test_regression_with_min_improvement() {
        let mut tester = PatchTester::with_min_improvement(10.0);
        let mut patch = make_validated_patch();

        // 5% improvement is below the 10% minimum
        let err = tester
            .test_patch(&mut patch, true, 10, 0, 100.0, 95.0)
            .unwrap_err();
        assert!(err.to_string().contains("improvement"));
        assert_eq!(patch.status, PatchStatus::Rejected);
    }

    #[test]
    fn test_must_be_validated_first() {
        let mut tester = PatchTester::new();
        let gen = PatchGenerator::new();
        let mut patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "fn f() { return; }", "opt")
            .unwrap();
        // Status is Generated, not Validated
        let err = tester
            .test_patch(&mut patch, true, 1, 0, 10.0, 5.0)
            .unwrap_err();
        assert!(err.to_string().contains("validated"));
    }

    #[test]
    fn run_benchmarks_calculation() {
        let tester = PatchTester::new();
        let pct = tester.run_benchmarks(100.0, 80.0);
        assert!((pct - 20.0).abs() < 0.01);

        let pct_zero = tester.run_benchmarks(0.0, 10.0);
        assert_eq!(pct_zero, 0.0);
    }

    #[test]
    fn test_run_serialization() {
        let run = TestRun {
            patch_id: Uuid::new_v4(),
            compile_success: true,
            tests_passed: 42,
            tests_failed: 0,
            benchmark_before: 100.0,
            benchmark_after: 60.0,
            improvement_pct: 40.0,
            timestamp: 1700000000,
        };
        let json = serde_json::to_string(&run).unwrap();
        let back: TestRun = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch_id, run.patch_id);
        assert_eq!(back.tests_passed, 42);
    }

    #[test]
    fn get_test_runs_filters_by_id() {
        let mut tester = PatchTester::new();
        let mut patch1 = make_validated_patch();
        let patch1_id = patch1.id;
        tester
            .test_patch(&mut patch1, true, 5, 0, 10.0, 8.0)
            .unwrap();

        let mut patch2 = make_validated_patch();
        tester
            .test_patch(&mut patch2, true, 3, 0, 10.0, 9.0)
            .unwrap();

        let runs = tester.get_test_runs(patch1_id);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].patch_id, patch1_id);
    }
}
