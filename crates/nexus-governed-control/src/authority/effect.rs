//! Effect classes and capability kinds.

use std::fmt;

/// How consequential a real-world effect is.
///
/// - `R0`: local observation or computation; no external communication and
///   no mutation outside Nexus-controlled ephemeral state. Still needs a
///   capability grant.
/// - `R1`: a bounded external or reversible effect. Needs a grant that
///   covers it, a commitment, revalidation at execution, a bound and
///   evidence.
/// - `R2`: a sensitive, irreversible or privileged effect. Additionally needs
///   the owner's exact, one-shot native approval of the commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectClass {
    R0,
    R1,
    R2,
}

impl EffectClass {
    pub fn requires_native_approval(self) -> bool {
        matches!(self, EffectClass::R2)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EffectClass::R0 => "R0",
            EffectClass::R1 => "R1",
            EffectClass::R2 => "R2",
        }
    }

    /// What a person approving it must understand, in one line.
    pub fn meaning(self) -> &'static str {
        match self {
            EffectClass::R0 => "local observation or computation",
            EffectClass::R1 => "a bounded external or reversible effect",
            EffectClass::R2 => "a sensitive or irreversible effect",
        }
    }
}

impl fmt::Display for EffectClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The governed capability domain an action belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CapabilityKind {
    /// P3-A: a trusted, typed tool run as a contained process.
    Tool,
    /// P3-B: a network request to a granted destination.
    Egress,
    /// P3-C: a browser session below egress authority.
    Browser,
    /// P3-D: observing the agent display.
    Perception,
    /// P3-E: mouse and keyboard on the agent display.
    Input,
    /// P3-F: a typed connector operation.
    Connector,
}

impl CapabilityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityKind::Tool => "tool",
            CapabilityKind::Egress => "egress",
            CapabilityKind::Browser => "browser",
            CapabilityKind::Perception => "perception",
            CapabilityKind::Input => "input",
            CapabilityKind::Connector => "connector",
        }
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
