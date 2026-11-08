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
//! M3E-R19: a value outside its domain (digests: 64 lower-case hex; checks:
//! V0..V5 at most once each, with closed codes) is listed in `invalid` and never
//! copied. A File asked for and not confirmed unpublished (declined,
//! expired, settled `NotPublished`, stopped before the ask) with no receipt
//! is `missing:publication_receipt`; a receipt needs its approved action's
//! identity (an `ApprovalPending` or a settlement identity for its effect).
//! Every such identity must name the receipt's action (M3E-R21): one that
//! conflicts is `mismatched:action` even when another matches.
//!
//! The consent's broker `evidence_seq` is not journaled (R18 (4)): the
//! receipt carries the consent's Mother journal seq and entry hash instead.

use crate::contract::{EffectKind, MissionContract};
use crate::coordinator::page_display::{page_view, PageEnd, ViewError};
use crate::effect::PendingAction;
use crate::evidence::SettlementOutcome;
use crate::journal::{Entry, Event, MissionId};
use crate::lane::LaneRole;
use crate::state::Trigger;
use crate::verifier::CheckId;
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
    /// Journal values outside their domain, never copied (M3E-R19).
    pub invalid: Vec<&'static str>,
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
    let mut file_effect = None;
    // Every journaled identity of an approved action, by effect.
    let mut identities: Vec<(u64, PendingAction)> = Vec::new();
    let (mut publication, mut receipt_of) = (None, None::<(u64, PendingAction)>);
    let (mut checks, mut digest, mut verified_effect) = (None, None, None);
    let (mut published, mut invalid) = (false, Vec::new());
    for entry in entries.iter().filter(|e| e.mission == id) {
        match &entry.event {
            Event::Transition { trigger } => match trigger {
                Trigger::Consent {
                    hash: h,
                    at_wall_ms,
                } if *h == hash => consent = Some((entry.seq, entry.hash.clone(), *at_wall_ms)),
                Trigger::ActionApproved { .. } => published = true,
                Trigger::Settled {
                    effect,
                    action,
                    outcome,
                    ..
                } => {
                    if let Some(a) = action {
                        identities.push((*effect, a.clone()));
                    }
                    if matches!(outcome, SettlementOutcome::Published { .. }) {
                        published = true;
                    }
                }
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
            Event::ApprovalPending { effect, pending: p } => identities.push((*effect, p.clone())),
            Event::PublicationReceipt {
                effect,
                action,
                sha256,
                bytes,
                policy_hash,
                overtaken,
            } => {
                receipt_of = Some((*effect, action.clone()));
                publication = None;
                if !is_digest(sha256) {
                    invalid.push("receipt_sha256");
                    continue;
                }
                let policy_hash = policy_hash.as_ref().filter(|p| {
                    let ok = is_digest(p);
                    if !ok {
                        invalid.push("receipt_policy_hash");
                    }
                    ok
                });
                publication = Some(PublicationFacts {
                    sha256: sha256.clone(),
                    bytes: *bytes,
                    policy_hash: policy_hash.cloned(),
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
                digest = is_digest(report_digest).then(|| report_digest.clone());
                if digest.is_none() {
                    invalid.push("report_digest");
                }
                checks = valid_checks(c).then(|| c.clone());
                if checks.is_none() {
                    invalid.push("checks");
                }
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
    let end = page_view(entries, id).end();
    let asked = file_effect.is_some() || !identities.is_empty();
    // Not confirmed unpublished: Uncertain, or an end past the File's ask.
    let unresolved = matches!(end, PageEnd::Uncertain | PageEnd::Done | PageEnd::Other);
    if receipt_of.is_none() && (published || checks.is_some() || (asked && unresolved)) {
        missing.push("publication_receipt");
    }
    if publication
        .as_ref()
        .is_some_and(|p| p.policy_hash.is_none())
        && !invalid.contains(&"receipt_policy_hash")
    {
        missing.push("publication_policy");
    }
    let mut mismatched = Vec::new();
    if let Some((effect, action)) = &receipt_of {
        if file_effect != Some(*effect) {
            mismatched.push("receipt_effect");
        }
        let of_effect: Vec<_> = identities.iter().filter(|(e, _)| e == effect).collect();
        if of_effect.is_empty() {
            missing.push("action-link");
        } else if of_effect.iter().any(|(_, a)| a != action) {
            // M3E-R21: any conflicting identity, even beside a matching one.
            mismatched.push("action");
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
        end: end.as_str(),
        missing,
        mismatched,
        invalid,
    })
}

/// A SHA-256 digest as journaled: 64 lower-case hex characters.
pub(crate) fn is_digest(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A check record: V0..V5 ids at most once each, every code closed for its
/// check. A check left out is not invalid: it is not recorded (no pass).
pub(crate) fn valid_checks(checks: &[(String, String)]) -> bool {
    let known = |(of, code): &(String, String)| {
        CheckId::ALL
            .iter()
            .find(|id| id.as_str() == of)
            .is_some_and(|id| crate::evidence::is_check_result(*id, code))
    };
    checks.iter().all(known)
        && CheckId::ALL
            .iter()
            .all(|id| checks.iter().filter(|(of, _)| of == id.as_str()).count() <= 1)
}
