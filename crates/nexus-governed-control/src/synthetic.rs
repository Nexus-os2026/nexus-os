//! P4-UAT05, a Phase Four amendment the owner adopted: the owner's synthetic
//! approval probe.
//!
//! One owner command, `synthetic-receipt`, prepares an R2 commitment of kind
//! `synthetic` whose effect is one bounded text receipt in Nexus's own
//! records: no network, process, browser, connector, display or input
//! action, and nothing written outside Nexus's own evidence and audit
//! records (which it does write, as every governed action does). It exists
//! so that the native R2 approval path can be exercised in a shipped
//! desktop; it is never, and never reports itself as, a real-world action.
//!
//! Only a run the backend opened as an owner-session command's
//! ([`RunClass::Owner`]) prepares it, never an agent's run, whatever its
//! agent is called. Only a live, natively granted `synthetic` grant covers
//! it (at most [`GRANT_TTL`]), and every commitment (at most
//! [`COMMITMENT_TTL`]) still needs its own native approval. Its class is
//! fixed here; its target and its receipt nonce are the backend's; the
//! caller chooses nothing. The authority core, the pipeline and the native
//! dialogs are unchanged.

use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::ids::{Digest, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunClass;
use crate::authority::{Authority, AuthorityError};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use getrandom::getrandom;
use hex::encode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The probe's command: it carries nothing the caller chooses.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SyntheticIntent {}

/// The probe's operation.
pub const OPERATION: &str = "synthetic.receipt";
/// The longest one of its commitments waits for the owner's approval.
pub const COMMITMENT_TTL: Duration = Duration::from_secs(180);
/// The longest one of its grants lasts: a test grant, never a standing one.
pub const GRANT_TTL: Duration = Duration::from_secs(15 * 60);
/// Its target: there is none outside Nexus.
pub const TARGET_TEXT: &str = "none (synthetic test receipt; no external target)";

/// The probe's grant scope.
pub(crate) fn grant_scope() -> GrantScope {
    GrantScope::Synthetic
}

/// `ttl` for a grant of `scope`: a synthetic grant lasts at most
/// [`GRANT_TTL`]; any other is left as it is.
pub(crate) fn capped(scope: &GrantScope, ttl: Duration) -> Duration {
    if scope.kind() == CapabilityKind::Synthetic {
        ttl.min(GRANT_TTL)
    } else {
        ttl
    }
}

/// Whether a prepared action may be committed: the harmless wording of the
/// native window belongs to the exact pair (`synthetic`, R2) only, so an
/// action of kind `synthetic` with any other class is refused here, before
/// a commitment exists (an R0 or R1 one would otherwise run without the
/// owner's native approval). Every other kind passes unchanged.
pub(crate) fn admit(action: &PreparedAction) -> Result<(), AuthorityError> {
    if action.kind == CapabilityKind::Synthetic && action.class != EffectClass::R2 {
        return Err(AuthorityError::InvalidAction(
            "a synthetic approval test is classed R2 only",
        ));
    }
    Ok(())
}

/// The constant target identity: nothing outside Nexus.
fn target_digest() -> Digest {
    Digest::of("nexus.p3.synthetic.target.v1", &[b"none"])
}

/// What the owner reads in the request's details: five plain lines, the
/// receipt among them.
fn summary(receipt: &str) -> Vec<String> {
    vec![
        "SYNTHETIC TEST: Nexus performs no external effect".to_string(),
        "Allow issues one test receipt; nothing is written outside Nexus's own records".to_string(),
        "No network, process, browser, connector, display or input action".to_string(),
        format!("Receipt: {receipt}"),
        "A receipt is never evidence that a real-world action happened".to_string(),
    ]
}

/// Prepare one synthetic receipt for `run`: only an owner-session command's
/// run, only under a live synthetic grant, with a fresh receipt nonce.
pub(crate) fn prepare(
    authority: &Authority,
    run: RunId,
    _intent: &SyntheticIntent,
) -> Result<Preparation, AuthorityError> {
    // The owner's own command only, by the class the backend gave the run
    // when it opened it, before any grant is looked at: never by a name.
    if authority.runs().class(run) != Some(RunClass::Owner) {
        return Err(AuthorityError::Closed(
            "a synthetic approval test is an owner-session command only",
        ));
    }
    let grant = authority
        .grants()
        .live_of(CapabilityKind::Synthetic)
        .first()
        .map(|grant| grant.id)
        .ok_or(AuthorityError::NoCoveringGrant)?;
    let mut nonce = [0u8; 16];
    getrandom(&mut nonce)
        .map_err(|_| AuthorityError::Unavailable("no randomness for a receipt"))?;
    let receipt = format!("rcpt-{}", encode(nonce));
    let target = target_digest();
    // The receipt is bound: the commitment's parameters are its nonce.
    let parameters = Digest::of(
        "nexus.p3.synthetic.receipt.v1",
        &[OPERATION.as_bytes(), &nonce],
    );
    Ok(Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Synthetic,
            class: EffectClass::R2,
            operation: OPERATION,
            target: TargetIdentity {
                display: TARGET_TEXT.to_string(),
                digest: target,
            },
            parameters,
            grants: vec![grant],
            leases: vec![],
            summary: summary(&receipt),
        },
        effect: Box::new(SyntheticReceipt {
            receipt,
            target,
            parameters,
        }),
        ttl: COMMITMENT_TTL,
    })
}

/// The probe's pending effect: one receipt, issued once, inside Nexus.
struct SyntheticReceipt {
    receipt: String,
    target: Digest,
    parameters: Digest,
}

impl PendingEffect for SyntheticReceipt {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        if guard.is_cancelled() {
            return Err((
                FailureClass::Refused,
                "cancelled before the receipt was issued".to_string(),
            ));
        }
        if let Some(why) = guard.lapse() {
            return Err((FailureClass::Refused, why.to_string()));
        }
        Ok(EffectOutput {
            text: Some(format!(
                "SYNTHETIC TEST RECEIPT {} for {}: no external effect was performed.",
                self.receipt,
                guard.commitment()
            )),
            bytes: None,
            meta: vec![
                ("synthetic".to_string(), "true".to_string()),
                ("external_effect".to_string(), "none".to_string()),
                ("receipt".to_string(), self.receipt),
            ],
            evidence: None,
        })
    }
}

#[cfg(test)]
mod tests;
