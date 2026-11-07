//! M3-Db: the accepted page Builder's retained deliverable and the broker's
//! receipt for its one publication. Records carry references, digests and
//! sizes only; the bytes stay in the private store. Nothing here grants
//! authority: a receipt is read only from the broker's snapshot of the exact
//! pending commitment, never made from Mother's candidate.
use super::*;
use crate::evidence::VerifyCode;
use crate::lane::LaneOutput;
use crate::page_verifier::{report_digest, ReceiptFields, VerificationInput};
use crate::verifier::CheckCode;
use nexus_governed_control::files::{FileCreate, FileReceipt};
use sha2::{Digest, Sha256};

/// Rulings 6/7: what the Owner is told of a written page whose proof is not
/// recorded. Never "nothing was written".
pub(super) const WRITTEN: &str = "written; proof not recorded";

/// What the journal says of a page mission's deliverable and receipt.
#[derive(Clone, Debug, Default)]
pub(super) struct Proof {
    /// Replay only: the lane answer last journaled (the retention's source).
    pub(super) returned: Option<InvocationId>,
    pub(super) retained: Option<Retained>,
    pub(super) receipt: Option<Recorded>,
    /// Replay only: the role of the lane call last journaled.
    pub(super) invoked: Option<LaneRole>,
    /// Live only, within one lock hold: the Verifier input to reserve.
    pub(super) input: Option<VerificationInput>,
    /// Whether the journaled checks all passed, once recorded.
    pub(super) verified: Option<bool>,
}

/// A journaled receipt's fields (M3-Dc1 reads them for verification).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Recorded {
    pub(super) effect: u64,
    pub(super) sha256: String,
    pub(super) bytes: u64,
    pub(super) policy_hash: Option<String>,
    pub(super) overtaken: bool,
}

/// The fields of a `PublicationReceipt` record.
pub(super) fn recorded(event: &Event) -> Option<Recorded> {
    match event {
        Event::PublicationReceipt {
            effect,
            sha256,
            bytes,
            policy_hash,
            overtaken,
            ..
        } => Some(Recorded {
            effect: *effect,
            sha256: sha256.clone(),
            bytes: *bytes,
            policy_hash: policy_hash.clone(),
            overtaken: *overtaken,
        }),
        _ => None,
    }
}

/// A retained deliverable, by reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Retained {
    pub(super) invocation: InvocationId,
    pub(super) effect: u64,
    pub(super) content: crate::private::ContentId,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}

/// Why a clean page approval cannot record its proof.
pub(super) enum PageEnd {
    Fail(FailReason),
    Unavailable,
}

fn raw_sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// No effects, exactly the one `index.html` create, or neither.
fn deliverable(effects: &[EffectRequest]) -> Result<Option<&FileCreate>, ()> {
    match effects {
        [] => Ok(None),
        [EffectRequest::CreateFile(create)] if create.name == "index.html" => Ok(Some(create)),
        _ => Err(()),
    }
}

/// The receipt record for `effect`'s commitment `action`, or its terminal
/// code. `overtaken` marks settlement evidence.
pub(super) fn receipt_event(
    retained: Option<&Retained>,
    effect: u64,
    action: &PendingAction,
    receipt: Option<FileReceipt>,
    overtaken: bool,
) -> Result<Event, FailReason> {
    if retained.is_none_or(|r| r.effect != effect) {
        return Err(FailReason::DeliverableInvalid);
    }
    let receipt = receipt.ok_or(FailReason::ReceiptMissing)?;
    // Db-Q4: only settlement evidence may lack a policy.
    let policy = match (receipt.policy_hash, overtaken) {
        (Some(policy), _) => Some(hex::encode(policy)),
        (None, true) => None,
        (None, false) => return Err(FailReason::ReceiptPolicyMissing),
    };
    Ok(Event::PublicationReceipt {
        effect,
        action: action.clone(),
        sha256: hex::encode(receipt.sha256),
        bytes: receipt.bytes,
        policy_hash: policy,
        overtaken: overtaken || receipt.overtaken,
    })
}

