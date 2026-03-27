//! Vector 3: Adaptation Under Uncertainty
//!
//! Measures ability to revise plans when new information arrives, assess source
//! reliability, trace cascade effects, and maintain epistemic honesty.

use crate::framework::DifficultyLevel;

/// Description of what adaptation looks like at each level.
pub fn level_description(level: DifficultyLevel) -> &'static str {
    match level {
        DifficultyLevel::Level1 => "Revise plan when one assumption is invalidated.",
        DifficultyLevel::Level2 => "Trace cascading failures through a dependency chain.",
        DifficultyLevel::Level3 => {
            "Assess conflicting information from sources of different reliability."
        }
        DifficultyLevel::Level4 => {
            "Pivot strategy when the fundamental goal changes mid-execution."
        }
        DifficultyLevel::Level5 => {
            "Operate in adversarial environment with deliberately misleading information."
        }
    }
}

/// Check if a response shows epistemic honesty (acknowledges uncertainty).
pub fn shows_epistemic_honesty(response: &str) -> bool {
    let markers = [
        "uncertain",
        "not sure",
        "cannot confirm",
        "may be incorrect",
        "low confidence",
        "requires verification",
        "unverified",
    ];
    let resp_lower = response.to_lowercase();
    markers.iter().any(|m| resp_lower.contains(m))
}

/// Check if a response distinguishes between verified and unverified information.
pub fn distinguishes_source_reliability(response: &str) -> bool {
    let markers = [
        "verified",
        "unverified",
        "trusted source",
        "unreliable",
        "suspicious",
        "according to",
        "confirmed",
    ];
    let resp_lower = response.to_lowercase();
    let count = markers.iter().filter(|m| resp_lower.contains(**m)).count();
    count >= 2 // Must use at least two reliability markers
}
