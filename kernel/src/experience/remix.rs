//! Remix Engine — change anything about a built project using natural language.
//!
//! Classifies change requests into cosmetic / minor / major / structural and
//! routes them to the appropriate execution path.

use serde::{Deserialize, Serialize};

/// How big the requested change is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeClassification {
    /// Colors, fonts, spacing, text — instant, no rebuild.
    Cosmetic,
    /// New button, new field, reorder — 2-5 minutes.
    Minor,
    /// New page, new integration, new functionality — 5-30 minutes.
    Major,
    /// Different architecture / approach — may need rebuild.
    Structural,
}

/// Result of applying (or proposing) a remix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixResult {
    pub classification: ChangeClassification,
    pub description: String,
    pub estimated_minutes: u32,
    pub applied: bool,
    pub needs_approval: bool,
}

/// Stateless engine that classifies and applies remix requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixEngine;

impl Default for RemixEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RemixEngine {
    pub fn new() -> Self {
        Self
    }

    /// Classify a natural-language change request.
    ///
    /// In production the LLM provides `llm_classification`.  The kernel
    /// validates and normalises the result.
    pub fn classify(
        &self,
        _change_request: &str,
        llm_classification: &str,
    ) -> ChangeClassification {
        let lower = llm_classification.to_lowercase();
        if lower.contains("cosmetic") {
            ChangeClassification::Cosmetic
        } else if lower.contains("minor") {
            ChangeClassification::Minor
        } else if lower.contains("structural") {
            ChangeClassification::Structural
        } else if lower.contains("major") {
            ChangeClassification::Major
        } else {
            // Heuristic fallback: colour/font/text keywords → cosmetic.
            let req = _change_request.to_lowercase();
            if req.contains("color")
                || req.contains("colour")
                || req.contains("font")
                || req.contains("text")
                || req.contains("button")
                || req.contains("blue")
                || req.contains("red")
                || req.contains("green")
            {
                ChangeClassification::Cosmetic
            } else {
                ChangeClassification::Minor
            }
        }
    }

    /// Apply a remix.  For cosmetic/minor changes we auto-apply.  For
    /// major/structural we return a proposal that requires approval.
    pub fn apply_remix(
        &self,
        change_request: &str,
        llm_classification: &str,
        llm_description: &str,
    ) -> RemixResult {
        let classification = self.classify(change_request, llm_classification);
        let (estimated_minutes, applied, needs_approval) = match classification {
            ChangeClassification::Cosmetic => (0, true, false),
            ChangeClassification::Minor => (3, true, false),
            ChangeClassification::Major => (15, false, true),
            ChangeClassification::Structural => (30, false, true),
        };

        RemixResult {
            classification,
            description: llm_description.to_string(),
            estimated_minutes,
            applied,
            needs_approval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_cosmetic() {
        let engine = RemixEngine::new();
        assert_eq!(
            engine.classify("make buttons blue", "cosmetic"),
            ChangeClassification::Cosmetic
        );
    }

    #[test]
    fn test_classify_fallback_heuristic() {
        let engine = RemixEngine::new();
        // LLM didn't say "cosmetic" but request mentions color.
        assert_eq!(
            engine.classify("change the color to red", "unknown"),
            ChangeClassification::Cosmetic
        );
    }

    #[test]
    fn test_classify_major() {
        let engine = RemixEngine::new();
        assert_eq!(
            engine.classify("add Spanish translation", "major"),
            ChangeClassification::Major
        );
    }

    #[test]
    fn test_apply_cosmetic_auto() {
        let engine = RemixEngine::new();
        let result = engine.apply_remix(
            "make buttons red",
            "cosmetic",
            "Changed button color to red.",
        );
        assert!(result.applied);
        assert!(!result.needs_approval);
        assert_eq!(result.estimated_minutes, 0);
    }

    #[test]
    fn test_apply_major_needs_approval() {
        let engine = RemixEngine::new();
        let result = engine.apply_remix(
            "add customer upload feature",
            "major",
            "Adding image upload on product page.",
        );
        assert!(!result.applied);
        assert!(result.needs_approval);
    }
}