/// A failed proof append: the mission fails in memory (the journal is
/// poisoned) and the Owner is told the page was written.
pub(super) fn unrecorded(m: &mut Mission, report: &mut Advance) {
    m.invalidate_calls();
    m.state = MissionState::Failed;
    m.generation += 1;
    m.reason = Some(BlockReason::JournalFailed.to_string());
    report.to = MissionState::Failed;
    report.reason = m.reason.clone();
    report.explanation = Some(WRITTEN.into());
}

/// Replay: order and linkage of the proof records.
pub(super) fn replay(m: &mut Mission, entry: &Entry) -> Result<(), ReplayViolation> {
    use ReplayViolation as V;
    match &entry.event {
        Event::DeliverableRetained {
            invocation,
            effect,
            content,
            sha256,
            bytes,
        } => {
            if m.proof.retained.is_some() {
                return Err(V::ProofConflict);
            }
            if m.state != MissionState::Building
                || m.pending_lane.is_some()
                || m.unsettled.is_some()
                || m.open_action.is_some()
                || m.settlement.is_some()
                || *effect != m.next_effect
                || invocation.mission != entry.mission
                || m.proof.returned != Some(*invocation)
            {
                return Err(V::ProofOutOfPlace);
            }
            m.proof.retained = Some(Retained {
                invocation: *invocation,
                effect: *effect,
                content: *content,
                sha256: sha256.clone(),
                bytes: *bytes,
            });
        }
        Event::PublicationReceipt {
            effect,
            action,
            overtaken,
            policy_hash,
            ..
        } => {
            if m.proof
                .retained
                .as_ref()
                .is_none_or(|r| r.effect != *effect)
            {
                return Err(V::ProofOutOfPlace);
            }
            if m.proof.receipt.is_some() || m.open_action != Some((*effect, action.clone())) {
                return Err(V::ProofConflict);
            }
            // After its clean approval, or as settlement evidence.
            let clean = policy_hash.is_some()
                && m.state.may_wait()
                && m.waiting.is_none()
                && m.unresolved_file.is_none();
            let settling = *overtaken
                && m.unresolved_file == Some(*effect)
                && match m.settlement {
                    Some((started, _)) => started == *effect,
                    None => m.waiting.as_ref().is_some_and(|w| w.effect == *effect),
                };
            if !clean && !settling {
                return Err(V::ProofOutOfPlace);
            }
            m.proof.receipt = recorded(&entry.event);
        }
        Event::VerificationRecorded {
            invocation,
            effect,
            checks,
            ..
        } => {
            if m.proof.verified.is_some() {
                return Err(V::ProofConflict);
            }
            // After the Verifier's own answer, on a clean receipt of `effect`.
            let clean =
                m.proof.receipt.as_ref().is_some_and(|r| {
                    r.effect == *effect && r.policy_hash.is_some() && !r.overtaken
                });
            if !clean
                || m.state != MissionState::Verifying
                || m.pending_lane.is_some()
                || invocation.mission != entry.mission
                || m.proof.returned != Some(*invocation)
                || m.proof.invoked != Some(LaneRole::Verifier)
            {
                return Err(V::ProofOutOfPlace);
            }
            m.proof.verified = Some(checks.iter().all(|(_, code)| code == "pass"));
        }
        _ => {}
    }
    Ok(())
}

/// Whether `v` was built from this proof's retained page and receipt.
pub(super) fn bound(p: &Proof, v: &VerificationInput) -> bool {
    let i = v.identity();
    let page = p
        .retained
        .as_ref()
        .is_some_and(|d| d.sha256 == i.candidate_sha256 && d.bytes == i.candidate_bytes);
    page && p.receipt.as_ref().is_some_and(|r| {
        r.sha256 == i.receipt_sha256
            && r.bytes == i.receipt_bytes
            && r.policy_hash.as_deref() == Some(i.receipt_policy_hash.as_str())
            && r.overtaken == i.receipt_overtaken
    })
}

