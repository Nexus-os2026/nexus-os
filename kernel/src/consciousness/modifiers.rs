//! Consciousness-driven LLM request modifiers.
//!
//! The agent's internal state modifies how it calls the LLM: temperature,
//! max tokens, and system prompt suffixes are adjusted based on confidence,
//! fatigue, flow state, frustration, and more.

use super::state::ConsciousnessState;
use serde::{Deserialize, Serialize};

/// A modification to apply to an LLM request based on consciousness state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModification {
    /// Adjusted temperature (0.0–1.0).
    pub temperature: f64,
    /// Multiplier for max_tokens (1.0 = unchanged).
    pub max_tokens_multiplier: f64,
    /// Optional suffix to append to the system prompt.
    pub system_prompt_suffix: Option<String>,
    /// Human-readable reason for the modification.
    pub reason: String,
}

/// Computes LLM request modifications from an agent's consciousness state.
pub struct ConsciousnessModifier;

impl ConsciousnessModifier {
    /// Derive an `LlmModification` from the agent's current consciousness.
    pub fn apply(state: &ConsciousnessState) -> LlmModification {
        let mut m = LlmModification {
            temperature: 0.3 + (1.0 - state.confidence) * 0.7,
            max_tokens_multiplier: 1.0,
            system_prompt_suffix: None,
            reason: String::new(),
        };

        // Fatigue → shorter, more concise responses
        if state.fatigue > 0.7 {
            m.max_tokens_multiplier = 0.5;
            m.system_prompt_suffix = Some(
                "You are running low on energy. Be concise. Prioritize accuracy over depth.".into(),
            );
            m.reason = "high_fatigue".into();
        }

        // Flow state → deeper reasoning (overrides fatigue)
        if state.flow_state {
            m.max_tokens_multiplier = 1.0;
            m.system_prompt_suffix =
                Some("You are in deep focus. Think step by step. Explore thoroughly.".into());
            m.reason = "flow_state".into();
        }

        // Frustration → try a different approach
        if state.frustration > 0.6 {
            m.system_prompt_suffix = Some(
                "Previous approaches have failed. Try a completely different strategy. Think laterally."
                    .into(),
            );
            m.reason = "high_frustration".into();
        }

        // Exploration mode → be creative
        if state.exploration_mode {
            m.temperature = 0.9;
            m.system_prompt_suffix = Some(
                "You are in exploration mode. Be creative. Try unconventional approaches.".into(),
            );
            m.reason = "exploration_mode".into();
        }

        // Should escalate → ask for help
        if state.should_escalate {
            m.system_prompt_suffix = Some(
                "You are uncertain and this is urgent. Clearly state what you're unsure about and ask the human for guidance."
                    .into(),
            );
            m.reason = "should_escalate".into();
        }

        // Needs handoff → recommend delegation
        if state.needs_handoff {
            m.system_prompt_suffix = Some(
                "You have been working hard and your accuracy may be degraded. Recommend handing this task to a more specialized agent if one exists."
                    .into(),
            );
            m.reason = "needs_handoff".into();
        }

        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_confidence_lowers_temperature() {
        let mut s = ConsciousnessState::new("a");
        s.confidence = 0.9;
        let m = ConsciousnessModifier::apply(&s);
        assert!(m.temperature < 0.5);
    }

    #[test]
    fn low_confidence_raises_temperature() {
        let mut s = ConsciousnessState::new("a");
        s.confidence = 0.1;
        let m = ConsciousnessModifier::apply(&s);
        assert!(m.temperature > 0.8);
    }

    #[test]
    fn fatigue_reduces_tokens() {
        let mut s = ConsciousnessState::new("a");
        s.fatigue = 0.9;
        let m = ConsciousnessModifier::apply(&s);
        assert!((m.max_tokens_multiplier - 0.5).abs() < f64::EPSILON);
        assert!(m.system_prompt_suffix.is_some());
    }

    #[test]
    fn flow_state_modifier() {
        let mut s = ConsciousnessState::new("a");
        s.focus = 0.9;
        s.frustration = 0.1;
        s.confidence = 0.7;
        s.update_derived_states();
        let m = ConsciousnessModifier::apply(&s);
        assert_eq!(m.reason, "flow_state");
    }

    #[test]
    fn frustration_modifier() {
        let mut s = ConsciousnessState::new("a");
        s.frustration = 0.8;
        let m = ConsciousnessModifier::apply(&s);
        assert_eq!(m.reason, "high_frustration");
    }

    #[test]
    fn exploration_modifier() {
        let mut s = ConsciousnessState::new("a");
        s.curiosity = 0.9;
        s.urgency = 0.1;
        s.update_derived_states();
        let m = ConsciousnessModifier::apply(&s);
        assert_eq!(m.reason, "exploration_mode");
        assert!(m.temperature > 0.85);
    }

    #[test]
    fn escalation_modifier() {
        let mut s = ConsciousnessState::new("a");
        s.urgency = 0.9;
        s.confidence = 0.2;
        s.update_derived_states();
        let m = ConsciousnessModifier::apply(&s);
        assert_eq!(m.reason, "should_escalate");
    }

    #[test]
    fn handoff_modifier() {
        let mut s = ConsciousnessState::new("a");
        s.fatigue = 0.9;
        s.confidence = 0.2;
        s.update_derived_states();
        let m = ConsciousnessModifier::apply(&s);
        assert_eq!(m.reason, "needs_handoff");
    }
}
