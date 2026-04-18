//! Task profile vocabulary.
//!
//! A [`TaskProfile`] is the compact contract the Director attaches to each
//! DAG node. The [`Router`](crate::routing::Router) consumes profiles to pick
//! a concrete (provider, model) pair at runtime. Profiles never carry prompt
//! text — just the shape of the work.

use serde::{Deserialize, Serialize};

/// Privacy class of a task.
///
/// Hard deny rules, not downgrade rules:
/// - `StrictLocal` — may only run on providers whose `privacy_class ==
///   StrictLocal` (i.e. ollama). The router must never downgrade.
/// - `Sensitive` — also restricted to local providers.
/// - `Public` — any provider is eligible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyClass {
    Public,
    Sensitive,
    StrictLocal,
}

impl PrivacyClass {
    /// True if a task with `self` privacy may run on a provider with
    /// `provider` privacy. Local providers satisfy every class; cloud
    /// providers (Public) satisfy only `Public`.
    pub fn satisfied_by(self, provider: PrivacyClass) -> bool {
        match self {
            PrivacyClass::Public => true,
            PrivacyClass::Sensitive | PrivacyClass::StrictLocal => {
                matches!(provider, PrivacyClass::StrictLocal)
            }
        }
    }
}

/// Reasoning tier required by the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReasoningTier {
    Trivial,
    Light,
    Medium,
    Heavy,
    Expert,
}

/// Tool-use level required by the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ToolUseLevel {
    None,
    Basic,
    Advanced,
}

/// Latency class — a hint, not a hard gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LatencyClass {
    Interactive,
    Batch,
    Background,
}

/// Context-window size required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ContextSize {
    Small,  // ≤8K
    Medium, // ≤32K
    Large,  // ≤128K
    Huge,   // >128K
}

/// Cost class expresses willingness-to-spend, not measured cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CostClass {
    Free,
    Low,
    Standard,
    Premium,
}

/// The composite profile attached to a DAG node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskProfile {
    pub reasoning: ReasoningTier,
    pub tool_use: ToolUseLevel,
    pub latency: LatencyClass,
    pub context: ContextSize,
    pub privacy: PrivacyClass,
    pub cost: CostClass,
}

impl TaskProfile {
    /// A conservative default suitable for unit tests only.
    pub fn local_light() -> Self {
        Self {
            reasoning: ReasoningTier::Light,
            tool_use: ToolUseLevel::None,
            latency: LatencyClass::Batch,
            context: ContextSize::Small,
            privacy: PrivacyClass::StrictLocal,
            cost: CostClass::Free,
        }
    }

    pub fn public_heavy() -> Self {
        Self {
            reasoning: ReasoningTier::Heavy,
            tool_use: ToolUseLevel::Advanced,
            latency: LatencyClass::Batch,
            context: ContextSize::Large,
            privacy: PrivacyClass::Public,
            cost: CostClass::Standard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_strict_local_rejects_public_provider() {
        assert!(!PrivacyClass::StrictLocal.satisfied_by(PrivacyClass::Public));
    }

    #[test]
    fn privacy_sensitive_rejects_public_provider() {
        assert!(!PrivacyClass::Sensitive.satisfied_by(PrivacyClass::Public));
    }

    #[test]
    fn privacy_strict_local_accepts_local_provider() {
        assert!(PrivacyClass::StrictLocal.satisfied_by(PrivacyClass::StrictLocal));
    }

    #[test]
    fn privacy_public_accepts_anything() {
        assert!(PrivacyClass::Public.satisfied_by(PrivacyClass::Public));
        assert!(PrivacyClass::Public.satisfied_by(PrivacyClass::StrictLocal));
        assert!(PrivacyClass::Public.satisfied_by(PrivacyClass::Sensitive));
    }

    #[test]
    fn reasoning_tier_ordering() {
        assert!(ReasoningTier::Trivial < ReasoningTier::Light);
        assert!(ReasoningTier::Light < ReasoningTier::Medium);
        assert!(ReasoningTier::Medium < ReasoningTier::Heavy);
        assert!(ReasoningTier::Heavy < ReasoningTier::Expert);
    }

    #[test]
    fn profile_round_trips_json() {
        let p = TaskProfile::public_heavy();
        let j = serde_json::to_string(&p).expect("serialize");
        let back: TaskProfile = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(p, back);
    }
}