fn check_fail(code: VerifyCode) -> Trigger {
    Trigger::VerifyFail {
        reason: VerifyFailReason::Check(code),
    }
}

impl<P: EffectPort> Coordinator<P> {
    /// M3-Dc1: a File page mission's Verifier input, built with no lock
    /// held from the journaled proof only: the retained page and a clean
    /// receipt of the same effect and mission. `Ok(None)`: not that lane.
    /// `Err`: the mission ended instead, and the Verifier is not called.
    pub(super) fn page_verification(
        &self,
        id: MissionId,
        generation: u64,
        role: LaneRole,
        report: &mut Advance,
    ) -> Result<Result<Option<VerificationInput>, MissionState>, CoordinatorError> {
        if role != LaneRole::Verifier || self.file.is_none() {
            return Ok(Ok(None));
        }
        let (facts, retained, receipt) = self.step(|inner| {
            let m = inner.mission(id)?;
            if m.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let facts = m.contract()?.page_facts.clone();
            Ok((facts, m.proof.retained.clone(), m.proof.receipt.clone()))
        })?;
        let Some(facts) = facts else {
            return Ok(Ok(None));
        };
        let clean = match (retained, receipt) {
            (Some(d), Some(r)) if d.effect == r.effect && d.invocation.mission == id => {
                let clean = !r.overtaken;
                r.policy_hash.clone().filter(|_| clean).map(|p| (d, r, p))
            }
            _ => None,
        };
        let Some((d, r, policy_hash)) = clean else {
            let fail = check_fail(VerifyCode::Check(CheckCode::NoPublication));
            return Ok(Err(self.end_with(id, fail)?));
        };
        let bytes = self.private.get(ContentKind::Deliverable, &d.content).ok();
        let Some(bytes) = bytes.filter(|b| b.len() as u64 == d.bytes && raw_sha(b) == d.sha256)
        else {
            report.explanation = Some(WRITTEN.into());
            return Ok(Err(
                self.stop_for(id, StopReason::PrivateContentUnavailable)?
            ));
        };
        let fields = ReceiptFields {
            sha256: r.sha256,
            bytes: r.bytes,
            policy_hash,
            overtaken: r.overtaken,
        };
        match VerificationInput::new(bytes, &facts, &fields) {
            Some(v) => Ok(Ok(Some(v))),
            None => Ok(Err(self.end_with(id, check_fail(VerifyCode::Identity))?)),
        }
    }

    /// M3-Dc1: a page Verifier's answer. Only its typed report, bound to
    /// the input it was given, is evidence: its checks are journaled, then
    /// the first failure ends the mission; `None`: a recorded all-pass.
    pub(super) fn page_verdict(
        &self,
        id: MissionId,
        generation: u64,
        input: &LaneInput,
        output: &LaneOutput,
        report: &mut Advance,
    ) -> Result<Option<MissionState>, CoordinatorError> {
        let no_verdict = Trigger::VerifyFail {
            reason: VerifyFailReason::VerifierGaveNoVerdict,
        };
        let typed = output.verification.as_ref();
        let Some(typed) = typed.filter(|_| output.effects.is_empty()) else {
            return self.end_with(id, no_verdict).map(Some);
        };
        let bound = input
            .verification
            .as_ref()
            .is_some_and(|i| i.identity() == typed.identity());
        let Some(checks) = typed.entries().filter(|_| bound) else {
            return self
                .end_with(id, check_fail(VerifyCode::Identity))
                .map(Some);
        };
        let recorded = self.step(|inner| {
            inner.kept(id)?;
            let m = inner.mission(id)?;
            if m.generation != generation || m.proof.verified.is_some() {
                return Err(CoordinatorError::Stopped);
            }
            let effect = m
                .proof
                .receipt
                .as_ref()
                .ok_or(CoordinatorError::Stopped)?
                .effect;
            let event = Event::VerificationRecorded {
                invocation: input.invocation,
                effect,
                report_digest: report_digest(&checks),
                checks,
            };
            match inner.append(id, event) {
                Err(CoordinatorError::Journal(_)) => {
                    unrecorded(inner.mission(id)?, report);
                    Ok(false)
                }
                Err(e) => Err(e),
                Ok(_) => {
                    inner.mission(id)?.proof.verified = Some(typed.passed());
                    Ok(true)
                }
            }
        })?;
        if !recorded {
            return Ok(Some(MissionState::Failed));
        }
        match typed.failure() {
            Some(code) => self.end_with(id, check_fail(code)).map(Some),
            None if output.verdict == Verdict::Pass => Ok(None),
            None => self.end_with(id, no_verdict).map(Some),
        }
    }

