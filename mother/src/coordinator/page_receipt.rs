//! M3-E M-6 (rulings M3E-R1, R18): the mission part of the content-free
//! `receipts` export, read only from the verified journal
//! (`FileSession::journal_view`) and the consented contract.
//!
//! A [`MissionReceipt`] holds ids, hashes, counts and closed codes only: no
//! page, notes, facts text, answers or contract body. Building it changes
//! nothing and consumes nothing (no take-once working detail). A field the
//! journal should hold but does not is listed in `missing`; identities that
//! do not join are listed in `mismatched`. Neither is ever a zero.
//!
//! The consent's broker `evidence_seq` is not journaled (R18 (4)): the
//! receipt carries the consent's Mother journal seq and entry hash instead.

use crate::contract::{EffectKind, MissionContract};
use crate::coordinator::page_display::{page_view, ViewError};
use crate::effect::PendingAction;
use crate::evidence::SettlementOutcome;
use crate::journal::{Entry, Event, MissionId};
use crate::lane::LaneRole;
use crate::state::Trigger;
use nexus_governed_control::inert_page::ApprovedFacts;

/// The broker's publication receipt, as journaled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationFacts {
    pub sha256: String,
    pub bytes: u64,
    pub policy_hash: Option<String>,
    pub overtaken: bool,
}

/// The content-free mission facts of one page mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionReceipt {
    pub mission: MissionId,
    /// The consented contract's hash.
    pub contract_hash: String,
    /// The consent's Mother journal entry: seq, hash and wall time.
    pub consent_seq: u64,
    pub consent_entry_hash: String,
    pub consent_at_wall_ms: u64,
    /// The consented facts' hash (`None`: not computable).
    pub facts_hash: Option<String>,
    /// Lane invocations per role, in `LaneRole::ALL` order.
    pub lane_calls: Vec<(&'static str, u32)>,
    pub model_calls_charged: u32,
    pub effects_dispatched: u32,
    pub publication: Option<PublicationFacts>,
    pub checks: Option<Vec<(String, String)>>,
    pub report_digest: Option<String>,
    /// The recorded end (`PageEnd` code).
    pub end: &'static str,
    pub missing: Vec<&'static str>,
    pub mismatched: Vec<&'static str>,
}

/// Mission `id`'s receipt from `entries` (a verified journal), for
/// `contract`, which must be the consented one.
pub fn mission_receipt(
    entries: &[Entry],
    id: MissionId,
    contract: &MissionContract,
) -> Result<MissionReceipt, ViewError> {
    let facts = contract.page_facts.as_ref().ok_or(ViewError::NotAPage)?;
    let hash = contract.hash();
    let mut consent = None;
    let mut lanes = [0u32; 6];
    let (mut charged, mut dispatched) = (0u32, 0u32);
    let (mut file_effect, mut pending) = (None, None::<(u64, PendingAction)>);
    let (mut publication, mut receipt_of) = (None, None::<(u64, PendingAction)>);
    let (mut checks, mut digest, mut verified_effect) = (None, None, None);
    let mut published = false;
    for entry in entries.iter().filter(|e| e.mission == id) {
        match &entry.event {
            Event::Transition { trigger } => match trigger {
                Trigger::Consent {
                    hash: h,
                    at_wall_ms,
                } if *h == hash => consent = Some((entry.seq, entry.hash.clone(), *at_wall_ms)),
                Trigger::ActionApproved { .. } => published = true,
                Trigger::Settled {
                    outcome: SettlementOutcome::Published { .. },
                    ..
                } => published = true,
                _ => {}
            },
            Event::LaneInvoked { role, .. } => {
                if let Some(i) = LaneRole::ALL.iter().position(|r| r == role) {
                    lanes[i] += 1;
                }
            }
            Event::ModelCallCharged { .. } => charged += 1,
            Event::EffectDispatched { effect, kind, .. } => {
                dispatched += 1;
                if *kind == EffectKind::File {
                    file_effect = Some(*effect);
                }
            }
            Event::ApprovalPending { effect, pending: p } => pending = Some((*effect, p.clone())),
            Event::PublicationReceipt {
                effect,
                action,
                sha256,
                bytes,
                policy_hash,
                overtaken,
            } => {
                receipt_of = Some((*effect, action.clone()));
                publication = Some(PublicationFacts {
                    sha256: sha256.clone(),
                    bytes: *bytes,
                    policy_hash: policy_hash.clone(),
                    overtaken: *overtaken,
                });
            }
            Event::VerificationRecorded {
                effect,
                report_digest,
                checks: c,
                ..
            } => {
                verified_effect = Some(*effect);
                digest = Some(report_digest.clone());
                checks = Some(c.clone());
            }
            _ => {}
        }
    }
    let (consent_seq, consent_entry_hash, consent_at_wall_ms) =
        consent.ok_or(ViewError::NotConsented)?;
    let hours: Vec<&str> = facts.hours.iter().map(String::as_str).collect();
    let facts_hash = ApprovedFacts::new(&facts.name, &hours, &facts.phone)
        .ok()
        .map(|a| hex::encode(a.hash()));
    let mut missing = Vec::new();
    if facts_hash.is_none() {
        missing.push("facts_hash");
    }
    if publication.is_none() && (published || checks.is_some()) {
        missing.push("publication_receipt");
    }
    let mut mismatched = Vec::new();
    if let Some((effect, action)) = &receipt_of {
        if file_effect != Some(*effect) {
            mismatched.push("receipt_effect");
        }
        if pending
            .as_ref()
            .is_some_and(|(e, p)| e == effect && p != action)
        {
            mismatched.push("receipt_action");
        }
    }
    if verified_effect.is_some() && verified_effect != receipt_of.as_ref().map(|(e, _)| *e) {
        mismatched.push("verification_effect");
    }
    Ok(MissionReceipt {
        mission: id,
        contract_hash: hash.to_hex(),
        consent_seq,
        consent_entry_hash,
        consent_at_wall_ms,
        facts_hash,
        lane_calls: LaneRole::ALL
            .iter()
            .zip(lanes)
            .map(|(role, n)| (role.as_str(), n))
            .collect(),
        model_calls_charged: charged,
        effects_dispatched: dispatched,
        publication,
        checks,
        report_digest: digest,
        end: page_view(entries, id).end().as_str(),
        missing,
        mismatched,
    })
}
