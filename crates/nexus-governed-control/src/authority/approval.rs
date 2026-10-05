//! Native owner confirmation and the R2 approval it produces.
//!
//! The desktop's native dialogs are the only production `ControlConfirmer`.
//! An `R2Approval` exists only after the owner confirmed one exact
//! commitment: it cannot be constructed, cloned or deserialized outside this
//! crate, it names the commitment and its binding digest, and authorizing
//! the commitment consumes it.

use super::effect::{CapabilityKind, EffectClass};
use super::ids::{CommitmentId, Digest};

/// What the owner is asked to approve: one R2 commitment. Every line is
/// computed by the backend from the commitment it approves; nothing here is
/// a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionConfirmation {
    pub commitment: String,
    pub class: EffectClass,
    pub kind: CapabilityKind,
    pub operation: String,
    pub target: String,
    pub agent: String,
    pub run: String,
    pub summary: Vec<String>,
    pub expires_in_secs: u64,
    /// The first digits of the binding digest, so that two look-alike
    /// requests are visibly different.
    pub binding_short: String,
}

impl ActionConfirmation {
    pub fn title(&self) -> &'static str {
        "Allow this action?"
    }

    /// The text of the native confirmation.
    pub fn message(&self) -> String {
        let mut lines = vec![
            format!(
                "{} ({}): {}",
                self.class,
                self.class.meaning(),
                self.operation
            ),
            format!("Target: {}", self.target),
        ];
        lines.extend(self.summary.iter().cloned());
        lines.push(format!("Acting for: {} (run {})", self.agent, self.run));
        lines.push(format!(
            "Commitment: {} [{}], expires in {} s",
            self.commitment, self.binding_short, self.expires_in_secs
        ));
        lines.join("\n")
    }
}

/// What the owner is asked to grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantConfirmation {
    pub kind: CapabilityKind,
    pub lines: Vec<String>,
    pub expires_in_secs: u64,
}

impl GrantConfirmation {
    pub fn title(&self) -> &'static str {
        "Grant this capability?"
    }

    pub fn message(&self) -> String {
        let mut lines = self.lines.clone();
        lines.push("Any agent may use it until it expires or you revoke it".to_string());
        lines.push(format!("Expires in {} s", self.expires_in_secs));
        lines.join("\n")
    }
}

/// The backend's native confirmation. `true` only if the owner confirmed.
/// Everything that raises authority asks through it: a grant, an R2
/// approval, and lifting an emergency stop.
pub trait ControlConfirmer {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool;
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool;
    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool;
}

/// What the owner is asked before an emergency stop is lifted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeConfirmation {
    pub runs_cancelled_by_the_stop: usize,
}

impl ResumeConfirmation {
    pub fn title(&self) -> &'static str {
        "Resume governed control?"
    }

    pub fn message(&self) -> String {
        format!(
            "The emergency stop is in force. Resuming lets agents open new runs \
             and request actions again (each still needs its grants and, for \
             sensitive actions, your approval).\nRuns the stop cancelled stay \
             cancelled: {}.",
            self.runs_cancelled_by_the_stop
        )
    }
}

/// The owner's native approval of exactly one R2 commitment.
///
/// It has no `Clone`, `Copy`, `Default` or deserializer, and its fields are
/// private; `CommitmentRegistry::request_approval` is its only constructor,
/// and `CommitmentRegistry::authorize` consumes it.
#[derive(Debug)]
pub struct R2Approval {
    commitment: CommitmentId,
    binding: Digest,
}

impl R2Approval {
    /// Only after the owner confirmed the commitment natively.
    pub(crate) fn confirmed(commitment: CommitmentId, binding: Digest) -> Self {
        Self {
            commitment,
            binding,
        }
    }

    pub fn commitment(&self) -> CommitmentId {
        self.commitment
    }

    pub fn binding(&self) -> &Digest {
        &self.binding
    }
}