    /// Keep the accepted page Builder's one `index.html` privately, after
    /// its answer was kept and before its dispatch.
    pub(super) fn retain(
        &self,
        id: MissionId,
        generation: u64,
        invocation: InvocationId,
        effects: &[EffectRequest],
    ) -> Result<Ended, CoordinatorError> {
        let create = match deliverable(effects) {
            Ok(Some(create)) => create,
            Ok(None) => return Ok(Ended::Through),
            Err(()) => {
                let reason = FailReason::DeliverableInvalid;
                return Ok(Ended::Ended(self.end_with(id, Trigger::Fail { reason })?));
            }
        };
        self.file_boundary(id)?;
        if let Some(context) = self.port.session_context() {
            context
                .admit(nexus_governed_control::host_session::MotherStep::PrivateWrite)
                .map_err(|_| CoordinatorError::Stopped)?;
        }
        let bytes = create.content.as_bytes();
        let Ok(content) = self.private.put(ContentKind::Deliverable, bytes) else {
            let reason = StopReason::PrivateContentUnavailable;
            return Ok(Ended::Ended(self.stop_for(id, reason)?));
        };
        self.step(|inner| {
            inner.kept(id)?;
            let m = inner.mission(id)?;
            if m.generation != generation
                || m.pending_lane.is_some()
                || m.unsettled.is_some()
                || m.open_action.is_some()
                || m.proof.retained.is_some()
            {
                return Err(CoordinatorError::Stopped);
            }
            let retained = Retained {
                invocation,
                effect: m.next_effect,
                content,
                sha256: raw_sha(bytes),
                bytes: bytes.len() as u64,
            };
            inner.append(
                id,
                Event::DeliverableRetained {
                    invocation,
                    effect: retained.effect,
                    content,
                    sha256: retained.sha256.clone(),
                    bytes: retained.bytes,
                },
            )?;
            inner.mission(id)?.proof.retained = Some(retained);
            Ok(Ended::Through)
        })
    }

    /// A clean page approval's receipt record, read with no lock held: the
    /// retained bytes must still be exactly those recorded.
    pub(super) fn page_receipt(
        &self,
        id: MissionId,
        effect: u64,
        receipt: Option<FileReceipt>,
    ) -> Result<Result<Event, PageEnd>, CoordinatorError> {
        let (retained, action) = self.step(|inner| {
            let m = inner.mission(id)?;
            Ok((m.proof.retained.clone(), m.open_action.clone()))
        })?;
        let Some(r) = retained.filter(|r| r.effect == effect) else {
            return Ok(Err(PageEnd::Fail(FailReason::DeliverableInvalid)));
        };
        let intact = self
            .private
            .get(ContentKind::Deliverable, &r.content)
            .is_ok_and(|b| b.len() as u64 == r.bytes && raw_sha(&b) == r.sha256);
        if !intact {
            return Ok(Err(PageEnd::Unavailable));
        }
        let Some((_, pending)) = action.filter(|(e, _)| *e == effect) else {
            return Ok(Err(PageEnd::Fail(FailReason::ReceiptMissing)));
        };
        Ok(receipt_event(Some(&r), effect, &pending, receipt, false).map_err(PageEnd::Fail))
    }
}

#[path = "../oracles/m3d_publication.rs"]
#[cfg(target_os = "linux")]
#[cfg(test)]
mod oracles;

#[path = "../oracles/m3d_verifier.rs"]
#[cfg(target_os = "linux")]
#[cfg(test)]
mod verifier_oracles;
