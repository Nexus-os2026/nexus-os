//! Patch generation — creates optimized code patches from bottleneck analysis,
//! validates syntax, and prepares them for testing.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SelfRewriteError;

/// Status of a patch through its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchStatus {
    Generated,
    Validated,
    Testing,
    Tested,
    Approved,
    Applied,
    Reverted,
    Rejected,
}

/// A code patch targeting a specific function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub id: Uuid,
    pub target_file: String,
    pub target_function: String,
    pub original_code: String,
    pub optimized_code: String,
    pub optimization_goal: String,
    pub status: PatchStatus,
    pub created_at: u64,
    /// HITL approval required before application.  Always true.
    pub requires_approval: bool,
}

/// Diff representation of a patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchDiff {
    pub additions: Vec<String>,
    pub deletions: Vec<String>,
    pub context: String,
}

/// Generates optimized code patches based on profiling and analysis data.
#[derive(Debug, Clone)]
pub struct PatchGenerator {
    /// Forbidden keywords that must never appear in generated patches.
    forbidden_keywords: Vec<String>,
}

impl PatchGenerator {
    pub fn new() -> Self {
        Self {
            forbidden_keywords: vec![
                "unsafe".to_string(),
                "std::mem::transmute".to_string(),
                "std::ptr::".to_string(),
                "#[allow(unsafe_code)]".to_string(),
            ],
        }
    }

    /// Generate a patch that replaces `original_code` with `optimized_code`
    /// for the given function.
    ///
    /// In production this would call an LLM to generate the optimized code;
    /// here we accept the optimized code directly (or from an LLM gateway
    /// response).
    pub fn generate_patch(
        &self,
        target_file: &str,
        target_function: &str,
        original_code: &str,
        optimized_code: &str,
        optimization_goal: &str,
    ) -> Result<Patch, SelfRewriteError> {
        if original_code.is_empty() {
            return Err(SelfRewriteError::PatchGenerationFailed(
                "original code is empty".into(),
            ));
        }
        if optimized_code.is_empty() {
            return Err(SelfRewriteError::PatchGenerationFailed(
                "optimized code is empty".into(),
            ));
        }
        if original_code == optimized_code {
            return Err(SelfRewriteError::PatchGenerationFailed(
                "optimized code is identical to original".into(),
            ));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let patch = Patch {
            id: Uuid::new_v4(),
            target_file: target_file.to_string(),
            target_function: target_function.to_string(),
            original_code: original_code.to_string(),
            optimized_code: optimized_code.to_string(),
            optimization_goal: optimization_goal.to_string(),
            status: PatchStatus::Generated,
            created_at: now,
            requires_approval: true, // ALWAYS require HITL approval
        };

        Ok(patch)
    }

    /// Validate a patch for safety: no `unsafe` keyword, basic syntax checks.
    /// Returns the patch with status updated to `Validated` or an error.
    pub fn validate_syntax(&self, mut patch: Patch) -> Result<Patch, SelfRewriteError> {
        // Safety check 1: No forbidden keywords
        for keyword in &self.forbidden_keywords {
            if patch.optimized_code.contains(keyword.as_str()) {
                patch.status = PatchStatus::Rejected;
                return Err(SelfRewriteError::PatchValidationFailed(format!(
                    "patch contains forbidden keyword: {keyword}"
                )));
            }
        }

        // Safety check 2: Balanced braces
        let open_braces = patch.optimized_code.matches('{').count();
        let close_braces = patch.optimized_code.matches('}').count();
        if open_braces != close_braces {
            patch.status = PatchStatus::Rejected;
            return Err(SelfRewriteError::PatchValidationFailed(format!(
                "unbalanced braces: {open_braces} opening vs {close_braces} closing"
            )));
        }

        // Safety check 3: Balanced parentheses
        let open_parens = patch.optimized_code.matches('(').count();
        let close_parens = patch.optimized_code.matches(')').count();
        if open_parens != close_parens {
            patch.status = PatchStatus::Rejected;
            return Err(SelfRewriteError::PatchValidationFailed(format!(
                "unbalanced parentheses: {open_parens} opening vs {close_parens} closing"
            )));
        }

        // Safety check 4: Must contain `fn` keyword (it's replacing a function)
        if !patch.optimized_code.contains("fn ") {
            patch.status = PatchStatus::Rejected;
            return Err(SelfRewriteError::PatchValidationFailed(
                "patch does not contain a function definition".into(),
            ));
        }

        // Safety check 5: HITL approval must remain required
        if !patch.requires_approval {
            patch.status = PatchStatus::Rejected;
            return Err(SelfRewriteError::PatchValidationFailed(
                "HITL approval flag was disabled — this is not allowed".into(),
            ));
        }

        patch.status = PatchStatus::Validated;
        Ok(patch)
    }

    /// Compute the diff between original and optimized code.
    pub fn compute_diff(&self, patch: &Patch) -> PatchDiff {
        let original_lines: Vec<&str> = patch.original_code.lines().collect();
        let optimized_lines: Vec<&str> = patch.optimized_code.lines().collect();

        let deletions: Vec<String> = original_lines
            .iter()
            .filter(|l| !optimized_lines.contains(l))
            .map(|l| l.to_string())
            .collect();

        let additions: Vec<String> = optimized_lines
            .iter()
            .filter(|l| !original_lines.contains(l))
            .map(|l| l.to_string())
            .collect();

        PatchDiff {
            additions,
            deletions,
            context: format!(
                "{}::{} — {}",
                patch.target_file, patch.target_function, patch.optimization_goal
            ),
        }
    }
}

impl Default for PatchGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_valid_patch() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch(
                "src/lib.rs",
                "process",
                "fn process() { let v = Vec::new(); }",
                "fn process() { let v = Vec::with_capacity(100); }",
                "reduce allocations",
            )
            .unwrap();

