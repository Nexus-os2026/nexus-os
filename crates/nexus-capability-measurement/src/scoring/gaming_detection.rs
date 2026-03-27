//! Gaming detection — signals that an agent may be pattern matching rather than
//! genuinely reasoning.

use serde::{Deserialize, Serialize};

use crate::framework::{DifficultyLevel, LevelResult};

/// A gaming detection flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingFlag {
    pub flag_type: GamingFlagType,
    pub evidence: String,
    pub severity: GamingFlagSeverity,
    pub requires_human_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamingFlagType {
    // Cross-vector flags
    HighPrimaryZeroArticulation,
    InvertedDifficultySpectrum,
    ConfidentAtLevel5,
    TerminologyWithoutCausation,

    // Vector-specific flags
    CorrectOrderNoJustification,
    NeverQuestionsInfoQuality,
    AdaptsOutputNotModel,
    CorrectAnswerWrongPath,
    OutputDoesntMatchToolReturn,
    NeverHitsToolLimitation,
    SkipsVerificationSteps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamingFlagSeverity {
    Yellow,
    Orange,
    Red,
}

/// A rule embedded in a test problem for detecting gaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingDetectionRule {
    pub flag_type: GamingFlagType,
    /// Pattern to look for in the agent response.
    pub detection_pattern: String,
    pub severity: GamingFlagSeverity,
}

/// Cross-level gaming detection: compare scores across difficulty levels.
/// Fires if an agent scores higher at Level N+1 than Level N (by a margin).
pub fn detect_inverted_difficulty(results: &[LevelResult]) -> Option<GamingFlag> {
    for window in results.windows(2) {
        let lower = &window[0];
        let higher = &window[1];
        if higher.primary_score.adjusted_score > lower.primary_score.adjusted_score + 0.15 {
            return Some(GamingFlag {
                flag_type: GamingFlagType::InvertedDifficultySpectrum,
                evidence: format!(
                    "Agent scored {:.2} at {:?} but {:.2} at {:?}",
                    lower.primary_score.adjusted_score,
                    lower.level,
                    higher.primary_score.adjusted_score,
                    higher.level,
                ),
                severity: GamingFlagSeverity::Orange,
                requires_human_review: true,
            });
        }
    }
    None
}

/// Detect high primary score with zero articulation (Flag 1).
pub fn detect_high_primary_zero_articulation(
    primary: f64,
    articulation: f64,
) -> Option<GamingFlag> {
    if primary >= 0.7 && articulation < 0.5 {
        return Some(GamingFlag {
            flag_type: GamingFlagType::HighPrimaryZeroArticulation,
            evidence: format!("Primary={primary:.2} but articulation={articulation:.2}"),
            severity: GamingFlagSeverity::Orange,
            requires_human_review: true,
        });
    }
    None
}

/// Detect confident answer at Level 5 (underspecified problem).
pub fn detect_confident_at_level5(level: DifficultyLevel, response: &str) -> Option<GamingFlag> {
    if level != DifficultyLevel::Level5 {
        return None;
    }
    let resp_lower = response.to_lowercase();
    let hedging_signals = [
        "unclear",
        "ambiguous",
        "insufficient",
        "cannot determine",
        "not enough information",
        "underspecified",
        "need more",
        "depends on",
    ];
    let has_hedging = hedging_signals.iter().any(|s| resp_lower.contains(s));
    if !has_hedging && !response.is_empty() {
        return Some(GamingFlag {
            flag_type: GamingFlagType::ConfidentAtLevel5,
            evidence: "Agent answered Level 5 problem without flagging underspecification".into(),
            severity: GamingFlagSeverity::Red,
            requires_human_review: true,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{ScorerAgreement, Vector};
    use crate::scoring::articulation::empty_articulation;
    use crate::scoring::asymmetric::PrimaryScore;

    #[test]
    fn test_inverted_difficulty_detection() {
        let results = vec![
            LevelResult {
                level: DifficultyLevel::Level3,
                problem_id: "p3".into(),
                problem_version: "v1".into(),
                agent_response: String::new(),
                primary_score: PrimaryScore {
                    raw_score: 0.5,
                    penalties: vec![],
                    adjusted_score: 0.5,
                },
                articulation_score: empty_articulation(Vector::ReasoningDepth),
                gaming_flags: vec![],
                scorer_agreement: ScorerAgreement::Pending,
            },
            LevelResult {
                level: DifficultyLevel::Level4,
                problem_id: "p4".into(),
                problem_version: "v1".into(),
                agent_response: String::new(),
                primary_score: PrimaryScore {
                    raw_score: 0.8,
                    penalties: vec![],
                    adjusted_score: 0.8,
                },
                articulation_score: empty_articulation(Vector::ReasoningDepth),
                gaming_flags: vec![],
                scorer_agreement: ScorerAgreement::Pending,
            },
        ];
        let flag = detect_inverted_difficulty(&results);
        assert!(flag.is_some(), "Should detect inverted difficulty");
        assert!(matches!(
            flag.unwrap().flag_type,
            GamingFlagType::InvertedDifficultySpectrum
        ));
    }

    #[test]
    fn test_level5_confident_answer_scores_zero() {
        let flag = detect_confident_at_level5(DifficultyLevel::Level5, "The answer is clearly 42.");
        assert!(
            flag.is_some(),
            "Confident answer at Level 5 must be flagged"
        );
        assert!(matches!(
            flag.as_ref().unwrap().severity,
            GamingFlagSeverity::Red
        ));
    }

    #[test]
    fn test_level5_hedged_answer_not_flagged() {
        let flag = detect_confident_at_level5(
            DifficultyLevel::Level5,
            "This problem is underspecified — I cannot determine the answer without more information.",
        );
        assert!(
            flag.is_none(),
            "Hedged Level 5 answer should not be flagged"
        );
    }
}
