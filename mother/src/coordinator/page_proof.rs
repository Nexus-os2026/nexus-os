//! M3-Db: the accepted page Builder's retained deliverable and the broker's
//! receipt for its one publication. Records carry references, digests and
//! sizes only; the bytes stay in the private store. Nothing here grants
//! authority: a receipt is read only from the broker's snapshot of the exact
//! pending commitment, never made from Mother's candidate.
use super::*;
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
    pub(super) receipt: Option<u64>,
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
            m.proof.receipt = Some(*effect);
        }
        _ => {}
    }
    Ok(())
}

impl<P: EffectPort> Coordinator<P> {
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