        assert_eq!(patch.status, PatchStatus::Generated);
        assert!(patch.requires_approval);
        assert_eq!(patch.target_function, "process");
    }

    #[test]
    fn reject_empty_original() {
        let gen = PatchGenerator::new();
        let err = gen
            .generate_patch("f.rs", "f", "", "fn f() {}", "opt")
            .unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchGenerationFailed(_)));
    }

    #[test]
    fn reject_identical_code() {
        let gen = PatchGenerator::new();
        let err = gen
            .generate_patch("f.rs", "f", "fn f() {}", "fn f() {}", "opt")
            .unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchGenerationFailed(_)));
    }

    #[test]
    fn validate_rejects_unsafe() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "unsafe fn f() { }", "speed")
            .unwrap();
        let err = gen.validate_syntax(patch).unwrap_err();
        assert!(matches!(err, SelfRewriteError::PatchValidationFailed(_)));
    }

    #[test]
    fn validate_rejects_unbalanced_braces() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "fn f() { { }", "fix")
            .unwrap();
        let err = gen.validate_syntax(patch).unwrap_err();
        assert!(err.to_string().contains("unbalanced braces"));
    }

    #[test]
    fn validate_rejects_no_fn_keyword() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "let x = 5;", "fix")
            .unwrap();
        let err = gen.validate_syntax(patch).unwrap_err();
        assert!(err.to_string().contains("function definition"));
    }

    #[test]
    fn validate_passes_clean_patch() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch(
                "f.rs",
                "f",
                "fn f() { let v = Vec::new(); }",
                "fn f() { let v = Vec::with_capacity(16); }",
                "preallocate",
            )
            .unwrap();
        let validated = gen.validate_syntax(patch).unwrap();
        assert_eq!(validated.status, PatchStatus::Validated);
    }

    #[test]
    fn compute_diff_shows_changes() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch(
                "f.rs",
                "f",
                "fn f() {\n    let v = Vec::new();\n}",
                "fn f() {\n    let v = Vec::with_capacity(16);\n}",
                "preallocate",
            )
            .unwrap();
        let diff = gen.compute_diff(&patch);
        assert!(!diff.additions.is_empty());
        assert!(!diff.deletions.is_empty());
        assert!(diff.context.contains("preallocate"));
    }

    #[test]
    fn patch_serialization_roundtrip() {
        let gen = PatchGenerator::new();
        let patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "fn f() { return; }", "clarity")
            .unwrap();
        let json = serde_json::to_string(&patch).unwrap();
        let back: Patch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, patch.id);
        assert_eq!(back.target_function, "f");
        assert!(back.requires_approval);
    }

    #[test]
    fn hitl_approval_always_true() {
        let gen = PatchGenerator::new();
        let mut patch = gen
            .generate_patch("f.rs", "f", "fn f() { }", "fn f() { return; }", "opt")
            .unwrap();
        // Try to bypass HITL — should be rejected
        patch.requires_approval = false;
        let err = gen.validate_syntax(patch).unwrap_err();
        assert!(err.to_string().contains("HITL"));
    }
}
