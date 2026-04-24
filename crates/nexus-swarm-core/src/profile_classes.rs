//! Profile classification enums shared between the Provider trait
//! (capabilities, model descriptors) and the swarm's per-task profile.
//!
//! Only the three classes that `Provider` / `ProviderCapabilities` /
//! `ModelDescriptor` reference live here. `TaskProfile` and the rest of
//! the per-task classes (`ToolUseLevel`, `LatencyClass`, `ContextSize`)
//! stay in `nexus-swarm::profile` because they're orchestration-side
//! concerns that no agent crate needs to import.

use serde::{Deserialize, Serialize};

/// Reasoning capability tier used by `ModelDescriptor`. Ordered from
/// lightest to heaviest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReasoningTier {
    Trivial,
    Light,
    Medium,
    Heavy,
    Expert,
}

/// Privacy class on a task or a provider. Cloud providers MUST be
/// `Public`; `StrictLocal` and `Sensitive` only run on local providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyClass {
    Public,
    Sensitive,
    StrictLocal,
}

impl PrivacyClass {
    /// True if a task with `self` privacy may run on a provider with
    /// `provider` privacy. Local providers (StrictLocal) satisfy every
    /// class; cloud providers (Public) satisfy only `Public`.
    pub fn satisfied_by(self, provider: PrivacyClass) -> bool {
        match self {
            PrivacyClass::Public => true,
            PrivacyClass::Sensitive | PrivacyClass::StrictLocal => {
                matches!(provider, PrivacyClass::StrictLocal)
            }
        }
    }
}

/// Cost class — willingness-to-spend, not measured cost. Used by both
/// `ProviderCapabilities` and the per-task `TaskProfile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CostClass {
    Free,
    Low,
    Standard,
    Premium,
}
